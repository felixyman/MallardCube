use std::fs;
use std::path::Path;

use super::parse_bim;
use super::parse_folder;
use super::parse_tmdl;
use super::tabular_model::*;

#[derive(serde::Serialize)]
struct Inventory {
    tables: Vec<TableInfo>,
    relationships: Vec<RelInfo>,
    roles: Vec<RoleInfo>,
    summary: Summary,
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

pub fn run(args: Vec<String>) -> i32 {
    let src_dir = match args.get(1) {
        Some(d) => d,
        None => {
                eprintln!("Usage: xmla_proxy inventory <tabulareditor_src>");
                eprintln!("  <tabulareditor_src> can be a directory (folder/TMDL format) or .bim file");
                return 1;
            }
        };
    let src_path = Path::new(&src_dir);
    if !src_path.exists() {
        eprintln!("Error: Path '{}' does not exist", src_dir);
        return 1;
    }
    let detected = detect_format(src_path);
    let format_name = match detected {
        Some(TabularFormat::Bim) => "BIM",
        Some(TabularFormat::Tmdl) => "TMDL",
        Some(TabularFormat::Folder) => "folder",
        None => "",
    };
    if !format_name.is_empty() {
        eprintln!("Detected format: {}", format_name);
    }
    let (parsed, warnings) = match detected {
        Some(TabularFormat::Bim) => parse_bim::parse_model(&src_dir),
        Some(TabularFormat::Tmdl) => parse_tmdl::parse_model(&src_dir),
        Some(TabularFormat::Folder) => parse_folder::parse_model(&src_dir),
        None => {
            eprintln!("Error: '{}' is neither a .bim file nor a directory with Tabular Editor files", src_dir);
            eprintln!("Usage: xmla_proxy inventory <tabulareditor_src>");
            return 1;
        }
    };
    for w in &warnings {
        eprintln!("WARNING: {}", w);
    }
    let inventory = build_inventory(parsed);

    let json = serde_json::to_string_pretty(&inventory).unwrap();
    fs::write("conversion-inventory.json", &json).expect("write json");
    fs::write("conversion-inventory.md", &render_markdown(&inventory)).expect("write md");

    println!("{}", render_markdown(&inventory));
    0
}

fn build_inventory(parsed: TabularModel) -> Inventory {
    let tables = parsed.tables;
    let relationships = parsed.relationships;
    let roles = parsed.roles;

    // Classify tables
    let mut fact_tables = Vec::new();
    let mut dimension_tables = Vec::new();
    let mut date_role_tables = Vec::new();
    let mut calculated_tables = Vec::new();
    let mut m_partition_tables = Vec::new();

    for t in &tables {
        if t.is_calculated() {
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
        if t.is_m_partition() {
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
    if v.is_empty() { return "\u{2014}".into(); }
    v.iter().map(|s| format!("`{}`", s)).collect::<Vec<_>>().join(", ")
}
