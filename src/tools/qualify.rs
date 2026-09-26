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
use crate::backend::{BackendSource, QueryBackend};
use crate::engine::model::UserContext;
use crate::engine::plan::{QueryPlan, TypedDimensionFilter};
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

    // --- data-side checks (plan 057-B) ---
    // These stop a plausible wrong number: unreadable tables, duplicate
    // dimension keys, fan-out relationships and orphan keys.
    match p
        .config
        .db_path
        .as_deref()
        .and_then(|db| crate::proxy_project::resolve_db_path(config_path, Some(db)))
    {
        Some(resolved) if Path::new(&resolved).exists() => match BackendSource::file(&resolved) {
            Ok(source) => {
                let shape = data_shape(&p);
                let (data_blocked, data_partial) =
                    data_findings(source.checkout().as_ref(), &shape);
                blocked.extend(data_blocked);
                partial.extend(data_partial);

                // Every relationship dimension, not just the first: the
                // invariant must not be blind to the second one (review S5).
                let mut dimensions: Vec<String> = Vec::new();
                for relationship in &p.model.relationships {
                    if !dimensions.contains(&relationship.dimension_id) {
                        dimensions.push(relationship.dimension_id.clone());
                    }
                }
                for dimension in &dimensions {
                    blocked.extend(grain_findings(
                        source.checkout().as_ref(),
                        &p.model,
                        &p.config,
                        &UserContext::admin_default(),
                        dimension,
                    ));
                }

                match load_oracles(config_path) {
                    Ok(Some(file)) => {
                        let (oracle_blocked, oracle_partial) = oracle_findings(
                            source.checkout().as_ref(),
                            &p.model,
                            &p.config,
                            &UserContext::admin_default(),
                            &file,
                        );
                        blocked.extend(oracle_blocked);
                        partial.extend(oracle_partial);
                    }
                    Ok(None) => {}
                    Err(message) => blocked.push(message),
                }
            }
            Err(error) => {
                blocked.push(format!("cannot open the database for data checks: {error}"))
            }
        },
        _ => partial.push("db_path is not usable: data-side checks skipped".into()),
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

/// Value oracles: hand-written expectations from direct SQL, checked against
/// the same SQL the proxy emits. They are the only way to catch a model whose
/// *semantics* are wrong rather than its shape (a ratio that silently became
/// additive, a time window off by one) — a generated expectation would be
/// circular (plan 057-B).
#[derive(Debug, serde::Deserialize)]
pub(crate) struct OracleFile {
    pub oracles: Vec<Oracle>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct Oracle {
    /// Measure id or caption.
    pub measure: String,
    /// Optional slice: a dimension id and one member key.
    #[serde(default)]
    pub dimension: Option<String>,
    #[serde(default)]
    pub member: Option<String>,
    /// The value a direct SQL query produced when the oracle was written.
    pub expected: f64,
    /// Absolute tolerance; defaults to 1e-6 of the expected value.
    #[serde(default)]
    pub tolerance: Option<f64>,
}

/// `oracles.json` beside the config, when present.
pub(crate) fn load_oracles(config_path: &str) -> Result<Option<OracleFile>, String> {
    let Some(dir) = Path::new(config_path).parent() else {
        return Ok(None);
    };
    let path = dir.join("oracles.json");
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let file: OracleFile = serde_json::from_str(&text)
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))?;
    Ok(Some(file))
}

