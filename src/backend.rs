use rusqlite::Connection;
use std::sync::{Mutex, OnceLock};

pub struct Backend {
    conn: Mutex<Connection>,
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

fn instance() -> &'static Backend {
    static BACKEND: OnceLock<Backend> = OnceLock::new();
    BACKEND.get_or_init(|| Backend::new().expect("failed to initialise SQLite"))
}

impl Backend {
    pub fn get() -> &'static Self {
        instance()
    }

    pub fn new() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE faktatabell (
                 produktkategori TEXT NOT NULL,
                 region TEXT NOT NULL,
                 sales REAL NOT NULL
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

    pub fn new_with_config(config: &BenchmarkDataConfig) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE faktatabell (
                 produktkategori TEXT NOT NULL,
                 region TEXT NOT NULL,
                 sales REAL NOT NULL
             );",
        )?;

        let mut rng = SeededRng::new(config.seed);
        let categories: Vec<String> =
            (1..=config.category_count).map(|i| format!("Kategori {:03}", i)).collect();
        let regions: Vec<String> =
            (1..=config.region_count).map(|i| format!("Region {:02}", i)).collect();

        {
            let mut stmt = conn.prepare(
                "INSERT INTO faktatabell (produktkategori, region, sales) VALUES (?1, ?2, ?3)"
            )?;

            for i in 0..config.row_count {
                let kat = &categories[rng.next() as usize % config.category_count];
                let reg = &regions[rng.next() as usize % config.region_count];
                let sales = 10_000.0 + (rng.next() as f64 % 100_000.0);
                stmt.execute(rusqlite::params![kat, reg, sales])?;
            }
        }

        // add one skewed high-value row to ensure a known large aggregation
        conn.execute(
            "INSERT INTO faktatabell VALUES ('Kategori SKEW', 'Region SKEW', 9999999.0)",
            [],
        )?;

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
            [category],
            |row| row.get::<_, f64>(0),
        )
        .unwrap_or(0.0)
    }

    pub fn total_sales_for_region(&self, region: &str) -> f64 {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COALESCE(SUM(sales), 0) FROM faktatabell WHERE region = ?1",
            [region],
            |row| row.get::<_, f64>(0),
        )
        .unwrap_or(0.0)
    }

    pub fn sales_for_categories(&self, cats: &[String]) -> Vec<(String, f64)> {
        if cats.is_empty() {
            return self.sales_by_category();
        }
        let conn = self.conn.lock().unwrap();
        let placeholders: Vec<String> = (1..=cats.len()).map(|i| format!("?{}", i)).collect();
        let sql = format!(
            "SELECT produktkategori, SUM(sales) FROM faktatabell WHERE produktkategori IN ({}) GROUP BY 1 ORDER BY 1",
            placeholders.join(",")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> =
            cats.iter().map(|c| c as &dyn rusqlite::types::ToSql).collect();
        let mut stmt = conn.prepare(&sql).expect("prepare sales_for_categories");
        let rows = stmt
            .query_map(params.as_slice(), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
            })
            .expect("query_map sales_for_categories");
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn total_for_categories(&self, cats: &[String]) -> f64 {
        if cats.is_empty() {
            return self.total_sales();
        }
        let conn = self.conn.lock().unwrap();
        let placeholders: Vec<String> = (1..=cats.len()).map(|i| format!("?{}", i)).collect();
        let sql = format!(
            "SELECT COALESCE(SUM(sales), 0) FROM faktatabell WHERE produktkategori IN ({})",
            placeholders.join(",")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> =
            cats.iter().map(|c| c as &dyn rusqlite::types::ToSql).collect();
        conn.query_row(&sql, params.as_slice(), |row| row.get::<_, f64>(0))
            .unwrap_or(0.0)
    }

    pub fn sales_by_category(&self) -> Vec<(String, f64)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT produktkategori, SUM(sales) FROM faktatabell GROUP BY 1 ORDER BY 1")
            .expect("prepare sales_by_category");
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?)))
            .expect("query_map sales_by_category");
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn category_names(&self) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT DISTINCT produktkategori FROM faktatabell ORDER BY 1")
            .expect("prepare category_names");
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query_map category_names");
        rows.filter_map(|r| r.ok()).collect()
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

    pub fn sales_by_region(&self) -> Vec<(String, f64)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT region, SUM(sales) FROM faktatabell GROUP BY 1 ORDER BY 1")
            .expect("prepare sales_by_region");
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?)))
            .expect("query_map sales_by_region");
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn sales_for_regions(&self, regions: &[String]) -> Vec<(String, f64)> {
        if regions.is_empty() {
            return self.sales_by_region();
        }
        let conn = self.conn.lock().unwrap();
        let placeholders: Vec<String> = (1..=regions.len()).map(|i| format!("?{}", i)).collect();
        let sql = format!(
            "SELECT region, SUM(sales) FROM faktatabell WHERE region IN ({}) GROUP BY 1 ORDER BY 1",
            placeholders.join(",")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> =
            regions.iter().map(|c| c as &dyn rusqlite::types::ToSql).collect();
        let mut stmt = conn.prepare(&sql).expect("prepare sales_for_regions");
        let rows = stmt
            .query_map(params.as_slice(), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
            })
            .expect("query_map sales_for_regions");
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn total_for_regions(&self, regions: &[String]) -> f64 {
        if regions.is_empty() {
            return self.total_sales();
        }
        let conn = self.conn.lock().unwrap();
        let placeholders: Vec<String> = (1..=regions.len()).map(|i| format!("?{}", i)).collect();
        let sql = format!(
            "SELECT COALESCE(SUM(sales), 0) FROM faktatabell WHERE region IN ({})",
            placeholders.join(",")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> =
            regions.iter().map(|c| c as &dyn rusqlite::types::ToSql).collect();
        conn.query_row(&sql, params.as_slice(), |row| row.get::<_, f64>(0))
            .unwrap_or(0.0)
    }

    pub fn region_names(&self) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT DISTINCT region FROM faktatabell ORDER BY 1")
            .expect("prepare region_names");
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query_map region_names");
        rows.filter_map(|r| r.ok()).collect()
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

    pub fn grouped_by_produktkategori(&self, region_filter: &[String], kat_filter: &[String]) -> Vec<(String, f64)> {
        let conn = self.conn.lock().unwrap();
        if region_filter.is_empty() && kat_filter.is_empty() {
            let mut stmt = conn
                .prepare("SELECT produktkategori, SUM(sales) FROM faktatabell GROUP BY 1 ORDER BY 1")
                .expect("prepare grouped_by_produktkategori");
            let rows = stmt
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?)))
                .expect("query_map grouped_by_produktkategori");
            return rows.filter_map(|r| r.ok()).collect();
        }
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;
        if !region_filter.is_empty() {
            let ph: Vec<String> = (idx..idx + region_filter.len()).map(|i| format!("?{}", i)).collect();
            conditions.push(format!("region IN ({})", ph.join(",")));
            for r in region_filter {
                params.push(Box::new(r.clone()));
            }
            idx += region_filter.len();
        }
        if !kat_filter.is_empty() {
            let ph: Vec<String> = (idx..idx + kat_filter.len()).map(|i| format!("?{}", i)).collect();
            conditions.push(format!("produktkategori IN ({})", ph.join(",")));
            for k in kat_filter {
                params.push(Box::new(k.clone()));
            }
        }
        let sql = format!(
            "SELECT produktkategori, SUM(sales) FROM faktatabell WHERE {} GROUP BY 1 ORDER BY 1",
            conditions.join(" AND ")
        );
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).expect("prepare grouped_by_produktkategori filtered");
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
            })
            .expect("query_map grouped_by_produktkategori filtered");
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn grouped_by_region(&self, region_filter: &[String], kat_filter: &[String]) -> Vec<(String, f64)> {
        let conn = self.conn.lock().unwrap();
        if region_filter.is_empty() && kat_filter.is_empty() {
            let mut stmt = conn
                .prepare("SELECT region, SUM(sales) FROM faktatabell GROUP BY 1 ORDER BY 1")
                .expect("prepare grouped_by_region");
            let rows = stmt
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?)))
                .expect("query_map grouped_by_region");
            return rows.filter_map(|r| r.ok()).collect();
        }
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;
        if !region_filter.is_empty() {
            let ph: Vec<String> = (idx..idx + region_filter.len()).map(|i| format!("?{}", i)).collect();
            conditions.push(format!("region IN ({})", ph.join(",")));
            for r in region_filter {
                params.push(Box::new(r.clone()));
            }
            idx += region_filter.len();
        }
        if !kat_filter.is_empty() {
            let ph: Vec<String> = (idx..idx + kat_filter.len()).map(|i| format!("?{}", i)).collect();
            conditions.push(format!("produktkategori IN ({})", ph.join(",")));
            for k in kat_filter {
                params.push(Box::new(k.clone()));
            }
        }
        let sql = format!(
            "SELECT region, SUM(sales) FROM faktatabell WHERE {} GROUP BY 1 ORDER BY 1",
            conditions.join(" AND ")
        );
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).expect("prepare grouped_by_region filtered");
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
            })
            .expect("query_map grouped_by_region filtered");
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn total_with_filters(&self, region_filter: &[String], kat_filter: &[String]) -> f64 {
        let conn = self.conn.lock().unwrap();
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;
        if !region_filter.is_empty() {
            let ph: Vec<String> = (idx..idx + region_filter.len()).map(|i| format!("?{}", i)).collect();
            conditions.push(format!("region IN ({})", ph.join(",")));
            for r in region_filter {
                params.push(Box::new(r.clone()));
            }
            idx += region_filter.len();
        }
        if !kat_filter.is_empty() {
            let ph: Vec<String> = (idx..idx + kat_filter.len()).map(|i| format!("?{}", i)).collect();
            conditions.push(format!("produktkategori IN ({})", ph.join(",")));
            for k in kat_filter {
                params.push(Box::new(k.clone()));
            }
        }
        let sql = if conditions.is_empty() {
            "SELECT COALESCE(SUM(sales), 0) FROM faktatabell".to_string()
        } else {
            format!("SELECT COALESCE(SUM(sales), 0) FROM faktatabell WHERE {}", conditions.join(" AND "))
        };
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.query_row(&sql, param_refs.as_slice(), |row| row.get::<_, f64>(0))
            .unwrap_or(0.0)
    }

    pub fn grouped_pairs(&self) -> Vec<(String, String, f64)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT produktkategori, region, SUM(sales) FROM faktatabell GROUP BY 1, 2 ORDER BY 1, 2")
            .expect("prepare grouped_pairs");
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, f64>(2)?)))
            .expect("query_map grouped_pairs");
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn grouped_pairs_filtered(&self, region_filter: &[String], kat_filter: &[String]) -> Vec<(String, String, f64)> {
        let conn = self.conn.lock().unwrap();
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;
        if !region_filter.is_empty() {
            let ph: Vec<String> = (idx..idx + region_filter.len()).map(|i| format!("?{}", i)).collect();
            conditions.push(format!("region IN ({})", ph.join(",")));
            for r in region_filter {
                params.push(Box::new(r.clone()));
            }
            idx += region_filter.len();
        }
        if !kat_filter.is_empty() {
            let ph: Vec<String> = (idx..idx + kat_filter.len()).map(|i| format!("?{}", i)).collect();
            conditions.push(format!("produktkategori IN ({})", ph.join(",")));
            for k in kat_filter {
                params.push(Box::new(k.clone()));
            }
        }
        let sql = if conditions.is_empty() {
            "SELECT produktkategori, region, SUM(sales) FROM faktatabell GROUP BY 1, 2 ORDER BY 1, 2".to_string()
        } else {
            format!(
                "SELECT produktkategori, region, SUM(sales) FROM faktatabell WHERE {} GROUP BY 1, 2 ORDER BY 1, 2",
                conditions.join(" AND ")
            )
        };
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).expect("prepare grouped_pairs_filtered");
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, f64>(2)?))
            })
            .expect("query_map grouped_pairs_filtered");
        rows.filter_map(|r| r.ok()).collect()
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
