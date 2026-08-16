//! Pre-computed rollups ("aggregations") so coarse-grain queries avoid scanning
//! the fact table.
//!
//! SSAS/Power BI answer a year drilldown from a rollup instead of the fact.
//! This module (a) designs rollups from the model's multi-level dimensions,
//! (b) builds them into a proxy-owned sidecar DuckDB file (never the user's
//! database), and (c) matches a query plan to the coarsest rollup that can
//! answer it.
//!
//! Enabled by `MALLARDCUBE_AGG_CACHE=<path>`; disabled otherwise (queries hit
//! the fact, as before).

use crate::engine::model::SemanticModel;
use crate::engine::plan::{QueryPlan, TypedDimensionFilter};
use duckdb::Connection;
use std::collections::HashMap;
use std::sync::OnceLock;

/// The sidecar database is attached to pooled connections under this alias.
pub const AGG_ALIAS: &str = "agg";

/// One rollup table. Stored measure columns keep the fact's base column name
/// (e.g. `SUM(revenue) AS revenue`) so the measure's `sql_expr` (`SUM(revenue)`)
/// runs unchanged against the rollup.
#[derive(Debug, Clone)]
pub struct Aggregation {
    pub table: String,
    pub date_dim_id: String,
    /// Index into the date dim's `levels` this rollup aggregates to (0 = coarsest).
    pub date_level: usize,
    /// Ancestor level columns present in the rollup (["year"], ["year","quarter"], …).
    pub date_columns: Vec<String>,
    /// Non-date (degenerate) dims -> their rollup column.
    pub leaf_columns: HashMap<String, String>,
    /// measure id -> base column name (additive SUM measures only).
    pub measure_columns: HashMap<String, String>,
}

/// Process-wide rollup set, populated after a successful sidecar build.
/// Empty = aggregation routing disabled.
static AGGREGATIONS: OnceLock<Vec<Aggregation>> = OnceLock::new();

pub fn enable(aggs: Vec<Aggregation>) {
    // Routing is only active when a sidecar is configured (MALLARDCUBE_AGG_CACHE).
    // Tests build rollups without that env var and must not flip global routing.
    if cache_path().is_some() {
        let _ = AGGREGATIONS.set(aggs);
    }
}

pub fn aggregations() -> &'static [Aggregation] {
    AGGREGATIONS.get().map(|v| v.as_slice()).unwrap_or(&[])
}

pub fn cache_path() -> Option<String> {
    std::env::var("MALLARDCUBE_AGG_CACHE").ok()
}

/// Extract the base column from an additive measure expression, e.g.
/// `SUM(revenue)` -> `revenue`. Only plain `SUM(col)` is rollup-safe; anything
/// else (COUNT/MIN/MAX/AVG, expressions) returns `None` and falls back to the
/// fact table.
fn measure_base_column(sql_expr: &str) -> Option<String> {
    let s = sql_expr.trim();
    let rest = s.strip_prefix("SUM(")?;
    let inner = rest.strip_suffix(')')?.trim();
    if inner.is_empty()
        || !inner
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
    {
        return None;
    }
    Some(inner.rsplit('.').next().unwrap_or(inner).to_string())
}

/// Design one rollup per (non-leaf) level of each multi-level, relationship-backed
/// dimension, grouped by the level's ancestors + every degenerate leaf dimension.
pub fn design_aggregations(model: &SemanticModel) -> Vec<Aggregation> {
    let mut aggs = Vec::new();
    for dim in &model.dimensions {
        if dim.levels.len() < 2 {
            continue;
        }
        // Only roll up dimensions whose levels live in a relationship dim table.
        if model.rel_for_dimension(&dim.id).is_none() {
            continue;
        }
        for level_idx in 0..dim.levels.len().saturating_sub(1) {
            let date_columns: Vec<String> = dim.levels[..=level_idx]
                .iter()
                .map(|l| l.column.clone())
                .collect();
            let mut leaf_columns = HashMap::new();
            for d in &model.dimensions {
                if d.id == dim.id || model.rel_for_dimension(&d.id).is_some() {
                    continue;
                }
                leaf_columns.insert(d.id.clone(), d.physical_field.clone());
            }
            let mut measure_columns = HashMap::new();
            for m in &model.measures {
                if m.time_flag.is_some() {
                    continue;
                }
                if let Some(base) = measure_base_column(&m.sql_expr) {
                    measure_columns.insert(m.id.clone(), base);
                }
            }
            if measure_columns.is_empty() {
                continue;
            }
            aggs.push(Aggregation {
                table: format!("agg_{}", dim.levels[level_idx].name.to_lowercase()),
                date_dim_id: dim.id.clone(),
                date_level: level_idx,
                date_columns,
                leaf_columns,
                measure_columns,
            });
        }
    }
    // Coarsest first.
    aggs.sort_by_key(|a| a.date_level);
    aggs
}