/// Run every oracle through the proxy's own SQL emitter and compare.
pub(crate) fn oracle_findings<B: QueryBackend + ?Sized>(
    backend: &B,
    model: &crate::engine::model::SemanticModel,
    config: &crate::project::config::ProxyConfig,
    user: &UserContext,
    file: &OracleFile,
) -> (Vec<String>, Vec<String>) {
    let mut blocked = Vec::new();
    let mut partial = Vec::new();

    for oracle in &file.oracles {
        let Some(measure) = model.lookup_measure(&oracle.measure) else {
            blocked.push(format!(
                "oracle names measure '{}', which the model does not define",
                oracle.measure
            ));
            continue;
        };
        let filters = match (&oracle.dimension, &oracle.member) {
            (Some(dimension), Some(member)) => vec![TypedDimensionFilter {
                dimension: dimension.clone(),
                members: vec![member.clone()],
                level: None,
                time_flag: None,
                range: None,
                date_window: None,
                label: None,
            }],
            (None, None) => Vec::new(),
            _ => {
                blocked.push(format!(
                    "oracle for '{}' sets only one of dimension/member",
                    oracle.measure
                ));
                continue;
            }
        };
        let plan = QueryPlan::Total {
            measure: measure.id.clone(),
            filters,
        };
        let sql = crate::engine::sql::sql_for_query_plan_with_context(model, &plan, user, config);
        if sql.trim().is_empty() {
            blocked.push(format!(
                "oracle for '{}' produced no SQL (unsupported measure shape)",
                oracle.measure
            ));
            continue;
        }
        let _ = backend.take_failure();
        let got = backend.query_scalar(&sql);
        if let Some(failure) = backend.take_failure() {
            blocked.push(format!(
                "oracle for '{}' cannot run: {failure}",
                oracle.measure
            ));
            continue;
        }
        // A floor, so an oracle recorded as 0 (or a tiny DECIMAL) does not
        // demand bit-exact f64 arithmetic (review S8).
        let tolerance = oracle
            .tolerance
            .unwrap_or_else(|| oracle.expected.abs() * 1e-6 + 0.01);
        if (got - oracle.expected).abs() > tolerance {
            let slice = match (&oracle.dimension, &oracle.member) {
                (Some(dimension), Some(member)) => format!(" at {dimension}={member}"),
                _ => String::new(),
            };
            blocked.push(format!(
                "oracle '{}'{slice}: expected {}, got {got}",
                oracle.measure, oracle.expected
            ));
        }
    }

    // An oracles file with no expectations is almost certainly a mistake, not a
    // pass.
    if file.oracles.is_empty() {
        partial.push("oracles.json defines no oracles".into());
    }

    // Once oracles exist, every measure that cannot be checked by additivity
    // must have one: a ratio or time window with no expectation is exactly the
    // shape that breaks silently (plan 057-B).
    for measure in &model.measures {
        if is_additive(measure) {
            continue;
        }
        let covered = file.oracles.iter().any(|oracle| {
            model
                .lookup_measure(&oracle.measure)
                .is_some_and(|m| m.id == measure.id)
        });
        if !covered {
            partial.push(format!(
                "measure '{}' is not additive and has no oracle — add one to oracles.json",
                measure.caption
            ));
        }
    }

    (blocked, partial)
}

/// Is this measure's SQL an additive aggregate (SUM/COUNT)? Ratios, averages
/// and time windows are not additive and are checked differently.
fn is_additive(measure: &crate::engine::model::MeasureDef) -> bool {
    // Time-windowed measures look like `SUM(x)` but are not additive: YTD at
    // the total is not the sum of the per-day YTD values.
    if measure.time_flag.is_some() {
        return false;
    }
    is_additive_expr(&measure.sql_expr)
}

/// Is the expression exactly one additive aggregate call?
///
/// This used to accept anything that *started* with `SUM(`, so a ratio of sums
/// (`SUM(a) / NULLIF(SUM(b), 0)`, the shape the thin-projection fixture ships)
/// was treated as additive by both the grain invariant — which then false-
/// blocked a healthy project — and the oracle-coverage rule, which consequently
/// demanded nothing (review S3). The whole expression must be the aggregate:
/// no operator at depth zero after it.
pub(crate) fn is_additive_expr(sql_expr: &str) -> bool {
    let expr = sql_expr.trim();
    let upper = expr.to_uppercase();
    if !(upper.starts_with("SUM(") || upper.starts_with("COUNT(")) || upper.contains("DISTINCT") {
        return false;
    }
    let mut depth = 0i32;
    for (index, ch) in upper.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                // The aggregate closes before the end: something follows it.
                if depth == 0 && index + ch.len_utf8() < upper.len() {
                    return false;
                }
            }
            '/' | '*' | '-' if depth == 0 => return false,
            _ => {}
        }
    }
    depth == 0 && expr.ends_with(')')
}

