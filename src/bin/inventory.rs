use std::fs;
use std::path::Path;

#[derive(serde::Serialize)]
struct Inventory {
    tables: Vec<TableInfo>,
    relationships: Vec<RelInfo>,
    roles: Vec<RoleInfo>,
    summary: Summary,
}

#[derive(serde::Serialize)]
struct TableInfo {
    name: String,
    table_type: String,
    description: String,
    columns: Vec<ColumnInfo>,
    measures: Vec<MeasureInfo>,
    partitions: Vec<PartitionInfo>,
    hierarchies: Vec<String>,
}

#[derive(serde::Serialize)]
struct ColumnInfo {
    name: String,
    data_type: String,
    source_column: String,
    is_hidden: bool,
}

#[derive(serde::Serialize)]
struct MeasureInfo {
    name: String,
    expression: String,
    display_folder: String,
    classification: String,
}

#[derive(serde::Serialize)]
struct PartitionInfo {
    name: String,
    source_type: String,
    is_m: bool,
}

#[derive(serde::Serialize)]
struct RelInfo {
    from_table: String,
    from_column: String,
    to_table: String,
    to_column: String,
}

#[derive(serde::Serialize)]
struct RoleInfo {
    name: String,
    description: String,
}

#[derive(serde::Serialize)]
struct Summary {
    table_count: usize,
    fact_tables: Vec<String>,
    dimension_tables: Vec<String>,
    date_role_tables: Vec<String>,
    calculated_tables: Vec<String>,
    relationship_count: usize,
    measure_count: usize,
    simple_measures: usize,
    sql_fallback_measures: usize,
    manual_measures: usize,
    m_partition_tables: Vec<String>,
}

fn main() {
    let src_dir = match std::env::args().nth(1) {
        Some(d) => d,
        None => {
            eprintln!("Usage: cargo run --bin inventory -- <tabulareditor_src>");
            std::process::exit(1);
        }
    };

    let inventory = build_inventory(&src_dir);

    let json = serde_json::to_string_pretty(&inventory).unwrap();
    fs::write("conversion-inventory.json", &json).expect("write json");
    fs::write("conversion-inventory.md", &render_markdown(&inventory)).expect("write md");

    println!("{}", render_markdown(&inventory));
}

fn build_inventory(src_dir: &str) -> Inventory {
    let tables = parse_tables(&format!("{src_dir}/tables"));
    let relationships = parse_relationships(&format!("{src_dir}/relationships"));
    let roles = parse_roles(&format!("{src_dir}/roles"));

    // Classify tables
    let mut fact_tables = Vec::new();
    let mut dimension_tables = Vec::new();
    let mut date_role_tables = Vec::new();
    let mut calculated_tables = Vec::new();
    let mut m_partition_tables = Vec::new();

    for t in &tables {
        if t.partitions.iter().any(|p| p.source_type == "calculated") {
            calculated_tables.push(t.name.clone());
        } else {
            let is_date_role = t.name.to_lowercase().contains("kalender");
            let is_dimension = t.name.to_lowercase().starts_with("dw_fys d_");
            let is_fact = t.name.to_lowercase().contains("f_");

            if t.measures.len() > 5 || is_fact {
                fact_tables.push(t.name.clone());
            }
            if is_date_role {
                date_role_tables.push(t.name.clone());
            }
            if is_dimension {
                dimension_tables.push(t.name.clone());
            }
        }
        if t.partitions.iter().any(|p| p.is_m) {
            m_partition_tables.push(t.name.clone());
        }
    }

    // Measure classification
    let mut simple = 0;
    let mut sql_fallback = 0;
    let mut manual = 0;
    for t in &tables {
        for m in &t.measures {
            match m.classification.as_str() {
                "simple" => simple += 1,
                "sql_fallback" => sql_fallback += 1,
                "manual" => manual += 1,
                _ => {}
            }
        }
    }

    let measure_count = simple + sql_fallback + manual;

    Inventory {
        summary: Summary {
            table_count: tables.len(),
            fact_tables,
            dimension_tables,
            date_role_tables,
            calculated_tables,
            relationship_count: relationships.len(),
            measure_count,
            simple_measures: simple,
            sql_fallback_measures: sql_fallback,
            manual_measures: manual,
            m_partition_tables,
        },
        tables,
        relationships,
        roles,
    }
}

