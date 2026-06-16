/// Text caching for SQL and Malloy emission.
///
/// Keyed by normalized PlanKey from `engine/normalize`.
/// Caches only deterministic text, not runtime results.
/// Thread-safe via Mutex, designed for a single process.

use std::collections::HashMap;
use std::sync::{Mutex, atomic::{AtomicU64, Ordering}};
use crate::engine::plan::QueryPlan;
use crate::engine::model::SemanticModel;
use crate::engine::normalize::plan_key;
use crate::engine::sql::sql_for_query_plan;
use crate::engine::malloy::malloy_for_query_plan;
use crate::engine::malloy_compiler::{MalloyCompiler, MalloyCompileError};

pub struct PlanCache {
    sql: Mutex<HashMap<String, String>>,
    malloy: Mutex<HashMap<String, String>>,
    /// compiled SQL via Malloy runtime
    compiled: Mutex<HashMap<String, String>>,
    pub hits: AtomicU64,
    pub misses: AtomicU64,
}

impl PlanCache {
    pub fn new() -> Self {
        Self {
            sql: Mutex::new(HashMap::new()),
            malloy: Mutex::new(HashMap::new()),
            compiled: Mutex::new(HashMap::new()),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    pub fn get_or_generate_sql(&self, plan: &QueryPlan, model: &SemanticModel) -> String {
        let key = plan_key(plan);
        {
            let cache = self.sql.lock().unwrap();
            if let Some(sql) = cache.get(&key) {
                return sql.clone();
            }
        }
        let sql = sql_for_query_plan(model, plan);
        {
            let mut cache = self.sql.lock().unwrap();
            cache.insert(key, sql.clone());
        }
        sql
    }

    pub fn get_or_generate_malloy(&self, plan: &QueryPlan, model: &SemanticModel) -> String {
        let key = plan_key(plan);
        {
            let cache = self.malloy.lock().unwrap();
            if let Some(m) = cache.get(&key) {
                return m.clone();
            }
        }
        let m = malloy_for_query_plan(model, plan);
        {
            let mut cache = self.malloy.lock().unwrap();
            cache.insert(key, m.clone());
        }
        m
    }

    /// Get or compile SQL via Malloy runtime, caching the result by PlanKey.
    /// Returns (sql, was_cache_hit, js_compile_ms).
    /// js_compile_ms is 0.0 for cache hits, otherwise the worker-reported time.
    pub fn get_or_compile(
        &self,
        plan: &QueryPlan,
        model: &SemanticModel,
        compiler: &dyn MalloyCompiler,
    ) -> Result<(String, bool, f64), MalloyCompileError> {
        let key = plan_key(plan);
        {
            let cache = self.compiled.lock().unwrap();
            if let Some(sql) = cache.get(&key) {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Ok((sql.clone(), true, 0.0));
            }
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
        let r = compiler.compile_query(model, plan)?;
        {
            let mut cache = self.compiled.lock().unwrap();
            cache.insert(key, r.sql.clone());
        }
        Ok((r.sql, false, r.compile_ms))
    }

    pub fn hit_rate_pct(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed) as f64;
        let misses = self.misses.load(Ordering::Relaxed) as f64;
        let total = hits + misses;
        if total == 0.0 { 0.0 } else { hits / total * 100.0 }
    }
}

impl Default for PlanCache {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::plan::TypedDimensionFilter;
    use crate::engine::model::default_model;

    #[test]
    fn sql_is_cached_same_key() {
        let cache = PlanCache::new();
        let model = default_model();
        let plan = QueryPlan::Total { measure: "TotalSales".to_string(), filters: vec![] };
        let a = cache.get_or_generate_sql(&plan, &model);
        let b = cache.get_or_generate_sql(&plan, &model);
        assert_eq!(a, b, "same plan should return same cached SQL");
    }

    #[test]
    fn malloy_is_cached_same_key() {
        let cache = PlanCache::new();
        let model = default_model();
        let plan = QueryPlan::GroupBy {
            measure: "TotalSales".to_string(),
            group_by: vec!["Produktkategori".to_string(), "Region".to_string()],
            filters: vec![],
        };
        let a = cache.get_or_generate_malloy(&plan, &model);
        let b = cache.get_or_generate_malloy(&plan, &model);
        assert_eq!(a, b, "same plan should return same cached Malloy");
    }

    #[test]
    fn equivalent_filters_share_cache() {
        let cache = PlanCache::new();
        let model = default_model();
        let a = QueryPlan::Total {
            measure: "TotalSales".to_string(),
            filters: vec![
                TypedDimensionFilter { dimension: "Region".to_string(), members: vec!["North".into()] , time_flag: None },
                TypedDimensionFilter { dimension: "Produktkategori".to_string(), members: vec!["Kategori A".into()] , time_flag: None },
            ],
        };
        let b = QueryPlan::Total {
            measure: "TotalSales".to_string(),
            filters: vec![
                TypedDimensionFilter { dimension: "Produktkategori".to_string(), members: vec!["Kategori A".into()] , time_flag: None },
                TypedDimensionFilter { dimension: "Region".to_string(), members: vec!["North".into()] , time_flag: None },
            ],
        };
        let sql_a = cache.get_or_generate_sql(&a, &model);
        let sql_b = cache.get_or_generate_sql(&b, &model);
        assert_eq!(sql_a, sql_b, "equivalent filter order should share cache");
    }

    #[test]
    fn compiled_cache_is_used() {
        use crate::engine::malloy_compiler::NullCompiler;
        let cache = PlanCache::new();
        let model = default_model();
        let plan = QueryPlan::Total { measure: "TotalSales".to_string(), filters: vec![] };
        let compiler = NullCompiler;
        let (a, hit_a, _ms_a) = cache.get_or_compile(&plan, &model, &compiler).unwrap();
        assert!(!hit_a, "first call should be a miss");
        let (b, hit_b, _ms_b) = cache.get_or_compile(&plan, &model, &compiler).unwrap();
        assert!(hit_b, "second call should be a hit");
        assert_eq!(a, b, "compiled SQL should hit cache");
    }
}
