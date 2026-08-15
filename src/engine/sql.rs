use crate::engine::model::{SemanticModel, TableAccess, UserContext, effective_table_filter};
/// SQL emitter — converts a `QueryPlan` into DuckDB SQL.
///
/// Supports:
/// - flat table access (direct physical_field)
/// - star-schema joins via relationship metadata (dimension columns live in
///   separate dimension tables)
use crate::engine::plan::{QueryPlan, TypedDimensionFilter};
use crate::project::config::ProxyConfig;
use std::collections::{HashMap, HashSet};

/// Legacy: generate SQL for a plan with an admin-default (no role filtering) context.
pub fn sql_for_query_plan(model: &SemanticModel, plan: &QueryPlan) -> String {
    sql_for_query_plan_with_context(model, plan, &UserContext::admin_default(), &empty_config())
}

/// Full variant: generate SQL for a plan, injecting role-filter predicates.
///
/// Role predicates are obtained from `effective_table_filter` and:
/// - Emitted as raw SQL in the WHERE clause using `f.` (fact table) or
///   `_{dim_id}.` (dimension table) aliases.
/// - Join clauses for filtered dimension tables are emitted automatically
///   via `role_filter_join_clauses`.
///
/// The calling converter/operator is responsible for writing valid DuckDB
/// SQL fragments that match the alias convention.
pub fn sql_for_query_plan_with_context(
    model: &SemanticModel,
    plan: &QueryPlan,
    user: &UserContext,
    config: &ProxyConfig,
) -> String {
    match plan {
        QueryPlan::Total { measure, filters } => {
            let meas = model.meas_def(measure);
            let table = &model.fact_table(meas.fact_table_idx).table_name;
            let mut joined: HashSet<String> = HashSet::new();
            let (joins, wc) = joins_and_where(model, filters, &mut joined, user, config, table);
            format!("SELECT {} FROM {} f{}{}", meas.sql_expr, table, joins, wc)
        }

        QueryPlan::GroupBy {
            measure,
            group_by,
            filters,
            group_level,
        } => {
            let meas = model.meas_def(measure);
            let table = &model.fact_table(meas.fact_table_idx).table_name;

            let mut joined: HashSet<String> = HashSet::new();
            let (mut col_map, joins) =
                resolve_group_cols(model, group_by, &mut joined, user, config);
            // When drilling a specific hierarchy level, swap the dimension column
            // to the level's column (e.g. "year" instead of "full_date").
            if let (Some(level_idx), Some(dim_id)) = (group_level, group_by.first())
                && let Some(dim) = model.dim_def_opt(dim_id)
                && let Some(level) = dim.levels.get(*level_idx)
            {
                let alias_prefix = col_map
                    .get(dim_id)
                    .and_then(|v| v.rsplit_once('.').map(|(p, _)| p))
                    .unwrap_or("");
                let new_col = if alias_prefix.is_empty() {
                    level.column.clone()
                } else {
                    format!("{}.{}", alias_prefix, level.column)
                };
                col_map.insert(dim_id.clone(), new_col);
            }
            let col_names: Vec<String> = group_by
                .iter()
                .map(|d| {
                    let col = col_map.get(d.as_str()).map(|s| s.as_str()).unwrap_or("??");
                    format!("CAST({col} AS VARCHAR)")
                })
                .collect();

            // Build a WHERE column map: for the drilldown dimension at a
            // deeper level, the filter uses the parent level's column (e.g.
            // WHERE year = '2023') not the target level's column (quarter).
            let mut where_col_map = col_map.clone();
            if let (Some(level_idx), Some(dim_id)) = (group_level, group_by.first())
                && *level_idx > 0
                && let Some(dim) = model.dim_def_opt(dim_id)
                && let Some(parent_level) = dim.levels.get(level_idx - 1)
            {
                let alias_prefix = col_map
                    .get(dim_id)
                    .and_then(|v| v.rsplit_once('.').map(|(p, _)| p))
                    .unwrap_or("");
                let parent_col = if alias_prefix.is_empty() {
                    parent_level.column.clone()
                } else {
                    format!("{}.{}", alias_prefix, parent_level.column)
                };
                where_col_map.insert(dim_id.clone(), parent_col);
            }

            let wc = sql_where_with_cols(model, filters, &where_col_map, user, config, table);
            let group_nums: Vec<String> = (1..=col_names.len()).map(|i| i.to_string()).collect();
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
            let mut joined: HashSet<String> = HashSet::new();
            let (col_map, joins) = resolve_group_cols(
                model,
                std::slice::from_ref(dimension),
                &mut joined,
                user,
                config,
            );
            let col = col_map
                .get(dimension.as_str())
                .map(|s| s.as_str())
                .unwrap_or(&dim.physical_field);
            let from = if joins.is_empty() {
                format!("FROM {}", model.dim_table(dimension))
            } else {
                let table = &model.fact_table(0).table_name;
                format!("FROM {} f{}", table, joins)
            };
            format!("SELECT COUNT(DISTINCT {}) {}", col, from)
        }

        QueryPlan::MultiMeasure { .. } => String::new(),

        QueryPlan::Empty => String::new(),
    }
}

