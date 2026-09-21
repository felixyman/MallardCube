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
            eprintln!("Usage: mallard convert-tabular <tabulareditor_src> [output_dir]");
            eprintln!("  <tabulareditor_src> can be a directory (folder/TMDL format) or .bim file");
            return 1;
        }
    };
    let out_dir = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "converted-project".into());

    let mut dummy_rows = 10000usize;
    for arg in &args {
        if let Some(val) = arg.strip_prefix("--dummy-rows=")
            && let Ok(n) = val.parse::<usize>()
        {
            dummy_rows = n;
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
            eprintln!(
                "Error: '{}' is neither a .bim file nor a directory with Tabular Editor files",
                src_dir
            );
            eprintln!("Usage: mallard convert-tabular <tabulareditor_src> [output_dir]");
            return 1;
        }
    };
    for w in &warnings {
        eprintln!("WARNING: {}", w);
    }
    let mut model = classify_model(parsed);

    let total = 1
        + model.dimensions.len()
        + model.date_roles.len()
        + model.calculated_tables.len()
        + model.lookup_tables.len();
    eprintln!(
        "Classified {} tables ({} fact, {} dimension, {} date-role, {} calculated, {} lookup)",
        total,
        1,
        model.dimensions.len(),
        model.date_roles.len(),
        model.calculated_tables.len(),
        model.lookup_tables.len()
    );

    fs::create_dir_all(&out_dir).expect("create output dir");
    fs::create_dir_all(format!("{out_dir}/sql_fallback")).ok();

    // Reclassify "simple" measures whose SQL hints return None (placeholder)
    // as "sql_fallback" so the runtime never executes placeholder SQL. The
    // fact table is cloned so the resolver is available while the measures are
    // borrowed mutably.
    let fact_for_resolution = model.fact_table.clone();
    for meas in model
        .fact_table
        .measures
        .iter_mut()
        .chain(model.dimensions.iter_mut().flat_map(|t| &mut t.measures))
        .chain(model.date_roles.iter_mut().flat_map(|t| &mut t.measures))
    {
        if meas.classification == "simple"
            && dax_to_sql_hint(&meas.expression, "simple", &fact_for_resolution).is_none()
        {
            meas.classification = "sql_fallback".to_string();
        }
    }

    // Time intelligence needs flag columns upstream (plan 044, invariant 2).
    // When the measure's date role has no flag column, emit bridge code with an
    // upstream checklist instead of a measure that references a column which
    // does not exist.
    downgrade_time_intelligence_without_flags(&mut model);

    // Generate SQL fallback files
    for meas in &model.fact_table.measures {
        if meas.classification == "sql_fallback" {
            let sql = generate_fallback_sql(meas, &model);
            let file_name = format!("{out_dir}/sql_fallback/{}.sql", normalize_ident(&meas.name));
            fs::write(&file_name, sql).expect("write fallback");
        }
    }

    fs::write(
        format!("{out_dir}/proxy-config.json"),
        render_proxy_config(&model),
    )
    .expect("write config");
    fs::write(format!("{out_dir}/schema.sql"), render_schema(&model)).expect("write schema");

    // Data loading scripts
    let fact_names: Vec<String> = vec![model.fact_table.name.clone()];
    let date_role_names: Vec<String> = model.date_roles.iter().map(|t| t.name.clone()).collect();

    // Reconstruct TabularModel for load script generators
    let tabular_model = model.to_tabular_model();

    fs::write(
        format!("{out_dir}/load_data.sql"),
        data_loader::render_load_script(&tabular_model, &fact_names, &date_role_names),
    )
    .expect("write load_data.sql");

    let dim_rows = (dummy_rows / 10).max(100);
    fs::write(
        format!("{out_dir}/load_dummy_data.sql"),
        data_loader::render_dummy_data_script(
            &tabular_model,
            &fact_names,
            &date_role_names,
            dummy_rows,
            dim_rows,
        ),
    )
    .expect("write load_dummy_data.sql");

    fs::write(
        format!("{out_dir}/conversion-report.md"),
        render_report(&model),
    )
    .expect("write report");

    // Bootstrap script (always emitted)
    let cube_db = format!("{}.db", normalize_ident(&model.cube));
    fs::create_dir_all(format!("{out_dir}/data")).ok();

    let mut bootstrap = format!(
        "-- Bootstrap script for {cube}\n\
         -- Run against DuckDB to create a runnable database.\n\
         --   duckdb {cube_db} < bootstrap.sql\n\n\
         .read schema.sql\n",
        cube = model.cube,
        cube_db = cube_db,
    );

    bootstrap.push_str(".read load_dummy_data.sql\n");
    bootstrap.push_str("\n-- For real data, replace the line above with:\n");
    bootstrap.push_str("-- .read load_data.sql\n");

    fs::write(format!("{out_dir}/bootstrap.sql"), bootstrap).expect("write bootstrap.sql");

    eprintln!("Generated project in {out_dir}/");
    eprintln!(
        "  Files: proxy-config.json, schema.sql, load_data.sql, load_dummy_data.sql, bootstrap.sql, conversion-report.md"
    );
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
            let has_lookupvalue_only = !t.measures.is_empty()
                && t.measures
                    .iter()
                    .all(|m| m.expression.to_uppercase().contains("LOOKUPVALUE"));
            if (lower.contains("f_") || t.measures.len() > 5) && !has_lookupvalue_only {
                fact.push(t);
            } else if lower.contains("calendar") || lower == "dates" {
                dates.push(t);
            } else if lower.starts_with("dw_sales d_") {
                dims.push(t);
            } else {
                lookups.push(t);
            }
        }
    }

    // Fallback: if no fact table detected by heuristics, use relationship fromTable
    // as a signal — the table referenced most as a relationship source is the fact.
    if fact.is_empty() {
        let mut from_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for r in &rels {
            *from_counts.entry(r.from_table.clone()).or_insert(0) += 1;
        }
        if let Some((best_name, _)) = from_counts
            .into_iter()
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
            name: "unknown".into(),
            ssas_name: "unknown".into(),
            description: String::new(),
            columns: vec![],
            measures: vec![],
            partitions: vec![],
            hierarchies: vec![],
        }
    };

    // Heuristic: if the selected fact table has no outgoing relationships but
    // another table in lookups does, prefer the relationships-bearing table.
    // Metadata tables (e.g. Info with LOOKUPVALUE measures) should not be facts.
    let fact_rel_count = rels.iter().filter(|r| r.from_table == ft.name).count();
    if fact_rel_count == 0
        && !lookups.is_empty()
        && let Some(pos) = lookups
            .iter()
            .position(|t| rels.iter().any(|r| r.from_table == t.name))
    {
        let better = lookups.remove(pos);
        lookups.push(std::mem::replace(&mut ft, better));
    }

    // Merge DAX calculated table measures into the fact table
    let mut calc_measures: Vec<MeasureInfo> = calcs
        .iter()
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
        source = normalize_ident(&ft.ssas_name),
        table = normalize_ident(&ft.name),
        db_path = if m.date_roles.is_empty() {
            "null".to_string()
        } else {
            format!("\"data/{}.db\"", normalize_ident(&m.cube))
        },
        facts = facts,
        rels = rels,
        roles = roles,
        ti = ti_block,
        dims = dims,
        meas = meas,
    )
    .replace("{{", "{")
    .replace("}}", "}")
}

fn render_roles(m: &ConversionModel) -> String {
    let mut out = String::new();
    for (i, r) in m.roles.iter().enumerate() {
        let members_json: String = r.members.iter()
            .map(|m| format!(
                "        {{\n          \"member_name\": \"{}\",\n          \"member_type\": \"{}\"\n        }}",
                json_escape(&m.member_name),
                json_escape(&m.member_type),
            ))
            .collect::<Vec<_>>()
            .join(",\n");

        let tp_json: String = r.table_permissions.iter()
            .map(|tp| {
                let dax = tp.dax_filter.as_ref()
                    .map(|s| format!("\n          \"dax_filter\": \"{}\",", json_escape(s)))
                    .unwrap_or_default();
                format!(
                    "        {{\n          \"table\": \"{}\",\n          \"filter_expression\": \"\",{}\n          \"metadata_permission\": \"{}\"\n        }}",
                    json_escape(&tp.table),
                    dax,
                    tp.metadata_permission,
                )
            })
            .collect::<Vec<_>>()
            .join(",\n");

        out.push_str(
            &format!(
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
            )
            .replace("{{", "{")
            .replace("}}", "}"),
        );
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
        sn = normalize_ident(&ft.ssas_name),
        tn = normalize_ident(&ft.name),
        cube = m.cube,
    )
    .replace("{{", "{")
    .replace("}}", "}")
}

fn render_relationships(m: &ConversionModel) -> String {
    let mut out = String::new();
    let dim_tables: Vec<&TableInfo> = m
        .dimensions
        .iter()
        .chain(&m.date_roles)
        .chain(&m.lookup_tables)
        .collect();
    let all_tables: Vec<&TableInfo> = std::iter::once(&m.fact_table)
        .chain(dim_tables.iter().copied())
        .collect();
    let total = m.relationships.len();
    let mut emitted = 0usize;
    for rel in &m.relationships {
        if let Some(t) = dim_tables
            .iter()
            .find(|t| t.name == rel.to_table || t.ssas_name == rel.to_table)
        {
            let dim_id = t.ssas_name.clone();
            // Relationship endpoints are model column names; schema.sql is
            // built from source columns, so resolve both sides.
            let fact_col = all_tables
                .iter()
                .find(|t| t.name == rel.from_table || t.ssas_name == rel.from_table)
                .map(|t| schema_column(t, &rel.from_column))
                .unwrap_or_else(|| normalize_ident(&rel.from_column));
            out.push_str(
                &format!(
                    r##"    {{{{
      "fact_table": "default",
      "fact_column": "{fc}",
      "dimension_id": "{did}",
      "dim_table": "{dt}",
      "dim_column": "{dc}"
    }}}}"##,
                    fc = fact_col,
                    did = dim_id,
                    dt = normalize_ident(&rel.to_table),
                    dc = schema_column(t, &rel.to_column),
                )
                .replace("{{", "{")
                .replace("}}", "}"),
            );
            emitted += 1;
            if emitted < total {
                out.push_str(",\n");
            }
        }
    }
    out
}

// ---- date roles ----

/// Canonical time-intelligence flag columns. Flags are upstream columns
/// (plan 044, invariant 2) — the converter only emits the ones that exist.
const FLAG_COLUMNS: [(&str, &str); 5] = [
    ("ytd_flag_column", "ytd_flag"),
    ("prior_year_ytd_flag_column", "prior_year_ytd_flag"),
    ("current_year_flag_column", "current_year_flag"),
    ("qtd_flag_column", "qtd_flag"),
    ("mtd_flag_column", "mtd_flag"),
];

