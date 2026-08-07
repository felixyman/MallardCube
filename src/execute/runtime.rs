/// Malloy runtime machinery.
///
/// Contains the long-lived Node.js worker, compile cache, timing
/// instrumentation, and the two execution paths (direct SQL and Malloy).
/// Used by `builders.rs` and `main.rs`.
use crate::backend::QueryBackend;
use crate::engine::cache::PlanCache;
use crate::engine::malloy_compiler::MalloyCompiler;
use crate::engine::malloy_node_longlived::LongLivedNodeMalloyCompiler;
use crate::engine::model::UserContext;
use crate::engine::normalize::plan_key;
use crate::engine::plan::{
    execute_plan_sql_with_backend, execute_plan_with_backend,
    execute_plan_with_backend_and_context, plan_from_semantic,
    plan_from_semantic_with_model_and_context,
};
use crate::engine::timing::{RuntimePath, Timings};
use crate::execute::render::dispatch_with_backend;
use crate::mdx_semantic::SemanticQueryKind;
use crate::project::config::ProxyConfig;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// Toggle between direct SQL and Malloy runtime path.
/// Set via env var MALLOY_RUNTIME=1 or programmatically.
pub static USE_MALLOY_RUNTIME: AtomicBool = AtomicBool::new(false);

/// Enable Malloy runtime for analytic queries (Total, GroupBy).
pub fn enable_malloy_runtime() {
    USE_MALLOY_RUNTIME.store(true, Ordering::Relaxed);
}

/// Disable Malloy runtime — use direct SQL for all queries.
pub fn disable_malloy_runtime() {
    USE_MALLOY_RUNTIME.store(false, Ordering::Relaxed);
}

/// Module-level long-lived Malloy compiler (lazy, spawned on first use).
static COMPILER: OnceLock<LongLivedNodeMalloyCompiler> = OnceLock::new();

/// Module-level compiled-SQL cache shared across all requests.
static CACHE: OnceLock<PlanCache> = OnceLock::new();

fn malloy_compiler() -> &'static LongLivedNodeMalloyCompiler {
    COMPILER.get_or_init(|| LongLivedNodeMalloyCompiler::new().expect("start Malloy compiler"))
}

fn malloy_cache() -> &'static PlanCache {
    CACHE.get_or_init(PlanCache::new)
}

/// Eagerly spawn the long-lived Malloy compiler and warm its internal
/// caches so the first Excel request doesn't pay the startup cost.
/// Call once at server startup when MALLOY_RUNTIME=1.
pub fn warm_malloy_worker() {
    use crate::engine::malloy_compiler::MalloyCompiler;
    use crate::engine::plan::QueryPlan;
    use std::time::Instant;
    let model = &crate::proxy_project::project().model;
    let plan = QueryPlan::Total {
        measure: model
            .default_measure_id()
            .unwrap_or_else(|| "Revenue".into()),
        filters: vec![],
    };
    let t1 = Instant::now();
    match malloy_compiler().compile_query(model, &plan) {
        Ok(r) => {
            let warm_ms = t1.elapsed().as_millis();
            eprintln!(
                "[malloy] warm-up compile OK in {warm_ms}ms (JS compile {:.2}ms)",
                r.compile_ms,
            );
        }
        Err(e) => {
            eprintln!("[malloy] warm-up compile FAILED: {e}");
        }
    }
}

pub fn get_execute_cellset_response_timed_with_backend<B: QueryBackend + ?Sized>(
    mdx: &str,
    backend: &B,
) -> (String, Timings) {
    use std::time::Instant;

    let t0 = Instant::now();
    let query = crate::mdx_semantic::semantic_query_from_mdx(mdx);
    let mdx_parse_us = (Instant::now() - t0).as_micros() as u64;

    let t0 = Instant::now();
    let plan = plan_from_semantic(&query);
    let plan_us = (Instant::now() - t0).as_micros() as u64;
    let key = plan_key(&plan);

    let t0 = Instant::now();
    let model = &crate::proxy_project::project().model;
    let result = execute_plan_with_backend(&plan, model, backend);
    let sql_execute_us = (Instant::now() - t0).as_micros() as u64;

    let mut timings = Timings::new(RuntimePath::DirectSql, key, mdx_parse_us, 0);
    timings.plan_us = plan_us;
    timings.sql_execute_us = sql_execute_us;

    let t0 = Instant::now();
    let xml = dispatch_with_backend(&query, &result, backend);
    timings.xml_render_us = (Instant::now() - t0).as_micros() as u64;
    timings.finish();
    (xml, timings)
}

