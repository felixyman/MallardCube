/// AutoModel — zero-config semantic model from any DuckDB database.
///
/// Point MallardCube at a DuckDB file and it auto-detects a fact table,
/// measures (numeric columns), dimensions (FK-linked + degenerate string
/// columns), and date hierarchies (DATE columns → generated date_dim tables),
/// producing a `ProxyConfig` with no hand-written metadata.
///
/// Two entry points:
/// - CLI: `auto-model <db_path> [--output <dir>] [--fact <table>]`
///   writes `proxy-config.json` (+ `bootstrap.sql` for date dims), read-only.
/// - Runtime: `MALLARDCUBE_DB=<path>` triggers detection + date_dim seeding at
///   serve time (see `detect_config(.., seed_dates=true)`).
use crate::proxy_config::{
    DateDimensionConfig, DateFlagColumns, DimensionConfig, HierarchyLevelConfig, MeasureConfig,
    ProxyConfig, RelationshipConfig, TimeIntelligenceConfig,
};
use duckdb::{Connection, params};
use std::path::Path;

// ---------------------------------------------------------------------------
// Schema introspection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Column {
    name: String,
    data_type: String,
}

impl Column {
    fn is_numeric(&self) -> bool {
        matches!(
            self.data_type.to_uppercase().as_str(),
            "INTEGER"
                | "BIGINT"
                | "HUGEINT"
                | "SMALLINT"
                | "TINYINT"
                | "DOUBLE"
                | "FLOAT"
                | "REAL"
                | "DECIMAL"
        )
    }

    fn is_integer(&self) -> bool {
        matches!(
            self.data_type.to_uppercase().as_str(),
            "INTEGER" | "BIGINT" | "HUGEINT" | "SMALLINT" | "TINYINT"
        )
    }

    fn is_string(&self) -> bool {
        matches!(
            self.data_type.to_uppercase().as_str(),
            "VARCHAR" | "TEXT" | "STRING" | "CHAR" | "BOOLEAN"
        )
    }

    fn is_date(&self) -> bool {
        self.data_type.eq_ignore_ascii_case("DATE")
    }
}

#[derive(Debug, Clone)]
struct Table {
    name: String,
    row_count: i64,
    columns: Vec<Column>,
}

/// A declared or heuristic foreign key from the fact table to a dimension table.
#[derive(Debug, Clone)]
struct ForeignKey {
    fact_column: String,
    dim_table: String,
    dim_column: String,
}

fn is_safe_ident(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Column names that look like surrogate keys — meaningless as SUM measures.
fn is_key_like(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n == "id" || n.ends_with("_id") || n.ends_with("_key")
}

fn q(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn list_tables(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT table_name FROM information_schema.tables \
             WHERE table_schema = 'main' AND table_type = 'BASE TABLE' ORDER BY table_name",
        )
        .map_err(|e| format!("list tables: {e}"))?;
    Ok(stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| format!("list tables: {e}"))?
        .filter_map(|r| r.ok())
        .collect())
}

fn table_columns(conn: &Connection, table: &str) -> Result<Vec<Column>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT column_name, data_type FROM information_schema.columns \
             WHERE table_schema = 'main' AND table_name = ? ORDER BY ordinal_position",
        )
        .map_err(|e| format!("columns for {table}: {e}"))?;
    Ok(stmt
        .query_map(params![table], |r| {
            Ok(Column {
                name: r.get(0)?,
                data_type: r.get(1)?,
            })
        })
        .map_err(|e| format!("columns for {table}: {e}"))?
        .filter_map(|r| r.ok())
        .collect())
}

fn table_row_count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {}", q(table)), [], |r| {
        r.get(0)
    })
    .unwrap_or(0)
}