/// A date-role table with every time-intelligence column resolved against the
/// generated schema. Nothing is guessed: a field is `None` when the model has
/// no such column, and the conversion report says so.
struct DateRole {
    /// Config dimension id (`ssas_name`).
    dim_id: String,
    /// Model table name, as referenced by DAX (`Calendar_DeliveryDate`).
    source_name: String,
    /// Physical table name in `schema.sql`.
    table_name: String,
    /// Date key: the relationship's dimension column.
    date_key: Option<String>,
    /// Full date: the primary hierarchy's leaf level.
    full_date: Option<String>,
    year: Option<String>,
    quarter: Option<String>,
    month: Option<String>,
    /// Flag columns present on this table: `(config key, column)`.
    flags: Vec<(&'static str, String)>,
}

impl DateRole {
    fn flag(&self, config_key: &str) -> Option<&str> {
        self.flags
            .iter()
            .find(|(k, _)| *k == config_key)
            .map(|(_, c)| c.as_str())
    }
}

/// Resolve a date-part column: an exact source column (`year`, `år`), then a
/// numeric sibling (`quarternumber`), then any matching display name.
fn date_part_column(t: &TableInfo, words: &[&str]) -> Option<String> {
    for w in words {
        if let Some(c) = t
            .columns
            .iter()
            .find(|c| normalize_ident(&c.source_column) == *w)
        {
            return Some(normalize_ident(&c.source_column));
        }
    }
    for w in words {
        for c in &t.columns {
            let n = c.name.to_lowercase();
            if n.contains(w)
                && (n.ends_with("number") || n.ends_with("nummer") || n.ends_with("nr"))
            {
                return Some(normalize_ident(&c.source_column));
            }
        }
    }
    for w in words {
        if let Some(c) = resolve_column(t, w) {
            return Some(normalize_ident(&c.source_column));
        }
    }
    None
}

/// Build the resolved date-role description for one date-role table.
fn build_date_role(m: &ConversionModel, t: &TableInfo) -> DateRole {
    let date_key = m
        .relationships
        .iter()
        .find(|r| r.to_table == t.name || r.to_table == t.ssas_name)
        .and_then(|r| resolve_column(t, &r.to_column))
        .map(|c| normalize_ident(&c.source_column));

    // The primary hierarchy carries the date levels; its leaf is the full date.
    let (mut hier_year, mut hier_quarter, mut hier_month) = (None, None, None);
    let mut full_date = None;
    if let Some(h) = t.hierarchies.iter().find(|h| !h.levels.is_empty()) {
        for level in &h.levels {
            let Some(col) = level_schema_column(t, level) else {
                continue;
            };
            let n = level.name.to_lowercase();
            if hier_year.is_none() && (n.contains("year") || n.contains("år")) {
                hier_year = Some(col.clone());
            } else if hier_quarter.is_none() && (n.contains("quarter")) {
                hier_quarter = Some(col.clone());
            } else if hier_month.is_none() && (n.contains("month")) {
                hier_month = Some(col.clone());
            }
            full_date = Some(col);
        }
    }
    let full_date = full_date.or_else(|| {
        ["full_date", "fulldate", "date"]
            .iter()
            .find_map(|w| resolve_column(t, w).map(|c| normalize_ident(&c.source_column)))
    });

    let flags = FLAG_COLUMNS
        .iter()
        .filter_map(|(key, name)| {
            resolve_column(t, name).map(|c| (*key, normalize_ident(&c.source_column)))
        })
        .collect();

    DateRole {
        dim_id: t.ssas_name.clone(),
        source_name: t.name.clone(),
        table_name: normalize_ident(&t.name),
        date_key,
        full_date,
        year: date_part_column(t, &["year"]).or(hier_year),
        quarter: date_part_column(t, &["quarter"]).or(hier_quarter),
        month: date_part_column(t, &["month"]).or(hier_month),
        flags,
    }
}

fn date_roles(m: &ConversionModel) -> Vec<DateRole> {
    m.date_roles.iter().map(|t| build_date_role(m, t)).collect()
}

/// Downgrade time-intelligence measures whose date role has no flag column to
/// bridge code: they get a stub plus an upstream checklist entry instead of a
/// measure that references a column which does not exist.
fn downgrade_time_intelligence_without_flags(model: &mut ConversionModel) {
    let roles = date_roles(model);
    for meas in model.fact_table.measures.iter_mut() {
        let flag_key = match meas.classification.as_str() {
            "time_ytd" => "ytd_flag_column",
            "time_prior_year" => "prior_year_ytd_flag_column",
            "time_qtd" => "qtd_flag_column",
            "time_mtd" => "mtd_flag_column",
            _ => continue,
        };
        let (role, _) = time_role_for_measure(&roles, &meas.expression);
        if role.and_then(|r| r.flag(flag_key)).is_none() {
            meas.classification = "sql_fallback".to_string();
        }
    }
}

/// Quoted table references in a DAX expression (`'Calendar_X'[Date]`).
fn dax_table_refs(dax: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let mut rest = dax;
    while let Some(start) = rest.find('\'') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('\'') else {
            break;
        };
        refs.push(after[..end].to_string());
        rest = &after[end + 1..];
    }
    refs
}

/// Which date role a time-intelligence measure uses. Inferred from the tables
/// the DAX references; falls back to the first role (the report marks that as
/// assumed).
fn time_role_for_measure<'a>(roles: &'a [DateRole], dax: &str) -> (Option<&'a DateRole>, bool) {
    let refs = dax_table_refs(dax);
    for r in roles {
        if refs.iter().any(|q| {
            q.eq_ignore_ascii_case(&r.source_name)
                || q.eq_ignore_ascii_case(&r.dim_id)
                || normalize_ident(q) == r.table_name
        }) {
            return (Some(r), true);
        }
    }
    (roles.first(), false)
}

fn render_time_intelligence_block(m: &ConversionModel) -> String {
    let roles = date_roles(m);
    let Some(first) = roles.iter().find(|r| r.date_key.is_some()) else {
        return String::new();
    };
    let mut parts: Vec<String> = Vec::new();
    for (key, col) in [
        ("year_column", &first.year),
        ("quarter_column", &first.quarter),
        ("month_column", &first.month),
    ] {
        if let Some(col) = col {
            parts.push(format!("\"{key}\": \"{}\"", col.replace('"', "\\\"")));
        }
    }
    parts.extend(
        first
            .flags
            .iter()
            .map(|(key, col)| format!("\"{key}\": \"{}\"", col.replace('"', "\\\""))),
    );
    let full_date = first
        .full_date
        .as_deref()
        .or(first.date_key.as_deref())
        .unwrap_or("");
    format!(
        "\n  \"time_intelligence\": {{{{\n    \"date_dimension\": {{{{\n      \"dimension_id\": \"{did}\",\n      \"table_name\": \"{tn}\",\n      \"date_key_column\": \"{key}\",\n      \"full_date_column\": \"{full}\",\n      \"flag_columns\": {{{flags}}}\n    }}}}\n  }},\n",
        did = first.dim_id.replace('"', "\\\""),
        tn = first.table_name,
        key = first.date_key.as_deref().unwrap_or(""),
        full = full_date,
        flags = parts.join(", "),
    )
    .replace("{{", "{")
    .replace("}}", "}")
}

/// Resolve a model column reference (e.g. `Customer ID`, `Month Name`) to the
/// table's column list. The export names columns (`Customer ID`) while
/// `schema.sql` is built from source columns (`customerid`), so every reference
/// has to go through this lookup.
fn resolve_column<'a>(table: &'a TableInfo, column: &str) -> Option<&'a ColumnInfo> {
    let want = column.trim();
    let want_lower = want.to_lowercase();
    table
        .columns
        .iter()
        .find(|c| c.name == want)
        .or_else(|| table.columns.iter().find(|c| c.source_column == want))
        .or_else(|| {
            table.columns.iter().find(|c| {
                c.name.to_lowercase() == want_lower || c.source_column.to_lowercase() == want_lower
            })
        })
}

/// The column `schema.sql` creates for a model column reference. Falls back to
/// normalizing the reference itself when the column list doesn't have it.
fn schema_column(table: &TableInfo, column: &str) -> String {
    resolve_column(table, column)
        .map(|c| normalize_ident(&c.source_column))
        .unwrap_or_else(|| normalize_ident(column))
}

/// Map a hierarchy level to the column `schema.sql` creates.
fn level_schema_column(t: &TableInfo, level: &HierarchyLevelInfo) -> Option<String> {
    resolve_column(t, &level.column).map(|c| normalize_ident(&c.source_column))
}

/// Does this level's column exist in the table (as `schema.sql` names it)?
fn level_column_resolves(t: &TableInfo, level: &HierarchyLevelInfo) -> bool {
    level_schema_column(t, level).is_some()
}

/// The runtime config models one hierarchy per dimension: pick the table's
/// first hierarchy that has levels resolvable against the generated schema.
/// Returns the hierarchy and its `hierarchy_levels` JSON. Extra hierarchies are
/// surfaced in the conversion report.
fn emit_hierarchy(t: &TableInfo) -> Option<(&HierarchyInfo, String)> {
    let h = t.hierarchies.iter().find(|h| !h.levels.is_empty())?;
    let levels: Vec<String> = h
        .levels
        .iter()
        .filter_map(|l| {
            let column = level_schema_column(t, l)?;
            Some(format!(
                "{{\"name\": \"{}\", \"column\": \"{}\", \"level_number\": {}}}",
                l.name.replace('"', "\\\""),
                column,
                l.ordinal
            ))
        })
        .collect();
    if levels.is_empty() {
        return None;
    }
    Some((h, levels.join(", ")))
}

/// One row per declared hierarchy for the conversion report:
/// (table, hierarchy, levels, status).
fn hierarchy_report_rows(m: &ConversionModel) -> Vec<(String, String, String, String)> {
    let all_dims: Vec<&TableInfo> = m
        .dimensions
        .iter()
        .chain(&m.date_roles)
        .chain(&m.lookup_tables)
        .collect();
    let mut rows = Vec::new();
    for t in all_dims {
        if t.hierarchies.is_empty() {
            continue;
        }
        let emitted = emit_hierarchy(t);
        for h in &t.hierarchies {
            let levels = h
                .levels
                .iter()
                .map(|l| l.name.as_str())
                .collect::<Vec<_>>()
                .join(" → ");
            let status = if h.levels.is_empty() {
                "not emitted — no levels in the export".to_string()
            } else if emitted.as_ref().is_some_and(|(e, _)| e.name == h.name) {
                let resolved = h
                    .levels
                    .iter()
                    .filter(|l| level_column_resolves(t, l))
                    .count();
                if resolved == h.levels.len() {
                    "emitted".to_string()
                } else {
                    format!(
                        "emitted with {resolved} of {} levels (columns missing from schema.sql)",
                        h.levels.len()
                    )
                }
            } else if emitted.is_none() {
                "not emitted — no level column matches schema.sql".to_string()
            } else {
                let name = &emitted
                    .as_ref()
                    .map(|(e, _)| e.name.clone())
                    .unwrap_or_default();
                format!("not emitted — this dimension already carries `{name}`")
            };
            rows.push((t.ssas_name.clone(), h.name.clone(), levels, status));
        }
    }
    rows
}