/// Return the role-filter SQL predicate for a table, or empty string when the
/// user has unfettered access (`Full` or `Hidden` — gating handled in plan.rs).
fn role_filter_predicate_for_table(
    config: &ProxyConfig,
    user: &UserContext,
    table_name: &str,
) -> String {
    match effective_table_filter(config, user, table_name) {
        TableAccess::Filtered(sql) => sql,
        TableAccess::Full | TableAccess::Hidden => String::new(),
    }
}

/// Build JOIN clauses for dimension tables that have role filters, deduping
/// against already-joined aliases via the `joined` set.
///
/// Each filtered dimension table gets a JOIN on its relationship:
/// `JOIN dim_table _dim_id ON f.fact_col = _dim_id.dim_col`
fn role_filter_join_clauses(
    model: &SemanticModel,
    user: &UserContext,
    config: &ProxyConfig,
    joined: &mut HashSet<String>,
) -> String {
    let mut joins: Vec<String> = Vec::new();
    for rel in &model.relationships {
        let access = effective_table_filter(config, user, &rel.dim_table);
        if let TableAccess::Filtered(_) = access {
            let alias = format!("_{}", rel.dimension_id)
                .replace(' ', "_")
                .to_lowercase();
            if joined.insert(alias.clone()) {
                joins.push(format!(
                    " JOIN {} {alias} ON f.{fact_col} = {alias}.{dim_col}",
                    rel.dim_table,
                    fact_col = rel.fact_column,
                    dim_col = rel.dim_column,
                ));
            }
        }
    }
    joins.join("")
}

/// Minimal empty config used by the legacy wrappers (role predicates are
/// always empty for admin-default users, so config content is irrelevant).
fn empty_config() -> ProxyConfig {
    ProxyConfig {
        catalog: String::new(),
        cube: String::new(),
        source_name: String::new(),
        table_name: String::new(),
        dialect: "duckdb".into(),
        db_path: None,
        fact_tables: vec![],
        relationships: vec![],
        roles: vec![],
        auth: None,
        time_intelligence: None,
        dimensions: vec![],
        measures: vec![],
    }
}

/// Resolve column names including role-filter JOINs deduped via `joined`.
fn resolve_group_cols(
    model: &SemanticModel,
    group_by: &[String],
    joined: &mut HashSet<String>,
    user: &UserContext,
    config: &ProxyConfig,
) -> (HashMap<String, String>, String) {
    let mut col_map: HashMap<String, String> = HashMap::new();
    let mut join_lines: Vec<String> = Vec::new();

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
            let col_name = dim
                .physical_field
                .split('.')
                .next_back()
                .unwrap_or(&dim.physical_field);
            col_map.insert(dim_id.clone(), format!("{alias}.{col_name}"));
        } else {
            col_map.insert(dim_id.clone(), dim.physical_field.clone());
        }
    }

    // Add role-filter JOINs for dimension tables with role predicates.
    let role_joins = role_filter_join_clauses(model, user, config, joined);
    join_lines.push(role_joins);

    (col_map, join_lines.join(""))
}

