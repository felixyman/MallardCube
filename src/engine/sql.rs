/// SQL emitter — converts a `QueryPlan` into DuckDB SQL.
///
/// Supports:
/// - flat table access (direct physical_field)
/// - star-schema joins via relationship metadata (dimension columns live in
///   separate dimension tables)

use crate::engine::plan::{QueryPlan, TypedDimensionFilter};
use crate::engine::model::SemanticModel;
use std::collections::{HashMap, HashSet};

/// Generate SQL for the given plan.
pub fn sql_for_query_plan(model: &SemanticModel, plan: &QueryPlan) -> String {
    match plan {
        QueryPlan::Total { measure, filters } => {
            let meas = model.meas_def(measure);
            let table = &model.fact_table(meas.fact_table_idx).table_name;
            let (joins, wc) = joins_and_where(model, filters);
            format!("SELECT {} FROM {} f{}{}",
                meas.sql_expr, table, joins, wc)
        }

        QueryPlan::GroupBy { measure, group_by, filters } => {
            let meas = model.meas_def(measure);
            let table = &model.fact_table(meas.fact_table_idx).table_name;

            let (col_map, joins) = resolve_group_cols(model, group_by);
            let col_names: Vec<&str> = group_by.iter()
                .map(|d| col_map.get(d.as_str()).map(|s| s.as_str()).unwrap_or("??"))
                .collect();

            let wc = sql_where_with_cols(model, filters, &col_map);
            let group_nums: Vec<String> = (1..=col_names.len())
                .map(|i| i.to_string()).collect();
            format!(
                "SELECT {}, {} FROM {} f{}{} GROUP BY {} ORDER BY {}",
                col_names.join(", "),
                meas.sql_expr,
                table,
                joins,
                wc,
                group_nums.join(", "),
                group_nums.join(", "),
            )
        }

        QueryPlan::Count { dimension } => {
            let dim = model.dim_def(dimension);
            let (col_map, joins) = resolve_group_cols(model, &[dimension.clone()]);
            let col = col_map.get(dimension.as_str())
                .map(|s| s.as_str()).unwrap_or(&dim.physical_field);
            let from = if joins.is_empty() {
                format!("FROM {}", model.dim_table(dimension))
            } else {
                let table = &model.fact_table(0).table_name;
                format!("FROM {} f{}", table, joins)
            };
            format!("SELECT COUNT(DISTINCT {}) {}", col, from)
        }

        QueryPlan::Empty => String::new(),
    }
}

/// Resolve qualified column names and generate JOINs for group-by dimensions.
fn resolve_group_cols(
    model: &SemanticModel,
    group_by: &[String],
) -> (HashMap<String, String>, String) {
    let mut col_map: HashMap<String, String> = HashMap::new();
    let mut join_lines: Vec<String> = Vec::new();
    let mut joined: HashSet<String> = HashSet::new();

    for dim_id in group_by {
        let dim = model.dim_def(dim_id);
        if let Some(rel) = model.rel_for_dimension(dim_id) {
            let alias = format!("_{dim_id}").replace(' ', "_").to_lowercase();
            if joined.insert(alias.clone()) {
                join_lines.push(format!(
                    " JOIN {} {alias} ON f.{fact_col} = {alias}.{dim_col}",
                    rel.dim_table,
                    fact_col = rel.fact_column,
                    dim_col = rel.dim_column,
                ));
            }
            let col_name = dim.physical_field.split('.').last().unwrap_or(&dim.physical_field);
            col_map.insert(dim_id.clone(), format!("{alias}.{col_name}"));
        } else {
            col_map.insert(dim_id.clone(), dim.physical_field.clone());
        }
    }

    (col_map, join_lines.join(""))
}

