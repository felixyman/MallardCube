use duckdb::{Connection, params};
use std::path::Path;
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

// ---- wider demo data (project3) ----

pub struct SalesFactRow {
    pub category: String,
    pub territory: String,
    pub channel: String,
    pub segment: String,
    pub revenue: f64,
    pub units: f64,
    pub date_key: i32,
}

pub fn generate_sales_fact_rows() -> Vec<SalesFactRow> {
    let categories: Vec<&str> = (1..=20).map(|i| {
        match i {
            1 => "Electronics", 2 => "Clothing", 3 => "Food",
            4 => "Furniture", 5 => "Sports", 6 => "Books",
            7 => "Toys", 8 => "Automotive", 9 => "Health",
            10 => "Music", 11 => "Garden", 12 => "Office",
            13 => "Pet Supplies", 14 => "Jewelry", 15 => "Home",
            16 => "Baby", 17 => "Tools", 18 => "Beauty",
            19 => "Shoes", 20 => "Outdoors",
            _ => "Other",
        }
    }).collect();
    let territories: &[&str] = &[
        "North", "South", "East", "West", "Central",
        "Northeast", "Southeast", "Northwest",
    ];
    let channels: &[&str] = &["Online", "Retail", "Wholesale", "Direct"];
    let segments: &[&str] = &["Consumer", "Business", "Government", "Education", "Non-Profit"];

    let mut rng = SeededRng::new(99);
    let row_count = 20_000;
    let mut rows = Vec::with_capacity(row_count);
    for _ in 0..row_count {
        let cat = categories[rng.next() as usize % categories.len()];
        let ter = territories[rng.next() as usize % territories.len()];
        let ch = channels[rng.next() as usize % channels.len()];
        let seg = segments[rng.next() as usize % segments.len()];
        let revenue = 1_000.0 + (rng.next() as f64 % 50_000.0);
        let units = (rng.next() as f64 % 500.0).round();
        let year = 2020 + (rng.next() % 11) as i32;
        let month = (rng.next() % 12 + 1) as i32;
        let day = (rng.next() % 28 + 1) as i32;
        let date_key = year * 10000 + month * 100 + day;
        rows.push(SalesFactRow {
            category: cat.to_string(),
            territory: ter.to_string(),
            channel: ch.to_string(),
            segment: seg.to_string(),
            revenue,
            units,
            date_key,
        });
    }
    rows
}

// ---- inventory demo data (project4) ----

pub struct InventoryFactRow {
    pub category: String,
    pub territory: String,
    pub warehouse: String,
    pub stock_qty: f64,
    pub stock_cost: f64,
}

pub fn generate_inventory_fact_rows() -> Vec<InventoryFactRow> {
    let categories: Vec<&str> = (1..=20).map(|i| {
        match i {
            1 => "Electronics", 2 => "Clothing", 3 => "Food",
            4 => "Furniture", 5 => "Sports", 6 => "Books",
            7 => "Toys", 8 => "Automotive", 9 => "Health",
            10 => "Music", 11 => "Garden", 12 => "Office",
            13 => "Pet Supplies", 14 => "Jewelry", 15 => "Home",
            16 => "Baby", 17 => "Tools", 18 => "Beauty",
            19 => "Shoes", 20 => "Outdoors",
            _ => "Other",
        }
    }).collect();
    let territories: &[&str] = &[
        "North", "South", "East", "West", "Central",
        "Northeast", "Southeast", "Northwest",
    ];
    let warehouses: &[&str] = &["WH-1", "WH-2", "WH-3", "WH-4", "WH-5", "WH-6"];

    let mut rng = SeededRng::new(77);
    let row_count = 10_000;
    let mut rows = Vec::with_capacity(row_count);
    for _ in 0..row_count {
        let cat = categories[rng.next() as usize % categories.len()];
        let ter = territories[rng.next() as usize % territories.len()];
        let wh = warehouses[rng.next() as usize % warehouses.len()];
        let qty = 100.0 + (rng.next() as f64 % 10_000.0);
        let cost = qty * (5.0 + (rng.next() as f64 % 45.0));
        rows.push(InventoryFactRow {
            category: cat.to_string(),
            territory: ter.to_string(),
            warehouse: wh.to_string(),
            stock_qty: qty.round(),
            stock_cost: cost.round(),
        });
    }
    rows
}

static BACKEND: OnceLock<Backend> = OnceLock::new();

