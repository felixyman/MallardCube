/// QueryPlan normalization — produces stable, sortable cache keys
/// for memoization, deduplication, and parity testing.
///
/// Two plans that would produce the same SQL should produce
/// the same key. Key format is human-readable and deterministic.
use crate::engine::plan::{QueryPlan, TypedDimensionFilter};
use crate::mdx_parser::AxisSetOp;

/// Axis set operations change the row set (and the ordering), so they must be
/// part of the plan key: without them a cached `TopCount` answer is served for
/// a `BottomCount` / `Order` / `Filter` query on the same dimension (plan 048).
fn set_op_suffix(op: &Option<AxisSetOp>) -> String {
    match op {
        None => String::new(),
        Some(AxisSetOp::TopCount { n, desc }) => format!("|setop=topcount:{n}:{desc}"),
        Some(AxisSetOp::TopCountFilter { n, desc }) => {
            format!("|setop=topcountfilter:{n}:{desc}")
        }
        Some(AxisSetOp::TopPercent { p }) => format!("|setop=toppercent:{p}"),
        Some(AxisSetOp::Order { desc }) => format!("|setop=order:{desc}"),
        Some(AxisSetOp::Filter { op, value }) => format!("|setop=filter:{op:?}:{value}"),
    }
}

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
            set_op,
        } => {
            format!(
                "groupby|measure={}|dims={}|levels={:?}{}",
                measure,
                group_by.join(","),
                group_levels,
                set_op_suffix(set_op)
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
    if filters.iter().all(|f| {
        f.members.is_empty() && f.range.is_none() && f.date_window.is_none() && f.label.is_none()
    }) {
        return String::new();
    }

    // Sort filters by dimension key for determinism
    let mut ordered: Vec<(&TypedDimensionFilter, &str)> =
        filters.iter().map(|f| (f, f.dimension.as_str())).collect();
    ordered.sort_by_key(|(_, dk)| *dk);

    let parts: Vec<String> = ordered
        .iter()
        .filter(|(f, _)| {
            !f.members.is_empty()
                || f.range.is_some()
                || f.date_window.is_some()
                || f.label.is_some()
        })
        .map(|(f, dk)| {
            // Label filters change the member set, so they must change the key
            // too (the result cache would otherwise serve one filter's members
            // for another).
            if let Some(l) = &f.label {
                return format!("{dk}=label:{l:?}");
            }
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
                    DateWindow::Absolute { op, date } => {
                        format!("{dk}=abs:{op:?}:{date}")
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
                // The level and the time flag change which column the filter
                // targets, so two member filters with the same text but a
                // different level must not share a cache key (plan 051 review).
                format!(
                    "{}={}@{}@{:?}",
                    dk,
                    members.join(","),
                    f.level.as_deref().unwrap_or(""),
                    f.time_flag
                )
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
    use crate::mdx_parser::CmpOp;

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
            label: None,
        };
        let plain = TypedDimensionFilter {
            dimension: "Date".into(),
            members: vec![],
            level: Some("Year".into()),
            time_flag: None,
            range: None,
            date_window: None,
            label: None,
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
            label: None,
        };
        let plain = TypedDimensionFilter {
            dimension: "Date".into(),
            members: vec![],
            level: Some("Date".into()),
            time_flag: None,
            range: None,
            date_window: None,
            label: None,
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
                label: None,
            }],
        };
        assert_eq!(
            plan_key(&plan),
            "total|measure=TotalSales|filters=Region=North@@None"
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
                    label: None,
                },
                TypedDimensionFilter {
                    dimension: "ProductCategory".into(),
                    level: None,
                    time_flag: None,
                    members: vec!["Category B".into(), "Category A".into()],
                    range: None,
                    date_window: None,
                    label: None,
                },
            ],
        };
        let key = plan_key(&plan);
        assert_eq!(
            key,
            "groupby|measure=TotalSales|dims=ProductCategory|levels=[]|filters=ProductCategory=Category A,Category B@@None;Region=North@@None"
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
                    label: None,
                },
                TypedDimensionFilter {
                    dimension: "ProductCategory".into(),
                    level: None,
                    time_flag: None,
                    members: vec!["Category A".into()],
                    range: None,
                    date_window: None,
                    label: None,
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
                    label: None,
                },
                TypedDimensionFilter {
                    dimension: "Region".into(),
                    level: None,
                    time_flag: None,
                    members: vec!["North".into()],
                    range: None,
                    date_window: None,
                    label: None,
                },
            ],
        };
        assert_eq!(plan_key(&a), plan_key(&b));
    }

    /// A member filter's level changes the column it targets, so two filters
    /// with the same text at different levels must not share a cache key.
    #[test]
    fn member_level_and_time_flag_change_the_plan_key() {
        let filter = |level: Option<&str>, flag: Option<&str>| TypedDimensionFilter {
            dimension: "Date".into(),
            level: level.map(str::to_string),
            time_flag: flag.map(str::to_string),
            members: vec!["1".into()],
            range: None,
            date_window: None,
            label: None,
        };
        let key = |f: TypedDimensionFilter| {
            plan_key(&QueryPlan::Total {
                measure: "Revenue".into(),
                filters: vec![f],
            })
        };
        let month = key(filter(Some("Month"), None));
        let quarter = key(filter(Some("Quarter"), None));
        let flagged = key(filter(Some("Month"), Some("ytd")));
        assert_ne!(month, quarter, "level must change the key");
        assert_ne!(month, flagged, "time flag must change the key");
    }

    #[test]
    fn label_filters_change_the_plan_key() {
        // Regression: the label filter was missing from the key, so the result
        // cache served one filter's members for another (plan 048).
        use crate::mdx::ast::LabelFilter;
        let plan = |label: Option<LabelFilter>| QueryPlan::GroupBy {
            measure: "Revenue".into(),
            group_by: vec!["Category".into()],
            filters: vec![TypedDimensionFilter {
                dimension: "Category".into(),
                members: vec![],
                level: None,
                time_flag: None,
                range: None,
                date_window: None,
                label,
            }],
            group_levels: vec![Some(0)],
            set_op: None,
        };
        let begins = plan_key(&plan(Some(LabelFilter::BeginsWith("B".into()))));
        let contains = plan_key(&plan(Some(LabelFilter::Contains("oo".into()))));
        let none = plan_key(&plan(None));
        assert_ne!(begins, contains);
        assert_ne!(begins, none);
        assert_ne!(contains, none);
        assert!(begins.contains("label:BeginsWith"), "{begins}");
    }

    #[test]
    fn count_key() {
        let plan = QueryPlan::Count {
            dimension: "ProductCategory".into(),
        };
        assert_eq!(plan_key(&plan), "count|dim=ProductCategory");
    }

    #[test]
    fn set_ops_change_the_plan_key() {
        // Regression: the set op was missing from the key, so a cached
        // TopCount answer was served for BottomCount/Order/Filter queries on
        // the same dimension (plan 048).
        let group_by = |set_op: Option<AxisSetOp>| QueryPlan::GroupBy {
            measure: "Revenue".into(),
            group_by: vec!["Category".into()],
            filters: vec![],
            group_levels: vec![Some(0)],
            set_op,
        };
        let top3 = plan_key(&group_by(Some(AxisSetOp::TopCount { n: 3, desc: true })));
        let bottom2 = plan_key(&group_by(Some(AxisSetOp::TopCount { n: 2, desc: false })));
        let order = plan_key(&group_by(Some(AxisSetOp::Order { desc: true })));
        let filter = plan_key(&group_by(Some(AxisSetOp::Filter {
            op: CmpOp::Gt,
            value: 1000.0,
        })));
        let plain = plan_key(&group_by(None));
        let keys = [&top3, &bottom2, &order, &filter, &plain];
        for (i, a) in keys.iter().enumerate() {
            for b in keys.iter().skip(i + 1) {
                assert_ne!(a, b, "set ops must be distinguishable");
            }
        }
        assert!(top3.contains("setop=topcount:3:true"), "{top3}");
        assert!(bottom2.contains("setop=topcount:2:false"), "{bottom2}");
        assert!(order.contains("setop=order:true"), "{order}");
        assert!(filter.contains("setop=filter:Gt:1000"), "{filter}");
    }

    #[test]
    fn empty_key() {
        assert_eq!(plan_key(&QueryPlan::Empty), "empty");
    }
}
