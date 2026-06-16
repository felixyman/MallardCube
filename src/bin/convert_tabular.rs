use std::fs;
use std::path::Path;

// ---- conversion model ----

struct ConversionModel {
    catalog: String,
    cube: String,
    fact_table: TableInfo,
    dimensions: Vec<TableInfo>,
    date_roles: Vec<TableInfo>,
    calculated_tables: Vec<TableInfo>,
    lookup_tables: Vec<TableInfo>,
    relationships: Vec<RelInfo>,
    roles: Vec<RoleInfo>,
}

struct TableInfo {
    name: String,
    ssas_name: String,
    description: String,
    columns: Vec<ColumnInfo>,
    measures: Vec<MeasureInfo>,
    is_m_partition: bool,
    is_calculated: bool,
}

struct ColumnInfo {
    name: String,
    data_type: String,
    source_column: String,
    is_hidden: bool,
}

#[derive(Debug, Clone)]
struct MeasureInfo {
    name: String,
    expression: Vec<String>,
    display_folder: String,
    classification: String,
}

struct RelInfo {
    from_table: String,
    from_column: String,
    to_table: String,
    to_column: String,
}

struct RoleInfo {
    name: String,
    description: String,
}

fn main() {
    let src_dir = match std::env::args().nth(1) {
        Some(d) => d,
        None => {
            eprintln!("Usage: cargo run --bin convert_tabular -- <tabulareditor_src> [output_dir]");
            std::process::exit(1);
        }
    };
    let out_dir = std::env::args().nth(2).unwrap_or_else(|| "generated_project".into());

    let model = parse_model(&src_dir);

    fs::create_dir_all(&out_dir).expect("create output dir");
    fs::create_dir_all(format!("{out_dir}/sql_fallback")).ok();

    // Generate SQL fallback files
    for meas in &model.fact_table.measures {
        if meas.classification == "sql_fallback" {
            let sql = generate_fallback_sql(meas, &model);
            let file_name = format!("{out_dir}/sql_fallback/{}.sql", malloy_name(&meas.name));
            fs::write(&file_name, sql).expect("write fallback");
        }
    }

    fs::write(format!("{out_dir}/proxy-config.json"), render_proxy_config(&model)).expect("write config");
    fs::write(format!("{out_dir}/model.malloy"), render_malloy(&model)).expect("write malloy");
    fs::write(format!("{out_dir}/schema.sql"), render_schema(&model)).expect("write schema");
    fs::write(format!("{out_dir}/conversion-report.md"), render_report(&model)).expect("write report");

    eprintln!("Generated project in {out_dir}/");
}

// ---- parser ----

fn parse_model(src_dir: &str) -> ConversionModel {
    let tables = parse_all_tables(&format!("{src_dir}/tables"));
    let rels = parse_relationships(&format!("{src_dir}/relationships"));
    let roles = parse_roles(&format!("{src_dir}/roles"));

    // Classify tables
    let mut fact = Vec::new();
    let mut dims = Vec::new();
    let mut dates = Vec::new();
    let mut calcs = Vec::new();
    let mut lookups = Vec::new();

    for t in tables {
        if t.is_calculated {
            calcs.push(t);
        } else {
            let lower = t.name.to_lowercase();
            if lower.contains("f_") || t.measures.len() > 5 {
                fact.push(t);
            } else if lower.contains("kalender") || lower == "dates" {
                dates.push(t);
            } else if lower.starts_with("dw_fys d_") {
                dims.push(t);
            } else {
                lookups.push(t);
            }
        }
    }

    // Fallback: if no fact table detected by heuristics, use relationship fromTable
    // as a signal — the table referenced most as a relationship source is the fact.
    if fact.is_empty() {
        let mut from_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for r in &rels {
            *from_counts.entry(r.from_table.clone()).or_insert(0) += 1;
        }
        if let Some((best_name, _)) = from_counts.into_iter()
            .max_by_key(|(_, c)| *c)
            .filter(|(_, c)| *c >= 2)
        {
            if let Some(pos) = lookups.iter().position(|t| t.name == best_name) {
                fact.push(lookups.remove(pos));
            } else if let Some(pos) = dims.iter().position(|t| t.name == best_name) {
                fact.push(dims.remove(pos));
            }
        }
    }

    let mut ft = if fact.len() == 1 {
        fact.remove(0)
    } else if !fact.is_empty() {
        // Multiple candidates — pick the one with most measures
        fact.sort_by_key(|t| -(t.measures.len() as i64));
        fact.remove(0)
    } else if !dims.is_empty() {
        // Fallback: treat first dimension as fact
        dims.remove(0)
    } else {
        eprintln!("WARNING: no fact table detected");
        TableInfo {
            name: "unknown".into(), ssas_name: "unknown".into(),
            description: String::new(), columns: vec![], measures: vec![],
            is_m_partition: false, is_calculated: false,
        }
    };

    // Merge DAX calculated table measures into the fact table
    let mut calc_measures: Vec<MeasureInfo> = calcs.iter()
        .flat_map(|c| c.measures.iter().cloned())
        .collect();
    ft.measures.append(&mut calc_measures);

    ConversionModel {
        catalog: ssas_name_to_id("SemanticModel"),
        cube: ssas_name_to_id(&ft.ssas_name),
        fact_table: ft,
        dimensions: dims,
        date_roles: dates,
        calculated_tables: calcs,
        lookup_tables: lookups,
        relationships: rels,
        roles,
    }
}