/// Build WHERE clause including role-filter predicates for the fact table
/// and all relationship-dimension tables.
fn sql_where_with_cols(
    model: &SemanticModel,
    filters: &[TypedDimensionFilter],
    col_map: &HashMap<String, String>,
    user: &UserContext,
    config: &ProxyConfig,
    fact_table_name: &str,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    for f in filters {
        // Time-flag filters: emit date_dim subquery
        if f.time_flag.is_some() {
            let date_dim = model
                .date_dims
                .get(&f.dimension)
                .or(model.date_dim.as_ref());
            if let (Some(dd), Some(flag)) = (date_dim, &f.time_flag) {
                parts.push(format!(
                    "f.{} IN (SELECT {} FROM {} WHERE {} = true)",
                    dd.date_key_column, dd.date_key_column, dd.table_name, flag
                ));
            }
            continue;
        }
        if f.members.is_empty() {
            continue;
        }
        if let Some(d) = model.dim_def_opt(&f.dimension) {
            let col = col_map
                .get(f.dimension.as_str())
                .cloned()
                .unwrap_or_else(|| d.physical_field.clone());
            let vals: Vec<String> = f
                .members
                .iter()
                .map(|m| format!("'{}'", m.replace('\'', "''")))
                .collect();
            parts.push(format!("{} IN ({})", col, vals.join(", ")));
        }
    }

    // Append role predicates: fact table filter first, then each relationship
    // dimension table that has a role filter.
    let fact_pred = role_filter_predicate_for_table(config, user, fact_table_name);
    if !fact_pred.is_empty() {
        parts.push(format!("({})", fact_pred));
    }
    for rel in &model.relationships {
        let dim_pred = role_filter_predicate_for_table(config, user, &rel.dim_table);
        if !dim_pred.is_empty() {
            parts.push(format!("({})", dim_pred));
        }
    }

    if parts.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", parts.join(" AND "))
    }
}

