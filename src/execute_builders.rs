/// Cellset response builders.
///
/// Converts a `SemanticQuery` (from `mdx_semantic`) into a full
/// mddataset XML response, backed by the current `Backend`.
///
/// Also contains the flat-rowset fallback responses for MDX and DAX.
///
/// Member/cell/axis/slicer helpers live in `axis_members`.

use crate::response::wrap_in_soap_envelope;
use crate::backend::{Backend, QueryBackend};
use crate::engine::plan::{QueryResult, execute_plan, execute_plan_with_sql, execute_plan_with_backend, plan_from_semantic};
use crate::engine::model::SemanticModel;
use crate::engine::normalize::plan_key;
use crate::engine::timing::{Timings, RuntimePath};
use crate::mdx_semantic::{SemanticQuery, SemanticQueryKind};
use crate::axis_members::{
    render_response, full_slicer_axis, measures_axis,
    single_member_axis, member_list_axis, empty_member_list_axis,
    row_dim, leaf_member_for, all_member_for, hierarchy_for, leaf_members_from,
    measurement_cell, count_cell, measures_hierarchy, measures_total_member,
    cchildren_member,
};

// ---- cellset response builders ----

fn ordered_pair(
    dims: &[String],
    d0: &str,
    m0: crate::cellset::MemberConfig,
    d1: &str,
    m1: crate::cellset::MemberConfig,
) -> crate::cellset::TupleConfig {
    let first = dims.first().map(|s| s.as_str()).unwrap_or(d0);
    if first == d1 {
        crate::cellset::TupleConfig { members: vec![m1, m0] }
    } else {
        crate::cellset::TupleConfig { members: vec![m0, m1] }
    }
}

fn build_slicer_only(query: &SemanticQuery, result: &QueryResult) -> String {
    let total = match result {
        QueryResult::Scalar(v) => *v,
        _ => unreachable!(),
    };
    render_response(
        vec![full_slicer_axis(query)],
        vec![measurement_cell(0, total)],
        &query.cell_props,
    )
}

fn build_drilldown(query: &SemanticQuery, result: &QueryResult) -> String {
    let dims = &query.axis_dimensions;
    if dims.len() >= 2 {
        return build_drilldown_multi(query, result);
    }
    let mut fallback_dim = String::new();
    let dim = dims.first().map(|s| s.as_str())
        .unwrap_or_else(|| {
            fallback_dim = crate::proxy_project::project()
                .model.default_dimension_id()
                .unwrap_or_else(|| "Produktkategori".into());
            &fallback_dim
        });
    let mut data = match result {
        QueryResult::Grouped(data) => data.clone(),
        _ => unreachable!(),
    };
    data.sort_by(|a, b| a.0.cmp(&b.0));
    let members = leaf_members_from(dim,
        &data.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(),
        &query.dim_props,
    );

    let mut cells = Vec::new();
    for (i, (_name, value)) in data.iter().enumerate() {
        cells.push(measurement_cell(i as u32, *value));
    }

    render_response(
        vec![
            member_list_axis("Axis0", hierarchy_for(dim, &query.dim_props), members),
            full_slicer_axis(query),
        ],
        cells,
        &query.cell_props,
    )
}

fn build_drilldown_multi(query: &SemanticQuery, result: &QueryResult) -> String {
    let dims = &query.axis_dimensions;
    let mut all_data = match result {
        QueryResult::Pairs(pairs) => pairs.clone(),
        _ => unreachable!(),
    };
    // Stable sort by axis dimension order so Excel groups correctly.
    all_data.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let has_exclusions = !query.excluded_members.is_empty();

    let mut hierarchies: Vec<crate::cellset::HierarchyConfig> = Vec::new();
    for dim in dims {
        hierarchies.push(hierarchy_for(dim, &query.dim_props));
    }

    let d0 = &dims[0];
    let d1 = &dims[1];

    let mut tuples: Vec<crate::cellset::TupleConfig> = Vec::new();
    let mut cells = Vec::new();
    let mut ordinal = 0u32;
    for (first, second, value) in &all_data {
        if has_exclusions && query.excluded_members.iter().any(|e| e.key == *first || e.key == *second) {
            continue;
        }
        let m0 = leaf_member_for(d0, first, &query.dim_props);
        let m1 = leaf_member_for(d1, second, &query.dim_props);
        tuples.push(ordered_pair(dims, d0, m0, d1, m1));
        cells.push(measurement_cell(ordinal, *value));
        ordinal += 1;
    }

    let axis = crate::cellset::AxisConfig {
        name: "Axis0".into(),
        hierarchies,
        tuples,
    };

    render_response(
        vec![axis, full_slicer_axis(query)],
        cells,
        &query.cell_props,
    )
}