fn render_dimension_configs(m: &ConversionModel) -> String {
    let mut out = String::new();
    let all_dims: Vec<&TableInfo> = m
        .dimensions
        .iter()
        .chain(&m.date_roles) // index-based since both are &TableInfo
        .chain(&m.lookup_tables)
        .collect();
    for (i, t) in all_dims.iter().enumerate() {
        let dim_name = t.ssas_name.clone();
        // Pick a representative column for display
        // Use first non-hidden column, or first visible name column
        let rep_col = t
            .columns
            .iter()
            .find(|c| {
                !c.is_hidden
                    && (c.source_column.contains("Namn")
                        || c.source_column.contains("Kod")
                        || c.source_column == dim_name)
            })
            .or_else(|| t.columns.iter().find(|c| !c.is_hidden))
            .or_else(|| t.columns.first());
        let physical = rep_col
            .map(|c| normalize_ident(&c.source_column))
            .unwrap_or_else(|| normalize_ident(&dim_name));
        let ft_ref = if m.date_roles.iter().any(|d| d.name == t.name) {
            "default".to_string()
        } else {
            String::new()
        };

        let ft_line = if ft_ref.is_empty() {
            String::new()
        } else {
            format!("\n      \"fact_table\": \"{}\",", ft_ref)
        };
        let _shared = if m.date_roles.iter().any(|d| d.name == t.name)
            || m.dimensions.iter().any(|d| d.name == t.name)
        {
            ""
        } else {
            ",\n      \"shared\": true"
        };
        let is_date_role = m.date_roles.iter().any(|d| d.name == t.name);
        let date_role_line = if is_date_role {
            ",\n      \"is_date_role\": true"
        } else {
            ""
        };

        // Hierarchy levels: the runtime models one hierarchy per dimension, so
        // emit the first resolvable one and use its name for unique names
        // (`[Dates].[Calendar Hierarchy].[year]`).
        let hierarchy = emit_hierarchy(t);
        let hierarchy_name = hierarchy
            .as_ref()
            .map(|(h, _)| h.name.replace('"', "\\\""))
            .unwrap_or_else(|| t.ssas_name.clone());
        let levels_line = hierarchy
            .map(|(_, levels)| format!(",\n      \"hierarchy_levels\": [{levels}]"))
            .unwrap_or_default();

        let pf = format!("{}.{}", normalize_ident(&t.name), physical);
        out.push_str(
            &format!(
                r##"    {{{{
      "id": "{id}",
      "physical_field": "{pf}",
      "caption": "{caption}",
      "description": "{desc}",
      "hierarchy_name": "{hierarchy_name}",
      "all_level_name": "(All)",
      "leaf_level_name": "{caption}",
      "ordinal": {ord},{ft_line}
      "visible": true,
      "has_all": true,
          "cardinality_hint": 100{shared}{date_role_line}{levels_line}
    }}}}"##,
                id = t.ssas_name,
                pf = pf,
                caption = t.ssas_name,
                hierarchy_name = hierarchy_name,
                levels_line = levels_line,
                desc = t.description.replace('\"', "\\\""),
                ord = i + 1,
                ft_line = ft_line,
                date_role_line = date_role_line,
                shared = if m
                    .date_roles
                    .iter()
                    .chain(&m.dimensions)
                    .any(|d| d.name == t.name)
                {
                    ""
                } else {
                    ",\n      \"shared\": true"
                },
            )
            .replace("{{", "{")
            .replace("}}", "}"),
        );
        if i < all_dims.len() - 1 {
            out.push_str(",\n");
        }
    }
    out
}

fn render_measure_configs(m: &ConversionModel) -> String {
    let mut out = String::new();
    let roles = date_roles(m);
    let all_measures: Vec<&MeasureInfo> = m
        .fact_table
        .measures
        .iter()
        .chain(m.dimensions.iter().flat_map(|t| &t.measures))
        .chain(m.date_roles.iter().flat_map(|t| &t.measures))
        .chain(m.lookup_tables.iter().flat_map(|t| &t.measures))
        .collect();
    for (i, meas) in all_measures.iter().enumerate() {
        let dax_expr = meas.expression.as_str();
        // For time measures, extract the inner aggregation for the sql_expr.
        // The flag column comes from the measure's own date role; a role
        // without the flag was downgraded to bridge code before rendering.
        let (time_class, flag_key) = match meas.classification.as_str() {
            "time_ytd" => ("time_ytd", "ytd_flag_column"),
            "time_prior_year" => ("time_prior_year", "prior_year_ytd_flag_column"),
            "time_qtd" => ("time_qtd", "qtd_flag_column"),
            "time_mtd" => ("time_mtd", "mtd_flag_column"),
            _ => ("", ""),
        };
        let time_role = if time_class.is_empty() {
            None
        } else {
            time_role_for_measure(&roles, dax_expr).0
        };
        let time_flag = time_role.and_then(|r| r.flag(flag_key));
        let sql = if !time_class.is_empty() {
            let inner = extract_ti_inner(dax_expr);
            simple_aggregate_sql(&m.fact_table, &inner)
                .or_else(|| expr_to_sql(&dax_to_expr(&inner)))
                .unwrap_or_else(|| "null".to_string())
        } else {
            dax_to_sql_hint(dax_expr, &meas.classification, &m.fact_table)
                .unwrap_or_else(|| "null".to_string())
        };
        // When the converter cannot produce real SQL for a "simple" measure,
        // downgrade it to sql_fallback so the runtime never executes a placeholder.
        let effective_class = if meas.classification == "simple"
            && dax_to_sql_hint(dax_expr, &meas.classification, &m.fact_table).is_none()
        {
            "sql_fallback"
        } else {
            meas.classification.as_str()
        };
        let fb_line = if effective_class == "sql_fallback" {
            format!(
                ",\n      \"sql_fallback_file\": \"sql_fallback/{}.sql\"",
                normalize_ident(&meas.name)
            )
        } else if let (Some(role), Some(flag)) = (time_role, time_flag) {
            format!(
                ",\n      \"time_intelligence\": {{ \"dimension_id\": \"{did}\", \"flag_column\": \"{fc}\" }}",
                did = role.dim_id.replace('"', "\\\""),
                fc = flag
            )
        } else {
            String::new()
        };
        out.push_str(
            &format!(
                r##"    {{{{
      "id": "{id}",
      "fact_table": "default",
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
                sql = sql,
                caption = meas.name,
                dn = meas.name,
                desc = json_escape(&format!(
                    "[{}] {}",
                    match meas.classification.as_str() {
                        "sql_fallback" => "bridge",
                        other => other,
                    },
                    dax_expr
                )),
                ord = i + 1,
                cube = m.cube,
                fb = fb_line,
            )
            .replace("{{", "{")
            .replace("}}", "}"),
        );
        if i < all_measures.len() - 1 {
            out.push_str(",\n");
        }
    }
    out
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Mechanical plain aggregates (`SUM('T'[col])`, `COUNT(...)`,
/// `DISTINCTCOUNT(...)`, `AVERAGE(...)`, `MIN/MAX`) lowered to SQL. This is the
/// boundary's "plain SQL" allowance (docs/DESIGN-INVARIANTS.md): the simplest
/// measures must return numbers, not stubs. Columns resolve through the fact
/// table's schema mapping (`Customer ID` → `customerid`).
fn simple_aggregate_sql(fact: &TableInfo, dax: &str) -> Option<String> {
    let dax = normalize_dax(dax);
    let upper = dax.to_uppercase();
    // `DISTINCTCOUNT(` before `COUNT(` (prefix check).
    for func in [
        "DISTINCTCOUNT(",
        "COUNT(",
        "SUM(",
        "AVERAGE(",
        "MIN(",
        "MAX(",
    ] {
        if let Some(inner) = extract_dax_unary(&upper, func)
            && let Some(col) = extract_col(&inner)
        {
            let col = resolve_column(fact, &col)
                .map(|c| normalize_ident(&c.source_column))
                .unwrap_or_else(|| normalize_ident(&col));
            return Some(match func {
                "DISTINCTCOUNT(" => format!("COUNT(DISTINCT {col})"),
                "AVERAGE(" => format!("AVG({col})"),
                other => format!("{}({col})", other.trim_end_matches('(')),
            });
        }
    }
    None
}

fn dax_to_sql_hint(expr: &str, class: &str, fact: &TableInfo) -> Option<String> {
    match class {
        "simple" => simple_aggregate_sql(fact, expr).or_else(|| expr_to_sql(&dax_to_expr(expr))),
        _ => Some("null".to_string()),
    }
}

fn expr_to_sql(expr: &str) -> Option<String> {
    // Only return SQL for patterns the converter can truly lower.
    // Numeric constants are the only safe case; all aggregate/expression
    // patterns must be explicitly handwritten as fallback SQL.
    if let Ok(v) = expr.trim().parse::<f64>() {
        return Some(format!("{}", v));
    }
    None
}

fn dax_to_expr(dax: &str) -> String {
    let dax = normalize_dax(dax);
    let dax = dax.as_str();
    let upper = dax.to_uppercase();

    // Constant value (e.g. "0.8")
    if dax.trim().parse::<f64>().is_ok() {
        return dax.trim().to_string();
    }

    // DISTINCTCOUNT('table'[col]) → col.count(distinct true)
    if let Some(inner) = extract_dax_unary(&upper, "DISTINCTCOUNT(")
        && let Some(col) = extract_col(&inner)
    {
        return format!("{}.count(distinct true)", normalize_ident(&col));
    }

    // COUNT('table'[col]) → col.count()
    if let Some(inner) = extract_dax_unary(&upper, "COUNT(")
        && let Some(col) = extract_col(&inner)
    {
        return format!("{}.count()", normalize_ident(&col));
    }

    // AVERAGE('table'[col]) → col.avg()
    if let Some(inner) = extract_dax_unary(&upper, "AVERAGE(")
        && let Some(col) = extract_col(&inner)
    {
        return format!("{}.avg()", normalize_ident(&col));
    }

    // SUM('table'[col]) → col.sum()
    if let Some(inner) = extract_dax_unary(&upper, "SUM(")
        && let Some(col) = extract_col(&inner)
    {
        return format!("{}.sum()", normalize_ident(&col));
    }

    // DIVIDE(a, b) → a / b
    if let Some(inner) = upper.strip_prefix("DIVIDE(") {
        // Use the original (non-upper) string for splitting to preserve
        let orig_inner = &dax["DIVIDE(".len()..];
        let upper_parts = split_args(inner);
        let orig_parts = split_args(orig_inner);
        if upper_parts.len() >= 2 {
            let a = dax_to_expr(&orig_parts[0]);
            let b = dax_to_expr(&orig_parts[1]);
            return format!("{a} / {b}");
        }
    }

    // CALCULATE([measure], 'dim'[col]="value") → measure { where: col = 'value' }
    if let Some(inner) = upper.strip_prefix("CALCULATE(") {
        let parts = split_args(inner);
        if parts.len() >= 2 {
            let base = dax_to_expr(&parts[0]);
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
            return dax_to_expr(&parts[0]);
        }
    }

    // Reference to another measure: [Measure Name]
    if upper.starts_with('[') && upper.ends_with(']') {
        let name = &upper[1..upper.len() - 1];
        return normalize_ident(name);
    }

    // Compound expression like a / b
    if upper.contains("/") && !upper.contains('(') {
        let parts: Vec<&str> = dax.split('/').collect();
        if parts.len() == 2 {
            let a = dax_to_expr(parts[0].trim());
            let b = dax_to_expr(parts[1].trim());
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
    // Exporters keep the leading `=`; strip it so the function name matches.
    let dax = dax.trim().trim_start_matches('=').trim();
    let upper = dax.to_uppercase();
    let mut rest = None;
    for func in [
        "TOTALYTD(",
        "TOTALQTD(",
        "TOTALMTD(",
        "DATESYTD(",
        "DATESQTD(",
        "DATESMTD(",
        "SAMEPERIODLASTYEAR(",
    ] {
        if upper.starts_with(func) {
            rest = Some(&dax[func.len()..]);
            break;
        }
    }
    let Some(dax) = rest else {
        return dax.to_string();
    };
    // Find the closing paren matching the opening func, then extract inner.
    let mut depth = 1;
    let mut comma_pos = None;
    for (i, c) in dax.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            ',' if depth == 1 => {
                comma_pos = Some(i);
                break;
            }
            _ => {}
        }
    }
    comma_pos
        .map(|pos| dax[..pos].trim().to_string())
        .unwrap_or_else(|| dax.to_string())
}

fn extract_dax_unary(dax: &str, func: &str) -> Option<String> {
    if !dax.starts_with(func) {
        return None;
    }
    let inner = &dax[func.len()..];
    let inner = inner.trim_end_matches(')').trim();
    Some(inner.to_string())
}

fn extract_col(dax: &str) -> Option<String> {
    let trimmed = dax.trim().trim_matches('\'');
    if let Some(bracket) = trimmed.find('[') {
        let after = &trimmed[bracket + 1..];
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
    for (byte_idx, c) in inner.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                if depth > 0 {
                    depth -= 1
                } else {
                    parts.push(inner[start..byte_idx].trim().to_string());
                    return parts;
                }
            }
            ',' if depth == 0 => {
                parts.push(inner[start..byte_idx].trim().to_string());
                start = byte_idx + 1;
            }
            _ => {}
        }
    }
    let last = inner[start..].trim().to_string();
    if !last.is_empty() {
        parts.push(last);
    }
    parts
}

