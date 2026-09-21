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
            filters,
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
            let (filter_joins, wc) =
                joins_and_where(model, filters, &mut joined, user, config, table);
            format!(
                "SELECT {path} AS __path, {} FROM {table} f{joins}{filter_joins}{wc} GROUP BY 1 ORDER BY 1",
                meas.sql_expr
            )
        }
        QueryPlan::Total { measure, filters } => {
            if let Some((agg, role_predicates)) =
                route_plan(model, &aggregate::aggregations(), plan, user, config)
            {
                return agg_total_sql(model, agg, measure, filters, &role_predicates);
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
                && let Some((agg, role_predicates)) =
                    route_plan(model, &aggregate::aggregations(), plan, user, config)
            {
                return agg_groupby_sql(
                    model,
                    agg,
                    measure,
                    group_by,
                    &group_levels.first().copied().flatten(),
                    filters,
                    &role_predicates,
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

/// Rewrite a role-filter SQL fragment so it can be evaluated against a rollup.
///
/// Rollups carry the date dimension's level columns and the flat dimensions'
/// values, so a predicate is expressible when every column reference maps to a
/// rollup column holding the same values. Anything else — fact columns that
/// were not rolled up, other aliases/tables, quoted identifiers, subqueries —
/// returns `None`, and the caller keeps the fact path. Correctness over speed:
/// a rollup must never bypass row-level security.
fn rewrite_predicate_for_rollup(
    model: &SemanticModel,
    agg: &Aggregation,
    predicate: &str,
) -> Option<String> {
    use std::collections::HashSet;

    let rollup_columns: HashSet<&str> = agg
        .date_columns
        .iter()
        .map(|c| c.as_str())
        .chain(agg.leaf_columns.values().map(|c| c.as_str()))
        .collect();
    let date_dim = model.dim_def(&agg.date_dim_id);
    // `_<dim>.<col>` -> rollup column, when the rollup carries that value.
    let dim_column = |dim_id: &str, col: &str| -> Option<String> {
        if dim_id.eq_ignore_ascii_case(&agg.date_dim_id) {
            let idx = date_dim.levels.iter().position(|l| l.column == col)?;
            if idx > agg.date_level {
                return None;
            }
            return agg.date_columns.get(idx).cloned();
        }
        let leaf_id = agg
            .leaf_columns
            .keys()
            .find(|id| id.eq_ignore_ascii_case(dim_id))?;
        let dim = model.dim_def_opt(leaf_id)?;
        (dim.physical_field == col).then(|| col.to_string())
    };

    let bytes = predicate.as_bytes();
    let mut out = String::with_capacity(predicate.len());
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '\'' {
            // Copy string literals verbatim (with '' escapes).
            let start = i;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\'' {
                    if bytes.get(i + 1) == Some(&b'\'') {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push_str(&predicate[start..i]);
            continue;
        }
        if c == '"' {
            return None; // quoted identifiers: cannot reason about them
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let ident = &predicate[start..i];
            let lower = ident.to_ascii_lowercase();
            if matches!(
                lower.as_str(),
                "select" | "from" | "join" | "union" | "exists" | "with"
            ) {
                return None; // subqueries are not rollup-expressible
            }
            let mut j = i;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'.' {
                // alias.column
                j += 1;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                let col_start = j;
                while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                    j += 1;
                }
                if col_start == j {
                    return None;
                }
                let col = &predicate[col_start..j];
                let mapped = if lower == "f" {
                    rollup_columns.contains(col).then(|| col.to_string())
                } else {
                    dim_column(lower.strip_prefix('_')?, col)
                };
                out.push_str(&mapped?);
                i = j;
                continue;
            }
            let is_keyword = matches!(
                lower.as_str(),
                "and"
                    | "or"
                    | "not"
                    | "in"
                    | "is"
                    | "null"
                    | "true"
                    | "false"
                    | "like"
                    | "ilike"
                    | "between"
                    | "cast"
                    | "as"
                    | "case"
                    | "when"
                    | "then"
                    | "else"
                    | "end"
                    | "distinct"
                    | "all"
                    | "any"
                    | "some"
            );
            if is_keyword || (j < bytes.len() && bytes[j] == b'(') {
                // Function names and SQL keywords pass through; their column
                // arguments are still scanned above.
                out.push_str(ident);
                continue;
            }
            // A bare column reference must be one the rollup carries.
            if !rollup_columns.contains(ident) {
                return None;
            }
            out.push_str(ident);
            continue;
        }
        out.push(c);
        i += 1;
    }
    Some(out)
}

/// Role predicates rewritten for a rollup, or `None` when any of them cannot be
/// evaluated there (the caller then keeps the fact path).
fn rollup_role_predicates(
    model: &SemanticModel,
    user: &UserContext,
    config: &ProxyConfig,
    agg: &Aggregation,
) -> Option<Vec<String>> {
    let mut predicates = Vec::new();
    for ft in &model.fact_tables {
        if let TableAccess::Filtered(sql) = effective_table_filter(config, user, &ft.table_name) {
            predicates.push(rewrite_predicate_for_rollup(model, agg, &sql)?);
        }
    }
    for rel in &model.relationships {
        if let TableAccess::Filtered(sql) = effective_table_filter(config, user, &rel.dim_table) {
            predicates.push(rewrite_predicate_for_rollup(model, agg, &sql)?);
        }
    }
    Some(predicates)
}

/// The single aggregation routing decision: a rollup may answer the plan AND
/// every active role filter is rollup-expressible. The rewritten role
/// predicates are returned so the rollup query applies them (rollups are built
/// from the full fact — they must never bypass RLS). Otherwise the fact path.
fn route_plan<'a>(
    model: &SemanticModel,
    aggs: &'a [Aggregation],
    plan: &QueryPlan,
    user: &UserContext,
    config: &ProxyConfig,
) -> Option<(&'a Aggregation, Vec<String>)> {
    let agg = aggregate::agg_for_plan_with(model, aggs, plan)?;
    let predicates = rollup_role_predicates(model, user, config, agg)?;
    Some((agg, predicates))
}

/// WHERE clause for a rollup query: filters map to the rollup's direct columns.
fn agg_where(
    model: &SemanticModel,
    agg: &Aggregation,
    filters: &[TypedDimensionFilter],
    role_predicates: &[String],
) -> String {
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
    parts.extend(role_predicates.iter().cloned());
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
    role_predicates: &[String],
) -> String {
    let meas = model.meas_def(measure);
    let wc = agg_where(model, agg, filters, role_predicates);
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
    role_predicates: &[String],
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
    let wc = agg_where(model, agg, filters, role_predicates);
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
        dimensions_file: None,
        measures_file: None,
        relationships_file: None,
        roles_file: None,
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
                // The fact-side column comes from the relationship: a fact's
                // date column need not be named like the dimension's key
                // (`order_date_key` vs `date_key`). Fall back to the configured
                // key column for models without a relationship.
                let fact_id = model
                    .fact_tables
                    .iter()
                    .find(|ft| ft.table_name == fact_table_name)
                    .map(|ft| ft.id.as_str());
                let fact_col = fact_id
                    .and_then(|id| {
                        model
                            .relationships
                            .iter()
                            .find(|r| r.dimension_id == f.dimension && r.fact_table_id == id)
                    })
                    .or_else(|| model.rel_for_dimension(&f.dimension))
                    .map(|r| r.fact_column.as_str())
                    .unwrap_or(dd.date_key_column.as_str());
                parts.push(format!(
                    "f.{fact_col} IN (SELECT {} FROM {} WHERE {} = true)",
                    dd.date_key_column, dd.table_name, flag
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
            // Date windows (`YTD(m)`, member-value filters, `ParallelPeriod`,
            // `LastPeriods`): the anchor member's date bounds the window on the
            // role's full-date column.
            if let Some(w) = &f.date_window {
                use crate::mdx::ast::DateWindow;
                let date_col = model
                    .date_dims
                    .get(&f.dimension)
                    .map(|dd| dd.full_date_column.clone())
                    .unwrap_or_else(|| {
                        d.levels
                            .last()
                            .map(|l| l.column.clone())
                            .unwrap_or_else(|| d.physical_field.clone())
                    });
                let anchor_pins = |anchor: &[(String, String)]| -> String {
                    let mut pins: Vec<String> = Vec::new();
                    for (level_name, value) in anchor {
                        if let Some(l) = d.levels.iter().find(|l| &l.name == level_name) {
                            pins.push(format!(
                                "CAST({} AS VARCHAR) = '{}'",
                                l.column,
                                value.replace('\'', "''")
                            ));
                        }
                    }
                    if pins.is_empty() {
                        "TRUE".to_string()
                    } else {
                        pins.join(" AND ")
                    }
                };
                let anchor = |anchor: &[(String, String)]| {
                    format!(
                        "(SELECT MAX({date_col}) FROM {} WHERE {})",
                        rel.dim_table,
                        anchor_pins(anchor)
                    )
                };
                let window = match w {
                    DateWindow::Relative { op, amount, unit } => {
                        let cmp = match op {
                            crate::mdx::ast::CmpOp::Gt => ">",
                            crate::mdx::ast::CmpOp::Ge => ">=",
                            crate::mdx::ast::CmpOp::Lt => "<",
                            crate::mdx::ast::CmpOp::Le => "<=",
                            crate::mdx::ast::CmpOp::Eq => "=",
                            crate::mdx::ast::CmpOp::Ne => "<>",
                        };
                        format!("{date_col} {cmp} (CURRENT_DATE + INTERVAL '{amount} {unit}')")
                    }
                    DateWindow::ToDate {
                        anchor: pins,
                        period,
                    } => {
                        let a = anchor(pins);
                        format!("{date_col} BETWEEN date_trunc('{period}', {a}) AND {a}")
                    }
                    DateWindow::Parallel {
                        anchor: pins,
                        level,
                        offset,
                    } => {
                        let a = anchor(pins);
                        let next = offset + 1;
                        format!(
                            "{date_col} >= date_trunc('{level}', {a}) + INTERVAL '{offset} {level}' AND {date_col} < date_trunc('{level}', {a}) + INTERVAL '{next} {level}'"
                        )
                    }
                    DateWindow::LastPeriods {
                        anchor: pins,
                        level,
                        count,
                    } => {
                        let a = anchor(pins);
                        let back = (count - 1).max(0);
                        format!(
                            "{date_col} >= date_trunc('{level}', {a}) - INTERVAL '{back} {level}' AND {date_col} <= {a}"
                        )
                    }
                };
                parts.push(format!(
                    "f.{} IN (SELECT {} FROM {} WHERE {window})",
                    rel.fact_column, rel.dim_column, rel.dim_table
                ));
                continue;
            }
            // Inclusive member range (`{[D].[H].[L].&[a] : [D].[H].[L].&[b]}`):
            // ancestors are equal, the level column is compared directly. The
            // string literals cast to the column's type, so dates, numbers and
            // strings all order correctly.
            if let Some((from, to)) = &f.range {
                let mut preds: Vec<String> = Vec::new();
                let from_parts: Vec<&str> = from.split('|').collect();
                for (j, l) in d.levels.iter().enumerate().take(level_idx) {
                    if let Some(v) = from_parts.get(j) {
                        preds.push(format!(
                            "CAST({} AS VARCHAR) = '{}'",
                            l.column,
                            v.replace('\'', "''")
                        ));
                    }
                }
                if let Some(l) = d.levels.get(level_idx) {
                    let last = |k: &str| k.rsplit('|').next().unwrap_or(k).to_string();
                    preds.push(format!(
                        "{} BETWEEN '{}' AND '{}'",
                        l.column,
                        last(from).replace('\'', "''"),
                        last(to).replace('\'', "''")
                    ));
                }
                if !preds.is_empty() {
                    parts.push(format!(
                        "f.{} IN (SELECT {} FROM {} WHERE {})",
                        rel.fact_column,
                        rel.dim_column,
                        rel.dim_table,
                        preds.join(" AND ")
                    ));
                    continue;
                }
            }
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
                range: None,
                date_window: None,
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
                range: None,
                date_window: None,
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
                    range: None,
                    date_window: None,
                },
                TypedDimensionFilter {
                    dimension: "ProductCategory".into(),
                    level: None,
                    time_flag: None,
                    members: vec!["Category A".into(), "Category B".into()],
                    range: None,
                    date_window: None,
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
                    range: None,
                    date_window: None,
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
                range: None,
                date_window: None,
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

    // Security: rollups are built from the full fact, so RLS may only route
    // when the role predicate is expressible on the rollup (same values) — and
    // then the predicate is applied to the rollup query.
    #[test]
    fn rls_routing_requires_expressible_predicates() {
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
        let (_, admin_preds) =
            route_plan(&model, &aggs, &plan, &admin, &cfg).expect("admin should route to a rollup");
        assert!(admin_preds.is_empty(), "admin has no role predicates");

        // A predicate the rollup can evaluate (the Territory value is a leaf
        // column) routes, and the predicate is applied to the rollup.
        let filtered = crate::engine::model::resolve_user_context(&cfg, "user1", &[]);
        assert!(!filtered.is_administrator);
        let (agg, preds) = route_plan(&model, &aggs, &plan, &filtered, &cfg)
            .expect("rollup-expressible RLS may route");
        assert_eq!(preds, vec!["territory = 'North'".to_string()]);
        assert!(agg.leaf_columns.values().any(|c| c == "territory"));

        let sql = agg_total_sql(&model, agg, "Revenue", &[], &preds);
        assert!(
            sql.contains("agg.agg_year"),
            "expressible RLS should build a rollup query: {sql}"
        );
        assert!(
            sql.contains("territory = 'North'"),
            "the role predicate must be applied to the rollup: {sql}"
        );

        // A predicate on a fact column the rollup does not carry must fall
        // back to the fact table — correctness over speed.
        let cfg_unexpressible: ProxyConfig = serde_json::from_str(
            r#"{
                "catalog": "SALES_ANALYTICS", "cube": "Sales",
                "source_name": "sales_fact", "table_name": "sales_fact",
                "dialect": "duckdb", "dimensions": [], "measures": [],
                "auth": { "trusted_proxy": true },
                "roles": [{
                    "name": "BigRevenue",
                    "model_permission": "read",
                    "members": [{"member_name": "user1", "member_type": "user"}],
                    "table_permissions": [{
                        "table": "sales_fact",
                        "filter_expression": "revenue > 1000"
                    }]
                }]
            }"#,
        )
        .expect("parse config");
        let big = crate::engine::model::resolve_user_context(&cfg_unexpressible, "user1", &[]);
        assert!(
            route_plan(&model, &aggs, &plan, &big, &cfg_unexpressible).is_none(),
            "an unexpressible predicate must keep the fact path"
        );
        // (The end-to-end SQL path routes through the process-wide rollup set,
        // which tests keep disabled; the routing decision above is the seam.)
        let sql = sql_for_query_plan_with_context(&model, &plan, &big, &cfg_unexpressible);
        assert!(!sql.contains("agg."), "RLS SQL must not use rollups: {sql}");
        assert!(
            sql.contains("FROM sales_fact"),
            "RLS SQL should scan the fact: {sql}"
        );
    }

    // A fact's date column need not be named like the dimension key: the
    // time-flag filter must use the relationship's fact column (a model whose
    // fact uses `order_date_key` returned 0 for YTD before this).
    #[test]
    fn time_flag_filter_uses_the_relationship_fact_column() {
        let p = crate::project::project::ProxyProject::load(
            "projects/upstream_marts/proxy-config.yaml",
        )
        .expect("load upstream_marts demo");
        let model = &p.model;
        let plan = QueryPlan::Total {
            measure: "Revenue YTD".into(),
            filters: vec![TypedDimensionFilter {
                dimension: "Date".into(),
                members: vec![],
                level: None,
                time_flag: Some("ytd_flag".into()),
                range: None,
                date_window: None,
            }],
        };
        let sql = sql_for_query_plan(model, &plan);
        assert!(
            sql.contains(
                "f.order_date_key IN (SELECT date_key FROM dim_date WHERE ytd_flag = true)"
            ),
            "flag filter must use the relationship's fact column: {sql}"
        );
        assert!(
            !sql.contains("f.date_key"),
            "must not assume the dimension key name on the fact: {sql}"
        );
    }

    // The rewrite is the security boundary: it may only map references the
    // rollup can evaluate with identical semantics.
    #[test]
    fn rollup_predicate_rewrite_is_conservative() {
        let model =
            crate::project::project::ProxyProject::load("projects/project3/proxy-config.json")
                .expect("load project3")
                .model;
        let aggs = crate::engine::aggregate::design_aggregations(&model);
        let year = aggs
            .iter()
            .find(|a| a.table == "agg_year")
            .expect("agg_year");

        // Expressible: flat-dim values (bare or via f.), date level columns,
        // functions over those columns, and multi-condition predicates.
        assert_eq!(
            rewrite_predicate_for_rollup(&model, year, "territory = 'North'").as_deref(),
            Some("territory = 'North'")
        );
        assert_eq!(
            rewrite_predicate_for_rollup(&model, year, "f.territory = 'North'").as_deref(),
            Some("territory = 'North'")
        );
        assert_eq!(
            rewrite_predicate_for_rollup(
                &model,
                year,
                "territory = 'North' AND channel = 'Online'"
            )
            .as_deref(),
            Some("territory = 'North' AND channel = 'Online'")
        );
        assert_eq!(
            rewrite_predicate_for_rollup(&model, year, "RIGHT(territory, 3) = 'rth'").as_deref(),
            Some("RIGHT(territory, 3) = 'rth'")
        );
        assert_eq!(
            rewrite_predicate_for_rollup(&model, year, "_date.year = 2024").as_deref(),
            Some("year = 2024")
        );

        // Not expressible: measures, columns the rollup lacks, deeper date
        // levels than this rollup stores, other aliases, quoted identifiers,
        // subqueries.
        for predicate in [
            "revenue > 1000",
            "f.revenue > 1000",
            "f.region = 'EU'",
            "_date.month = 1",
            "_territory.region = 'EU'",
            "x.territory = 'North'",
            "\"Territory\" = 'North'",
            "EXISTS (SELECT 1 FROM other)",
        ] {
            assert_eq!(
                rewrite_predicate_for_rollup(&model, year, predicate),
                None,
                "must refuse to rewrite: {predicate}"
            );
        }
    }
}