fn parse_all_tables(dir: &str) -> Vec<TableInfo> {
    let mut tables = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return tables; };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() { continue; }
        let name = path.file_name().unwrap().to_string_lossy().to_string();

        let meta_path = path.join(format!("{name}.json"));
        let (ssas_name, desc) = parse_table_meta(&meta_path);

        let columns = parse_columns(&path.join("columns"));
        let measures = parse_measures(&path.join("measures"));
        let partitions = parse_partitions(&path.join("partitions"));

        let is_m = partitions.iter().any(|(_, st)| st == "m");
        let is_calc = partitions.iter().any(|(_, st)| st == "calculated");

        tables.push(TableInfo {
            name,
            ssas_name,
            description: desc,
            columns,
            measures,
            is_m_partition: is_m,
            is_calculated: is_calc,
        });
    }
    tables
}

fn parse_table_meta(path: &Path) -> (String, String) {
    if let Ok(text) = fs::read_to_string(path) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            let ssas = v["name"].as_str().unwrap_or("").to_string();
            let desc = v["description"].as_str().unwrap_or("").to_string();
            return (ssas, desc);
        }
    }
    (String::new(), String::new())
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
                measures.push(MeasureInfo { name, expression: vec![expr], display_folder: folder, classification });
            }
        }
    }
    measures.sort_by(|a, b| a.name.cmp(&b.name));
    measures
}

fn parse_partitions(dir: &Path) -> Vec<(String, String)> {
    let mut parts = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return parts; };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                let st = v["source"]["type"].as_str().unwrap_or("").to_string();
                parts.push((name, st));
            }
        }
    }
    parts
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
    // Time intelligence: emit structured date-flag measures instead of sql_fallback
    if upper.contains("TOTALYTD") || upper.contains("DATESYTD") { return "time_ytd".into(); }
    if upper.contains("SAMEPERIODLASTYEAR") { return "time_prior_year".into(); }
    if upper.contains("ALLSELECTED") || upper.contains("ISONORAFTER") || (upper.contains("CALCULATE(") && upper.contains("FILTER(")) { return "sql_fallback".into(); }
    if upper.contains("ALL(") || upper.contains("ALLEXCEPT") || upper.contains("KEEPFILTERS") { return "sql_fallback".into(); }
    if upper.contains("SUMX(") || upper.contains("AVERAGEX(") || upper.contains("MAXX(") || upper.contains("RANKX(") { return "sql_fallback".into(); }
    if upper.contains("TODAY()") || upper.contains("NOW()") || upper.contains("UTCNOW()") || upper.contains("SAMEPERIODLASTYEAR") { return "sql_fallback".into(); }
    if upper.contains("CALCULATE(") {
        if !upper.contains("ALL(") && !upper.contains("FILTER(") && !upper.contains("KEEPFILTERS") { return "simple".into(); }
        return "sql_fallback".into();
    }
    if upper.contains("MEDIAN(") || upper.contains("PERCENTILE(") { return "sql_fallback".into(); }
    let trimmed = expr.trim();
    if trimmed.parse::<f64>().is_ok() || trimmed.starts_with('"') { return "simple".into(); }
    if upper.contains("SUM(") || upper.contains("COUNT(") || upper.contains("COUNTA(") || upper.contains("COUNTROWS(") || upper.contains("DISTINCTCOUNT(") || upper.contains("MIN(") || upper.contains("MAX(") || upper.contains("AVERAGE(") { return "simple".into(); }
    if upper.contains("DIVIDE(") {
        if upper.contains("CALCULATE(") && !upper.contains("ALLSELECTED") && !upper.contains("ISONORAFTER")
            && !upper.contains("ALL(") && !upper.contains("FILTER(") {
            return "simple".into();
        }
        return "simple".into();
    }
    if upper.contains("DATATABLE(") { return "calculated_table".into(); }
    "manual".into()
}

fn ssas_name_to_id(name: &str) -> String {
    name.replace(' ', "_").replace('-', "_").to_uppercase()
}

fn malloy_name(name: &str) -> String {
    name.to_lowercase().replace(' ', "_").replace('-', "_")
}

fn duckdb_type(bim_type: &str) -> &str {
    match bim_type {
        "int64" => "BIGINT",
        "double" => "DOUBLE",
        "string" => "VARCHAR",
        "dateTime" => "TIMESTAMP",
        "boolean" => "BOOLEAN",
        _ => "VARCHAR",
    }
}

// ---- renderers ----

fn render_proxy_config(m: &ConversionModel) -> String {
    let ft = &m.fact_table;
    let dims = render_dimension_configs(m);
    let meas = render_measure_configs(m);
    let facts = render_fact_table_configs(m);
    let rels = render_relationships(m);
    let ti_block = render_time_intelligence_block(m);

    format!(
        r##"{{{{
  "catalog": "{catalog}",
  "cube": "{cube}",
  "source_name": "{source}",
  "table_name": "{table}",
  "dialect": "duckdb",
  "malloy_model_file": "model.malloy",
  "db_path": null,
  "fact_tables": [
{facts}
  ],
  "relationships": [
{rels}
  ],{ti}
  "dimensions": [
{dims}
  ],
  "measures": [
{meas}
  ]
}}"##,
        catalog = m.catalog,
        cube = m.cube,
        source = malloy_name(&ft.ssas_name),
        table = malloy_name(&ft.name),
        facts = facts,
        rels = rels,
        ti = ti_block,
        dims = dims,
        meas = meas,
    ).replace("{{", "{").replace("}}", "}")
}