/// Bump this when the rollup layout changes, so a sidecar built by an older
/// version is rebuilt rather than served stale.
const AGG_SCHEMA_VERSION: &str = "1";

/// Build the rollup tables into `agg_path` by scanning the user's `source_path`
/// once. Idempotent: a stamp (source size + mtime + schema version) in the
/// sidecar skips rebuilds. Rollups are validated against the fact before use;
/// any mismatch returns `Err` (callers should then serve without aggregations).
pub fn ensure_aggregations(
    source_path: &str,
    agg_path: &str,
    model: &SemanticModel,
) -> Result<(), duckdb::Error> {
    let aggs = design_aggregations(model);

    let stamp = source_stamp(source_path);
    let conn = Connection::open(agg_path)?;
    if sidecar_current(&conn, &stamp) && aggs.iter().all(|a| table_exists(&conn, &a.table)) {
        enable(aggs);
        return Ok(());
    }

    conn.execute_batch(&format!("ATTACH '{source_path}' AS src (READ_ONLY);"))?;
    for agg in &aggs {
        let sql = build_sql(model, agg);
        conn.execute_batch(&format!("DROP TABLE IF EXISTS {table};", table = agg.table))?;
        conn.execute_batch(&sql)?;
    }
    validate_rollups(&conn, model, &aggs)?;
    conn.execute_batch("DETACH src;")?;
    write_stamp(&conn, &stamp)?;
    enable(aggs);
    Ok(())
}

fn source_stamp(path: &str) -> (u64, u64) {
    std::fs::metadata(path)
        .map(|m| {
            (
                m.len(),
                m.modified()
                    .ok()
                    .map(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0)
                    })
                    .unwrap_or(0),
            )
        })
        .unwrap_or((0, 0))
}

fn sidecar_current(conn: &Connection, stamp: &(u64, u64)) -> bool {
    let Ok(meta) = conn.query_row(
        "SELECT size, mtime, version FROM agg_meta LIMIT 1",
        [],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
            ))
        },
    ) else {
        return false;
    };
    meta.0 as u64 == stamp.0 && meta.1 as u64 == stamp.1 && meta.2 == AGG_SCHEMA_VERSION
}

fn table_exists(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM information_schema.tables WHERE table_name = ? LIMIT 1",
        [table],
        |_| Ok(()),
    )
    .is_ok()
}

fn write_stamp(conn: &Connection, stamp: &(u64, u64)) -> Result<(), duckdb::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS agg_meta (size BIGINT, mtime BIGINT, version VARCHAR);",
    )?;
    conn.execute_batch(&format!(
        "DELETE FROM agg_meta; INSERT INTO agg_meta VALUES ({}, {}, '{}');",
        stamp.0, stamp.1, AGG_SCHEMA_VERSION
    ))?;
    Ok(())
}

/// Verify every rollup's measure sums match the source fact (relative tolerance
/// 1e-6 for floating-point aggregation-order differences). A mismatch means the
/// rollup is wrong (bad join, wrong fact, missing dimension) — never serve it.
fn validate_rollups(
    conn: &Connection,
    model: &SemanticModel,
    aggs: &[Aggregation],
) -> Result<(), duckdb::Error> {
    let fact = &model.fact_table(0).table_name;
    for agg in aggs {
        for base in agg.measure_columns.values() {
            let fact_total: f64 =
                conn.query_row(&format!("SELECT SUM({base}) FROM src.{fact}"), [], |r| {
                    r.get(0)
                })?;
            let rollup_total: f64 = conn.query_row(
                &format!("SELECT SUM({base}) FROM {table}", table = agg.table),
                [],
                |r| r.get(0),
            )?;
            let denom = fact_total.abs().max(rollup_total.abs()).max(1.0);
            if (fact_total - rollup_total).abs() > denom * 1e-6 {
                return Err(duckdb::Error::InvalidParameterName(format!(
                    "aggregation validation failed: {}.{base} sum {rollup_total} != fact {fact_total}",
                    agg.table
                )));
            }
        }
    }
    Ok(())
}

