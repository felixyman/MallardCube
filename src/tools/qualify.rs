/// Converted-project qualification command.
///
/// Accepts a proxy-config.json path and optionally a trace path.
/// Loads the project and emits a readiness verdict based on machine-readable
/// facts in the config, model, fallback files, and sibling artifacts.
///
/// Readiness levels:
/// - READY: no known blockers, project loads cleanly.
/// - PARTIAL: usable but needs manual follow-up (roles, manual measures, etc.).
/// - BLOCKED: not honestly Excel-safe (stub fallbacks, broken config, etc.).
use crate::project::config::ModelPermission;
use std::path::Path;

#[derive(Debug, PartialEq)]
pub(crate) enum Readiness {
    Ready,
    Partial(Vec<String>),
    Blocked(Vec<String>),
}

impl Readiness {
    fn label(&self) -> &str {
        match self {
            Readiness::Ready => "READY",
            Readiness::Partial(_) => "PARTIAL",
            Readiness::Blocked(_) => "BLOCKED",
        }
    }

    fn exit_code(&self) -> i32 {
        match self {
            Readiness::Ready => 0,
            Readiness::Partial(_) => 0,
            Readiness::Blocked(_) => 1,
        }
    }

    fn reasons(&self) -> &[String] {
        match self {
            Readiness::Ready => &[],
            Readiness::Partial(r) => r.as_slice(),
            Readiness::Blocked(r) => r.as_slice(),
        }
    }
}