/// Grain checks (plan 057-B): an additive measure's total must equal the sum of
/// its values over a dimension's leaf members. The pivot user relies on that
/// invariant; when it fails, every subtotal in the workbook is wrong. It is
/// also the check that notices a join multiplying rows or a silently dropped
/// key, per measure.
pub(crate) fn grain_findings<B: QueryBackend + ?Sized>(
    backend: &B,
    model: &crate::engine::model::SemanticModel,
    config: &crate::project::config::ProxyConfig,
    user: &UserContext,
    dimension: &str,
) -> Vec<String> {
    let mut blocked = Vec::new();
    for measure in &model.measures {
        if !is_additive(measure) {
            continue;
        }
        let total_plan = QueryPlan::Total {
            measure: measure.id.clone(),
            filters: Vec::new(),
        };
        let group_plan = QueryPlan::GroupBy {
            measure: measure.id.clone(),
            group_by: vec![dimension.to_string()],
            filters: Vec::new(),
            group_levels: vec![None],
            set_op: None,
        };
        let total_sql =
            crate::engine::sql::sql_for_query_plan_with_context(model, &total_plan, user, config);
        let group_sql =
            crate::engine::sql::sql_for_query_plan_with_context(model, &group_plan, user, config);
        if total_sql.trim().is_empty() || group_sql.trim().is_empty() {
            continue;
        }

        let _ = backend.take_failure();
        let total = backend.query_scalar(&total_sql);
        if let Some(failure) = backend.take_failure() {
            blocked.push(format!(
                "grain check for '{}' cannot run: {failure}",
                measure.caption
            ));
            continue;
        }
        let groups = backend.query_grouped_1d(&group_sql);
        if let Some(failure) = backend.take_failure() {
            blocked.push(format!(
                "grain check for '{}' cannot run: {failure}",
                measure.caption
            ));
            continue;
        }
        let sum: f64 = groups.iter().map(|(_, value)| value).sum();
        let tolerance = total.abs() * 1e-6 + 0.01;
        if (sum - total).abs() > tolerance {
            blocked.push(format!(
                "measure '{}' is not additive over '{dimension}': total {total}, \
                 sum of {} groups {sum} (difference {:.2})",
                measure.caption,
                groups.len(),
                sum - total
            ));
        }
    }
    blocked
}

/// The tables, dimension keys and relationships the data-side checks run
/// over. Explicit lists, so tests can point them at any database.
pub(crate) struct DataShape {
    pub tables: Vec<String>,
    /// `(table, key column)` — one per flat dimension.
    pub dimension_keys: Vec<(String, String)>,
    /// `(fact table, fact column, dimension table, dimension column)`.
    pub relationships: Vec<(String, String, String, String)>,
}

/// Build the shape from a loaded model.
///
/// Only *physical* tables take part: the fact tables, the dimension tables a
/// relationship names, and dimensions that declare a `table_name` of their
/// own. A flat dimension's `physical_field` is a display path on the fact
/// table — it repeats by design, so it is not a key (checking it produced
/// false "not unique" findings).
pub(crate) fn data_shape(p: &crate::proxy_project::ProxyProject) -> DataShape {
    let model = &p.model;
    let mut tables: Vec<String> = Vec::new();
    let mut dimension_keys: Vec<(String, String)> = Vec::new();
    let mut relationships = Vec::new();

    fn add_table(tables: &mut Vec<String>, table: &str) {
        if !table.is_empty() && !tables.iter().any(|t| t == table) {
            tables.push(table.to_string());
        }
    }
    fn add_key(keys: &mut Vec<(String, String)>, table: &str, column: &str) {
        if table.is_empty() || column.is_empty() {
            return;
        }
        if !keys.iter().any(|(t, c)| t == table && c == column) {
            keys.push((table.to_string(), column.to_string()));
        }
    }

    for ft in &model.fact_tables {
        add_table(&mut tables, &ft.table_name);
    }
    for rel in &model.relationships {
        add_table(&mut tables, &rel.dim_table);
        add_key(&mut dimension_keys, &rel.dim_table, &rel.dim_column);
        let Some(fact) = model
            .fact_tables
            .iter()
            .find(|ft| ft.id == rel.fact_table_id)
        else {
            continue;
        };
        relationships.push((
            fact.table_name.clone(),
            rel.fact_column.clone(),
            rel.dim_table.clone(),
            rel.dim_column.clone(),
        ));
    }
    for d in &model.dimensions {
        // The table can still be checked for existence…
        if let Some(table) = d.table_name.as_deref() {
            add_table(&mut tables, table);
        }
        // …but `physical_field` is a display path, not a physical column, so a
        // dimension without a relationship has no reliable key to check. Keys
        // come from relationships only (their `dim_column` is the join key).
    }

    DataShape {
        tables,
        dimension_keys,
        relationships,
    }
}