fn extract_calculate_filter(dax: &str) -> Option<String> {
    let trimmed = dax.trim_matches('\'').trim();
    // Try comparison operators: first standalone =, then <=, >=, <, >
    if let Some((op, pos)) = find_comparison_op(trimmed) {
        let col = trimmed[..pos].trim();
        let col = extract_col(col)?;
        let val = trimmed[pos + op.len()..]
            .trim()
            .trim_matches('"')
            .trim_matches('\'');
        let val = val.trim_end_matches(')');
        return Some(format!("{} {op} '{val}'", normalize_ident(&col)));
    }
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return Some(normalize_ident(&trimmed[1..trimmed.len() - 1]));
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
    if let Some(pos) = s.find('=')
        && (pos == 0
            || (bytes[pos - 1] != b'<' && bytes[pos - 1] != b'>' && bytes[pos - 1] != b'!'))
    {
        return Some(("=", pos));
    }
    // Regular < and >
    if let Some(pos) = s.find('>')
        && (pos + 1 >= bytes.len() || bytes[pos + 1] != b'=')
    {
        return Some((">", pos));
    }
    if let Some(pos) = s.find('<')
        && (pos + 1 >= bytes.len() || (bytes[pos + 1] != b'=' && bytes[pos + 1] != b'>'))
    {
        return Some(("<", pos));
    }
    None
}

fn render_fallback_stub(name: &str, dax: &str) -> String {
    let upper = dax.to_uppercase();
    let mut notes = Vec::new();
    if upper.contains("ALLSELECTED") {
        notes.push("ALLSELECTED — requires window function");
    }
    if upper.contains("ISONORAFTER") {
        notes.push("ISONORAFTER — cumulative window ordering");
    }
    if upper.contains("FILTER(") {
        notes.push("FILTER — context manipulation");
    }
    if upper.contains("YEAR(TODAY())") {
        notes.push("YEAR(TODAY()) — dynamic current year filter");
    }
    if upper.contains("YEAR(TODAY())-1") {
        notes.push("Previous year comparison");
    }
    if upper.contains("MEDIAN(") {
        notes.push("MEDIAN — DuckDB supports MEDIAN() natively");
    }
    if upper.contains("AVERAGEX(") {
        notes.push("AVERAGEX — row-level iteration");
    }
    if upper.contains("KEEPFILTERS") {
        notes.push("KEEPFILTERS — filter context preservation");
    }
    let note_str = if notes.is_empty() {
        "Complex DAX pattern — requires SQL fallback".to_string()
    } else {
        notes.join("\n--   ")
    };
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
        name = name,
        dax = dax,
        notes = note_str,
    )
}

// ---- SQL fallback generation ----

/// Bridge-code banner prepended to every generated fallback file (plan 044).
const BRIDGE_HEADER: &str = "\
-- BRIDGE CODE (plan 044) — move this definition upstream.
-- Fallback SQL exists so Excel keeps working during a migration; it is not
-- where metric logic should live. Prefer an additive column or a mart in your
-- transformation layer and delete this file. See docs/DESIGN-INVARIANTS.md.
-- `mallard qualify --strict` fails while bridge code remains.";

/// What to build upstream instead of this DAX (plan 044). A heuristic, but
/// enough to turn the conversion report into a migration checklist.
fn upstream_suggestion(dax: &str) -> &'static str {
    // DAX is often written with spaces before the parenthesis
    // (`CALCULATE ( SUM ( ... ) )`), so strip whitespace before matching.
    let upper: String = dax
        .to_uppercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if upper.contains("MEDIAN(") || upper.contains("PERCENTILE") {
        "median/percentile mart at the reporting grain"
    } else if upper.contains("DISTINCTCOUNT(") {
        "grain change (one row per counted entity) or an entity-grain fact"
    } else if upper.contains("TOTALYTD(")
        || upper.contains("DATESYTD(")
        || upper.contains("TOTALQTD(")
        || upper.contains("DATESQTD(")
        || upper.contains("TOTALMTD(")
        || upper.contains("DATESMTD(")
        || upper.contains("SAMEPERIODLASTYEAR(")
        || upper.contains("YEAR(TODAY())")
    {
        "date flag columns on the calendar (`ytd_flag`, `prior_year_ytd_flag`) or a cumulative snapshot mart"
    } else if upper.contains("ALLSELECTED(") || upper.contains("ISONORAFTER(") {
        "cumulative snapshot mart at the calendar grain"
    } else if upper.contains("SUMX(") && upper.contains("RELATED(") {
        "additive column for the row-level multiplication"
    } else if upper.contains("DIVIDE(") {
        "numerator and denominator as additive columns"
    } else if upper.contains("CALCULATE(") {
        "status/bucket flag column for the CALCULATE filter"
    } else {
        "a SQL model in the transformation layer"
    }
}

fn generate_fallback_sql(meas: &MeasureInfo, model: &ConversionModel) -> String {
    let body = generate_fallback_sql_recursive(meas, model, &mut Vec::new());
    format!("{BRIDGE_HEADER}\n\n{body}")
}

/// DAX lowering — **FROZEN** (plan 044).
///
/// Policy: mechanical patterns only; no new DAX coverage is added here.
/// Complex measures are not lowered — they fall through to the stub and become
/// an entry in the conversion report's "define upstream" checklist, so the
/// definition moves to the transformation layer instead of growing a DAX engine
/// inside the proxy (`docs/DESIGN-INVARIANTS.md`). Everything this function
/// emits is bridge code, labelled as such in the file header and the report.
fn generate_fallback_sql_recursive(
    meas: &MeasureInfo,
    model: &ConversionModel,
    visited: &mut [String],
) -> String {
    let dax_raw = meas.expression.clone();
    let dax = dax_raw
        .trim_start()
        .trim_start_matches("=")
        .trim()
        .to_string();
    let dax_one_line = normalize_dax(&dax);
    let upper_one = dax_one_line.to_uppercase();

    // Pattern 1: MEDIAN(col) — DuckDB native
    if upper_one.contains("MEDIAN(")
        && let Some(col_expr) = extract_dax_unary(&upper_one, "MEDIAN(")
        && let Some(col) = extract_col(&col_expr)
    {
        let fact_table = normalize_ident(&model.fact_table.name);
        return format!(
            "-- Auto-generated from DAX: {dax}\n-- DuckDB supports MEDIAN() natively.\n\nSELECT MEDIAN({col}) FROM {fact_table};\n",
            dax = dax,
            col = normalize_ident(&col),
            fact_table = fact_table,
        );
    }

    // Pattern 2: Cumulative YTD (FILTER + ALLSELECTED + ISONORAFTER)
    if upper_one.contains("ALLSELECTED") && upper_one.contains("ISONORAFTER") {
        return generate_cumulative_sql(&dax_one_line, &upper_one, meas, model);
    }

    // Pattern 3: SUMX(FILTER(table, col=val), qty_col * RELATED(dim.dimcol))
    if upper_one.contains("SUMX(")
        && upper_one.contains("FILTER(")
        && upper_one.contains("RELATED(")
        && let Some(sql) = generate_sumx_filter_related(&dax_one_line, &upper_one, model)
    {
        return sql;
    }

    // Pattern 4: CALCULATE(SUM(col), filter) — simple filtered SUM
    if upper_one.contains("CALCULATE(")
        && upper_one.contains("SUM(")
        && let Some(sql) = generate_calculate_sum(&dax_one_line, &upper_one, model)
    {
        return sql;
    }

    // Pattern 5: [MeasureA] - [MeasureB] — arithmetic between two measures
    if dax_one_line.trim().starts_with("[")
        && (dax_one_line.contains("- [") || dax_one_line.contains("-["))
        && let Some(sql) = generate_measure_arithmetic(&dax_one_line, model, visited)
    {
        return sql;
    }

    // Pattern 6: DIVIDE([MeasureA], [MeasureB], ...) — safe division
    if upper_one.starts_with("DIVIDE(")
        && dax_one_line.contains('[')
        && let Some(sql) = generate_divide_measure_recursive(&dax_one_line, model, visited)
    {
        return sql;
    }

    // Unsupported: keep annotated stub
    render_fallback_stub(&meas.name, &dax_one_line)
}

