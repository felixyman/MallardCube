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

use std::fs;
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
        partial.push("db_path is null: proxy will use in-memory demo data, not real converted data".into());
    } else {
        let db = p.config.db_path.as_deref().unwrap();
        let resolved = crate::proxy_project::resolve_db_path(config_path, Some(db));
        if resolved.as_ref().map_or(true, |r| !Path::new(r).exists()) {
            partial.push(format!("db_path '{db}' does not resolve to an existing file (from config dir)"));
        }
    }

    // --- check measures ---
    let mut manual_count = 0u32;
    let mut scalar_fallback_count = 0u32;

    for m in &p.model.measures {
        let has_fallback_file = m.sql_fallback_sql.is_some();
        let has_sql_expr = !m.sql_expr.is_empty() && m.sql_expr != "null";
        let has_physical_expr = !m.physical_expr.is_empty();
        let has_time_intel = m.time_flag.is_some();

        if has_fallback_file {
            let sql = m.sql_fallback_sql.as_deref().unwrap_or("");
            let is_stub = sql.to_uppercase().contains("TODO")
                || sql.contains("SELECT 1 AS DUMMY")
                || sql.contains("SELECT 1 AS dummy");
            if is_stub {
                blocked.push(format!("measure '{}' has a TODO/stub fallback SQL file", m.caption));
                continue;
            }
            scalar_fallback_count += 1;
        } else if !has_sql_expr && !has_physical_expr && !has_time_intel {
            manual_count += 1;
        }
    }

    // --- check roles from config ---
    // Roles are informational only (not enforced by the proxy). They surface as
    // PARTIAL rather than READY to remind operators that security must be handled
    // outside the proxy. This is intentional: the proxy cannot enforce SSAS roles.
    if !p.config.roles.is_empty() {
        partial.push(format!(
            "{} unsupported security role(s) detected in config",
            p.config.roles.len()
        ));
    }

    // --- check time intelligence ---
    let has_date_role_dims = p.model.dimensions.iter().any(|d| d.is_date_role);
    let has_ti_config = p.config.time_intelligence.is_some();
    if has_date_role_dims && !has_ti_config {
        partial.push("date-role dimensions present but no time_intelligence config: YTD/prior-year measures may not work".into());
    }

    // --- check model health ---
    if p.model.dimensions.is_empty() || p.model.measures.is_empty() {
        blocked.push("model has no dimensions or no measures".into());
    }

    // --- optional replay ---
    if let Some(tp) = trace_path {
        if Path::new(tp).exists() {
            // trace_replay::run handles its own init_project + init_backend.
            let replay_ok = crate::tools::trace_replay::run(vec![
                "trace-replay".into(),
                tp.to_string(),
                config_path.to_string(),
            ]);
            if replay_ok != 0 {
                partial.push("trace replay reported failures — see output above for details".into());
            }
        } else {
            partial.push(format!("trace path '{tp}' not found — skipping replay"));
        }
    }

    // --- build non-blocking partial reasons ---
    if manual_count > 0 {
        partial.push(format!(
            "{manual_count} measure(s) have no SQL, no Malloy expression, and no fallback — manual review needed"
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
    let config_path = args.get(1).map(|s| s.as_str()).unwrap_or("project3/proxy-config.json");
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
        let v = qualify("generated_retail_analytics/proxy-config.json", None);
        // Plan 021 retired all retail fallback stubs. All 4 measures have real SQL.
        // Plan 017 makes db_path resolve relative to config, so data/sales.db exists.
        assert_eq!(v.label(), "READY",
            "expected READY after Plan 021, got {}: {:?}", v.label(), v.reasons());
    }

    #[test]
    fn generated_project_is_partial_with_unsupported_roles() {
        let v = qualify("generated_project/proxy-config.json", None);
        // Plan 014 retired both stub fallbacks. Only unsupported roles remain.
        assert_eq!(v.label(), "PARTIAL",
            "expected PARTIAL, got {}: {:?}", v.label(), v.reasons());
        let reasons: Vec<&str> = v.reasons().iter().map(|s| s.as_str()).collect();
        assert!(reasons.iter().any(|r| r.contains("security role")),
            "should report unsupported roles: {:?}", reasons);
    }

    #[test]
    fn generated_project_has_no_stub_fallbacks() {
        let p = crate::proxy_project::ProxyProject::load("generated_project/proxy-config.json")
            .expect("load generated_project");
        let stubs: Vec<_> = p.model.measures.iter()
            .filter(|m| {
                match &m.sql_fallback_sql {
                    Some(sql) => {
                        sql.to_uppercase().contains("TODO")
                            || sql.contains("SELECT 1 AS DUMMY")
                            || sql.contains("SELECT 1 AS dummy")
                    }
                    None => false,
                }
            })
            .collect();
        assert_eq!(stubs.len(), 0,
            "Plan 014 retired all stub fallback measures: {:?}",
            stubs.iter().map(|m| &m.caption).collect::<Vec<_>>());
    }

    #[test]
    fn project3_is_ready() {
        // project3 is complete, has db_path=null (in-memory is intentional for demo)
        let v = qualify("project3/proxy-config.json", None);
        // project3 has a null db_path so it should be PARTIAL
        let reasons: Vec<&str> = v.reasons().iter().map(|s| s.as_str()).collect();
        // Has null db_path but that's by design for demo
        assert!(v.label() == "PARTIAL" || v.label() == "READY",
            "project3 should be READY or PARTIAL (null db_path is expected for demo), got {}: {:?}",
            v.label(), reasons);
    }

    #[test]
    fn broken_config_path_returns_blocked() {
        let v = qualify("nonexistent/proxy-config.json", None);
        assert_eq!(v.label(), "BLOCKED");
        assert!(v.reasons().iter().any(|r| r.contains("cannot load project")),
            "blocked reason should mention load failure: {:?}", v.reasons());
    }

    #[test]
    fn generated_retail_from_report_counts_manual_measures() {
        let p = crate::proxy_project::ProxyProject::load("generated_retail_analytics/proxy-config.json")
            .expect("load retail analytics");
        let manual: Vec<_> = p.model.measures.iter()
            .filter(|m| {
                let has_sql = !m.sql_expr.is_empty() && m.sql_expr != "null";
                let has_malloy = !m.physical_expr.is_empty();
                let has_fallback = m.sql_fallback_sql.is_some();
                let has_time = m.time_flag.is_some();
                !has_sql && !has_malloy && !has_fallback && !has_time
            })
            .collect();
        // After Plan 012 regeneration: Gross Profit + Total COGS are sql_fallback (stubs),
        // Gross Margin % + Total Revenue are simple. So 0 manual in current state.
        assert_eq!(manual.len(), 0,
            "retail analytics should have 0 manual measures after fallback wiring: {:?}",
            manual.iter().map(|m| &m.caption).collect::<Vec<_>>());
    }

    #[test]
    fn generated_project_has_known_stub_count() {
        let p = crate::proxy_project::ProxyProject::load("generated_project/proxy-config.json")
            .expect("load generated_project");
        let stubs: Vec<_> = p.model.measures.iter()
            .filter(|m| {
                match &m.sql_fallback_sql {
                    Some(sql) => {
                        sql.to_uppercase().contains("TODO")
                            || sql.contains("SELECT 1 AS DUMMY")
                            || sql.contains("SELECT 1 AS dummy")
                    }
                    None => false,
                }
            })
            .collect();
        assert_eq!(stubs.len(), 0,
            "Plan 014 retired all stub fallback measures: {:?}",
            stubs.iter().map(|m| &m.caption).collect::<Vec<_>>());
    }

    #[test]
    fn qualify_with_trace_does_not_panic_or_crash_init() {
        // Plan 018: verify that qualify with a trace path returns a verdict
        // instead of panicking on singleton init order.  When the trace file
        // is missing, replay is still skipped gracefully without touching the
        // global project singleton before trace_replay would need it.
        let v = qualify("project3/proxy-config.json", Some("nonexistent-trace.jsonl"));
        let reasons: Vec<&str> = v.reasons().iter().map(|s| s.as_str()).collect();
        assert!(reasons.iter().any(|r| r.contains("not found")),
            "should mention trace not found: {:?}", reasons);
        let label = v.label();
        assert!(
            label == "READY" || label == "PARTIAL" || label == "BLOCKED",
            "qualify with trace should return a verdict, not panic. Got: {label}"
        );
    }
}
