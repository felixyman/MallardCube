use crate::engine::aggregate::{self, AGG_ALIAS, Aggregation};
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
        // The count is known at parse time; no SQL needed.
        QueryPlan::MetaCountLiteral(_) => String::new(),
        // Measures render from the model; no SQL needed.
        QueryPlan::MeasuresList(_) => String::new(),
        QueryPlan::SetMembers {
            dim,
            group_level,
            measure,
        } => {
            // Mirror the GroupBy emitter (joins + OLS), but group by the FULL
            // ancestor path so compound members stay distinct.
            let d = model.dim_def(dim);
            let meas = model.meas_def(measure);
            let table = &model.fact_table(meas.fact_table_idx).table_name;
            let mut joined: HashSet<String> = HashSet::new();
            let (col_map, joins) =
                resolve_group_cols(model, std::slice::from_ref(dim), &mut joined, user, config);
            let alias_prefix = col_map
                .get(dim.as_str())
                .and_then(|v| v.rsplit_once('.').map(|(p, _)| p))
                .unwrap_or("")
                .to_string();
            let qual = |col: &str| -> String {
                if alias_prefix.is_empty() {
                    col.to_string()
                } else {
                    format!("{alias_prefix}.{col}")
                }
            };
            let path = match group_level.and_then(|i| d.levels.get(i)) {
                Some(_) => {
                    let depth = group_level.unwrap_or(0);
                    let cols: Vec<String> = d.levels[..=depth]
                        .iter()
                        .map(|l| format!("CAST({} AS VARCHAR)", qual(&l.column)))
                        .collect();
                    if cols.len() == 1 {
                        cols[0].clone()
                    } else {
                        format!("CONCAT_WS('|', {})", cols.join(", "))
                    }
                }
                None => format!("CAST({} AS VARCHAR)", qual(&d.physical_field)),
            };
            format!(
                "SELECT {path} AS __path, {} FROM {table} f{joins} GROUP BY 1 ORDER BY 1",
                meas.sql_expr
            )
        }
        QueryPlan::Total { measure, filters } => {
            if let Some(agg) = route_plan(model, &aggregate::aggregations(), plan, user, config) {
                return agg_total_sql(model, agg, measure, filters);
            }
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
            group_levels,
            ..
        } => {
            // A dimension at a non-unique level with several expanded parents
            // must group by its full ancestor path; the date-aggregation
            // fast path only knows single date columns, so skip it then.
            let needs_path = group_by.iter().enumerate().any(|(i, dim_id)| {
                let Some(level_idx) = group_levels.get(i).copied().flatten() else {
                    return false;
                };
                let Some(dim) = model.dim_def_opt(dim_id) else {
                    return false;
                };
                let single_parent = filters
                    .iter()
                    .any(|f| f.dimension == *dim_id && f.members.len() == 1);
                level_idx > 0 && level_idx + 1 != dim.levels.len() && !single_parent
            });
            if !needs_path
                && let Some(agg) = route_plan(model, &aggregate::aggregations(), plan, user, config)
            {
                return agg_groupby_sql(
                    model,
                    agg,
                    measure,
                    group_by,
                    &group_levels.first().copied().flatten(),
                    filters,
                );
            }
            let meas = model.meas_def(measure);
            let table = &model.fact_table(meas.fact_table_idx).table_name;

            let mut joined: HashSet<String> = HashSet::new();
            let (mut col_map, joins) =
                resolve_group_cols(model, group_by, &mut joined, user, config);
            // When drilling a specific hierarchy level, swap the dimension
            // column to the level's column (e.g. "year" instead of "full_date").
            // Non-unique levels get their full ancestor path so compound-key
            // members (Q1-2020 vs Q1-2021) stay distinct; a drilldown-child
            // query filters on the parent and lets the renderer prefix the
            // parent key, so it keeps the plain level column.
            let mut path_exprs: std::collections::HashMap<usize, String> =
                std::collections::HashMap::new();
            let mut path_order: std::collections::HashMap<usize, Vec<String>> =
                std::collections::HashMap::new();
            for (i, dim_id) in group_by.iter().enumerate() {
                let Some(level_idx) = group_levels.get(i).copied().flatten() else {
                    continue;
                };
                let Some(dim) = model.dim_def_opt(dim_id) else {
                    continue;
                };
                let Some(level) = dim.levels.get(level_idx) else {
                    continue;
                };
                let alias_prefix = col_map
                    .get(dim_id)
                    .and_then(|v| v.rsplit_once('.').map(|(p, _)| p))
                    .unwrap_or("");
                let qual = |col: &str| -> String {
                    if alias_prefix.is_empty() {
                        col.to_string()
                    } else {
                        format!("{alias_prefix}.{col}")
                    }
                };
                let single_parent = filters
                    .iter()
                    .find(|f| f.dimension == *dim_id && f.members.len() == 1);
                // A single parent filter can prefix the parent key only when we
                // drill exactly one level below it. Deeper drills carry the full
                // ancestor path so intermediate levels can be derived.
                let single_parent_level = single_parent
                    .and_then(|f| f.level.as_ref())
                    .and_then(|name| dim.levels.iter().position(|l| l.name == *name));
                let is_leaf_level = level_idx + 1 == dim.levels.len();
                let plain_column = level_idx == 0
                    || is_leaf_level
                    || single_parent.is_some()
                        && (single_parent_level.is_none()
                            || single_parent_level.is_some_and(|l| level_idx == l + 1));
                if plain_column {
                    col_map.insert(dim_id.clone(), qual(&level.column));
                } else {
                    // Compound ancestor path: "2020|1" for Q1-2020.
                    let cols: Vec<String> = dim.levels[..=level_idx]
                        .iter()
                        .map(|l| format!("CAST({} AS VARCHAR)", qual(&l.column)))
                        .collect();
                    let path = if cols.len() == 1 {
                        cols[0].clone()
                    } else {
                        format!("CONCAT_WS('|', {})", cols.join(", "))
                    };
                    path_exprs.insert(i, path);
                    path_order.insert(
                        i,
                        dim.levels[..=level_idx]
                            .iter()
                            .map(|l| qual(&l.column))
                            .collect(),
                    );
                }
            }
            let col_names: Vec<String> = group_by
                .iter()
                .enumerate()
                .map(|(i, d)| match path_exprs.get(&i) {
                    Some(path) => path.clone(),
                    None => {
                        let col = col_map.get(d.as_str()).map(|s| s.as_str()).unwrap_or("??");
                        format!("CAST({col} AS VARCHAR)")
                    }
                })
                .collect();

            // Build a WHERE column map: for the drilldown dimension at a
            // deeper level, the filter uses the parent level's column (e.g.
            // WHERE year = '2023') not the target level's column (quarter).
            let mut where_col_map = col_map.clone();
            for (i, dim_id) in group_by.iter().enumerate() {
                let Some(level_idx) = group_levels.get(i).copied().flatten() else {
                    continue;
                };
                if level_idx == 0 {
                    continue;
                }
                let Some(dim) = model.dim_def_opt(dim_id) else {
                    continue;
                };
                let Some(parent_level) = dim.levels.get(level_idx - 1) else {
                    continue;
                };
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
            // Path-grouped dimensions sort by their underlying level columns
            // (so months order 1..12, not "1","10","11"...). Those columns are
            // also added to GROUP BY so the ORDER BY binds.
            let mut group_terms: Vec<String> = Vec::new();
            let mut order_terms: Vec<String> = Vec::new();
            for i in 0..group_by.len() {
                match (path_exprs.get(&i), path_order.get(&i)) {
                    (Some(path), Some(cols)) => {
                        group_terms.push(path.clone());
                        group_terms.extend(cols.iter().cloned());
                        order_terms.extend(cols.iter().cloned());
                    }
                    _ => {
                        group_terms.push((i + 1).to_string());
                        order_terms.push((i + 1).to_string());
                    }
                }
            }
            format!(
                "SELECT {}, {} FROM {} f{}{} GROUP BY {} ORDER BY {}",
                col_names.join(", "),
                meas.sql_expr,
                table,
                joins,
                wc,
                group_terms.join(", "),
                order_terms.join(", "),
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

        QueryPlan::MetaCount { dim, group_level } => {
            let d = model.dim_def(dim);
            let table = model.dim_table_for_discovery(dim);
            // A level member is identified by its FULL ancestor path (Q1 of
            // 2020 differs from Q1 of 2021), so count distinct paths.
            match group_level.and_then(|i| d.levels.get(i)) {
                Some(_) => {
                    let depth = group_level.unwrap_or(0);
                    let cols: Vec<String> = d.levels[..=depth]
                        .iter()
                        .map(|l| format!("CAST({} AS VARCHAR)", l.column))
                        .collect();
                    let path = if cols.len() == 1 {
                        cols[0].clone()
                    } else {
                        format!("CONCAT_WS('|', {})", cols.join(", "))
                    };
                    format!("SELECT COUNT(DISTINCT {path}) FROM {table}")
                }
                None => format!(
                    "SELECT COUNT(DISTINCT CAST({} AS VARCHAR)) FROM {table}",
                    d.physical_field
                ),
            }
        }

        QueryPlan::MultiMeasure { .. }
        | QueryPlan::MultiGroupBy { .. }
        | QueryPlan::TupleSet { .. } => String::new(),

        QueryPlan::Empty => String::new(),
    }
}

/// Return the role-filter SQL predicate for a table, or empty string when the
/// user has unfettered access (`Full` or `Hidden` — gating handled in plan.rs).
///
/// Rollups are built from the full fact, so they cannot be used when any role
/// filter is active — that would bypass RLS.
fn aggregation_safe(model: &SemanticModel, user: &UserContext, config: &ProxyConfig) -> bool {
    for ft in &model.fact_tables {
        if matches!(
            effective_table_filter(config, user, &ft.table_name),
            TableAccess::Filtered(_)
        ) {
            return false;
        }
    }
    for rel in &model.relationships {
        if matches!(
            effective_table_filter(config, user, &rel.dim_table),
            TableAccess::Filtered(_)
        ) {
            return false;
        }
    }
    true
}

/// The single aggregation routing decision: only route to a rollup when no role
/// filter is active (rollups are built from the full fact and would bypass RLS)
/// AND a rollup can answer the plan. Tested directly by the RLS test.
fn route_plan<'a>(
    model: &SemanticModel,
    aggs: &'a [Aggregation],
    plan: &QueryPlan,
    user: &UserContext,
    config: &ProxyConfig,
) -> Option<&'a Aggregation> {
    if !aggregation_safe(model, user, config) {
        return None;
    }
    aggregate::agg_for_plan_with(model, aggs, plan)
}

