use crate::backend::{Backend, QueryBackend};
use crate::engine::model::{
    FallbackCapability, SemanticModel, TableAccess, UserContext, effective_model_permission,
    effective_table_filter,
};
use crate::engine::sql::sql_for_query_plan_with_context;
use crate::mdx_parser::{AxisSetOp, CmpOp};
/// Backend-neutral query plan.
///
/// Describes what to compute, not how to format the XML response.
/// Produced from a `SemanticQuery` and consumed by the cellset
/// builders via `execute_plan`.
///
/// Translates directly to SQL.
use crate::mdx_semantic::{DimensionFilter, SemanticQuery, SemanticQueryKind};
use crate::project::config::{ModelPermission, ProxyConfig};
use crate::proxy_project;

// ---------------------------------------------------------------------------
// Semantic types
// ---------------------------------------------------------------------------

/// Internal dimension identifier — matches the `id` field from proxy config,
/// or for the default model, the XMLA dimension name (e.g. "Produktkategori").
pub type DimId = String;

/// Internal measure identifier — matches the `id` field from proxy config,
/// or for the default model, the measure config id (e.g. "TotalSales").
pub type MeasId = String;

#[derive(Debug, Clone, PartialEq)]
pub struct TypedDimensionFilter {
    pub dimension: DimId,
    pub members: Vec<String>,
    /// Hierarchy level name for level-qualified filters (e.g. "Year").
    pub level: Option<String>,
    /// When set, this is a time-intelligence filter that joins date_dim
    /// on the given flag column (e.g., "ytd_flag").
    pub time_flag: Option<String>,
}

// ---------------------------------------------------------------------------
// Query plan
// ---------------------------------------------------------------------------

/// A backend-neutral description of what data to fetch.
///
/// Variants:
/// - `Total`      — single scalar (aggregate over all rows).
/// - `GroupBy`    — grouping by 1 or more dimensions. Returns one row per group.
/// - `Count`      — distinct-member count for one dimension.
/// - `Empty`      — no data needed (empty result sets, some probes).
#[derive(Debug, Clone, PartialEq)]
pub enum QueryPlan {
    Total {
        measure: MeasId,
        filters: Vec<TypedDimensionFilter>,
    },

    GroupBy {
        measure: MeasId,
        group_by: Vec<DimId>,
        filters: Vec<TypedDimensionFilter>,
        /// When drilling a multi-level hierarchy, which level index to group by.
        /// The SQL emitter uses the level's `column` instead of the dimension's
        /// `physical_field`.  None = use the leaf physical_field.
        group_level: Option<usize>,
        /// Axis set function (TopCount/Order/Filter) applied to the grouped rows.
        set_op: Option<AxisSetOp>,
    },

    /// Multiple measures on the SELECT axis with no row dimension (e.g. batched
    /// CUBEVALUE cells). Each measure is an independent scalar cell.
    MultiMeasure {
        measures: Vec<MeasId>,
        filters: Vec<TypedDimensionFilter>,
    },

    /// Multiple measures cross-joined with one or more row dimensions (e.g. a
    /// PivotTable with several measures in Values and a dimension on Rows).
    MultiGroupBy {
        measures: Vec<MeasId>,
        group_by: Vec<DimId>,
        filters: Vec<TypedDimensionFilter>,
        group_level: Option<usize>,
    },

    Count {
        dimension: DimId,
    },

    Empty,
}

// ---------------------------------------------------------------------------
// Query result (unchanged)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum QueryResult {
    Scalar(f64),
    Grouped(Vec<(String, f64)>),
    Pairs(Vec<(String, String, f64)>),
    Count(u32),
    /// One scalar per measure, in the same order as `MultiMeasure.measures`.
    Multi(Vec<f64>),
    /// One entry per group; `Vec<f64>` is one value per measure, in order.
    MultiGrouped(Vec<(String, Vec<f64>)>),
    Empty,
}

// ---------------------------------------------------------------------------
// Filter helpers
// ---------------------------------------------------------------------------

fn typed_filters(source: &[DimensionFilter]) -> Vec<TypedDimensionFilter> {
    source
        .iter()
        .map(|f| TypedDimensionFilter {
            dimension: f.dimension.clone(),
            members: f.members.clone(),
            level: f.level.clone(),
            time_flag: None,
        })
        .collect()
}