fn render_fact_table_configs(m: &ConversionModel) -> String {
    let ft = &m.fact_table;
    format!(
        r##"    {{{{
      "id": "default",
      "source_name": "{sn}",
      "table_name": "{tn}",
      "measure_group_name": "{cube}"
    }}}}"##,
        sn = malloy_name(&ft.ssas_name),
        tn = malloy_name(&ft.name),
        cube = m.cube,
    ).replace("{{", "{").replace("}}", "}")
}

fn render_relationships(m: &ConversionModel) -> String {
    let mut out = String::new();
    let dim_tables: Vec<&TableInfo> = m.dimensions.iter()
        .chain(&m.date_roles)
        .chain(&m.lookup_tables)
        .collect();
    let total = m.relationships.len();
    let mut emitted = 0usize;
    for rel in &m.relationships {
        if let Some(t) = dim_tables.iter().find(|t| t.name == rel.to_table || t.ssas_name == rel.to_table) {
            let dim_id = t.ssas_name.clone();
            out.push_str(&format!(
                r##"    {{{{
      "fact_table": "default",
      "fact_column": "{fc}",
      "dimension_id": "{did}",
      "dim_table": "{dt}",
      "dim_column": "{dc}"
    }}}}"##,
                fc = malloy_name(&rel.from_column),
                did = dim_id,
                dt = malloy_name(&rel.to_table),
                dc = malloy_name(&rel.to_column),
            ).replace("{{", "{").replace("}}", "}"));
            emitted += 1;
            if emitted < total { out.push_str(",\n"); }
        }
    }
    out
}

fn render_time_intelligence_block(m: &ConversionModel) -> String {
    if m.date_roles.is_empty() {
        return String::new();
    }
    // Use the first date-role dimension as the default calendar dimension.
    let first = &m.date_roles[0];
    format!(
        "\n  \"time_intelligence\": {{{{\n    \"date_dimension\": {{{{\n      \"dimension_id\": \"{did}\",\n      \"table_name\": \"{tn}\",\n      \"date_key_column\": \"date_key\",\n      \"full_date_column\": \"full_date\",\n      \"flag_columns\": {{{{\n        \"year_column\": \"year\",\n        \"quarter_column\": \"quarter\",\n        \"month_column\": \"month\",\n        \"ytd_flag_column\": \"ytd_flag\",\n        \"prior_year_ytd_flag_column\": \"prior_year_ytd_flag\",\n        \"current_year_flag_column\": \"current_year_flag\",\n        \"qtd_flag_column\": \"qtd_flag\",\n        \"mtd_flag_column\": \"mtd_flag\"\n      }}}}\n    }}}}\n  }},\n",
        did = first.ssas_name,
        tn = malloy_name(&first.name),
    ).replace("{{", "{").replace("}}", "}")
}

fn render_dimension_configs(m: &ConversionModel) -> String {
    let mut out = String::new();
    let all_dims: Vec<&TableInfo> = m.dimensions.iter()
        .chain(&m.date_roles)      // index-based since both are &TableInfo
        .chain(&m.lookup_tables)
        .collect();
    for (i, t) in all_dims.iter().enumerate() {
        let dim_name = t.ssas_name.clone();
        // Pick a representative column for display
        // Use first non-hidden column, or first visible name column
        let rep_col = t.columns.iter()
            .find(|c| !c.is_hidden && (c.source_column.contains("Namn") || c.source_column.contains("Kod") || c.source_column == dim_name))
            .or_else(|| t.columns.iter().find(|c| !c.is_hidden))
            .or_else(|| t.columns.first());
        let physical = rep_col.map(|c| malloy_name(&c.source_column)).unwrap_or_else(|| malloy_name(&dim_name));
        let ft_ref = if m.date_roles.iter().any(|d| d.name == t.name) {
            "default".to_string()
        } else if m.dimensions.iter().any(|d| d.name == t.name) {
            String::new()
        } else {
            String::new()
        };

        let ft_line = if ft_ref.is_empty() { String::new() } else { format!("\n      \"fact_table\": \"{}\",", ft_ref) };
        let shared = if m.date_roles.iter().any(|d| d.name == t.name) || m.dimensions.iter().any(|d| d.name == t.name) { "" } else { ",\n      \"shared\": true" };
        let is_date_role = m.date_roles.iter().any(|d| d.name == t.name);
        let date_role_line = if is_date_role { ",\n      \"is_date_role\": true" } else { "" };

        out.push_str(&format!(
            r##"    {{{{
      "id": "{id}",
      "malloy_name": "{mn}",
      "physical_field": "{pf}",
      "caption": "{caption}",
      "description": "{desc}",
      "hierarchy_name": "{caption}",
      "all_level_name": "(All)",
      "leaf_level_name": "{caption}",
      "ordinal": {ord},{ft_line}
      "visible": true,
      "has_all": true,
          "cardinality_hint": 100{shared}{date_role_line}
    }}}}"##,
            id = t.ssas_name,
            mn = malloy_name(&t.ssas_name),
            pf = format!("{}.{}", malloy_name(&t.name), physical),
            caption = t.ssas_name,
            desc = t.description.replace('\"', "\\\""),
            ord = i + 1,
            ft_line = ft_line,
            date_role_line = date_role_line,
            shared = if m.date_roles.iter().chain(&m.dimensions).any(|d| d.name == t.name) { "" } else { ",\n      \"shared\": true" },
        ).replace("{{", "{").replace("}}", "}"));
        if i < all_dims.len() - 1 { out.push_str(",\n"); }
    }
    out
}

