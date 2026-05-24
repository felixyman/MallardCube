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

// ---------------------------------------------------------------------------
// Semantic types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Dimension {
    Produktkategori,
    Region,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Measure {
    TotalSales,
}

impl Dimension {
    pub fn dimension_name(&self) -> &str {
        match self {
            Dimension::Produktkategori => "Produktkategori",
            Dimension::Region => "Region",
        }
    }

    pub fn malloy_name(&self) -> &str {
        match self {
            Dimension::Produktkategori => "produktkategori",
            Dimension::Region => "region",
        }
    }
}

impl Measure {
    pub fn malloy_name(&self) -> &str {
        match self {
            Measure::TotalSales => "total_forsaljning",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedDimensionFilter {
    pub dimension: Dimension,
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
        measure: Measure,
        filters: Vec<TypedDimensionFilter>,
    },

    GroupBy {
        measure: Measure,
        group_by: Vec<Dimension>,
        filters: Vec<TypedDimensionFilter>,
    },

    Count {
        dimension: Dimension,
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
        let dim = match f.dimension.as_str() {
            "Region" => Dimension::Region,
            "Produktkategori" => Dimension::Produktkategori,
            _ => Dimension::Produktkategori,
        };
        TypedDimensionFilter {
            dimension: dim,
            members: f.members.clone(),
        }
    }).collect()
}

fn axis_dimension(s: &str) -> Dimension {
    match s {
        "Region" => Dimension::Region,
        _ => Dimension::Produktkategori,
    }
}

// ---------------------------------------------------------------------------
// Plan construction
// ---------------------------------------------------------------------------

pub fn plan_from_semantic(query: &SemanticQuery) -> QueryPlan {
    let dim = query.axis_dimensions.first()
        .map(|s| s.as_str())
        .unwrap_or("Produktkategori");

    match query.kind {
        SemanticQueryKind::ChildrenCountForAll
        | SemanticQueryKind::ChildrenCountLeafProduct => {
            QueryPlan::Count { dimension: axis_dimension(dim) }
        }

        SemanticQueryKind::ChildrenCountMeasures
        | SemanticQueryKind::MeasureChildrenEmpty
        | SemanticQueryKind::LeafChildrenEmpty => {
            QueryPlan::Empty
        }

        SemanticQueryKind::SlicerAllAndMeasure
        | SemanticQueryKind::SlicerOnly => {
            QueryPlan::Total {
                measure: Measure::TotalSales,
                filters: typed_filters(&query.filters),
            }
        }

        SemanticQueryKind::DrilldownCategories
        | SemanticQueryKind::LeafLevelMembers
        | SemanticQueryKind::MeasureByCategory
        | SemanticQueryKind::DrilldownMemberProbe => {
            let group_by = if query.axis_dimensions.len() >= 2 {
                query.axis_dimensions.iter().map(|a| axis_dimension(a)).collect()
            } else {
                vec![axis_dimension(dim)]
            };
            QueryPlan::GroupBy {
                measure: Measure::TotalSales,
                group_by,
                filters: typed_filters(&query.filters),
            }
        }

        SemanticQueryKind::AllLevelMembers => {
            QueryPlan::Total {
                measure: Measure::TotalSales,
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