fn build_drilldown_member(query: &SemanticQuery, result: &QueryResult) -> String {
    let dims = &query.axis_dimensions;
    let mut all_data = match result {
        QueryResult::Pairs(pairs) => pairs.clone(),
        _ => unreachable!(),
    };
    // Stable sort so collapsed + visible rows group correctly.
    all_data.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    let d0 = &dims[0];
    let d1 = &dims[1];

    let mut hierarchies: Vec<crate::cellset::HierarchyConfig> = Vec::new();
    for dim in dims {
        hierarchies.push(hierarchy_for(dim, &query.dim_props));
    }

    let excluded_d0: std::collections::HashSet<&str> = query.excluded_members.iter()
        .filter(|e| e.dimension == *d0)
        .map(|e| e.key.as_str())
        .collect();
    let excluded_d1: std::collections::HashSet<&str> = query.excluded_members.iter()
        .filter(|e| e.dimension == *d1)
        .map(|e| e.key.as_str())
        .collect();

    let mut col_d0_totals: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    let mut col_d1_totals: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for (first, second, value) in &all_data {
        if excluded_d0.contains(first.as_str()) {
            *col_d0_totals.entry(first.clone()).or_insert(0.0) += value;
        }
        if excluded_d1.contains(second.as_str()) {
            *col_d1_totals.entry(second.clone()).or_insert(0.0) += value;
        }
    }

    let mut tuples: Vec<crate::cellset::TupleConfig> = Vec::new();
    let mut cells = Vec::new();
    let mut ordinal = 0u32;
    let mut seen_d0_col: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut seen_d1_col: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (first, second, value) in &all_data {
        if excluded_d0.contains(first.as_str()) {
            if !seen_d0_col.contains(first) {
                seen_d0_col.insert(first.clone());
                let total = col_d0_totals.get(first).copied().unwrap_or(0.0);
                let m0 = leaf_member_for(d0, first, &query.dim_props);
                let m1 = all_member_for(d1, &query.dim_props);
                tuples.push(ordered_pair(dims, d0, m0, d1, m1));
                cells.push(measurement_cell(ordinal, total));
                ordinal += 1;
            }
            continue;
        }

        if excluded_d1.contains(second.as_str()) {
            if !seen_d1_col.contains(second) {
                seen_d1_col.insert(second.clone());
                let total = col_d1_totals.get(second).copied().unwrap_or(0.0);
                let m0 = all_member_for(d0, &query.dim_props);
                let m1 = leaf_member_for(d1, second, &query.dim_props);
                tuples.push(ordered_pair(dims, d0, m0, d1, m1));
                cells.push(measurement_cell(ordinal, total));
                ordinal += 1;
            }
            continue;
        }

        let m0 = leaf_member_for(d0, first, &query.dim_props);
        let m1 = leaf_member_for(d1, second, &query.dim_props);
        tuples.push(ordered_pair(dims, d0, m0, d1, m1));
        cells.push(measurement_cell(ordinal, *value));
        ordinal += 1;
    }

    let axis = crate::cellset::AxisConfig {
        name: "Axis0".into(),
        hierarchies,
        tuples,
    };

    render_response(
        vec![axis, full_slicer_axis(query)],
        cells,
        &query.cell_props,
    )
}