fn render_measure_configs(m: &ConversionModel) -> String {
    let mut out = String::new();
    let all_measures: Vec<&MeasureInfo> = m.fact_table.measures.iter()
        .chain(m.dimensions.iter().flat_map(|t| &t.measures))
        .chain(m.date_roles.iter().flat_map(|t| &t.measures))
        .collect();
    for (i, meas) in all_measures.iter().enumerate() {
        let dax_expr = meas.expression.first().map(|s| s.as_str()).unwrap_or("");
        // For time measures, extract the inner aggregation for the sql_expr.
        let (time_class, flag_col, time_dim_id) = match meas.classification.as_str() {
            "time_ytd" => ("time_ytd", "ytd_flag",
                m.date_roles.first().map(|d| d.ssas_name.as_str()).unwrap_or("Date")),
            "time_prior_year" => ("time_prior_year", "prior_year_ytd_flag",
                m.date_roles.first().map(|d| d.ssas_name.as_str()).unwrap_or("Date")),
            _ => ("", "", ""),
        };
        let sql = if !time_class.is_empty() {
            // Extract base expression from TOTALYTD(inner, ...) or SAMEPERIODLASTYEAR(inner, ...)
            let inner = extract_ti_inner(dax_expr);
            let malloy = dax_to_malloy_expr(&inner);
            malloy_to_sql(&malloy)
        } else {
            dax_to_sql_hint(dax_expr, &meas.classification)
        };
        let pe = if meas.classification == "simple" || meas.classification == "time_ytd" || meas.classification == "time_prior_year" {
            if !time_class.is_empty() {
                dax_to_malloy_expr(&extract_ti_inner(dax_expr))
            } else {
                dax_to_malloy_expr(dax_expr)
            }
        } else {
            String::new()
        };
        let fb_line = if meas.classification == "sql_fallback" {
            format!(",\n      \"sql_fallback_file\": \"sql_fallback/{}.sql\"", malloy_name(&meas.name))
        } else if !time_class.is_empty() {
            format!(",\n      \"time_intelligence\": {{ \"dimension_id\": \"{did}\", \"flag_column\": \"{fc}\" }}",
                did = time_dim_id, fc = flag_col)
        } else {
            String::new()
        };
        out.push_str(&format!(
            r##"    {{{{
      "id": "{id}",
      "fact_table": "default",
      "malloy_name": "{mn}",
      "physical_expr": "{pe}",
      "sql_expr": "{sql}",
      "caption": "{caption}",
      "display_name": "{dn}",
      "description": "{desc}",
      "format_string": "#,##0.00",
      "units": "",
      "ordinal": {ord},
      "visible": true,
      "measure_group_name": "{cube}"{fb}
    }}}}"##,
            id = meas.name,
            mn = malloy_name(&meas.name),
            pe = json_escape(&pe),
            sql = sql,
            caption = meas.name,
            dn = meas.name,
            desc = json_escape(&format!("[{}] {}", meas.classification, dax_expr)),
            ord = i + 1,
            cube = m.cube,
            fb = fb_line,
        ).replace("{{", "{").replace("}}", "}"));
        if i < all_measures.len() - 1 { out.push_str(",\n"); }
    }
    out
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn dax_to_sql_hint(expr: &str, class: &str) -> String {
    match class {
        "simple" => {
            // Try to emit a real SQL expression
            let malloy = dax_to_malloy_expr(expr);
            malloy_to_sql(&malloy)
        }
        "sql_fallback" => "null".to_string(),
        _ => "null".to_string(),
    }
}

fn malloy_to_sql(malloy: &str) -> String {
    if malloy.contains("count(distinct true") { return "COUNT(DISTINCT ...)".into(); }
    if malloy.contains(".count()") { return "COUNT(...)".into(); }
    if malloy.contains(".sum()") { return "SUM(...)".into(); }
    if malloy.contains(".avg()") { return "AVG(...)".into(); }
    if malloy == "0.8" { return "0.8".into(); }
    "SUM(1)".to_string()
}

