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
use crate::execute::cache;
use crate::execute::render::dispatch_with_backend;
use crate::project::config::ProxyConfig;
use std::time::Instant;

pub fn get_execute_cellset_response_with_backend_and_context<B: QueryBackend + ?Sized>(
    mdx: &str,
    backend: &B,
    user: &UserContext,
    config: &ProxyConfig,
) -> (String, Timings) {
    // Excel's pivot Refresh issues `REFRESH CUBE [<cube>]`; the data is live,
    // so answer with an empty success instead of a fault (plan 048).
    if let Some(resp) = crate::execute::builders::ddl_noop_response(mdx) {
        let timings = Timings::new(RuntimePath::DirectSql, "ddl-noop".to_string(), 0, 0);
        return (resp, timings);
    }

    // Unsupported constructs fault loudly instead of returning a dropped axis
    // or a wrong-hierarchy cellset (plan 046).
    if let Some(fault) = crate::execute::builders::unsupported_fault(mdx) {
        let timings = Timings::new(RuntimePath::DirectSql, "unsupported".to_string(), 0, 0);
        return (fault, timings);
    }

    let t0 = Instant::now();
    let query = crate::mdx_semantic::semantic_query_from_mdx(mdx);
    let mdx_parse_us = (Instant::now() - t0).as_micros() as u64;

    let t0 = Instant::now();
    let model = &crate::proxy_project::project().model;
    let plan = plan_from_semantic_with_model_and_context(&query, model, user, config);
    let plan_us = (Instant::now() - t0).as_micros() as u64;

    // Authored (fallback) SQL is pre-written and carries no role predicates, so
    // a restricted user must not reach it: refusing the query is the honest
    // answer, where running it returned unfiltered rows (the limitation that
    // used to be documented in plan.rs).
    if let Some(measure) = fallback_measure_in_plan(&plan)
        && model.classify_fallback(measure).is_some()
        && user_is_restricted(config, user)
    {
        let timings = Timings::new(RuntimePath::DirectSql, "restricted-fallback".into(), 0, 0);
        return (
            crate::xmla::response::fault_response(&format!(
                "measure '{measure}' uses authored SQL that cannot be filtered for the requesting \
                 role; the query is refused rather than returning unfiltered rows"
            )),
            timings,
        );
    }

    let key = plan_key(&plan);

    // Excel repeats the same query once per CELL PROPERTIES variant; serve the
    // repeats from a short-lived cache (plan 032). The cellset is rendered
    // fresh below, so every variant keeps its own cell properties.
    let cache_enabled = cache::enabled();
    let cache_key = cache::cache_key(&key, &config.catalog, &config.cube, user);
    let t0 = Instant::now();
    let cached = if cache_enabled {
        cache::RESULT_CACHE.get(&cache_key)
    } else {
        None
    };
    let (result, cache_hit) = match cached {
        Some(hit) => (hit, true),
        None => {
            let result = execute_plan_with_backend_and_context(&plan, model, backend, user, config);
            if cache_enabled {
                cache::RESULT_CACHE.insert(cache_key, result.clone());
            }
            (result, false)
        }
    };
    let sql_execute_us = (Instant::now() - t0).as_micros() as u64;

    let mut timings = Timings::new(RuntimePath::DirectSql, key, mdx_parse_us, 0);
    timings.plan_us = plan_us;
    timings.cache_hit = cache_hit;
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

/// The measure a plan would execute through authored (fallback) SQL, if any.
fn fallback_measure_in_plan(plan: &crate::engine::plan::QueryPlan) -> Option<&str> {
    use crate::engine::plan::QueryPlan;
    match plan {
        QueryPlan::Total { measure, .. } | QueryPlan::GroupBy { measure, .. } => Some(measure),
        _ => None,
    }
}

/// Does this user's access get narrowed by any of their roles? Administrators
/// and users without roles are unrestricted.
fn user_is_restricted(config: &ProxyConfig, user: &crate::engine::model::UserContext) -> bool {
    if user.is_administrator {
        return false;
    }
    config
        .roles
        .iter()
        .filter(|role| user.roles.iter().any(|name| name == &role.name))
        .any(|role| role.narrows_access())
}

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
            "dialect": "duckdb", 
            "dimensions": [], "measures": [],
            "auth": { "trusted_proxy": true },
            "roles": [{
                "name": "EU_Region",
                "model_permission": "read",
                "members": [{"member_name": "user1", "member_type": "user"}],
                "table_permissions": [{
                    "table": "fact_table",
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

    /// A role that narrows access marks its holders restricted — which is what
    /// refuses authored (fallback) SQL, since that SQL carries no role
    /// predicates. Administrators and role-less users stay unrestricted, so a
    /// trusted single-user deployment is unchanged.
    #[test]
    fn restricted_roles_mark_their_holders_restricted() {
        use super::user_is_restricted;
        use crate::engine::model::UserContext;
        use crate::project::config::{ModelPermission, RoleConfig, TablePermissionConfig};

        let mut config = crate::proxy_project::project().config.clone();
        config.roles = vec![
            RoleConfig {
                name: "EU".into(),
                description: String::new(),
                model_permission: ModelPermission::Read,
                members: vec![],
                table_permissions: vec![TablePermissionConfig {
                    table: "sales_fact".into(),
                    filter_expression: "territory = 'North'".into(),
                    dax_filter: None,
                    metadata_permission: ModelPermission::Read,
                }],
            },
            RoleConfig {
                name: "Ops".into(),
                description: String::new(),
                model_permission: ModelPermission::Administrator,
                members: vec![],
                table_permissions: vec![],
            },
        ];

        assert!(config.any_role_narrows_access(), "EU filters a table");

        let mut restricted = UserContext::deny_all();
        restricted.roles = vec!["EU".into()];
        assert!(user_is_restricted(&config, &restricted));

        let mut administrator = UserContext::deny_all();
        administrator.roles = vec!["Ops".into()];
        assert!(
            !user_is_restricted(&config, &administrator),
            "an administrator role narrows nothing"
        );

        let mut unrelated = UserContext::deny_all();
        unrelated.roles = vec!["Finance".into()];
        assert!(!user_is_restricted(&config, &unrelated));

        assert!(!user_is_restricted(&config, &UserContext::admin_default()));
    }
}