fn build_measure_by_category(query: &SemanticQuery, result: &QueryResult) -> String {
    let dim = row_dim(query);
    let data = match result {
        QueryResult::Grouped(data) => data,
        _ => unreachable!(),
    };
    let axis1_members = leaf_members_from(dim,
        &data.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(),
        &query.dim_props,
    );
    let mut cells = Vec::new();
    for (i, (_name, value)) in data.iter().enumerate() {
        cells.push(measurement_cell(i as u32, *value));
    }

    render_response(
        vec![
            measures_axis(),
            member_list_axis("Axis1", hierarchy_for(dim, &query.dim_props), axis1_members),
            full_slicer_axis(query),
        ],
        cells,
        &query.cell_props,
    )
}

fn build_slicer_all_and_measure(query: &SemanticQuery, result: &QueryResult) -> String {
    let total = match result {
        QueryResult::Scalar(v) => *v,
        _ => unreachable!(),
    };
    render_response(
        vec![full_slicer_axis(query)],
        vec![measurement_cell(0, total)],
        &query.cell_props,
    )
}

fn build_all_level_members(query: &SemanticQuery, result: &QueryResult) -> String {
    let dim = row_dim(query);
    let total = match result {
        QueryResult::Scalar(v) => *v,
        _ => unreachable!(),
    };
    render_response(
        vec![
            single_member_axis("Axis0", hierarchy_for(dim, &query.dim_props), all_member_for(dim, &query.dim_props)),
            full_slicer_axis(query),
        ],
        vec![measurement_cell(0, total)],
        &query.cell_props,
    )
}

fn build_leaf_level_members(query: &SemanticQuery, result: &QueryResult) -> String {
    let dim = row_dim(query);
    let data = match result {
        QueryResult::Grouped(data) => data,
        _ => unreachable!(),
    };
    let members = leaf_members_from(dim,
        &data.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(),
        &query.dim_props,
    );
    let mut cells = Vec::new();
    for (i, (_name, value)) in data.iter().enumerate() {
        cells.push(measurement_cell(i as u32, *value));
    }

    render_response(
        vec![
            member_list_axis("Axis0", hierarchy_for(dim, &query.dim_props), members),
            full_slicer_axis(query),
        ],
        cells,
        &query.cell_props,
    )
}

fn build_leaf_children_empty(query: &SemanticQuery, _result: &QueryResult) -> String {
    let dim = row_dim(query);
    render_response(
        vec![
            empty_member_list_axis("Axis0", hierarchy_for(dim, &query.dim_props)),
            full_slicer_axis(query),
        ],
        vec![],
        &query.cell_props,
    )
}

fn build_measure_children_empty(query: &SemanticQuery, _result: &QueryResult) -> String {
    render_response(
        vec![
            empty_member_list_axis("Axis0", measures_hierarchy()),
            full_slicer_axis(query),
        ],
        vec![],
        &query.cell_props,
    )
}

fn build_cchildren_for_all(query: &SemanticQuery, result: &QueryResult) -> String {
    let dim = row_dim(query);
    let count = match result {
        QueryResult::Count(c) => *c,
        _ => unreachable!(),
    };
    render_response(
        vec![
            single_member_axis("Axis0", hierarchy_for(dim, &query.dim_props), all_member_for(dim, &query.dim_props)),
            single_member_axis("Axis1", measures_hierarchy(), cchildren_member()),
            full_slicer_axis(query),
        ],
        vec![count_cell(0, count)],
        &query.cell_props,
    )
}

fn build_cchildren_for_leaf_product(query: &SemanticQuery, name: &str, result: &QueryResult) -> String {
    let dim = row_dim(query);
    let leaf = leaf_member_for(dim, name, &query.dim_props);
    let all = all_member_for(dim, &query.dim_props);
    let real_count = match result {
        QueryResult::Count(c) => *c,
        _ => unreachable!(),
    };
    render_response(
        vec![
            member_list_axis("Axis0", hierarchy_for(dim, &query.dim_props), vec![all, leaf]),
            single_member_axis("Axis1", measures_hierarchy(), cchildren_member()),
            full_slicer_axis(query),
        ],
        vec![count_cell(0, real_count), count_cell(1, 0)],
        &query.cell_props,
    )
}