fn dax_to_malloy_expr(dax: &str) -> String {
    let dax = dax.trim();
    let upper = dax.to_uppercase();

    // Constant value (e.g. "0.8")
    if let Ok(_) = dax.trim().parse::<f64>() {
        return dax.trim().to_string();
    }

    // DISTINCTCOUNT('table'[col]) → col.count(distinct true)
    if let Some(inner) = extract_dax_unary(&upper, "DISTINCTCOUNT(") {
        if let Some(col) = extract_col(&inner) {
            return format!("{}.count(distinct true)", malloy_name(&col));
        }
    }

    // COUNT('table'[col]) → col.count()
    if let Some(inner) = extract_dax_unary(&upper, "COUNT(") {
        if let Some(col) = extract_col(&inner) {
            return format!("{}.count()", malloy_name(&col));
        }
    }

    // AVERAGE('table'[col]) → col.avg()
    if let Some(inner) = extract_dax_unary(&upper, "AVERAGE(") {
        if let Some(col) = extract_col(&inner) {
            return format!("{}.avg()", malloy_name(&col));
        }
    }

    // SUM('table'[col]) → col.sum()
    if let Some(inner) = extract_dax_unary(&upper, "SUM(") {
        if let Some(col) = extract_col(&inner) {
            return format!("{}.sum()", malloy_name(&col));
        }
    }

    // DIVIDE(a, b) → a / b
    if upper.starts_with("DIVIDE(") {
        let inner = &upper["DIVIDE(".len()..];
        // Use the original (non-upper) string for splitting to preserve
        let orig_inner = &dax["DIVIDE(".len()..];
        let upper_parts = split_args(inner);
        let orig_parts = split_args(orig_inner);
        if upper_parts.len() >= 2 {
            let a = dax_to_malloy_expr(&orig_parts[0]);
            let b = dax_to_malloy_expr(&orig_parts[1]);
            return format!("{a} / {b}");
        }
    }

    // CALCULATE([measure], 'dim'[col]="value") → measure { where: col = 'value' }
    if upper.starts_with("CALCULATE(") {
        let inner = &upper["CALCULATE(".len()..];
        let parts = split_args(inner);
        if parts.len() >= 2 {
            let base = dax_to_malloy_expr(&parts[0]);
            let filter = extract_calculate_filter(&parts[1]);
            if let Some(f) = filter {
                return format!("{base} {{ where: {f} }}");
            }
            // Multiple filters
            let mut filters = Vec::new();
            for p in &parts[1..] {
                if let Some(f) = extract_calculate_filter(p) {
                    filters.push(f);
                }
            }
            if !filters.is_empty() {
                return format!("{base} {{ where: {} }}", filters.join(", "));
            }
            return base;
        }
        if parts.len() == 1 {
            return dax_to_malloy_expr(&parts[0]);
        }
    }

    // Reference to another measure: [Measure Name]
    if upper.starts_with('[') && upper.ends_with(']') {
        let name = &upper[1..upper.len()-1];
        return malloy_name(name);
    }

    // Compound expression like a / b
    if upper.contains("/") && !upper.contains('(') {
        let parts: Vec<&str> = dax.split('/').collect();
        if parts.len() == 2 {
            let a = dax_to_malloy_expr(parts[0].trim());
            let b = dax_to_malloy_expr(parts[1].trim());
            return format!("{a} / {b}");
        }
    }

    // Fallback
    "1.sum()".to_string()
}

/// Extract the inner expression from a time-intelligence DAX wrapper:
/// TOTALYTD(inner, dates) → inner
/// SAMEPERIODLASTYEAR(inner, dates) → inner
fn extract_ti_inner(dax: &str) -> String {
    let upper = dax.to_uppercase();
    let (func, dax) = if upper.starts_with("TOTALYTD(") {
        ("TOTALYTD(", &dax["TOTALYTD(".len()..])
    } else if upper.starts_with("SAMEPERIODLASTYEAR(") {
        ("SAMEPERIODLASTYEAR(", &dax["SAMEPERIODLASTYEAR(".len()..])
    } else if upper.starts_with("DATESYTD(") {
        ("DATESYTD(", &dax["DATESYTD(".len()..])
    } else {
        return dax.to_string();
    };
    // Find the closing paren matching the opening func, then extract inner.
    let mut depth = 1;
    let mut comma_pos = None;
    for (i, c) in dax.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => { depth -= 1; if depth == 0 { break; } }
            ',' if depth == 1 => { comma_pos = Some(i); break; }
            _ => {}
        }
    }
    comma_pos.map(|pos| dax[..pos].trim().to_string())
        .unwrap_or_else(|| dax.to_string())
}

fn extract_dax_unary(dax: &str, func: &str) -> Option<String> {
    if !dax.starts_with(func) { return None; }
    let inner = &dax[func.len()..];
    let inner = inner.trim_end_matches(')').trim();
    Some(inner.to_string())
}

fn extract_col(dax: &str) -> Option<String> {
    let trimmed = dax.trim().trim_matches('\'');
    if let Some(bracket) = trimmed.find('[') {
        let after = &trimmed[bracket+1..];
        if let Some(close) = after.find(']') {
            return Some(after[..close].to_string());
        }
    }
    None
}

fn split_args(inner: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    let mut last_byte = 0;
    for (byte_idx, c) in inner.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => if depth > 0 { depth -= 1 } else { parts.push(inner[start..byte_idx].trim().to_string()); return parts; },
            ',' if depth == 0 => {
                parts.push(inner[start..byte_idx].trim().to_string());
                start = byte_idx + 1;
            }
            _ => {}
        }
        last_byte = byte_idx + c.len_utf8();
    }
    let last = inner[start..].trim().to_string();
    if !last.is_empty() { parts.push(last); }
    parts
}

fn extract_calculate_filter(dax: &str) -> Option<String> {
    let trimmed = dax.trim_matches('\'').trim();
    // Try comparison operators: first standalone =, then <=, >=, <, >
    if let Some((op, pos)) = find_comparison_op(trimmed) {
        let col = trimmed[..pos].trim();
        let col = extract_col(col)?;
        let val = trimmed[pos + op.len()..].trim().trim_matches('"').trim_matches('\'');
        let val = val.trim_end_matches(')');
        return Some(format!("{} {op} '{val}'", malloy_name(&col)));
    }
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return Some(malloy_name(&trimmed[1..trimmed.len()-1]));
    }
    None
}

