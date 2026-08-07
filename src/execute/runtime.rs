/// Execute request entry point — direct SQL runtime.
///
/// Parses MDX, builds a role-aware query plan, executes via DuckDB,
/// and renders the XMLA cellset response with timing instrumentation.
/// Called by `main.rs` (production) and `builders.rs` (test seam).
use crate::backend::QueryBackend;
use crate::engine::model::UserContext;
use crate::engine::normalize::plan_key;
use crate::engine::plan::{
    execute_plan_with_backend_and_context, plan_from_semantic_with_model_and_context,
};
use crate::engine::timing::{RuntimePath, Timings};
use crate::execute::render::dispatch_with_backend;
use crate::project::config::ProxyConfig;
use std::time::Instant;

pub fn get_execute_cellset_response_with_backend_and_context<B: QueryBackend + ?Sized>(
    mdx: &str,
    backend: &B,
    user: &UserContext,
    config: &ProxyConfig,
) -> (String, Timings) {
    let t0 = Instant::now();
    let query = crate::mdx_semantic::semantic_query_from_mdx(mdx);
    let mdx_parse_us = (Instant::now() - t0).as_micros() as u64;

    let t0 = Instant::now();
    let model = &crate::proxy_project::project().model;
    let plan = plan_from_semantic_with_model_and_context(&query, model, user, config);
    let plan_us = (Instant::now() - t0).as_micros() as u64;
    let key = plan_key(&plan);

    let t0 = Instant::now();
    let result = execute_plan_with_backend_and_context(&plan, model, backend, user, config);
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
