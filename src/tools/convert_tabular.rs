use std::fs;
use std::path::Path;

use super::data_loader;
use super::parse_bim;
use super::parse_folder;
use super::parse_tmdl;
use super::tabular_model::*;

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
    data_sources: Vec<DataSourceInfo>,
}

impl ConversionModel {
    /// Reconstruct a `TabularModel` from this classified model.
    /// Used by the data loader renderers which operate on `TabularModel`.
    fn to_tabular_model(&self) -> TabularModel {
        TabularModel {
            name: self.catalog.clone(),
            compatibility_level: 0,
            tables: std::iter::once(&self.fact_table)
                .chain(&self.dimensions)
                .chain(&self.date_roles)
                .chain(&self.lookup_tables)
                .cloned()
                .collect(),
            relationships: self.relationships.clone(),
            roles: self.roles.clone(),
            data_sources: self.data_sources.clone(),
        }
    }
}

pub fn run(args: Vec<String>) -> i32 {
    let src_dir = match args.get(1) {
        Some(d) => d,
        None => {
                eprintln!("Usage: xmla_proxy convert-tabular <tabulareditor_src> [output_dir]");
                eprintln!("  <tabulareditor_src> can be a directory (folder/TMDL format) or .bim file");
                return 1;
            }
        };
        let out_dir = args.get(2).cloned().unwrap_or_else(|| "generated_project".into());
 
         let mut dummy_rows = 10000usize;
         for arg in &args {
             if let Some(val) = arg.strip_prefix("--dummy-rows=") {
                 if let Ok(n) = val.parse::<usize>() {
                     dummy_rows = n;
                 }
             }
         }
 
         let src_path = Path::new(src_dir);
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
            Some(TabularFormat::Bim) => parse_bim::parse_model(src_dir),
            Some(TabularFormat::Tmdl) => parse_tmdl::parse_model(src_dir),
            Some(TabularFormat::Folder) => parse_folder::parse_model(src_dir),
            None => {
                eprintln!("Error: '{}' is neither a .bim file nor a directory with Tabular Editor files", src_dir);
                eprintln!("Usage: xmla_proxy convert-tabular <tabulareditor_src> [output_dir]");
                return 1;
            }
        };
    for w in &warnings {
        eprintln!("WARNING: {}", w);
    }
    let mut model = classify_model(parsed);

    let total = 1 + model.dimensions.len() + model.date_roles.len() + model.calculated_tables.len() + model.lookup_tables.len();
    eprintln!("Classified {} tables ({} fact, {} dimension, {} date-role, {} calculated, {} lookup)",
        total, 1, model.dimensions.len(), model.date_roles.len(), model.calculated_tables.len(), model.lookup_tables.len());

    fs::create_dir_all(&out_dir).expect("create output dir");
    fs::create_dir_all(format!("{out_dir}/sql_fallback")).ok();

    // Reclassify "simple" measures whose SQL hints return None (placeholder)
    // as "sql_fallback" so the runtime never executes placeholder SQL.
    for meas in model.fact_table.measures.iter_mut()
        .chain(model.dimensions.iter_mut().flat_map(|t| &mut t.measures))
        .chain(model.date_roles.iter_mut().flat_map(|t| &mut t.measures))
    {
        if meas.classification == "simple" {
            let dax_expr = meas.expression.as_str();
            if dax_to_sql_hint(dax_expr, &meas.classification).is_none() {
                meas.classification = "sql_fallback".to_string();
            }
        }
    }

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

    // Data loading scripts
    let fact_names: Vec<String> = vec![model.fact_table.name.clone()];
    let date_role_names: Vec<String> = model.date_roles.iter().map(|t| t.name.clone()).collect();

    // Reconstruct TabularModel for load script generators
    let tabular_model = model.to_tabular_model();

    fs::write(format!("{out_dir}/load_data.sql"),
        data_loader::render_load_script(&tabular_model, &fact_names, &date_role_names))
        .expect("write load_data.sql");

    let dim_rows = (dummy_rows / 10).max(100);
    fs::write(format!("{out_dir}/load_dummy_data.sql"),
        data_loader::render_dummy_data_script(&tabular_model, &fact_names, &date_role_names, dummy_rows, dim_rows))
        .expect("write load_dummy_data.sql");

    fs::write(format!("{out_dir}/conversion-report.md"), render_report(&model)).expect("write report");

    // Bootstrap script (always emitted)
    let cube_db = format!("{}.db", malloy_name(&model.cube));
    fs::create_dir_all(format!("{out_dir}/data")).ok();

    let mut bootstrap = format!(
        "-- Bootstrap script for {cube}\n\
         -- Run against DuckDB to create a runnable database.\n\
         --   duckdb {cube_db} < bootstrap.sql\n\n\
         .read schema.sql\n",
        cube = model.cube,
        cube_db = cube_db,
    );

    if !model.date_roles.is_empty() {
        let seed_sql = include_str!("../../data/seed_date_dim.sql");
        fs::write(format!("{out_dir}/seed_date_dim.sql"), seed_sql).ok();
        bootstrap.push_str(".read seed_date_dim.sql\n");
    }

    bootstrap.push_str(".read load_dummy_data.sql\n");
    bootstrap.push_str("\n-- For real data, replace the line above with:\n");
    bootstrap.push_str("-- .read load_data.sql\n");

    fs::write(format!("{out_dir}/bootstrap.sql"), bootstrap).expect("write bootstrap.sql");

    eprintln!("Generated project in {out_dir}/");
    eprintln!("  Files: proxy-config.json, model.malloy, schema.sql, load_data.sql, load_dummy_data.sql, bootstrap.sql, conversion-report.md");
    0
}