/// Build WHERE clause from filters, using optional resolved column names.
fn sql_where_with_cols(
    model: &SemanticModel,
    filters: &[TypedDimensionFilter],
    col_map: &HashMap<String, String>,
) -> String {
    let parts: Vec<String> = filters.iter()
        .filter(|f| !f.members.is_empty())
        .filter_map(|f| {
            model.dim_def_opt(&f.dimension).map(|d| {
                let col = col_map.get(f.dimension.as_str())
                    .cloned()
                    .unwrap_or_else(|| d.physical_field.clone());
                (col, &f.members)
            })
        })
        .map(|(col, members)| {
            let vals: Vec<String> = members.iter()
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

/// Collect JOIN clauses and WHERE clauses for filters.
fn joins_and_where(
    model: &SemanticModel,
    filters: &[TypedDimensionFilter],
) -> (String, String) {
    let mut join_lines: Vec<String> = Vec::new();
    let mut joined: HashSet<String> = HashSet::new();
    let mut col_map: HashMap<String, String> = HashMap::new();

    for f in filters {
        if !f.members.is_empty() {
            if let Some(rel) = model.rel_for_dimension(&f.dimension) {
                let alias = format!("_{}", f.dimension).replace(' ', "_").to_lowercase();
                if joined.insert(alias.clone()) {
                    join_lines.push(format!(
                        " JOIN {} {alias} ON f.{fact_col} = {alias}.{dim_col}",
                        rel.dim_table,
                        fact_col = rel.fact_column,
                        dim_col = rel.dim_column,
                    ));
                }
                let dim = model.dim_def(&f.dimension);
                let col_name = dim.physical_field.split('.').last().unwrap_or(&dim.physical_field);
                col_map.insert(f.dimension.clone(), format!("{alias}.{col_name}"));
            }
        }
    }

    let wc = sql_where_with_cols(model, filters, &col_map);
    (join_lines.join(""), wc)
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
        assert_eq!(sql, "SELECT SUM(sales) FROM faktatabell f");
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
        assert!(sql.contains("SELECT SUM(sales) FROM faktatabell f"));
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
            "SELECT produktkategori, SUM(sales) FROM faktatabell f GROUP BY 1 ORDER BY 1"
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
            "SELECT produktkategori, region, SUM(sales) FROM faktatabell f GROUP BY 1, 2 ORDER BY 1, 2"
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

    // ---- star-schema join ----

    use crate::engine::model::{FactTable, MeasureDef, SemanticModel, Dialect, RelationshipDef, DimensionDef};

    fn star_model() -> SemanticModel {
        SemanticModel {
            fact_tables: vec![
                FactTable { id: "default".into(), source_name: "fact".into(),
                    table_name: "fact_table".into(), measure_group_name: "Fact".into() },
            ],
            dialect: Dialect::DuckDB,
            dimensions: vec![
                DimensionDef {
                    id: "Product".into(), semantic_name: "product".into(),
                    physical_field: "dim_product.product_name".into(),
                    table_name: Some("dim_product".into()), shared: false,
                    caption: "Product".into(), description: String::new(),
                    visible: true, ordinal: 1,
                    hierarchy_name: "Product".into(), all_level_name: "(All)".into(),
                    leaf_level_name: "Product".into(), cardinality_hint: 100,
                },
            ],
            measures: vec![
                MeasureDef {
                    id: "Revenue".into(), fact_table_idx: 0,
                    semantic_name: "revenue".into(), physical_expr: "revenue.sum()".into(),
                    sql_expr: "SUM(revenue)".into(), caption: "Revenue".into(),
                    display_name: "Revenue".into(), description: String::new(),
                    visible: true, aggregator: 1, units: String::new(),
                    format_string: String::new(), measure_group_name: "Fact".into(),
                    numeric_precision: 18, numeric_scale: 2, expression: String::new(),
                    sql_fallback_sql: None,
                },
            ],
            relationships: vec![
                RelationshipDef {
                    fact_table_id: "default".into(), fact_column: "product_id".into(),
                    dimension_id: "Product".into(),
                    dim_table: "dim_product".into(), dim_column: "product_id".into(),
                },
            ],
        }
    }

    #[test]
    fn total_with_relationship_join() {
        let m = star_model();
        let sql = sql_for_query_plan(&m, &QueryPlan::Total {
            measure: "Revenue".into(),
            filters: vec![TypedDimensionFilter {
                dimension: "Product".into(),
                members: vec!["Widget".into()],
            }],
        });
        assert!(sql.contains("FROM fact_table f"));
        assert!(sql.contains("JOIN dim_product _product ON f.product_id = _product.product_id"));
        assert!(sql.contains("WHERE _product.product_name IN ('Widget')"));
    }

    #[test]
    fn group_by_with_relationship_join() {
        let m = star_model();
        let sql = sql_for_query_plan(&m, &QueryPlan::GroupBy {
            measure: "Revenue".into(),
            group_by: vec!["Product".into()],
            filters: vec![],
        });
        assert!(sql.contains("FROM fact_table f"));
        assert!(sql.contains("JOIN dim_product _product ON f.product_id = _product.product_id"));
        assert!(sql.contains("SELECT _product.product_name"));
    }

    // ---- multi-fact-table ----

    fn two_fact_model() -> SemanticModel {
        SemanticModel {
            fact_tables: vec![
                FactTable { id: "sales".into(), source_name: "sales_data".into(),
                    table_name: "sales_fact".into(), measure_group_name: "Sales".into() },
                FactTable { id: "inventory".into(), source_name: "inv_data".into(),
                    table_name: "inv_fact".into(), measure_group_name: "Inventory".into() },
            ],
            dialect: Dialect::DuckDB,
            dimensions: vec![],
            measures: vec![
                MeasureDef {
                    id: "Revenue".into(), fact_table_idx: 0,
                    semantic_name: "revenue".into(), physical_expr: "revenue.sum()".into(),
                    sql_expr: "SUM(revenue)".into(), caption: "Revenue".into(),
                    display_name: "Revenue".into(), description: String::new(),
                    visible: true, aggregator: 1, units: String::new(),
                    format_string: String::new(), measure_group_name: "Sales".into(),
                    numeric_precision: 18, numeric_scale: 2, expression: String::new(),
                    sql_fallback_sql: None,
                },
                MeasureDef {
                    id: "Stock".into(), fact_table_idx: 1,
                    semantic_name: "stock".into(), physical_expr: "stock.sum()".into(),
                    sql_expr: "SUM(stock)".into(), caption: "Stock".into(),
                    display_name: "Stock".into(), description: String::new(),
                    visible: true, aggregator: 1, units: String::new(),
                    format_string: String::new(), measure_group_name: "Inventory".into(),
                    numeric_precision: 18, numeric_scale: 2, expression: String::new(),
                    sql_fallback_sql: None,
                },
            ],
            relationships: vec![],
        }
    }

    #[test]
    fn total_uses_measure_fact_table() {
        let m = two_fact_model();
        let sql = sql_for_query_plan(&m, &QueryPlan::Total { measure: "Revenue".into(), filters: vec![] });
        assert!(sql.contains("FROM sales_fact"), "Revenue should use sales_fact, got: {sql}");
        let sql = sql_for_query_plan(&m, &QueryPlan::Total { measure: "Stock".into(), filters: vec![] });
        assert!(sql.contains("FROM inv_fact"), "Stock should use inv_fact, got: {sql}");
    }

    #[test]
    fn group_by_uses_measure_fact_table() {
        let m = two_fact_model();
        let m2 = SemanticModel {
            dimensions: vec![
                DimensionDef {
                    id: "Category".into(), semantic_name: "cat".into(),
                    physical_field: "cat".into(), caption: "Category".into(),
                    description: String::new(), visible: true, ordinal: 1,
                    hierarchy_name: "Category".into(), all_level_name: "(All)".into(),
                    leaf_level_name: "Category".into(), cardinality_hint: 20,
                    table_name: None, shared: false,
                },
            ],
            ..m
        };
        let sql = sql_for_query_plan(&m2, &QueryPlan::GroupBy { measure: "Stock".into(), group_by: vec!["Category".into()], filters: vec![] });
        assert!(sql.contains("FROM inv_fact"), "Stock should use inv_fact, got: {sql}");
    }

    #[test]
    fn count_uses_dimension_table() {
        let m = SemanticModel {
            dimensions: vec![
                DimensionDef {
                    id: "Category".into(), semantic_name: "cat".into(),
                    physical_field: "cat".into(), table_name: Some("inventory_dim".into()),
                    shared: false, caption: "Category".into(), description: String::new(),
                    visible: true, ordinal: 1, hierarchy_name: "Category".into(),
                    all_level_name: "(All)".into(), leaf_level_name: "Category".into(),
                    cardinality_hint: 20,
                },
            ],
            ..two_fact_model()
        };
        let sql = sql_for_query_plan(&m, &QueryPlan::Count { dimension: "Category".into() });
        assert!(sql.contains("FROM inventory_dim"), "Count should use dimension's table: {sql}");
    }
}
