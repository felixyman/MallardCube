/// Regenerate the converted-project DuckDB databases from checked-in sources,
/// so the binary `.db` files stay out of git.
///
/// Usage: cargo run --bin seed_projects_db
///
/// - retail: `schema.sql` + `seed_date_dim.sql` (deterministic, empty fact)
/// - contoso: load the tracked CSVs in `data/contoso/data/` via `read_csv_auto`
///
/// Tests and CI jobs that read the generated projects call [`ensure_seeded`],
/// which builds only what is missing; `run` is the explicit regeneration.
use std::path::Path;

const RETAIL_DB: &str = "projects/generated_retail_analytics/data/sales.db";
const CONTOSO_DB: &str = "projects/generated_contoso/data/sales.db";

pub fn run(_args: Vec<String>) -> i32 {
    seed_retail();
    seed_contoso();
    0
}

/// Seed any generated-project database that is missing or empty.
///
/// The `.db` files are deliberately not in git and the tracked inputs are
/// deterministic, so a clean checkout can rebuild them on demand. Callers that
/// read the converted projects (the test suite, the CI test jobs) use this so
/// no manual setup step is required; the work happens once per process.
pub fn ensure_seeded() {
    static SEEDED: std::sync::Once = std::sync::Once::new();
    SEEDED.call_once(|| {
        if !is_seeded(RETAIL_DB) {
            seed_retail();
        }
        if !is_seeded(CONTOSO_DB) {
            seed_contoso();
        }
    });
}

/// A database file only counts as seeded when it actually holds tables: a
/// plain `Connection::open` creates the file, so an empty one (left by some
/// earlier run that opened it before seeding) must not stop the fixture from
/// being rebuilt. A zero-byte placeholder is never a database, so it is
/// rebuilt too; other unreadable files are left alone — the caller fails
/// loudly rather than deleting a database it cannot inspect.
fn is_seeded(db_path: &str) -> bool {
    let Ok(metadata) = std::fs::metadata(db_path) else {
        return false;
    };
    if metadata.len() == 0 {
        return false;
    }
    let Ok(conn) = duckdb::Connection::open(db_path) else {
        return true;
    };
    conn.query_row(
        "SELECT count(*) FROM information_schema.tables",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|tables| tables > 0)
    .unwrap_or(true)
}

fn seed_retail() {
    reset_db(RETAIL_DB);
    let db = duckdb::Connection::open(RETAIL_DB).expect("open retail db");
    for sql_path in [
        "projects/generated_retail_analytics/schema.sql",
        "projects/generated_retail_analytics/seed_date_dim.sql",
    ] {
        execute_sql_file(&db, sql_path);
    }
    eprintln!("Created {RETAIL_DB}");
}

fn seed_contoso() {
    reset_db(CONTOSO_DB);
    let db = duckdb::Connection::open(CONTOSO_DB).expect("open contoso db");
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
    eprintln!("Created {CONTOSO_DB}");
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