fn find_comparison_op(s: &str) -> Option<(&'static str, usize)> {
    let bytes = s.as_bytes();
    for op_str in ["<=", ">=", "<>"] {
        if let Some(pos) = s.find(op_str) {
            return Some((op_str, pos));
        }
    }
    // Handle standalone = (not part of <=, >=, <>)
    if let Some(pos) = s.find('=') {
        if pos == 0 || (bytes[pos-1] != b'<' && bytes[pos-1] != b'>' && bytes[pos-1] != b'!') {
            return Some(("=", pos));
        }
    }
    // Regular < and >
    if let Some(pos) = s.find('>') {
        if pos + 1 >= bytes.len() || bytes[pos+1] != b'=' { return Some((">", pos)); }
    }
    if let Some(pos) = s.find('<') {
        if pos + 1 >= bytes.len() || (bytes[pos+1] != b'=' && bytes[pos+1] != b'>') { return Some(("<", pos)); }
    }
    None
}

fn find_standalone_eq(s: &str) -> Option<usize> {
    find_comparison_op(s).map(|(_, pos)| pos)
}

fn render_malloy(m: &ConversionModel) -> String {
    let ft = &m.fact_table;
    let mut out = String::new();

    // Build a lookup: SSAS table name → malloy source name
    let table_source: std::collections::HashMap<String, String> = std::iter::once((ft.name.clone(), malloy_name(&ft.ssas_name)))
        .chain(m.dimensions.iter().map(|t| (t.name.clone(), malloy_name(&t.ssas_name))))
        .chain(m.date_roles.iter().map(|t| (t.name.clone(), malloy_name(&t.ssas_name))))
        .chain(m.lookup_tables.iter().map(|t| (t.name.clone(), malloy_name(&t.ssas_name))))
        .collect();

    // Fact source with joins and measures
    out.push_str(&format!(
        "source: {} is duckdb.table('{}') extend {{\n",
        malloy_name(&ft.ssas_name),
        malloy_name(&ft.name),
    ));

    // Emit joins: find relationships where from_table == fact table
    for rel in &m.relationships {
        if rel.from_table == ft.name {
            if let Some(to_src) = table_source.get(&rel.to_table) {
                let join_col = malloy_name(&rel.from_column);
                out.push_str(&format!("  join_one: {to_src} with {join_col}\n"));
            }
        }
    }

    out.push('\n');

    // Emit simple measures
    for meas in &ft.measures {
        if meas.classification == "simple" {
            let expr = meas.expression.first().map(|s| s.as_str()).unwrap_or("");
            let malloy_expr = dax_to_malloy_expr(expr);
            out.push_str(&format!("  measure: {} is {malloy_expr}  -- {}\n",
                malloy_name(&meas.name), meas.name));
        }
    }

    out.push_str("}\n\n");

    // Dimension table sources
    for t in &m.dimensions {
        out.push_str(&format!(
            "source: {} is duckdb.table('{}') extend {{\n}}\n\n",
            malloy_name(&t.ssas_name),
            malloy_name(&t.name),
        ));
    }
    for t in &m.date_roles {
        out.push_str(&format!(
            "source: {} is duckdb.table('{}') extend {{\n}}\n\n",
            malloy_name(&t.ssas_name),
            malloy_name(&t.name),
        ));
    }

    out
}

fn render_fallback_stub(name: &str, dax: &str) -> String {
    let upper = dax.to_uppercase();
    let mut notes = Vec::new();
    if upper.contains("ALLSELECTED") { notes.push("ALLSELECTED — requires window function"); }
    if upper.contains("ISONORAFTER") { notes.push("ISONORAFTER — cumulative window ordering"); }
    if upper.contains("FILTER(") { notes.push("FILTER — context manipulation"); }
    if upper.contains("YEAR(TODAY())") { notes.push("YEAR(TODAY()) — dynamic current year filter"); }
    if upper.contains("YEAR(TODAY())-1") { notes.push("Previous year comparison"); }
    if upper.contains("MEDIAN(") { notes.push("MEDIAN — DuckDB supports MEDIAN() natively"); }
    if upper.contains("AVERAGEX(") { notes.push("AVERAGEX — row-level iteration"); }
    if upper.contains("KEEPFILTERS") { notes.push("KEEPFILTERS — filter context preservation"); }
    let note_str = if notes.is_empty() { "Complex DAX pattern — requires SQL fallback".to_string() } else { notes.join("\n--   ") };
    format!(
        r#"-- SQL fallback for: {name}
-- Original DAX: {dax}
--
-- Pattern notes:
--   {notes}
--
-- TODO: Implement DuckDB SQL equivalent.
-- Runs via the proxy's direct SQL fallback path.

SELECT 1 AS dummy;
"#,
        name = name, dax = dax, notes = note_str,
    )
}

// ---- SQL fallback generation ----