fn instance() -> &'static Backend {
    BACKEND.get_or_init(|| Backend::new().expect("failed to initialise DuckDB"))
}

/// Called once at startup, before any queries. When `db_path` is `Some`,
/// opens the file-based DuckDB database (user-owned schema — no seeding).
/// When `None`, uses the demo in-memory database with synthetic data.
pub fn init_backend(db_path: Option<&str>) -> Result<(), duckdb::Error> {
    let backend = match db_path {
        Some(path) => Backend::open(Path::new(path))?,
        None => Backend::new()?,
    };
    BACKEND.set(backend).map_err(|_| {
        duckdb::Error::InvalidParameterName("Backend already initialised".into())
    })?;
    Ok(())
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

    /// Called once at startup. When `db_path` is `Some`, opens the file-based
    /// DuckDB database. When `None`, uses the demo in-memory database.
    pub fn init(db_path: Option<&str>) -> Result<(), duckdb::Error> {
        let backend = match db_path {
            Some(path) => Backend::open(Path::new(path))?,
            None => Backend::new()?,
        };
        static BACKEND: OnceLock<Backend> = OnceLock::new();
        BACKEND.set(backend).map_err(|_| {
            duckdb::Error::InvalidParameterName("Backend already initialised".into())
        }).ok();
        Ok(())
    }

    /// Open a file-based DuckDB database. No seeding — the user owns the schema.
    pub fn open(path: &Path) -> Result<Self, duckdb::Error> {
        let conn = Connection::open(path)?;
        Ok(Backend {
            conn: Mutex::new(conn),
        })
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
        conn.execute_batch(
            "CREATE TABLE sales_fact (
                 category   VARCHAR NOT NULL,
                 territory  VARCHAR NOT NULL,
                 channel    VARCHAR NOT NULL,
                 segment    VARCHAR NOT NULL,
                 revenue    DOUBLE NOT NULL,
                 units      DOUBLE NOT NULL,
                 date_key   INTEGER NOT NULL
             );",
        )?;
        let wider_rows = generate_sales_fact_rows();
        {
            let mut app = conn.appender("sales_fact")?;
            for r in &wider_rows {
                app.append_row(params![
                    r.category.as_str(),
                    r.territory.as_str(),
                    r.channel.as_str(),
                    r.segment.as_str(),
                    r.revenue,
                    r.units,
                    r.date_key,
                ]);
            }
            let _ = app.flush();
        }
        // Seed date_dim calendar so YTD measures resolve.
        let date_dim_sql = include_str!("../../data/seed_date_dim.sql");
        conn.execute_batch(date_dim_sql)?;
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

    pub fn execute_ddl(&self, sql: &str) {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(sql).expect("execute_ddl");
    }

    // ---- metadata helpers (used by members.rs) ----

    pub fn distinct_count(&self, column: &str) -> u32 {
        self.distinct_count_in("faktatabell", column)
    }

    pub fn distinct_values(&self, column: &str) -> Vec<String> {
        self.distinct_values_in("faktatabell", column)
    }

    pub fn distinct_count_in(&self, table: &str, column: &str) -> u32 {
        let sql = format!("SELECT COUNT(DISTINCT {column}) FROM {table}");
        self.query_count(&sql)
    }

    pub fn distinct_values_in(&self, table: &str, column: &str) -> Vec<String> {
        let sql = format!("SELECT DISTINCT {column} FROM {table} ORDER BY {column}");
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&sql).expect("prepare distinct_values");
        let rows: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query_map distinct_values")
            .filter_map(|r| r.ok())
            .collect();
        rows
    }
}

#[cfg(test)]
mod tests {
    use crate::backend::Backend;

    #[test]
    fn date_dim_seed_has_all_period_flags() {
        let db = Backend::new().expect("create in-memory backend");
        let sql = include_str!("../../data/seed_date_dim.sql");
        db.execute_ddl(sql);
        let total = db.query_count("SELECT COUNT(*) FROM date_dim");
        assert!(total >= 4000, "should generate 11 years of dates");

        for (flag, max) in [
            ("ytd_flag", 366),
            ("current_year_flag", 365),
            ("qtd_flag", 92),
            ("mtd_flag", 31),
            ("prior_year_ytd_flag", 366),
        ] {
            let count = db.query_count(
                &format!("SELECT COUNT(*) FROM date_dim WHERE {flag} = true"),
            );
            assert!(count > 0, "should have at least one {flag} = true row today");
            assert!(count <= max, "{flag} should not exceed {max} rows");
        }
    }
}
