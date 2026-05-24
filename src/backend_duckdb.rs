use duckdb::{Connection, params};
use std::fmt;
use std::sync::Mutex;
use crate::backend::{QueryBackend, BenchmarkDataConfig, generate_rows};

pub struct DuckDbBackend {
    conn: Mutex<Connection>,
}

impl fmt::Debug for DuckDbBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DuckDbBackend").finish_non_exhaustive()
    }
}

impl DuckDbBackend {
    pub fn new_with_config(config: &BenchmarkDataConfig) -> Result<Self, duckdb::Error> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE faktatabell (
                 produktkategori VARCHAR NOT NULL,
                 region VARCHAR NOT NULL,
                 sales DOUBLE NOT NULL
             );",
        )?;

        let rows = generate_rows(config);
        {
            let mut stmt = conn.prepare(
                "INSERT INTO faktatabell (produktkategori, region, sales) VALUES (?1, ?2, ?3)"
            )?;
            for row in &rows {
                stmt.execute(params![row.produktkategori.as_str(), row.region.as_str(), row.sales])?;
            }
        }

        Ok(DuckDbBackend {
            conn: Mutex::new(conn),
        })
    }
}

impl QueryBackend for DuckDbBackend {
    fn query_scalar(&self, sql: &str) -> f64 {
        let conn = self.conn.lock().unwrap();
        conn.query_row(sql, [], |row| row.get::<_, f64>(0))
            .unwrap_or(0.0)
    }

    fn query_grouped_1d(&self, sql: &str) -> Vec<(String, f64)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(sql).expect("prepare duckdb query_grouped_1d");
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?)))
            .expect("query_map duckdb query_grouped_1d");
        rows.filter_map(|r| r.ok()).collect()
    }

    fn query_pairs(&self, sql: &str) -> Vec<(String, String, f64)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(sql).expect("prepare duckdb query_pairs");
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, f64>(2)?))
            })
            .expect("query_map duckdb query_pairs");
        rows.filter_map(|r| r.ok()).collect()
    }

    fn query_count(&self, sql: &str) -> u32 {
        let conn = self.conn.lock().unwrap();
        conn.query_row(sql, [], |row| row.get::<_, u32>(0))
            .unwrap_or(0)
    }
}