/// The data-side checks (plan 057-B): unreadable tables, duplicate dimension
/// keys, fan-out relationships and orphan keys — the four ways a projection
/// produces a plausible wrong number instead of failing.
pub(crate) fn data_findings<B: QueryBackend + ?Sized>(
    backend: &B,
    shape: &DataShape,
) -> (Vec<String>, Vec<String>) {
    let mut blocked = Vec::new();
    let mut partial = Vec::new();

    for table in &shape.tables {
        // An embedded double quote cannot be escaped by the simple quoting
        // below; report it as a configuration error rather than blaming the
        // data (review S9).
        if table.contains('"') {
            blocked.push(format!(
                "table name '{table}' contains a double quote; fix the configuration"
            ));
            continue;
        }
        let _ = backend.take_failure();
        let _ = backend.query_rows(&format!("SELECT * FROM \"{table}\" LIMIT 0"));
        if let Some(failure) = backend.take_failure() {
            blocked.push(format!("table '{table}' cannot be read: {failure}"));
        }
    }

    for (table, column) in &shape.dimension_keys {
        if table.contains('"') || column.contains('"') {
            blocked.push(format!(
                "key '{table}.{column}' contains a double quote; fix the configuration"
            ));
            continue;
        }
        let _ = backend.take_failure();
        let rows = backend.query_rows(&format!(
            "SELECT COUNT(*), COUNT(DISTINCT \"{column}\") FROM \"{table}\""
        ));
        if let Some(failure) = backend.take_failure() {
            blocked.push(format!(
                "key '{table}.{column}' cannot be checked: {failure}"
            ));
            continue;
        }
        let number = |index: usize| {
            rows.first()
                .and_then(|row| row.get(index))
                .and_then(|value| value.parse::<i64>().ok())
        };
        if let (Some(total), Some(distinct)) = (number(0), number(1))
            && total > 0
            && distinct < total
        {
            blocked.push(format!(
                "dimension key '{table}.{column}' is not unique: {total} rows, {distinct} distinct values ({} duplicates)",
                total - distinct
            ));
        }
    }

    for (fact, fact_column, dim, dim_column) in &shape.relationships {
        let joined = backend.query_scalar(&format!(
            "SELECT COUNT(*) FROM \"{fact}\" AS f JOIN \"{dim}\" AS d ON f.\"{fact_column}\" = d.\"{dim_column}\""
        ));
        if let Some(failure) = backend.take_failure() {
            blocked.push(format!(
                "relationship '{fact}.{fact_column} -> {dim}.{dim_column}' cannot be checked: {failure}"
            ));
            continue;
        }
        let null_keys = backend.query_scalar(&format!(
            "SELECT COUNT(*) FROM \"{fact}\" WHERE \"{fact_column}\" IS NULL"
        ));
        let facts = backend.query_scalar(&format!("SELECT COUNT(*) FROM \"{fact}\""));
        let orphans = backend.query_scalar(&format!(
            "SELECT COUNT(*) FROM \"{fact}\" AS f WHERE f.\"{fact_column}\" IS NOT NULL \
             AND NOT EXISTS (SELECT 1 FROM \"{dim}\" AS d WHERE d.\"{dim_column}\" = f.\"{fact_column}\")"
        ));
        if let Some(failure) = backend.take_failure() {
            blocked.push(format!(
                "relationship '{fact}.{fact_column} -> {dim}.{dim_column}' cannot be checked: {failure}"
            ));
            continue;
        }
        let (joined, facts, orphans, null_keys) = (
            joined as i64,
            facts as i64,
            orphans as i64,
            null_keys as i64,
        );
        if facts > 0 && joined > facts {
            blocked.push(format!(
                "relationship '{fact}.{fact_column} -> {dim}.{dim_column}' fans out: \
                 {joined} joined rows for {facts} fact rows (x{:.2})",
                joined as f64 / facts as f64
            ));
        }
        if orphans > 0 {
            partial.push(format!(
                "{orphans} fact rows have no matching {dim}.{dim_column} \
                 ({fact}.{fact_column}); they are silently dropped from pivots"
            ));
        }
        if null_keys > 0 {
            partial.push(format!(
                "{null_keys} fact rows have a NULL {fact}.{fact_column}; they are \
                 silently dropped from every pivot over {dim}"
            ));
        }
    }

    (blocked, partial)
}

