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
    get_execute_response_with_format(mdx, None, None, backend, user, config)
}

/// The same execution, rendered as the flattened rowset when the request asked
/// for `<Format>Tabular</Format>` (what ADODB reads) and as a cellset otherwise
/// (plan 051).
pub fn get_execute_response_with_format<B: QueryBackend + ?Sized>(
    mdx: &str,
    format: Option<&str>,
    content: Option<&str>,
    backend: &B,
    user: &UserContext,
    config: &ProxyConfig,
) -> (String, Timings) {
    let cache = if cache::enabled() {
        Some(&*cache::RESULT_CACHE)
    } else {
        None
    };
    get_execute_response_with_format_and_cache(mdx, format, content, backend, user, config, cache)
}

/// [`get_execute_response_with_format`] against a caller-supplied result
/// cache, `None` disabling it.
///
/// Tests pass their own instance: the production cache is process-wide, so
/// parallel tests would otherwise populate (or evict) the entry a cache
/// assertion is about to make.
pub(crate) fn get_execute_response_with_format_and_cache<B: QueryBackend + ?Sized>(
    mdx: &str,
    format: Option<&str>,
    content: Option<&str>,
    backend: &B,
    user: &UserContext,
    config: &ProxyConfig,
    cache: Option<&cache::ResultCache>,
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

    // A `FROM` clause naming another cube is a scope error: the reference
    // faults "The <name> cube does not exist." (measured 2026-09-25).
    if let Some(cube) = query.cube.as_deref()
        && !cube.trim().eq_ignore_ascii_case(config.cube.trim())
    {
        let timings = Timings::new(RuntimePath::DirectSql, "scope".to_string(), 0);
        crate::audit::emit(
            "refusal",
            user,
            "cube-scope",
            "the FROM clause names another cube",
        );
        return (
            crate::xmla::response::fault_response(&format!("The {cube} cube does not exist.")),
            timings,
        );
    }
    let mdx_parse_us = (Instant::now() - t0).as_micros() as u64;

    let t0 = Instant::now();
    let model = &crate::proxy_project::project().model;

    // A measure the statement names but the model does not define is a parse
    // error in the reference ("The '[Bogus]' member was not found in the cube
    // when the string, [Measures].[Bogus], was parsed.", measured 2026-09-30).
    // The planner used to drop unknown names from a measure set and fall back
    // to the model's first measure, silently answering a different measure.
    if let Some(fault) = unknown_measure_fault(&query, model) {
        let timings = Timings::new(
            RuntimePath::DirectSql,
            "unknown-measure".to_string(),
            mdx_parse_us,
        );
        crate::audit::emit(
            "refusal",
            user,
            "unknown-measure",
            "a measure the model does not define",
        );
        return (fault, timings);
    }

    // The renderer's slicer axis lists every dimension a query does not name.
    // A dimension hidden by OLS must not appear there, and the Measures
    // hierarchy follows its measures (plan 051 RLS review).
    let mut query = query;

    // Excel's Top-N / value filter arrives as a subselect; turn it into a
    // member filter before planning (measured 2026-09-25: the reference answers
    // the filtered set, we answered the whole set).
    let subselect_fault = |message: String| {
        let timings = Timings::new(
            RuntimePath::DirectSql,
            "filter-subselect".to_string(),
            mdx_parse_us,
        );
        (crate::xmla::response::fault_response(&message), timings)
    };
    match crate::mdx_semantic::excel_filter_subselect(mdx) {
        Err(message) => return subselect_fault(message),
        Ok(None) => {}
        Ok(Some(idiom)) => match filter_members_for_subselect(&idiom, model, backend, user, config)
        {
            Ok(filter) => query.filters.push(filter),
            Err(message) => return subselect_fault(message),
        },
    }

    if !user.is_administrator {
        let hidden_dimensions = model
            .dimensions
            .iter()
            .filter(|d| !crate::xmla::discover::dimension_visible(model, config, user, &d.id))
            .map(|d| d.id.clone())
            .collect();
        // Per-measure, not per-model: a role can see one fact table and be
        // denied another, and the probes must not count or name the denied
        // measures (plan 058).
        let visible_measures: Vec<String> = model
            .measures
            .iter()
            .filter(|m| {
                crate::xmla::discover::table_visible(
                    config,
                    user,
                    &model.fact_table(m.fact_table_idx).table_name,
                )
            })
            .map(|m| m.id.clone())
            .collect();
        // Role-filtered dimensions: the axis and drill builders must not read
        // the unfiltered dictionary or the static hint (plan 058). The
        // per-role dictionary is cached by (epoch, dim, predicate), so this
        // costs scans only on the first request for a role and data epoch.
        let mut filtered_dims = std::collections::HashMap::new();
        for dim in &model.dimensions {
            if !crate::xmla::discover::dimension_visible(model, config, user, &dim.id) {
                continue;
            }
            let table = model.dim_table_for_discovery(&dim.id);
            if let crate::engine::model::TableAccess::Filtered(predicate) =
                crate::engine::model::effective_table_filter(config, user, table)
                && !predicate.is_empty()
            {
                let members = model
                    .dim_cache
                    .get_filtered(model, dim, backend, &predicate);
                let per_level: Vec<u32> = if dim.levels.is_empty() {
                    vec![members.all_cardinality]
                } else {
                    members
                        .level_paths
                        .iter()
                        .map(|paths| paths.len() as u32)
                        .collect()
                };
                filtered_dims.insert(
                    dim.id.clone(),
                    crate::mdx_semantic::FilteredDim {
                        predicate,
                        per_level,
                    },
                );
            }
        }
        // A dictionary built from a failed query is empty (and deliberately
        // uncached); rendering on it would answer CHILDREN_CARDINALITY 0 as if
        // the dimension had no members. Fault instead — and consume the
        // failure, otherwise the latch would poison this pooled connection for
        // every later request (plan 058).
        if let Some(failure) = backend.take_failure() {
            crate::audit::emit(
                "error",
                user,
                "dimension-dictionary-failed",
                "a dimension dictionary query failed",
            );
            let timings = Timings::new(
                RuntimePath::DirectSql,
                "dimension-dictionary-failed".to_string(),
                mdx_parse_us,
            );
            return (
                crate::xmla::response::fault_response(&format!(
                    "a dimension dictionary query failed: {failure}"
                )),
                timings,
            );
        }
        query.access = Some(crate::mdx_semantic::AccessView {
            hidden_dimensions,
            visible_measures: Some(visible_measures),
            filtered_dims,
        });
    }
    // A metadata probe (`strtomember`), a member-only probe, or a set probe
    // naming an object the requesting role cannot see must not resolve it: the
    // renderers echo the set's members verbatim, so the answer would leak a
    // hidden measure or dimension. Fail closed with a fault that names the
    // class rather than repeating the hidden name (plan 058).
    if let Some(access) = query.access.as_ref() {
        let mut targets: Vec<String> = query.metadata_probe_targets.clone();
        targets.extend(query.member_only_unames.iter().cloned());
        if let Some(set) = &query.set_probe {
            collect_set_member_targets(set, &mut targets);
        }
        if targets
            .iter()
            .any(|target| crate::engine::plan::member_hidden_by_access(target, model, access))
        {
            crate::audit::emit(
                "refusal",
                user,
                "hidden-probe",
                "a probe names an object the role cannot see",
            );
            let timings = Timings::new(
                RuntimePath::DirectSql,
                "hidden-probe".to_string(),
                mdx_parse_us,
            );
            return (
                crate::xmla::response::fault_response(
                    "a member named by this probe is not available to the requesting role; the \
                     probe is refused rather than resolved against the full model",
                ),
                timings,
            );
        }
    }
    // The all-level-members probe renders its axis dimension directly from the
    // query, so a relationship-backed dimension table hidden by OLS would be
    // named and counted; Gate 2 only sees the measure's fact table. Its
    // siblings carry the dimension through the plan's group-by (checked at
    // plan time) or name no axis (plan 058).
    if let Some(access) = query.access.as_ref()
        && matches!(
            query.kind,
            crate::mdx_semantic::SemanticQueryKind::AllLevelMembers
        )
        && query
            .axis_dimensions
            .first()
            .is_some_and(|dimension| access.dimension_hidden(dimension))
    {
        crate::audit::emit(
            "refusal",
            user,
            "hidden-dimension-probe",
            "the probe names a dimension the role cannot see",
        );
        let timings = Timings::new(
            RuntimePath::DirectSql,
            "hidden-dimension-probe".to_string(),
            mdx_parse_us,
        );
        return (
            crate::xmla::response::fault_response(
                "the dimension named by this probe is not available to the requesting role; the \
                 probe is refused rather than rendered from the full model",
            ),
            timings,
        );
    }

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
        crate::audit::emit(
            "refusal",
            user,
            "restricted-fallback",
            "a measure uses authored SQL that cannot carry role predicates",
        );
        let timings = Timings::new(RuntimePath::DirectSql, "restricted-fallback".into(), 0);
        return (
            crate::xmla::response::fault_response(&format!(
                "measure '{measure}' uses authored SQL that cannot be filtered for the requesting \
                 role; the query is refused rather than returning unfiltered rows"
            )),
            timings,
        );
    }

    // A dimension hidden by OLS must not be read through an axis or a filter:
    // the plan carries no deny predicate for it, so the query would return its
    // members and values (plan 051 RLS review). Refusing is the fail-closed
    // answer where the reference faults for an inaccessible object.
    if let Some(dimension) = plan_hidden_dimension(&plan, model, user, config) {
        crate::audit::emit(
            "refusal",
            user,
            "hidden-dimension",
            "a query reads a dimension the role cannot see",
        );
        let timings = Timings::new(RuntimePath::DirectSql, "hidden-dimension".into(), 0);
        return (
            crate::xmla::response::fault_response(&format!(
                "dimension '{dimension}' is hidden for the requesting role; the query is \
                 refused rather than reading its members"
            )),
            timings,
        );
    }

    let key = plan_key(&plan);

    // Excel repeats the same query once per CELL PROPERTIES variant; serve the
    // repeats from a short-lived cache (plan 032). The cellset is rendered
    // fresh below, so every variant keeps its own cell properties.
    // Start clean: a failure latched by an earlier request on this pooled
    // connection must not fault this one (the checks below cover this
    // request's own queries; plan 057-C).
    let _ = backend.take_failure();

    let cache_key = cache::cache_key(&key, &config.catalog, &config.cube, user, &config.roles);
    let t0 = Instant::now();
    let cached = cache.and_then(|cache| cache.get(&cache_key));
    let (result, cache_hit) = match cached {
        Some(hit) => (hit, true),
        None => {
            let result = std::sync::Arc::new(execute_plan_with_backend_and_context(
                &plan, model, backend, user, config,
            ));
            // A failed query must not be cached, let alone rendered as a
            // plausible number (plan 057-C).
            if let Some(failure) = backend.take_failure() {
                let timings = Timings::new(
                    RuntimePath::DirectSql,
                    "query-failed".to_string(),
                    mdx_parse_us,
                );
                return (
                    crate::xmla::response::fault_response(&format!(
                        "a query against the database failed: {failure}"
                    )),
                    timings,
                );
            }
            if let Some(cache) = cache {
                cache.insert(cache_key, std::sync::Arc::clone(&result));
            }
            (result, false)
        }
    };
    let sql_execute_us = (Instant::now() - t0).as_micros() as u64;

    let mut timings = Timings::new(RuntimePath::DirectSql, key, mdx_parse_us);
    timings.plan_us = plan_us;
    timings.cache_hit = cache_hit;
    timings.sql_execute_us = sql_execute_us;

    // The renderers synthesize the `(All)` cell of a drilldown axis from the
    // axis members; the reference evaluates the measure in the `(All)`
    // context instead (measured 2026-09-30). Inject the engine's values. A
    // set-op axis keeps its summed `(All)` — the reference aggregates the
    // *returned subset* there (verified against the mirror) — so it is left
    // alone, and a hidden fact table yields no values at all.
    let drill_shaped = matches!(
        query.kind,
        crate::mdx_semantic::SemanticQueryKind::DrilldownCategories
            | crate::mdx_semantic::SemanticQueryKind::DrilldownMemberProbe
    );
    // The cross-tab family (measures against two dimensions) rolls its
    // `(All)`-side cells up per dimension, so it needs the grouped values too.
    let cross_tab_shaped = query.axis_dimensions.len() >= 2 || query.crossjoin_axis;
    if !matches!(plan, crate::engine::plan::QueryPlan::Empty)
        && query.axis_set_op.is_none()
        && (drill_shaped || cross_tab_shaped)
    {
        query.drilldown_all_values = drilldown_all_values(&query, model, user, config, backend);
        if cross_tab_shaped {
            query.dimension_values = dimension_values(&query, model, user, config, backend);
        }
    }

    let t0 = Instant::now();
    let tabular = format.is_some_and(|format| format.eq_ignore_ascii_case("tabular"));
    // `Content` selects schema and/or rows: absent means `SchemaData`,
    // `Schema` is schema-only, `Data` (what ADODB sends) is rows-only
    // (measured 2026-09-27).
    let rowset_content = RowsetContent::from_property(content);
    let xml = if tabular {
        match render_tabular_rowset(&query, &plan, &result, model, rowset_content, backend) {
            Ok(rowset) => rowset,
            Err(message) => {
                timings.finish();
                return (crate::xmla::response::fault_response(&message), timings);
            }
        }
    } else {
        dispatch_with_backend(&query, &result, backend)
    };
    timings.xml_render_us = (Instant::now() - t0).as_micros() as u64;
    // Rendering can query too (member dictionaries for axis and child counts);
    // a failure there replaces whatever was rendered.
    if let Some(failure) = backend.take_failure() {
        timings.finish();
        return (
            crate::xmla::response::fault_response(&format!(
                "a query against the database failed: {failure}"
            )),
            timings,
        );
    }
    timings.finish();
    (xml, timings)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The reference's refusal for a `<Catalog>` property naming another database