// ---- model classification ----

fn classify_model(parsed: TabularModel) -> ConversionModel {
    let model_name = parsed.name.clone();
    let mut tables = parsed.tables;
    let rels = parsed.relationships;
    let roles = parsed.roles;
    let data_sources = parsed.data_sources;

    // Classify tables
    let mut fact = Vec::new();
    let mut dims = Vec::new();
    let mut dates = Vec::new();
    let mut calcs = Vec::new();
    let mut lookups = Vec::new();

    for t in tables.drain(..) {
        if t.is_calculated() {
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
            partitions: vec![], hierarchies: vec![],
        }
    };

    // Merge DAX calculated table measures into the fact table
    let mut calc_measures: Vec<MeasureInfo> = calcs.iter()
        .flat_map(|c| c.measures.iter().cloned())
        .collect();
    ft.measures.append(&mut calc_measures);

    ConversionModel {
        catalog: ssas_name_to_id(&model_name),
        cube: ssas_name_to_id(&ft.ssas_name),
        fact_table: ft,
        dimensions: dims,
        date_roles: dates,
        calculated_tables: calcs,
        lookup_tables: lookups,
        relationships: rels,
        roles,
        data_sources,
    }
}

// ---- renderers ----

fn render_proxy_config(m: &ConversionModel) -> String {
    let ft = &m.fact_table;
    let dims = render_dimension_configs(m);
    let meas = render_measure_configs(m);
    let facts = render_fact_table_configs(m);
    let rels = render_relationships(m);
    let roles = render_roles(m);
    let ti_block = render_time_intelligence_block(m);

    format!(
        r##"{{{{
  "catalog": "{catalog}",
  "cube": "{cube}",
  "source_name": "{source}",
  "table_name": "{table}",
  "dialect": "duckdb",
  "malloy_model_file": "model.malloy",
  "db_path": {db_path},
  "fact_tables": [
{facts}
  ],
  "relationships": [
{rels}
  ],
  "roles": [
{roles}
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
        db_path = if m.date_roles.is_empty() { "null".to_string() } else { format!("\"data/{}.db\"", malloy_name(&m.cube)) },
        facts = facts,
        rels = rels,
        roles = roles,
        ti = ti_block,
        dims = dims,
        meas = meas,
    ).replace("{{", "{").replace("}}", "}")
}

fn render_roles(m: &ConversionModel) -> String {
    let mut out = String::new();
    for (i, r) in m.roles.iter().enumerate() {
        // Render members
        let members_json: String = r.members.iter()
            .map(|m| format!(
                r##"{{{{\n          \"member_name\": \"{}\",\n          \"member_type\": \"{}\"\n        }}}}"##,
                json_escape(&m.member_name),
                json_escape(&m.member_type),
            ).replace("{{", "{").replace("}}", "}"))
            .collect::<Vec<_>>()
            .join(",\n");

        // Render table permissions
        let tp_json: String = r.table_permissions.iter()
            .map(|tp| {
                let dax = tp.dax_filter.as_ref()
                    .map(|s| format!("\n          \"dax_filter\": \"{}\",", json_escape(s)))
                    .unwrap_or_default();
                format!(
                    r##"{{{{\n          \"table\": \"{}\",\n          \"filter_expression\": \"\",{}\n          \"metadata_permission\": \"{}\"\n        }}}}"##,
                    json_escape(&tp.table),
                    dax,
                    tp.metadata_permission,
                ).replace("{{", "{").replace("}}", "}")
            })
            .collect::<Vec<_>>()
            .join(",\n");

        out.push_str(&format!(
            r##"    {{{{
      "name": "{}",
      "description": "{}",
      "model_permission": "{}",
      "members": [
        {}
      ],
      "table_permissions": [
        {}
      ]
    }}}}"##,
            json_escape(&r.name),
            json_escape(&r.description),
            r.model_permission,
            members_json,
            tp_json,
        ).replace("{{", "{").replace("}}", "}"));
        if i + 1 < m.roles.len() {
            out.push_str(",\n");
        }
    }
    out
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
        let dax_expr = meas.expression.as_str();
        // For time measures, extract the inner aggregation for the sql_expr.
        let (time_class, flag_col, time_dim_id) = match meas.classification.as_str() {
            "time_ytd" => ("time_ytd", "ytd_flag",
                m.date_roles.first().map(|d| d.ssas_name.as_str()).unwrap_or("Date")),
            "time_prior_year" => ("time_prior_year", "prior_year_ytd_flag",
                m.date_roles.first().map(|d| d.ssas_name.as_str()).unwrap_or("Date")),
            _ => ("", "", ""),
        };
        let sql = if !time_class.is_empty() {
            let inner = extract_ti_inner(dax_expr);
            let malloy = dax_to_malloy_expr(&inner);
            malloy_to_sql(&malloy).unwrap_or_else(|| "null".to_string())
        } else {
            dax_to_sql_hint(dax_expr, &meas.classification).unwrap_or_else(|| "null".to_string())
        };
        // When the converter cannot produce real SQL for a "simple" measure,
        // downgrade it to sql_fallback so the runtime never executes a placeholder.
        let effective_class = if meas.classification == "simple" && dax_to_sql_hint(dax_expr, &meas.classification).is_none() {
            "sql_fallback"
        } else {
            meas.classification.as_str()
        };
        let pe = if effective_class == "simple" || effective_class == "time_ytd" || effective_class == "time_prior_year" {
            if !time_class.is_empty() {
                dax_to_malloy_expr(&extract_ti_inner(dax_expr))
            } else {
                dax_to_malloy_expr(dax_expr)
            }
        } else {
            String::new()
        };
        let fb_line = if effective_class == "sql_fallback" {
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

fn dax_to_sql_hint(expr: &str, class: &str) -> Option<String> {
    match class {
        "simple" => {
            let malloy = dax_to_malloy_expr(expr);
            malloy_to_sql(&malloy)
        }
        _ => Some("null".to_string()),
    }
}

fn malloy_to_sql(malloy: &str) -> Option<String> {
    // Only return SQL for patterns the converter can truly lower.
    // Numeric constants are the only safe case; all aggregate/expression
    // patterns must be explicitly handwritten as fallback SQL.
    if let Ok(v) = malloy.trim().parse::<f64>() {
        return Some(format!("{}", v));
    }
    None
}

fn dax_to_malloy_expr(dax: &str) -> String {
    let dax = normalize_dax(dax);
    let dax = dax.as_str();
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
            let expr = meas.expression.as_str();
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
    generate_fallback_sql_recursive(meas, model, &mut Vec::new())
}

fn generate_fallback_sql_recursive(meas: &MeasureInfo, model: &ConversionModel, visited: &mut Vec<String>) -> String {
    let dax_raw = meas.expression.clone();
    let dax = dax_raw.trim_start().trim_start_matches("=").trim().to_string();
    let dax_one_line = normalize_dax(&dax);
    let upper_one = dax_one_line.to_uppercase();

    // Pattern 1: MEDIAN(col) — DuckDB native
    if upper_one.contains("MEDIAN(") {
        if let Some(col_expr) = extract_dax_unary(&upper_one, "MEDIAN(") {
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
    if upper_one.contains("ALLSELECTED") && upper_one.contains("ISONORAFTER") {
        return generate_cumulative_sql(&dax_one_line, &upper_one, meas, model);
    }

    // Pattern 3: SUMX(FILTER(table, col=val), qty_col * RELATED(dim.dimcol))
    if upper_one.contains("SUMX(") && upper_one.contains("FILTER(") && upper_one.contains("RELATED(") {
        if let Some(sql) = generate_sumx_filter_related(&dax_one_line, &upper_one, model) {
            return sql;
        }
    }

    // Pattern 4: CALCULATE(SUM(col), filter) — simple filtered SUM
    if upper_one.contains("CALCULATE(") && upper_one.contains("SUM(") {
        if let Some(sql) = generate_calculate_sum(&dax_one_line, &upper_one, model) {
            return sql;
        }
    }

    // Pattern 5: [MeasureA] - [MeasureB] — arithmetic between two measures
    if dax_one_line.trim().starts_with("[") && (dax_one_line.contains("- [") || dax_one_line.contains("-[")) {
        if let Some(sql) = generate_measure_arithmetic(&dax_one_line, model, visited) {
            return sql;
        }
    }

    // Pattern 6: DIVIDE([MeasureA], [MeasureB], ...) — safe division
    if upper_one.starts_with("DIVIDE(") && dax_one_line.contains('[') {
        if let Some(sql) = generate_divide_measure_recursive(&dax_one_line, model, visited) {
            return sql;
        }
    }

    // Unsupported: keep annotated stub
    render_fallback_stub(&meas.name, &dax_one_line)
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

fn generate_sumx_filter_related(dax: &str, upper: &str, model: &ConversionModel) -> Option<String> {
    let fact = malloy_name(&model.fact_table.name);
    let filter_parts = extract_filter_eq(dax)?;
    let filter_col_raw = &filter_parts.0;
    let filter_col = resolve_source_column(filter_col_raw, model);
    let filter_val = &filter_parts.1;
    let qty_col_raw = extract_first_mul_col(dax)?;
    let qty_col = resolve_source_column(&qty_col_raw, model);
    let related = extract_related_ref(dax)?;
    let dim_table = malloy_name(&related.0);
    let dim_col_raw = &related.1;
    let dim_col = resolve_source_column(dim_col_raw, model);
    let join_col = model.relationships.iter()
        .find(|r| malloy_name(&r.to_table) == dim_table)
        .and_then(|r| {
            // Resolve SSAS column ref to actual sourceColumn
            let raw = r.from_column.clone();
            Some(resolve_source_column(&raw, model))
        })
        .unwrap_or_else(|| "id".into());

    let sql = format!(
        "-- Auto-generated from DAX: {dax}\n\
         -- SUMX(FILTER(...), qty * RELATED(dim.col))\n\n\
         SELECT COALESCE(SUM(f.{qty_col} * CAST(d.{dim_col} AS DOUBLE)), 0) AS value\n\
         FROM {fact} f\n\
         JOIN {dim_table} d ON f.{join_col} = d.{join_col}\n\
         WHERE f.{filter_col} = {filter_val};\n",
        dax = dax, qty_col = qty_col, dim_col = dim_col,
        fact = fact, dim_table = dim_table, join_col = join_col,
        filter_col = filter_col, filter_val = filter_val,
    );
    Some(sql)
}

fn extract_filter_eq(dax: &str) -> Option<(String, String)> {
    // Parse 'Table'[Col] = value from FILTER(...) expression
    let after_filter = dax.find("FILTER(")?;
    let inner = &dax[after_filter + "FILTER(".len()..];
    // Find the comparison: find '[' char after the first comma, then extract Col] = val
    let first_comma = inner.find(',')?;
    let rest = &inner[first_comma + 1..].trim();
    let bracket_start = rest.find('[')?;
    let bracket_end = rest[bracket_start..].find(']')? + bracket_start;
    let col_name = &rest[bracket_start + 1..bracket_end];
    let after_eq = rest[bracket_end + 1..].trim();
    let eq_pos = after_eq.find('=')?;
    let val = after_eq[eq_pos + 1..].trim();
    // Stop at space, comma, or paren
    let val_end = val.find(|c: char| c == ' ' || c == ',' || c == ')').unwrap_or(val.len());
    let val = &val[..val_end];
    Some((col_name.trim().to_string(), val.trim().to_string()))
}

fn extract_first_mul_col(dax: &str) -> Option<String> {
    // Extract the first column in multiplication after FILTER
    // Pattern: FILTER(...), 'Table'[QtyCol] * ...
    let after_filter = dax.find("FILTER(")?;
    let inner = &dax[after_filter + "FILTER(".len()..];
    // Find the closing paren of FILTER (match depth)
    let mut depth = 1;
    let mut filter_end = 0;
    for (i, c) in inner.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => { depth -= 1; if depth == 0 { filter_end = i; break; } }
            _ => {}
        }
    }
    let after_filter_end = &inner[filter_end + 1..].trim().trim_start_matches(',').trim();
    // Now find the first bracketed column reference (e.g., 'Sales'[Qty])
    let bracket_start = after_filter_end.find('[')?;
    let bracket_end = after_filter_end[bracket_start..].find(']')? + bracket_start;
    Some(after_filter_end[bracket_start + 1..bracket_end].to_string())
}

fn extract_related_ref(dax: &str) -> Option<(String, String)> {
    // Extract RELATED('DimTable'[DimCol])
    let related_pos = dax.find("RELATED(")?;
    let inner = &dax[related_pos + "RELATED(".len()..];
    let close_paren = inner.find(')')?;
    let related_inner = &inner[..close_paren];
    // Parse 'DimTable'[DimCol] or [DimCol]
    let (table, col) = parse_dax_col_ref(related_inner.trim())?;
    Some((table, col))
}

/// Resolve a DAX column name to the actual sourceColumn (DB column) name.
fn resolve_source_column(ssas_name: &str, model: &ConversionModel) -> String {
    let needle = ssas_name.trim().to_lowercase().replace(' ', "");
    for t in std::iter::once(&model.fact_table)
        .chain(model.dimensions.iter())
        .chain(model.date_roles.iter())
        .chain(model.lookup_tables.iter())
    {
        for c in &t.columns {
            if c.name.to_lowercase().replace(' ', "") == needle {
                return c.source_column.clone();
            }
        }
    }
    // Fallback: lowercase with underscores
    malloy_name(ssas_name)
}
fn parse_dax_col_ref(s: &str) -> Option<(String, String)> {
    let s = s.trim();
    if let Some(apos) = s.find('\'') {
        let table_end = s[apos + 1..].find('\'')? + apos + 1;
        let table = &s[apos + 1..table_end];
        let rest = s[table_end + 1..].trim();
        let col = if rest.starts_with('[') && rest.contains(']') {
            rest[1..].split(']').next()?
        } else {
            return None;
        };
        Some((table.to_string(), col.to_string()))
    } else if s.starts_with('[') && s.contains(']') {
        Some((String::new(), s[1..].split(']').next()?.to_string()))
    } else {
        None
    }
}

fn generate_calculate_sum(dax: &str, upper: &str, model: &ConversionModel) -> Option<String> {
    let fact = malloy_name(&model.fact_table.name);
    let sum_col_raw = extract_aggregate_col(dax, "SUM")?;
    let sum_col_name = resolve_source_column(&sum_col_raw, model);
    let filter_info = extract_calculate_filter_eq(dax)?;
    let filter_col = resolve_source_column(&filter_info.0, model);
    let filter_val = &filter_info.1;

    let sql = format!(
        "-- Auto-generated from DAX: {dax}\n\
         -- CALCULATE(SUM(col), filter)\n\n\
         SELECT COALESCE(SUM(CAST({sum_col_name} AS DOUBLE)), 0) AS value\n\
         FROM {fact}\n\
         WHERE {filter_col} = {filter_val};\n",
        dax = dax, sum_col_name = sum_col_name, fact = fact,
        filter_col = filter_col, filter_val = filter_val,
    );
    Some(sql)
}

fn extract_aggregate_col(dax: &str, func: &str) -> Option<String> {
    let pos = dax.to_uppercase().find(&format!("{}(", func))?;
    let inner = &dax[pos + func.len() + 1..];
    // Parse 'Table'[Col] — the aggregate's first argument
    if let Some((_, col)) = parse_dax_col_ref(inner.trim()) {
        return Some(col);
    }
    None
}

fn extract_calculate_filter_eq(dax: &str) -> Option<(String, String)> {
    // In CALCULATE(expr, 'Table'[Col] = val, ...)
    // Find the first comma after CALCULATE(, then parse the filter
    let calc_start = dax.find("CALCULATE(")?;
    let inner = &dax[calc_start + "CALCULATE(".len()..];
    // Skip past the aggregate expression (match parens)
    let mut depth = 1;
    let mut comma_pos = None;
    for (i, c) in inner.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => { depth -= 1; if depth == 0 { break; } }
            ',' if depth == 1 => { comma_pos = Some(i); break; }
            _ => {}
        }
    }
    let rest = &inner[comma_pos? + 1..].trim();
    // Now parse 'Table'[Col] = val
    let (table, col) = parse_dax_col_ref(rest)?;
    let after_col = rest[rest.find(']')? + 1..].trim();
    let eq_pos = after_col.find('=')?;
    let val = after_col[eq_pos + 1..].trim();
    let val_end = val.find(|c: char| c == ' ' || c == ',' || c == ')').unwrap_or(val.len());
    Some((col, val[..val_end].to_string()))
}