fn generate_fallback_sql(meas: &MeasureInfo, model: &ConversionModel) -> String {
    let dax = meas.expression.first().map(|s| s.as_str()).unwrap_or("");
    let upper = dax.to_uppercase();

    // Pattern 1: MEDIAN(col) — DuckDB native
    if upper.contains("MEDIAN(") {
        if let Some(col_expr) = extract_dax_unary(&upper, "MEDIAN(") {
            if let Some(col) = extract_col(&col_expr) {
                let fact_table = malloy_name(&model.fact_table.name);
                return format!(
                    "-- Auto-generated from DAX: {dax}\n-- DuckDB supports MEDIAN() natively.\n\nSELECT MEDIAN({col}) FROM {fact_table};\n",
                    dax = dax, col = malloy_name(&col), fact_table = fact_table,
                );
            }
        }
    }

    // Pattern 2: Cumulative YTD (FILTER + ALLSELECTED + ISONORAFTER)
    if upper.contains("ALLSELECTED") && upper.contains("ISONORAFTER") {
        return generate_cumulative_sql(dax, &upper, meas, model);
    }

    // Pattern 3: AVERAGEX + KEEPFILTERS — too complex for auto-generation
    // Keep annotated stub
    render_fallback_stub(&meas.name, dax)
}

fn generate_cumulative_sql(dax: &str, upper: &str, meas: &MeasureInfo, model: &ConversionModel) -> String {
    let cal_table = extract_calendar_table(dax);
    let period_col = extract_period_column(dax);
    let year_col = extract_year_column(dax);
    let is_prior_year = upper.contains("YEAR(TODAY())-1") || upper.contains("YEAR(TODAY()) - 1");
    let base_meas = extract_base_measure(dax);

    let cal_malloy = cal_table.as_deref().map(malloy_name).unwrap_or_else(|| "calendar".into());
    let period_col_name = period_col.as_deref().map(malloy_name).unwrap_or_else(|| "period".into());
    let year_col_name = year_col.as_deref().map(malloy_name).unwrap_or_else(|| "year".into());

    let join_col = cal_table.as_ref().and_then(|ct| {
        model.relationships.iter()
            .find(|r| r.to_table == *ct)
            .map(|r| r.from_column.clone())
    }).unwrap_or_default();
    let join_col_name = malloy_name(&join_col);
    let fact_table = malloy_name(&model.fact_table.name);
    let year_expr = if is_prior_year { "EXTRACT(YEAR FROM CURRENT_DATE) - 1" } else { "EXTRACT(YEAR FROM CURRENT_DATE)" };

    format!(
        r#"-- Cumulative YTD for: {name}
-- Original DAX: {dax}
-- Calendar: {cal_malloy}, Period: {period}, Year: {year}
-- Base measure: {base_meas}
-- Join: f.{join_col} = c.{join_col}

SELECT
  c.{period},
  c.{year},
  SUM(base_count) OVER (
    PARTITION BY c.{year}
    ORDER BY c.{period}
    ROWS UNBOUNDED PRECEDING
  ) AS ack_value
FROM (
  SELECT
    c.{period},
    c.{year},
    COUNT(DISTINCT f.remissnummer) AS base_count
  FROM {fact_table} f
  JOIN {cal_malloy} c ON f.{join_col} = c.{join_col}
  WHERE c.{year} = {year_expr}
  GROUP BY c.{period}, c.{year}
);
"#,
        name = meas.name, dax = dax,
        cal_malloy = cal_malloy,
        period = period_col_name, year = year_col_name,
        base_meas = base_meas, join_col = join_col_name,
        fact_table = fact_table, year_expr = year_expr,
    )
}

fn extract_calendar_table(dax: &str) -> Option<String> {
    if let Some(start) = dax.find("ALLSELECTED('") {
        let after = &dax[start + "ALLSELECTED('".len()..];
        if let Some(end) = after.find('\'') {
            return Some(after[..end].to_string());
        }
    }
    None
}

fn extract_period_column(dax: &str) -> Option<String> {
    if let Some(start) = dax.find("ALLSELECTED('") {
        let after = &dax[start + "ALLSELECTED('".len()..];
        if let Some(table_end) = after.find('\'') {
            let after_table = &after[table_end + 1..];
            if after_table.starts_with('[') {
                if let Some(close) = after_table.find(']') {
                    return Some(after_table[1..close].to_string());
                }
            }
        }
    }
    None
}

fn extract_year_column(dax: &str) -> Option<String> {
    // Find the year filter: 'calendar'[ÅR] = YEAR(TODAY())
    if let Some(year_pos) = dax.to_uppercase().find("YEAR(TODAY())") {
        let before = &dax[..year_pos];
        if let Some(last_brace) = before.rfind("'[") {
            let after = &before[last_brace + 2..];
            if let Some(close) = after.find(']') {
                return Some(after[..close].to_string());
            }
        }
    }
    None
}

fn extract_base_measure(dax: &str) -> String {
    if let Some(calc_start) = dax.find("CALCULATE(") {
        let after = &dax[calc_start + "CALCULATE(".len()..];
        let trimmed = after.trim_start();
        if trimmed.starts_with('[') {
            if let Some(close) = trimmed.find(']') {
                return trimmed[1..close].to_string();
            }
        }
    }
    String::new()
}

fn render_schema(m: &ConversionModel) -> String {
    let mut out = String::new();
    out.push_str("-- Generated from Tabular Editor model\n");
    out.push_str("-- Data loading via M partitions must be done manually.\n\n");

    // Fact table
    out.push_str(&render_create_table(&m.fact_table, true));

    // Dimensions
    for t in &m.dimensions {
        out.push_str(&render_create_table(t, false));
    }

    // Date roles
    for t in &m.date_roles {
        out.push_str(&render_create_table(t, false));
    }

    // Lookup tables
    for t in &m.lookup_tables {
        out.push_str(&render_create_table(t, false));
    }

    // Calculated tables
    if !m.calculated_tables.is_empty() {
        out.push_str("\n-- Calculated tables (see calculated_tables.sql)\n");
    }

    out
}

