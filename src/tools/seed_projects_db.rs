/// Regenerate the converted-project DuckDB databases from checked-in sources,
/// so the binary `.db` files stay out of git.
///
/// Usage: cargo run --bin seed_projects_db
///
/// - retail: `schema.sql` + `seed_date_dim.sql` (deterministic, empty fact)
/// - contoso: load the tracked CSVs in `data/contoso/data/` via `read_csv_auto`
use std::path::Path;

pub fn run(_args: Vec<String>) -> i32 {
    seed_retail();
    seed_contoso();
    0
}

fn seed_retail() {
    let db_path = "projects/generated_retail_analytics/data/sales.db";
    reset_db(db_path);
    let db = duckdb::Connection::open(db_path).expect("open retail db");
    for sql_path in [
        "projects/generated_retail_analytics/schema.sql",
        "projects/generated_retail_analytics/seed_date_dim.sql",
    ] {
        execute_sql_file(&db, sql_path);
    }
    eprintln!("Created {db_path}");
}

fn seed_contoso() {
    let db_path = "projects/generated_contoso/data/sales.db";
    reset_db(db_path);
    let db = duckdb::Connection::open(db_path).expect("open contoso db");
    for table in [
        "sales",
        "customer",
        "date",
        "product",
        "store",
        "orders",
        "orderrows",
        "currencyexchange",
    ] {
        let csv_path = format!("data/contoso/data/{table}.csv");
        let sql = format!("CREATE TABLE {table} AS SELECT * FROM read_csv_auto('{csv_path}')");
        db.execute_batch(&sql)
            .unwrap_or_else(|e| panic!("load {csv_path}: {e}"));
    }
    eprintln!("Created {db_path}");
}

fn reset_db(db_path: &str) {
    if let Some(parent) = Path::new(db_path).parent() {
        std::fs::create_dir_all(parent).expect("create db parent dir");
    }
    if Path::new(db_path).exists() {
        std::fs::remove_file(db_path).expect("remove db");
    }
}

fn execute_sql_file(db: &duckdb::Connection, sql_path: &str) {
    let sql = std::fs::read_to_string(sql_path).expect("read sql");
    for stmt in split_sql(&sql) {
        if stmt.trim().is_empty() {
            continue;
        }
        db.execute_batch(stmt).unwrap_or_else(|e| {
            eprintln!(
                "SQL error: {e}\nStatement: {}...",
                &stmt[..stmt.len().min(200)]
            );
            panic!("seed failed for {sql_path}");
        });
    }
}

fn split_sql(sql: &str) -> Vec<&str> {
    let mut stmts = Vec::new();
    let mut start = 0;
    let bytes = sql.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b';' {
            stmts.push(&sql[start..=i]);
            start = i + 1;
        }
        i += 1;
    }
    if start < sql.len() {
        let rest = sql[start..].trim();
        if !rest.is_empty() {
            stmts.push(rest);
        }
    }
    stmts
}