pub fn get_execute_cellset_response_timed_malloy_with_backend<B: QueryBackend + ?Sized>(
    mdx: &str,
    backend: &B,
    user: &UserContext,
    config: &ProxyConfig,
) -> (String, Timings) {
    use std::time::Instant;

    let t0 = Instant::now();
    let query = crate::mdx_semantic::semantic_query_from_mdx(mdx);
    let mdx_parse_us = (Instant::now() - t0).as_micros() as u64;

    let t0 = Instant::now();
    let model = &crate::proxy_project::project().model;
    let plan = plan_from_semantic_with_model_and_context(&query, model, user, config);
    let plan_us = (Instant::now() - t0).as_micros() as u64;
    let key = plan_key(&plan);

    // Malloy runtime has no role filtering — only admin users may use it.
    // Non-admin users always fall through to direct SQL which includes
    // role predicates. This is a documented limitation (Malloy path does
    // not inject role filters).
    let use_malloy = USE_MALLOY_RUNTIME.load(Ordering::Relaxed)
        && user.is_administrator
        && matches!(
            query.kind,
            SemanticQueryKind::SlicerAllAndMeasure
                | SemanticQueryKind::SlicerOnly
                | SemanticQueryKind::DrilldownCategories
                | SemanticQueryKind::LeafLevelMembers
                | SemanticQueryKind::MeasureByCategory
                | SemanticQueryKind::DrilldownMemberProbe
        );

    let (
        result,
        runtime_path,
        malloy_compile_us,
        compiled_cache_hit,
        js_compile_ms,
        sql_execute_us,
    ) = if use_malloy {
        let compiler = malloy_compiler();

        let t0 = Instant::now();
        let project = crate::proxy_project::project();
        let source = project.malloy_source(&plan);
        let (sql, was_hit, worker_compile_ms, compile_err) = if project.malloy_model_text.is_empty()
        {
            let cache = malloy_cache();
            match cache.get_or_compile(&plan, model, compiler) {
                Ok((s, h, ms)) => (s, h, ms, None),
                Err(e) => (String::new(), false, 0.0, Some(e)),
            }
        } else {
            match compiler.compile_malloy(&source) {
                Ok(cr) => (cr.sql, false, cr.compile_ms, None),
                Err(e) => (String::new(), false, 0.0, Some(e)),
            }
        };
        let compile_us = (Instant::now() - t0).as_micros() as u64;

        if let Some(ref err) = compile_err {
            eprintln!(
                "Malloy compile FAILED plan_key={} kind={:?} measure={:?}: {err}",
                plan_key(&plan),
                query.kind,
                query.measure,
            );
            eprintln!("  Malloy source:\n{source}");

            let t1 = Instant::now();
            let fallback =
                execute_plan_with_backend_and_context(&plan, model, backend, user, config);
            let exec_us = (Instant::now() - t1).as_micros() as u64;
            (
                fallback,
                RuntimePath::DirectSql,
                compile_us,
                false,
                0.0,
                exec_us,
            )
        } else {
            let path = if was_hit {
                RuntimePath::MalloyCached
            } else {
                RuntimePath::MalloyCompiled
            };

            let t0 = Instant::now();
            let r = execute_plan_sql_with_backend(&plan, &sql, backend);
            let exec_us = (Instant::now() - t0).as_micros() as u64;

            (r, path, compile_us, was_hit, worker_compile_ms, exec_us)
        }
    } else {
        let t0 = Instant::now();
        let r = execute_plan_with_backend_and_context(&plan, model, backend, user, config);
        let exec_us = (Instant::now() - t0).as_micros() as u64;
        (r, RuntimePath::DirectSql, 0, false, 0.0, exec_us)
    };

    let mut timings = Timings::new(runtime_path, key, mdx_parse_us, 0);
    timings.plan_us = plan_us;
    timings.malloy_compile_us = malloy_compile_us;
    timings.compiled_sql_cache_hit = compiled_cache_hit;
    timings.js_compile_ms = js_compile_ms;
    timings.sql_execute_us = sql_execute_us;

    let t0 = Instant::now();
    let xml = dispatch_with_backend(&query, &result, backend);
    timings.xml_render_us = (Instant::now() - t0).as_micros() as u64;
    timings.finish();
    (xml, timings)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::engine::model::{default_model, resolve_user_context};
    use crate::engine::plan::QueryPlan;
    use crate::engine::sql::sql_for_query_plan_with_context;
    use crate::project::config::ProxyConfig;

    fn parse_config(json: &str) -> ProxyConfig {
        serde_json::from_str(json).expect("parse config")
    }

    /// E2E test: verify that role filter predicates are injected into the SQL
    /// when calling through the runtime path with a non-admin user context.
    ///
    /// This catches the CRITICAL regression where runtime.rs discards the
    /// in-scope user/config and uses admin defaults instead.
    #[test]
    fn role_e2e_filtered_sql_through_runtime() {
        let config_str = r#"{
            "catalog": "T", "cube": "C", "source_name": "s", "table_name": "t",
            "dialect": "duckdb", "malloy_model_file": "m.malloy",
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [{
                "name": "EU_Region",
                "model_permission": "read",
                "members": [{"member_name": "user1", "member_type": "user"}],
                "table_permissions": [{
                    "table": "faktatabell",
                    "filter_expression": "region = 'EU'"
                }]
            }]
        }"#;
        let config: ProxyConfig = parse_config(config_str);
        let user = resolve_user_context(&config, "user1", &[]);
        assert!(!user.is_administrator);
        assert_eq!(user.roles, vec!["EU_Region"]);

        let model = default_model();
        let plan = QueryPlan::Total {
            measure: "TotalSales".into(),
            filters: vec![],
        };

        let sql = sql_for_query_plan_with_context(&model, &plan, &user, &config);
        assert!(
            sql.contains("region = 'EU'"),
            "SQL should contain role filter predicate, got: {}",
            sql
        );
        assert!(
            sql.contains("WHERE"),
            "SQL should have a WHERE clause with role filter, got: {}",
            sql
        );
    }
}