/// (measured 2026-09-25); names match case-insensitively. Split from
/// `catalog_scope_fault` so the streaming member route can answer the same
/// message through its own fault type.
pub fn catalog_scope_message(
    catalog: Option<&str>,
    config: &ProxyConfig,
    user: &crate::engine::model::UserContext,
) -> Option<String> {
    let wanted = catalog?.trim();
    if wanted.is_empty() || wanted.eq_ignore_ascii_case(config.catalog.trim()) {
        return None;
    }
    let who = if user.user_id.is_empty() {
        "the user".to_string()
    } else {
        format!("the user, '{}',", user.user_id)
    };
    Some(format!(
        "Either {who} does not have access to the '{wanted}' database, or the database does not exist."
    ))
}

/// The same refusal as a complete SOAP fault.
pub fn catalog_scope_fault(
    catalog: Option<&str>,
    config: &ProxyConfig,
    user: &crate::engine::model::UserContext,
) -> Option<String> {
    catalog_scope_message(catalog, config, user)
        .map(|message| crate::xmla::response::fault_response(&message))
}

/// A `FROM` clause naming another cube, for the paths that do not build a
/// `SemanticQuery` (drillthrough); measured 2026-09-25.
pub fn mdx_cube_scope_fault(mdx: &str, config: &ProxyConfig) -> Option<String> {
    let cube = crate::mdx::parser::parse_mdx(mdx).cube_name?;
    if cube.trim().eq_ignore_ascii_case(config.cube.trim()) {
        return None;
    }
    Some(crate::xmla::response::fault_response(&format!(
        "The {cube} cube does not exist."
    )))
}

/// A measure the statement *references* that the model does not define.
///
/// The reference faults — "The '[Bogus]' member was not found in the cube when
/// the string, [Measures].[Bogus], was parsed." (measured 2026-09-30, SSAS 2025
/// tabular) — while the planner dropped unknown names and fell back to a
/// default measure, silently answering a different measure. Measured on the
/// same reference: names resolve case-insensitively, a *named*
/// `[Measures].[All]` / `[Measures].[Members]` faults, the postfix set forms
/// (`.Members`, `.All`) answer, and the hidden `__Default measure` answers.
pub fn unknown_measure_fault(
    query: &crate::mdx_semantic::SemanticQuery,
    model: &crate::engine::model::SemanticModel,
) -> Option<String> {
    for (name, bracketed) in &query.measure_references {
        let name = name.trim();
        if name.is_empty()
            || name.eq_ignore_ascii_case("__Default measure")
            // A bare `[Measures].X` is a property/set function (`currentmember`,
            // `Members`, …) — the reference answers those (measured 2026-09-30).
            || (!bracketed && is_measure_property_or_set(name))
        {
            continue;
        }
        if query
            .defined_measures
            .iter()
            .any(|defined| defined.eq_ignore_ascii_case(name))
            || model.lookup_measure(name).is_some()
        {
            continue;
        }
        return Some(unknown_measure_message(name));
    }
    None
}

/// The same refusal for a `DRILLTHROUGH` statement, which the MDX frontend
/// cannot parse: scan its `[Measures].[Name]` references. Measured 2026-09-30:
/// the reference faults for a measure a drillthrough names and the model does
/// not define, and this entry bypasses the cellset path's guard.
pub fn unknown_drillthrough_measure_fault(
    statement: &str,
    model: &crate::engine::model::SemanticModel,
) -> Option<String> {
    for (name, bracketed) in crate::mdx::frontend::measure_references_in_text(statement) {
        if name.eq_ignore_ascii_case("__Default measure")
            || (!bracketed && is_measure_property_or_set(&name))
            || model.lookup_measure(&name).is_some()
        {
            continue;
        }
        return Some(unknown_measure_message(&name));
    }
    None
}

/// The reference's phrasing for a measure the cube does not have (the
/// reference also prints a `Query (1, 9)` position; ours omits it — recorded).
fn unknown_measure_message(name: &str) -> String {
    crate::xmla::response::fault_response(&format!(
        "The '[{name}]' member was not found in the cube when the string, [Measures].[{name}], was parsed."
    ))
}

/// A bare `[Measures].X` the reference accepts: the postfix set functions and
/// the member-property functions (Excel's probes use `.currentmember`).
fn is_measure_property_or_set(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "members"
            | "allmembers"
            | "children"
            | "all"
            | "currentmember"
            | "member_value"
            | "member_key"
            | "member_unique_name"
            | "member_caption"
            | "member_name"
    )
}

/// The measure's value at the drilldown's input `(All)`, per measure.
///
/// The reference evaluates the *measure* in the `(All)` context — measured
/// 2026-09-30 on a ratio measure (`DIVIDE(SUM(amount), SUM(units))`): the
/// `(All)` cell is the ratio of sums (18.1818…), not the sum of the members'
/// ratios (110). The renderers summed the axis members instead; this gives
/// them the engine's own value. The query is built through the context-aware
/// SQL builder, so the user's row filter still applies.
/// Per-dimension member values for the cross-tab family's `(All)` roll-ups:
/// each axis measure grouped by each axis dimension within the query's
/// slicers, through the context-aware executor so the role's row filter
/// applies. One `GroupBy` per (dimension, measure) — the cross-tab shapes are
/// two flat dimensions, so this stays a handful of cheap aggregations.
fn dimension_values<B: QueryBackend + ?Sized>(
    query: &crate::mdx_semantic::SemanticQuery,
    model: &crate::engine::model::SemanticModel,
    user: &UserContext,
    config: &ProxyConfig,
    backend: &B,
) -> Vec<(String, String, String, f64)> {
    use crate::engine::plan::{
        QueryPlan, QueryResult, execute_plan_with_backend_and_context, filters_with_time_flag,
        typed_filters,
    };
    let names: Vec<String> = if !query.measures.is_empty() {
        query.measures.clone()
    } else if let Some(single) = &query.measure {
        vec![single.clone()]
    } else {
        vec![crate::execute::axis_members::measure_id_for_query(query)]
    };
    let mut out = Vec::new();
    for dim in &query.axis_dimensions {
        for name in &names {
            let Some(measure) = model.lookup_measure(name) else {
                continue;
            };
            let plan = QueryPlan::GroupBy {
                measure: measure.id.clone(),
                group_by: vec![dim.clone()],
                filters: filters_with_time_flag(model, &measure.id, &typed_filters(&query.filters)),
                group_levels: vec![None],
                set_op: None,
            };
            if let QueryResult::Grouped(rows) =
                execute_plan_with_backend_and_context(&plan, model, backend, user, config)
            {
                for (key, value) in rows {
                    out.push((dim.clone(), key, measure.id.clone(), value));
                }
            }
        }
    }
    out
}

