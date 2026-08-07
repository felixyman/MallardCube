/// Create a synthetic DuckDB database for generated_project smoke testing.
///
/// Usage: cargo run --bin seed_generated_db
///
/// Creates data/generated.db from data/seed_generated.sql.

use std::path::Path;

pub fn run(_args: Vec<String>) -> i32 {
    let db_path = "data/generated.db";
    let sql_path = "data/seed_generated.sql";

    if Path::new(db_path).exists() {
        eprintln!("Removing existing {db_path}");
        std::fs::remove_file(db_path).expect("remove db");
    }

    let db = duckdb::Connection::open(db_path).expect("open db");

    let sql = std::fs::read_to_string(sql_path).expect("read seed sql");

    // Execute each statement separately
    for stmt in split_sql(&sql) {
        if stmt.trim().is_empty() {
            continue;
        }
        db.execute_batch(stmt).unwrap_or_else(|e| {
            eprintln!("SQL error: {e}\nStatement: {}...", &stmt[..stmt.len().min(200)]);
            panic!("seed failed");
        });
    }

    // Verify
    let count: i64 = db.query_row(
        "SELECT COUNT(*) FROM dw_fys_f_undersökning", [], |r| r.get(0)
    ).unwrap();
    let dims: i64 = db.query_row(
        "SELECT COUNT(*) FROM dw_fys_d_produkt", [], |r| r.get(0)
    ).unwrap();

    eprintln!("Created {db_path}: {count} fact rows, {dims} product dims");
    0
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