fn build_cchildren_for_measures(query: &SemanticQuery, _result: &QueryResult) -> String {
    render_response(
        vec![
            single_member_axis("Axis0", measures_hierarchy(), measures_total_member()),
            single_member_axis("Axis1", measures_hierarchy(), cchildren_member()),
            full_slicer_axis(query),
        ],
        vec![count_cell(0, 0)],
        &query.cell_props,
    )
}

// ---- public API consumed by execute.rs dispatch ----

pub fn execute_semantic_query(query: &SemanticQuery) -> String {
    let plan = plan_from_semantic(query);
    let model = &crate::proxy_project::project().model;
    let result = execute_plan(&plan, model);
    dispatch(query, &result)
}

pub fn execute_semantic_query_with_backend<B: QueryBackend>(
    query: &SemanticQuery,
    backend: &B,
    model: &SemanticModel,
) -> String {
    let plan = plan_from_semantic(query);
    let result = execute_plan_with_backend(&plan, model, backend);
    dispatch(query, &result)
}

fn dispatch(query: &SemanticQuery, result: &QueryResult) -> String {
    match query.kind {
        SemanticQueryKind::ChildrenCountForAll => build_cchildren_for_all(query, &result),
        SemanticQueryKind::ChildrenCountLeafProduct => {
            let name = query.cchildren_leaf_name.as_deref().unwrap_or("");
            build_cchildren_for_leaf_product(query, name, &result)
        }
        SemanticQueryKind::ChildrenCountMeasures => build_cchildren_for_measures(query, &result),
        SemanticQueryKind::SlicerAllAndMeasure => build_slicer_all_and_measure(query, &result),
        SemanticQueryKind::MeasureChildrenEmpty => build_measure_children_empty(query, &result),
        SemanticQueryKind::LeafChildrenEmpty => build_leaf_children_empty(query, &result),
        SemanticQueryKind::AllLevelMembers => build_all_level_members(query, &result),
        SemanticQueryKind::LeafLevelMembers => build_leaf_level_members(query, &result),
        SemanticQueryKind::MeasureByCategory => build_measure_by_category(query, &result),
        SemanticQueryKind::DrilldownCategories => build_drilldown(query, &result),
        SemanticQueryKind::SlicerOnly => build_slicer_only(query, &result),
        SemanticQueryKind::DrilldownMemberProbe => build_drilldown_member(query, &result),
    }
}

pub fn get_execute_cellset_response(mdx: &str) -> String {
    let query = crate::mdx_semantic::semantic_query_from_mdx(mdx);
    execute_semantic_query(&query)
}

use crate::engine::malloy_compiler::MalloyCompiler;
use crate::engine::malloy_node_longlived::LongLivedNodeMalloyCompiler;
use crate::engine::cache::PlanCache;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// Toggle between direct SQL and Malloy runtime path.
/// Set via env var MALLOY_RUNTIME=1 or programmatically.
pub static USE_MALLOY_RUNTIME: AtomicBool = AtomicBool::new(false);

/// Enable Malloy runtime for analytic queries (Total, GroupBy).
pub fn enable_malloy_runtime() {
    USE_MALLOY_RUNTIME.store(true, Ordering::Relaxed);
}

pub fn disable_malloy_runtime() {
    USE_MALLOY_RUNTIME.store(false, Ordering::Relaxed);
}

/// Module-level long-lived Malloy compiler (lazy, spawned on first use).
static COMPILER: OnceLock<LongLivedNodeMalloyCompiler> = OnceLock::new();

/// Module-level compiled-SQL cache shared across all requests.
static CACHE: OnceLock<PlanCache> = OnceLock::new();

fn malloy_compiler() -> &'static LongLivedNodeMalloyCompiler {
    COMPILER.get_or_init(|| {
        LongLivedNodeMalloyCompiler::new().expect("start Malloy compiler")
    })
}