fn parse_tables(dir: &str) -> Vec<TableInfo> {
    let mut tables = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        eprintln!("tables dir not found: {dir}");
        return tables;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() { continue; }
        let table_name = path.file_name().unwrap().to_string_lossy().to_string();

        // Parse table metadata
        let meta_path = path.join(format!("{table_name}.json"));
        let (desc, _) = parse_table_meta(&meta_path);

        // Columns
        let columns = parse_columns(&path.join("columns"));

        // Measures
        let measures = parse_measures(&path.join("measures"));

        // Partitions
        let partitions = parse_partitions(&path.join("partitions"));

        // Hierarchies
        let hierarchies = parse_hierarchies(&path.join("hierarchies"));

        tables.push(TableInfo {
            name: table_name,
            description: desc,
            columns,
            measures,
            partitions,
            hierarchies,
            table_type: String::new(), // filled in summary
        });
    }

    tables.sort_by(|a, b| a.name.cmp(&b.name));
    tables
}

fn parse_table_meta(path: &Path) -> (String, Vec<String>) {
    if let Ok(text) = fs::read_to_string(path) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            let desc = v["description"].as_str().unwrap_or("").to_string();
            return (desc, vec![]);
        }
    }
    (String::new(), vec![])
}

fn parse_columns(dir: &Path) -> Vec<ColumnInfo> {
    let mut cols = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return cols; };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                let dt = v["dataType"].as_str().unwrap_or("string").to_string();
                let sc = v["sourceColumn"].as_str().unwrap_or(&name).to_string();
                let hidden = v["isHidden"].as_bool().unwrap_or(false);
                cols.push(ColumnInfo { name, data_type: dt, source_column: sc, is_hidden: hidden });
            }
        }
    }
    cols.sort_by(|a, b| a.name.cmp(&b.name));
    cols
}

fn parse_measures(dir: &Path) -> Vec<MeasureInfo> {
    let mut measures = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return measures; };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                let expr = flatten_json_array(&v["expression"]);
                let folder = v["displayFolder"].as_str().unwrap_or("").to_string();
                let classification = classify_dax(&expr);
                measures.push(MeasureInfo {
                    name,
                    expression: expr,
                    display_folder: folder,
                    classification,
                });
            }
        }
    }
    measures.sort_by(|a, b| a.name.cmp(&b.name));
    measures
}

fn parse_partitions(dir: &Path) -> Vec<PartitionInfo> {
    let mut parts = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return parts; };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                let st = v["source"]["type"].as_str().unwrap_or("").to_string();
                let is_m = st == "m";
                parts.push(PartitionInfo { name, source_type: st, is_m });
            }
        }
    }
    parts
}

fn parse_hierarchies(dir: &Path) -> Vec<String> {
    let mut hiers = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return hiers; };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        hiers.push(name.trim_end_matches(".json").to_string());
    }
    hiers
}

fn parse_relationships(dir: &str) -> Vec<RelInfo> {
    let mut rels = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return rels; };
    for entry in entries.flatten() {
        let path = entry.path();
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                rels.push(RelInfo {
                    from_table: v["fromTable"].as_str().unwrap_or("").to_string(),
                    from_column: v["fromColumn"].as_str().unwrap_or("").to_string(),
                    to_table: v["toTable"].as_str().unwrap_or("").to_string(),
                    to_column: v["toColumn"].as_str().unwrap_or("").to_string(),
                });
            }
        }
    }
    rels.sort_by(|a, b| a.from_table.cmp(&b.from_table).then(a.to_table.cmp(&b.to_table)));
    rels
}

fn parse_roles(dir: &str) -> Vec<RoleInfo> {
    let mut roles = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return roles; };
    for entry in entries.flatten() {
        let path = entry.path();
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                roles.push(RoleInfo {
                    name: v["name"].as_str().unwrap_or("").to_string(),
                    description: v["description"].as_str().unwrap_or("").to_string(),
                });
            }
        }
    }
    roles
}

fn flatten_json_array(arr: &serde_json::Value) -> String {
    match arr {
        serde_json::Value::Array(vals) => vals.iter()
            .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
            .collect::<Vec<_>>()
            .join(" "),
        serde_json::Value::String(s) => s.clone(),
        _ => String::new(),
    }
}