fn render_create_table(t: &TableInfo, is_fact: bool) -> String {
    let table_name = malloy_name(&t.name);
    let mut out = format!("CREATE TABLE IF NOT EXISTS {table_name} (\n");
    let visible_cols: Vec<&ColumnInfo> = t.columns.iter().collect();
    for (i, c) in visible_cols.iter().enumerate() {
        let col_name = malloy_name(&c.source_column);
        let dt = duckdb_type(&c.data_type);
        let comma = if i < visible_cols.len() - 1 { "," } else { "" };
        out.push_str(&format!("    {col_name} {dt}{comma}\n"));
    }
    out.push_str(");\n");
    if is_fact {
        out.push_str(&format!("-- FACT TABLE: {}\n", t.ssas_name));
    }
    out.push('\n');
    out
}

fn render_report(m: &ConversionModel) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Conversion Report — {}\n\n", m.catalog));

    let simple: Vec<_> = m.fact_table.measures.iter().filter(|m| m.classification == "simple").collect();
    let fallback: Vec<_> = m.fact_table.measures.iter().filter(|m| m.classification == "sql_fallback").collect();
    let manual: Vec<_> = m.fact_table.measures.iter().filter(|m| m.classification == "manual").collect();

    out.push_str("## Summary\n\n");
    out.push_str(&format!("- Fact table: {}\n", m.fact_table.ssas_name));
    out.push_str(&format!("- Dimensions: {}\n", m.dimensions.len() + m.date_roles.len() + m.lookup_tables.len()));
    out.push_str(&format!("- Date-role tables: {}\n", m.date_roles.len()));
    out.push_str(&format!("- Relationships: {}\n", m.relationships.len()));
    out.push_str(&format!("- Measures: {} (simple: {}, sql_fallback: {}, manual: {})\n",
        m.fact_table.measures.len(), simple.len(), fallback.len(), manual.len()));
    out.push_str(&format!("- M-partition tables: {} (all must be loaded manually)\n\n",
        if m.fact_table.is_m_partition { 1usize } else { 0 } + m.dimensions.iter().filter(|t| t.is_m_partition).count() + m.date_roles.iter().filter(|t| t.is_m_partition).count()));

    // Join map
    out.push_str("## Join Map\n\n");
    out.push_str("| Fact Column | Dimension Table | Join Column |\n|---|---|---|\n");
    for rel in &m.relationships {
        if rel.from_table == m.fact_table.name {
            out.push_str(&format!("| {} | {} | {} |\n", rel.from_column, rel.to_table, rel.to_column));
        }
    }
    out.push('\n');

    out.push_str("## Simple measures (Malloy)\n\n");
    out.push_str("| Measure | DAX | Malloy |\n|---|---|---|\n");
    for m in &simple {
        let dax = m.expression.first().map(|s| s.as_str()).unwrap_or("");
        let malloy = dax_to_malloy_expr(dax);
        out.push_str(&format!("| {} | {} | {} |\n", m.name, dax, malloy));
    }

    out.push_str("\n## SQL fallback measures\n\n");
    out.push_str("| Measure | DAX pattern | Fallback file |\n|---|---|---|\n");
    for m in &fallback {
        let dax = m.expression.first().map(|s| s.as_str()).unwrap_or("");
        out.push_str(&format!("| {} | {} | sql_fallback/{}.sql |\n",
            m.name, dax, malloy_name(&m.name)));
    }

    if !manual.is_empty() {
        out.push_str("\n## Manual review required\n\n");
        out.push_str("| Measure | DAX pattern |\n|---|---|\n");
        for m in &manual {
            out.push_str(&format!("| {} | {} |\n", m.name, m.expression.first().map(|s| s.as_str()).unwrap_or("")));
        }
    }

    out.push_str("\n## Data loading checklist\n\n");
    out.push_str("All tables use M (Power Query) partitions and must be loaded into DuckDB manually.\n\n");
    out.push_str("Run `schema.sql` to create the tables, then load data via:\n\n");
    out.push_str("- DuckDB CLI: `INSERT INTO ... SELECT ... FROM 'source.csv'`\n");
    out.push_str("- Or export your SSAS source to Parquet/CSV and import into DuckDB.\n\n");

    out.push_str("### Tables to load\n\n");
    out.push_str(&format!("- [ ] `{}` (fact)\n", malloy_name(&m.fact_table.name)));
    for t in &m.dimensions {
        out.push_str(&format!("- [ ] `{}` (dimension)\n", malloy_name(&t.name)));
    }
    for t in &m.date_roles {
        out.push_str(&format!("- [ ] `{}` (date-role)\n", malloy_name(&t.name)));
    }
    for t in &m.lookup_tables {
        out.push_str(&format!("- [ ] `{}` (lookup)\n", malloy_name(&t.name)));
    }

    if !m.roles.is_empty() {
        out.push_str("\n## Roles\n\n");
        out.push_str("Security roles detected but NOT supported by the proxy:\n\n");
        for r in &m.roles {
            out.push_str(&format!("- {}: {}\n", r.name, r.description));
        }
        out.push_str("\nMust be enforced outside the proxy if needed.\n");
    }

    out
}
