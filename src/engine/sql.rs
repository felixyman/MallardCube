/// SQL emitter — converts a `QueryPlan` into SQL text for the current
/// backend database (SQLite/DuckDB-compatible).
///
/// Uses the `SemanticModel` for physical field mappings.
/// Produces parameterised-like SQL with inline literal values for
/// the supported demo subset.

use crate::engine::plan::{QueryPlan, TypedDimensionFilter};
use crate::engine::model::SemanticModel;

/// Generate SQL for the given plan, using the semantic model's physical
/// field mappings. Empty plan returns an empty string.
pub fn sql_for_query_plan(model: &SemanticModel, plan: &QueryPlan) -> String {
    match plan {
        QueryPlan::Total { measure, filters } => {
            let meas = model.meas_def(measure);
            let wc = sql_where(model, filters);
            format!("SELECT {} FROM {}{}",
                meas.sql_expr, model.table_name, wc)
        }

        QueryPlan::GroupBy { measure, group_by, filters } => {
            let meas = model.meas_def(measure);
            let dim_cols: Vec<&str> = group_by.iter()
                .map(|d| model.dim_def(d).physical_field.as_str())
                .collect();
            let wc = sql_where(model, filters);
            let group_nums: Vec<String> = (1..=dim_cols.len())
                .map(|i| i.to_string()).collect();
            format!(
                "SELECT {}, {} FROM {}{} GROUP BY {} ORDER BY {}",
                dim_cols.join(", "),
                meas.sql_expr,
                model.table_name,
                wc,
                group_nums.join(", "),
                group_nums.join(", "),
            )
        }

        QueryPlan::Count { dimension } => {
            let dim = model.dim_def(dimension);
            format!(
                "SELECT COUNT(DISTINCT {}) FROM {}",
                dim.physical_field, model.table_name,
            )
        }

        QueryPlan::Empty => String::new(),
    }
}

fn sql_where(model: &SemanticModel, filters: &[TypedDimensionFilter]) -> String {
    let parts: Vec<String> = filters.iter()
        .filter(|f| !f.members.is_empty())
        .map(|f| {
            let col = &model.dim_def(&f.dimension).physical_field;
            let vals: Vec<String> = f.members.iter()
                .map(|m| format!("'{}'", m.replace('\'', "''")))
                .collect();
            format!("{} IN ({})", col, vals.join(", "))
        })
        .collect();

    if parts.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", parts.join(" AND "))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::plan::TypedDimensionFilter;
    use crate::engine::model::default_model;

    #[test]
    fn sql_total_no_filters() {
        let plan = QueryPlan::Total { measure: "TotalSales".into(), filters: vec![] };
        let sql = sql_for_query_plan(&default_model(), &plan);
        assert_eq!(sql, "SELECT SUM(sales) FROM faktatabell");
    }

    #[test]
    fn sql_total_with_filter() {
        let plan = QueryPlan::Total {
            measure: "TotalSales".into(),
            filters: vec![TypedDimensionFilter {
                dimension: "Region".into(),
                members: vec!["North".into()],
            }],
        };
        let sql = sql_for_query_plan(&default_model(), &plan);
        assert!(sql.contains("SELECT SUM(sales) FROM faktatabell"));
        assert!(sql.contains("WHERE region IN ('North')"));
    }

    #[test]
    fn sql_group_by_one_dim() {
        let plan = QueryPlan::GroupBy {
            measure: "TotalSales".into(),
            group_by: vec!["Produktkategori".into()],
            filters: vec![],
        };
        let sql = sql_for_query_plan(&default_model(), &plan);
        assert_eq!(
            sql,
            "SELECT produktkategori, SUM(sales) FROM faktatabell GROUP BY 1 ORDER BY 1"
        );
    }

    #[test]
    fn sql_group_by_two_dims() {
        let plan = QueryPlan::GroupBy {
            measure: "TotalSales".into(),
            group_by: vec!["Produktkategori".into(), "Region".into()],
            filters: vec![],
        };
        let sql = sql_for_query_plan(&default_model(), &plan);
        assert_eq!(
            sql,
            "SELECT produktkategori, region, SUM(sales) FROM faktatabell GROUP BY 1, 2 ORDER BY 1, 2"
        );
    }

    #[test]
    fn sql_group_by_with_filter() {
        let plan = QueryPlan::GroupBy {
            measure: "TotalSales".into(),
            group_by: vec!["Produktkategori".into()],
            filters: vec![TypedDimensionFilter {
                dimension: "Region".into(),
                members: vec!["North".into()],
            }],
        };
        let sql = sql_for_query_plan(&default_model(), &plan);
        assert!(sql.contains("WHERE region IN ('North')"));
        assert!(sql.contains("GROUP BY 1"));
    }

    #[test]
    fn sql_count() {
        let plan = QueryPlan::Count { dimension: "Produktkategori".into() };
        let sql = sql_for_query_plan(&default_model(), &plan);
        assert_eq!(sql, "SELECT COUNT(DISTINCT produktkategori) FROM faktatabell");
    }

    #[test]
    fn sql_total_multi_filter_both_dims() {
        let plan = QueryPlan::Total {
            measure: "TotalSales".into(),
            filters: vec![
                TypedDimensionFilter {
                    dimension: "Region".into(),
                    members: vec!["North".into()],
                },
                TypedDimensionFilter {
                    dimension: "Produktkategori".into(),
                    members: vec!["Kategori A".into(), "Kategori B".into()],
                },
            ],
        };
        let sql = sql_for_query_plan(&default_model(), &plan);
        assert!(sql.contains("WHERE region IN ('North')"));
        assert!(sql.contains("AND produktkategori IN ('Kategori A', 'Kategori B')"));
    }
}