/// The `CREATE TABLE AS` for one rollup: scan the source fact, join the date
/// dim, group by the date ancestors + leaf dims, and pre-sum the measures.
fn build_sql(model: &SemanticModel, agg: &Aggregation) -> String {
    let fact = &model.fact_table(0).table_name;
    let rel = model
        .rel_for_dimension(&agg.date_dim_id)
        .expect("date dim has relationship");
    let mut cols: Vec<String> = Vec::new();
    for (i, level) in model.dim_def(&agg.date_dim_id).levels[..=agg.date_level]
        .iter()
        .enumerate()
    {
        cols.push(format!("d.{} AS {}", level.column, agg.date_columns[i]));
    }
    for col in agg.leaf_columns.values() {
        cols.push(format!("f.{col} AS {col}"));
    }
    for base in agg.measure_columns.values() {
        cols.push(format!("SUM(f.{base}) AS {base}"));
    }
    let group_count = agg.date_columns.len() + agg.leaf_columns.len();
    let groups: Vec<String> = (1..=group_count).map(|i| i.to_string()).collect();
    format!(
        "CREATE TABLE {table} AS SELECT {cols} FROM src.{fact} f \
         JOIN src.{dim_table} d ON f.{fact_col} = d.{dim_col} GROUP BY {groups}",
        table = agg.table,
        cols = cols.join(", "),
        fact = fact,
        dim_table = rel.dim_table,
        fact_col = rel.fact_column,
        dim_col = rel.dim_column,
        groups = groups.join(", "),
    )
}

/// Map a date-dim filter to its level index (leaf = usize::MAX, unknown = 0).
pub(crate) fn filter_level(
    model: &SemanticModel,
    dim_id: &str,
    filter: &TypedDimensionFilter,
) -> usize {
    let Some(dim) = model.dim_def_opt(dim_id) else {
        return 0;
    };
    match &filter.level {
        Some(name) => dim
            .levels
            .iter()
            .position(|l| &l.name == name)
            .unwrap_or(usize::MAX),
        None => usize::MAX,
    }
}

/// Whether `agg` can answer a plan whose group-by dims are `group_by`, date
/// grain is `group_level`, and filters are `filters` (measure checked separately).
fn agg_covers(
    model: &SemanticModel,
    agg: &Aggregation,
    group_by: &[String],
    group_level: usize,
    filters: &[TypedDimensionFilter],
) -> bool {
    if agg.date_level < group_level {
        return false;
    }
    for dim in group_by {
        if *dim != agg.date_dim_id && !agg.leaf_columns.contains_key(dim) {
            return false;
        }
    }
    for f in filters {
        if f.time_flag.is_some() {
            return false;
        }
        if f.dimension == agg.date_dim_id {
            if filter_level(model, &agg.date_dim_id, f) > agg.date_level {
                return false;
            }
        } else if !agg.leaf_columns.contains_key(&f.dimension) {
            return false;
        }
    }
    true
}

/// Match a plan to the coarsest rollup that can answer it, if any.
pub fn agg_for_plan(model: &SemanticModel, plan: &QueryPlan) -> Option<&'static Aggregation> {
    agg_for_plan_with(model, aggregations(), plan)
}