/// Build filters for a plan, adding a time_flag filter if the selected
/// measure has time_intelligence configured and the model has a date_dim.
fn filters_with_time_flag(
    model: &SemanticModel,
    meas_id: &str,
    source_filters: &[TypedDimensionFilter],
) -> Vec<TypedDimensionFilter> {
    let mut result = compatible_filters(model, meas_id, source_filters);
    if let Some(date_dim) = model.date_dim_for_measure(meas_id)
        && let Some(flag) = model.meas_def(meas_id).time_flag.as_ref()
    {
        result.push(TypedDimensionFilter {
            dimension: date_dim.dimension_id.clone(),
            members: vec![],
            level: None,
            time_flag: Some(flag.clone()),
        });
    }
    result
}

/// Apply an axis set function (TopCount/Order/Filter) to the grouped rows.
fn apply_set_op(rows: &mut Vec<(String, f64)>, op: &Option<AxisSetOp>) {
    let Some(op) = op else { return };
    match op {
        AxisSetOp::TopCount { n, desc } => {
            sort_by_value(rows, *desc);
            rows.truncate(*n);
        }
        AxisSetOp::TopPercent { p } => {
            let n = ((rows.len() as f64) * (*p / 100.0)).ceil() as usize;
            sort_by_value(rows, true);
            rows.truncate(n);
        }
        AxisSetOp::Order { desc } => sort_by_value(rows, *desc),
        AxisSetOp::Filter { op, value } => {
            rows.retain(|(_, v)| match op {
                CmpOp::Gt => *v > *value,
                CmpOp::Ge => *v >= *value,
                CmpOp::Lt => *v < *value,
                CmpOp::Le => *v <= *value,
                CmpOp::Eq => (*v - *value).abs() < 1e-9,
                CmpOp::Ne => (*v - *value).abs() >= 1e-9,
            });
        }
    }
}

fn sort_by_value(rows: &mut [(String, f64)], desc: bool) {
    rows.sort_by(|a, b| {
        if desc {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
        }
    });
}

/// Resolve an MDX-axis string to a model dimension ID, falling back
/// to `default_dim` when the string doesn't match any configured dimension.
fn resolve_dim(s: &str, model: &SemanticModel, default: DimId) -> DimId {
    model
        .lookup_dimension(s)
        .map(|d| d.id.clone())
        .unwrap_or(default)
}