/// WHERE clause for a rollup query: filters map to the rollup's direct columns.
fn agg_where(model: &SemanticModel, agg: &Aggregation, filters: &[TypedDimensionFilter]) -> String {
    let mut parts = Vec::new();
    for f in filters {
        let col = if f.dimension == agg.date_dim_id {
            let lvl = aggregate::filter_level(model, &agg.date_dim_id, f);
            agg.date_columns.get(lvl).cloned()
        } else {
            agg.leaf_columns.get(&f.dimension).cloned()
        };
        if let Some(col) = col {
            let vals: Vec<String> = f
                .members
                .iter()
                .map(|m| format!("'{}'", m.replace('\'', "''")))
                .collect();
            if !vals.is_empty() {
                parts.push(format!("CAST({col} AS VARCHAR) IN ({})", vals.join(", ")));
            }
        }
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", parts.join(" AND "))
    }
}

fn agg_total_sql(
    model: &SemanticModel,
    agg: &Aggregation,
    measure: &str,
    filters: &[TypedDimensionFilter],
) -> String {
    let meas = model.meas_def(measure);
    let wc = agg_where(model, agg, filters);
    format!(
        "SELECT {} FROM {}.{}{}",
        meas.sql_expr, AGG_ALIAS, agg.table, wc
    )
}

