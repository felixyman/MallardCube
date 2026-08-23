/// QueryPlan normalization — produces stable, sortable cache keys
/// for memoization, deduplication, and parity testing.
///
/// Two plans that would produce the same SQL should produce
/// the same key. Key format is human-readable and deterministic.
use crate::engine::plan::{QueryPlan, TypedDimensionFilter};

/// Return a stable string key for a QueryPlan.
/// Two plans that differ only in filter order produce the same key.
/// Two plans with different group_by dims produce different keys.
pub fn plan_key(plan: &QueryPlan) -> String {
    match plan {
        QueryPlan::Total { measure, filters } => {
            format!("total|measure={}", measure) + &filter_suffix(filters)
        }

        QueryPlan::GroupBy {
            measure,
            group_by,
            filters,
            ..
        } => {
            format!("groupby|measure={}|dims={}", measure, group_by.join(","),)
                + &filter_suffix(filters)
        }

        QueryPlan::Count { dimension } => {
            format!("count|dim={}", dimension)
        }

        QueryPlan::MetaCount { dim, group_level } => {
            format!("metacount|dim={}|level={:?}", dim, group_level)
        }

        QueryPlan::MetaCountLiteral(n) => {
            format!("metacountliteral|{n}")
        }

        QueryPlan::MeasuresList(m) => {
            format!("measureslist|{m}")
        }

        QueryPlan::SetMembers {
            dim,
            group_level,
            measure,
        } => {
            format!(
                "setmembers|dim={}|level={:?}|m={}",
                dim, group_level, measure
            )
        }

        QueryPlan::TupleSet { cells } => {
            let m: Vec<String> = cells
                .iter()
                .map(|c| c.measure.clone() + &filter_suffix(&c.filters))
                .collect();
            format!("tupleset|{}", m.join(";"))
        }

        QueryPlan::MultiMeasure { measures, filters } => {
            format!(
                "multi|measures={}|{}",
                measures.join(","),
                filter_suffix(filters)
            )
        }

        QueryPlan::MultiGroupBy {
            measures,
            group_by,
            filters,
            ..
        } => {
            format!(
                "multigrp|measures={}|dims={}|{}",
                measures.join(","),
                group_by.join(","),
                filter_suffix(filters)
            )
        }

        QueryPlan::Empty => "empty".into(),
    }
}

fn filter_suffix(filters: &[TypedDimensionFilter]) -> String {
    if filters.iter().all(|f| f.members.is_empty()) {
        return String::new();
    }

    // Sort filters by dimension key for determinism
    let mut ordered: Vec<(&TypedDimensionFilter, &str)> =
        filters.iter().map(|f| (f, f.dimension.as_str())).collect();
    ordered.sort_by_key(|(_, dk)| *dk);

    let parts: Vec<String> = ordered
        .iter()
        .filter(|(f, _)| !f.members.is_empty())
        .map(|(f, dk)| {
            let mut members: Vec<&str> = f.members.iter().map(|s| s.as_str()).collect();
            members.sort();
            format!("{}={}", dk, members.join(","))
        })
        .collect();

    if parts.is_empty() {
        String::new()
    } else {
        format!("|filters={}", parts.join(";"))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::plan::TypedDimensionFilter;

    #[test]
    fn total_no_filter() {
        let plan = QueryPlan::Total {
            measure: "TotalSales".into(),
            filters: vec![],
        };
        assert_eq!(plan_key(&plan), "total|measure=TotalSales");
    }

    #[test]
    fn total_single_filter() {
        let plan = QueryPlan::Total {
            measure: "TotalSales".into(),
            filters: vec![TypedDimensionFilter {
                dimension: "Region".into(),
                level: None,
                time_flag: None,
                members: vec!["North".into()],
            }],
        };
        assert_eq!(
            plan_key(&plan),
            "total|measure=TotalSales|filters=Region=North"
        );
    }

    #[test]
    fn group_by_two_dims_no_filter() {
        let plan = QueryPlan::GroupBy {
            measure: "TotalSales".into(),
            group_by: vec!["ProductCategory".into(), "Region".into()],
            group_level: None,
            set_op: None,
            filters: vec![],
        };
        assert_eq!(
            plan_key(&plan),
            "groupby|measure=TotalSales|dims=ProductCategory,Region"
        );
    }

    #[test]
    fn group_by_with_sorted_filters() {
        let plan = QueryPlan::GroupBy {
            measure: "TotalSales".into(),
            group_by: vec!["ProductCategory".into()],
            group_level: None,
            set_op: None,
            filters: vec![
                TypedDimensionFilter {
                    dimension: "Region".into(),
                    level: None,
                    time_flag: None,
                    members: vec!["North".into()],
                },
                TypedDimensionFilter {
                    dimension: "ProductCategory".into(),
                    level: None,
                    time_flag: None,
                    members: vec!["Category B".into(), "Category A".into()],
                },
            ],
        };
        let key = plan_key(&plan);
        assert_eq!(
            key,
            "groupby|measure=TotalSales|dims=ProductCategory|filters=ProductCategory=Category A,Category B;Region=North"
        );
    }

    #[test]
    fn same_key_for_reordered_filters() {
        let a = QueryPlan::Total {
            measure: "TotalSales".into(),
            filters: vec![
                TypedDimensionFilter {
                    dimension: "Region".into(),
                    level: None,
                    time_flag: None,
                    members: vec!["North".into()],
                },
                TypedDimensionFilter {
                    dimension: "ProductCategory".into(),
                    level: None,
                    time_flag: None,
                    members: vec!["Category A".into()],
                },
            ],
        };
        let b = QueryPlan::Total {
            measure: "TotalSales".into(),
            filters: vec![
                TypedDimensionFilter {
                    dimension: "ProductCategory".into(),
                    level: None,
                    time_flag: None,
                    members: vec!["Category A".into()],
                },
                TypedDimensionFilter {
                    dimension: "Region".into(),
                    level: None,
                    time_flag: None,
                    members: vec!["North".into()],
                },
            ],
        };
        assert_eq!(plan_key(&a), plan_key(&b));
    }

    #[test]
    fn count_key() {
        let plan = QueryPlan::Count {
            dimension: "ProductCategory".into(),
        };
        assert_eq!(plan_key(&plan), "count|dim=ProductCategory");
    }

    #[test]
    fn empty_key() {
        assert_eq!(plan_key(&QueryPlan::Empty), "empty");
    }
}