fn malloy_cache() -> &'static PlanCache {
    CACHE.get_or_init(PlanCache::new)
}

/// Eagerly spawn the long-lived Malloy compiler and warm its internal
/// caches so the first Excel request doesn't pay the startup cost.
/// Call once at server startup when MALLOY_RUNTIME=1.
pub fn warm_malloy_worker() {
    use std::time::Instant;
    use crate::engine::malloy_compiler::MalloyCompiler;
    use crate::engine::plan::QueryPlan;
    let model = &crate::proxy_project::project().model;
    let plan = QueryPlan::Total { measure: "TotalSales".into(), filters: vec![] };
    let t1 = Instant::now();
    match malloy_compiler().compile_query(&model, &plan) {
        Ok(r) => {
            let warm_ms = t1.elapsed().as_millis();
            eprintln!(
                "[malloy] warm-up compile OK in {warm_ms}ms (JS compile {:.2}ms)",
                r.compile_ms,
            );
        }
        Err(e) => {
            eprintln!("[malloy] warm-up compile FAILED: {e}");
        }
    }
}

/// Instrumented variant — collects timing spans and logs them to stderr.
/// Use for Excel workload measurement. Always uses the direct SQL path.
pub fn get_execute_cellset_response_timed(mdx: &str) -> (String, Timings) {
    use std::time::Instant;

    let t0 = Instant::now();
    let query = crate::mdx_semantic::semantic_query_from_mdx(mdx);
    let mdx_parse_us = (Instant::now() - t0).as_micros() as u64;

    let t0 = Instant::now();
    let plan = plan_from_semantic(&query);
    let plan_us = (Instant::now() - t0).as_micros() as u64;
    let key = plan_key(&plan);

    let t0 = Instant::now();
    let model = &crate::proxy_project::project().model;
    let result = execute_plan(&plan, model);
    let sql_execute_us = (Instant::now() - t0).as_micros() as u64;

    let mut timings = Timings::new(RuntimePath::DirectSql, key, mdx_parse_us, 0);
    timings.plan_us = plan_us;
    timings.sql_execute_us = sql_execute_us;

    let t0 = Instant::now();
    let xml = dispatch(&query, &result);
    timings.xml_render_us = (Instant::now() - t0).as_micros() as u64;
    timings.finish();
    (xml, timings)
}

