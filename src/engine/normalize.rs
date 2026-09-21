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
            group_levels,
            ..
        } => {
            format!(
                "groupby|measure={}|dims={}|levels={:?}",
                measure,
                group_by.join(","),
                group_levels
            ) + &filter_suffix(filters)
        }

        QueryPlan::Count { dimension } => {
            format!("count|dim={}", dimension)
        }

        QueryPlan::MetaCount {
            dim,
            group_level,
            filters,
        } => {
            format!(
                "metacount|dim={}|level={:?}{}",
                dim,
                group_level,
                filter_suffix(filters)
            )
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
            filters,
        } => {
            format!(
                "setmembers|dim={}|level={:?}|m={}{}",
                dim,
                group_level,
                measure,
                filter_suffix(filters)
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
            group_levels,
        } => {
            format!(
                "multigrp|measures={}|dims={}|levels={:?}|{}",
                measures.join(","),
                group_by.join(","),
                group_levels,
                filter_suffix(filters)
            )
        }

        QueryPlan::Empty => "empty".into(),
    }
}

fn filter_suffix(filters: &[TypedDimensionFilter]) -> String {
    if filters
        .iter()
        .all(|f| f.members.is_empty() && f.range.is_none() && f.date_window.is_none())
    {
        return String::new();
    }

    // Sort filters by dimension key for determinism
    let mut ordered: Vec<(&TypedDimensionFilter, &str)> =
        filters.iter().map(|f| (f, f.dimension.as_str())).collect();
    ordered.sort_by_key(|(_, dk)| *dk);

    let parts: Vec<String> = ordered
        .iter()
        .filter(|(f, _)| !f.members.is_empty() || f.range.is_some() || f.date_window.is_some())
        .map(|(f, dk)| {
            // Ranges and date windows must change the key too, or the result
            // cache serves one probe's response for another (and vice versa).
            if let Some(w) = &f.date_window {
                use crate::mdx::ast::DateWindow;
                let pins = |anchor: &[(String, String)]| {
                    anchor
                        .iter()
                        .map(|(l, v)| format!("{l}:{v}"))
                        .collect::<Vec<_>>()
                        .join(",")
                };
                match w {
                    DateWindow::Relative { op, amount, unit } => {
                        format!("{dk}=rel:{op:?}:{amount}{unit}")
                    }
                    DateWindow::ToDate { anchor, period } => {
                        format!("{dk}={}@{period}", pins(anchor))
                    }
                    DateWindow::Parallel {
                        anchor,
                        level,
                        offset,
                    } => format!("{dk}={}@par:{level}:{offset}", pins(anchor)),
                    DateWindow::LastPeriods {
                        anchor,
                        level,
                        count,
                    } => format!("{dk}={}@last:{level}:{count}", pins(anchor)),
                }
            } else if let Some((from, to)) = &f.range {
                format!("{dk}={from}..{to}@{}", f.level.as_deref().unwrap_or(""))
            } else {
                let mut members: Vec<&str> = f.members.iter().map(|s| s.as_str()).collect();
                members.sort();
                format!("{}={}", dk, members.join(","))
            }
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

    // Plan 047: a range filter must change the plan key, or the result cache
    // serves a range probe's response for a plain probe.
    #[test]
    fn range_filters_change_the_plan_key() {
        let ranged = TypedDimensionFilter {
            dimension: "Date".into(),
            members: vec![],
            level: Some("Year".into()),
            time_flag: None,
            range: Some(("2022".into(), "2024".into())),
            date_window: None,
        };
        let plain = TypedDimensionFilter {
            dimension: "Date".into(),
            members: vec![],
            level: Some("Year".into()),
            time_flag: None,
            range: None,
            date_window: None,
        };
        assert_ne!(
            filter_suffix(std::slice::from_ref(&ranged)),
            filter_suffix(std::slice::from_ref(&plain))
        );
        assert!(filter_suffix(&[ranged]).contains("2022..2024"));
    }

    // Plan 046 slice 4: a date window must change the plan key too.
    #[test]
    fn date_windows_change_the_plan_key() {
        let windowed = TypedDimensionFilter {
            dimension: "Date".into(),
            members: vec![],
            level: Some("Date".into()),
            time_flag: None,
            range: None,
            date_window: Some(crate::mdx::ast::DateWindow::Relative {
                op: crate::mdx::ast::CmpOp::Ge,
                amount: -30,
                unit: "day".into(),
            }),
        };
        let plain = TypedDimensionFilter {
            dimension: "Date".into(),
            members: vec![],
            level: Some("Date".into()),
            time_flag: None,
            range: None,
            date_window: None,
        };
        assert_ne!(
            filter_suffix(std::slice::from_ref(&windowed)),
            filter_suffix(std::slice::from_ref(&plain))
        );
        assert!(filter_suffix(&[windowed]).contains("-30day"));
    }

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
                range: None,
                date_window: None,
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
            group_levels: vec![],
            set_op: None,
            filters: vec![],
        };
        assert_eq!(
            plan_key(&plan),
            "groupby|measure=TotalSales|dims=ProductCategory,Region|levels=[]"
        );
    }

    #[test]
    fn group_by_with_sorted_filters() {
        let plan = QueryPlan::GroupBy {
            measure: "TotalSales".into(),
            group_by: vec!["ProductCategory".into()],
            group_levels: vec![],
            set_op: None,
            filters: vec![
                TypedDimensionFilter {
                    dimension: "Region".into(),
                    level: None,
                    time_flag: None,
                    members: vec!["North".into()],
                    range: None,
                    date_window: None,
                },
                TypedDimensionFilter {
                    dimension: "ProductCategory".into(),
                    level: None,
                    time_flag: None,
                    members: vec!["Category B".into(), "Category A".into()],
                    range: None,
                    date_window: None,
                },
            ],
        };
        let key = plan_key(&plan);
        assert_eq!(
            key,
            "groupby|measure=TotalSales|dims=ProductCategory|levels=[]|filters=ProductCategory=Category A,Category B;Region=North"
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
                    range: None,
                    date_window: None,
                },
                TypedDimensionFilter {
                    dimension: "ProductCategory".into(),
                    level: None,
                    time_flag: None,
                    members: vec!["Category A".into()],
                    range: None,
                    date_window: None,
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
                    range: None,
                    date_window: None,
                },
                TypedDimensionFilter {
                    dimension: "Region".into(),
                    level: None,
                    time_flag: None,
                    members: vec!["North".into()],
                    range: None,
                    date_window: None,
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