/// Collect JOINs and WHERE including role-filter JOINs and predicates.
fn joins_and_where(
    model: &SemanticModel,
    filters: &[TypedDimensionFilter],
    joined: &mut HashSet<String>,
    user: &UserContext,
    config: &ProxyConfig,
    fact_table_name: &str,
) -> (String, String) {
    let mut join_lines: Vec<String> = Vec::new();
    let mut col_map: HashMap<String, String> = HashMap::new();

    for f in filters {
        if !f.members.is_empty()
            && let Some(rel) = model.rel_for_dimension(&f.dimension)
        {
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
            let col_name = dim
                .physical_field
                .split('.')
                .next_back()
                .unwrap_or(&dim.physical_field);
            col_map.insert(f.dimension.clone(), format!("{alias}.{col_name}"));
        }
    }

    // Add role-filter JOINs for dimension tables with role predicates.
    let role_joins = role_filter_join_clauses(model, user, config, joined);
    join_lines.push(role_joins);

    let wc = sql_where_with_cols(model, filters, &col_map, user, config, fact_table_name);
    (join_lines.join(""), wc)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::model::default_model;
    use crate::engine::plan::TypedDimensionFilter;

    #[test]
    fn sql_total_no_filters() {
        let plan = QueryPlan::Total {
            measure: "TotalSales".into(),
            filters: vec![],
        };
        let sql = sql_for_query_plan(&default_model(), &plan);
        assert_eq!(sql, "SELECT SUM(sales) FROM faktatabell f");
    }

    #[test]
    fn sql_total_with_filter() {
        let plan = QueryPlan::Total {
            measure: "TotalSales".into(),
            filters: vec![TypedDimensionFilter {
                dimension: "Region".into(),
                time_flag: None,
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
            group_level: None,
            filters: vec![],
        };
        let sql = sql_for_query_plan(&default_model(), &plan);
        assert_eq!(
            sql,
            "SELECT CAST(produktkategori AS VARCHAR), SUM(sales) FROM faktatabell f GROUP BY 1 ORDER BY 1"
        );
    }

    #[test]
    fn sql_group_by_two_dims() {
        let plan = QueryPlan::GroupBy {
            measure: "TotalSales".into(),
            group_by: vec!["Produktkategori".into(), "Region".into()],
            group_level: None,
            filters: vec![],
        };
        let sql = sql_for_query_plan(&default_model(), &plan);
        assert_eq!(
            sql,
            "SELECT CAST(produktkategori AS VARCHAR), CAST(region AS VARCHAR), SUM(sales) FROM faktatabell f GROUP BY 1, 2 ORDER BY 1, 2"
        );
    }

    #[test]
    fn sql_group_by_with_filter() {
        let plan = QueryPlan::GroupBy {
            measure: "TotalSales".into(),
            group_by: vec!["Produktkategori".into()],
            group_level: None,
            filters: vec![TypedDimensionFilter {
                dimension: "Region".into(),
                time_flag: None,
                members: vec!["North".into()],
            }],
        };
        let sql = sql_for_query_plan(&default_model(), &plan);
        assert!(sql.contains("WHERE region IN ('North')"));
        assert!(sql.contains("GROUP BY 1"));
    }

    #[test]
    fn sql_count() {
        let plan = QueryPlan::Count {
            dimension: "Produktkategori".into(),
        };
        let sql = sql_for_query_plan(&default_model(), &plan);
        assert_eq!(
            sql,
            "SELECT COUNT(DISTINCT produktkategori) FROM faktatabell"
        );
    }

    #[test]
    fn sql_total_multi_filter_both_dims() {
        let plan = QueryPlan::Total {
            measure: "TotalSales".into(),
            filters: vec![
                TypedDimensionFilter {
                    dimension: "Region".into(),
                    time_flag: None,
                    members: vec!["North".into()],
                },
                TypedDimensionFilter {
                    dimension: "Produktkategori".into(),
                    time_flag: None,
                    members: vec!["Kategori A".into(), "Kategori B".into()],
                },
            ],
        };
        let sql = sql_for_query_plan(&default_model(), &plan);
        assert!(sql.contains("WHERE region IN ('North')"));
        assert!(sql.contains("AND produktkategori IN ('Kategori A', 'Kategori B')"));
    }

    // ---- star-schema join ----

    use crate::engine::model::{
        Dialect, DimensionDef, FactTable, LevelDef, MeasureDef, RelationshipDef, SemanticModel,
    };

    fn star_model() -> SemanticModel {
        SemanticModel {
            fact_tables: vec![FactTable {
                id: "default".into(),
                source_name: "fact".into(),
                table_name: "fact_table".into(),
                measure_group_name: "Fact".into(),
            }],
            dialect: Dialect::DuckDB,
            dimensions: vec![DimensionDef {
                id: "Product".into(),
                physical_field: "dim_product.product_name".into(),
                table_name: Some("dim_product".into()),
                shared: false,
                caption: "Product".into(),
                description: String::new(),
                visible: true,
                ordinal: 1,
                hierarchy_name: "Product".into(),
                all_level_name: "(All)".into(),
                leaf_level_name: "Product".into(),
                cardinality_hint: 100,
                is_date_role: false,
                levels: vec![],
            }],
            measures: vec![MeasureDef {
                id: "Revenue".into(),
                fact_table_idx: 0,
                sql_expr: "SUM(revenue)".into(),
                caption: "Revenue".into(),
                display_name: "Revenue".into(),
                description: String::new(),
                visible: true,
                aggregator: 1,
                units: String::new(),
                format_string: String::new(),
                measure_group_name: "Fact".into(),
                numeric_precision: 18,
                numeric_scale: 2,
                expression: String::new(),
                sql_fallback_sql: None,
                date_dimension_id: None,
                fallback_capability: None,
                time_flag: None,
            }],
            relationships: vec![RelationshipDef {
                fact_table_id: "default".into(),
                fact_column: "product_id".into(),
                dimension_id: "Product".into(),
                dim_table: "dim_product".into(),
                dim_column: "product_id".into(),
            }],
            date_dim: None,
            date_dims: HashMap::new(),
        }
    }

    #[test]
    fn total_with_relationship_join() {
        let m = star_model();
        let sql = sql_for_query_plan(
            &m,
            &QueryPlan::Total {
                measure: "Revenue".into(),
                filters: vec![TypedDimensionFilter {
                    dimension: "Product".into(),
                    time_flag: None,
                    members: vec!["Widget".into()],
                }],
            },
        );
        assert!(sql.contains("FROM fact_table f"));
        assert!(sql.contains("JOIN dim_product _product ON f.product_id = _product.product_id"));
        assert!(sql.contains("WHERE _product.product_name IN ('Widget')"));
    }

    #[test]
    fn group_by_with_relationship_join() {
        let m = star_model();
        let sql = sql_for_query_plan(
            &m,
            &QueryPlan::GroupBy {
                measure: "Revenue".into(),
                group_by: vec!["Product".into()],
                group_level: None,
                filters: vec![],
            },
        );
        assert!(sql.contains("FROM fact_table f"));
        assert!(sql.contains("JOIN dim_product _product ON f.product_id = _product.product_id"));
        assert!(sql.contains("SELECT CAST(_product.product_name AS VARCHAR)"));
    }

    // ---- multi-fact-table ----

    fn two_fact_model() -> SemanticModel {
        SemanticModel {
            fact_tables: vec![
                FactTable {
                    id: "sales".into(),
                    source_name: "sales_data".into(),
                    table_name: "sales_fact".into(),
                    measure_group_name: "Sales".into(),
                },
                FactTable {
                    id: "inventory".into(),
                    source_name: "inv_data".into(),
                    table_name: "inv_fact".into(),
                    measure_group_name: "Inventory".into(),
                },
            ],
            dialect: Dialect::DuckDB,
            dimensions: vec![],
            measures: vec![
                MeasureDef {
                    id: "Revenue".into(),
                    fact_table_idx: 0,
                    sql_expr: "SUM(revenue)".into(),
                    caption: "Revenue".into(),
                    display_name: "Revenue".into(),
                    description: String::new(),
                    visible: true,
                    aggregator: 1,
                    units: String::new(),
                    format_string: String::new(),
                    measure_group_name: "Sales".into(),
                    numeric_precision: 18,
                    numeric_scale: 2,
                    expression: String::new(),
                    sql_fallback_sql: None,
                    date_dimension_id: None,
                    fallback_capability: None,
                    time_flag: None,
                },
                MeasureDef {
                    id: "Stock".into(),
                    fact_table_idx: 1,
                    sql_expr: "SUM(stock)".into(),
                    caption: "Stock".into(),
                    display_name: "Stock".into(),
                    description: String::new(),
                    visible: true,
                    aggregator: 1,
                    units: String::new(),
                    format_string: String::new(),
                    measure_group_name: "Inventory".into(),
                    numeric_precision: 18,
                    numeric_scale: 2,
                    expression: String::new(),
                    sql_fallback_sql: None,
                    date_dimension_id: None,
                    fallback_capability: None,
                    time_flag: None,
                },
            ],
            relationships: vec![],
            date_dim: None,
            date_dims: HashMap::new(),
        }
    }

    #[test]
    fn total_uses_measure_fact_table() {
        let m = two_fact_model();
        let sql = sql_for_query_plan(
            &m,
            &QueryPlan::Total {
                measure: "Revenue".into(),
                filters: vec![],
            },
        );
        assert!(
            sql.contains("FROM sales_fact"),
            "Revenue should use sales_fact, got: {sql}"
        );
        let sql = sql_for_query_plan(
            &m,
            &QueryPlan::Total {
                measure: "Stock".into(),
                filters: vec![],
            },
        );
        assert!(
            sql.contains("FROM inv_fact"),
            "Stock should use inv_fact, got: {sql}"
        );
    }

    #[test]
    fn group_by_uses_measure_fact_table() {
        let m = two_fact_model();
        let m2 = SemanticModel {
            dimensions: vec![DimensionDef {
                id: "Category".into(),
                physical_field: "cat".into(),
                caption: "Category".into(),
                description: String::new(),
                visible: true,
                ordinal: 1,
                hierarchy_name: "Category".into(),
                all_level_name: "(All)".into(),
                leaf_level_name: "Category".into(),
                cardinality_hint: 20,
                is_date_role: false,
                levels: vec![],
                table_name: None,
                shared: false,
            }],
            ..m
        };
        let sql = sql_for_query_plan(
            &m2,
            &QueryPlan::GroupBy {
                measure: "Stock".into(),
                group_by: vec!["Category".into()],
                group_level: None,
                filters: vec![],
            },
        );
        assert!(
            sql.contains("FROM inv_fact"),
            "Stock should use inv_fact, got: {sql}"
        );
    }

    #[test]
    fn count_uses_dimension_table() {
        let m = SemanticModel {
            dimensions: vec![DimensionDef {
                id: "Category".into(),
                physical_field: "cat".into(),
                table_name: Some("inventory_dim".into()),
                shared: false,
                caption: "Category".into(),
                description: String::new(),
                visible: true,
                ordinal: 1,
                hierarchy_name: "Category".into(),
                all_level_name: "(All)".into(),
                leaf_level_name: "Category".into(),
                cardinality_hint: 20,
                is_date_role: false,
                levels: vec![],
            }],
            ..two_fact_model()
        };
        let sql = sql_for_query_plan(
            &m,
            &QueryPlan::Count {
                dimension: "Category".into(),
            },
        );
        assert!(
            sql.contains("FROM inventory_dim"),
            "Count should use dimension's table: {sql}"
        );
    }

    fn date_level_model() -> SemanticModel {
        SemanticModel {
            fact_tables: vec![FactTable {
                id: "default".into(),
                source_name: "fact".into(),
                table_name: "fact_table".into(),
                measure_group_name: "Fact".into(),
            }],
            dialect: Dialect::DuckDB,
            dimensions: vec![DimensionDef {
                id: "Date".into(),
                physical_field: "full_date".into(),
                table_name: Some("date_dim".into()),
                shared: false,
                caption: "Date".into(),
                description: String::new(),
                visible: true,
                ordinal: 1,
                hierarchy_name: "Date".into(),
                all_level_name: "(All)".into(),
                leaf_level_name: "Date".into(),
                cardinality_hint: 5000,
                is_date_role: true,
                levels: vec![
                    LevelDef {
                        name: "Year".into(),
                        column: "year".into(),
                        level_number: 0,
                        cardinality: 11,
                    },
                    LevelDef {
                        name: "Quarter".into(),
                        column: "quarter".into(),
                        level_number: 1,
                        cardinality: 44,
                    },
                    LevelDef {
                        name: "Month".into(),
                        column: "month".into(),
                        level_number: 2,
                        cardinality: 132,
                    },
                ],
            }],
            measures: vec![MeasureDef {
                id: "Revenue".into(),
                fact_table_idx: 0,
                sql_expr: "SUM(revenue)".into(),
                caption: "Revenue".into(),
                display_name: "Revenue".into(),
                description: String::new(),
                visible: true,
                aggregator: 1,
                units: String::new(),
                format_string: String::new(),
                measure_group_name: "Fact".into(),
                numeric_precision: 18,
                numeric_scale: 2,
                expression: String::new(),
                sql_fallback_sql: None,
                time_flag: None,
                date_dimension_id: None,
                fallback_capability: None,
            }],
            relationships: vec![RelationshipDef {
                fact_table_id: "default".into(),
                fact_column: "date_key".into(),
                dimension_id: "Date".into(),
                dim_table: "date_dim".into(),
                dim_column: "date_key".into(),
            }],
            date_dim: None,
            date_dims: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn sql_group_level_0() {
        let m = date_level_model();
        let plan = QueryPlan::GroupBy {
            measure: "Revenue".into(),
            group_by: vec!["Date".into()],
            filters: vec![],
            group_level: Some(0),
        };
        let sql = sql_for_query_plan(&m, &plan);
        assert!(
            sql.contains("CAST(_date.year AS VARCHAR)"),
            "should group by year: {sql}"
        );
        assert!(sql.contains("GROUP BY 1"), "should have GROUP BY: {sql}");
    }

    #[test]
    fn sql_group_level_1() {
        let m = date_level_model();
        let plan = QueryPlan::GroupBy {
            measure: "Revenue".into(),
            group_by: vec!["Date".into()],
            filters: vec![TypedDimensionFilter {
                dimension: "Date".into(),
                members: vec!["2024".into()],
                time_flag: None,
            }],
            group_level: Some(1),
        };
        let sql = sql_for_query_plan(&m, &plan);
        assert!(
            sql.contains("CAST(_date.quarter AS VARCHAR)"),
            "should group by quarter: {sql}"
        );
        assert!(
            sql.contains("_date.year IN ('2024')"),
            "should filter by year: {sql}"
        );
    }

    #[test]
    fn sql_no_group_level() {
        let m = date_level_model();
        let plan = QueryPlan::GroupBy {
            measure: "Revenue".into(),
            group_by: vec!["Date".into()],
            filters: vec![],
            group_level: None,
        };
        let sql = sql_for_query_plan(&m, &plan);
        assert!(
            sql.contains("CAST(_date.full_date AS VARCHAR)"),
            "should group by physical_field: {sql}"
        );
    }
}