/// An All-rooted drilldown (`DrilldownLevel({[Dim].[Hier].[All]})`): the
/// reference answers the `(All)` member first, unlike a level set
/// (`level_drag`), which has no total row (both measured).
fn all_rooted_drilldown(query: &crate::mdx_semantic::SemanticQuery) -> bool {
    !query.level_drag && query.drilldown_level() == Some(0)
}

fn drilldown_all_values<B: QueryBackend + ?Sized>(
    query: &crate::mdx_semantic::SemanticQuery,
    model: &crate::engine::model::SemanticModel,
    user: &UserContext,
    config: &ProxyConfig,
    backend: &B,
) -> Vec<(String, f64)> {
    use crate::engine::plan::{QueryPlan, filters_with_time_flag, typed_filters};
    let names: Vec<String> = if !query.measures.is_empty() {
        query.measures.clone()
    } else if let Some(single) = &query.measure {
        vec![single.clone()]
    } else {
        // A measure-less drilldown (Excel's field discovery) answers the
        // model's default measure — the same resolution the renderers use.
        vec![crate::execute::axis_members::measure_id_for_query(query)]
    };
    // The drill's own member filter must not constrain the input set; real
    // slicers on the same dimension are kept (as in `level0_member_values`).
    let is_drill_filter = |filter: &crate::mdx_semantic::DimensionFilter| {
        query
            .drill_members
            .iter()
            .any(|(dim, keys)| filter.dimension == *dim && filter.members == *keys)
    };
    let slicers: Vec<crate::mdx_semantic::DimensionFilter> = query
        .filters
        .iter()
        .filter(|filter| !is_drill_filter(filter))
        .cloned()
        .collect();
    let mut values = Vec::new();
    for name in &names {
        let Some(measure) = model.lookup_measure(name) else {
            continue;
        };
        let plan = QueryPlan::Total {
            measure: measure.id.clone(),
            filters: filters_with_time_flag(model, &measure.id, &typed_filters(&slicers)),
        };
        let sql = crate::engine::sql::sql_for_query_plan_with_context(model, &plan, user, config);
        values.push((measure.id.clone(), backend.query_scalar(&sql)));
    }
    values
}

/// The member set Excel's filter idiom selects: the dimension's leaf members
/// sorted by the measure ascending, taken until the running total reaches the
/// `BottomSum` limit (the reference's semantics — for a Top-5 filter over
/// revenue that is the single lowest member, which is why the mirror answers
/// `{All, Toys}`).
fn filter_members_for_subselect<B: QueryBackend + ?Sized>(
    idiom: &crate::mdx_semantic::ExcelFilterSubselect,
    model: &crate::engine::model::SemanticModel,
    backend: &B,
    user: &UserContext,
    config: &ProxyConfig,
) -> Result<crate::mdx_semantic::DimensionFilter, String> {
    let measure = model
        .lookup_measure(&idiom.measure)
        .ok_or_else(|| unknown_measure_message(&idiom.measure))?;
    if model.dim_def_opt(&idiom.dimension).is_none() {
        return Err(format!(
            "the filter subselect names dimension '{}', which the model does not define",
            idiom.dimension
        ));
    }
    // Rank with the same time window the outer query uses: a YTD measure's
    // BottomSum must not rank on unfiltered numbers (review F9).
    let plan = crate::engine::plan::QueryPlan::GroupBy {
        measure: measure.id.clone(),
        group_by: vec![idiom.dimension.clone()],
        filters: crate::engine::plan::filters_with_time_flag(model, &measure.id, &[]),
        group_levels: vec![None],
        set_op: None,
    };
    let sql = crate::engine::sql::sql_for_query_plan_with_context(model, &plan, user, config);
    if sql.trim().is_empty() {
        return Err("the filter subselect could not be evaluated".to_string());
    }
    let _ = backend.take_failure();
    let mut groups = backend.query_grouped_1d(&sql);
    if let Some(failure) = backend.take_failure() {
        return Err(format!("a query against the database failed: {failure}"));
    }
    groups.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut members = Vec::new();
    let mut running = 0.0;
    for (key, value) in groups {
        members.push(key);
        running += value;
        if running >= idiom.limit {
            break;
        }
    }

    Ok(crate::mdx_semantic::DimensionFilter {
        dimension: idiom.dimension.clone(),
        members,
        level: None,
        range: None,
        date_window: None,
        label: None,
    })
}