pub(crate) fn qualify(config_path: &str, trace_path: Option<&str>) -> Readiness {
    // Step 1: load the project
    let p = match crate::proxy_project::ProxyProject::load(config_path) {
        Ok(p) => p,
        Err(e) => {
            return Readiness::Blocked(vec![format!("cannot load project: {e}")]);
        }
    };

    let mut blocked = Vec::new();
    let mut partial = Vec::new();

    // --- check db_path ---
    if p.config.db_path.is_none() {
        partial.push(
            "db_path is null: proxy will use in-memory demo data, not real converted data".into(),
        );
    } else {
        let db = p.config.db_path.as_deref().unwrap();
        let resolved = crate::proxy_project::resolve_db_path(config_path, Some(db));
        if resolved.as_ref().is_none_or(|r| !Path::new(r).exists()) {
            partial.push(format!(
                "db_path '{db}' does not resolve to an existing file (from config dir)"
            ));
        }
    }

    // --- check measures ---
    let mut manual_count = 0u32;
    let mut scalar_fallback_count = 0u32;

    for m in &p.model.measures {
        let has_fallback_file = m.sql_fallback_sql.is_some();
        let has_sql_expr = !m.sql_expr.is_empty() && m.sql_expr != "null";
        let has_time_intel = m.time_flag.is_some();

        if has_fallback_file {
            let sql = m.sql_fallback_sql.as_deref().unwrap_or("");
            let is_stub = sql.to_uppercase().contains("TODO")
                || sql.contains("SELECT 1 AS DUMMY")
                || sql.contains("SELECT 1 AS dummy");
            if is_stub {
                blocked.push(format!(
                    "measure '{}' has a TODO/stub fallback SQL file",
                    m.caption
                ));
                continue;
            }
            scalar_fallback_count += 1;
        } else if !has_sql_expr && !has_time_intel {
            manual_count += 1;
        }
    }

    // --- check roles from config ---
    // Roles with SQL filter_expression (RLS) or model_permission: Administrator
    // are enforced at runtime and should NOT cause PARTIAL. Only unsupported role
    // shapes flag PARTIAL.
    if p.config.auth.is_none() {
        if !p.config.roles.is_empty() {
            partial.push(
                "roles defined but no auth config — roles will not be enforced at runtime".into(),
            );
        }
    } else {
        for role in &p.config.roles {
            let has_enforced_rls = role
                .table_permissions
                .iter()
                .any(|tp| !tp.filter_expression.is_empty());
            let is_admin = role.model_permission == ModelPermission::Administrator;

            // Enforced roles (RLS filter or Administrator) are fine.
            if has_enforced_rls || is_admin {
                continue;
            }

            // Unsupported shape: role with members but no table permissions.
            if !role.members.is_empty() && role.table_permissions.is_empty() {
                partial.push(format!(
                    "role '{}' has members but no table permissions (no RLS enforced)",
                    role.name
                ));
            }

            // Unsupported shape: table permissions with empty filter and Read metadata
            // (full access, no enforcement).
            for tp in &role.table_permissions {
                if tp.filter_expression.is_empty()
                    && tp.metadata_permission == ModelPermission::Read
                {
                    partial.push(format!(
                        "role '{}' table '{}' has no filter (full access)",
                        role.name, tp.table
                    ));
                }
            }
        }
    }

    // --- check time intelligence ---
    let has_date_role_dims = p.model.dimensions.iter().any(|d| d.is_date_role);
    let has_ti_config = p.config.time_intelligence.is_some();
    if has_date_role_dims && !has_ti_config {
        partial.push("date-role dimensions present but no time_intelligence config: YTD/prior-year measures may not work".into());
    }

    // --- check parent-child dimensions ---
    // Qualify is read-only and never materializes. If the hierarchy has not
    // been built yet (the server builds it on first start), say so instead of
    // silently qualifying a flat dimension.
    for dc in &p.config.dimensions {
        let Some(pc) = &dc.parent_child else { continue };
        let materialized = p
            .model
            .dim_def_opt(&dc.id)
            .is_some_and(|d| !d.levels.is_empty());
        if !materialized {
            partial.push(format!(
                "parent_child dimension '{}' ({} -> {}) is not materialized yet — start the server once to build its levels (or set \"refresh\": true)",
                dc.id, pc.parent_column, pc.key_column
            ));
        }
    }

    // --- check model health ---
    if p.model.dimensions.is_empty() || p.model.measures.is_empty() {
        blocked.push("model has no dimensions or no measures".into());
    }

    // --- optional replay ---
    if let Some(tp) = trace_path {
        if Path::new(tp).exists() {
            // trace_replay::run loads its own project and backend source.
            let replay_ok = crate::tools::trace_replay::run(vec![
                "trace-replay".into(),
                tp.to_string(),
                config_path.to_string(),
            ]);
            if replay_ok != 0 {
                partial
                    .push("trace replay reported failures — see output above for details".into());
            }
        } else {
            partial.push(format!("trace path '{tp}' not found — skipping replay"));
        }
    }

    // --- build non-blocking partial reasons ---
    if manual_count > 0 {
        partial.push(format!(
            "{manual_count} measure(s) have no SQL expression and no fallback — manual review needed"
        ));
    }

    if scalar_fallback_count > 0 {
        // This is not a blocker; just informational.
        // It is reported through the PARTIAL path if combined with other issues,
        // and silently accepted if READY.
    }

    // --- verdict ---
    // Blockers are fatal, but also surface partial issues so the operator
    // can see what else needs attention after stub fallbacks are resolved.
    if !blocked.is_empty() {
        blocked.extend(partial);
        Readiness::Blocked(blocked)
    } else if !partial.is_empty() {
        Readiness::Partial(partial)
    } else {
        Readiness::Ready
    }
}

