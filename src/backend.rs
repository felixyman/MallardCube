use duckdb::{Connection, params};
use std::sync::{Mutex, OnceLock};

pub struct Backend {
    conn: Mutex<Connection>,
}

// ---- backend trait ----

pub trait QueryBackend {
    fn query_scalar(&self, sql: &str) -> f64;
    fn query_grouped_1d(&self, sql: &str) -> Vec<(String, f64)>;
    fn query_pairs(&self, sql: &str) -> Vec<(String, String, f64)>;
    fn query_count(&self, sql: &str) -> u32;
}

// ---- benchmark config ----

#[derive(Debug, Clone)]
pub struct BenchmarkDataConfig {
    pub row_count: usize,
    pub category_count: usize,
    pub region_count: usize,
    pub seed: u64,
}

impl Default for BenchmarkDataConfig {
    fn default() -> Self {
        Self {
            row_count: 10_000,
            category_count: 20,
            region_count: 8,
            seed: 42,
        }
    }
}

impl BenchmarkDataConfig {
    pub fn tiny() -> Self {
        Self { row_count: 10, category_count: 4, region_count: 2, seed: 1 }
    }
    pub fn small() -> Self {
        Self { row_count: 10_000, category_count: 20, region_count: 8, seed: 42 }
    }
    pub fn medium() -> Self {
        Self { row_count: 100_000, category_count: 100, region_count: 16, seed: 43 }
    }
    pub fn large() -> Self {
        Self { row_count: 1_000_000, category_count: 500, region_count: 32, seed: 44 }
    }
}

// ---- deterministic pseudo-random for benchmark data ----

struct SeededRng {
    state: u64,
}

impl SeededRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }
}

// ---- shared benchmark data ----

pub struct FactRow {
    pub produktkategori: String,
    pub region: String,
    pub sales: f64,
}

pub fn generate_rows(config: &BenchmarkDataConfig) -> Vec<FactRow> {
    let mut rng = SeededRng::new(config.seed);
    let categories: Vec<String> =
        (1..=config.category_count).map(|i| format!("Kategori {:03}", i)).collect();
    let regions: Vec<String> =
        (1..=config.region_count).map(|i| format!("Region {:02}", i)).collect();

    let mut rows = Vec::with_capacity(config.row_count + 1);
    for _ in 0..config.row_count {
        let kat = &categories[rng.next() as usize % config.category_count];
        let reg = &regions[rng.next() as usize % config.region_count];
        let sales = 10_000.0 + (rng.next() as f64 % 100_000.0);
        rows.push(FactRow {
            produktkategori: kat.clone(),
            region: reg.clone(),
            sales,
        });
    }
    rows.push(FactRow {
        produktkategori: "Kategori SKEW".into(),
        region: "Region SKEW".into(),
        sales: 9_999_999.0,
    });
    rows
}

fn instance() -> &'static Backend {
    static BACKEND: OnceLock<Backend> = OnceLock::new();
    BACKEND.get_or_init(|| Backend::new().expect("failed to initialise DuckDB"))
}

// ---- DuckDB Backend impl of QueryBackend ----

impl QueryBackend for Backend {
    fn query_scalar(&self, sql: &str) -> f64 {
        Backend::query_scalar(self, sql)
    }

    fn query_grouped_1d(&self, sql: &str) -> Vec<(String, f64)> {
        Backend::query_grouped_1d(self, sql)
    }

    fn query_pairs(&self, sql: &str) -> Vec<(String, String, f64)> {
        Backend::query_pairs(self, sql)
    }

    fn query_count(&self, sql: &str) -> u32 {
        Backend::query_count(self, sql)
    }
}

impl Backend {
    pub fn get() -> &'static Self {
        instance()
    }

    pub fn new() -> Result<Self, duckdb::Error> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE faktatabell (
                 produktkategori VARCHAR NOT NULL,
                 region VARCHAR NOT NULL,
                 sales DOUBLE NOT NULL
             );
             INSERT INTO faktatabell VALUES
                 ('Kategori A', 'North', 100000.0),
                 ('Kategori A', 'South', 200000.0),
                 ('Kategori B', 'North', 150000.0),
                 ('Kategori B', 'South', 100000.0),
                 ('Kategori C', 'North', 200000.0),
                 ('Kategori C', 'South', 200000.0),
                 ('Kategori D', 'North', 200000.0),
                 ('Kategori D', 'South', 100500.5);
             ",
        )?;
        Ok(Backend {
            conn: Mutex::new(conn),
        })
    }

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
            let mut app = conn.appender("faktatabell")?;
            for row in &rows {
                app.append_row(params![
                    row.produktkategori.as_str(),
                    row.region.as_str(),
                    row.sales,
                ]);
            }
            app.flush();
        }

        Ok(Backend {
            conn: Mutex::new(conn),
        })
    }

    pub fn total_sales(&self) -> f64 {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COALESCE(SUM(sales), 0) FROM faktatabell",
            [],
            |row| row.get::<_, f64>(0),
        )
        .unwrap_or(0.0)
    }

    pub fn total_sales_for(&self, category: &str) -> f64 {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COALESCE(SUM(sales), 0) FROM faktatabell WHERE produktkategori = ?1",
            params![category],
            |row| row.get::<_, f64>(0),
        )
        .unwrap_or(0.0)
    }

    pub fn total_sales_for_region(&self, region: &str) -> f64 {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COALESCE(SUM(sales), 0) FROM faktatabell WHERE region = ?1",
            params![region],
            |row| row.get::<_, f64>(0),
        )
        .unwrap_or(0.0)
    }

    pub fn category_count(&self) -> u32 {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(DISTINCT produktkategori) FROM faktatabell",
            [],
            |row| row.get::<_, u32>(0),
        )
        .unwrap_or(0)
    }

    pub fn region_count(&self) -> u32 {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(DISTINCT region) FROM faktatabell",
            [],
            |row| row.get::<_, u32>(0),
        )
        .unwrap_or(0)
    }

    // ---- generic SQL execution (used by engine/plan via sql.rs) ----

    pub fn query_scalar(&self, sql: &str) -> f64 {
        let conn = self.conn.lock().unwrap();
        conn.query_row(sql, [], |row| row.get::<_, f64>(0))
            .unwrap_or(0.0)
    }

    pub fn query_grouped_1d(&self, sql: &str) -> Vec<(String, f64)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(sql).expect("prepare query_grouped_1d");
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?)))
            .expect("query_map query_grouped_1d");
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn query_pairs(&self, sql: &str) -> Vec<(String, String, f64)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(sql).expect("prepare query_pairs");
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, f64>(2)?))
            })
            .expect("query_map query_pairs");
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn query_count(&self, sql: &str) -> u32 {
        let conn = self.conn.lock().unwrap();
        conn.query_row(sql, [], |row| row.get::<_, u32>(0))
            .unwrap_or(0)
    }
}