/// The reference's flattened rowset for `<Format>Tabular</Format>`: one row
/// per axis tuple, one column per grouped member plus one per measure. A
/// member column is omitted when that coordinate is `(All)` (the grand total
/// carries measures only), a level grouping is named after its level and has
/// no `(All)` row, and only a genuine cross-join axis may flatten two
/// dimensions — a member list or explicit tuple is refused rather than
/// answered with the unfiltered grid (measured 2026-09-27; plan 051 review).
pub(crate) fn render_tabular_rowset<B: QueryBackend + ?Sized>(
    query: &crate::mdx_semantic::SemanticQuery,
    plan: &crate::engine::plan::QueryPlan,
    result: &crate::engine::plan::QueryResult,
    model: &crate::engine::model::SemanticModel,
    content: RowsetContent,
    backend: &B,
) -> Result<String, String> {
    use crate::engine::plan::{QueryPlan, QueryResult};

    if let Some(fault) = crate::execute::render::time_window_shape_fault(query) {
        return Err(fault);
    }
    // A crossjoin that includes a multi-level dimension needs per-level caption
    // columns (measured 2026-09-27: `Year | Quarter | Month | Category |
    // measure`, the (All) side omitting its columns); the two-dimension arm
    // writes one raw-key column per dimension. Refused rather than answered
    // with the wrong shape.
    if query.crossjoin_axis
        && query.axis_dimensions.len() == 2
        && query.axis_dimensions.iter().any(|dim| {
            model
                .dim_def_opt(dim)
                .is_some_and(|def| def.levels.len() > 1)
        })
    {
        return Err(
            "this query shape is not supported yet: a crossjoin with a multi-level dimension is              refused rather than answered with leaf-key captions"
                .to_string(),
        );
    }
    // A set function (TopCount/Order/Filter) over a *dimension* member set
    // ranks members by their own aggregates — which the fact-driven plan
    // cannot provide (measured 2026-09-27: the reference answers the top
    // members with their totals and the (All) row with the grand total; the
    // proxy answered the top leaves and a summed (All)). Refused rather than
    // answered with the wrong members.
    if query.axis_set_op.is_some()
        && query.axis_dimensions.iter().any(|dim| {
            query
                .dimension_member_sets
                .iter()
                .any(|(set_dim, _)| set_dim == dim)
        })
    {
        return Err(
            "this query shape is not supported yet: a set function over a dimension member set              is refused rather than answered with the leaf members and a summed (All)"
                .to_string(),
        );
    }
    let mut columns: Vec<(String, bool)> = Vec::new();
    let mut rows_out: Vec<Vec<String>> = Vec::new();

    match (plan, result) {
        (QueryPlan::Total { measure, .. }, QueryResult::Scalar(value)) => {
            columns.push((model.meas_def(measure).measure_unique_name(), true));
            rows_out.push(vec![g9_checked(*value)?]);
        }
        (QueryPlan::MultiMeasure { measures, .. }, QueryResult::Multi(values))
            if measures.len() == values.len() =>
        {
            for measure in measures {
                columns.push((model.meas_def(measure).measure_unique_name(), true));
            }
            rows_out.push(
                values
                    .iter()
                    .map(|value| g9_checked(*value))
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        (
            QueryPlan::GroupBy {
                measure,
                group_by,
                group_levels,
                ..
            },
            QueryResult::Grouped(groups),
        ) if group_by.len() == 1 => {
            let dimension = model.dim_def(&group_by[0]);
            let level = group_levels.first().copied().flatten();
            // One column per level up to the grouped level (every level for a
            // dimension member set): the reference's rows carry a caption per
            // level up to the member's own depth (measured 2026-09-27).
            let last = level.unwrap_or_else(|| dimension.levels.len().saturating_sub(1));
            for index in 0..=last {
                columns.push((tabular_member_column(dimension, Some(index)), false));
            }
            columns.push((model.meas_def(measure).measure_unique_name(), true));
            // A `.Members` set enumerates the dictionary with sparse measures;
            // a drilldown or aggregate stays fact-driven (measured 2026-09-27).
            let members: Vec<(String, Option<f64>)> = if let Some(members) =
                tabular_dimension_set_members(query, dimension, backend, groups)
            {
                members
            } else if let Some(paths) = tabular_set_member_paths(query, dimension, backend) {
                paths
                    .into_iter()
                    .map(|path| {
                        let value = groups
                            .iter()
                            .find(|(key, _)| *key == path)
                            .map(|(_, value)| *value);
                        (path, value)
                    })
                    .collect()
            } else {
                groups
                    .iter()
                    .map(|(key, value)| (key.clone(), Some(*value)))
                    .collect()
            };
            // A dimension member set includes the (All) member — its row
            // carries the measure only; a level set has no total row
            // (measured 2026-09-27).
            if level.is_none() || all_rooted_drilldown(query) {
                let summed: f64 = groups.iter().map(|(_, value)| value).sum();
                let total = crate::execute::render::all_value(
                    query,
                    &crate::execute::axis_members::measure_id_for_query(query),
                    summed,
                );
                let mut row = vec![String::new(); last + 1];
                row.push(g9_checked(total)?);
                rows_out.push(row);
            }
            for (key, value) in members {
                let mut row = tabular_member_captions(dimension, &key, last);
                match value {
                    Some(value) => row.push(g9_checked(value)?),
                    None => row.push(String::new()),
                }
                rows_out.push(row);
            }
        }
        (
            QueryPlan::GroupBy {
                measure,
                group_by,
                group_levels,
                ..
            },
            QueryResult::Pairs(pairs),
        ) if group_by.len() == 2
            && query.crossjoin_axis
            && group_levels.iter().all(Option::is_none) =>
        {
            let (d0, d1) = (model.dim_def(&group_by[0]), model.dim_def(&group_by[1]));
            columns.push((tabular_member_column(d0, None), false));
            columns.push((tabular_member_column(d1, None), false));
            columns.push((model.meas_def(measure).measure_unique_name(), true));
            for (k0, k1, value) in
                tabular_two_dim_rows(query, (&group_by[0], &group_by[1]), measure, pairs)
            {
                rows_out.push(vec![
                    k0.unwrap_or_default(),
                    k1.unwrap_or_default(),
                    g9_checked(value)?,
                ]);
            }
        }
        (
            QueryPlan::MultiGroupBy {
                measures,
                group_by,
                group_levels,
                ..
            },
            QueryResult::MultiGrouped(rows),
        ) if group_by.len() == 1
            && rows
                .iter()
                .all(|(_, values)| values.len() == measures.len()) =>
        {
            let dimension = model.dim_def(&group_by[0]);
            let level = group_levels.first().copied().flatten();
            let last = level.unwrap_or_else(|| dimension.levels.len().saturating_sub(1));
            for index in 0..=last {
                columns.push((tabular_member_column(dimension, Some(index)), false));
            }
            for measure in measures {
                columns.push((model.meas_def(measure).measure_unique_name(), true));
            }
            let n = measures.len();
            // A `.Members` set enumerates the dictionary with sparse measures;
            // a drilldown or aggregate stays fact-driven (measured 2026-09-27).
            let members: Vec<(String, Option<Vec<f64>>)> = if let Some(members) =
                tabular_dimension_set_members_n(query, dimension, backend, rows)
            {
                members
            } else if let Some(paths) = tabular_set_member_paths(query, dimension, backend) {
                paths
                    .into_iter()
                    .map(|path| {
                        let values = rows
                            .iter()
                            .find(|(key, _)| *key == path)
                            .map(|(_, values)| values.clone());
                        (path, values)
                    })
                    .collect()
            } else {
                rows.iter()
                    .map(|(key, values)| (key.clone(), Some(values.clone())))
                    .collect()
            };
            if level.is_none() || all_rooted_drilldown(query) {
                let mut summed = vec![0.0f64; n];
                for (_, values) in rows {
                    for (total, value) in summed.iter_mut().zip(values) {
                        *total += value;
                    }
                }
                let totals: Vec<f64> = measures
                    .iter()
                    .zip(&summed)
                    .map(|(measure, value)| {
                        crate::execute::render::all_value(query, measure, *value)
                    })
                    .collect();
                let mut all = vec![String::new(); last + 1];
                all.extend(
                    totals
                        .iter()
                        .map(|value| g9_checked(*value))
                        .collect::<Result<Vec<_>, _>>()?,
                );
                rows_out.push(all);
            }
            for (label, values) in members {
                let mut row = tabular_member_captions(dimension, &label, last);
                match values {
                    Some(values) => row.extend(
                        values
                            .iter()
                            .map(|value| g9_checked(*value))
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                    None => row.extend(std::iter::repeat_n(String::new(), n)),
                }
                rows_out.push(row);
            }
        }
        (
            QueryPlan::MultiGroupBy {
                measures,
                group_by,
                group_levels,
                ..
            },
            QueryResult::MultiGrouped2(pairs),
        ) if group_by.len() == 2
            && query.crossjoin_axis
            && group_levels.iter().all(Option::is_none)
            && pairs
                .iter()
                .all(|(_, _, values)| values.len() == measures.len()) =>
        {
            columns.push((
                tabular_member_column(model.dim_def(&group_by[0]), None),
                false,
            ));
            columns.push((
                tabular_member_column(model.dim_def(&group_by[1]), None),
                false,
            ));
            for measure in measures {
                columns.push((model.meas_def(measure).measure_unique_name(), true));
            }
            for (k0, k1, values) in
                tabular_two_dim_rows_n(query, (&group_by[0], &group_by[1]), measures, pairs)
            {
                let mut row = vec![k0.unwrap_or_default(), k1.unwrap_or_default()];
                row.extend(
                    values
                        .iter()
                        .map(|value| g9_checked(*value))
                        .collect::<Result<Vec<_>, _>>()?,
                );
                rows_out.push(row);
            }
        }
        _ => {
            return Err(
                "this query shape is not supported in the tabular format yet; it is refused \
                 rather than answered with a rowset that does not match the cells"
                    .to_string(),
            );
        }
    }

    Ok(tabular_rowset(columns, rows_out, content))
}

/// What the request asked for with `<Content>`: `SchemaData` (the default when
/// absent) carries the schema and the rows, `Schema` only the schema, `Data`
/// only the rows (measured 2026-09-27).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowsetContent {
    SchemaData,
    Schema,
    Data,
}

impl RowsetContent {
    pub(crate) fn from_property(content: Option<&str>) -> Self {
        match content.map(str::trim) {
            None => Self::SchemaData,
            Some(value) if value.eq_ignore_ascii_case("schemadata") => Self::SchemaData,
            Some(value) if value.eq_ignore_ascii_case("schema") => Self::Schema,
            Some(value) if value.eq_ignore_ascii_case("data") => Self::Data,
            // Unmeasured; the default mode is the safe reading.
            Some(_) => Self::SchemaData,
        }
    }

    fn includes_schema(self) -> bool {
        matches!(self, Self::SchemaData | Self::Schema)
    }

    fn includes_rows(self) -> bool {
        matches!(self, Self::SchemaData | Self::Data)
    }
}

/// A finite measure value in the reference's G9 form, or a refusal: `NaN` and
/// `inf` cannot carry `xsi:type="xsd:double"`.
fn g9_checked(value: f64) -> Result<String, String> {
    if !value.is_finite() {
        return Err(
            "a measure value is not finite (NaN or infinite); the tabular rowset is refused"
                .to_string(),
        );
    }
    Ok(format_g9(value))
}

/// The reference's two-dimension rowset order: the grand total, then each
/// coordinate with `(All)` on one side, then the pairs that carry data. A
/// `None` coordinate means the `(All)` member and its column is omitted.
fn tabular_two_dim_rows(
    query: &crate::mdx_semantic::SemanticQuery,
    dimensions: (&str, &str),
    measure: &str,
    pairs: &[(String, String, f64)],
) -> Vec<(Option<String>, Option<String>, f64)> {
    let triples: Vec<(String, String, Vec<f64>)> = pairs
        .iter()
        .map(|(a, b, value)| (a.clone(), b.clone(), vec![*value]))
        .collect();
    tabular_two_dim_rows_n(query, dimensions, &[measure.to_string()], &triples)
        .into_iter()
        .map(|(a, b, values)| (a, b, values[0]))
        .collect()
}

/// [`tabular_two_dim_rows`] for several measures per coordinate. Coordinates
/// are indexed once (a two-field axis can carry thousands of pairs).
fn tabular_two_dim_rows_n(
    query: &crate::mdx_semantic::SemanticQuery,
    dimensions: (&str, &str),
    measure_ids: &[String],
    triples: &[(String, String, Vec<f64>)],
) -> Vec<(Option<String>, Option<String>, Vec<f64>)> {
    use std::collections::BTreeMap;

    let n = triples.first().map(|(_, _, v)| v.len()).unwrap_or(0);
    /// One first coordinate: its totals and its second coordinates.
    type FirstCoordinate<'a> = (Vec<f64>, BTreeMap<&'a str, &'a Vec<f64>>);
    let mut by_first: BTreeMap<&str, FirstCoordinate> = BTreeMap::new();
    let mut total = vec![0.0f64; n];
    for (first, second, values) in triples {
        for (slot, value) in total.iter_mut().zip(values) {
            *slot += value;
        }
        let entry = by_first
            .entry(first.as_str())
            .or_insert_with(|| (vec![0.0f64; n], BTreeMap::<&str, &Vec<f64>>::new()));
        for (slot, value) in entry.0.iter_mut().zip(values) {
            *slot += value;
        }
        entry.1.insert(second.as_str(), values);
    }
    let mut by_second: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    for (_, second, values) in triples {
        let entry = by_second
            .entry(second.as_str())
            .or_insert_with(|| vec![0.0f64; n]);
        for (slot, value) in entry.iter_mut().zip(values) {
            *slot += value;
        }
    }

    // The `(All)`-side rows are the measure evaluated in that context, not the
    // sum of the cells (measured 2026-09-30; wrong for a ratio measure). The
    // injected per-dimension values supply the grouped member values, and the
    // injected totals supply the grand total, with the summed maps as fallback.
    let per_measure = |values: &[f64], pick: &dyn Fn(&str, f64) -> f64| -> Vec<f64> {
        values
            .iter()
            .enumerate()
            .map(|(mi, summed)| {
                let id = measure_ids.get(mi).map(String::as_str).unwrap_or_default();
                pick(id, *summed)
            })
            .collect()
    };
    let (d0, d1) = dimensions;
    let mut rows: Vec<(Option<String>, Option<String>, Vec<f64>)> = Vec::new();
    rows.push((
        None,
        None,
        per_measure(&total, &|id, summed| {
            crate::execute::render::all_value(query, id, summed)
        }),
    ));
    for (second, values) in &by_second {
        rows.push((
            None,
            Some((*second).to_string()),
            per_measure(values, &|id, summed| {
                crate::execute::render::dimension_value(query, d1, second, id).unwrap_or(summed)
            }),
        ));
    }
    for (first, (first_total, seconds)) in &by_first {
        rows.push((
            Some((*first).to_string()),
            None,
            per_measure(first_total, &|id, summed| {
                crate::execute::render::dimension_value(query, d0, first, id).unwrap_or(summed)
            }),
        ));
        for (second, values) in seconds {
            rows.push((
                Some((*first).to_string()),
                Some((*second).to_string()),
                (*values).clone(),
            ));
        }
    }
    rows
}

/// `[Dim].[Hier].[Level].[MEMBER_CAPTION]` — the reference's column for a
/// grouped member, named after the grouped level (the leaf level for a
/// dimension member set) — measured 2026-09-25 and 2026-09-27.
fn tabular_member_column(
    dimension: &crate::engine::model::DimensionDef,
    level: Option<usize>,
) -> String {
    let level_name = level
        .and_then(|index| dimension.levels.get(index))
        .map(|level| level.name.clone())
        .or_else(|| dimension.levels.last().map(|level| level.name.clone()))
        .unwrap_or_else(|| dimension.caption.clone());
    format!(
        "{}.[{}].[MEMBER_CAPTION]",
        dimension.hierarchy_unique_name(),
        level_name
    )
}

/// The captions a member's key path contributes, one per level up to `last`,
/// padded with empties for the levels below the member's own depth (the row
/// writer omits empty cells, so the reference's ragged rows come out right —
/// a year row carries one caption, a quarter row two, measured 2026-09-27).
fn tabular_member_captions(
    dimension: &crate::engine::model::DimensionDef,
    key: &str,
    last: usize,
) -> Vec<String> {
    let segments: Vec<&str> = key.split('|').collect();
    (0..=last)
        .map(|index| {
            segments
                .get(index)
                .map(|segment| {
                    if dimension.is_date_role && crate::xmla::discover::is_iso_date(segment) {
                        crate::xmla::discover::date_member_caption(segment)
                    } else {
                        (*segment).to_string()
                    }
                })
                .unwrap_or_default()
        })
        .collect()
}

/// The dictionary member paths a `.Members` set enumerates, or `None` for a
/// fact-driven shape (a drilldown or an aggregate). The reference lists the
/// dimension's members — every level for a dimension set — with a sparse
/// measure (a dataless member still has its row, without the measure element;
/// measured 2026-09-27).
fn tabular_set_member_paths<B: QueryBackend + ?Sized>(
    query: &crate::mdx_semantic::SemanticQuery,
    dimension: &crate::engine::model::DimensionDef,
    backend: &B,
) -> Option<Vec<String>> {
    // A level drag (`[Dim].[Hier].[Level].Members` or a `DrilldownLevel`
    // level target) enumerates the dictionary; a dimension-named set and a
    // fact-driven shape do not (recorded).
    if !query.level_drag {
        return None;
    }
    // A set function (TopCount/Order/Filter) or a Head/Tail wrapper scopes the
    // set: keep the fact-driven rows rather than enumerate the whole
    // dictionary (the cellset applies the prune; these arms do not).
    if query.axis_set_op.is_some()
        || query.set_probe.as_ref().is_some_and(|source| {
            !matches!(source, crate::mdx_parser::SetExpr::LevelMembers { .. })
        })
    {
        return None;
    }
    // A date-window set (`YTD`) is scoped by its window: keep the fact-driven
    // rows, like the cellset path.
    if query
        .filters
        .iter()
        .any(|filter| filter.dimension == dimension.id && filter.date_window.is_some())
    {
        return None;
    }
    // NON EMPTY keeps the fact-driven rows: the reference drops dataless
    // members there (measured 2026-09-27).
    if crate::execute::render::axis_non_empty(query, &dimension.id) {
        return None;
    }
    let index = query.drilldown_level()?;
    let dictionary = crate::axis_members::effective_dictionary(query, dimension, backend);
    Some(
        dictionary
            .level_paths
            .get(index)
            .filter(|paths| !paths.is_empty())
            .map(|paths| paths.iter().map(|path| path.join("|")).collect())
            .unwrap_or_else(|| dictionary.leaf_values.clone()),
    )
}

/// The member rows a dimension-named set (`[Dim].[Hier].Members`) enumerates:
/// every level's members with their aggregates, or `None` for a fact-driven
/// shape. The grouped rows are leaf-grain, so a mid-level member sums its
/// descendants — the reference's rows for the calendar carry year and quarter
/// totals (measured 2026-09-27). `NON EMPTY` keeps the fact-driven rows.
fn tabular_dimension_set_members<B: QueryBackend + ?Sized>(
    query: &crate::mdx_semantic::SemanticQuery,
    dimension: &crate::engine::model::DimensionDef,
    backend: &B,
    groups: &[(String, f64)],
) -> Option<Vec<(String, Option<f64>)>> {
    let named_hierarchy = query
        .dimension_member_sets
        .iter()
        .find(|(dim, _)| dim == &dimension.id)
        .map(|(_, hierarchy)| hierarchy.as_str())?;
    // The key-attribute hierarchy is single-level: the user hierarchy's levels
    // are not its members. The tabular path keeps the fact-driven rows for it
    // (recorded).
    if dimension.key_hierarchy_name() == Some(named_hierarchy) {
        return None;
    }
    // A set function (TopCount/Order/Filter) or a Head/Tail wrapper scopes the
    // set: keep the fact-driven rows rather than enumerate the whole
    // dictionary (the cellset applies the prune; these arms do not).
    if query.axis_set_op.is_some()
        || query.set_probe.as_ref().is_some_and(|source| {
            !matches!(source, crate::mdx_parser::SetExpr::LevelMembers { .. })
        })
        || crate::execute::render::axis_non_empty(query, &dimension.id)
    {
        return None;
    }
    let dictionary = crate::axis_members::effective_dictionary(query, dimension, backend);
    let leaf_index = dimension.levels.len().saturating_sub(1);
    let leaf_path_by_key: std::collections::HashMap<String, String> = dictionary
        .level_paths
        .get(leaf_index)
        .map(|paths| {
            paths
                .iter()
                .filter_map(|path| Some((path.last()?.clone(), path.join("|"))))
                .collect()
        })
        .unwrap_or_default();
    let keyed: Vec<(String, f64)> = groups
        .iter()
        .map(|(name, value)| {
            (
                leaf_path_by_key
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| name.clone()),
                *value,
            )
        })
        .collect();
    let value_for = |name: &str| -> Option<f64> {
        if let Some((_, value)) = keyed.iter().find(|(key, _)| key == name) {
            return Some(*value);
        }
        let prefix = format!("{name}|");
        let mut sum = 0.0;
        let mut found = false;
        for (key, value) in &keyed {
            if key.starts_with(&prefix) {
                sum += value;
                found = true;
            }
        }
        found.then_some(sum)
    };
    // A flat dimension carries no level paths: its keys live in `leaf_values`.
    let paths: Vec<String> = if dictionary.level_paths.is_empty() {
        dictionary.leaf_values.clone()
    } else {
        dictionary
            .level_paths
            .iter()
            .flat_map(|paths| paths.iter().map(|path| path.join("|")))
            .collect()
    };
    Some(
        paths
            .into_iter()
            .map(|path| {
                let value = value_for(&path);
                (path, value)
            })
            .collect(),
    )
}

/// [`tabular_dimension_set_members`] for several measures per coordinate.
fn tabular_dimension_set_members_n<B: QueryBackend + ?Sized>(
    query: &crate::mdx_semantic::SemanticQuery,
    dimension: &crate::engine::model::DimensionDef,
    backend: &B,
    rows: &[(String, Vec<f64>)],
) -> Option<Vec<(String, Option<Vec<f64>>)>> {
    let named_hierarchy = query
        .dimension_member_sets
        .iter()
        .find(|(dim, _)| dim == &dimension.id)
        .map(|(_, hierarchy)| hierarchy.as_str())?;
    if dimension.key_hierarchy_name() == Some(named_hierarchy)
        || query.axis_set_op.is_some()
        || query.set_probe.as_ref().is_some_and(|source| {
            !matches!(source, crate::mdx_parser::SetExpr::LevelMembers { .. })
        })
        || crate::execute::render::axis_non_empty(query, &dimension.id)
    {
        return None;
    }
    let dictionary = crate::axis_members::effective_dictionary(query, dimension, backend);
    let leaf_index = dimension.levels.len().saturating_sub(1);
    let leaf_path_by_key: std::collections::HashMap<String, String> = dictionary
        .level_paths
        .get(leaf_index)
        .map(|paths| {
            paths
                .iter()
                .filter_map(|path| Some((path.last()?.clone(), path.join("|"))))
                .collect()
        })
        .unwrap_or_default();
    let keyed: Vec<(String, Vec<f64>)> = rows
        .iter()
        .map(|(name, values)| {
            (
                leaf_path_by_key
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| name.clone()),
                values.clone(),
            )
        })
        .collect();
    let value_for = |name: &str| -> Option<Vec<f64>> {
        if let Some((_, values)) = keyed.iter().find(|(key, _)| key == name) {
            return Some(values.clone());
        }
        let prefix = format!("{name}|");
        let mut sums: Option<Vec<f64>> = None;
        for (key, values) in &keyed {
            if key.starts_with(&prefix) {
                let sums = sums.get_or_insert_with(|| vec![0.0; values.len()]);
                for (slot, value) in sums.iter_mut().zip(values) {
                    *slot += value;
                }
            }
        }
        sums
    };
    let paths: Vec<String> = if dictionary.level_paths.is_empty() {
        dictionary.leaf_values.clone()
    } else {
        dictionary
            .level_paths
            .iter()
            .flat_map(|paths| paths.iter().map(|path| path.join("|")))
            .collect()
    };
    Some(
        paths
            .into_iter()
            .map(|path| {
                let values = value_for(&path);
                (path, values)
            })
            .collect(),
    )
}