/// Instrumented variant with optional Malloy runtime path.
/// When USE_MALLOY_RUNTIME is true and the query is a supported analytic shape,
/// the SQL is obtained via the long-lived Malloy compiler instead of the Rust
/// SQL emitter. Compiled SQL is cached by PlanKey.
pub fn get_execute_cellset_response_timed_malloy(mdx: &str) -> (String, Timings) {
    use std::time::Instant;

    let t0 = Instant::now();
    let query = crate::mdx_semantic::semantic_query_from_mdx(mdx);
    let mdx_parse_us = (Instant::now() - t0).as_micros() as u64;

    let t0 = Instant::now();
    let plan = plan_from_semantic(&query);
    let plan_us = (Instant::now() - t0).as_micros() as u64;
    let key = plan_key(&plan);

    let model = &crate::proxy_project::project().model;
    let use_malloy = USE_MALLOY_RUNTIME.load(Ordering::Relaxed)
        && matches!(query.kind, SemanticQueryKind::SlicerAllAndMeasure
            | SemanticQueryKind::SlicerOnly
            | SemanticQueryKind::DrilldownCategories
            | SemanticQueryKind::LeafLevelMembers
            | SemanticQueryKind::MeasureByCategory
            | SemanticQueryKind::DrilldownMemberProbe);

    let (result, runtime_path, malloy_compile_us, compiled_cache_hit, js_compile_ms, sql_execute_us) = if use_malloy {
        let compiler = malloy_compiler();

        let t0 = Instant::now();
        // When a developer-supplied Malloy model is loaded, use it
        // directly instead of generating model text from SemanticModel.
        let project = crate::proxy_project::project();
        let source = project.malloy_source(&plan);
        let (sql, was_hit, worker_compile_ms) = if project.malloy_model_text.is_empty() {
            let cache = malloy_cache();
            cache.get_or_compile(&plan, &model, compiler)
                .unwrap_or_else(|_| (String::new(), false, 0.0))
        } else {
            match compiler.compile_malloy(&source) {
                Ok(cr) => (cr.sql, false, cr.compile_ms),
                Err(_) => (String::new(), false, 0.0),
            }
        };
        let compile_us = (Instant::now() - t0).as_micros() as u64;
        let path = if was_hit { RuntimePath::MalloyCached } else { RuntimePath::MalloyCompiled };

        // Execute compiled SQL instead of direct Rust-generated SQL
        let t0 = Instant::now();
        let r = execute_plan_with_sql(&plan, &sql);
        let exec_us = (Instant::now() - t0).as_micros() as u64;

        (r, path, compile_us, was_hit, worker_compile_ms, exec_us)
    } else {
        let t0 = Instant::now();
        let r = execute_plan(&plan, &model);
        let exec_us = (Instant::now() - t0).as_micros() as u64;
        (r, RuntimePath::DirectSql, 0, false, 0.0, exec_us)
    };

    let mut timings = Timings::new(runtime_path, key, mdx_parse_us, 0);
    timings.plan_us = plan_us;
    timings.malloy_compile_us = malloy_compile_us;
    timings.compiled_sql_cache_hit = compiled_cache_hit;
    timings.js_compile_ms = js_compile_ms;
    timings.sql_execute_us = sql_execute_us;

    let t0 = Instant::now();
    let xml = dispatch(&query, &result);
    timings.xml_render_us = (Instant::now() - t0).as_micros() as u64;
    timings.finish();
    (xml, timings)
}

pub fn get_execute_cellset_response_with_backend<B: QueryBackend>(
    mdx: &str,
    backend: &B,
    model: &SemanticModel,
) -> String {
    let query = crate::mdx_semantic::semantic_query_from_mdx(mdx);
    execute_semantic_query_with_backend(&query, backend, model)
}

pub fn get_execute_mdx_response(mdx: &str) -> String {
    let has_measures = mdx.contains("Measures") || mdx.contains("measures");
    let measure_name = "Total_Forsaljning";
    let measure_value = if has_measures { Backend::get().total_sales() } else { 0.0 };

    let inner = format!(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence minOccurs="0" maxOccurs="unbounded"><xsd:element name="row" type="row"/></xsd:sequence></xsd:complexType>
            </xsd:element>
            <xsd:complexType name="row">
              <xsd:sequence>
                <xsd:element sql:field="{name}" name="{name}" type="xsd:double" minOccurs="0"/>
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
          <row>
            <{name}>{val}</{name}>
          </row>
        </root>
      </return>
    </ExecuteResponse>"#,
        name = measure_name,
        val = measure_value,
    );
    wrap_in_soap_envelope(&inner)
}

pub fn get_execute_dax_response(_dax: &str) -> String {
    let total = Backend::get().total_sales();
    let col_xml_name = "Faktatabell_x005B_Total_x0020_Försäljning_x0020__x0028_SEK_x0029__x005D_";
    let col_sql_field = "[Faktatabell].[Total Försäljning (SEK)]";

    let inner = format!(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence minOccurs="0" maxOccurs="unbounded"><xsd:element name="row" type="row"/></xsd:sequence></xsd:complexType>
            </xsd:element>
            <xsd:complexType name="row">
              <xsd:sequence>
                <xsd:element sql:field="{sqlf}" name="{xname}" type="xsd:double" minOccurs="0"/>
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
          <row>
            <{xname}>{val}</{xname}>
          </row>
        </root>
      </return>
    </ExecuteResponse>"#,
        sqlf = col_sql_field,
        xname = col_xml_name,
        val = total,
    );
    wrap_in_soap_envelope(&inner)
}