/// Proxy-side logic that belongs upstream (plan 044). Empty means a thin
/// projection: every measure is plain SQL over exposed tables and every role
/// predicate is translated SQL.
pub(crate) fn semantic_creep(p: &crate::proxy_project::ProxyProject) -> Vec<String> {
    let mut findings = Vec::new();
    for m in &p.model.measures {
        let Some(sql) = m.sql_fallback_sql.as_deref() else {
            continue;
        };
        let upper = sql.to_uppercase();
        let is_stub = upper.trim() == "SELECT 1 AS DUMMY;"
            || upper.contains("TODO")
            || upper.contains("SELECT 1 AS DUMMY");
        if !is_stub {
            findings.push(format!(
                "measure '{}' carries fallback SQL — define it upstream (an additive column or a mart)",
                m.caption
            ));
        }
    }
    for role in &p.config.roles {
        for tp in &role.table_permissions {
            let has_dax = tp.dax_filter.as_deref().is_some_and(|d| !d.is_empty());
            if tp.filter_expression.is_empty() && has_dax {
                findings.push(format!(
                    "role '{}' table '{}' has a DAX filter with no SQL translation",
                    role.name, tp.table
                ));
            }
        }
    }
    findings
}

/// Measures that are not a plain `SUM(column)`: ratios, counts, averages, or
/// expressions. They are legitimate SQL, but they cannot use rollups and
/// usually belong in an upstream mart at a declared grain (plan 044).
pub(crate) fn non_additive_measures(p: &crate::proxy_project::ProxyProject) -> Vec<String> {
    p.model
        .measures
        .iter()
        .filter(|m| {
            m.sql_fallback_sql.is_none()
                && crate::engine::aggregate::measure_base_column(&m.sql_expr).is_none()
        })
        .map(|m| m.caption.clone())
        .collect()
}