/// The reference writes doubles in a 9-significant-digit scientific form
/// (`5.21586767E8`, `4.93164E6`): trailing zeros trimmed, exponent without a
/// sign or padding (measured 2026-09-27).
fn format_g9(value: f64) -> String {
    let formatted = format!("{value:.8E}");
    match formatted.split_once('E') {
        Some((mantissa, exponent)) => {
            let mantissa = mantissa.trim_end_matches('0').trim_end_matches('.');
            format!("{mantissa}E{exponent}")
        }
        None => formatted,
    }
}

fn tabular_rowset(
    columns: Vec<(String, bool)>,
    rows: Vec<Vec<String>>,
    content: RowsetContent,
) -> String {
    let mut schema = String::new();
    if content.includes_schema() {
        schema.push_str(crate::xmla::response::ROWSET_SCHEMA_PREAMBLE);
        schema.push_str(crate::xmla::response::ROWSET_SCHEMA_ROW_OPEN);
        for (column, is_measure) in &columns {
            // A member column is typed `xsd:string`; a measure carries no type
            // (its cells tag `xsi:type="xsd:double"`), as the reference does.
            let column_type = if *is_measure {
                String::new()
            } else {
                " type=\"xsd:string\"".to_string()
            };
            schema.push_str(&format!(
                "                    <xsd:element sql:field=\"{}\" name=\"{}\"{column_type} minOccurs=\"0\"/>\n",
                crate::response::xml_escape(column),
                crate::xmla::response::encoded_element_name(column)
            ));
        }
        schema.push_str(crate::xmla::response::ROWSET_SCHEMA_CLOSE);
    }

    let mut body = String::new();
    if content.includes_rows() {
        for row in &rows {
            body.push_str("          <row>");
            for (index, (column, typed)) in columns.iter().enumerate() {
                let value = row.get(index).cloned().unwrap_or_default();
                if value.is_empty() {
                    continue;
                }
                let name = crate::xmla::response::encoded_element_name(column);
                let attribute = if *typed {
                    " xsi:type=\"xsd:double\""
                } else {
                    ""
                };
                body.push_str(&format!(
                    "<{name}{attribute}>{}</{name}>",
                    crate::response::xml_escape(&value)
                ));
            }
            body.push_str("</row>\n");
        }
    }

    format!(
        "<soap:Envelope xmlns:soap=\"http://schemas.xmlsoap.org/soap/envelope/\"><soap:Body><ExecuteResponse xmlns=\"urn:schemas-microsoft-com:xml-analysis\"><return><root xmlns=\"urn:schemas-microsoft-com:xml-analysis:rowset\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\" xmlns:msxmla=\"http://schemas.microsoft.com/analysisservices/2003/xmla\">{schema}{body}</root></return></ExecuteResponse></soap:Body></soap:Envelope>"
    )
}