fn generate_measure_arithmetic(dax: &str, model: &ConversionModel, visited: &mut Vec<String>) -> Option<String> {
    let parts: Vec<&str> = dax.split(|c: char| c == '-' || c == '+' || c == '*' || c == '/')
        .filter(|p| !p.trim().is_empty())
        .collect();
    if parts.len() < 2 { return None; }
    let op = if dax.contains(" - [") || dax.contains("- [") { " - " }
             else if dax.contains(" + [") || dax.contains("+ [") { " + " }
             else if dax.contains(" * [") || dax.contains("* [") { " * " }
             else if dax.contains(" / [") || dax.contains("/ [") { " / " }
             else { return None; };

    let measure_names: Vec<String> = parts.iter()
        .map(|p| p.trim().trim_matches(|c: char| c == '[' || c == ']' || c == ' ').to_string())
        .collect();

    let mut subqueries = Vec::new();
    for name in &measure_names {
        let inner_sql = generate_sql_for_measure(name, model, visited)?;
        subqueries.push(format!("({inner_sql})"));
    }

    let sql = format!(
        "-- Auto-generated from DAX: {dax}\n\
         -- Arithmetic between measures\n\n\
         SELECT COALESCE({subq_a}, 0) {op} COALESCE({subq_b}, 0) AS value;\n",
        dax = dax,
        subq_a = subqueries.get(0)?,
        subq_b = subqueries.get(1)?,
        op = op.trim(),
    );
    Some(sql)
}