fn generate_cumulative_sql(
    dax: &str,
    upper: &str,
    meas: &MeasureInfo,
    model: &ConversionModel,
) -> String {
    let cal_table = extract_calendar_table(dax);
    let period_col = extract_period_column(dax);
    let year_col = extract_year_column(dax);
    let is_prior_year = upper.contains("YEAR(TODAY())-1") || upper.contains("YEAR(TODAY()) - 1");
    let base_meas = extract_base_measure(dax);

    let cal_name = cal_table
        .as_deref()
        .map(normalize_ident)
        .unwrap_or_else(|| "calendar".into());
    let period_col_name = period_col
        .as_deref()
        .map(normalize_ident)
        .unwrap_or_else(|| "period".into());
    let year_col_name = year_col
        .as_deref()
        .map(normalize_ident)
        .unwrap_or_else(|| "year".into());

    let join_col = cal_table
        .as_ref()
        .and_then(|ct| {
            model
                .relationships
                .iter()
                .find(|r| r.to_table == *ct)
                .map(|r| r.from_column.clone())
        })
        .unwrap_or_default();
    let join_col_name = normalize_ident(&join_col);
    let fact_table = normalize_ident(&model.fact_table.name);
    let year_expr = if is_prior_year {
        "EXTRACT(YEAR FROM CURRENT_DATE) - 1"
    } else {
        "EXTRACT(YEAR FROM CURRENT_DATE)"
    };

    format!(
        r#"-- Cumulative YTD for: {name}
-- Original DAX: {dax}
-- Calendar: {cal_name}, Period: {period}, Year: {year}
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
    COUNT(DISTINCT f.order_id) AS base_count
  FROM {fact_table} f
  JOIN {cal_name} c ON f.{join_col} = c.{join_col}
  WHERE c.{year} = {year_expr}
  GROUP BY c.{period}, c.{year}
);
"#,
        name = meas.name,
        dax = dax,
        cal_name = cal_name,
        period = period_col_name,
        year = year_col_name,
        base_meas = base_meas,
        join_col = join_col_name,
        fact_table = fact_table,
        year_expr = year_expr,
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
            if after_table.starts_with('[')
                && let Some(close) = after_table.find(']')
            {
                return Some(after_table[1..close].to_string());
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
        if trimmed.starts_with('[')
            && let Some(close) = trimmed.find(']')
        {
            return trimmed[1..close].to_string();
        }
    }
    String::new()
}

