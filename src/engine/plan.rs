/// Backend-neutral execution plan.
///
/// Describes what to compute, not how to format the XML response.
/// Produced from a `SemanticQuery` and consumed by the cellset
/// builders via `execute_plan`.

use crate::mdx_semantic::{DimensionFilter, SemanticQuery, SemanticQueryKind};
use crate::backend::Backend;

// ---------------------------------------------------------------------------
// Plan types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionPlan {
    /// Fetch a single scalar value.
    Total {
        filters: Vec<DimensionFilter>,
    },

    /// Fetch data grouped by one dimension.
    GroupByOneDim {
        dim: String,
        filters: Vec<DimensionFilter>,
    },

    /// Fetch all cross-product pairs (no collapse).
    GroupByTwoDims,

    /// Fetch pairs with some dimension-members collapsed.
    GroupByTwoDimsCollapse {
        excluded_members: Vec<String>,
        collapse_hierarchy: String,
        filters: Vec<DimensionFilter>,
    },

    /// Count distinct members in a dimension.
    DimensionCount {
        dim: String,
    },

    /// No backend data needed.
    Empty,
}

// ---------------------------------------------------------------------------
// Plan result types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum PlanResult {
    Scalar(f64),
    Grouped(Vec<(String, f64)>),
    Paired(Vec<(String, String, f64)>),
    PairedCollapsed {
        pairs: Vec<(String, String, f64)>,
        total_per_excluded: Vec<(String, f64)>,
    },
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

pub fn plan_from_semantic(query: &SemanticQuery) -> ExecutionPlan {
    let dim = query.axis_dimensions.first()
        .map(|s| s.as_str())
        .unwrap_or("Produktkategori");

    match query.kind {
        SemanticQueryKind::ChildrenCountForAll
        | SemanticQueryKind::ChildrenCountLeafProduct => {
            ExecutionPlan::DimensionCount { dim: dim.to_string() }
        }

        SemanticQueryKind::ChildrenCountMeasures
        | SemanticQueryKind::MeasureChildrenEmpty
        | SemanticQueryKind::LeafChildrenEmpty => {
            ExecutionPlan::Empty
        }

        SemanticQueryKind::SlicerAllAndMeasure
        | SemanticQueryKind::SlicerOnly => {
            ExecutionPlan::Total {
                filters: query.filters.clone(),
            }
        }

        SemanticQueryKind::DrilldownCategories
        | SemanticQueryKind::LeafLevelMembers
        | SemanticQueryKind::MeasureByCategory => {
            if query.axis_dimensions.len() >= 2 {
                ExecutionPlan::GroupByTwoDims
            } else {
                ExecutionPlan::GroupByOneDim {
                    dim: dim.to_string(),
                    filters: query.filters.clone(),
                }
            }
        }

        SemanticQueryKind::DrilldownMemberProbe => {
            ExecutionPlan::GroupByTwoDimsCollapse {
                excluded_members: query.excluded_members.clone(),
                collapse_hierarchy: query.drilldown_member_hierarchy
                    .clone()
                    .unwrap_or_else(|| "Region".into()),
                filters: query.filters.clone(),
            }
        }

        SemanticQueryKind::AllLevelMembers => {
            ExecutionPlan::Total { filters: vec![] }
        }
    }
}

// ---------------------------------------------------------------------------
// Plan execution — calls Backend
// ---------------------------------------------------------------------------

pub fn execute_plan(plan: &ExecutionPlan) -> PlanResult {
    match plan {
        ExecutionPlan::Total { filters } => {
            let total = Backend::get().total_with_filters(
                &region_filter(filters),
                &kat_filter(filters),
            );
            PlanResult::Scalar(total)
        }

        ExecutionPlan::GroupByOneDim { dim, filters } => {
            let backend = Backend::get();
            let kf = kat_filter(filters);
            let rf = region_filter(filters);
            let rows: Vec<(String, f64)> = match dim.as_str() {
                "Region" => backend.grouped_by_region(&rf, &kf),
                _ => backend.grouped_by_produktkategori(&rf, &kf),
            };
            PlanResult::Grouped(rows)
        }

        ExecutionPlan::GroupByTwoDims => {
            let pairs = Backend::get().grouped_pairs();
            PlanResult::Paired(pairs)
        }

        ExecutionPlan::GroupByTwoDimsCollapse {
            excluded_members,
            collapse_hierarchy,
            filters: _filters,
        } => {
            let pairs = Backend::get().grouped_pairs();
            let mut total_per_excluded: Vec<(String, f64)> = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for ex in excluded_members {
                if seen.insert(ex.clone()) {
                    let total = match collapse_hierarchy.as_str() {
                        "Region" => Backend::get().total_sales_for(ex),
                        _ => {
                            // For Produktkategori collapse, no per-member total needed;
                            // the pair value itself is used.
                            0.0
                        }
                    };
                    total_per_excluded.push((ex.clone(), total));
                }
            }
            PlanResult::PairedCollapsed {
                pairs,
                total_per_excluded,
            }
        }

        ExecutionPlan::DimensionCount { dim } => {
            let count = match dim.as_str() {
                "Region" => Backend::get().region_count(),
                _ => Backend::get().category_count(),
            };
            PlanResult::Count(count)
        }

        ExecutionPlan::Empty => PlanResult::Empty,
    }
}
