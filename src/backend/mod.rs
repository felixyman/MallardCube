use duckdb::{AccessMode, Config, Connection, params};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

pub struct Backend {
    conn: Mutex<Connection>,
}

/// A fixed set of pre-opened, read-only DuckDB connections shared across
/// requests. Opening a connection per request (the previous behaviour) pays the
/// catalog-load cost on every request and, worse, serializes on DuckDB's file
/// lock when several read-write connections open the same file concurrently.
pub struct BackendPool {
    backends: Arc<[Arc<Backend>]>,
    next: AtomicUsize,
}

impl std::fmt::Debug for BackendPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BackendPool(len={})", self.backends.len())
    }
}

impl Clone for BackendPool {
    fn clone(&self) -> Self {
        BackendPool {
            backends: self.backends.clone(),
            next: AtomicUsize::new(0),
        }
    }
}

#[derive(Debug, Clone)]
pub enum BackendSource {
    File { path: PathBuf, pool: BackendPool },
    Demo { path: PathBuf, pool: BackendPool },
}

static DEMO_DB_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Number of pooled connections. Overridable via `MALLARDCUBE_POOL_SIZE`; the
/// proxy is read-only, so a handful of connections saturates most workloads.
fn pool_size() -> usize {
    std::env::var("MALLARDCUBE_POOL_SIZE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n >= 1)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .clamp(1, 32)
        })
}

fn open_read_only(path: &Path) -> Result<Connection, duckdb::Error> {
    let config = Config::default().access_mode(AccessMode::ReadOnly)?;
    Connection::open_with_flags(path, config)
}

// ---- backend trait ----

pub trait QueryBackend {
    fn query_scalar(&self, sql: &str) -> f64;
    fn query_grouped_1d(&self, sql: &str) -> Vec<(String, f64)>;
    fn query_pairs(&self, sql: &str) -> Vec<(String, String, f64)>;
    fn query_count(&self, sql: &str) -> u32;
    fn query_strings(&self, sql: &str) -> Vec<String>;
    fn query_rows(&self, sql: &str) -> Vec<Vec<String>>;
    fn query_column_names(&self, sql: &str) -> Vec<String>;
}

impl BackendSource {
    pub fn file(path: impl Into<PathBuf>) -> Result<Self, duckdb::Error> {
        let path = path.into();
        let pool = BackendPool::open(&path)?;
        Ok(Self::File { path, pool })
    }

    pub fn demo() -> Result<Self, duckdb::Error> {
        let path = std::env::temp_dir().join(format!(
            "mallardcube-demo-{}-{}.duckdb",
            std::process::id(),
            DEMO_DB_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| {
                duckdb::Error::InvalidParameterName(format!(
                    "failed to remove stale demo DuckDB {}: {e}",
                    path.display()
                ))
            })?;
        }
        Backend::create_demo_file(&path)?;
        let pool = BackendPool::open(&path)?;
        Ok(Self::Demo { path, pool })
    }

    /// Check out a pooled connection (round-robin). The pool is pre-opened, so
    /// this never fails; concurrent requests are spread across connections,
    /// each serialized by the per-connection mutex.
    pub fn checkout(&self) -> Arc<Backend> {
        match self {
            Self::File { pool, .. } | Self::Demo { pool, .. } => pool.checkout(),
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            Self::File { path, .. } | Self::Demo { path, .. } => path,
        }
    }
}

impl BackendPool {
    fn open(path: &Path) -> Result<Self, duckdb::Error> {
        let size = pool_size();
        let mut backends = Vec::with_capacity(size);
        for _ in 0..size {
            let conn = open_read_only(path)?;
            // Aggregation sidecar: attached read-only so rollup queries share the
            // pooled connection. Only attach when routing was actually enabled
            // (build succeeded and MALLARDCUBE_AGG_CACHE is set); a failed build
            // must degrade to the fact path, not fail the pool.
            if !crate::engine::aggregate::aggregations().is_empty()
                && let Ok(agg) = std::env::var("MALLARDCUBE_AGG_CACHE")
            {
                conn.execute_batch(&format!(
                    "ATTACH '{agg}' AS {} (READ_ONLY);",
                    crate::engine::aggregate::AGG_ALIAS
                ))?;
            }
            backends.push(Arc::new(Backend {
                conn: Mutex::new(conn),
            }));
        }
        Ok(BackendPool {
            backends: backends.into(),
            next: AtomicUsize::new(0),
        })
    }