fn agg_groupby_sql(
    model: &SemanticModel,
    agg: &Aggregation,
    measure: &str,
    group_by: &[String],
    group_level: &Option<usize>,
    filters: &[TypedDimensionFilter],
) -> String {
    let meas = model.meas_def(measure);
    let mut cols = Vec::new();
    for dim in group_by {
        let col = if *dim == agg.date_dim_id {
            agg.date_columns.get(group_level.unwrap_or(0)).cloned()
        } else {
            agg.leaf_columns.get(dim).cloned()
        };
        cols.push(format!(
            "CAST({} AS VARCHAR)",
            col.unwrap_or_else(|| "1".into())
        ));
    }
    let wc = agg_where(model, agg, filters);
    let nums: Vec<String> = (1..=cols.len()).map(|i| i.to_string()).collect();
    format!(
        "SELECT {}, {} FROM {}.{} {} GROUP BY {} ORDER BY {}",
        cols.join(", "),
        meas.sql_expr,
        AGG_ALIAS,
        agg.table,
        wc,
        nums.join(", "),
        nums.join(", "),
    )
}

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
        // Level-qualified filter (e.g. [Date].[Date].[Year].&[2024], or a
        // compound [Date].[Date].[Quarter].&[2026]&[4]): filter the hierarchy
        // level's column via a subquery on the relationship's dim table. A
        // compound key carries ancestor values, so every ancestor predicate is
        // applied (year=2026 AND quarter=4) to scope correctly.
        if let Some(level_name) = &f.level
            && let (Some(d), Some(rel)) = (
                model.dim_def_opt(&f.dimension),
                model.rel_for_dimension(&f.dimension),
            )
            && let Some(level_idx) = d.levels.iter().position(|l| &l.name == level_name)
        {
            let mut ors: Vec<String> = Vec::new();
            for key in &f.members {
                let parts: Vec<&str> = key.split('|').collect();
                // Align the key path to the level chain. A key with fewer parts
                // than the filter's level is anchored at that level (a single
                // [Month].&[6] scopes month=6); a key with more parts is itself
                // a deeper member and root-anchored ([Quarter].&[2026]&[4]
                // scopes year=2026 AND quarter=4). Excel mixes both in one
                // DrilldownMember set, so never assume a single length.
                let wanted = level_idx + 1;
                let base = if parts.len() > wanted {
                    0
                } else {
                    wanted - parts.len()
                };
                let mut ands: Vec<String> = Vec::new();
                for (j, v) in parts.iter().enumerate() {
                    if let Some(l) = d.levels.get(base + j) {
                        ands.push(format!(
                            "CAST({} AS VARCHAR) = '{}'",
                            l.column,
                            v.replace('\'', "''")
                        ));
                    }
                }
                if !ands.is_empty() {
                    ors.push(format!("({})", ands.join(" AND ")));
                }
            }
            parts.push(format!(
                "f.{} IN (SELECT {} FROM {} WHERE {})",
                rel.fact_column,
                rel.dim_column,
                rel.dim_table,
                ors.join(" OR ")
            ));
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
            parts.push(format!("CAST({col} AS VARCHAR) IN ({})", vals.join(", ")));
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
        assert_eq!(sql, "SELECT SUM(sales) FROM fact_table f");
    }

    #[test]
    fn sql_total_with_filter() {
        let plan = QueryPlan::Total {
            measure: "TotalSales".into(),
            filters: vec![TypedDimensionFilter {
                dimension: "Region".into(),
                level: None,
                time_flag: None,
                members: vec!["North".into()],
            }],
        };
        let sql = sql_for_query_plan(&default_model(), &plan);
        assert!(sql.contains("SELECT SUM(sales) FROM fact_table f"));
        assert!(sql.contains("WHERE CAST(region AS VARCHAR) IN ('North')"));
    }

    #[test]
    fn sql_group_by_one_dim() {
        let plan = QueryPlan::GroupBy {
            measure: "TotalSales".into(),
            group_by: vec!["ProductCategory".into()],
            group_levels: vec![],
            set_op: None,
            filters: vec![],
        };
        let sql = sql_for_query_plan(&default_model(), &plan);
        assert_eq!(
            sql,
            "SELECT CAST(product_category AS VARCHAR), SUM(sales) FROM fact_table f GROUP BY 1 ORDER BY 1"
        );
    }

    #[test]
    fn sql_group_by_two_dims() {
        let plan = QueryPlan::GroupBy {
            measure: "TotalSales".into(),
            group_by: vec!["ProductCategory".into(), "Region".into()],
            group_levels: vec![],
            set_op: None,
            filters: vec![],
        };
        let sql = sql_for_query_plan(&default_model(), &plan);
        assert_eq!(
            sql,
            "SELECT CAST(product_category AS VARCHAR), CAST(region AS VARCHAR), SUM(sales) FROM fact_table f GROUP BY 1, 2 ORDER BY 1, 2"
        );
    }

    #[test]
    fn sql_group_by_with_filter() {
        let plan = QueryPlan::GroupBy {
            measure: "TotalSales".into(),
            group_by: vec!["ProductCategory".into()],
            group_levels: vec![],
            set_op: None,
            filters: vec![TypedDimensionFilter {
                dimension: "Region".into(),
                level: None,
                time_flag: None,
                members: vec!["North".into()],
            }],
        };
        let sql = sql_for_query_plan(&default_model(), &plan);
        assert!(sql.contains("WHERE CAST(region AS VARCHAR) IN ('North')"));
        assert!(sql.contains("GROUP BY 1"));
    }

    #[test]
    fn sql_count() {
        let plan = QueryPlan::Count {
            dimension: "ProductCategory".into(),
        };
        let sql = sql_for_query_plan(&default_model(), &plan);
        assert_eq!(
            sql,
            "SELECT COUNT(DISTINCT product_category) FROM fact_table"
        );
    }

    #[test]
    fn sql_total_multi_filter_both_dims() {
        let plan = QueryPlan::Total {
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
                    members: vec!["Category A".into(), "Category B".into()],
                },
            ],
        };
        let sql = sql_for_query_plan(&default_model(), &plan);
        assert!(sql.contains("WHERE CAST(region AS VARCHAR) IN ('North')"));
        assert!(
            sql.contains("AND CAST(product_category AS VARCHAR) IN ('Category A', 'Category B')")
        );
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
            dim_cache: Default::default(),
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
                    level: None,
                    time_flag: None,
                    members: vec!["Widget".into()],
                }],
            },
        );
        assert!(sql.contains("FROM fact_table f"));
        assert!(sql.contains("JOIN dim_product _product ON f.product_id = _product.product_id"));
        assert!(sql.contains("WHERE CAST(_product.product_name AS VARCHAR) IN ('Widget')"));
    }

    #[test]
    fn group_by_with_relationship_join() {
        let m = star_model();
        let sql = sql_for_query_plan(
            &m,
            &QueryPlan::GroupBy {
                measure: "Revenue".into(),
                group_by: vec!["Product".into()],
                group_levels: vec![],
                set_op: None,
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
            dim_cache: Default::default(),
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
                group_levels: vec![],
                set_op: None,
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
            dim_cache: Default::default(),
        }
    }

    #[test]
    fn sql_group_level_0() {
        let m = date_level_model();
        let plan = QueryPlan::GroupBy {
            measure: "Revenue".into(),
            group_by: vec!["Date".into()],
            filters: vec![],
            group_levels: vec![Some(0)],
            set_op: None,
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
                level: None,
                time_flag: None,
            }],
            group_levels: vec![Some(1)],
            set_op: None,
        };
        let sql = sql_for_query_plan(&m, &plan);
        assert!(
            sql.contains("CAST(_date.quarter AS VARCHAR)"),
            "should group by quarter: {sql}"
        );
        assert!(
            sql.contains("CAST(_date.year AS VARCHAR) IN ('2024')"),
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
            group_levels: vec![],
            set_op: None,
        };
        let sql = sql_for_query_plan(&m, &plan);
        assert!(
            sql.contains("CAST(_date.full_date AS VARCHAR)"),
            "should group by physical_field: {sql}"
        );
    }

    // Security: a role-filtered user must never be routed to a rollup (rollups
    // are built from the full fact and would bypass RLS). Admin users may route.
    #[test]
    fn rls_blocks_aggregation_routing() {
        let model =
            crate::project::project::ProxyProject::load("projects/project3/proxy-config.json")
                .expect("load project3")
                .model;
        let aggs = crate::engine::aggregate::design_aggregations(&model);
        assert!(!aggs.is_empty(), "project3 should design rollups");

        let cfg: ProxyConfig = serde_json::from_str(
            r#"{
                "catalog": "SALES_ANALYTICS", "cube": "Sales",
                "source_name": "sales_fact", "table_name": "sales_fact",
                "dialect": "duckdb", "dimensions": [], "measures": [],
                "auth": { "trusted_proxy": true },
                "roles": [{
                    "name": "TerritoryNorth",
                    "model_permission": "read",
                    "members": [{"member_name": "user1", "member_type": "user"}],
                    "table_permissions": [{
                        "table": "sales_fact",
                        "filter_expression": "territory = 'North'"
                    }]
                }]
            }"#,
        )
        .expect("parse config");

        let plan = QueryPlan::Total {
            measure: "Revenue".into(),
            filters: vec![],
        };

        let admin = UserContext::admin_default();
        assert!(
            route_plan(&model, &aggs, &plan, &admin, &cfg).is_some(),
            "admin should route to a rollup"
        );

        let filtered = crate::engine::model::resolve_user_context(&cfg, "user1", &[]);
        assert!(!filtered.is_administrator);
        assert!(
            route_plan(&model, &aggs, &plan, &filtered, &cfg).is_none(),
            "role-filtered user must never route to a rollup"
        );

        // End-to-end: the generated SQL for the filtered user has no rollup table.
        let sql = sql_for_query_plan_with_context(&model, &plan, &filtered, &cfg);
        assert!(!sql.contains("agg."), "RLS SQL must not use rollups: {sql}");
        assert!(
            sql.contains("FROM sales_fact"),
            "RLS SQL should scan the fact: {sql}"
        );
    }
}