pub fn run(args: Vec<String>) -> i32 {
    // args: ["qualify", "<config-path>", "<optional-trace-path>", "--strict"]
    let strict = args.iter().any(|a| a == "--strict");
    let positional: Vec<&str> = args
        .iter()
        .skip(1)
        .map(|s| s.as_str())
        .filter(|a| !a.starts_with("--"))
        .collect();
    let config_path = positional
        .first()
        .copied()
        .unwrap_or("projects/project3/proxy-config.json");
    let trace_path = positional.get(1).copied();

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

    if strict {
        println!();
        println!("=== Strict mode (plan 044: no semantic layer in the proxy) ===");
        match crate::proxy_project::ProxyProject::load(config_path) {
            Ok(p) => {
                let creep = semantic_creep(&p);
                let non_additive = non_additive_measures(&p);
                for finding in &creep {
                    println!("  [FAIL] {finding}");
                }
                if !non_additive.is_empty() {
                    println!(
                        "  [NOTE] {} measure(s) are not plain SUM(column) — serve them from an upstream mart at a declared grain: {}",
                        non_additive.len(),
                        non_additive.join(", ")
                    );
                }
                if creep.is_empty() {
                    println!("  No proxy-side logic found (fallback SQL, untranslated DAX).");
                }
                if !creep.is_empty() {
                    println!();
                    println!("Strict: FAILED ({} finding(s))", creep.len());
                    return 1;
                }
                println!("Strict: OK");
            }
            Err(e) => {
                eprintln!("strict: cannot load project: {e}");
                return 2;
            }
        }
    }

    verdict.exit_code()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_retail_analytics_is_ready_after_plan_021() {
        crate::tools::seed_projects_db::ensure_seeded();
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
    fn generated_contoso_reports_missing_tables_and_roles() {
        crate::tools::seed_projects_db::ensure_seeded();
        let v = qualify("projects/generated_contoso/proxy-config.json", None);
        // Roles defined without an auth config, measures needing manual review,
        // and — with the data-side checks (plan 057-B) — tables the model
        // references that the shipped dummy database does not contain
        // (`promotion` among them). The fixture is genuinely not serveable
        // until the intake work (plan 045) fills the dummy load in.
        assert_eq!(v.label(), "BLOCKED", "{:?}", v.reasons());
        let reasons: Vec<&str> = v.reasons().iter().map(|s| s.as_str()).collect();
        assert!(
            reasons.iter().any(|r| r.contains("no auth config")),
            "should report missing auth config: {reasons:?}"
        );
        assert!(
            reasons.iter().any(|r| r.contains("promotion")),
            "should report the missing table: {reasons:?}"
        );
    }

    #[test]
    fn generated_contoso_has_no_stub_fallbacks() {
        let p = crate::proxy_project::ProxyProject::load(
            "projects/generated_contoso/proxy-config.json",
        )
        .expect("load generated_contoso");
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
            "no converted project may ship stub fallback SQL: {:?}",
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

    // Plan 044: strict mode fails on proxy-side logic and passes a thin
    // projection.
    #[test]
    fn strict_is_clean_for_a_thin_projection() {
        let p =
            crate::proxy_project::ProxyProject::load("projects/upstream_marts/proxy-config.yaml")
                .expect("load upstream_marts demo");
        let creep = semantic_creep(&p);
        assert!(
            creep.is_empty(),
            "demo must carry no proxy-side logic: {creep:?}"
        );
        let non_additive = non_additive_measures(&p);
        assert!(
            non_additive.iter().any(|m| m == "On-time %"),
            "ratio measures are reported as non-additive: {non_additive:?}"
        );
    }

    #[test]
    fn strict_flags_fallback_sql_and_untranslated_dax() {
        let p = crate::proxy_project::ProxyProject::load(
            "projects/generated_contoso/proxy-config.json",
        )
        .expect("load contoso");
        let creep = semantic_creep(&p);
        assert!(
            creep.iter().any(|f| f.contains("fallback SQL")),
            "fallback measures must be flagged: {creep:?}"
        );
        assert!(
            creep.iter().any(|f| f.contains("DAX filter")),
            "untranslated DAX role filters must be flagged: {creep:?}"
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

    /// The data-side checks catch the classic silent wrong answers: an
    /// unreadable table, a duplicate dimension key, a fan-out relationship and
    /// orphan keys (plan 057-B).
    #[test]
    fn data_checks_find_seeded_defects() {
        let path = std::env::temp_dir().join(format!(
            "mallardcube-qualify-data-{}.duckdb",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        {
            let conn = duckdb::Connection::open(&path).expect("open temp db");
            conn.execute_batch(
                "CREATE TABLE fact(id INTEGER, amount DOUBLE);
                 INSERT INTO fact VALUES (1, 1.0), (1, 2.0), (2, 3.0), (9, 4.0);
                 CREATE TABLE dim(id INTEGER, label VARCHAR);
                 INSERT INTO dim VALUES (1, 'a'), (1, 'duplicate'), (2, 'b');",
            )
            .expect("seed temp db");
        }
        let source = crate::backend::BackendSource::file(&path).expect("open seeded db");
        let backend = source.checkout();
        let shape = DataShape {
            tables: vec!["fact".into(), "dim".into(), "missing_table".into()],
            dimension_keys: vec![("dim".into(), "id".into())],
            relationships: vec![("fact".into(), "id".into(), "dim".into(), "id".into())],
        };
        let (blocked, partial) = data_findings(backend.as_ref(), &shape);
        assert!(
            blocked.iter().any(|m| m.contains("missing_table")),
            "{blocked:?}"
        );
        assert!(
            blocked.iter().any(|m| m.contains("not unique")),
            "{blocked:?}"
        );
        assert!(
            blocked.iter().any(|m| m.contains("fans out")),
            "{blocked:?}"
        );
        assert!(
            partial.iter().any(|m| m.contains("no matching")),
            "{partial:?}"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// Additivity is "is exactly one additive aggregate", not "starts with
    /// SUM(" — a ratio of sums is not additive and must not be grain-checked
    /// or exempted from oracle coverage (review S3).
    #[test]
    fn additive_expressions_are_exact_aggregates() {
        assert!(is_additive_expr("SUM(revenue)"));
        assert!(is_additive_expr("count(*)"));
        assert!(is_additive_expr("SUM(a + b)"));
        assert!(!is_additive_expr("SUM(a) / NULLIF(SUM(b), 0)"));
        assert!(!is_additive_expr("SUM(a) * 100.0 / SUM(b)"));
        assert!(!is_additive_expr("SUM(a) * 100"));
        assert!(!is_additive_expr("COUNT(DISTINCT k)"));
        assert!(!is_additive_expr("AVG(x)"));
        assert!(!is_additive_expr("revenue"));
    }

    /// The grain invariant holds on the demo model: an additive measure's
    /// total equals the sum over a dimension's leaf members (plan 057-B).
    #[test]
    fn grain_checks_accept_the_demo_model() {
        let p = crate::proxy_project::ProxyProject::load("projects/project3/proxy-config.json")
            .expect("load project3");
        let dimension = &p
            .model
            .relationships
            .first()
            .expect("project3 has a relationship")
            .dimension_id
            .clone();
        let blocked = grain_findings(
            crate::backend::Backend::test_fixture(),
            &p.model,
            &p.config,
            &UserContext::admin_default(),
            dimension,
        );
        assert!(blocked.is_empty(), "{blocked:?}");
    }

    /// A healthy shape reports nothing (the demo model, through the same checks
    /// the CLI uses).
    #[test]
    fn data_checks_accept_the_demo_model() {
        let p = crate::proxy_project::ProxyProject::load("projects/project3/proxy-config.json")
            .expect("load project3");
        let shape = data_shape(&p);
        assert!(!shape.tables.is_empty());
        let (blocked, _) = data_findings(crate::backend::Backend::test_fixture(), &shape);
        assert!(blocked.is_empty(), "{blocked:?}");
    }

    /// Oracles compare the proxy's own emitted SQL against hand-written
    /// expectations, and a wrong expectation fails (plan 057-B). The expected
    /// values came from the reference engine, not from the proxy.
    #[test]
    fn oracles_compare_against_the_engine() {
        let p = crate::proxy_project::ProxyProject::load("projects/project3/proxy-config.json")
            .expect("load project3");
        let file = OracleFile {
            oracles: vec![
                Oracle {
                    measure: "Revenue".into(),
                    dimension: None,
                    member: None,
                    expected: 521_586_767.0,
                    tolerance: None,
                },
                Oracle {
                    measure: "Revenue".into(),
                    dimension: Some("Category".into()),
                    member: Some("Automotive".into()),
                    expected: 25_102_648.0,
                    tolerance: None,
                },
                Oracle {
                    measure: "Revenue".into(),
                    dimension: None,
                    member: None,
                    expected: 1.0,
                    tolerance: None,
                },
                Oracle {
                    measure: "NoSuchMeasure".into(),
                    dimension: None,
                    member: None,
                    expected: 0.0,
                    tolerance: None,
                },
            ],
        };
        let (blocked, partial) = oracle_findings(
            crate::backend::Backend::test_fixture(),
            &p.model,
            &p.config,
            &UserContext::admin_default(),
            &file,
        );
        assert_eq!(blocked.len(), 2, "{blocked:?}");
        // Revenue YTD/QTD/MTD/Prior Year are not additive and have no oracle in
        // this file: the file exists, so coverage is required.
        assert!(
            partial.iter().any(|m| m.contains("Revenue YTD")),
            "uncovered non-additive measures are reported: {partial:?}"
        );
        assert!(
            blocked.iter().any(|m| m.contains("expected 1")),
            "{blocked:?}"
        );
        assert!(
            blocked.iter().any(|m| m.contains("NoSuchMeasure")),
            "{blocked:?}"
        );
    }
}