/// Return only the filters that are compatible with the selected measure.
/// Unrelated dimension filters are silently ignored (matching SSAS behavior
/// for unrelated dimensions).
fn compatible_filters(
    model: &SemanticModel,
    meas_id: &str,
    filters: &[TypedDimensionFilter],
) -> Vec<TypedDimensionFilter> {
    filters
        .iter()
        .filter(|f| model.dim_is_compatible_with_measure(&f.dimension, meas_id))
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Plan construction
// ---------------------------------------------------------------------------

pub fn plan_from_semantic(query: &SemanticQuery) -> QueryPlan {
    let project = proxy_project::project();
    plan_from_semantic_with_model(query, &project.model)
}

/// Testable variant that accepts a model directly instead of reading
/// the global project singleton.
///
/// Legacy wrapper: passes admin_default() UserContext, uses global project config.
pub fn plan_from_semantic_with_model(query: &SemanticQuery, model: &SemanticModel) -> QueryPlan {
    let project = proxy_project::project();
    plan_from_semantic_with_model_and_context(
        query,
        model,
        &UserContext::admin_default(),
        &project.config,
    )
}

/// Full variant: applies role-based gating on top of plan construction.
///
/// Gating rules:
/// 1. If `effective_model_permission(config, user) == None` → `QueryPlan::Empty`
/// 2. If the plan's fact/dimension table is Hidden via OLS → `QueryPlan::Empty`
pub fn plan_from_semantic_with_model_and_context(
    query: &SemanticQuery,
    model: &SemanticModel,
    user: &UserContext,
    config: &ProxyConfig,
) -> QueryPlan {
    // Gate 1: model-level permission — None means deny all.
    if effective_model_permission(config, user) == ModelPermission::None {
        return QueryPlan::Empty;
    }

    let plan = build_plan_inner(query, model);

    // Gate 2: table-level access — Hidden means deny.
    match &plan {
        QueryPlan::Total { measure, .. } | QueryPlan::GroupBy { measure, .. } => {
            let fact_table = model.fact_table_for_measure(measure);
            if effective_table_filter(config, user, &fact_table.table_name) == TableAccess::Hidden {
                return QueryPlan::Empty;
            }
        }
        QueryPlan::MultiMeasure { measures, .. } | QueryPlan::MultiGroupBy { measures, .. } => {
            for measure in measures {
                let fact_table = model.fact_table_for_measure(measure);
                if effective_table_filter(config, user, &fact_table.table_name)
                    == TableAccess::Hidden
                {
                    return QueryPlan::Empty;
                }
            }
        }
        QueryPlan::Count { dimension } => {
            let table = model.dim_table_for_discovery(dimension);
            if effective_table_filter(config, user, table) == TableAccess::Hidden {
                return QueryPlan::Empty;
            }
        }
        QueryPlan::Empty => {}
    }

    plan
}

/// Build a plan from a semantic query without role gating.
fn build_plan_inner(query: &SemanticQuery, model: &SemanticModel) -> QueryPlan {
    // Resolve measure: use the explicitly requested one, or pick a
    // default that is compatible with the axis dimensions' fact tables.
    let meas: MeasId = query
        .measure
        .as_deref()
        .and_then(|name| model.lookup_measure(name).map(|m| m.id.clone()))
        .or_else(|| {
            for dim_id in &query.axis_dimensions {
                if let Some(dim) = model.dim_def_opt(dim_id)
                    && let Some(ref dim_table) = dim.table_name
                    && let Some(id) = model.default_measure_for_table(dim_table)
                {
                    return Some(id);
                }
            }
            None
        })
        .or_else(|| model.default_measure_id())
        .or_else(|| model.measures.first().map(|m| m.id.clone()))
        .expect("model has no measures");
    let default_dim = model
        .default_dimension_id()
        .or_else(|| model.dimensions.first().map(|d| d.id.clone()))
        .expect("model has no dimensions");

    let dim = query
        .axis_dimensions
        .first()
        .map(|s| s.as_str())
        .unwrap_or("");

    match query.kind {
        SemanticQueryKind::ChildrenCountForAll | SemanticQueryKind::ChildrenCountLeafProduct => {
            let d = if dim.is_empty() {
                default_dim
            } else {
                resolve_dim(dim, model, default_dim.clone())
            };
            QueryPlan::Count { dimension: d }
        }

        SemanticQueryKind::ChildrenCountMeasures
        | SemanticQueryKind::MeasureChildrenEmpty
        | SemanticQueryKind::LeafChildrenEmpty
        | SemanticQueryKind::MeasureMetadataProbe
        | SemanticQueryKind::MemberOnlyProbe => QueryPlan::Empty,

        SemanticQueryKind::SlicerAllAndMeasure | SemanticQueryKind::SlicerOnly => {
            if query.measures.len() > 1 {
                let measures: Vec<MeasId> = query
                    .measures
                    .iter()
                    .filter_map(|name| model.lookup_measure(name).map(|m| m.id.clone()))
                    .collect();
                if measures.len() > 1 {
                    QueryPlan::MultiMeasure {
                        measures,
                        filters: typed_filters(&query.filters),
                    }
                } else {
                    QueryPlan::Total {
                        measure: meas.clone(),
                        filters: filters_with_time_flag(
                            model,
                            &meas,
                            &typed_filters(&query.filters),
                        ),
                    }
                }
            } else {
                QueryPlan::Total {
                    measure: meas.clone(),
                    filters: filters_with_time_flag(model, &meas, &typed_filters(&query.filters)),
                }
            }
        }

        SemanticQueryKind::DrilldownCategories
        | SemanticQueryKind::LeafLevelMembers
        | SemanticQueryKind::MeasureByCategory
        | SemanticQueryKind::DrilldownMemberProbe => {
            let group_by: Vec<DimId> = if query.axis_dimensions.len() >= 2 {
                query
                    .axis_dimensions
                    .iter()
                    .map(|a| resolve_dim(a, model, default_dim.clone()))
                    .collect()
            } else {
                let d = if dim.is_empty() {
                    default_dim
                } else {
                    resolve_dim(dim, model, default_dim)
                };
                vec![d]
            };
            if query.measures.len() > 1 {
                let measures: Vec<MeasId> = query
                    .measures
                    .iter()
                    .filter_map(|name| model.lookup_measure(name).map(|m| m.id.clone()))
                    .collect();
                if measures.len() > 1 {
                    QueryPlan::MultiGroupBy {
                        measures,
                        group_by,
                        filters: typed_filters(&query.filters),
                        group_level: query.drilldown_level,
                    }
                } else {
                    QueryPlan::GroupBy {
                        measure: meas.clone(),
                        group_by,
                        filters: filters_with_time_flag(
                            model,
                            &meas,
                            &typed_filters(&query.filters),
                        ),
                        group_level: query.drilldown_level,
                        set_op: query.axis_set_op.clone(),
                    }
                }
            } else {
                QueryPlan::GroupBy {
                    measure: meas.clone(),
                    group_by,
                    filters: filters_with_time_flag(model, &meas, &typed_filters(&query.filters)),
                    group_level: query.drilldown_level,
                    set_op: query.axis_set_op.clone(),
                }
            }
        }

        SemanticQueryKind::AllLevelMembers => QueryPlan::Total {
            measure: meas,
            filters: vec![],
        },
    }
}

// ---------------------------------------------------------------------------
// Plan execution — generates SQL from plan + model, executes via Backend
// ---------------------------------------------------------------------------

pub fn execute_plan(plan: &QueryPlan, model: &SemanticModel) -> QueryResult {
    execute_plan_with_backend(plan, model, Backend::get())
}

/// Execute a plan using the given pre-compiled SQL string instead of
/// generating SQL from `sql_for_query_plan`. Used when the SQL comes
/// from a pre-compiled path.
pub fn execute_plan_with_sql(plan: &QueryPlan, sql: &str) -> QueryResult {
    execute_plan_sql_with_backend(plan, sql, Backend::get())
}

pub fn execute_plan_sql_with_backend<B: QueryBackend + ?Sized>(
    plan: &QueryPlan,
    sql: &str,
    backend: &B,
) -> QueryResult {
    if sql.is_empty() {
        return QueryResult::Empty;
    }
    match plan {
        QueryPlan::Total { .. } => QueryResult::Scalar(backend.query_scalar(sql)),
        QueryPlan::GroupBy {
            group_by, set_op, ..
        } => {
            if group_by.len() >= 2 {
                QueryResult::Pairs(backend.query_pairs(sql))
            } else {
                let mut rows = backend.query_grouped_1d(sql);
                apply_set_op(&mut rows, set_op);
                QueryResult::Grouped(rows)
            }
        }
        QueryPlan::MultiMeasure { .. } | QueryPlan::MultiGroupBy { .. } => QueryResult::Empty,
        QueryPlan::Count { .. } => QueryResult::Count(backend.query_count(sql)),
        QueryPlan::Empty => QueryResult::Empty,
    }
}

/// Execute a plan with user-context-aware SQL generation.
/// Falls back to admin-default SQL only when no fallback SQL is configured.
/// Role predicates are appended to generated SQL for non-admin users.
pub fn execute_plan_with_backend_and_context<B: QueryBackend + ?Sized>(
    plan: &QueryPlan,
    model: &SemanticModel,
    backend: &B,
    user: &UserContext,
    config: &ProxyConfig,
) -> QueryResult {
    // A multi-measure plan is just N independent single-measure scalars, each
    // with its own time-flag/filter resolution. Recurse per measure.
    if let QueryPlan::MultiMeasure { measures, filters } = plan {
        let mut values = Vec::with_capacity(measures.len());
        for measure in measures {
            let per_measure = QueryPlan::Total {
                measure: measure.clone(),
                filters: filters_with_time_flag(model, measure, filters),
            };
            match execute_plan_with_backend_and_context(&per_measure, model, backend, user, config)
            {
                QueryResult::Scalar(v) => values.push(v),
                _ => values.push(0.0),
            }
        }
        return QueryResult::Multi(values);
    }

    // Multi-measure cross-join: N measures × M groups. Execute each measure's
    // GroupBy (each with its own time-flag/filter resolution) and merge by group
    // label, so a time-flag measure that filters rows still aligns with the
    // unfiltered measures' groups (missing groups get 0.0).
    if let QueryPlan::MultiGroupBy {
        measures,
        group_by,
        filters,
        group_level,
    } = plan
    {
        let mut per_measure: Vec<Vec<(String, f64)>> = Vec::with_capacity(measures.len());
        for measure in measures {
            let per_measure_plan = QueryPlan::GroupBy {
                measure: measure.clone(),
                group_by: group_by.clone(),
                filters: filters_with_time_flag(model, measure, filters),
                group_level: *group_level,
                set_op: None,
            };
            match execute_plan_with_backend_and_context(
                &per_measure_plan,
                model,
                backend,
                user,
                config,
            ) {
                QueryResult::Grouped(rows) => per_measure.push(rows),
                _ => per_measure.push(Vec::new()),
            }
        }

        // Merge by label: preserve the first measure's group order, then append
        // any groups that only appear in later measures.
        let mut order: Vec<String> = Vec::new();
        let mut columns: Vec<std::collections::HashMap<String, f64>> =
            vec![std::collections::HashMap::new(); measures.len()];
        for (mi, rows) in per_measure.iter().enumerate() {
            for (label, value) in rows {
                if !order.contains(label) {
                    order.push(label.clone());
                }
                columns[mi].insert(label.clone(), *value);
            }
        }
        let merged: Vec<(String, Vec<f64>)> = order
            .iter()
            .map(|label| {
                let vals = columns
                    .iter()
                    .map(|col| col.get(label).copied().unwrap_or(0.0))
                    .collect();
                (label.clone(), vals)
            })
            .collect();
        return QueryResult::MultiGrouped(merged);
    }

    // If the plan's measure has pre-loaded fallback SQL, use it instead of
    // generating SQL from the plan. Role predicates are NOT applied to
    // fallback SQL (pre-written static SQL). This is a documented limitation.
    let fallback_result = match plan {
        QueryPlan::Total { measure, .. } | QueryPlan::GroupBy { measure, .. } => {
            match model.classify_fallback(measure) {
                Some(FallbackCapability::Stub) => {
                    eprintln!(
                        "plan: measure '{}' fallback SQL is a TODO stub — returning empty",
                        measure
                    );
                    Some(QueryResult::Empty)
                }
                Some(FallbackCapability::ScalarOnly) => match plan {
                    QueryPlan::Total { .. } => None,
                    QueryPlan::GroupBy { .. } => {
                        eprintln!(
                            "plan: measure '{}' fallback SQL is scalar-only, cannot satisfy GroupBy — returning empty",
                            measure
                        );
                        Some(QueryResult::Empty)
                    }
                    _ => Some(QueryResult::Empty),
                },
                Some(FallbackCapability::GroupedSpecific(ref dims)) => match plan {
                    QueryPlan::GroupBy { group_by, .. } => {
                        if group_by == dims {
                            None
                        } else {
                            eprintln!(
                                "plan: measure '{}' fallback SQL only supports grouping by {:?}, got {:?} — returning empty",
                                measure, dims, group_by
                            );
                            Some(QueryResult::Empty)
                        }
                    }
                    _ => {
                        eprintln!(
                            "plan: measure '{}' fallback SQL is grouped-specific, cannot satisfy non-GroupBy plan — returning empty",
                            measure
                        );
                        Some(QueryResult::Empty)
                    }
                },
                Some(FallbackCapability::Universal) => None,
                None => None,
            }
        }
        _ => None,
    };
    if let Some(early) = fallback_result {
        return early;
    }

    let fallback_sql = match plan {
        QueryPlan::Total { measure, .. } | QueryPlan::GroupBy { measure, .. } => {
            model.meas_def(measure).sql_fallback_sql.as_deref()
        }
        _ => None,
    };
    // Use context-aware SQL generation when not using pre-written fallback SQL.
    let sql = fallback_sql
        .map(|s| s.to_string())
        .unwrap_or_else(|| sql_for_query_plan_with_context(model, plan, user, config));

    if sql.is_empty() {
        return QueryResult::Empty;
    }

    match plan {
        QueryPlan::Total { .. } => {
            let total = backend.query_scalar(&sql);
            QueryResult::Scalar(total)
        }

        QueryPlan::MultiMeasure { .. } | QueryPlan::MultiGroupBy { .. } => QueryResult::Empty,

        QueryPlan::GroupBy {
            group_by, set_op, ..
        } => {
            if group_by.len() >= 2 {
                let pairs = backend.query_pairs(&sql);
                QueryResult::Pairs(pairs)
            } else {
                let mut rows = backend.query_grouped_1d(&sql);
                apply_set_op(&mut rows, set_op);
                QueryResult::Grouped(rows)
            }
        }

        QueryPlan::Count { .. } => {
            let count = backend.query_count(&sql);
            QueryResult::Count(count)
        }

        QueryPlan::Empty => QueryResult::Empty,
    }
}

pub fn execute_plan_with_backend<B: QueryBackend + ?Sized>(
    plan: &QueryPlan,
    model: &SemanticModel,
    backend: &B,
) -> QueryResult {
    // Legacy wrapper: uses admin-default user context (no role filtering).
    execute_plan_with_backend_and_context(
        plan,
        model,
        backend,
        &UserContext::admin_default(),
        &proxy_project::project().config,
    )
}