    fn checkout(&self) -> Arc<Backend> {
        let i = self.next.fetch_add(1, Ordering::Relaxed) % self.backends.len();
        self.backends[i].clone()
    }
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
        Self {
            row_count: 10,
            category_count: 4,
            region_count: 2,
            seed: 1,
        }
    }
    pub fn small() -> Self {
        Self {
            row_count: 10_000,
            category_count: 20,
            region_count: 8,
            seed: 42,
        }
    }
    pub fn medium() -> Self {
        Self {
            row_count: 100_000,
            category_count: 100,
            region_count: 16,
            seed: 43,
        }
    }
    pub fn large() -> Self {
        Self {
            row_count: 1_000_000,
            category_count: 500,
            region_count: 32,
            seed: 44,
        }
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
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }
}

// ---- shared benchmark data ----

pub struct FactRow {
    pub produktkategori: String,
    pub region: String,
    pub order_datum: String,
    pub date_key: i32,
    pub sales: f64,
}

pub fn generate_rows(config: &BenchmarkDataConfig) -> Vec<FactRow> {
    let mut rng = SeededRng::new(config.seed);
    let categories: Vec<String> = (1..=config.category_count)
        .map(|i| format!("Kategori {:03}", i))
        .collect();
    let regions: Vec<String> = (1..=config.region_count)
        .map(|i| format!("Region {:02}", i))
        .collect();

    let mut rows = Vec::with_capacity(config.row_count + 1);
    for i in 0..config.row_count {
        let kat = &categories[rng.next() as usize % config.category_count];
        let reg = &regions[rng.next() as usize % config.region_count];
        let sales = 10_000.0 + (rng.next() as f64 % 100_000.0);
        let day_offset = (i % (365 * 6)) as i64;
        // Generate a date string 2020-MM-DD within 2020-2026 range
        let months = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let total_days = day_offset;
        let mut remaining = total_days;
        let mut y = 2020;
        loop {
            let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
                366
            } else {
                365
            };
            if remaining < days_in_year {
                break;
            }
            remaining -= days_in_year;
            y += 1;
        }
        let mut mo = 0;
        let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
        while mo < 12 && remaining >= months[mo] + if mo == 1 && leap { 1 } else { 0 } {
            remaining -= months[mo] + if mo == 1 && leap { 1 } else { 0 };
            mo += 1;
        }
        let day = remaining + 1;
        let date = format!("{y:04}-{:02}-{:02}", mo + 1, day);
        let date_key_val = y * 10000 + (mo as i32 + 1) * 100 + day as i32;
        rows.push(FactRow {
            produktkategori: kat.clone(),
            region: reg.clone(),
            order_datum: date,
            date_key: date_key_val,
            sales,
        });
    }
    rows.push(FactRow {
        produktkategori: "Kategori SKEW".into(),
        region: "Region SKEW".into(),
        order_datum: "2020-01-01".into(),
        date_key: 20200101,
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
    let categories: Vec<&str> = (1..=20)
        .map(|i| match i {
            1 => "Electronics",
            2 => "Clothing",
            3 => "Food",
            4 => "Furniture",
            5 => "Sports",
            6 => "Books",
            7 => "Toys",
            8 => "Automotive",
            9 => "Health",
            10 => "Music",
            11 => "Garden",
            12 => "Office",
            13 => "Pet Supplies",
            14 => "Jewelry",
            15 => "Home",
            16 => "Baby",
            17 => "Tools",
            18 => "Beauty",
            19 => "Shoes",
            20 => "Outdoors",
            _ => "Other",
        })
        .collect();
    let territories: &[&str] = &[
        "North",
        "South",
        "East",
        "West",
        "Central",
        "Northeast",
        "Southeast",
        "Northwest",
    ];
    let channels: &[&str] = &["Online", "Retail", "Wholesale", "Direct"];
    let segments: &[&str] = &[
        "Consumer",
        "Business",
        "Government",
        "Education",
        "Non-Profit",
    ];

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
    let categories: Vec<&str> = (1..=20)
        .map(|i| match i {
            1 => "Electronics",
            2 => "Clothing",
            3 => "Food",
            4 => "Furniture",
            5 => "Sports",
            6 => "Books",
            7 => "Toys",
            8 => "Automotive",
            9 => "Health",
            10 => "Music",
            11 => "Garden",
            12 => "Office",
            13 => "Pet Supplies",
            14 => "Jewelry",
            15 => "Home",
            16 => "Baby",
            17 => "Tools",
            18 => "Beauty",
            19 => "Shoes",
            20 => "Outdoors",
            _ => "Other",
        })
        .collect();
    let territories: &[&str] = &[
        "North",
        "South",
        "East",
        "West",
        "Central",
        "Northeast",
        "Southeast",
        "Northwest",
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
    BACKEND
        .set(backend)
        .map_err(|_| duckdb::Error::InvalidParameterName("Backend already initialised".into()))?;
    Ok(())
}

pub fn init_backend_source(db_path: Option<&str>) -> Result<BackendSource, duckdb::Error> {
    match db_path {
        Some(path) => BackendSource::file(path),
        None => BackendSource::demo(),
    }
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

    fn query_strings(&self, sql: &str) -> Vec<String> {
        Backend::query_strings(self, sql)
    }

    fn query_rows(&self, sql: &str) -> Vec<Vec<String>> {
        Backend::query_rows(self, sql)
    }

    fn query_column_names(&self, sql: &str) -> Vec<String> {
        Backend::query_column_names(self, sql)
    }
}

/// Convert a DuckDB value to f64 for measure/scalar reads, preserving decimal
/// fractions. The naive `row.get::<_, f64>()` rounds a DECIMAL to an integer
/// (dropping the fraction), which silently corrupted any measure over a decimal
/// column. Returns None for NULL or non-numeric values.
fn value_to_f64(v: &duckdb::types::Value) -> Option<f64> {
    use duckdb::types::Value;
    Some(match v {
        Value::Null => return None,
        Value::Boolean(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Value::TinyInt(i) => *i as f64,
        Value::SmallInt(i) => *i as f64,
        Value::Int(i) => *i as f64,
        Value::BigInt(i) => *i as f64,
        Value::HugeInt(i) => *i as f64,
        Value::UTinyInt(i) => *i as f64,
        Value::USmallInt(i) => *i as f64,
        Value::UInt(i) => *i as f64,
        Value::UBigInt(i) => *i as f64,
        Value::Float(f) => *f as f64,
        Value::Double(f) => *f,
        Value::Decimal(d) => d.mantissa() as f64 / 10f64.powi(d.scale() as i32),
        _ => return None,
    })
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
        BACKEND
            .set(backend)
            .map_err(|_| duckdb::Error::InvalidParameterName("Backend already initialised".into()))
            .ok();
        Ok(())
    }

    /// Open a file-based DuckDB database. No seeding — the user owns the schema.
    pub fn open(path: &Path) -> Result<Self, duckdb::Error> {
        let conn = Connection::open(path)?;
        Ok(Backend {
            conn: Mutex::new(conn),
        })
    }

    pub fn create_demo_file(path: &Path) -> Result<Self, duckdb::Error> {
        let conn = Connection::open(path)?;
        Self::seed_demo_connection(&conn)?;
        Ok(Backend {
            conn: Mutex::new(conn),
        })
    }

    pub fn new() -> Result<Self, duckdb::Error> {
        let conn = Connection::open_in_memory()?;
        Self::seed_demo_connection(&conn)?;
        Ok(Backend {
            conn: Mutex::new(conn),
        })
    }

    fn seed_demo_connection(conn: &Connection) -> Result<(), duckdb::Error> {
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
                ])?;
            }
            app.flush()?;
        }
        // Seed date_dim calendar so YTD measures resolve.
        let date_dim_sql = include_str!("../../data/seed_date_dim.sql");
        conn.execute_batch(date_dim_sql)?;
        Ok(())
    }

    pub fn new_with_config(config: &BenchmarkDataConfig) -> Result<Self, duckdb::Error> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE faktatabell (
                  produktkategori VARCHAR NOT NULL,
                  region VARCHAR NOT NULL,
                  order_datum DATE NOT NULL,
                  date_key INTEGER NOT NULL,
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
                    row.order_datum.as_str(),
                    row.date_key,
                    row.sales,
                ])?;
            }
            app.flush()?;
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
        conn.query_row(sql, [], |row| {
            Ok(value_to_f64(&row.get::<_, duckdb::types::Value>(0)?).unwrap_or(0.0))
        })
        .unwrap_or(0.0)
    }

    pub fn query_grouped_1d(&self, sql: &str) -> Vec<(String, f64)> {
        let conn = self.conn.lock().unwrap();
        let Ok(mut stmt) = conn.prepare(sql) else {
            eprintln!("query_grouped_1d: prepare failed: {sql}");
            return Vec::new();
        };
        let rows = match stmt.query_map([], |row| {
            let label = row.get::<_, String>(0)?;
            let value = value_to_f64(&row.get::<_, duckdb::types::Value>(1)?).unwrap_or(0.0);
            Ok((label, value))
        }) {
            Ok(rows) => rows,
            Err(e) => {
                eprintln!("query_grouped_1d: query failed: {e}");
                return Vec::new();
            }
        };
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn query_pairs(&self, sql: &str) -> Vec<(String, String, f64)> {
        let conn = self.conn.lock().unwrap();
        let Ok(mut stmt) = conn.prepare(sql) else {
            eprintln!("query_pairs: prepare failed: {sql}");
            return Vec::new();
        };
        let rows = match stmt.query_map([], |row| {
            let a = row.get::<_, String>(0)?;
            let b = row.get::<_, String>(1)?;
            let value = value_to_f64(&row.get::<_, duckdb::types::Value>(2)?).unwrap_or(0.0);
            Ok((a, b, value))
        }) {
            Ok(rows) => rows,
            Err(e) => {
                eprintln!("query_pairs: query failed: {e}");
                return Vec::new();
            }
        };
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn query_count(&self, sql: &str) -> u32 {
        let conn = self.conn.lock().unwrap();
        conn.query_row(sql, [], |row| row.get::<_, u32>(0))
            .unwrap_or(0)
    }

    pub fn query_strings(&self, sql: &str) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        let Ok(mut stmt) = conn.prepare(sql) else {
            eprintln!("query_strings: prepare failed: {sql}");
            return Vec::new();
        };
        let rows = match stmt.query_map([], |row| row.get::<_, String>(0)) {
            Ok(rows) => rows,
            Err(e) => {
                eprintln!("query_strings: query failed: {e}");
                return Vec::new();
            }
        };
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn query_rows(&self, sql: &str) -> Vec<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        // Get column count via pragma (avoids DuckDB's unexecuted-statement requirement)
        let upper = sql.to_uppercase();
        let from_pos = upper.find("FROM ").unwrap_or(0);
        let after_from = &sql[from_pos + 5..].trim();
        let table = after_from.split_whitespace().next().unwrap_or("?");
        let pragma = format!("SELECT count(*) FROM pragma_table_info('{table}')");
        let col_count: usize = conn.query_row(&pragma, [], |r| r.get(0)).unwrap_or(0);
        let Ok(mut stmt) = conn.prepare(sql) else {
            eprintln!("query_rows: prepare failed: {sql}");
            return Vec::new();
        };
        if col_count > 0 {
            let rows = match stmt.query_map([], move |row| {
                let mut cols = Vec::with_capacity(col_count);
                for i in 0..col_count {
                    cols.push(
                        row.get::<_, duckdb::types::Value>(i)
                            .map(val_to_string)
                            .unwrap_or_default(),
                    );
                }
                Ok(cols)
            }) {
                Ok(rows) => rows,
                Err(e) => {
                    eprintln!("query_rows: query failed: {e}");
                    return Vec::new();
                }
            };
            rows.filter_map(|r| r.ok()).collect()
        } else {
            vec![]
        }
    }

    pub fn query_column_names(&self, sql: &str) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        let upper = sql.to_uppercase();
        let from_pos = upper.find("FROM ").unwrap_or(0);
        let after_from = &sql[from_pos + 5..].trim();
        let table = after_from.split_whitespace().next().unwrap_or("?");
        let pragma = format!("SELECT name FROM pragma_table_info('{table}') ORDER BY cid");
        let Ok(mut stmt) = conn.prepare(&pragma) else {
            eprintln!("query_column_names: prepare failed: {pragma}");
            return Vec::new();
        };
        let rows = match stmt.query_map([], |r| r.get::<_, String>(0)) {
            Ok(rows) => rows,
            Err(e) => {
                eprintln!("query_column_names: query failed: {e}");
                return Vec::new();
            }
        };
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn execute_ddl(&self, sql: &str) {
        let conn = self.conn.lock().unwrap();
        if let Err(e) = conn.execute_batch(sql) {
            eprintln!("execute_ddl failed: {e}");
        }
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

/// Convert a DuckDB Value enum variant to a plain string.
pub(crate) fn val_to_string(v: duckdb::types::Value) -> String {
    match v {
        duckdb::types::Value::Null => String::new(),
        duckdb::types::Value::Boolean(b) => b.to_string(),
        duckdb::types::Value::TinyInt(i) => i.to_string(),
        duckdb::types::Value::SmallInt(i) => i.to_string(),
        duckdb::types::Value::Int(i) => i.to_string(),
        duckdb::types::Value::BigInt(i) => i.to_string(),
        duckdb::types::Value::Float(f) => f.to_string(),
        duckdb::types::Value::Double(f) => f.to_string(),
        duckdb::types::Value::Text(s) => s,
        _ => format!("{v:?}"),
    }
}

#[cfg(test)]
mod tests {
    use crate::backend::{Backend, BackendSource};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_DB_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_db_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "mallardcube-{name}-{}-{}.duckdb",
            std::process::id(),
            TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn query_scalar_and_grouped_read_integer_and_decimal_columns() {
        let path = temp_db_path("numeric-coercion");
        let _ = std::fs::remove_file(&path);
        {
            let conn = duckdb::Connection::open(&path).expect("open temp db");
            conn.execute_batch(
                "CREATE TABLE t (i INTEGER, b BIGINT, d DECIMAL(10,2));
                 INSERT INTO t VALUES (5, 7000000000, 12.34), (7, 8000000000, 5.67);",
            )
            .expect("seed");
        }
        let backend = Backend::open(&path).expect("open backend");

        // Integer / BIGINT SUM columns are HUGEINT; must coerce, not zero out.
        assert_eq!(backend.query_scalar("SELECT SUM(i) FROM t"), 12.0);
        assert_eq!(
            backend.query_scalar("SELECT SUM(b) FROM t"),
            15_000_000_000.0
        );

        // DECIMAL SUM must keep its fraction (18.01), not round to 18.0.
        let dec = backend.query_scalar("SELECT SUM(d) FROM t");
        assert!((dec - 18.01).abs() < 1e-9, "decimal fraction lost: {dec}");

        let grouped = backend.query_grouped_1d("SELECT 'x', SUM(d) FROM t GROUP BY 1");
        assert_eq!(grouped.len(), 1);
        assert!((grouped[0].1 - 18.01).abs() < 1e-9);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn query_methods_do_not_panic_on_bad_sql() {
        let backend = Backend::new().expect("create in-memory backend");
        // Malformed SQL / missing tables must degrade to empty/default, not panic.
        assert!(
            backend
                .query_grouped_1d("SELECT FROM nonexistent")
                .is_empty()
        );
        assert!(backend.query_strings("NOT VALID SQL AT ALL").is_empty());
        assert!(backend.query_pairs("SELECT FROM nothing").is_empty());
        assert_eq!(backend.query_scalar("SELECT * FROM no_such_table"), 0.0);
        assert!(backend.query_rows("SELECT * FROM no_such_table").is_empty());
    }

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
            let count = db.query_count(&format!(
                "SELECT COUNT(*) FROM date_dim WHERE {flag} = true"
            ));
            assert!(
                count > 0,
                "should have at least one {flag} = true row today"
            );
            assert!(count <= max, "{flag} should not exceed {max} rows");
        }
    }

    #[test]
    fn concurrent_file_backed_checkouts_read_same_database() {
        let path = temp_db_path("concurrent-file-backed");
        let db = Backend::create_demo_file(&path).expect("create demo file");
        let expected = db.query_scalar("SELECT SUM(revenue) FROM sales_fact");
        drop(db);

        let source = BackendSource::file(path.clone()).expect("open pooled source");
        let mut handles = Vec::new();
        for _ in 0..8 {
            let source = source.clone();
            handles.push(std::thread::spawn(move || {
                let backend = source.checkout();
                backend.query_scalar("SELECT SUM(revenue) FROM sales_fact")
            }));
        }

        for handle in handles {
            assert_eq!(handle.join().expect("join reader"), expected);
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn concurrent_demo_checkouts_share_seeded_data() {
        let source = BackendSource::demo().expect("create demo source");
        let expected = source
            .checkout()
            .query_scalar("SELECT SUM(revenue) FROM sales_fact");

        let mut handles = Vec::new();
        for _ in 0..8 {
            let source = source.clone();
            handles.push(std::thread::spawn(move || {
                let backend = source.checkout();
                backend.query_count("SELECT COUNT(*) FROM sales_fact") as u64
                    + backend.query_scalar("SELECT SUM(revenue) FROM sales_fact") as u64
            }));
        }

        for handle in handles {
            let combined = handle.join().expect("join demo reader");
            assert_eq!(combined, 20_000 + expected as u64);
        }
        let _ = std::fs::remove_file(source.path());
    }
}
