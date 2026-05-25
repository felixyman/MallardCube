/// Backend-neutral query plan.
///
/// Describes what to compute, not how to format the XML response.
/// Produced from a `SemanticQuery` and consumed by the cellset
/// builders via `execute_plan`.
///
/// Designed to be translatable to both SQL (current) and Malloy (future).

use crate::mdx_semantic::{DimensionFilter, SemanticQuery, SemanticQueryKind};
use crate::backend::{Backend, QueryBackend};
use crate::engine::model::SemanticModel;
use crate::engine::sql::sql_for_query_plan;
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
    Empty,
}

// ---------------------------------------------------------------------------
// Filter helpers
// ---------------------------------------------------------------------------

fn typed_filters(source: &[DimensionFilter]) -> Vec<TypedDimensionFilter> {
    source.iter().map(|f| {
        TypedDimensionFilter {
            dimension: f.dimension.clone(),
            members: f.members.clone(),
        }
    }).collect()
}

/// Resolve an MDX-axis string to a model dimension ID, falling back
/// to `default_dim` when the string doesn't match any configured dimension.
fn resolve_dim(s: &str, model: &SemanticModel, default: DimId) -> DimId {
    model.lookup_dimension(s)
        .map(|d| d.id.clone())
        .unwrap_or(default)
}

// ---------------------------------------------------------------------------
// Plan construction
// ---------------------------------------------------------------------------

pub fn plan_from_semantic(query: &SemanticQuery) -> QueryPlan {
    let project = proxy_project::project();
    let model = &project.model;

    let meas: MeasId = query.measure.as_deref()
        .and_then(|name| model.measures.iter().find(|m| m.caption == name).map(|m| m.id.clone()))
        .unwrap_or_else(|| model.default_measure_id()
            .unwrap_or_else(|| "TotalSales".into()));
    let default_dim = model.default_dimension_id()
        .unwrap_or_else(|| "Produktkategori".into());

    let dim = query.axis_dimensions.first()
        .map(|s| s.as_str())
        .unwrap_or("");

    match query.kind {
        SemanticQueryKind::ChildrenCountForAll
        | SemanticQueryKind::ChildrenCountLeafProduct => {
            let d = if dim.is_empty() { default_dim } else { resolve_dim(dim, model, default_dim.clone()) };
            QueryPlan::Count { dimension: d }
        }

        SemanticQueryKind::ChildrenCountMeasures
        | SemanticQueryKind::MeasureChildrenEmpty
        | SemanticQueryKind::LeafChildrenEmpty => {
            QueryPlan::Empty
        }

        SemanticQueryKind::SlicerAllAndMeasure
        | SemanticQueryKind::SlicerOnly => {
            QueryPlan::Total {
                measure: meas,
                filters: typed_filters(&query.filters),
            }
        }

        SemanticQueryKind::DrilldownCategories
        | SemanticQueryKind::LeafLevelMembers
        | SemanticQueryKind::MeasureByCategory
        | SemanticQueryKind::DrilldownMemberProbe => {
            let group_by: Vec<DimId> = if query.axis_dimensions.len() >= 2 {
                query.axis_dimensions.iter()
                    .map(|a| resolve_dim(a, model, default_dim.clone()))
                    .collect()
            } else {
                let d = if dim.is_empty() { default_dim } else { resolve_dim(dim, model, default_dim) };
                vec![d]
            };
            QueryPlan::GroupBy {
                measure: meas,
                group_by,
                filters: typed_filters(&query.filters),
            }
        }

        SemanticQueryKind::AllLevelMembers => {
            QueryPlan::Total {
                measure: meas,
                filters: vec![],
            }
        }
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
/// from the Malloy runtime path.
pub fn execute_plan_with_sql(plan: &QueryPlan, sql: &str) -> QueryResult {
    execute_plan_sql_with_backend(plan, sql, Backend::get())
}

pub fn execute_plan_sql_with_backend<B: QueryBackend>(
    plan: &QueryPlan,
    sql: &str,
    backend: &B,
) -> QueryResult {
    if sql.is_empty() {
        return QueryResult::Empty;
    }
    match plan {
        QueryPlan::Total { .. } => {
            QueryResult::Scalar(backend.query_scalar(sql))
        }
        QueryPlan::GroupBy { group_by, .. } => {
            if group_by.len() >= 2 {
                QueryResult::Pairs(backend.query_pairs(sql))
            } else {
                QueryResult::Grouped(backend.query_grouped_1d(sql))
            }
        }
        QueryPlan::Count { .. } => {
            QueryResult::Count(backend.query_count(sql))
        }
        QueryPlan::Empty => QueryResult::Empty,
    }
}

pub fn execute_plan_with_backend<B: QueryBackend>(
    plan: &QueryPlan,
    model: &SemanticModel,
    backend: &B,
) -> QueryResult {
    let sql = sql_for_query_plan(model, plan);
    if sql.is_empty() {
        return QueryResult::Empty;
    }

    match plan {
        QueryPlan::Total { .. } => {
            let total = backend.query_scalar(&sql);
            QueryResult::Scalar(total)
        }

        QueryPlan::GroupBy { group_by, .. } => {
            if group_by.len() >= 2 {
                let pairs = backend.query_pairs(&sql);
                QueryResult::Pairs(pairs)
            } else {
                let rows = backend.query_grouped_1d(&sql);
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