/// The first dimension a plan reads that is OLS-hidden for this user, if any.
fn plan_hidden_dimension(
    plan: &crate::engine::plan::QueryPlan,
    model: &crate::engine::model::SemanticModel,
    user: &crate::engine::model::UserContext,
    config: &ProxyConfig,
) -> Option<String> {
    use crate::engine::model::{TableAccess, effective_table_filter};
    use crate::engine::plan::QueryPlan;

    let mut dimensions: Vec<&str> = Vec::new();
    match plan {
        QueryPlan::Total { filters, .. } | QueryPlan::MultiMeasure { filters, .. } => {
            dimensions.extend(filters.iter().map(|f| f.dimension.as_str()));
        }
        QueryPlan::GroupBy {
            group_by, filters, ..
        }
        | QueryPlan::MultiGroupBy {
            group_by, filters, ..
        } => {
            dimensions.extend(group_by.iter().map(String::as_str));
            dimensions.extend(filters.iter().map(|f| f.dimension.as_str()));
        }
        QueryPlan::Count { dimension } => dimensions.push(dimension),
        QueryPlan::MetaCount { dim, .. } => dimensions.push(dim),
        QueryPlan::TupleSet { cells } => {
            dimensions.extend(
                cells
                    .iter()
                    .flat_map(|cell| cell.filters.iter().map(|f| f.dimension.as_str())),
            );
        }
        _ => {}
    }

    dimensions
        .into_iter()
        .find(|dimension| {
            model.dim_def_opt(dimension).is_some_and(|def| {
                effective_table_filter(config, user, model.dim_table_for_discovery(&def.id))
                    == TableAccess::Hidden
            })
        })
        .map(str::to_string)
}

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

/// The role predicate a drillthrough can lower onto its own table, or the
/// reason it cannot: `Ok(None)` means unrestricted, `Ok(Some(sql))` means the
/// raw `SELECT *` must carry that predicate, `Err` means the honest answer is
/// still a refusal.
///
/// A filter on a *dimension* table narrows the cube space through a join the
/// drillthrough's `SELECT *` cannot make, a hidden table has no rows to show,
/// and a hidden dimension reachable from the drilled table would have its
/// columns and values returned by `SELECT *` — all refuse until drillthrough
/// can express or project them (plan 058-C).
///
/// Callers must run [`unhonourable_filter_fault`] first: a DAX-only filter
/// surfaces here as `Hidden` and faults, and the union semantics of
/// `effective_table_filter` mean another role granting full access removes the
/// restriction, exactly as the aggregate path does.
pub fn drillthrough_row_predicate(
    config: &ProxyConfig,
    user: &crate::engine::model::UserContext,
) -> Result<Option<String>, String> {
    use crate::engine::model::{TableAccess, effective_table_filter};

    if user.is_administrator {
        return Ok(None);
    }
    let project = crate::proxy_project::project();
    let model = &project.model;
    let primary = model.primary_table_name().to_string();
    let primary_fact_id = model
        .fact_tables
        .iter()
        .find(|table| table.table_name == primary)
        .map(|table| table.id.clone());

    // OLS on a dimension reachable from the drilled table: `SELECT *` returns
    // that dimension's key column (and, through the join, its values), so the
    // role would read an object it was denied. Refuse until the projection can
    // exclude hidden objects (review 2026-09-27).
    for dim in &model.dimensions {
        if crate::xmla::discover::dimension_visible(model, config, user, &dim.id) {
            continue;
        }
        let flat_on_primary = model.dim_table_for_discovery(&dim.id) == primary;
        let related_to_primary = model
            .rel_for_dimension(&dim.id)
            .is_some_and(|relationship| {
                primary_fact_id
                    .as_ref()
                    .is_some_and(|id| relationship.fact_table_id == *id)
            });
        if flat_on_primary || related_to_primary {
            return Err(format!(
                "dimension '{}' is hidden for the requesting role, and a drillthrough's \
                 SELECT * would return its columns",
                dim.id
            ));
        }
    }

    for role in config
        .roles
        .iter()
        .filter(|role| user.roles.iter().any(|name| name == &role.name))
    {
        for permission in &role.table_permissions {
            if permission.table.trim().eq_ignore_ascii_case(primary.trim()) {
                continue;
            }
            if matches!(
                effective_table_filter(config, user, &permission.table),
                TableAccess::Filtered(_)
            ) {
                return Err(format!(
                    "the row filter on '{}' cannot be applied to a drillthrough of '{primary}'",
                    permission.table
                ));
            }
        }
    }
    match effective_table_filter(config, user, &primary) {
        TableAccess::Full => Ok(None),
        TableAccess::Filtered(sql) => Ok(Some(sql)),
        TableAccess::Hidden => Err(format!(
            "'{primary}' is hidden for the requesting role: there are no rows to drill through"
        )),
    }
}

