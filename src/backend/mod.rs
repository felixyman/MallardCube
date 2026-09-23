use crate::engine::settings::EngineSettings;
use duckdb::{AccessMode, Config, Connection, params};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub struct Backend {
    conn: Mutex<Connection>,
    /// Engine-side cancellation handle (plan 051-B): the request handler
    /// interrupts this connection when a query outlives its timeout.
    interrupt: Arc<duckdb::InterruptHandle>,
}

/// A fixed set of pre-opened, read-only DuckDB connections shared across
/// requests. Opening a connection per request (the previous behaviour) pays the
/// catalog-load cost on every request and, worse, serializes on DuckDB's file
/// lock when several read-write connections open the same file concurrently.
pub struct BackendPool {
    backends: Arc<[Arc<Backend>]>,
    /// Shared across clones: a per-request clone of the source must keep
    /// rotating through the pool, not reset to connection 0.
    next: Arc<AtomicUsize>,
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
            next: self.next.clone(),
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
/// Reported by `GET /status`.
pub fn pool_size() -> usize {
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

fn open_read_only(path: &Path, settings: &EngineSettings) -> Result<Connection, duckdb::Error> {
    match open_with_settings(path, settings) {
        Ok(conn) => Ok(conn),
        Err(e) => {
            // A rejected setting must not take the server down: retry with the
            // engine's own defaults and say so.
            eprintln!("⚠️  engine settings rejected ({e}); opening with engine defaults");
            let config = Config::default().access_mode(AccessMode::ReadOnly)?;
            Connection::open_with_flags(path, config)
        }
    }
}

fn open_with_settings(path: &Path, settings: &EngineSettings) -> Result<Connection, duckdb::Error> {
    let mut config = Config::default().access_mode(AccessMode::ReadOnly)?;
    if let Some(limit) = &settings.memory_limit {
        config = config.max_memory(&limit.value.text)?;
    }
    if let Some(threads) = &settings.threads {
        config = config.threads(threads.value as i64)?;
    }
    if let Some(dir) = &settings.temp_directory {
        config = config.with("temp_directory", dir.value.display().to_string())?;
    }
    Connection::open_with_flags(path, config)
}

// ---- backend trait ----

pub trait QueryBackend {
    fn query_scalar(&self, sql: &str) -> f64;
    fn query_grouped_1d(&self, sql: &str) -> Vec<(String, f64)>;
    fn query_pairs(&self, sql: &str) -> Vec<(String, String, f64)>;
    /// N dimension keys plus one value per row. The multi-axis layouts group
    /// by every dimension on every edge, so two-key `query_pairs` cannot carry
    /// the result (plan 049, phase 3). The default reads `query_rows`.
    fn query_grouped_n(&self, sql: &str, dims: usize) -> Vec<(Vec<String>, f64)> {
        self.query_rows(sql)
            .into_iter()
            .filter_map(|mut row| {
                if row.len() <= dims {
                    return None;
                }
                let value = row[dims].parse::<f64>().ok()?;
                row.truncate(dims);
                Some((row, value))
            })
            .collect()
    }
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
        // Resolved once per process (plan 051-A / 054-C), then reported by
        // /status. Until the shared-engine change lands, each pooled
        // connection is its own DuckDB instance and the ceiling applies per
        // connection rather than process-wide.
        let settings = crate::engine::settings::effective();
        let mut backends = Vec::with_capacity(size);
        for _ in 0..size {
            let conn = open_read_only(path, &settings)?;
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
            let interrupt = conn.interrupt_handle();
            backends.push(Arc::new(Backend {
                conn: Mutex::new(conn),
                interrupt,
            }));
        }
        Ok(BackendPool {
            backends: backends.into(),
            next: Arc::new(AtomicUsize::new(0)),
        })
    }

    fn checkout(&self) -> Arc<Backend> {
        let i = self.next.fetch_add(1, Ordering::Relaxed) % self.backends.len();
        self.backends[i].clone()
    }
}

// ---- deterministic pseudo-random for demo data ----

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
        // The three date draws keep their position (non-date columns are
        // unchanged) but are mixed into a day offset within
        // [2020-01-01, today]: the demo never shows future revenue, and rows
        // spread evenly across months instead of following the LCG's
        // correlated low bits.
        let d1 = rng.next();
        let d2 = rng.next();
        let d3 = rng.next();
        const DEMO_START_DAYS: i64 = 18_262; // 2020-01-01 since epoch
        let span = (today_epoch_days() - DEMO_START_DAYS + 1).max(1) as u64;
        let offset = mix64(d1 ^ d2.rotate_left(21) ^ d3.rotate_left(42)) % span;
        let (year, month, day) = ymd_from_epoch_days(DEMO_START_DAYS + offset as i64);
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

/// Days since 1970-01-01 (UTC), used to bound demo dates to the past.
fn today_epoch_days() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_secs() / 86_400) as i64)
        .unwrap_or(18_628)
}