fn generate_sumx_filter_related(
    dax: &str,
    _upper: &str,
    model: &ConversionModel,
) -> Option<String> {
    let fact = normalize_ident(&model.fact_table.name);
    let filter_parts = extract_filter_eq(dax)?;
    let filter_col_raw = &filter_parts.0;
    let filter_col = resolve_source_column(filter_col_raw, model);
    let filter_val = &filter_parts.1;
    let qty_col_raw = extract_first_mul_col(dax)?;
    let qty_col = resolve_source_column(&qty_col_raw, model);
    let related = extract_related_ref(dax)?;
    let dim_table = normalize_ident(&related.0);
    let dim_col_raw = &related.1;
    let dim_col = resolve_source_column(dim_col_raw, model);
    let join_col = model
        .relationships
        .iter()
        .find(|r| normalize_ident(&r.to_table) == dim_table)
        .map(|r| {
            // Resolve SSAS column ref to actual sourceColumn
            let raw = r.from_column.clone();
            resolve_source_column(&raw, model)
        })
        .unwrap_or_else(|| "id".into());

    let sql = format!(
        "-- Auto-generated from DAX: {dax}\n\
         -- SUMX(FILTER(...), qty * RELATED(dim.col))\n\n\
         SELECT COALESCE(SUM(f.{qty_col} * CAST(d.{dim_col} AS DOUBLE)), 0) AS value\n\
         FROM {fact} f\n\
         JOIN {dim_table} d ON f.{join_col} = d.{join_col}\n\
         WHERE f.{filter_col} = {filter_val};\n",
        dax = dax,
        qty_col = qty_col,
        dim_col = dim_col,
        fact = fact,
        dim_table = dim_table,
        join_col = join_col,
        filter_col = filter_col,
        filter_val = filter_val,
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
    let val_end = val.find([' ', ',', ')']).unwrap_or(val.len());
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
            ')' => {
                depth -= 1;
                if depth == 0 {
                    filter_end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    let after_filter_end = &inner[filter_end + 1..]
        .trim()
        .trim_start_matches(',')
        .trim();
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
    normalize_ident(ssas_name)
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

fn generate_calculate_sum(dax: &str, _upper: &str, model: &ConversionModel) -> Option<String> {
    let fact = normalize_ident(&model.fact_table.name);
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
        dax = dax,
        sum_col_name = sum_col_name,
        fact = fact,
        filter_col = filter_col,
        filter_val = filter_val,
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
            ')' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            ',' if depth == 1 => {
                comma_pos = Some(i);
                break;
            }
            _ => {}
        }
    }
    let rest = &inner[comma_pos? + 1..].trim();
    // Now parse 'Table'[Col] = val
    let (_table, col) = parse_dax_col_ref(rest)?;
    let after_col = rest[rest.find(']')? + 1..].trim();
    let eq_pos = after_col.find('=')?;
    let val = after_col[eq_pos + 1..].trim();
    let val_end = val.find([' ', ',', ')']).unwrap_or(val.len());
    Some((col, val[..val_end].to_string()))
}

fn generate_measure_arithmetic(
    dax: &str,
    model: &ConversionModel,
    visited: &mut [String],
) -> Option<String> {
    let parts: Vec<&str> = dax
        .split(['-', '+', '*', '/'])
        .filter(|p| !p.trim().is_empty())
        .collect();
    if parts.len() < 2 {
        return None;
    }
    let op = if dax.contains(" - [") || dax.contains("- [") {
        " - "
    } else if dax.contains(" + [") || dax.contains("+ [") {
        " + "
    } else if dax.contains(" * [") || dax.contains("* [") {
        " * "
    } else if dax.contains(" / [") || dax.contains("/ [") {
        " / "
    } else {
        return None;
    };

    let measure_names: Vec<String> = parts
        .iter()
        .map(|p| {
            p.trim()
                .trim_matches(|c: char| c == '[' || c == ']' || c == ' ')
                .to_string()
        })
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
        subq_a = subqueries.first()?,
        subq_b = subqueries.get(1)?,
        op = op.trim(),
    );
    Some(sql)
}

fn generate_divide_measure_recursive(
    dax: &str,
    model: &ConversionModel,
    visited: &mut [String],
) -> Option<String> {
    let rest = dax.trim_start_matches("DIVIDE(").trim();
    let args = split_args(rest);
    if args.len() < 2 {
        return None;
    }
    let meas_a = args[0]
        .trim()
        .trim_matches(|c: char| c == '[' || c == ']' || c == ' ')
        .to_string();
    let meas_b = args[1]
        .trim()
        .trim_matches(|c: char| c == '[' || c == ']' || c == ' ')
        .to_string();
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

fn generate_sql_for_measure(
    name: &str,
    model: &ConversionModel,
    visited: &[String],
) -> Option<String> {
    let target = name.trim().to_lowercase();

    if visited.iter().any(|v| v == &target) {
        return None;
    }

    let meas = model
        .fact_table
        .measures
        .iter()
        .find(|m| m.name.trim().to_lowercase() == target)?;

    let mut new_visited = visited.to_vec();
    new_visited.push(target.clone());

    let all_measures: Vec<&MeasureInfo> = model
        .fact_table
        .measures
        .iter()
        .chain(model.dimensions.iter().flat_map(|t| &t.measures))
        .chain(model.date_roles.iter().flat_map(|t| &t.measures))
        .collect();
    let meas_ref = all_measures
        .iter()
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
    let table_name = normalize_ident(&t.name);
    let mut out = format!("CREATE TABLE IF NOT EXISTS {table_name} (\n");
    let visible_cols: Vec<&ColumnInfo> = t.columns.iter().collect();
    for (i, c) in visible_cols.iter().enumerate() {
        let col_name = normalize_ident(&c.source_column);
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

    let simple: Vec<_> = m
        .fact_table
        .measures
        .iter()
        .filter(|m| m.classification == "simple")
        .collect();
    let fallback: Vec<_> = m
        .fact_table
        .measures
        .iter()
        .filter(|m| m.classification == "sql_fallback")
        .collect();
    let manual: Vec<_> = m
        .fact_table
        .measures
        .iter()
        .filter(|m| m.classification == "manual")
        .collect();

    out.push_str("## Summary\n\n");
    out.push_str(&format!("- Fact table: {}\n", m.fact_table.ssas_name));
    out.push_str(&format!(
        "- Dimensions: {}\n",
        m.dimensions.len() + m.date_roles.len() + m.lookup_tables.len()
    ));
    out.push_str(&format!("- Date-role tables: {}\n", m.date_roles.len()));
    out.push_str(&format!("- Relationships: {}\n", m.relationships.len()));
    out.push_str(&format!(
        "- Measures: {} (simple: {}, bridge: {}, manual: {})\n",
        m.fact_table.measures.len(),
        simple.len(),
        fallback.len(),
        manual.len()
    ));
    out.push_str(
        "- Boundary: bridge/manual definitions belong upstream (an additive column or a mart) —\n  \
         see docs/DESIGN-INVARIANTS.md. `mallard qualify --strict` fails while bridge code remains.\n",
    );
    let hier_rows = hierarchy_report_rows(m);
    if !hier_rows.is_empty() {
        let emitted = hier_rows
            .iter()
            .filter(|(_, _, _, status)| status.starts_with("emitted"))
            .count();
        out.push_str(&format!(
            "- Hierarchies: {} of {} emitted (levels become Excel drill paths)\n",
            emitted,
            hier_rows.len()
        ));
    }
    let roles = date_roles(m);
    if !roles.is_empty() {
        let with_flags = roles.iter().filter(|r| !r.flags.is_empty()).count();
        out.push_str(&format!(
            "- Date roles: {} ({} with flag columns; period-over-period measures need them upstream)\n",
            roles.len(),
            with_flags
        ));
    }
    out.push_str(&format!("- M-partition tables: {} (load_data.sql attempts automated loading, see load_data.sql for details)\n\n",
        if m.fact_table.is_m_partition() { 1usize } else { 0 }
        + m.dimensions.iter().filter(|t| t.is_m_partition()).count()
        + m.date_roles.iter().filter(|t| t.is_m_partition()).count()));

    // Join map
    out.push_str("## Join Map\n\n");
    out.push_str("| Fact Column | Dimension Table | Join Column |\n|---|---|---|\n");
    for rel in &m.relationships {
        if rel.from_table == m.fact_table.name {
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                rel.from_column, rel.to_table, rel.to_column
            ));
        }
    }
    out.push('\n');

    if !hier_rows.is_empty() {
        out.push_str("## Hierarchies\n\n");
        out.push_str(
            "Levels become drill paths in Excel. The runtime config carries one hierarchy per\n\
             dimension, so when a table declares several, the first with resolvable levels is\n\
             emitted and the rest are listed here.\n\n",
        );
        out.push_str("| Table | Hierarchy | Levels | Status |\n|---|---|---|---|\n");
        for (table, hierarchy, levels, status) in &hier_rows {
            out.push_str(&format!(
                "| {table} | {hierarchy} | {levels} | {status} |\n"
            ));
        }
        out.push('\n');
    }

    if !roles.is_empty() {
        out.push_str("## Date roles\n\n");
        out.push_str(
            "Period-over-period measures (`TOTALYTD`, `SAMEPERIODLASTYEAR`, …) need flag columns\n\
             on the calendar. Flags are upstream columns (docs/DESIGN-INVARIANTS.md): the converter\n\
             emits the ones that exist and reports the rest as bridge code.\n\n",
        );
        out.push_str(
            "| Table | Dimension | Date key | Full date | Year / Quarter / Month | Flags |\n|---|---|---|---|---|---|\n",
        );
        let or_dash = |c: &Option<String>| c.clone().unwrap_or_else(|| "—".to_string());
        for r in &roles {
            let flags = if r.flags.is_empty() {
                "none".to_string()
            } else {
                r.flags
                    .iter()
                    .map(|(_, c)| c.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} / {} / {} | {} |\n",
                r.table_name,
                r.dim_id,
                or_dash(&r.date_key),
                or_dash(&r.full_date),
                or_dash(&r.year),
                or_dash(&r.quarter),
                or_dash(&r.month),
                flags
            ));
        }
        out.push('\n');
        let ti: Vec<String> = m
            .fact_table
            .measures
            .iter()
            .filter(|meas| {
                matches!(
                    meas.classification.as_str(),
                    "time_ytd" | "time_prior_year" | "time_qtd" | "time_mtd"
                )
            })
            .map(|meas| {
                let (role, inferred) = time_role_for_measure(&roles, &meas.expression);
                format!(
                    "{} → {} ({})",
                    meas.name,
                    role.map(|r| r.dim_id.as_str()).unwrap_or("—"),
                    if inferred {
                        "inferred"
                    } else {
                        "assumed first role"
                    }
                )
            })
            .collect();
        if !ti.is_empty() {
            out.push_str(&format!(
                "Time-intelligence measures: {}\n\n",
                ti.join("; ")
            ));
        }
    }

    out.push_str("## Simple measures\n\n");
    out.push_str("| Measure | DAX | SQL |\n|---|---|---|\n");
    let fact_for_sql = &m.fact_table;
    for meas in &simple {
        let dax = meas.expression.as_str();
        let sql = simple_aggregate_sql(fact_for_sql, dax)
            .or_else(|| expr_to_sql(&dax_to_expr(dax)))
            .unwrap_or_default();
        out.push_str(&format!("| {} | {} | {} |\n", meas.name, dax, sql));
    }

    out.push_str("\n## Bridge code — define upstream\n\n");
    out.push_str(
        "These measures carry DAX-derived SQL in `sql_fallback/`. It is bridge code: it keeps\n\
         Excel working during a migration. Move each definition upstream and delete the file —\n\
         see `docs/DESIGN-INVARIANTS.md`.\n\n",
    );
    out.push_str(
        "| Measure | DAX pattern | Suggested upstream artifact | Bridge file |\n|---|---|---|---|\n",
    );
    for m in &fallback {
        let dax = m.expression.as_str();
        out.push_str(&format!(
            "| {} | {} | {} | sql_fallback/{}.sql |\n",
            m.name,
            dax,
            upstream_suggestion(dax),
            normalize_ident(&m.name)
        ));
    }

    if !manual.is_empty() {
        out.push_str("\n## Manual review required — define upstream\n\n");
        out.push_str(
            "No SQL was generated for these. Define them in the transformation layer\n\
             (see `docs/DESIGN-INVARIANTS.md`):\n\n",
        );
        out.push_str("| Measure | DAX pattern | Suggested upstream artifact |\n|---|---|---|\n");
        for m in &manual {
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                m.name,
                m.expression.as_str(),
                upstream_suggestion(m.expression.as_str())
            ));
        }
    }

    out.push_str("\n## Data loading\n\n");
    out.push_str("The converter generates three SQL files for data loading:\n\n");
    out.push_str("- `schema.sql` — CREATE TABLE statements (run first)\n");
    out.push_str("- `load_data.sql` — loads real data from source databases (requires DuckDB extensions or CSV files)\n");
    out.push_str(
        "- `load_dummy_data.sql` — generates synthetic data for testing (always works)\n\n",
    );

    if !m.data_sources.is_empty() {
        out.push_str("### Data sources detected\n\n");
        out.push_str("| Name | Provider | Server | Database |\n|---|---|---|---|\n");
        for ds in &m.data_sources {
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                ds.name, ds.provider, ds.server, ds.database
            ));
        }
        out.push('\n');
    }

    out.push_str("### Quick start\n\n");
    let cube_db = format!("{}.db", normalize_ident(&m.cube));
    out.push_str(&format!(
        "```\nduckdb data/{cube_db} < bootstrap.sql\n```\n\n\
         This creates the schema, seeds `date_dim` (if needed), and loads dummy data.\n\
         For real data, edit `bootstrap.sql` to use `load_data.sql` instead.\n\n",
        cube_db = cube_db,
    ));

    out.push_str("### Tables to load\n\n");
    out.push_str(&format!(
        "- [ ] `{}` (fact)\n",
        normalize_ident(&m.fact_table.name)
    ));
    for t in &m.dimensions {
        out.push_str(&format!(
            "- [ ] `{}` (dimension)\n",
            normalize_ident(&t.name)
        ));
    }
    for t in &m.date_roles {
        out.push_str(&format!(
            "- [ ] `{}` (date-role)\n",
            normalize_ident(&t.name)
        ));
    }
    for t in &m.lookup_tables {
        out.push_str(&format!("- [ ] `{}` (lookup)\n", normalize_ident(&t.name)));
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
                    let sql_str = if tp.filter_expression.is_empty() {
                        "(empty)"
                    } else {
                        &tp.filter_expression
                    };
                    let status = if tp.metadata_permission == "none" {
                        "OLS — table hidden"
                    } else if tp.dax_filter.is_some() && tp.filter_expression.is_empty() {
                        "DAX filter preserved, SQL filter empty — manual SQL translation required"
                    } else if !tp.filter_expression.is_empty() {
                        "Enforced (SQL filter)"
                    } else {
                        "No filter — full access"
                    };
                    out.push_str(&format!(
                        "| {} | {} | {} | {} | {} |\n",
                        tp.table, sql_str, dax_str, tp.metadata_permission, status
                    ));
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
                ColumnInfo {
                    name: "itemid".into(),
                    data_type: "int64".into(),
                    source_column: "itemid".into(),
                    is_hidden: false,
                },
                ColumnInfo {
                    name: "unitcost".into(),
                    data_type: "double".into(),
                    source_column: "unitcost".into(),
                    is_hidden: false,
                },
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

    /// A synthetic date-role table with a hierarchy and optional flag columns.
    fn date_role(
        name: &str,
        columns: &[&str],
        levels: &[(&str, &str, u32)],
        flags: &[&str],
    ) -> TableInfo {
        let mut cols: Vec<ColumnInfo> = columns
            .iter()
            .map(|c| ColumnInfo {
                name: c.to_string(),
                data_type: "string".into(),
                source_column: c.to_lowercase().replace(' ', "_"),
                is_hidden: false,
            })
            .collect();
        for f in flags {
            cols.push(ColumnInfo {
                name: f.to_string(),
                data_type: "boolean".into(),
                source_column: f.to_string(),
                is_hidden: false,
            });
        }
        TableInfo {
            name: name.into(),
            ssas_name: name.into(),
            description: String::new(),
            columns: cols,
            measures: vec![],
            partitions: vec![],
            hierarchies: vec![HierarchyInfo {
                name: "Calendar Hierarchy".into(),
                levels: levels
                    .iter()
                    .map(|(n, c, o)| HierarchyLevelInfo {
                        name: n.to_string(),
                        column: c.to_string(),
                        ordinal: *o,
                    })
                    .collect(),
            }],
        }
    }

    fn model_with_date_roles(roles: Vec<TableInfo>) -> ConversionModel {
        let mut m = make_generic_model();
        for r in &roles {
            m.relationships.push(RelInfo {
                from_table: "Orders".into(),
                from_column: "itemid".into(),
                to_table: r.name.clone(),
                to_column: "Date Key".into(),
            });
        }
        m.date_roles = roles;
        m
    }

    fn ti_measure(name: &str, dax: &str) -> MeasureInfo {
        MeasureInfo {
            name: name.into(),
            expression: dax.into(),
            display_folder: String::new(),
            classification: "time_ytd".into(),
        }
    }

    const CAL_COLUMNS: [&str; 5] = ["Date Key", "Full Date", "Year", "Quarter", "Month"];
    const CAL_LEVELS: [(&str, &str, u32); 4] = [
        ("Year", "Year", 0),
        ("Quarter", "Quarter", 1),
        ("Month", "Month", 2),
        ("Full Date", "Full Date", 3),
    ];

    #[test]
    fn converter_resolves_date_role_columns() {
        let (parsed, _warnings) = parse_bim::parse_model("data/retailanalytics.bim");
        let model = classify_model(parsed);
        let cfg: crate::project::config::ProxyConfig =
            serde_json::from_str(&render_proxy_config(&model)).expect("config parses");

        let dd = &cfg
            .time_intelligence
            .as_ref()
            .expect("time_intelligence block")
            .date_dimension;
        assert_eq!(dd.dimension_id, "Dates");
        assert_eq!(dd.table_name, "dates");
        assert_eq!(dd.date_key_column, "datekey");
        assert_eq!(dd.full_date_column, "fulldate");
        assert_eq!(dd.flag_columns.year_column, "year");
        assert_eq!(dd.flag_columns.quarter_column, "quarternumber");
        assert_eq!(dd.flag_columns.month_column, "monthnumber");
        assert!(
            dd.flag_columns.ytd_flag_column.is_empty(),
            "the retail sample has no flag columns — none may be invented"
        );

        // Every emitted column must exist in the generated schema.
        let schema = render_schema(&model);
        for col in [
            &dd.date_key_column,
            &dd.full_date_column,
            &dd.flag_columns.year_column,
            &dd.flag_columns.quarter_column,
            &dd.flag_columns.month_column,
        ] {
            assert!(
                schema.contains(&format!("    {col} ")),
                "schema.sql is missing {col}:\n{schema}"
            );
        }

        let report = render_report(&model);
        assert!(report.contains("## Date roles"), "{report}");
        assert!(
            report.contains(
                "| dates | Dates | datekey | fulldate | year / quarternumber / monthnumber | none |"
            ),
            "{report}"
        );
    }

    #[test]
    fn time_intelligence_binds_to_the_measure_date_role() {
        let roles = vec![
            date_role("Cal A", &CAL_COLUMNS, &CAL_LEVELS, &["ytd_flag"]),
            date_role("Cal B", &CAL_COLUMNS, &CAL_LEVELS, &["ytd_flag"]),
        ];
        let mut model = model_with_date_roles(roles);
        model.fact_table.measures = vec![ti_measure(
            "YTD B",
            "= TOTALYTD(SUM('Orders'[amount]), 'Cal B'[Full Date])",
        )];

        let cfg: crate::project::config::ProxyConfig =
            serde_json::from_str(&render_proxy_config(&model)).expect("config parses");
        let meas = cfg.measures.iter().find(|m| m.id == "YTD B").unwrap();
        let ti = meas
            .time_intelligence
            .as_ref()
            .expect("per-measure time intelligence");
        assert_eq!(ti.dimension_id.as_deref(), Some("Cal B"));
        assert_eq!(ti.flag_column, "ytd_flag");

        // The global block resolves the first role's real columns.
        let dd = &cfg.time_intelligence.as_ref().unwrap().date_dimension;
        assert_eq!(dd.dimension_id, "Cal A");
        assert_eq!(dd.date_key_column, "date_key");
        assert_eq!(dd.full_date_column, "full_date");
        assert_eq!(dd.flag_columns.year_column, "year");
        assert_eq!(dd.flag_columns.ytd_flag_column, "ytd_flag");

        // The report records which role was inferred.
        let report = render_report(&model);
        assert!(report.contains("YTD B → Cal B (inferred)"), "{report}");
    }

    #[test]
    fn time_intelligence_without_flags_becomes_bridge_code() {
        let roles = vec![date_role("Cal A", &CAL_COLUMNS, &CAL_LEVELS, &[])];
        let mut model = model_with_date_roles(roles);
        model.fact_table.measures = vec![ti_measure(
            "YTD",
            "= TOTALYTD(SUM('Orders'[amount]), 'Cal A'[Full Date])",
        )];

        downgrade_time_intelligence_without_flags(&mut model);
        assert_eq!(model.fact_table.measures[0].classification, "sql_fallback");

        let cfg: crate::project::config::ProxyConfig =
            serde_json::from_str(&render_proxy_config(&model)).expect("config parses");
        assert!(
            cfg.measures
                .iter()
                .find(|m| m.id == "YTD")
                .unwrap()
                .time_intelligence
                .is_none(),
            "a measure whose flag does not exist must not reference it"
        );
        assert!(
            cfg.time_intelligence
                .as_ref()
                .unwrap()
                .date_dimension
                .flag_columns
                .ytd_flag_column
                .is_empty()
        );

        let report = render_report(&model);
        assert!(
            report.contains("date flag columns on the calendar"),
            "{report}"
        );
        assert!(
            report.contains(
                "| cal_a | Cal A | date_key | full_date | year / quarter / month | none |"
            ),
            "{report}"
        );
    }

    #[test]
    fn time_intelligence_keeps_flags_when_present() {
        let roles = vec![date_role("Cal A", &CAL_COLUMNS, &CAL_LEVELS, &["ytd_flag"])];
        let mut model = model_with_date_roles(roles);
        model.fact_table.measures = vec![ti_measure(
            "YTD",
            "= TOTALYTD(SUM('Orders'[amount]), 'Cal A'[Full Date])",
        )];
        downgrade_time_intelligence_without_flags(&mut model);
        assert_eq!(model.fact_table.measures[0].classification, "time_ytd");
    }

    #[test]
    fn generic_calculate_sum_produces_real_sql() {
        let model = make_generic_model();
        let meas = model
            .fact_table
            .measures
            .iter()
            .find(|m| m.name == "Total Sales")
            .unwrap();
        let sql = generate_fallback_sql(meas, &model);
        assert!(!sql.contains("SELECT 1 AS dummy"), "should not be a stub");
        assert!(sql.contains("amount"), "should resolve amount column");
        assert!(sql.contains("status"), "should resolve status filter");
        assert!(sql.contains("orders"), "should use orders table");
    }

    #[test]
    fn generic_sumx_filter_related_produces_real_sql() {
        let model = make_generic_model();
        let meas = model
            .fact_table
            .measures
            .iter()
            .find(|m| m.name == "Total Cost")
            .unwrap();
        let sql = generate_fallback_sql(meas, &model);
        assert!(!sql.contains("SELECT 1 AS dummy"), "should not be a stub");
        assert!(sql.contains("qty"), "should resolve qty column");
        assert!(sql.contains("unitcost"), "should resolve unitcost column");
        assert!(sql.contains("items"), "should join items table");
    }

    #[test]
    fn generic_measure_arithmetic_produces_real_sql() {
        let model = make_generic_model();
        let meas = model
            .fact_table
            .measures
            .iter()
            .find(|m| m.name == "Net Profit")
            .unwrap();
        let sql = generate_fallback_sql(meas, &model);
        assert!(!sql.contains("SELECT 1 AS dummy"), "should not be a stub");
        assert!(
            sql.contains("amount"),
            "should contain Total Sales subquery"
        );
        assert!(sql.contains("qty"), "should contain Total Cost subquery");
    }

    #[test]
    fn generic_divide_measure_produces_real_sql() {
        let model = make_generic_model();
        let meas = model
            .fact_table
            .measures
            .iter()
            .find(|m| m.name == "Margin Pct")
            .unwrap();
        let sql = generate_fallback_sql(meas, &model);
        assert!(!sql.contains("SELECT 1 AS dummy"), "should not be a stub");
        assert!(sql.contains("CASE WHEN"), "should be safe division");
        assert!(
            sql.contains("amount"),
            "should contain Total Sales subquery"
        );
    }

    #[test]
    fn generate_sql_for_measure_no_hardcoded_retail_names() {
        let source = include_str!("convert_tabular.rs");
        assert!(
            !source.contains("\"TOTAL REVENUE\""),
            "no hardcoded TOTAL REVENUE"
        );
        assert!(
            !source.contains("\"TOTAL COGS\""),
            "no hardcoded TOTAL COGS"
        );
        assert!(
            !source.contains("\"GROSS PROFIT\""),
            "no hardcoded GROSS PROFIT"
        );
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

        assert!(
            out_dir.join("load_data.sql").exists(),
            "load_data.sql should exist"
        );
        assert!(
            out_dir.join("load_dummy_data.sql").exists(),
            "load_dummy_data.sql should exist"
        );
        assert!(
            out_dir.join("bootstrap.sql").exists(),
            "bootstrap.sql should exist"
        );
        assert!(
            out_dir.join("proxy-config.json").exists(),
            "proxy-config.json should exist"
        );

        // Verify dummy data uses the flag value
        let dummy = fs::read_to_string(out_dir.join("load_dummy_data.sql")).unwrap();
        assert!(
            dummy.contains("generate_series(1, 5000)"),
            "should use --dummy-rows=5000 for fact table"
        );
        assert!(
            dummy.contains("generate_series(1, 500)"),
            "dimension table rows should be dummy_rows/10"
        );

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
        assert!(
            out_dir.join("bootstrap.sql").exists(),
            "bootstrap.sql should always exist"
        );
        assert!(
            !out_dir.join("seed_date_dim.sql").exists(),
            "seed_date_dim.sql should NOT exist (no date roles)"
        );

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

        let code = run(vec!["convert-tabular".into(), src.into(), out_str]);
        assert_eq!(code, 0);

        let bootstrap = fs::read_to_string(out_dir.join("bootstrap.sql")).unwrap();
        assert!(
            bootstrap.contains(".read load_dummy_data.sql"),
            "bootstrap should reference load_dummy_data.sql"
        );
        assert!(
            bootstrap.contains(".read schema.sql"),
            "bootstrap should reference schema.sql"
        );
        assert!(
            bootstrap.contains("-- .read load_data.sql"),
            "bootstrap should have load_data.sql commented out"
        );

        // Date-role tables are real model tables; the legacy date_dim seed is
        // no longer emitted.
        assert!(
            !bootstrap.contains("seed_date_dim.sql"),
            "bootstrap must not reference the legacy date_dim seed"
        );
        assert!(
            !out_dir.join("seed_date_dim.sql").exists(),
            "seed_date_dim.sql should not be emitted"
        );

        // Cleanup
        let _ = fs::remove_dir_all(&out_dir);
    }

    // Plan 044: fallback files are bridge code and say so; the report turns
    // them into an upstream checklist.
    #[test]
    fn fallback_files_are_labelled_bridge_code() {
        let model = make_generic_model();
        let sql = generate_fallback_sql(&model.fact_table.measures[0], &model);
        assert!(
            sql.starts_with("-- BRIDGE CODE"),
            "fallback files must be labelled: {sql}"
        );
        assert!(sql.contains("docs/DESIGN-INVARIANTS.md"), "{sql}");
        // Comment lines are legal SQL; the statement is still there.
        let body: String = sql
            .lines()
            .filter(|l| !l.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(body.to_uppercase().contains("SELECT"), "{body}");
    }

    #[test]
    fn report_lists_the_upstream_checklist() {
        let report = render_report(&make_generic_model());
        assert!(report.contains("Bridge code — define upstream"), "{report}");
        assert!(report.contains("Suggested upstream artifact"), "{report}");
        assert!(
            report.contains("status/bucket flag column for the CALCULATE filter"),
            "CALCULATE measures get a flag suggestion: {report}"
        );
        assert!(report.contains("docs/DESIGN-INVARIANTS.md"), "{report}");
        assert!(
            report.contains("qualify --strict"),
            "the report points at the enforcement gate: {report}"
        );
    }

    #[test]
    fn upstream_suggestions_cover_the_patterns() {
        assert!(upstream_suggestion("MEDIAN('T'[c])").contains("median"));
        assert!(upstream_suggestion("DISTINCTCOUNT('T'[c])").contains("grain change"));
        assert!(
            upstream_suggestion(
                "CALCULATE([M], FILTER(ALLSELECTED('D'[x]), ISONORAFTER('D'[x], MAX('D'[x]), DESC)))"
            )
            .contains("cumulative")
        );
        assert!(
            upstream_suggestion("SUMX(FILTER('T', 'T'[x]=1), 'T'[q] * RELATED('D'[c]))")
                .contains("additive column")
        );
        assert!(upstream_suggestion("DIVIDE([A],[B])").contains("numerator"));
        assert!(upstream_suggestion("CALCULATE([M], 'D'[x]=\"y\")").contains("flag column"));
        assert!(upstream_suggestion("[A] + 1").contains("SQL model"));
    }

    #[test]
    fn converter_emits_hierarchy_levels() {
        let (parsed, _warnings) = parse_bim::parse_model("data/retailanalytics.bim");
        let model = classify_model(parsed);
        let config = render_proxy_config(&model);

        // The converted config must be valid for the runtime, levels included.
        let cfg: crate::project::config::ProxyConfig =
            serde_json::from_str(&config).expect("converted config must parse");
        let dates = cfg
            .dimensions
            .iter()
            .find(|d| d.caption == "Dates")
            .expect("Dates dimension");
        assert_eq!(dates.hierarchy_name, "Calendar Hierarchy");
        assert_eq!(
            dates
                .hierarchy_levels
                .iter()
                .map(|l| (l.name.as_str(), l.column.as_str(), l.level_number))
                .collect::<Vec<_>>(),
            vec![
                ("year", "year", 0),
                ("quartername", "quartername", 1),
                ("monthname", "monthname", 2),
                ("fulldate", "fulldate", 3),
            ]
        );

        // Every level column must exist in the generated schema.
        let schema = render_schema(&model);
        for col in ["year", "quartername", "monthname", "fulldate"] {
            assert!(
                schema.contains(&format!("    {col} ")),
                "schema.sql is missing {col}:\n{schema}"
            );
        }

        // The report tells the user what happened to the hierarchy.
        let report = render_report(&model);
        assert!(report.contains("## Hierarchies"), "{report}");
        assert!(
            report.contains(
                "| Dates | Calendar Hierarchy | year → quartername → monthname → fulldate | emitted |"
            ),
            "{report}"
        );
    }

    #[test]
    fn converter_resolves_relationship_columns_to_schema() {
        let (parsed, _warnings) = parse_bim::parse_model("data/retailanalytics.bim");
        let model = classify_model(parsed);
        let cfg: crate::project::config::ProxyConfig =
            serde_json::from_str(&render_proxy_config(&model)).expect("config parses");
        let schema = render_schema(&model);

        let mut cols = Vec::new();
        for rel in &cfg.relationships {
            cols.push(rel.fact_column.clone());
            cols.push(rel.dim_column.clone());
        }
        assert_eq!(cols.len(), 10, "5 relationships x 2 endpoints");
        // The export names columns ("Customer ID"); schema.sql creates the
        // source columns ("customerid"). Unmapped names break every join.
        for expected in ["datekey", "customerid", "productid", "promoid", "storeid"] {
            assert!(cols.contains(&expected.to_string()), "{cols:?}");
        }
        for c in &cols {
            assert!(
                schema.contains(&format!("    {c} ")),
                "schema.sql is missing relationship column {c}:\n{schema}"
            );
        }
    }

    #[test]
    fn hierarchy_levels_skip_columns_missing_from_schema() {
        let mut model = make_generic_model();
        model.lookup_tables[0].hierarchies = vec![HierarchyInfo {
            name: "Product".into(),
            levels: vec![
                HierarchyLevelInfo {
                    name: "Item".into(),
                    column: "itemid".into(),
                    ordinal: 0,
                },
                HierarchyLevelInfo {
                    name: "Ghost".into(),
                    column: "Does Not Exist".into(),
                    ordinal: 1,
                },
            ],
        }];
        let (hier, levels) = emit_hierarchy(&model.lookup_tables[0]).expect("resolvable level");
        assert_eq!(hier.name, "Product");
        assert!(levels.contains("\"name\": \"Item\""), "{levels}");
        assert!(
            !levels.contains("Ghost"),
            "unresolvable level dropped: {levels}"
        );

        let report = render_report(&model);
        assert!(report.contains("emitted with 1 of 2 levels"), "{report}");
    }

    #[test]
    fn hierarchy_levels_emit_the_first_hierarchy_only() {
        let mut model = make_generic_model();
        model.lookup_tables[0].hierarchies = vec![
            HierarchyInfo {
                name: "First".into(),
                levels: vec![HierarchyLevelInfo {
                    name: "Item".into(),
                    column: "itemid".into(),
                    ordinal: 0,
                }],
            },
            HierarchyInfo {
                name: "Second".into(),
                levels: vec![HierarchyLevelInfo {
                    name: "Cost".into(),
                    column: "unitcost".into(),
                    ordinal: 0,
                }],
            },
        ];
        let (hier, _) = emit_hierarchy(&model.lookup_tables[0]).expect("first hierarchy");
        assert_eq!(hier.name, "First");

        let report = render_report(&model);
        assert!(
            report.contains("not emitted — this dimension already carries `First`"),
            "{report}"
        );
    }

    #[test]
    fn period_to_date_measures_bind_to_their_flag() {
        let roles = vec![date_role(
            "Cal A",
            &CAL_COLUMNS,
            &CAL_LEVELS,
            &["qtd_flag", "mtd_flag"],
        )];
        let mut model = model_with_date_roles(roles);
        model.fact_table.measures = vec![
            MeasureInfo {
                name: "QTD".into(),
                expression: "= TOTALQTD(SUM('Orders'[amount]), 'Cal A'[Full Date])".into(),
                display_folder: String::new(),
                classification: "time_qtd".into(),
            },
            MeasureInfo {
                name: "MTD".into(),
                expression: "= TOTALMTD(SUM('Orders'[amount]), 'Cal A'[Full Date])".into(),
                display_folder: String::new(),
                classification: "time_mtd".into(),
            },
        ];
        downgrade_time_intelligence_without_flags(&mut model);
        assert_eq!(model.fact_table.measures[0].classification, "time_qtd");
        assert_eq!(model.fact_table.measures[1].classification, "time_mtd");

        let cfg: crate::project::config::ProxyConfig =
            serde_json::from_str(&render_proxy_config(&model)).expect("config parses");
        let qtd = cfg.measures.iter().find(|m| m.id == "QTD").unwrap();
        let qtd_ti = qtd
            .time_intelligence
            .as_ref()
            .expect("QTD time intelligence");
        assert_eq!(qtd_ti.flag_column, "qtd_flag");
        assert_eq!(qtd_ti.dimension_id.as_deref(), Some("Cal A"));
        assert!(
            qtd.sql_expr.contains("SUM(amount)"),
            "inner aggregation extracted: {}",
            qtd.sql_expr
        );
        let mtd = cfg.measures.iter().find(|m| m.id == "MTD").unwrap();
        assert_eq!(
            mtd.time_intelligence.as_ref().unwrap().flag_column,
            "mtd_flag"
        );
    }

    #[test]
    fn period_to_date_without_flags_becomes_bridge_code() {
        // The calendar has a YTD flag but no QTD/MTD flags.
        let roles = vec![date_role("Cal A", &CAL_COLUMNS, &CAL_LEVELS, &["ytd_flag"])];
        let mut model = model_with_date_roles(roles);
        model.fact_table.measures = vec![MeasureInfo {
            name: "QTD".into(),
            expression: "= TOTALQTD(SUM('Orders'[amount]), 'Cal A'[Full Date])".into(),
            display_folder: String::new(),
            classification: "time_qtd".into(),
        }];
        downgrade_time_intelligence_without_flags(&mut model);
        assert_eq!(model.fact_table.measures[0].classification, "sql_fallback");
        let report = render_report(&model);
        assert!(
            report.contains("date flag columns on the calendar"),
            "{report}"
        );
    }

    #[test]
    fn plain_aggregates_are_lowered() {
        let model = make_generic_model();
        let fact = &model.fact_table;
        for (dax, expected) in [
            ("= SUM('Orders'[Amount])", "SUM(amount)"),
            ("= COUNT('Orders'[Itemid])", "COUNT(itemid)"),
            (
                "= DISTINCTCOUNT('Orders'[Status])",
                "COUNT(DISTINCT status)",
            ),
            ("= AVERAGE('Orders'[Qty])", "AVG(qty)"),
            ("= MIN('Orders'[Amount])", "MIN(amount)"),
            ("= MAX('Orders'[Amount])", "MAX(amount)"),
            (
                "= CALCULATE(SUM('Orders'[Amount]), 'Orders'[Status] = 1)",
                "",
            ),
        ] {
            assert_eq!(
                simple_aggregate_sql(fact, dax).unwrap_or_default(),
                expected,
                "{dax}"
            );
        }

        // Model column names resolve through the schema mapping
        // (`Order Quantity` → `orderqty`), like relationships and levels.
        let mut mapped = model.fact_table.clone();
        mapped.columns.push(ColumnInfo {
            name: "Order Quantity".into(),
            data_type: "int64".into(),
            source_column: "orderqty".into(),
            is_hidden: false,
        });
        assert_eq!(
            simple_aggregate_sql(&mapped, "= SUM('Orders'[Order Quantity])").unwrap(),
            "SUM(orderqty)"
        );
    }

    #[test]
    fn plain_aggregate_measures_keep_real_sql() {
        let mut model = make_generic_model();
        model.fact_table.measures = vec![MeasureInfo {
            name: "Customers".into(),
            expression: "= DISTINCTCOUNT('Orders'[Status])".into(),
            display_folder: String::new(),
            classification: "simple".into(),
        }];
        let cfg: crate::project::config::ProxyConfig =
            serde_json::from_str(&render_proxy_config(&model)).expect("config parses");
        let meas = cfg.measures.iter().find(|m| m.id == "Customers").unwrap();
        assert_eq!(meas.sql_expr, "COUNT(DISTINCT status)");
        assert!(
            meas.sql_fallback_file.is_none(),
            "a plain aggregate is not bridge code"
        );
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
        assert!(
            report.contains("Data sources detected"),
            "report should mention data sources"
        );
        assert!(
            report.contains("TestSource"),
            "report should include data source name"
        );
        assert!(
            report.contains("MY-SERVER"),
            "report should include data source server"
        );
        assert!(
            report.contains("load_data.sql"),
            "report should reference load_data.sql"
        );
        assert!(
            report.contains("load_dummy_data.sql"),
            "report should reference load_dummy_data.sql"
        );
        assert!(
            report.contains("bootstrap.sql"),
            "report should reference bootstrap.sql"
        );
        assert!(
            report.contains("Quick start"),
            "report should have quick start section"
        );
    }
}