fn generate_divide_measure_recursive(dax: &str, model: &ConversionModel, visited: &mut Vec<String>) -> Option<String> {
    let rest = dax.trim_start_matches("DIVIDE(").trim();
    let args = split_args(rest);
    if args.len() < 2 { return None; }
    let meas_a = args[0].trim().trim_matches(|c: char| c == '[' || c == ']' || c == ' ').to_string();
    let meas_b = args[1].trim().trim_matches(|c: char| c == '[' || c == ']' || c == ' ').to_string();
    let subq_a = generate_sql_for_measure(&meas_a, model, visited)?;
    let subq_b = generate_sql_for_measure(&meas_b, model, visited)?;

    Some(format!(
        "-- Auto-generated from DAX: {dax}\n\
         -- DIVIDE(a, b) safe division\n\n\
         SELECT CASE WHEN COALESCE(({subq_b}), 0) = 0 THEN NULL ELSE COALESCE(({subq_a}), 0) / COALESCE(({subq_b}), 0) END AS value;\n",
        dax = dax,
        subq_a = subq_a,
        subq_b = subq_b,
    ))
}

fn generate_sql_for_measure(name: &str, model: &ConversionModel, visited: &[String]) -> Option<String> {
    let target = name.trim().to_lowercase();

    if visited.iter().any(|v| v == &target) {
        return None;
    }

    let meas = model.fact_table.measures.iter()
        .find(|m| m.name.trim().to_lowercase() == target)?;

    let mut new_visited = visited.to_vec();
    new_visited.push(target.clone());

    let all_measures: Vec<&MeasureInfo> = model.fact_table.measures.iter()
        .chain(model.dimensions.iter().flat_map(|t| &t.measures))
        .chain(model.date_roles.iter().flat_map(|t| &t.measures))
        .collect();
    let meas_ref = all_measures.iter()
        .find(|m| m.name.trim().to_lowercase() == target)
        .copied()
        .unwrap_or(meas);

    let sql = generate_fallback_sql_recursive(meas_ref, model, &mut new_visited);

    if sql.contains("SELECT 1 AS dummy") || sql.contains("TODO") {
        return None;
    }

    let sql = sql.trim().trim_end_matches(';').to_string();
    if sql.is_empty() {
        return None;
    }
    Some(sql)
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
    out.push_str(&format!("- M-partition tables: {} (load_data.sql attempts automated loading, see load_data.sql for details)\n\n",
        if m.fact_table.is_m_partition() { 1usize } else { 0 }
        + m.dimensions.iter().filter(|t| t.is_m_partition()).count()
        + m.date_roles.iter().filter(|t| t.is_m_partition()).count()));

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
        let dax = m.expression.as_str();
        let malloy = dax_to_malloy_expr(dax);
        out.push_str(&format!("| {} | {} | {} |\n", m.name, dax, malloy));
    }

    out.push_str("\n## SQL fallback measures\n\n");
    out.push_str("| Measure | DAX pattern | Fallback file |\n|---|---|---|\n");
    for m in &fallback {
        let dax = m.expression.as_str();
        out.push_str(&format!("| {} | {} | sql_fallback/{}.sql |\n",
            m.name, dax, malloy_name(&m.name)));
    }

    if !manual.is_empty() {
        out.push_str("\n## Manual review required\n\n");
        out.push_str("| Measure | DAX pattern |\n|---|---|\n");
        for m in &manual {
            out.push_str(&format!("| {} | {} |\n", m.name, m.expression.as_str()));
        }
    }

    out.push_str("\n## Data loading\n\n");
    out.push_str("The converter generates three SQL files for data loading:\n\n");
    out.push_str("- `schema.sql` — CREATE TABLE statements (run first)\n");
    out.push_str("- `load_data.sql` — loads real data from source databases (requires DuckDB extensions or CSV files)\n");
    out.push_str("- `load_dummy_data.sql` — generates synthetic data for testing (always works)\n\n");

    if !m.data_sources.is_empty() {
        out.push_str("### Data sources detected\n\n");
        out.push_str("| Name | Provider | Server | Database |\n|---|---|---|---|\n");
        for ds in &m.data_sources {
            out.push_str(&format!("| {} | {} | {} | {} |\n", ds.name, ds.provider, ds.server, ds.database));
        }
        out.push('\n');
    }

    out.push_str("### Quick start\n\n");
    let cube_db = format!("{}.db", malloy_name(&m.cube));
    out.push_str(&format!(
        "```\nduckdb data/{cube_db} < bootstrap.sql\n```\n\n\
         This creates the schema, seeds `date_dim` (if needed), and loads dummy data.\n\
         For real data, edit `bootstrap.sql` to use `load_data.sql` instead.\n\n",
        cube_db = cube_db,
    ));

    out.push_str("### Tables to load\n\n");
    out.push_str(&format!("- [ ] `{}` (fact)\n", malloy_name(&m.fact_table.name)));
    for t in &m.dimensions {
        out.push_str(&format!("- [ ] `{}` (dimension)\n", malloy_name(&t.name)));
    }
    for t in &m.date_roles {
        out.push_str(&format!("- [ ] `{}` (date-role, seeded by seed_date_dim.sql)\n", malloy_name(&t.name)));
    }
    for t in &m.lookup_tables {
        out.push_str(&format!("- [ ] `{}` (lookup)\n", malloy_name(&t.name)));
    }

    if !m.roles.is_empty() {
        out.push_str("\n## Roles\n\n");
        out.push_str(&format!("{} roles detected\n\n", m.roles.len()));
        for r in &m.roles {
            out.push_str(&format!("### {} ({})\n\n", r.name, r.model_permission));
            if !r.description.is_empty() {
                out.push_str(&format!("{}\n\n", r.description));
            }

            if !r.members.is_empty() {
                out.push_str("**Members:**\n\n");
                out.push_str("| Name | Type |\n|---|---|\n");
                for m in &r.members {
                    out.push_str(&format!("| {} | {} |\n", m.member_name, m.member_type));
                }
                out.push('\n');
            }

            if r.table_permissions.is_empty() {
                out.push_str("No table permissions — full access to all tables.\n\n");
            } else {
                out.push_str("**Table permissions:**\n\n");
                out.push_str("| Table | SQL filter | DAX filter | Metadata permission | Status |\n|---|---|---|---|---|\n");
                for tp in &r.table_permissions {
                    let dax_str = tp.dax_filter.as_deref().unwrap_or("-");
                    let sql_str = if tp.filter_expression.is_empty() { "(empty)" } else { &tp.filter_expression };
                    let status = if tp.metadata_permission == "none" {
                        "OLS — table hidden"
                    } else if tp.dax_filter.is_some() && tp.filter_expression.is_empty() {
                        "DAX filter preserved, SQL filter empty — manual SQL translation required"
                    } else if !tp.filter_expression.is_empty() {
                        "Enforced (SQL filter)"
                    } else {
                        "No filter — full access"
                    };
                    out.push_str(&format!("| {} | {} | {} | {} | {} |\n", tp.table, sql_str, dax_str, tp.metadata_permission, status));
                }
                out.push('\n');
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_generic_model() -> ConversionModel {
        let fact = TableInfo {
            name: "orders".into(),
            ssas_name: "Orders".into(),
            description: String::new(),
            columns: vec![
                ColumnInfo { name: "amount".into(), data_type: "double".into(), source_column: "amount".into(), is_hidden: false },
                ColumnInfo { name: "qty".into(), data_type: "int64".into(), source_column: "qty".into(), is_hidden: false },
                ColumnInfo { name: "status".into(), data_type: "int64".into(), source_column: "status".into(), is_hidden: false },
                ColumnInfo { name: "itemid".into(), data_type: "int64".into(), source_column: "itemid".into(), is_hidden: false },
            ],
            measures: vec![
                MeasureInfo {
                    name: "Total Sales".into(),
                    expression: "= CALCULATE ( SUM ( 'Orders'[Amount] ), 'Orders'[Status] = 1 )".into(),
                    display_folder: String::new(),
                    classification: "sql_fallback".into(),
                },
                MeasureInfo {
                    name: "Total Cost".into(),
                    expression: "= SUMX ( FILTER ( 'Orders', 'Orders'[Status] = 1 ), 'Orders'[Qty] * RELATED ( 'Items'[Unit Cost] ) )".into(),
                    display_folder: String::new(),
                    classification: "sql_fallback".into(),
                },
                MeasureInfo {
                    name: "Net Profit".into(),
                    expression: "= [Total Sales] - [Total Cost]".into(),
                    display_folder: String::new(),
                    classification: "sql_fallback".into(),
                },
                MeasureInfo {
                    name: "Margin Pct".into(),
                    expression: "= DIVIDE ( [Net Profit], [Total Sales], 0 )".into(),
                    display_folder: String::new(),
                    classification: "sql_fallback".into(),
                },
            ],
            partitions: vec![],
            hierarchies: vec![],
        };

        let items = TableInfo {
            name: "items".into(),
            ssas_name: "Items".into(),
            description: String::new(),
            columns: vec![
                ColumnInfo { name: "itemid".into(), data_type: "int64".into(), source_column: "itemid".into(), is_hidden: false },
                ColumnInfo { name: "unitcost".into(), data_type: "double".into(), source_column: "unitcost".into(), is_hidden: false },
            ],
            measures: vec![],
            partitions: vec![],
            hierarchies: vec![],
        };

        ConversionModel {
            catalog: "TEST".into(),
            cube: "Orders".into(),
            fact_table: fact,
            dimensions: vec![],
            date_roles: vec![],
            calculated_tables: vec![],
            lookup_tables: vec![items],
            relationships: vec![RelInfo {
                from_table: "Orders".into(),
                from_column: "Item ID".into(),
                to_table: "Items".into(),
                to_column: "Item ID".into(),
            }],
            roles: vec![],
            data_sources: vec![],
        }
    }

    #[test]
    fn generic_calculate_sum_produces_real_sql() {
        let model = make_generic_model();
        let meas = model.fact_table.measures.iter()
            .find(|m| m.name == "Total Sales").unwrap();
        let sql = generate_fallback_sql(meas, &model);
        assert!(!sql.contains("SELECT 1 AS dummy"), "should not be a stub");
        assert!(sql.contains("amount"), "should resolve amount column");
        assert!(sql.contains("status"), "should resolve status filter");
        assert!(sql.contains("orders"), "should use orders table");
    }

    #[test]
    fn generic_sumx_filter_related_produces_real_sql() {
        let model = make_generic_model();
        let meas = model.fact_table.measures.iter()
            .find(|m| m.name == "Total Cost").unwrap();
        let sql = generate_fallback_sql(meas, &model);
        assert!(!sql.contains("SELECT 1 AS dummy"), "should not be a stub");
        assert!(sql.contains("qty"), "should resolve qty column");
        assert!(sql.contains("unitcost"), "should resolve unitcost column");
        assert!(sql.contains("items"), "should join items table");
    }

    #[test]
    fn generic_measure_arithmetic_produces_real_sql() {
        let model = make_generic_model();
        let meas = model.fact_table.measures.iter()
            .find(|m| m.name == "Net Profit").unwrap();
        let sql = generate_fallback_sql(meas, &model);
        assert!(!sql.contains("SELECT 1 AS dummy"), "should not be a stub");
        assert!(sql.contains("amount"), "should contain Total Sales subquery");
        assert!(sql.contains("qty"), "should contain Total Cost subquery");
    }

    #[test]
    fn generic_divide_measure_produces_real_sql() {
        let model = make_generic_model();
        let meas = model.fact_table.measures.iter()
            .find(|m| m.name == "Margin Pct").unwrap();
        let sql = generate_fallback_sql(meas, &model);
        assert!(!sql.contains("SELECT 1 AS dummy"), "should not be a stub");
        assert!(sql.contains("CASE WHEN"), "should be safe division");
        assert!(sql.contains("amount"), "should contain Total Sales subquery");
    }

    #[test]
    fn generate_sql_for_measure_no_hardcoded_retail_names() {
        let source = include_str!("convert_tabular.rs");
        assert!(!source.contains("\"TOTAL REVENUE\""), "no hardcoded TOTAL REVENUE");
        assert!(!source.contains("\"TOTAL COGS\""), "no hardcoded TOTAL COGS");
        assert!(!source.contains("\"GROSS PROFIT\""), "no hardcoded GROSS PROFIT");
    }

    #[test]
    fn test_conversion_model_has_data_sources() {
        let mut parsed = TabularModel {
            name: "Test".into(),
            compatibility_level: 1500,
            tables: vec![],
            relationships: vec![],
            roles: vec![],
            data_sources: vec![DataSourceInfo {
                name: "Src".into(),
                provider: "SqlClient".into(),
                server: "srv".into(),
                database: "db".into(),
                connection_string: "".into(),
            }],
        };
        // Add at least one table to avoid panic
        parsed.tables.push(TableInfo {
            name: "Sales".into(),
            ssas_name: "Sales".into(),
            description: String::new(),
            columns: vec![ColumnInfo {
                name: "Amount".into(),
                data_type: "double".into(),
                source_column: "amount".into(),
                is_hidden: false,
            }],
            measures: vec![],
            partitions: vec![],
            hierarchies: vec![],
        });
        let model = classify_model(parsed);
        assert_eq!(model.data_sources.len(), 1);
        assert_eq!(model.data_sources[0].name, "Src");
        assert_eq!(model.data_sources[0].server, "srv");
    }

    #[test]
    fn test_run_emits_load_scripts() {
        let src = "data/retailanalytics_tabular";
        let out_dir = std::env::temp_dir().join("test_run_emits_load_scripts");
        let _ = fs::remove_dir_all(&out_dir);
        let out_str = out_dir.to_string_lossy().to_string();

        let code = run(vec![
            "convert-tabular".into(),
            src.into(),
            out_str,
            "--dummy-rows=5000".into(),
        ]);
        assert_eq!(code, 0);

        assert!(out_dir.join("load_data.sql").exists(), "load_data.sql should exist");
        assert!(out_dir.join("load_dummy_data.sql").exists(), "load_dummy_data.sql should exist");
        assert!(out_dir.join("bootstrap.sql").exists(), "bootstrap.sql should exist");
        assert!(out_dir.join("proxy-config.json").exists(), "proxy-config.json should exist");

        // Verify dummy data uses the flag value
        let dummy = fs::read_to_string(out_dir.join("load_dummy_data.sql")).unwrap();
        assert!(dummy.contains("generate_series(1, 5000)"), "should use --dummy-rows=5000 for fact table");
        assert!(dummy.contains("generate_series(1, 500)"), "dimension table rows should be dummy_rows/10");

        // Cleanup
        let _ = fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn test_run_always_emits_bootstrap() {
        // Create a minimal .bim file with no date roles
        let src_dir = std::env::temp_dir().join("test_no_date_model");
        let _ = fs::create_dir_all(&src_dir);
        let bim_path = src_dir.join("model.bim");
        // Minimal BIM with a single table and no date tables
        let bim_content = r#"{
  "name": "NoDate",
  "compatibilityLevel": 1500,
  "model": {
    "tables": [
      {
        "name": "Sales",
        "columns": [
          { "name": "Amount", "dataType": "double", "sourceColumn": "amount" }
        ],
        "partitions": [
          {
            "name": "Part",
            "source": {
              "type": "query",
              "query": "SELECT * FROM dbo.sales"
            }
          }
        ]
      },
      {
        "name": "Products",
        "columns": [
          { "name": "Name", "dataType": "string", "sourceColumn": "name" }
        ],
        "partitions": [
          {
            "name": "Part",
            "source": {
              "type": "query",
              "query": "SELECT * FROM dbo.products"
            }
          }
        ]
      }
    ],
    "relationships": []
  }
}"#;
        fs::write(&bim_path, bim_content).unwrap();

        let out_dir = std::env::temp_dir().join("test_no_date_out");
        let _ = fs::remove_dir_all(&out_dir);
        let out_str = out_dir.to_string_lossy().to_string();

        let code = run(vec![
            "convert-tabular".into(),
            bim_path.to_string_lossy().to_string(),
            out_str,
        ]);
        assert_eq!(code, 0);

        // bootstrap.sql must always be emitted
        assert!(out_dir.join("bootstrap.sql").exists(), "bootstrap.sql should always exist");
        assert!(!out_dir.join("seed_date_dim.sql").exists(), "seed_date_dim.sql should NOT exist (no date roles)");

        // Cleanup
        let _ = fs::remove_dir_all(&src_dir);
        let _ = fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn test_bootstrap_references_dummy_data() {
        // Use the retail fixture which has date roles
        let src = "data/retailanalytics_tabular";
        let out_dir = std::env::temp_dir().join("test_bootstrap_refs");
        let _ = fs::remove_dir_all(&out_dir);
        let out_str = out_dir.to_string_lossy().to_string();

        let code = run(vec![
            "convert-tabular".into(),
            src.into(),
            out_str,
        ]);
        assert_eq!(code, 0);

        let bootstrap = fs::read_to_string(out_dir.join("bootstrap.sql")).unwrap();
        assert!(bootstrap.contains(".read load_dummy_data.sql"), "bootstrap should reference load_dummy_data.sql");
        assert!(bootstrap.contains(".read schema.sql"), "bootstrap should reference schema.sql");
        assert!(bootstrap.contains("-- .read load_data.sql"), "bootstrap should have load_data.sql commented out");

        // With date roles, should also reference seed_date_dim.sql
        assert!(bootstrap.contains(".read seed_date_dim.sql"), "should reference seed_date_dim.sql when date roles present");

        // Cleanup
        let _ = fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn test_report_documents_data_sources() {
        let mut model = make_generic_model();
        model.data_sources.push(DataSourceInfo {
            name: "TestSource".into(),
            provider: "System.Data.SqlClient".into(),
            server: "MY-SERVER".into(),
            database: "MyDB".into(),
            connection_string: "".into(),
        });

        let report = render_report(&model);
        assert!(report.contains("Data sources detected"), "report should mention data sources");
        assert!(report.contains("TestSource"), "report should include data source name");
        assert!(report.contains("MY-SERVER"), "report should include data source server");
        assert!(report.contains("load_data.sql"), "report should reference load_data.sql");
        assert!(report.contains("load_dummy_data.sql"), "report should reference load_dummy_data.sql");
        assert!(report.contains("bootstrap.sql"), "report should reference bootstrap.sql");
        assert!(report.contains("Quick start"), "report should have quick start section");
    }
}