/// Civil (year, month, day) from days since 1970-01-01 (Hinnant's algorithm).
fn ymd_from_epoch_days(days: i64) -> (i32, i32, i32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y } as i32, m as i32, d as i32)
}

/// SplitMix64 finalizer: breaks up the LCG's correlated low bits.
fn mix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
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

    fn query_grouped_n(&self, sql: &str, dims: usize) -> Vec<(Vec<String>, f64)> {
        Backend::query_grouped_n(self, sql, dims)
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
    /// Cancel whatever query is running on this connection. DuckDB aborts the
    /// query at its next interruption point; the caller sees a query error
    /// (plan 051-B request timeouts).
    pub fn interrupt(&self) {
        self.interrupt.interrupt();
    }

    /// Lock the connection, recovering from a poisoned mutex.
    ///
    /// Request handling catches panics so the server survives (`main.rs`). If a
    /// panic unwound while this lock was held, a plain `lock().unwrap()` would
    /// make every subsequent request panic on the poisoned lock — the server
    /// stays up but serves faults forever. The DuckDB connection itself is
    /// usable after a Rust-side panic, so recovering the guard is strictly
    /// better than refusing to serve.
    fn lock_conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Open a file-based DuckDB database. No seeding — the user owns the schema.
    pub fn open(path: &Path) -> Result<Self, duckdb::Error> {
        let conn = Connection::open(path)?;
        let interrupt = conn.interrupt_handle();
        Ok(Backend {
            conn: Mutex::new(conn),
            interrupt,
        })
    }

    pub fn create_demo_file(path: &Path) -> Result<Self, duckdb::Error> {
        let conn = Connection::open(path)?;
        Self::seed_demo_connection(&conn)?;
        let interrupt = conn.interrupt_handle();
        Ok(Backend {
            conn: Mutex::new(conn),
            interrupt,
        })
    }

    /// Demo fixture for tests: a temp-file copy of the synthetic demo database.
    ///
    /// Production code never touches demo data — it always carries the
    /// project's [`BackendSource`]. This fixture exists so tests can exercise
    /// the demo model (project3) without a process-wide backend static.
    #[cfg(test)]
    pub fn test_fixture() -> &'static Backend {
        static FIXTURE: std::sync::OnceLock<Backend> = std::sync::OnceLock::new();
        FIXTURE.get_or_init(|| {
            let path = std::env::temp_dir()
                .join(format!("mallardcube-test-{}.duckdb", std::process::id()));
            let _ = std::fs::remove_file(&path);
            Backend::create_demo_file(&path).expect("seed test fixture")
        })
    }

    fn seed_demo_connection(conn: &Connection) -> Result<(), duckdb::Error> {
        conn.execute_batch(
            "CREATE TABLE fact_table (
                 product_category VARCHAR NOT NULL,
                 region VARCHAR NOT NULL,
                 sales DOUBLE NOT NULL
             );
             INSERT INTO fact_table VALUES
                 ('Category A', 'North', 100000.0),
                 ('Category A', 'South', 200000.0),
                 ('Category B', 'North', 150000.0),
                 ('Category B', 'South', 100000.0),
                 ('Category C', 'North', 200000.0),
                 ('Category C', 'South', 200000.0),
                 ('Category D', 'North', 200000.0),
                 ('Category D', 'South', 100500.5);
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

    pub fn total_sales(&self) -> f64 {
        let conn = self.lock_conn();
        conn.query_row(
            "SELECT COALESCE(SUM(sales), 0) FROM fact_table",
            [],
            |row| row.get::<_, f64>(0),
        )
        .unwrap_or(0.0)
    }

    pub fn total_sales_for(&self, category: &str) -> f64 {
        let conn = self.lock_conn();
        conn.query_row(
            "SELECT COALESCE(SUM(sales), 0) FROM fact_table WHERE product_category = ?1",
            params![category],
            |row| row.get::<_, f64>(0),
        )
        .unwrap_or(0.0)
    }

    pub fn total_sales_for_region(&self, region: &str) -> f64 {
        let conn = self.lock_conn();
        conn.query_row(
            "SELECT COALESCE(SUM(sales), 0) FROM fact_table WHERE region = ?1",
            params![region],
            |row| row.get::<_, f64>(0),
        )
        .unwrap_or(0.0)
    }

    pub fn category_count(&self) -> u32 {
        let conn = self.lock_conn();
        conn.query_row(
            "SELECT COUNT(DISTINCT product_category) FROM fact_table",
            [],
            |row| row.get::<_, u32>(0),
        )
        .unwrap_or(0)
    }

    pub fn region_count(&self) -> u32 {
        let conn = self.lock_conn();
        conn.query_row("SELECT COUNT(DISTINCT region) FROM fact_table", [], |row| {
            row.get::<_, u32>(0)
        })
        .unwrap_or(0)
    }

    // ---- generic SQL execution (used by engine/plan via sql.rs) ----

    pub fn query_scalar(&self, sql: &str) -> f64 {
        let conn = self.lock_conn();
        conn.query_row(sql, [], |row| {
            Ok(value_to_f64(&row.get::<_, duckdb::types::Value>(0)?).unwrap_or(0.0))
        })
        .unwrap_or(0.0)
    }

    pub fn query_grouped_1d(&self, sql: &str) -> Vec<(String, f64)> {
        let conn = self.lock_conn();
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
        let conn = self.lock_conn();
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

    /// N dimension keys + one value per row (plan 049, phase 3).
    pub fn query_grouped_n(&self, sql: &str, dims: usize) -> Vec<(Vec<String>, f64)> {
        let conn = self.lock_conn();
        let Ok(mut stmt) = conn.prepare(sql) else {
            eprintln!("query_grouped_n: prepare failed: {sql}");
            return Vec::new();
        };
        let rows = match stmt.query_map([], |row| {
            let mut keys = Vec::with_capacity(dims);
            for i in 0..dims {
                keys.push(row.get::<_, String>(i)?);
            }
            let value = value_to_f64(&row.get::<_, duckdb::types::Value>(dims)?).unwrap_or(0.0);
            Ok((keys, value))
        }) {
            Ok(rows) => rows,
            Err(e) => {
                eprintln!("query_grouped_n: query failed: {e}");
                return Vec::new();
            }
        };
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn query_count(&self, sql: &str) -> u32 {
        let conn = self.lock_conn();
        conn.query_row(sql, [], |row| row.get::<_, u32>(0))
            .unwrap_or(0)
    }

    pub fn query_strings(&self, sql: &str) -> Vec<String> {
        let conn = self.lock_conn();
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
        let conn = self.lock_conn();
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
        let conn = self.lock_conn();
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
        let conn = self.lock_conn();
        if let Err(e) = conn.execute_batch(sql) {
            eprintln!("execute_ddl failed: {e}");
        }
    }

    // ---- metadata helpers (used by members.rs) ----

    pub fn distinct_count(&self, column: &str) -> u32 {
        self.distinct_count_in("fact_table", column)
    }

    pub fn distinct_values(&self, column: &str) -> Vec<String> {
        self.distinct_values_in("fact_table", column)
    }

    pub fn distinct_count_in(&self, table: &str, column: &str) -> u32 {
        let sql = format!("SELECT COUNT(DISTINCT {column}) FROM {table}");
        self.query_count(&sql)
    }

    pub fn distinct_values_in(&self, table: &str, column: &str) -> Vec<String> {
        let sql = format!("SELECT DISTINCT {column} FROM {table} ORDER BY {column}");
        let conn = self.lock_conn();
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

/// How [`prepare_parent_child`] treats an existing materialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParentChildMode {
    /// Never write. Read back an existing materialization; an unmaterialized
    /// dimension yields no levels so callers degrade to a flat dimension.
    ReadOnly,
    /// Write only when the dimension is not materialized yet; reuse otherwise.
    Materialize,
    /// Always recompute and rewrite (from `parent_child.refresh: true`).
    Refresh,
}

/// Materialize (or read back) a parent-child hierarchy.
///
/// The write path runs a recursive CTE over the `(key, parent)` pair, adds one
/// ancestor-key column per depth (`{prefix}__pc_l1..N`) plus path/depth, and
/// returns `(level name, column, cardinality)` per depth.
///
/// `Materialize` and `ReadOnly` reuse an existing materialization: reading it
/// back costs one `COUNT(DISTINCT ...)` per level and never touches the user's
/// table. `Refresh` always recomputes. The writes are idempotent.
pub fn prepare_parent_child(
    conn: &duckdb::Connection,
    table: &str,
    key_column: &str,
    parent_column: &str,
    prefix: &str,
    mode: ParentChildMode,
) -> Result<Vec<(String, String, u32)>, duckdb::Error> {
    if mode != ParentChildMode::Refresh {
        let existing = read_materialized_parent_child(conn, table, prefix)?;
        if !existing.is_empty() || mode == ParentChildMode::ReadOnly {
            return Ok(existing);
        }
    }

    let dq = '"';
    let q = |id: &str| format!("{dq}{}{dq}", id.replace(dq, "\"\""));
    let (t, k, p) = (q(table), q(key_column), q(parent_column));
    let tmp = q(&format!("{prefix}__pc"));
    let path_col = q(&format!("{prefix}__pc_path"));
    let depth_col = q(&format!("{prefix}__pc_depth"));

    // Roots: NULL parent, empty parent, or self-reference (cycle guard).
    conn.execute_batch(&format!(
        r#"CREATE OR REPLACE TEMP TABLE {tmp} AS
WITH RECURSIVE pc AS (
  SELECT CAST({t}.{k} AS VARCHAR) AS k,
     CAST({t}.{k} AS VARCHAR) AS path,
     1 AS depth
FROM {t}
   WHERE {t}.{p} IS NULL
  OR CAST({t}.{p} AS VARCHAR) = ''
  OR CAST({t}.{p} AS VARCHAR) = CAST({t}.{k} AS VARCHAR)
  UNION ALL
  SELECT CAST(c.{k} AS VARCHAR), pc.path || '|' || CAST(c.{k} AS VARCHAR), pc.depth + 1
FROM {t} c JOIN pc ON CAST(c.{p} AS VARCHAR) = pc.k
   WHERE pc.depth < 64
)
SELECT k, path, depth FROM pc;"#
    ))?;

    conn.execute_batch(&format!(
        "ALTER TABLE {t} ADD COLUMN IF NOT EXISTS {path_col} VARCHAR; \
         ALTER TABLE {t} ADD COLUMN IF NOT EXISTS {depth_col} INTEGER;"
    ))?;
    conn.execute_batch(&format!(
        r#"UPDATE {t} SET {path_col} = pc.path, {depth_col} = pc.depth
             FROM {tmp} pc WHERE CAST({t}.{k} AS VARCHAR) = pc.k;"#
    ))?;

    let depth: usize = conn.query_row(
        &format!("SELECT COALESCE(MAX(depth), 0) FROM {tmp}"),
        [],
        |r| r.get::<_, usize>(0),
    )?;

    // A previous, deeper materialization may have left ancestor columns beyond
    // the current depth; their values are stale and would resurface on the next
    // read-back. Drop them before (re)building.
    for (i, col) in parent_child_level_columns(conn, table, prefix)? {
        if i as usize > depth {
            conn.execute_batch(&format!(
                "ALTER TABLE {t} DROP COLUMN IF EXISTS {};",
                q(&col)
            ))?;
        }
    }

    let mut out = Vec::new();
    for i in 1..=depth {
        let col = q(&format!("{prefix}__pc_l{i}"));
        conn.execute_batch(&format!(
            "ALTER TABLE {t} ADD COLUMN IF NOT EXISTS {col} VARCHAR;"
        ))?;
        conn.execute_batch(&format!(
            r#"UPDATE {t} SET {col} = CASE WHEN pc.depth >= {i}
                     THEN split_part(pc.path, '|', {i}) END
                 FROM {tmp} pc WHERE CAST({t}.{k} AS VARCHAR) = pc.k;"#
        ))?;
        let card: u32 =
            conn.query_row(&format!("SELECT COUNT(DISTINCT {col}) FROM {t}"), [], |r| {
                r.get::<_, u32>(0)
            })?;
        out.push((format!("Level {i:02}"), format!("{prefix}__pc_l{i}"), card));
    }
    Ok(out)
}

/// Read back an existing parent-child materialization without writing: level
/// columns are discovered from the table schema, cardinalities from live
/// distinct counts. Empty when `{prefix}__pc_depth` is not present.
fn read_materialized_parent_child(
    conn: &duckdb::Connection,
    table: &str,
    prefix: &str,
) -> Result<Vec<(String, String, u32)>, duckdb::Error> {
    let dq = '"';
    let q = |id: &str| format!("{dq}{}{dq}", id.replace(dq, "\"\""));
    let depth_col = format!("{prefix}__pc_depth");
    let escaped = table.replace('\'', "''");
    let mut stmt = conn.prepare(&format!("SELECT name FROM pragma_table_info('{escaped}')"))?;
    let has_depth = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .any(|n| n == depth_col);
    if !has_depth {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for (i, col) in parent_child_level_columns(conn, table, prefix)? {
        let card: u32 = conn.query_row(
            &format!("SELECT COUNT(DISTINCT {}) FROM {}", q(&col), q(table)),
            [],
            |r| r.get::<_, u32>(0),
        )?;
        out.push((format!("Level {i:02}"), col, card));
    }
    Ok(out)
}

/// The `{prefix}__pc_lN` columns present on `table`, sorted by depth N.
fn parent_child_level_columns(
    conn: &duckdb::Connection,
    table: &str,
    prefix: &str,
) -> Result<Vec<(u32, String)>, duckdb::Error> {
    let level_prefix = format!("{prefix}__pc_l");
    let escaped = table.replace('\'', "''");
    let mut stmt = conn.prepare(&format!("SELECT name FROM pragma_table_info('{escaped}')"))?;
    let names: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    let mut levels: Vec<(u32, String)> = names
        .iter()
        .filter_map(|n| {
            n.strip_prefix(&level_prefix)
                .and_then(|s| s.parse::<u32>().ok())
                .map(|i| (i, n.clone()))
        })
        .collect();
    levels.sort();
    Ok(levels)
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

    // The demo must look believable: no future-dated revenue, and every month
    // from the start of the range through the current month has facts. The old
    // LCG date draws left visible gaps and produced "revenue" years ahead.
    #[test]
    fn sales_fact_rows_stay_in_the_past_and_cover_every_month() {
        use std::collections::BTreeSet;

        let rows = super::generate_sales_fact_rows();
        let (ty, tm, td) = super::ymd_from_epoch_days(super::today_epoch_days());
        let today_key = ty * 10000 + tm * 100 + td;
        assert_eq!(rows.len(), 20_000);
        assert!(
            rows.iter()
                .all(|r| r.date_key >= 20_200_101 && r.date_key <= today_key),
            "dates must lie in [2020-01-01, today]"
        );

        let mut months: BTreeSet<(i32, i32)> = BTreeSet::new();
        let mut combos: BTreeSet<(String, (i32, i32))> = BTreeSet::new();
        for r in &rows {
            let y = r.date_key / 10000;
            let m = (r.date_key / 100) % 100;
            months.insert((y, m));
            combos.insert((r.category.clone(), (y, m)));
        }
        for y in 2020..ty {
            for m in 1..=12 {
                assert!(months.contains(&(y, m)), "missing month {y}-{m:02}");
            }
        }
        for m in 1..=tm {
            assert!(months.contains(&(ty, m)), "missing month {ty}-{m:02}");
        }
        assert_eq!(
            months.len(),
            12 * (ty - 2020) as usize + tm as usize,
            "every month in the range must have facts"
        );

        let categories: BTreeSet<&str> = rows.iter().map(|r| r.category.as_str()).collect();
        let total = categories.len() * months.len();
        let missing = total - combos.len();
        // A uniform spread leaves ~12 rows per category-month, so gaps are
        // essentially impossible; allow 1% for date-range drift over time.
        assert!(
            missing * 100 <= total,
            "{missing} of {total} category-month combos have no facts"
        );
    }

    // A cloned source must keep rotating through the pool. Cloning used to
    // reset the counter, so every request checked out connection 0 and the
    // pool serialized all queries on a single connection.
    #[test]
    fn pool_clone_shares_round_robin() {
        if super::pool_size() < 2 {
            return; // a single-connection pool cannot demonstrate rotation
        }
        let path = temp_db_path("pool-rotation");
        let _ = std::fs::remove_file(&path);
        {
            let conn = duckdb::Connection::open(&path).unwrap();
            conn.execute_batch("CREATE TABLE t (i INT); INSERT INTO t VALUES (1);")
                .unwrap();
        }
        let source = BackendSource::file(&path).expect("open pool");
        let first = source.checkout();
        let second = source.checkout();
        assert!(
            !std::sync::Arc::ptr_eq(&first, &second),
            "checkout must rotate connections"
        );

        let clone = source.clone();
        let third = clone.checkout();
        assert!(
            !std::sync::Arc::ptr_eq(&third, &first),
            "a cloned source must continue the rotation (shared counter)"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn engine_settings_reach_the_duckdb_connection() {
        // The crate's config path (not `SET` after connect) is what the pool
        // uses, so prove the values land on a real connection.
        use crate::engine::settings::{EngineSettings, MemoryLimit, Setting, SettingSource};
        let settings = EngineSettings {
            memory_limit: Some(Setting {
                value: MemoryLimit {
                    text: "3221225472B".into(),
                    bytes: Some(3 << 30),
                },
                source: SettingSource::Env,
            }),
            temp_directory: None,
            threads: Some(Setting {
                value: 3,
                source: SettingSource::Env,
            }),
            max_concurrent_queries: Setting {
                value: 1,
                source: SettingSource::Default,
            },
            query_timeout: Setting {
                value: None,
                source: SettingSource::Default,
            },
        };
        let path = temp_db_path("engine-settings");
        {
            let conn = duckdb::Connection::open(&path).unwrap();
            conn.execute_batch("CREATE TABLE t (i INT);").unwrap();
        }
        let conn =
            super::open_with_settings(&path, &settings).expect("read-only conn with settings");
        let (limit, threads): (String, i64) = conn
            .query_row(
                "SELECT current_setting('memory_limit'), current_setting('threads')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(limit, "3.0 GiB", "max_memory applied: {limit}");
        assert_eq!(threads, 3, "threads applied");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn parent_child_prepare_materializes_levels() {
        let conn = duckdb::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE emp (k INT, p INT);
             INSERT INTO emp VALUES (1,NULL),(2,1),(3,1),(4,2),(5,2),(6,3),(9,NULL);",
        )
        .unwrap();
        let levels = super::prepare_parent_child(
            &conn,
            "emp",
            "k",
            "p",
            "emp",
            super::ParentChildMode::Materialize,
        )
        .unwrap();
        assert_eq!(levels.len(), 3, "three depths: {levels:?}");
        assert_eq!(levels[0].0, "Level 01");
        assert_eq!(levels[0].1, "emp__pc_l1");
        // L1 roots = {1, 9}; L2 = {2, 3}; L3 = {4, 5, 6}
        assert_eq!(levels[0].2, 2, "{levels:?}");
        assert_eq!(levels[1].2, 2, "{levels:?}");
        assert_eq!(levels[2].2, 3, "{levels:?}");

        // Ragged columns: depth-2 members have L3 NULL; depth-3 members don't.
        let shallow: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM emp WHERE emp__pc_l1 = '1' AND emp__pc_l3 IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(shallow, 3, "root + members 2,3 stop before depth 3");
        let deep: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM emp WHERE emp__pc_l1 = '1' AND emp__pc_l3 = '4'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(deep, 1, "member 4 carries its full path");

        // Idempotent: rerunning must not change results. The second call takes
        // the read-back fast path, so this also proves it matches the write path.
        let again = super::prepare_parent_child(
            &conn,
            "emp",
            "k",
            "p",
            "emp",
            super::ParentChildMode::Materialize,
        )
        .unwrap();
        assert_eq!(again, levels);
    }

    /// Read-only mode must never add or populate columns, and a dimension that
    /// has not been materialized yields no levels (callers degrade to flat).
    #[test]
    fn parent_child_read_only_never_materializes() {
        let conn = duckdb::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE emp (k INT, p INT);
             INSERT INTO emp VALUES (1,NULL),(2,1),(3,2);",
        )
        .unwrap();
        let levels = super::prepare_parent_child(
            &conn,
            "emp",
            "k",
            "p",
            "emp",
            super::ParentChildMode::ReadOnly,
        )
        .unwrap();
        assert!(levels.is_empty(), "nothing to read back yet: {levels:?}");
        let cols: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('emp') WHERE name LIKE 'emp__pc%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cols, 0, "read-only mode must not add columns");
    }

    /// Refresh after the hierarchy shrinks must drop stale deeper level
    /// columns so they cannot resurface on the next read-back.
    #[test]
    fn parent_child_refresh_drops_stale_deeper_levels() {
        let conn = duckdb::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE emp (k INT, p INT);
             INSERT INTO emp VALUES (1,NULL),(2,1),(3,2);",
        )
        .unwrap();
        let three = super::prepare_parent_child(
            &conn,
            "emp",
            "k",
            "p",
            "emp",
            super::ParentChildMode::Refresh,
        )
        .unwrap();
        assert_eq!(three.len(), 3, "{three:?}");

        // Collapse the hierarchy to two levels and refresh.
        conn.execute_batch("DELETE FROM emp WHERE k = 3;").unwrap();
        let two = super::prepare_parent_child(
            &conn,
            "emp",
            "k",
            "p",
            "emp",
            super::ParentChildMode::Refresh,
        )
        .unwrap();
        assert_eq!(two.len(), 2, "stale Level 03 must be dropped: {two:?}");

        // The read-back fast path sees exactly the same two levels.
        let read_back = super::prepare_parent_child(
            &conn,
            "emp",
            "k",
            "p",
            "emp",
            super::ParentChildMode::Materialize,
        )
        .unwrap();
        assert_eq!(read_back, two);
    }

    /// A panic while the connection lock is held must not brick the backend.
    /// Request handling catches panics (`main.rs`); without poison recovery the
    /// poisoned lock would turn that one failure into permanent SOAP faults.
    #[test]
    fn poisoned_connection_lock_recovers() {
        let conn = duckdb::Connection::open_in_memory().unwrap();
        let interrupt = conn.interrupt_handle();
        let backend = Backend {
            conn: std::sync::Mutex::new(conn),
            interrupt,
        };
        backend.execute_ddl("CREATE TABLE sales_fact (i INT); INSERT INTO sales_fact VALUES (1);");
        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = backend.conn.lock().unwrap();
            panic!("simulated request panic while holding the connection lock");
        }));
        assert!(poisoned.is_err(), "the panic must have been caught");
        assert!(
            backend.conn.lock().is_err(),
            "the mutex must be poisoned before recovery is exercised"
        );
        // Must still serve through the poison-tolerant accessor.
        assert_eq!(backend.query_scalar("SELECT 1"), 1.0);
        assert!(
            backend.query_count("SELECT COUNT(*) FROM sales_fact") > 0,
            "queries must keep working after a poisoned lock"
        );
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
        let conn = duckdb::Connection::open_in_memory().unwrap();
        let interrupt = conn.interrupt_handle();
        let backend = Backend {
            conn: std::sync::Mutex::new(conn),
            interrupt,
        };
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
        let conn = duckdb::Connection::open_in_memory().unwrap();
        let interrupt = conn.interrupt_handle();
        let db = Backend {
            conn: std::sync::Mutex::new(conn),
            interrupt,
        };
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