/// The refusal for a drillthrough the role predicate cannot be lowered onto.
pub fn drillthrough_refusal(reason: &str) -> String {
    crate::xmla::response::fault_response(&format!(
        "drillthrough is not available for a restricted role: {reason}"
    ))
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
            permission.dax_filter.is_some()
                && permission.filter_expression.trim().is_empty()
                // Only when the table is *still* hidden effectively: another
                // role may grant full access to it (union semantics), in which
                // case nothing leaks and nothing is refused.
                && crate::engine::model::effective_table_filter(config, user, &permission.table)
                    == crate::engine::model::TableAccess::Hidden
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

/// Member unique names a set expression names explicitly; generated sets and
/// ranges carry none the renderer would echo, and their plans already empty
/// out for a hidden source.
fn collect_set_member_targets(set: &crate::mdx_parser::SetExpr, out: &mut Vec<String>) {
    use crate::mdx_parser::SetExpr;
    match set {
        SetExpr::MemberList { unames } => {
            out.extend(unames.iter().map(|name| name.replace("&amp;", "&")))
        }
        SetExpr::Head(inner, _) | SetExpr::Tail(inner, _) => {
            collect_set_member_targets(inner, out);
        }
        _ => {}
    }
}

/// Does this user's access get narrowed by any of their roles? Administrators
/// and users without roles are unrestricted.
///
/// The answer is about *effective* access: a second role granting full access
/// to the same table wins (the documented union semantics), so a union user
/// must not be refused (plan 051 RLS review).
fn user_is_restricted(config: &ProxyConfig, user: &crate::engine::model::UserContext) -> bool {
    use crate::engine::model::{TableAccess, effective_table_filter};

    if user.is_administrator {
        return false;
    }
    config
        .roles
        .iter()
        .filter(|role| user.roles.iter().any(|name| name == &role.name))
        .flat_map(|role| role.table_permissions.iter())
        .any(|permission| {
            effective_table_filter(config, user, &permission.table) != TableAccess::Full
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::model::{default_model, resolve_user_context};

    #[test]
    fn g9_formatting_matches_the_reference() {
        assert_eq!(format_g9(521_586_767.0), "5.21586767E8");
        assert_eq!(format_g9(4_931_640.0), "4.93164E6");
        assert_eq!(format_g9(232_966.0), "2.32966E5");
        assert_eq!(format_g9(25_102_648.0), "2.5102648E7");
        assert_eq!(format_g9(-1_305.5), "-1.3055E3");
        assert_eq!(format_g9(0.0), "0E0");
        // The reference was only observed at E5..E8; the same rule is applied
        // below that range (recorded as unprobed in plan 051).
        assert_eq!(format_g9(18.01), "1.801E1");
        assert_eq!(format_g9(0.5), "5E-1");
    }
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
        use super::{unhonourable_filter_fault, user_is_restricted};
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
        assert!(user_is_restricted(&config, &user));

        // A second role granting full access to the same table wins the union:
        // nothing leaks, so nothing is refused.
        let mut union = config.clone();
        union.roles.push(RoleConfig {
            name: "Full".into(),
            description: String::new(),
            model_permission: ModelPermission::Read,
            members: vec![],
            table_permissions: vec![TablePermissionConfig {
                table: "sales_fact".into(),
                filter_expression: String::new(),
                dax_filter: None,
                metadata_permission: ModelPermission::Read,
            }],
        });
        let mut union_user = user.clone();
        union_user.roles.push("Full".into());
        assert!(
            unhonourable_filter_fault(&union, &union_user).is_none(),
            "full access from another role wins"
        );
        assert!(!user_is_restricted(&union, &union_user));
    }

    /// A `FROM` clause naming another cube, and a `<Catalog>` property naming
    /// another database, are scope faults like the reference's (measured
    /// 2026-09-25; names match case-insensitively).
    #[test]
    fn scope_mismatches_fault() {
        use super::catalog_scope_fault;
        use crate::backend::Backend;
        use crate::engine::model::UserContext;

        let project =
            crate::proxy_project::ProxyProject::load("projects/project3/proxy-config.json")
                .expect("load project3");
        crate::project::project::with_test_project(project, || {
            let config = &crate::proxy_project::project().config;
            let user = UserContext::admin_default();

            let (response, _) =
                crate::execute_builders::get_execute_cellset_response_with_backend_and_context(
                    "SELECT [Measures].[Revenue] ON 0 FROM [NoSuchCube]",
                    Backend::test_fixture(),
                    &user,
                    config,
                );
            assert!(
                response.contains("faultstring")
                    && response.contains("NoSuchCube cube does not exist"),
                "{response}"
            );

            assert!(catalog_scope_fault(Some("Elsewhere"), config, &user).is_some());
            assert!(catalog_scope_fault(Some(config.catalog.as_str()), config, &user).is_none());
            assert!(
                catalog_scope_fault(Some(&config.catalog.to_lowercase()), config, &user).is_none(),
                "catalog names match case-insensitively"
            );
            assert!(catalog_scope_fault(None, config, &user).is_none());
        });
    }

    /// A dimension hidden by OLS refuses the query rather than serving its
    /// members (plan 051 RLS review).
    #[test]
    fn hidden_dimensions_refuse_queries() {
        use crate::backend::Backend;
        use crate::engine::model::UserContext;
        use crate::project::config::{ModelPermission, RoleConfig, TablePermissionConfig};

        let project =
            crate::proxy_project::ProxyProject::load("projects/project3/proxy-config.json")
                .expect("load project3");
        crate::project::project::with_test_project(project, || {
            let project = crate::proxy_project::project();
            let mut config = project.config.clone();
            let table = project.model.dim_table_for_discovery("Date").to_string();
            config.roles = vec![RoleConfig {
                name: "OLS".into(),
                description: String::new(),
                model_permission: ModelPermission::Read,
                members: vec![],
                table_permissions: vec![TablePermissionConfig {
                    table,
                    filter_expression: String::new(),
                    dax_filter: None,
                    metadata_permission: ModelPermission::None,
                }],
            }];
            let mut user = UserContext::deny_all();
            user.roles = vec!["OLS".into()];

            let (response, _) =
                crate::execute_builders::get_execute_cellset_response_with_backend_and_context(
                    "SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales] WHERE ([Date].[Calendar].[Year].&[2020])",
                    Backend::test_fixture(),
                    &user,
                    &config,
                );
            assert!(
                response.contains("faultstring") && response.contains("hidden"),
                "{response}"
            );
        });
    }

    /// Drillthrough applies no role predicates, so a restricted user must be
    /// refused rather than served unfiltered rows.
    #[test]
    fn drillthrough_lowers_primary_filters_and_refuses_the_rest() {
        use super::drillthrough_row_predicate;
        use crate::engine::model::UserContext;
        use crate::project::config::{ModelPermission, RoleConfig, TablePermissionConfig};

        let project = crate::proxy_project::project();
        let primary = project.model.primary_table_name().to_string();
        let role = |table: &str, filter: &str, metadata: ModelPermission| RoleConfig {
            name: "EU".into(),
            description: String::new(),
            model_permission: ModelPermission::Read,
            members: vec![],
            table_permissions: vec![TablePermissionConfig {
                table: table.into(),
                filter_expression: filter.into(),
                dax_filter: None,
                metadata_permission: metadata,
            }],
        };
        let mut restricted = UserContext::deny_all();
        restricted.roles = vec!["EU".into()];
        let mut config = project.config.clone();

        // A filter on the drilled table lowers into the SQL.
        config.roles = vec![role(&primary, "territory = 'North'", ModelPermission::Read)];
        assert_eq!(
            drillthrough_row_predicate(&config, &restricted),
            Ok(Some("territory = 'North'".into())),
            "a filter on the drilled table must be applied, not refused"
        );
        // A filtered dimension table narrows the cube space through a join the
        // raw SELECT * cannot make: refuse until it can.
        config.roles = vec![role("date_dim", "year = 2024", ModelPermission::Read)];
        assert!(
            drillthrough_row_predicate(&config, &restricted).is_err(),
            "a filter on another table cannot be lowered and must refuse"
        );
        // Effective union: another matched role that names nothing grants full
        // access to every table, so the filter is not effective.
        let mut both = role("date_dim", "year = 2024", ModelPermission::Read);
        both.name = "ALL".into();
        let mut union_user = UserContext::deny_all();
        union_user.roles = vec!["EU".into(), "ALL".into()];
        config.roles = vec![role("date_dim", "year = 2024", ModelPermission::Read), {
            let mut full = role(&primary, "", ModelPermission::Read);
            full.name = "ALL".into();
            full
        }];
        assert_eq!(
            drillthrough_row_predicate(&config, &union_user),
            Ok(None),
            "a role granting full access removes the restriction (union semantics)"
        );
        // A hidden table has no rows to drill through.
        config.roles = vec![role(&primary, "", ModelPermission::None)];
        assert!(
            drillthrough_row_predicate(&config, &restricted).is_err(),
            "a hidden table must refuse"
        );
        // The administrator is unrestricted.
        assert_eq!(
            drillthrough_row_predicate(&project.config, &UserContext::admin_default()),
            Ok(None)
        );
    }

    /// Closes the review's hijack of the newly served path: a dimension hidden
    /// by OLS whose columns a `SELECT *` would return refuses the drillthrough
    /// (the projection is a follow-up, plan 058-C).
    #[test]
    fn drillthrough_refuses_when_a_reachable_dimension_is_hidden() {
        use super::drillthrough_row_predicate;
        use crate::engine::model::UserContext;
        use crate::project::config::{ModelPermission, RoleConfig, TablePermissionConfig};

        crate::tools::seed_projects_db::ensure_seeded();
        let project = crate::proxy_project::ProxyProject::load(
            "projects/generated_contoso/proxy-config.json",
        )
        .expect("load generated_contoso");
        crate::project::project::with_test_project(project, || {
            let project = crate::proxy_project::project();
            let relationship = project
                .model
                .relationships
                .first()
                .expect("the converted model has relationships");
            let dim_id = relationship.dimension_id.clone();
            let dim_table = relationship.dim_table.clone();

            let mut config = project.config.clone();
            config.roles = vec![RoleConfig {
                name: "OLS".into(),
                description: String::new(),
                model_permission: ModelPermission::Read,
                members: vec![],
                table_permissions: vec![TablePermissionConfig {
                    table: dim_table,
                    filter_expression: String::new(),
                    dax_filter: None,
                    metadata_permission: ModelPermission::None,
                }],
            }];
            let mut user = UserContext::deny_all();
            user.roles = vec!["OLS".into()];

            let reason = drillthrough_row_predicate(&config, &user)
                .err()
                .unwrap_or_default();
            assert!(
                reason.contains(&dim_id) && reason.contains("hidden"),
                "a hidden reachable dimension must refuse and name itself: {reason}"
            );
        });
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

    /// The schema must place the helper types as siblings *before* the row
    /// complexType: nesting them inside the row sequence is invalid XSD and
    /// made MSOLAP reject every tabular response (measured 2026-09-30).
    #[test]
    fn rowset_schema_helper_types_are_siblings_of_row() {
        let xml = tabular_rowset(
            vec![
                ("[Date].[Calendar].[Year].[MEMBER_CAPTION]".into(), false),
                ("[Measures].[Revenue]".into(), true),
            ],
            vec![vec!["1992".into(), "3.1E9".into()]],
            RowsetContent::SchemaData,
        );
        let schema = &xml[xml.find("<xsd:schema").expect("schema")..];
        let root = schema.find("name=\"root\"").expect("root");
        let uuid = schema.find("name=\"uuid\"").expect("uuid");
        let xml_document = schema.find("name=\"xmlDocument\"").expect("xmlDocument");
        let row = schema
            .find("<xsd:complexType name=\"row\">")
            .expect("row type");
        assert!(
            root < uuid && uuid < xml_document && xml_document < row,
            "helper types must precede the row type: {schema}"
        );
        assert!(
            xml.contains(
                "_x005B_Date_x005D_._x005B_Calendar_x005D_._x005B_Year_x005D_._x005B_MEMBER_CAPTION_x005D_"
            ),
            "{xml}"
        );
    }

    /// A measure the statement references but the model does not define faults
    /// like the reference ("The '[Bogus]' member was not found in the cube …",
    /// measured 2026-09-30); the planner used to drop unknown names and answer
    /// a default measure, silently returning different numbers. The reference
    /// also resolves names case-insensitively, faults for a *named*
    /// `[Measures].[All]`, and faults wherever the reference appears — slicer
    /// tuples, filter predicates, subselects and drillthrough statements
    /// (measured).
    #[test]
    fn unknown_measures_fault_like_the_reference() {
        use crate::backend::Backend;
        use crate::engine::model::UserContext;

        let project =
            crate::proxy_project::ProxyProject::load("projects/project3/proxy-config.json")
                .expect("load project3");
        crate::project::project::with_test_project(project, || {
            let config = &crate::proxy_project::project().config;
            let user = UserContext::admin_default();
            let execute = |mdx: &str| {
                crate::execute_builders::get_execute_cellset_response_with_backend_and_context(
                    mdx,
                    Backend::test_fixture(),
                    &user,
                    config,
                )
                .0
            };

            for mdx in [
                // Axis, slicer, a second slicer measure, a mixed set, a
                // subselect and a named `[Measures].[All]`. A *measure filter*
                // in the slicer is refused earlier (its own refusal — measured
                // 2026-10-03), so it is asserted separately below.
                "SELECT {[Measures].[Bogus]} ON 0, [Date].[Calendar].[Year].Members ON 1 FROM [Sales]",
                "SELECT [Date].[Calendar].[Year].Members ON 0 FROM [Sales] WHERE ([Measures].[Nope])",
                "SELECT [Date].[Calendar].[Year].Members ON 0 FROM [Sales] WHERE ([Measures].[Revenue],[Measures].[Bogus])",
                "SELECT {[Measures].[Revenue],[Measures].[Bogus],[Measures].[Units]} ON 0, [Date].[Calendar].[Year].Members ON 1 FROM [Sales]",
                "SELECT {[Measures].[Revenue]} ON 0 FROM (SELECT {[Measures].[Bogus]} ON 0 FROM [Sales])",
                "SELECT {[Measures].[All]} ON 0, [Date].[Calendar].[Year].Members ON 1 FROM [Sales]",
            ] {
                let response = execute(mdx);
                assert!(
                    response.contains("faultstring")
                        && response.contains("member was not found in the cube"),
                    "{mdx}\n{response}"
                );
            }

            // The reference's hidden default measure answers, a measure the
            // statement defines itself is not a model measure, and the postfix
            // set form is not a reference.
            for mdx in [
                "SELECT {[Measures].[__Default measure]} ON 0 FROM [Sales]",
                "WITH MEMBER [Measures].[XL_SD] AS 'COUNT([Date].[Calendar].[Year].Members)' SELECT {[Measures].[XL_SD]} ON 0 FROM [Sales]",
                "SELECT {[Measures].[Revenue], [Measures].Members} ON 0 FROM [Sales]",
            ] {
                let response = execute(mdx);
                assert!(!response.contains("faultstring"), "{mdx}\n{response}");
            }

            // A known measure that is not the model's first answers, and the
            // reference resolves names case-insensitively — so a case variant
            // must answer that measure, not a substituted one.
            for (mdx, wanted) in [
                (
                    "SELECT {[Measures].[Units]} ON 0, [Date].[Calendar].[Year].Members ON 1 FROM [Sales]",
                    "[Measures].[Units]",
                ),
                (
                    "SELECT {[Measures].[units]} ON 0, [Date].[Calendar].[Year].Members ON 1 FROM [Sales]",
                    "[Measures].[Units]",
                ),
            ] {
                let response = execute(mdx);
                assert!(!response.contains("faultstring"), "{mdx}\n{response}");
                assert!(response.contains(wanted), "{mdx}\n{response}");
            }

            // A measure predicate inside a slicer `Filter` is refused (the
            // lowering never reached the slicer — measured 2026-10-03).
            let response = execute(
                "SELECT {[Measures].[Revenue]} ON 0, NON EMPTY [Date].[Calendar].[Year].Members ON 1 FROM [Sales] WHERE Filter([Date].[Calendar].[Year].Members, [Measures].[Bogus] > 5)",
            );
            assert!(
                response.contains("faultstring")
                    && response.contains("measure filters in a WHERE clause"),
                "{response}"
            );

            // The drillthrough route bypasses the cellset path; it carries the
            // same refusal.
            let response = crate::execute::dispatch::get_execute_drillthrough_response(
                "DRILLTHROUGH FROM [Sales] (SELECT ([Measures].[Bogus]) ON 0 FROM [Sales])",
                Backend::test_fixture(),
            );
            assert!(
                response.contains("faultstring")
                    && response.contains("member was not found in the cube"),
                "{response}"
            );
        });
    }

    /// A drilldown's `(All)` cell is the measure *evaluated in that context*,
    /// not the sum of the axis members. Measured on the reference 2026-09-30
    /// with a ratio measure: `(All)` is the ratio of sums (18.1818…), not the
    /// sum of the members' ratios (110).
    #[test]
    fn drilldown_all_cell_evaluates_non_additive_measures() {
        use crate::backend::Backend;
        use crate::engine::model::UserContext;
        use crate::project::config::ProxyConfig;

        let project =
            crate::proxy_project::ProxyProject::load("projects/project3/proxy-config.json")
                .expect("load project3");
        let mut config: ProxyConfig = project.config.clone();
        config.measures.insert(
            0,
            serde_json::from_value(serde_json::json!({
                "id": "Revenue per unit",
                "sql_expr": "SUM(revenue) / NULLIF(SUM(units), 0)",
                "caption": "Revenue per unit",
                "measure_group_name": "Sales",
                "format_string": "0.0000"
            }))
            .expect("ratio measure config"),
        );
        let project =
            crate::proxy_project::ProxyProject::from_config(config, std::path::Path::new("."))
                .expect("build project");
        crate::project::project::with_test_project(project, || {
            let config = &crate::proxy_project::project().config;
            let user = UserContext::admin_default();
            let backend = Backend::test_fixture();
            let expected = backend.query_scalar("SELECT SUM(revenue) / SUM(units) FROM sales_fact");
            assert!(expected > 0.0, "fixture must have units");

            // Native cellset: ordinal 0 is Revenue, 1 the ratio, both on the
            // (All) row that the drilldown puts first.
            let mdx = "SELECT {[Measures].[Revenue],[Measures].[Revenue per unit]} ON 0, \
                 NON EMPTY Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}) \
                 ON 1 FROM [Sales] CELL PROPERTIES VALUE";
            let (xml, _) =
                crate::execute_builders::get_execute_cellset_response_with_backend_and_context(
                    mdx, backend, &user, config,
                );
            let all_ratio_cell = xml
                .split(r#"<Cell CellOrdinal="1">"#)
                .nth(1)
                .unwrap_or_default();
            assert!(
                all_ratio_cell.contains(&expected.to_string()),
                "the (All) ratio cell must be the ratio of sums ({expected}): {all_ratio_cell}"
            );

            // The single-measure shape (Excel with one value field) and the
            // measure-less discovery shape (which answers the model's default
            // measure — set to the ratio above) both get the evaluated (All).
            for mdx in [
                "SELECT {[Measures].[Revenue per unit]} ON 0, \
                 NON EMPTY Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}) \
                 ON 1 FROM [Sales] CELL PROPERTIES VALUE",
                "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}) \
                 ON 0 FROM [Sales] CELL PROPERTIES VALUE",
            ] {
                let (xml, _) =
                    crate::execute_builders::get_execute_cellset_response_with_backend_and_context(
                        mdx, backend, &user, config,
                    );
                let first_cell = xml
                    .split(r#"<Cell CellOrdinal="0">"#)
                    .nth(1)
                    .unwrap_or_default();
                assert!(
                    first_cell.contains(&expected.to_string()),
                    "the (All) cell must be the ratio of sums ({expected}): {mdx}\n{first_cell}"
                );
            }

            // Tabular: the (All) row comes first and carries the measures only.
            let mdx_tabular = "SELECT {[Measures].[Revenue],[Measures].[Revenue per unit]} ON 0, \
                 NON EMPTY Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}) \
                 ON 1 FROM [Sales]";
            let (xml, _) = crate::execute::runtime::get_execute_response_with_format(
                mdx_tabular,
                Some("tabular"),
                Some("Data"),
                backend,
                &user,
                config,
            );
            let first_row = xml.split("<row>").nth(1).unwrap_or_default();
            assert!(
                !first_row.contains("MEMBER_CAPTION"),
                "the (All) row carries the measures only: {first_row}"
            );
            // A cross-tab's (All)-side cells are evaluated per dimension, not
            // summed: (All, All) is the ratio of sums and (a, All)/(All, b) the
            // measure grouped by the other dimension.
            let mdx_cross = "SELECT [Category].Members ON 0, [Territory].Members ON 1 FROM [Sales] \
                 WHERE ([Measures].[Revenue per unit]) CELL PROPERTIES VALUE";
            let (xml, _) =
                crate::execute_builders::get_execute_cellset_response_with_backend_and_context(
                    mdx_cross, backend, &user, config,
                );
            assert!(
                xml.contains(&expected.to_string()),
                "the (All, All) cell must be the ratio of sums ({expected}): {xml}"
            );
            let category = backend
                .query_rows("SELECT DISTINCT category FROM sales_fact LIMIT 1")
                .first()
                .and_then(|row| row.first().cloned())
                .unwrap_or_default();
            let grouped = backend.query_scalar(&format!(
                "SELECT SUM(revenue) / SUM(units) FROM sales_fact WHERE category = '{category}'"
            ));
            assert!(
                xml.contains(&grouped.to_string()),
                "(a, All) for {category} must be the grouped ratio ({grouped}): {xml}"
            );
            // Tabular cells render in the reference's G9 form.
            let rendered = format_g9(expected);
            assert!(
                first_row.contains(&rendered),
                "the tabular (All) row must be the ratio of sums ({rendered}): {first_row}"
            );
        });
    }
}