pub(crate) fn agg_for_plan_with<'a>(
    model: &SemanticModel,
    aggs: &'a [Aggregation],
    plan: &QueryPlan,
) -> Option<&'a Aggregation> {
    if aggs.is_empty() {
        return None;
    }
    let (measure, group_by, group_level, filters) = match plan {
        QueryPlan::Total { measure, filters } => (measure, &[][..], 0, filters),
        QueryPlan::GroupBy {
            measure,
            group_by,
            filters,
            group_level,
            ..
        } => {
            let gl = if group_by.is_empty() {
                0
            } else {
                group_level.unwrap_or(0)
            };
            (measure, group_by.as_slice(), gl, filters)
        }
        _ => return None,
    };
    aggs.iter().find(|agg| {
        agg.measure_columns.contains_key(measure)
            && agg_covers(model, agg, group_by, group_level, filters)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::plan::QueryPlan;

    #[test]
    fn measure_base_column_extracts_sum_col() {
        assert_eq!(measure_base_column("SUM(revenue)"), Some("revenue".into()));
        assert_eq!(measure_base_column(" SUM(units) "), Some("units".into()));
        assert_eq!(measure_base_column("COUNT(*)"), None);
        assert_eq!(measure_base_column("AVG(x)"), None);
        assert_eq!(measure_base_column("SUM(a+b)"), None);
    }

    fn project3_model() -> crate::engine::model::SemanticModel {
        crate::project::project::ProxyProject::load("projects/project3/proxy-config.json")
            .expect("load project3")
            .model
    }

    #[test]
    fn design_aggregations_builds_date_rollups() {
        let model = project3_model();
        let aggs = design_aggregations(&model);
        // Date dim has Year/Quarter/Month/Date; leaf level is skipped.
        let tables: Vec<&str> = aggs.iter().map(|a| a.table.as_str()).collect();
        assert_eq!(tables, vec!["agg_year", "agg_quarter", "agg_month"]);
        let year = &aggs[0];
        assert_eq!(year.date_columns, vec!["year"]);
        assert!(year.leaf_columns.contains_key("Category"));
        assert_eq!(year.measure_columns.get("Revenue"), Some(&"revenue".into()));
        assert_eq!(year.measure_columns.get("Units"), Some(&"units".into()));
        // Time-intelligence measures fall back to the fact.
        assert!(!year.measure_columns.contains_key("RevenueYTD"));
    }

    #[test]
    fn agg_for_plan_routes_grain_and_falls_back_at_leaf() {
        let model = project3_model();
        let aggs = design_aggregations(&model);

        let total = QueryPlan::Total {
            measure: "Revenue".into(),
            filters: vec![],
        };
        assert_eq!(
            agg_for_plan_with(&model, &aggs, &total).unwrap().table,
            "agg_year"
        );

        let year_drill = QueryPlan::GroupBy {
            measure: "Revenue".into(),
            group_by: vec!["Date".into()],
            filters: vec![],
            group_level: Some(0),
            set_op: None,
        };
        assert_eq!(
            agg_for_plan_with(&model, &aggs, &year_drill).unwrap().table,
            "agg_year"
        );

        let month_drill = QueryPlan::GroupBy {
            measure: "Revenue".into(),
            group_by: vec!["Date".into()],
            filters: vec![],
            group_level: Some(2),
            set_op: None,
        };
        assert_eq!(
            agg_for_plan_with(&model, &aggs, &month_drill)
                .unwrap()
                .table,
            "agg_month"
        );

        // Leaf (day) grain: no rollup can answer it -> fall back to the fact.
        let leaf_drill = QueryPlan::GroupBy {
            measure: "Revenue".into(),
            group_by: vec!["Date".into()],
            filters: vec![],
            group_level: Some(3),
            set_op: None,
        };
        assert!(agg_for_plan_with(&model, &aggs, &leaf_drill).is_none());

        // Non-additive / time-intelligence measure -> fall back.
        let ytd = QueryPlan::Total {
            measure: "RevenueYTD".into(),
            filters: vec![],
        };
        assert!(agg_for_plan_with(&model, &aggs, &ytd).is_none());
    }

    #[test]
    fn ensure_aggregations_builds_correct_rollups() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("mallardcube-agg-src-{}.duckdb", std::process::id()));
        let agg_path = dir.join(format!(
            "mallardcube-agg-sidecar-{}.duckdb",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&agg_path);

        crate::backend::Backend::create_demo_file(&path).expect("create demo file");
        let model = project3_model();

        ensure_aggregations(path.to_str().unwrap(), agg_path.to_str().unwrap(), &model)
            .expect("build aggregations");

        let fact = crate::backend::Backend::open(&path).expect("open fact");
        let fact_total = fact.query_scalar("SELECT SUM(revenue) FROM sales_fact");
        drop(fact);

        let sidecar = Connection::open(&agg_path).expect("open sidecar");
        let rollup_total: f64 = sidecar
            .query_row("SELECT SUM(revenue) FROM agg_year", [], |r| r.get(0))
            .expect("rollup sum");
        assert!(
            (fact_total - rollup_total).abs() <= fact_total.abs().max(1.0) * 1e-6,
            "rollup total {rollup_total} != fact total {fact_total}"
        );

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&agg_path);
    }

    #[test]
    fn validate_rollups_detects_mismatch() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("mallardcube-agg-val-{}.duckdb", std::process::id()));
        let _ = std::fs::remove_file(&path);
        crate::backend::Backend::create_demo_file(&path).expect("create demo file");
        let model = project3_model();
        let aggs = design_aggregations(&model);
        let year = &aggs[0];

        let conn = Connection::open_in_memory().expect("in-memory sidecar");
        conn.execute_batch(&format!(
            "ATTACH '{}' AS src (READ_ONLY);",
            path.to_str().unwrap()
        ))
        .expect("attach source");

        // A wrong rollup (all zeros) must fail validation.
        conn.execute_batch("CREATE TABLE agg_year (year BIGINT, revenue DOUBLE, units DOUBLE)")
            .expect("create wrong rollup");
        conn.execute_batch("INSERT INTO agg_year VALUES (0, 0.0, 0.0)")
            .expect("insert wrong rollup");
        assert!(validate_rollups(&conn, &model, std::slice::from_ref(year)).is_err());

        // The real rollup must pass.
        conn.execute_batch("DROP TABLE agg_year").expect("drop");
        conn.execute_batch(&build_sql(&model, year))
            .expect("build correct rollup");
        assert!(validate_rollups(&conn, &model, std::slice::from_ref(year)).is_ok());

        let _ = std::fs::remove_file(&path);
    }
}