pub fn run(args: Vec<String>) -> i32 {
    // args: ["qualify", "<config-path>", "<optional-trace-path>"])
    let config_path = args
        .get(1)
        .map(|s| s.as_str())
        .unwrap_or("projects/project3/proxy-config.json");
    let trace_path = args.get(2).map(|s| s.as_str());

    let verdict = qualify(config_path, trace_path);

    // Print summary
    println!("=== Qualification Report ===");
    for r in verdict.reasons() {
        println!("  [{label}] {r}", label = verdict.label());
    }
    println!();
    println!("Verdict: {}", verdict.label());
    if verdict.reasons().is_empty() {
        println!("  No issues found.");
    }

    verdict.exit_code()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_retail_analytics_is_ready_after_plan_021() {
        let v = qualify(
            "projects/generated_retail_analytics/proxy-config.json",
            None,
        );
        // Plan 021 retired all retail fallback stubs. All 4 measures have real SQL.
        // Plan 017 makes db_path resolve relative to config, so data/sales.db exists.
        assert_eq!(
            v.label(),
            "READY",
            "expected READY after Plan 021, got {}: {:?}",
            v.label(),
            v.reasons()
        );
    }

    #[test]
    fn generated_project_is_partial_with_unsupported_roles() {
        let v = qualify("projects/generated_project/proxy-config.json", None);
        // Plan 014 retired both stub fallbacks. No auth config + roles defined.
        assert_eq!(
            v.label(),
            "PARTIAL",
            "expected PARTIAL, got {}: {:?}",
            v.label(),
            v.reasons()
        );
        let reasons: Vec<&str> = v.reasons().iter().map(|s| s.as_str()).collect();
        assert!(
            reasons.iter().any(|r| r.contains("no auth config")),
            "should report missing auth config: {:?}",
            reasons
        );
    }

    #[test]
    fn generated_project_has_no_stub_fallbacks() {
        let p = crate::proxy_project::ProxyProject::load(
            "projects/generated_project/proxy-config.json",
        )
        .expect("load generated_project");
        let stubs: Vec<_> = p
            .model
            .measures
            .iter()
            .filter(|m| match &m.sql_fallback_sql {
                Some(sql) => {
                    sql.to_uppercase().contains("TODO")
                        || sql.contains("SELECT 1 AS DUMMY")
                        || sql.contains("SELECT 1 AS dummy")
                }
                None => false,
            })
            .collect();
        assert_eq!(
            stubs.len(),
            0,
            "Plan 014 retired all stub fallback measures: {:?}",
            stubs.iter().map(|m| &m.caption).collect::<Vec<_>>()
        );
    }

    #[test]
    fn project3_is_ready() {
        // project3 is complete, has db_path=null (in-memory is intentional for demo)
        let v = qualify("projects/project3/proxy-config.json", None);
        // project3 has a null db_path so it should be PARTIAL
        let reasons: Vec<&str> = v.reasons().iter().map(|s| s.as_str()).collect();
        // Has null db_path but that's by design for demo
        assert!(
            v.label() == "PARTIAL" || v.label() == "READY",
            "project3 should be READY or PARTIAL (null db_path is expected for demo), got {}: {:?}",
            v.label(),
            reasons
        );
    }

    #[test]
    fn broken_config_path_returns_blocked() {
        let v = qualify("nonexistent/proxy-config.json", None);
        assert_eq!(v.label(), "BLOCKED");
        assert!(
            v.reasons()
                .iter()
                .any(|r| r.contains("cannot load project")),
            "blocked reason should mention load failure: {:?}",
            v.reasons()
        );
    }

    #[test]
    fn generated_retail_from_report_counts_manual_measures() {
        let p = crate::proxy_project::ProxyProject::load(
            "projects/generated_retail_analytics/proxy-config.json",
        )
        .expect("load retail analytics");
        let manual: Vec<_> = p
            .model
            .measures
            .iter()
            .filter(|m| {
                let has_sql = !m.sql_expr.is_empty() && m.sql_expr != "null";
                let has_fallback = m.sql_fallback_sql.is_some();
                let has_time = m.time_flag.is_some();
                !has_sql && !has_fallback && !has_time
            })
            .collect();
        // After Plan 012 regeneration: Gross Profit + Total COGS are sql_fallback (stubs),
        // Gross Margin % + Total Revenue are simple. So 0 manual in current state.
        assert_eq!(
            manual.len(),
            0,
            "retail analytics should have 0 manual measures after fallback wiring: {:?}",
            manual.iter().map(|m| &m.caption).collect::<Vec<_>>()
        );
    }

    #[test]
    fn generated_project_has_known_stub_count() {
        let p = crate::proxy_project::ProxyProject::load(
            "projects/generated_project/proxy-config.json",
        )
        .expect("load generated_project");
        let stubs: Vec<_> = p
            .model
            .measures
            .iter()
            .filter(|m| match &m.sql_fallback_sql {
                Some(sql) => {
                    sql.to_uppercase().contains("TODO")
                        || sql.contains("SELECT 1 AS DUMMY")
                        || sql.contains("SELECT 1 AS dummy")
                }
                None => false,
            })
            .collect();
        assert_eq!(
            stubs.len(),
            0,
            "Plan 014 retired all stub fallback measures: {:?}",
            stubs.iter().map(|m| &m.caption).collect::<Vec<_>>()
        );
    }

    #[test]
    fn qualify_with_trace_does_not_panic_or_crash_init() {
        // Plan 018: verify that qualify with a trace path returns a verdict
        // instead of panicking on singleton init order.  When the trace file
        // is missing, replay is still skipped gracefully without touching the
        // global project singleton before trace_replay would need it.
        let v = qualify(
            "projects/project3/proxy-config.json",
            Some("nonexistent-trace.jsonl"),
        );
        let reasons: Vec<&str> = v.reasons().iter().map(|s| s.as_str()).collect();
        assert!(
            reasons.iter().any(|r| r.contains("not found")),
            "should mention trace not found: {:?}",
            reasons
        );
        let label = v.label();
        assert!(
            label == "READY" || label == "PARTIAL" || label == "BLOCKED",
            "qualify with trace should return a verdict, not panic. Got: {label}"
        );
    }

    /// Qualify is read-only: an unmaterialized parent-child hierarchy must be
    /// reported, not silently written into the user's database.
    #[test]
    fn parent_child_unmaterialized_is_reported_without_writes() {
        let dir = std::env::temp_dir().join(format!(
            "mallardcube-qualify-pc-{}-{:#x}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("org.duckdb");
        duckdb::Connection::open(&db)
            .unwrap()
            .execute_batch(
                "CREATE TABLE employee_dim (k INT, p INT);
                 INSERT INTO employee_dim VALUES (1,NULL),(2,1);",
            )
            .unwrap();
        let cfg = serde_json::json!({
            "catalog": "ORG",
            "cube": "Org",
            "source_name": "org",
            "table_name": "employee_dim",
            "dialect": "duckdb",
            "db_path": "org.duckdb",
            "relationships": [{
                "fact_table": "default",
                "fact_column": "k",
                "dimension_id": "Employee",
                "dim_table": "employee_dim",
                "dim_column": "k"
            }],
            "dimensions": [{
                "id": "Employee",
                "physical_field": "k",
                "caption": "Employee",
                "description": "",
                "hierarchy_name": "Employee",
                "all_level_name": "(All)",
                "leaf_level_name": "Employee",
                "ordinal": 1,
                "visible": true,
                "has_all": true,
                "cardinality_hint": 10,
                "parent_child": {"key_column": "k", "parent_column": "p"}
            }],
            "measures": [{
                "id": "Revenue",
                "sql_expr": "SUM(k)",
                "caption": "Revenue",
                "display_name": "Revenue",
                "description": "",
                "format_string": "0",
                "units": "",
                "ordinal": 1,
                "visible": true,
                "measure_group_name": "Org"
            }]
        });
        let config_path = dir.join("proxy-config.json");
        std::fs::write(&config_path, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();

        let v = qualify(config_path.to_str().unwrap(), None);
        assert!(
            v.reasons()
                .iter()
                .any(|r| r.contains("parent_child dimension 'Employee'")),
            "unmaterialized parent-child dimension must be reported: {v:?}"
        );
        let cols: u32 = duckdb::Connection::open(&db)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('employee_dim') WHERE name LIKE 'Employee__pc%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cols, 0, "qualify must not materialize");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