fn declared_foreign_keys(conn: &Connection, fact: &str) -> Vec<ForeignKey> {
    let mut stmt = match conn.prepare(&format!(
        "SELECT \"from\", \"table\", \"to\" FROM pragma_foreign_key_list('{fact}')"
    )) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map([], |r| {
        Ok(ForeignKey {
            fact_column: r.get(0)?,
            dim_table: r.get(1)?,
            dim_column: r.get(2)?,
        })
    })
    .ok()
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

fn approx_distinct(conn: &Connection, table: &str, column: &str) -> u32 {
    conn.query_row(
        &format!(
            "SELECT approx_count_distinct({})::INTEGER FROM {}",
            q(column),
            q(table)
        ),
        [],
        |r| r.get::<_, u32>(0),
    )
    .unwrap_or(0)
    .min(10_000)
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

pub struct Detected {
    pub config: ProxyConfig,
    /// (fact date column, generated date_dim table) — for bootstrap.sql.
    pub date_dims: Vec<(String, String)>,
    pub fact_table: String,
}

/// Detect a semantic model from a DuckDB database.
///
/// `fact_override` pins the fact table (bypassing row-count heuristics).
/// When `seed_dates` is true the connection must be writable and the date_dim
/// tables are created in-place; otherwise they are only described in the config
/// (and the caller may emit `bootstrap.sql` separately).
pub fn detect_config(
    db_path: &str,
    fact_override: Option<&str>,
    seed_dates: bool,
) -> Result<Detected, String> {
    let conn = Connection::open(db_path).map_err(|e| format!("open {db_path}: {e}"))?;

    let tables: Vec<Table> = list_tables(&conn)?
        .into_iter()
        .filter(|t| is_safe_ident(t))
        .map(|name| {
            let columns = table_columns(&conn, &name).unwrap_or_default();
            let row_count = table_row_count(&conn, &name);
            Table {
                name,
                row_count,
                columns,
            }
        })
        .collect();

    if tables.is_empty() {
        return Err(format!("no base tables found in {db_path}"));
    }

    // Fact table: explicit override, else largest table (tie-break: numeric
    // column density).
    let fact = match fact_override {
        Some(t) => tables
            .iter()
            .find(|x| x.name == t)
            .cloned()
            .ok_or_else(|| format!("fact table '{t}' not found"))?,
        None => tables
            .iter()
            .max_by(|a, b| {
                a.row_count
                    .cmp(&b.row_count)
                    .then_with(|| numeric_density(a).cmp(&numeric_density(b)))
            })
            .cloned()
            .expect("tables is non-empty"),
    };

    let mut config = ProxyConfig {
        catalog: db_filename_stem(db_path).to_uppercase(),
        cube: "AutoModel".into(),
        source_name: fact.name.clone(),
        table_name: fact.name.clone(),
        dialect: "duckdb".into(),
        db_path: Some(db_path.to_string()),
        fact_tables: vec![],
        relationships: vec![],
        roles: vec![],
        auth: None,
        time_intelligence: None,
        dimensions: vec![],
        measures: vec![],
    };

    let declared = declared_foreign_keys(&conn, &fact.name);
    let heuristic = heuristic_foreign_keys(&tables, &fact, &declared);

    let mut consumed_cols: Vec<String> = Vec::new();

    // ---- dimensions from FK-linked tables ----
    for fk in declared.iter().chain(heuristic.iter()) {
        if !is_safe_ident(&fk.fact_column) || !is_safe_ident(&fk.dim_column) {
            continue;
        }
        consumed_cols.push(fk.fact_column.clone());
        let Some(dim_table) = tables.iter().find(|t| t.name == fk.dim_table) else {
            continue;
        };
        for col in &dim_table.columns {
            if !col.is_string() || !is_safe_ident(&col.name) {
                continue;
            }
            let id = unique_dim_id(&config.dimensions, &col.name, &dim_table.name);
            config.dimensions.push(dimension(
                &id,
                &col.name,
                config.dimensions.len() as u32 + 1,
                approx_distinct(&conn, &dim_table.name, &col.name),
            ));
            config.relationships.push(RelationshipConfig {
                fact_table: "default".into(),
                fact_column: fk.fact_column.clone(),
                dimension_id: id,
                dim_table: dim_table.name.clone(),
                dim_column: fk.dim_column.clone(),
            });
        }
    }

    // ---- degenerate dimensions on the fact table ----
    for col in &fact.columns {
        if !col.is_string() || !is_safe_ident(&col.name) || consumed_cols.contains(&col.name) {
            continue;
        }
        let id = unique_dim_id(&config.dimensions, &col.name, &fact.name);
        config.dimensions.push(dimension(
            &id,
            &col.name,
            config.dimensions.len() as u32 + 1,
            approx_distinct(&conn, &fact.name, &col.name),
        ));
    }

    // ---- measures (numeric fact columns) ----
    for col in &fact.columns {
        if !col.is_numeric()
            || !is_safe_ident(&col.name)
            || consumed_cols.contains(&col.name)
            || col.is_date()
            || is_key_like(&col.name)
        {
            continue;
        }
        config
            .measures
            .push(measure(col, config.measures.len() as u32 + 1, &fact.name));
    }
    // Fallback: a COUNT measure if no numeric columns were detected.
    if config.measures.is_empty() {
        config.measures.push(count_measure(&fact.name));
    }

    // ---- date dimensions ----
    let mut date_dims: Vec<(String, String)> = Vec::new();
    let date_cols: Vec<Column> = fact
        .columns
        .iter()
        .filter(|c| c.is_date() && is_safe_ident(&c.name))
        .cloned()
        .collect();
    for (i, col) in date_cols.iter().enumerate() {
        let dim_table = if i == 0 {
            "date_dim".to_string()
        } else {
            format!("date_dim_{}", col.name)
        };
        let dim_id = unique_dim_id(&config.dimensions, &col.name, &fact.name);

        if seed_dates {
            conn.execute_batch(&date_dim_seed_sql(&fact.name, &col.name, &dim_table))
                .map_err(|e| format!("seed {dim_table}: {e}"))?;
        }
        date_dims.push((col.name.clone(), dim_table.clone()));

        let mut dim = dimension(
            &dim_id,
            &col.name,
            config.dimensions.len() as u32 + 1,
            approx_distinct(&conn, &fact.name, &col.name),
        );
        dim.is_date_role = true;
        dim.hierarchy_levels = vec![
            hierarchy_level("Year", "year", 0, 11),
            hierarchy_level("Quarter", "quarter", 1, 44),
            hierarchy_level("Month", "month", 2, 132),
            hierarchy_level("Date", &col.name, 3, 4018),
        ];
        config.dimensions.push(dim);
        config.relationships.push(RelationshipConfig {
            fact_table: "default".into(),
            fact_column: col.name.clone(),
            dimension_id: dim_id.clone(),
            dim_table: dim_table.clone(),
            dim_column: col.name.clone(),
        });

        if i == 0 {
            config.time_intelligence = Some(TimeIntelligenceConfig {
                date_dimension: DateDimensionConfig {
                    dimension_id: dim_id,
                    date_key_column: col.name.clone(),
                    full_date_column: col.name.clone(),
                    table_name: dim_table,
                    flag_columns: DateFlagColumns::default(),
                },
            });
        }
    }

    Ok(Detected {
        config,
        date_dims,
        fact_table: fact.name.clone(),
    })
}

fn numeric_density(t: &Table) -> i64 {
    t.columns.iter().filter(|c| c.is_numeric()).count() as i64
}

fn db_filename_stem(path: &str) -> &str {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("AUTO_DETECTED")
}

/// Infer foreign keys by column-name convention: a fact column `x_id`/`x_key`
/// referencing another table's `id`/`code`/`<x>_id`/`<x>` column.
fn heuristic_foreign_keys(
    tables: &[Table],
    fact: &Table,
    declared: &[ForeignKey],
) -> Vec<ForeignKey> {
    let mut out = Vec::new();
    for col in &fact.columns {
        if declared.iter().any(|fk| fk.fact_column == col.name) {
            continue;
        }
        let base = col
            .name
            .strip_suffix("_id")
            .or_else(|| col.name.strip_suffix("_key"))
            .or_else(|| col.name.strip_suffix("_ID"))
            .or_else(|| col.name.strip_suffix("_KEY"));
        let Some(base) = base else { continue };
        if base.is_empty() || base == fact.name {
            continue;
        }
        let Some(target) = tables.iter().find(|t| {
            t.name != fact.name
                && (t.name == base
                    || t.name == format!("{base}s")
                    || t.name == format!("dim_{base}"))
        }) else {
            continue;
        };
        let dim_col = target
            .columns
            .iter()
            .find(|c| c.name == "id" || c.name == "code" || c.name == col.name)
            .or_else(|| target.columns.first())
            .map(|c| c.name.clone());
        if let Some(dim_col) = dim_col {
            out.push(ForeignKey {
                fact_column: col.name.clone(),
                dim_table: target.name.clone(),
                dim_column: dim_col,
            });
        }
    }
    out
}

fn unique_dim_id(existing: &[DimensionConfig], base: &str, table: &str) -> String {
    if existing.iter().all(|d| d.id != base) {
        return base.to_string();
    }
    format!("{table}_{base}")
}

fn humanize(name: &str) -> String {
    name.replace('_', " ")
}

fn dimension(id: &str, physical_field: &str, ordinal: u32, cardinality: u32) -> DimensionConfig {
    DimensionConfig {
        id: id.to_string(),
        physical_field: physical_field.to_string(),
        caption: humanize(id),
        description: String::new(),
        hierarchy_name: id.to_string(),
        all_level_name: "(All)".into(),
        leaf_level_name: id.to_string(),
        ordinal,
        visible: true,
        has_all: true,
        cardinality_hint: cardinality,
        fact_table: None,
        shared: false,
        is_date_role: false,
        hierarchy_levels: vec![],
    }
}

fn hierarchy_level(
    name: &str,
    column: &str,
    level_number: u32,
    cardinality: u32,
) -> HierarchyLevelConfig {
    HierarchyLevelConfig {
        name: name.to_string(),
        column: column.to_string(),
        level_number,
        cardinality,
    }
}

fn measure(col: &Column, ordinal: u32, group: &str) -> MeasureConfig {
    let id = col.name.clone();
    let (sql_expr, format_string, scale) = if col.is_integer() {
        (
            format!("SUM(CAST({} AS DOUBLE))", q(&col.name)),
            "#,##0".to_string(),
            0,
        )
    } else {
        (format!("SUM({})", q(&col.name)), "#,##0.00".to_string(), 2)
    };
    MeasureConfig {
        id: id.clone(),
        sql_expr,
        caption: humanize(&id),
        display_name: humanize(&id),
        description: String::new(),
        format_string,
        units: String::new(),
        ordinal,
        visible: true,
        fact_table: None,
        aggregator: 1,
        measure_group_name: group.to_string(),
        numeric_precision: 18,
        numeric_scale: scale,
        expression: String::new(),
        sql_fallback_file: None,
        time_intelligence: None,
        fallback_capability: None,
    }
}

fn count_measure(group: &str) -> MeasureConfig {
    MeasureConfig {
        id: "Row Count".into(),
        sql_expr: "COUNT(*)".into(),
        caption: "Row Count".into(),
        display_name: "Row Count".into(),
        description: "Number of rows".into(),
        format_string: "#,##0".into(),
        units: String::new(),
        ordinal: 1,
        visible: true,
        fact_table: None,
        aggregator: 2,
        measure_group_name: group.to_string(),
        numeric_precision: 18,
        numeric_scale: 0,
        expression: String::new(),
        sql_fallback_file: None,
        time_intelligence: None,
        fallback_capability: None,
    }
}

/// DuckDB SQL that materializes a date dimension spanning the fact table's
/// MIN..MAX of `date_col`, with Year/Quarter/Month columns and the standard
/// period flags. The full-date column is named after the fact column so the
/// time-flag filter (`f.<col> IN (SELECT <col> FROM <dim_table> WHERE flag)`)
/// lines up.
pub fn date_dim_seed_sql(fact: &str, date_col: &str, dim_table: &str) -> String {
    format!(
        r#"CREATE TABLE {dim} AS
WITH RECURSIVE dates(d) AS (
    SELECT MIN({col})::DATE FROM {fact}
    UNION ALL
    SELECT d + 1 FROM dates WHERE d < (SELECT MAX({col})::DATE FROM {fact})
)
SELECT
    d AS {col},
    strftime(d, '%Y%m%d')::INTEGER AS date_key,
    strftime(d, '%Y')::INTEGER AS year,
    CEIL(strftime(d, '%m')::INTEGER / 3.0)::INTEGER AS quarter,
    strftime(d, '%m')::INTEGER AS month,
    d <= CURRENT_DATE AND strftime(d, '%Y') = strftime(CURRENT_DATE, '%Y') AS ytd_flag,
    strftime(d, '%Y') = (strftime(CURRENT_DATE, '%Y')::INTEGER - 1)::TEXT
        AND strftime(d, '%j')::INTEGER <= strftime(CURRENT_DATE, '%j')::INTEGER AS prior_year_ytd_flag,
    strftime(d, '%Y') = strftime(CURRENT_DATE, '%Y') AS current_year_flag,
    d <= CURRENT_DATE
        AND strftime(d, '%Y') = strftime(CURRENT_DATE, '%Y')
        AND CEIL(strftime(d, '%m')::INTEGER / 3.0) = CEIL(strftime(CURRENT_DATE, '%m')::INTEGER / 3.0) AS qtd_flag,
    d <= CURRENT_DATE
        AND strftime(d, '%Y') = strftime(CURRENT_DATE, '%Y')
        AND strftime(d, '%m') = strftime(CURRENT_DATE, '%m') AS mtd_flag
FROM dates"#,
        dim = q(dim_table),
        col = q(date_col),
        fact = q(fact),
    )
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

/// `auto-model <db_path> [--output <dir>] [--fact <table>]`
pub fn run(args: Vec<String>) -> i32 {
    let mut db_path: Option<String> = None;
    let mut output: Option<String> = None;
    let mut fact: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--output" | "-o" => {
                i += 1;
                output = args.get(i).cloned();
            }
            "--fact" | "-f" => {
                i += 1;
                fact = args.get(i).cloned();
            }
            _ if db_path.is_none() => db_path = Some(args[i].clone()),
            _ => {}
        }
        i += 1;
    }

    let Some(db_path) = db_path else {
        eprintln!("usage: auto-model <db_path> [--output <dir>] [--fact <table>]");
        return 2;
    };

    let detected = match detect_config(&db_path, fact.as_deref(), false) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("auto-model: {e}");
            return 1;
        }
    };

    let json = match serde_json::to_string_pretty(&detected.config) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("auto-model: serialize config: {e}");
            return 1;
        }
    };

    let out_dir = output.unwrap_or_else(|| ".".to_string());
    if std::fs::create_dir_all(&out_dir).is_err() {
        eprintln!("auto-model: cannot create output dir {out_dir}");
        return 1;
    }
    let config_path = Path::new(&out_dir).join("proxy-config.json");
    if let Err(e) = std::fs::write(&config_path, &json) {
        eprintln!("auto-model: write {}: {e}", config_path.display());
        return 1;
    }
    println!(
        "wrote {} ({} dims, {} measures, fact={})",
        config_path.display(),
        detected.config.dimensions.len(),
        detected.config.measures.len(),
        detected.fact_table,
    );

    if !detected.date_dims.is_empty() {
        let mut bootstrap = String::from("-- Seed date dimensions for AutoModel.\n");
        for (date_col, dim_table) in &detected.date_dims {
            bootstrap.push_str(&date_dim_seed_sql(
                &detected.fact_table,
                date_col,
                dim_table,
            ));
            bootstrap.push_str(";\n");
        }
        let boot_path = Path::new(&out_dir).join("bootstrap.sql");
        if std::fs::write(&boot_path, bootstrap).is_ok() {
            println!(
                "wrote {} (run it in duckdb to seed date dimensions)",
                boot_path.display()
            );
        }
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_db(name: &str) -> String {
        let p = std::env::temp_dir().join(format!(
            "automodel-{name}-{}-{}.duckdb",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_file(&p);
        p.to_string_lossy().to_string()
    }

    fn seed(conn: &Connection) {
        conn.execute_batch(
            r#"CREATE TABLE product (
                 id INTEGER PRIMARY KEY, name VARCHAR, category VARCHAR);
               INSERT INTO product VALUES
                 (1,'Widget','Gadgets'),(2,'Gadget','Gadgets'),(3,'Doodad','Tools');
               CREATE TABLE sales (
                 order_id INTEGER, product_id INTEGER, region VARCHAR,
                 revenue DOUBLE, units INTEGER, order_date DATE);
               INSERT INTO sales VALUES
                 (1,1,'North',100.0,2,'2024-01-15'),
                 (2,1,'South',200.0,1,'2024-02-20'),
                 (3,2,'North',150.0,3,'2025-03-10'),
                 (4,3,'East',300.0,4,'2025-06-05');
               CREATE TABLE inventory (
                 sku INTEGER, warehouse VARCHAR, stock_qty INTEGER);
               INSERT INTO inventory VALUES (1,'WH1',10),(2,'WH2',20);"#,
        )
        .unwrap();
    }

    #[test]
    fn detects_largest_table_as_fact() {
        let db = temp_db("fact");
        let conn = Connection::open(&db).unwrap();
        seed(&conn);
        drop(conn);
        let d = detect_config(&db, None, false).unwrap();
        assert_eq!(d.fact_table, "sales");
        assert_eq!(d.config.table_name, "sales");
    }

    #[test]
    fn fact_override_wins() {
        let db = temp_db("override");
        let conn = Connection::open(&db).unwrap();
        seed(&conn);
        drop(conn);
        let d = detect_config(&db, Some("inventory"), false).unwrap();
        assert_eq!(d.fact_table, "inventory");
    }

    #[test]
    fn measures_exclude_keys_and_join_columns() {
        let db = temp_db("measures");
        let conn = Connection::open(&db).unwrap();
        seed(&conn);
        drop(conn);
        let d = detect_config(&db, None, false).unwrap();
        let ids: Vec<&str> = d.config.measures.iter().map(|m| m.id.as_str()).collect();
        assert!(
            ids.contains(&"revenue"),
            "revenue should be a measure: {ids:?}"
        );
        assert!(ids.contains(&"units"), "units should be a measure: {ids:?}");
        assert!(
            !ids.contains(&"order_id"),
            "surrogate key must not be a measure"
        );
        assert!(
            !ids.contains(&"product_id"),
            "FK column must not be a measure"
        );
    }

    #[test]
    fn fk_table_columns_become_relationship_dimensions() {
        let db = temp_db("dims");
        let conn = Connection::open(&db).unwrap();
        seed(&conn);
        drop(conn);
        let d = detect_config(&db, None, false).unwrap();
        let names: Vec<&str> = d.config.dimensions.iter().map(|x| x.id.as_str()).collect();
        assert!(names.contains(&"name"), "product.name dim: {names:?}");
        assert!(
            names.contains(&"category"),
            "product.category dim: {names:?}"
        );
        assert!(
            names.contains(&"region"),
            "degenerate region dim: {names:?}"
        );
        assert!(
            d.config
                .relationships
                .iter()
                .any(|r| r.dim_table == "product"),
            "relationship to product table expected: {:?}",
            d.config.relationships
        );
    }

    #[test]
    fn date_column_seeds_date_dim_and_builds_hierarchy() {
        let db = temp_db("date");
        let conn = Connection::open(&db).unwrap();
        seed(&conn);
        drop(conn);
        let d = detect_config(&db, None, true).unwrap();
        // date_dim table seeded in the DB
        let check = Connection::open(&db).unwrap();
        let n: i64 = check
            .query_row("SELECT COUNT(*) FROM date_dim", [], |r| r.get(0))
            .unwrap();
        assert!(n >= 4, "date_dim should span MIN..MAX, got {n} rows");

        let date_dim = d
            .config
            .dimensions
            .iter()
            .find(|x| x.is_date_role)
            .expect("a date-role dimension");
        assert_eq!(date_dim.hierarchy_levels.len(), 4);
        assert_eq!(date_dim.hierarchy_levels[0].name, "Year");
        assert!(d.config.time_intelligence.is_some());
        assert!(
            d.config
                .relationships
                .iter()
                .any(|r| r.dim_table == "date_dim"),
            "relationship to date_dim expected"
        );
    }

    #[test]
    fn generated_config_builds_semantic_model() {
        let db = temp_db("model");
        let conn = Connection::open(&db).unwrap();
        seed(&conn);
        drop(conn);
        let d = detect_config(&db, None, true).unwrap();
        let project =
            crate::proxy_project::ProxyProject::from_config(d.config, std::path::Path::new("."))
                .expect("build semantic model from detected config");
        assert_eq!(project.model.primary_source_name(), "sales");
        assert!(project.model.measures.iter().any(|m| m.id == "revenue"));
        assert!(project.model.dimensions.iter().any(|x| x.id == "region"));
        assert!(
            project.model.dimensions.iter().any(|x| x.is_date_role),
            "date-role dimension should survive the model build"
        );
    }
}
