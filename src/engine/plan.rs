/// Backend-neutral query plan.
///
/// Describes what to compute, not how to format the XML response.
/// Produced from a `SemanticQuery` and consumed by the cellset
/// builders via `execute_plan`.
///
/// Designed to be translatable to both SQL (current) and Malloy (future).

use crate::mdx_semantic::{DimensionFilter, SemanticQuery, SemanticQueryKind};
use crate::backend::Backend;

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
        filters: Vec<DimensionFilter>,
    },

    GroupBy {
        /// Dimensions to group by, in order (1 for single-dim, 2 for cross-join).
        dims: Vec<String>,
        filters: Vec<DimensionFilter>,
    },

    Count {
        dim: String,
    },

    Empty,
}

// ---------------------------------------------------------------------------
// Query result
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
// Plan construction
// ---------------------------------------------------------------------------

fn filters_for_dim(filters: &[DimensionFilter], dim: &str) -> Vec<String> {
    filters.iter()
        .find(|f| f.dimension == dim)
        .map(|f| f.members.clone())
        .unwrap_or_default()
}

fn kat_filter(filters: &[DimensionFilter]) -> Vec<String> {
    filters_for_dim(filters, "Produktkategori")
}

fn region_filter(filters: &[DimensionFilter]) -> Vec<String> {
    filters_for_dim(filters, "Region")
}

pub fn plan_from_semantic(query: &SemanticQuery) -> QueryPlan {
    let dim = query.axis_dimensions.first()
        .map(|s| s.as_str())
        .unwrap_or("Produktkategori");

    match query.kind {
        SemanticQueryKind::ChildrenCountForAll
        | SemanticQueryKind::ChildrenCountLeafProduct => {
            QueryPlan::Count { dim: dim.to_string() }
        }

        SemanticQueryKind::ChildrenCountMeasures
        | SemanticQueryKind::MeasureChildrenEmpty
        | SemanticQueryKind::LeafChildrenEmpty => {
            QueryPlan::Empty
        }

        SemanticQueryKind::SlicerAllAndMeasure
        | SemanticQueryKind::SlicerOnly => {
            QueryPlan::Total {
                filters: query.filters.clone(),
            }
        }

        SemanticQueryKind::DrilldownCategories
        | SemanticQueryKind::LeafLevelMembers
        | SemanticQueryKind::MeasureByCategory
        | SemanticQueryKind::DrilldownMemberProbe => {
            let dims = if query.axis_dimensions.len() >= 2 {
                query.axis_dimensions.clone()
            } else {
                vec![dim.to_string()]
            };
            QueryPlan::GroupBy {
                dims,
                filters: query.filters.clone(),
            }
        }

        SemanticQueryKind::AllLevelMembers => {
            QueryPlan::Total { filters: vec![] }
        }
    }
}

// ---------------------------------------------------------------------------
// Plan execution — calls Backend
// ---------------------------------------------------------------------------

pub fn execute_plan(plan: &QueryPlan) -> QueryResult {
    match plan {
        QueryPlan::Total { filters } => {
            let total = Backend::get().total_with_filters(
                &region_filter(filters),
                &kat_filter(filters),
            );
            QueryResult::Scalar(total)
        }

        QueryPlan::GroupBy { dims, filters } => {
            let backend = Backend::get();
            let kf = kat_filter(filters);
            let rf = region_filter(filters);

            if dims.len() >= 2 {
                let pairs = backend.grouped_pairs();
                QueryResult::Pairs(pairs)
            } else {
                let dim = dims.first().map(|s| s.as_str()).unwrap_or("Produktkategori");
                let rows: Vec<(String, f64)> = match dim {
                    "Region" => backend.grouped_by_region(&rf, &kf),
                    _ => backend.grouped_by_produktkategori(&rf, &kf),
                };
                QueryResult::Grouped(rows)
            }
        }

        QueryPlan::Count { dim } => {
            let count = match dim.as_str() {
                "Region" => Backend::get().region_count(),
                _ => Backend::get().category_count(),
            };
            QueryResult::Count(count)
        }

        QueryPlan::Empty => QueryResult::Empty,
    }
}
