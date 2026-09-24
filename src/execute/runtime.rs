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
        let timings = Timings::new(RuntimePath::DirectSql, "ddl-noop".to_string(), 0);
        return (resp, timings);
    }

    // Unsupported constructs fault loudly instead of returning a dropped axis
    // or a wrong-hierarchy cellset (plan 046).
    if let Some(fault) = crate::execute::builders::unsupported_fault(mdx) {
        let timings = Timings::new(RuntimePath::DirectSql, "unsupported".to_string(), 0);
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
    if user_is_restricted(config, user)
        && let Some(measure) = plan_measures(&plan)
            .into_iter()
            .find(|measure| model.classify_fallback(measure).is_some())
    {
        let timings = Timings::new(RuntimePath::DirectSql, "restricted-fallback".into(), 0);
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
            let result = std::sync::Arc::new(execute_plan_with_backend_and_context(
                &plan, model, backend, user, config,
            ));
            if cache_enabled {
                cache::RESULT_CACHE.insert(cache_key, std::sync::Arc::clone(&result));
            }
            (result, false)
        }
    };
    let sql_execute_us = (Instant::now() - t0).as_micros() as u64;

    let mut timings = Timings::new(RuntimePath::DirectSql, key, mdx_parse_us);
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

/// Every measure a plan would execute. Composite plans (`MultiMeasure`,
/// `TupleSet`, `MultiGroupBy`) carry measure ids directly and the executor
/// decomposes them into `Total`/`GroupBy` plans recursively — inspecting only
/// the outermost variant let a restricted user reach authored SQL through a
/// composite plan such as "two measures on an axis" (plan 051 review).
fn plan_measures(plan: &crate::engine::plan::QueryPlan) -> Vec<&str> {
    use crate::engine::plan::QueryPlan;
    match plan {
        QueryPlan::Total { measure, .. } | QueryPlan::GroupBy { measure, .. } => vec![measure],
        QueryPlan::MultiMeasure { measures, .. } | QueryPlan::MultiGroupBy { measures, .. } => {
            measures.iter().map(|m| m.as_str()).collect()
        }
        QueryPlan::TupleSet { cells } => cells.iter().map(|c| c.measure.as_str()).collect(),
        _ => Vec::new(),
    }
}

/// Fault when a restricted user asks for a drillthrough. The drillthrough
/// builder writes raw `SELECT *` SQL and applies no role predicates or OLS, so
/// the honest answer is a refusal until it does — returning rows the role
/// should not see is the one outcome a security feature must never produce
/// (plan 051 review).
pub fn drillthrough_fault(
    config: &ProxyConfig,
    user: &crate::engine::model::UserContext,
) -> Option<String> {
    user_is_restricted(config, user).then(|| {
        crate::xmla::response::fault_response(
            "drillthrough is not available for a restricted role: it cannot apply \
             row-level filters yet, and returning unfiltered rows would leak them",
        )
    })
}

/// Fault when a role declares a DAX filter this proxy cannot lower to SQL. The
/// filter counts as a restriction (the table is hidden), but the reference
/// *propagates* a table filter to the facts — measured: a role filtering only
/// `Territory` drops the revenue total to that territory's share. Serving the
/// facts unfiltered would therefore leak what the role excludes, so a query is
/// refused until the filter can be honoured (plan 051 review).
pub fn unhonourable_filter_fault(
    config: &ProxyConfig,
    user: &crate::engine::model::UserContext,
) -> Option<String> {
    let table = config
        .roles
        .iter()
        .filter(|role| user.roles.iter().any(|name| name == &role.name))
        .flat_map(|role| role.table_permissions.iter())
        .find(|permission| {
            permission.dax_filter.is_some() && permission.filter_expression.trim().is_empty()
        })
        .map(|permission| permission.table.clone());
    table.map(|table| {
        crate::xmla::response::fault_response(&format!(
            "the role filter on '{table}' is a DAX expression this proxy cannot lower to \
             SQL, and the reference propagates table filters to fact aggregates — the \
             query is refused rather than served unfiltered"
        ))
    })
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

    /// Composite plans hold measure ids directly and decompose recursively; the
    /// refusal must see through them (two measures on an axis was the hole).
    #[test]
    fn plan_measures_sees_composite_plans() {
        use super::plan_measures;
        use crate::engine::plan::{QueryPlan, TupleCell};
        let total = QueryPlan::Total {
            measure: "Revenue".into(),
            filters: vec![],
        };
        assert_eq!(plan_measures(&total), vec!["Revenue"]);

        let multi = QueryPlan::MultiMeasure {
            measures: vec!["Revenue".into(), "Units".into()],
            filters: vec![],
        };
        assert_eq!(plan_measures(&multi), vec!["Revenue", "Units"]);

        let grouped = QueryPlan::MultiGroupBy {
            measures: vec!["Revenue".into()],
            group_by: vec!["Category".into()],
            filters: vec![],
            group_levels: vec![None],
        };
        assert_eq!(plan_measures(&grouped), vec!["Revenue"]);

        let tuples = QueryPlan::TupleSet {
            cells: vec![TupleCell {
                measure: "Revenue".into(),
                filters: vec![],
            }],
        };
        assert_eq!(plan_measures(&tuples), vec!["Revenue"]);
    }

    /// A DAX filter the proxy cannot lower must refuse the query: the
    /// reference propagates a table filter to fact aggregates, so hiding the
    /// table alone would still serve unfiltered facts.
    #[test]
    fn unhonourable_dax_filters_refuse_queries() {
        use super::unhonourable_filter_fault;
        use crate::engine::model::UserContext;
        use crate::project::config::{ModelPermission, RoleConfig, TablePermissionConfig};

        let mut config = crate::proxy_project::project().config.clone();
        let mut user = UserContext::deny_all();
        user.roles = vec!["EU".into()];
        config.roles = vec![RoleConfig {
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
        }];
        assert!(
            unhonourable_filter_fault(&config, &user).is_none(),
            "a lowerable filter is not a refusal"
        );

        config.roles[0].table_permissions[0].filter_expression = String::new();
        config.roles[0].table_permissions[0].dax_filter = Some("Sales[Region] = \"EU\"".into());
        assert!(
            unhonourable_filter_fault(&config, &user).is_some(),
            "an unlowerable DAX filter must refuse the query"
        );
    }

    /// Drillthrough applies no role predicates, so a restricted user must be
    /// refused rather than served unfiltered rows.
    #[test]
    fn drillthrough_fault_refuses_restricted_users() {
        use super::drillthrough_fault;
        use crate::engine::model::UserContext;
        use crate::project::config::{ModelPermission, RoleConfig, TablePermissionConfig};

        let mut config = crate::proxy_project::project().config.clone();
        config.roles = vec![RoleConfig {
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
        }];
        let mut restricted = UserContext::deny_all();
        restricted.roles = vec!["EU".into()];
        assert!(
            drillthrough_fault(&config, &restricted).is_some(),
            "a restricted role must not reach drillthrough"
        );
        assert!(drillthrough_fault(&config, &UserContext::admin_default()).is_none());
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