fn classify_dax(expr: &str) -> String {
    let upper = expr.to_uppercase();

    // Time-intelligence and cumulative patterns
    if upper.contains("ALLSELECTED") || upper.contains("ISONORAFTER")
        || upper.contains("TOTALYTD") || upper.contains("DATESYTD")
        || (upper.contains("CALCULATE(") && upper.contains("FILTER("))
    {
        return "sql_fallback".into();
    }

    // Context manipulation
    if upper.contains("ALL(") || upper.contains("ALLEXCEPT") || upper.contains("KEEPFILTERS") {
        return "sql_fallback".into();
    }

    // Iterator functions
    if upper.contains("SUMX(") || upper.contains("AVERAGEX(") || upper.contains("MAXX(")
        || upper.contains("RANKX(")
    {
        return "sql_fallback".into();
    }

    // Dynamic date functions
    if upper.contains("TODAY()") || upper.contains("NOW()") || upper.contains("UTCNOW()")
        || upper.contains("SAMEPERIODLASTYEAR")
    {
        return "sql_fallback".into();
    }

    // CALCULATE with simple static filter (including references to other measures)
    if upper.contains("CALCULATE(") {
        if !upper.contains("ALL(") && !upper.contains("FILTER(") && !upper.contains("KEEPFILTERS") {
            return "simple".into();
        }
        return "sql_fallback".into();
    }

    // Statistical functions — SQL fallback
    if upper.contains("MEDIAN(") || upper.contains("PERCENTILE(") {
        return "sql_fallback".into();
    }

    // Constant values
    let trimmed = expr.trim();
    if trimmed.parse::<f64>().is_ok() || trimmed.starts_with('"') {
        return "simple".into();
    }

    // Simple aggregates
    if upper.contains("SUM(") || upper.contains("COUNT(")
        || upper.contains("COUNTA(") || upper.contains("COUNTROWS(")
        || upper.contains("DISTINCTCOUNT(") || upper.contains("MIN(")
        || upper.contains("MAX(") || upper.contains("AVERAGE(")
    {
        return "simple".into();
    }

    // Arithmetic with simple measures
    if upper.contains("DIVIDE(") {
        return "simple".into();
    }

    // Calculated table functions — not measures
    if upper.contains("DATATABLE(") {
        return "simple".into();
    }

    // Unknown
    "manual".into()
}

fn render_markdown(inv: &Inventory) -> String {
    let mut out = String::new();
    let s = &inv.summary;

    out.push_str("# Conversion Inventory\n\n");
    out.push_str("## Summary\n\n");
    out.push_str(&format!("- **Tables**: {}\n", s.table_count));
    out.push_str(&format!("- **Fact tables**: {}\n", comma_list(&s.fact_tables)));
    out.push_str(&format!("- **Dimension tables**: {}\n", comma_list(&s.dimension_tables)));
    out.push_str(&format!("- **Date-role tables**: {}\n", comma_list(&s.date_role_tables)));
    out.push_str(&format!("- **Calculated tables**: {}\n", comma_list(&s.calculated_tables)));
    out.push_str(&format!("- **M-partition tables**: {}\n", comma_list(&s.m_partition_tables)));
    out.push_str(&format!("- **Relationships**: {}\n", s.relationship_count));
    out.push_str(&format!("- **Measures**: {} (simple: {}, sql_fallback: {}, manual: {})\n\n",
        s.measure_count, s.simple_measures, s.sql_fallback_measures, s.manual_measures));

    out.push_str("## Relationships\n\n");
    out.push_str("| From | From Col | To | To Col |\n|---|---|---|---|\n");
    for r in &inv.relationships {
        out.push_str(&format!("| {} | {} | {} | {} |\n",
            r.from_table, r.from_column, r.to_table, r.to_column));
    }

    out.push_str("\n## Tables\n\n");
    for t in &inv.tables {
        out.push_str(&format!("### {}\n\n", t.name));
        if !t.description.is_empty() {
            out.push_str(&format!("_{}_\n\n", t.description));
        }
        if !t.partitions.is_empty() {
            out.push_str("**Partitions**: ");
            let pts: Vec<String> = t.partitions.iter()
                .map(|p| format!("{} ({})", p.name, p.source_type))
                .collect();
            out.push_str(&pts.join(", "));
            out.push_str("\n\n");
        }
        if !t.hierarchies.is_empty() {
            out.push_str(&format!("**Hierarchies**: {}\n\n", t.hierarchies.join(", ")));
        }
        if !t.columns.is_empty() {
            out.push_str("| Column | Type | Source | Hidden |\n|---|---|---|---|\n");
            for c in &t.columns {
                out.push_str(&format!("| {} | {} | {} | {} |\n",
                    c.name, c.data_type, c.source_column, c.is_hidden));
            }
            out.push('\n');
        }
        if !t.measures.is_empty() {
            out.push_str("| Measure | Classification |\n|---|---|\n");
            for m in &t.measures {
                out.push_str(&format!("| {} | {} |\n", m.name, m.classification));
            }
            out.push('\n');
        }
    }

    out.push_str("## Roles\n\n");
    for r in &inv.roles {
        out.push_str(&format!("- **{}**: {}\n", r.name, r.description));
    }
    out.push('\n');

    out
}

fn comma_list(v: &[String]) -> String {
    if v.is_empty() { return "—".into(); }
    v.iter().map(|s| format!("`{}`", s)).collect::<Vec<_>>().join(", ")
}
