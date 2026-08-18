use crate::axis_members::{
    all_member_for_with_backend, cchildren_member, count_cell, dims_only_slicer_axis_with_backend,
    empty_member_list_axis, filter_dim_props, full_slicer_axis_with_backend, hierarchy_for,
    leaf_member_for, leaf_member_for_level, leaf_members_from, measurement_cell_for,
    measurement_cell_for_query, measures_axis_for_query, measures_hierarchy, measures_member,
    measures_total_member, member_list_axis, render_response, row_dim, single_member_axis,
};
use crate::backend::QueryBackend;
use crate::cellset;
use crate::engine::plan::QueryResult;
/// Cellset render functions.
///
/// Converts a `SemanticQuery` + `QueryResult` into an XMLA cellset
/// XML string.  Each `build_*` function handles one query shape.
/// `dispatch()` routes by `SemanticQueryKind`.
use crate::mdx_semantic::{SemanticQuery, SemanticQueryKind};
use crate::response::xml_escape;

pub(crate) fn ordered_pair(
    dims: &[String],
    d0: &str,
    m0: crate::cellset::MemberConfig,
    d1: &str,
    m1: crate::cellset::MemberConfig,
) -> crate::cellset::TupleConfig {
    let first = dims.first().map(|s| s.as_str()).unwrap_or(d0);
    if first == d1 {
        crate::cellset::TupleConfig {
            members: vec![m1, m0],
        }
    } else {
        crate::cellset::TupleConfig {
            members: vec![m0, m1],
        }
    }
}

pub(crate) fn build_slicer_only<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    result: &QueryResult,
    backend: &B,
) -> String {
    let total = match result {
        QueryResult::Scalar(v) => *v,
        _ => unreachable!(),
    };
    render_response(
        vec![full_slicer_axis_with_backend(query, backend)],
        vec![measurement_cell_for_query(query, 0, total)],
        &query.cell_props,
    )
}

/// Render a multi-measure query (several measures on Axis0, no row dimension).
/// Each measure gets its own tuple on the measures axis and one cell.
fn build_multi_measure<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    result: &QueryResult,
    backend: &B,
) -> String {
    let values = match result {
        QueryResult::Multi(v) => v.clone(),
        _ => return empty_cellset(query, backend),
    };
    let project = crate::proxy_project::project();
    let mut members = Vec::new();
    let mut measure_ids = Vec::new();
    for name in &query.measures {
        if let Some(m) = project.model.lookup_measure(name) {
            members.push(measures_member(&m.measure_unique_name(), &m.display_name));
            measure_ids.push(m.id.clone());
        }
    }
    let mut cells = Vec::new();
    for (i, value) in values.iter().enumerate() {
        if let Some(measure_id) = measure_ids.get(i) {
            cells.push(measurement_cell_for(i as u32, *value, measure_id));
        }
    }
    render_response(
        vec![
            member_list_axis("Axis0", measures_hierarchy(), members),
            dims_only_slicer_axis_with_backend(query, backend),
        ],
        cells,
        &query.cell_props,
    )
}

/// Render a set of arbitrary tuples on the axis (batched CUBEVALUE with
/// different slicers). Each input tuple is `(measure, member slicers)` and
/// produces one Axis0 tuple and one cell.
fn build_tuple_set<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    result: &QueryResult,
    backend: &B,
) -> String {
    let values = match result {
        QueryResult::Multi(v) => v.clone(),
        _ => return empty_cellset(query, backend),
    };
    let project = crate::proxy_project::project();

    let mut hierarchies = vec![measures_hierarchy()];
    let mut seen_dims: Vec<String> = Vec::new();
    let mut tuples = Vec::new();
    let mut measure_ids: Vec<String> = Vec::new();

    for t in &query.axis_tuples {
        let mut members = Vec::new();
        let measure_id = match &t.measure {
            Some(name) => project.model.lookup_measure(name).map(|m| {
                members.push(measures_member(&m.measure_unique_name(), &m.display_name));
                m.id.clone()
            }),
            None => None,
        };
        measure_ids.push(measure_id.unwrap_or_default());
        for f in &t.filters {
            if let Some(dim) = project.model.dim_def_opt(&f.dimension) {
                if !seen_dims.contains(&dim.id) {
                    seen_dims.push(dim.id.clone());
                    hierarchies.push(hierarchy_for(&dim.id, &query.dim_props));
                }
                for key in &f.members {
                    members.push(leaf_member_for_level(
                        &f.dimension,
                        key,
                        &query.dim_props,
                        f.level.as_deref(),
                    ));
                }
            }
        }
        tuples.push(crate::cellset::TupleConfig { members });
    }

    let mut cells = Vec::new();
    for (i, value) in values.iter().enumerate() {
        let measure_id = measure_ids.get(i).map(|s| s.as_str()).unwrap_or("");
        if !measure_id.is_empty() {
            cells.push(measurement_cell_for(i as u32, *value, measure_id));
        }
    }

    let axis = crate::cellset::AxisConfig {
        name: "Axis0".into(),
        hierarchies,
        tuples,
    };

    render_response(
        vec![axis, dims_only_slicer_axis_with_backend(query, backend)],
        cells,
        &query.cell_props,
    )
}

pub(crate) fn build_drilldown<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    result: &QueryResult,
    backend: &B,
) -> String {
    let dims = &query.axis_dimensions;
    if dims.len() >= 2 {
        return build_drilldown_multi(query, result, backend);
    }
    let mut fallback_dim = String::new();
    let dim = dims.first().map(|s| s.as_str()).unwrap_or_else(|| {
        fallback_dim = crate::proxy_project::project()
            .model
            .dimensions
            .first()
            .map(|d| d.id.clone())
            .expect("model has no dimensions");
        &fallback_dim
    });
    let mut data = match result {
        QueryResult::Grouped(data) => data.clone(),
        _ => unreachable!(),
    };
    // A set_op (TopCount/Order/Filter) already ordered the rows; keep that order.
    if query.axis_set_op.is_none() {
        data.sort_by(|a, b| a.0.cmp(&b.0));
    }
    let parent_uname: Option<String> = query.drilldown_level.and_then(|dl| {
        if dl == 0 {
            return None;
        }
        let project = crate::proxy_project::project();
        let dim_def = project.model.dim_def_opt(dim)?;
        let key = query
            .filters
            .iter()
            .find(|f| f.dimension == *dim)
            .and_then(|f| f.members.first())?;
        Some(level_member_uname(dim_def, dl - 1, key))
    });
    let mut members = leaf_members_from(
        dim,
        &data.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(),
        &query.dim_props,
        query.drilldown_level,
        parent_uname.as_deref(),
    );
    // Report the real per-member child count so Excel's expand indicators stay
    // consistent (a year has 4 quarters, a quarter has 3 months, a month has its
    // day count). Without this, Excel shows a missing "+" on years and a
    // mismatched expand state on quarters.
    if let Some(dl) = query.drilldown_level {
        let key_path = query
            .filters
            .iter()
            .find(|f| f.dimension == *dim)
            .and_then(|f| f.members.first())
            .cloned()
            .unwrap_or_default();
        let cc_map = drill_children_cardinalities(backend, dim, dl, &key_path);
        for m in &mut members {
            if let Some(cc) = cc_map.get(&m.caption) {
                m.children_cardinality = *cc;
                m.display_info = if *cc > 0 { 131075 } else { 3 };
            }
        }
    }
    // Prepend the full ancestor chain so every member's parent is either on the
    // axis or is the (All) root. Excel rebuilds the hierarchy tree from the
    // axis via PARENT_UNIQUE_NAME / PARENT_SAME_AS_PREV; a non-root parent that
    // is missing from the axis (drilling a quarter without its year, for
    // example) makes MDDSAxis::MoveToHierProperty crash. Real SSAS
    // DrilldownLevel returns the whole chain: [All, Year, Quarter, months...].
    let mut num_ancestors: u32 = 0;
    if let Some(dl) = query.drilldown_level {
        let project = crate::proxy_project::project();
        let dim_def = project.model.dim_def_opt(dim);
        let filter_key = query
            .filters
            .iter()
            .find(|f| f.dimension == *dim)
            .and_then(|f| f.members.first())
            .cloned()
            .unwrap_or_default();
        if let Some(def) = dim_def {
            let key_parts: Vec<&str> = filter_key.split('|').filter(|s| !s.is_empty()).collect();
            let mut ancestors: Vec<cellset::MemberConfig> = Vec::new();
            // (All) at hierarchy level 0.
            ancestors.push(all_member_for_with_backend(dim, &query.dim_props, backend));
            // Members at hierarchy levels 1..=dl (dim.levels indices 0..=dl-1).
            for i in 0..dl {
                if def.levels.get(i).is_none() || key_parts.len() < i + 1 {
                    break;
                }
                let anc_key = key_parts[..i + 1].join("|");
                let u_name = level_member_uname(def, i, &anc_key);
                let caption = anc_key.rsplit('|').next().unwrap_or(&anc_key).to_string();
                let l_name = format!("{}.[{}]", def.hierarchy_unique_name(), def.levels[i].name);
                let parent_uname = if i == 0 {
                    def.all_member_unique_name()
                } else {
                    level_member_uname(def, i - 1, &key_parts[..i].join("|"))
                };
                let cc = member_child_count(backend, dim, i, &anc_key);
                ancestors.push(cellset::MemberConfig {
                    hierarchy: def.hierarchy_unique_name(),
                    u_name,
                    caption: caption.clone(),
                    l_name,
                    l_num: (i + 1) as i32,
                    display_info: 0,
                    children_cardinality: cc,
                    dim_props: filter_dim_props(
                        vec![
                            ("PARENT_UNIQUE_NAME".into(), parent_uname),
                            ("HIERARCHY_UNIQUE_NAME".into(), def.hierarchy_unique_name()),
                            ("MEMBER_NAME".into(), caption.clone()),
                            ("MEMBER_KEY".into(), anc_key.clone()),
                            ("MEMBER_TYPE".into(), "1".into()),
                            ("MEMBER_VALUE".into(), caption.clone()),
                            ("PARENT_LEVEL".into(), i.to_string()),
                            ("PARENT_COUNT".into(), "1".into()),
                        ],
                        &query.dim_props,
                    ),
                });
            }
            num_ancestors = ancestors.len() as u32;
            for a in ancestors.into_iter().rev() {
                members.insert(0, a);
            }
        }
    }

    // Emit DISPLAY_INFO per the OLE DB for OLAP "Axis Rowsets" definition: the
    // low 16 bits are the number of children of the member, and the high word
    // holds two flags — DRILLED_DOWN (0x10000, a child of this member appears
    // immediately after it on the axis) and PARENT_SAME_AS_PREV (0x20000, this
    // member's parent equals the previous member's parent). Excel's
    // MDDSAxis::MoveToHierProperty walks the axis on these fields, so a wrong
    // child count (e.g. emitting 3 for a month that has ~30 day-children)
    // corrupts the hierarchy tree and crashes.
    let member_count = members.len();
    let mut prev_parent: Option<String> = None;
    for (i, m) in members.iter_mut().enumerate() {
        let parent = m
            .dim_props
            .iter()
            .find(|(tag, _)| tag == "PARENT_UNIQUE_NAME")
            .map(|(_, v)| v.clone());
        // Ancestor members are drilled down: a child immediately follows each.
        let drilled_down = (i as u32) < num_ancestors && member_count > num_ancestors as usize;
        let same_as_prev = i > 0 && parent == prev_parent;
        let mut di = m.children_cardinality.min(65535);
        if drilled_down {
            di |= 0x10000;
        }
        if same_as_prev {
            di |= 0x20000;
        }
        m.display_info = di;
        prev_parent = parent;
    }

    // One cell per axis tuple. Each ancestor carries the branch total (the
    // subquery restricts the slice to a single branch, so (All), the year, and
    // the quarter all aggregate to the same value as the visible children).
    let mut cells = Vec::new();
    let total: f64 = data.iter().map(|(_, v)| *v).sum();
    for ord in 0..num_ancestors {
        cells.push(measurement_cell_for_query(query, ord, total));
    }
    for (i, (_name, value)) in data.iter().enumerate() {
        cells.push(measurement_cell_for_query(
            query,
            num_ancestors + i as u32,
            *value,
        ));
    }

    render_response(
        vec![
            member_list_axis("Axis0", hierarchy_for(dim, &query.dim_props), members),
            full_slicer_axis_with_backend(query, backend),
        ],
        cells,
        &query.cell_props,
    )
}

/// Member unique name for a level, converting an internal pipe path to the
/// SSAS compound-key form: `2026|4` at level `Quarter` becomes
/// `[Date].[Date].[Quarter].&amp;[2026]&amp;[4]`.
fn level_member_uname(
    dim: &crate::engine::model::DimensionDef,
    level_idx: usize,
    key: &str,
) -> String {
    let level = dim
        .levels
        .get(level_idx)
        .map(|l| l.name.as_str())
        .unwrap_or("");
    let suffix: String = key
        .split('|')
        .map(|part| format!("&amp;[{part}]"))
        .collect();
    format!("{}.[{}].{suffix}", dim.hierarchy_unique_name(), level)
}

/// Per-member child count at `level_idx` of `dim`, scoped by the ancestor path
/// in `key_path` (e.g. `2026` for quarters under year 2026). Returns the number
/// of distinct values at the next level beneath each member.
fn drill_children_cardinalities<B: QueryBackend + ?Sized>(
    backend: &B,
    dim: &str,
    level_idx: usize,
    key_path: &str,
) -> std::collections::HashMap<String, u32> {
    let project = crate::proxy_project::project();
    let model = &project.model;
    let Some(dim_def) = model.dim_def_opt(dim) else {
        return std::collections::HashMap::new();
    };
    let Some(level) = dim_def.levels.get(level_idx) else {
        return std::collections::HashMap::new();
    };
    let Some(next) = dim_def.levels.get(level_idx + 1) else {
        return std::collections::HashMap::new();
    };
    let table = model.dim_table_for_discovery(dim);
    let key_parts: Vec<&str> = key_path.split('|').filter(|s| !s.is_empty()).collect();
    let mut wc = String::new();
    if key_parts.len() == level_idx && level_idx > 0 {
        let conds: Vec<String> = dim_def.levels[..level_idx]
            .iter()
            .zip(key_parts.iter())
            .map(|(l, v)| {
                format!(
                    "CAST({} AS VARCHAR) = '{}'",
                    l.column,
                    v.replace('\'', "''")
                )
            })
            .collect();
        wc = format!(" WHERE {}", conds.join(" AND "));
    }
    let sql = format!(
        "SELECT CAST({} AS VARCHAR), COUNT(DISTINCT {}) FROM {}{} GROUP BY 1",
        level.column, next.column, table, wc
    );
    backend
        .query_grouped_1d(&sql)
        .into_iter()
        .map(|(name, count)| (name, count as u32))
        .collect()
}

/// Number of children of the member at `level_idx` (a `dim.levels` index:
/// 0 = Year, 1 = Quarter, ...) identified by the ancestor key path `key_path`
/// (e.g. `2025` for the year, `2025|3` for the quarter). Returns 0 when the
/// level has no further level beneath it (a leaf).
fn member_child_count<B: QueryBackend + ?Sized>(
    backend: &B,
    dim: &str,
    level_idx: usize,
    key_path: &str,
) -> u32 {
    let project = crate::proxy_project::project();
    let model = &project.model;
    let Some(dim_def) = model.dim_def_opt(dim) else {
        return 0;
    };
    let Some(next) = dim_def.levels.get(level_idx + 1) else {
        return 0;
    };
    let table = model.dim_table_for_discovery(dim);
    let key_parts: Vec<&str> = key_path.split('|').filter(|s| !s.is_empty()).collect();
    let mut wc = String::new();
    if key_parts.len() > level_idx {
        let conds: Vec<String> = dim_def.levels[..=level_idx]
            .iter()
            .zip(key_parts.iter().take(level_idx + 1))
            .map(|(l, v)| {
                format!(
                    "CAST({} AS VARCHAR) = '{}'",
                    l.column,
                    v.replace('\'', "''")
                )
            })
            .collect();
        wc = format!(" WHERE {}", conds.join(" AND "));
    }
    let sql = format!(
        "SELECT COUNT(DISTINCT {}) FROM {}{}",
        next.column, table, wc
    );
    backend.query_count(&sql)
}

pub(crate) fn build_drilldown_multi<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    result: &QueryResult,
    backend: &B,
) -> String {
    let dims = &query.axis_dimensions;
    let mut all_data = match result {
        QueryResult::Pairs(pairs) => pairs.clone(),
        _ => unreachable!(),
    };
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
        if has_exclusions
            && query
                .excluded_members
                .iter()
                .any(|e| e.key == *first || e.key == *second)
        {
            continue;
        }
        let m0 = leaf_member_for(d0, first, &query.dim_props);
        let m1 = leaf_member_for(d1, second, &query.dim_props);
        tuples.push(ordered_pair(dims, d0, m0, d1, m1));
        cells.push(measurement_cell_for_query(query, ordinal, *value));
        ordinal += 1;
    }

    let axis = crate::cellset::AxisConfig {
        name: "Axis0".into(),
        hierarchies,
        tuples,
    };

    render_response(
        vec![axis, full_slicer_axis_with_backend(query, backend)],
        cells,
        &query.cell_props,
    )
}

pub(crate) fn build_drilldown_member<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    result: &QueryResult,
    backend: &B,
) -> String {
    let dims = &query.axis_dimensions;
    let mut all_data = match result {
        QueryResult::Pairs(pairs) => pairs.clone(),
        _ => unreachable!(),
    };
    all_data.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    let d0 = &dims[0];
    let d1 = &dims[1];

    let mut hierarchies: Vec<crate::cellset::HierarchyConfig> = Vec::new();
    for dim in dims {
        hierarchies.push(hierarchy_for(dim, &query.dim_props));
    }

    let excluded_d0: std::collections::HashSet<&str> = query
        .excluded_members
        .iter()
        .filter(|e| e.dimension == *d0)
        .map(|e| e.key.as_str())
        .collect();
    let excluded_d1: std::collections::HashSet<&str> = query
        .excluded_members
        .iter()
        .filter(|e| e.dimension == *d1)
        .map(|e| e.key.as_str())
        .collect();

    let mut col_d0_totals: std::collections::HashMap<String, f64> =
        std::collections::HashMap::new();
    let mut col_d1_totals: std::collections::HashMap<String, f64> =
        std::collections::HashMap::new();
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
                let m1 = all_member_for_with_backend(d1, &query.dim_props, backend);
                tuples.push(ordered_pair(dims, d0, m0, d1, m1));
                cells.push(measurement_cell_for_query(query, ordinal, total));
                ordinal += 1;
            }
            continue;
        }

        if excluded_d1.contains(second.as_str()) {
            if !seen_d1_col.contains(second) {
                seen_d1_col.insert(second.clone());
                let total = col_d1_totals.get(second).copied().unwrap_or(0.0);
                let m0 = all_member_for_with_backend(d0, &query.dim_props, backend);
                let m1 = leaf_member_for(d1, second, &query.dim_props);
                tuples.push(ordered_pair(dims, d0, m0, d1, m1));
                cells.push(measurement_cell_for_query(query, ordinal, total));
                ordinal += 1;
            }
            continue;
        }

        let m0 = leaf_member_for(d0, first, &query.dim_props);
        let m1 = leaf_member_for(d1, second, &query.dim_props);
        tuples.push(ordered_pair(dims, d0, m0, d1, m1));
        cells.push(measurement_cell_for_query(query, ordinal, *value));
        ordinal += 1;
    }

    let axis = crate::cellset::AxisConfig {
        name: "Axis0".into(),
        hierarchies,
        tuples,
    };

    render_response(
        vec![axis, full_slicer_axis_with_backend(query, backend)],
        cells,
        &query.cell_props,
    )
}

pub(crate) fn build_measure_by_category<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    result: &QueryResult,
    backend: &B,
) -> String {
    let dim = row_dim(query);
    let data = match result {
        QueryResult::Grouped(data) => data,
        _ => unreachable!(),
    };
    let axis1_members = leaf_members_from(
        dim,
        &data.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(),
        &query.dim_props,
        query.drilldown_level,
        None,
    );
    let mut cells = Vec::new();
    for (i, (_name, value)) in data.iter().enumerate() {
        cells.push(measurement_cell_for_query(query, i as u32, *value));
    }

    render_response(
        vec![
            measures_axis_for_query(query),
            member_list_axis("Axis1", hierarchy_for(dim, &query.dim_props), axis1_members),
            full_slicer_axis_with_backend(query, backend),
        ],
        cells,
        &query.cell_props,
    )
}

/// Render a multi-measure × dimension cross-join (several measures on Axis0,
/// a dimension on Axis1). Cells are ordered measure-major (Axis0 varies
/// slowest), matching SSAS cell-ordinal convention.
fn build_multi_measure_by_category<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    result: &QueryResult,
    backend: &B,
) -> String {
    let dim = row_dim(query);
    let merged = match result {
        QueryResult::MultiGrouped(rows) => rows.clone(),
        _ => return empty_cellset(query, backend),
    };
    let project = crate::proxy_project::project();

    let mut measure_members = Vec::new();
    let mut measure_ids = Vec::new();
    for name in &query.measures {
        if let Some(m) = project.model.lookup_measure(name) {
            measure_members.push(measures_member(&m.measure_unique_name(), &m.display_name));
            measure_ids.push(m.id.clone());
        }
    }

    let axis1_members = leaf_members_from(
        dim,
        &merged.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(),
        &query.dim_props,
        query.drilldown_level,
        None,
    );

    let n_measures = measure_ids.len();
    let mut cells = Vec::new();
    // SSAS CellOrdinal is row-major: `row * num_columns + column`. Here the
    // columns are the measures (Axis0) and the rows are the dimension members
    // (Axis1), so cells interleave measure values per group.
    for (ci, (_label, values)) in merged.iter().enumerate() {
        for (mi, measure_id) in measure_ids.iter().enumerate() {
            let value = values.get(mi).copied().unwrap_or(0.0);
            let ordinal = (ci * n_measures + mi) as u32;
            cells.push(measurement_cell_for(ordinal, value, measure_id));
        }
    }

    render_response(
        vec![
            member_list_axis("Axis0", measures_hierarchy(), measure_members),
            member_list_axis("Axis1", hierarchy_for(dim, &query.dim_props), axis1_members),
            dims_only_slicer_axis_with_backend(query, backend),
        ],
        cells,
        &query.cell_props,
    )
}

/// Render a multi-measure × two-dimension cross-join (measures on Axis0, a
/// (dim0, dim1) pair on Axis1). Cells are ordered row-major.
fn build_multi_measure_crossjoin<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    result: &QueryResult,
    backend: &B,
) -> String {
    let merged = match result {
        QueryResult::MultiGrouped2(rows) => rows.clone(),
        _ => return empty_cellset(query, backend),
    };
    let project = crate::proxy_project::project();
    let dims = &query.axis_dimensions;

    let mut measure_members = Vec::new();
    let mut measure_ids = Vec::new();
    for name in &query.measures {
        if let Some(m) = project.model.lookup_measure(name) {
            measure_members.push(measures_member(&m.measure_unique_name(), &m.display_name));
            measure_ids.push(m.id.clone());
        }
    }

    let d0 = dims.first().map(|s| s.as_str()).unwrap_or("");
    let d1 = dims.get(1).map(|s| s.as_str()).unwrap_or("");

    let mut hierarchies = Vec::new();
    for dim in dims {
        hierarchies.push(hierarchy_for(dim, &query.dim_props));
    }

    let mut tuples = Vec::new();
    for (a, b, _values) in &merged {
        let m0 = leaf_member_for(d0, a, &query.dim_props);
        let m1 = leaf_member_for(d1, b, &query.dim_props);
        tuples.push(ordered_pair(dims, d0, m0, d1, m1));
    }

    let n_measures = measure_ids.len();
    let mut cells = Vec::new();
    for (pi, (_a, _b, values)) in merged.iter().enumerate() {
        for (mi, measure_id) in measure_ids.iter().enumerate() {
            let value = values.get(mi).copied().unwrap_or(0.0);
            let ordinal = (pi * n_measures + mi) as u32;
            cells.push(measurement_cell_for(ordinal, value, measure_id));
        }
    }

    let axis1 = crate::cellset::AxisConfig {
        name: "Axis1".into(),
        hierarchies,
        tuples,
    };

    render_response(
        vec![
            member_list_axis("Axis0", measures_hierarchy(), measure_members),
            axis1,
            dims_only_slicer_axis_with_backend(query, backend),
        ],
        cells,
        &query.cell_props,
    )
}

pub(crate) fn build_slicer_all_and_measure<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    result: &QueryResult,
    backend: &B,
) -> String {
    let total = match result {
        QueryResult::Scalar(v) => *v,
        _ => unreachable!(),
    };
    render_response(
        vec![full_slicer_axis_with_backend(query, backend)],
        vec![measurement_cell_for_query(query, 0, total)],
        &query.cell_props,
    )
}

pub(crate) fn build_all_level_members<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    result: &QueryResult,
    backend: &B,
) -> String {
    let dim = row_dim(query);
    let total = match result {
        QueryResult::Scalar(v) => *v,
        _ => unreachable!(),
    };
    render_response(
        vec![
            single_member_axis(
                "Axis0",
                hierarchy_for(dim, &query.dim_props),
                all_member_for_with_backend(dim, &query.dim_props, backend),
            ),
            full_slicer_axis_with_backend(query, backend),
        ],
        vec![measurement_cell_for_query(query, 0, total)],
        &query.cell_props,
    )
}

pub(crate) fn build_leaf_level_members<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    result: &QueryResult,
    backend: &B,
) -> String {
    let dim = row_dim(query);
    let data = match result {
        QueryResult::Grouped(data) => data,
        _ => unreachable!(),
    };
    let members = leaf_members_from(
        dim,
        &data.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(),
        &query.dim_props,
        None,
        None,
    );
    let mut cells = Vec::new();
    for (i, (_name, value)) in data.iter().enumerate() {
        cells.push(measurement_cell_for_query(query, i as u32, *value));
    }

    render_response(
        vec![
            member_list_axis("Axis0", hierarchy_for(dim, &query.dim_props), members),
            full_slicer_axis_with_backend(query, backend),
        ],
        cells,
        &query.cell_props,
    )
}

pub(crate) fn build_leaf_children_empty<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    _result: &QueryResult,
    backend: &B,
) -> String {
    let dim = row_dim(query);
    render_response(
        vec![
            empty_member_list_axis("Axis0", hierarchy_for(dim, &query.dim_props)),
            full_slicer_axis_with_backend(query, backend),
        ],
        vec![],
        &query.cell_props,
    )
}

pub(crate) fn build_measure_children_empty<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    _result: &QueryResult,
    backend: &B,
) -> String {
    render_response(
        vec![
            empty_member_list_axis("Axis0", measures_hierarchy()),
            full_slicer_axis_with_backend(query, backend),
        ],
        vec![],
        &query.cell_props,
    )
}

pub(crate) fn build_cchildren_for_all<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    result: &QueryResult,
    backend: &B,
) -> String {
    let dim = row_dim(query);
    let count = match result {
        QueryResult::Count(c) => *c,
        _ => unreachable!(),
    };
    render_response(
        vec![
            single_member_axis(
                "Axis0",
                hierarchy_for(dim, &query.dim_props),
                all_member_for_with_backend(dim, &query.dim_props, backend),
            ),
            single_member_axis("Axis1", measures_hierarchy(), cchildren_member()),
            full_slicer_axis_with_backend(query, backend),
        ],
        vec![count_cell(0, count)],
        &query.cell_props,
    )
}

pub(crate) fn build_cchildren_for_leaf_product<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    name: &str,
    result: &QueryResult,
    backend: &B,
) -> String {
    let dim = row_dim(query);
    let leaf = leaf_member_for(dim, name, &query.dim_props);
    let all = all_member_for_with_backend(dim, &query.dim_props, backend);
    let real_count = match result {
        QueryResult::Count(c) => *c,
        _ => unreachable!(),
    };
    render_response(
        vec![
            member_list_axis(
                "Axis0",
                hierarchy_for(dim, &query.dim_props),
                vec![all, leaf],
            ),
            single_member_axis("Axis1", measures_hierarchy(), cchildren_member()),
            full_slicer_axis_with_backend(query, backend),
        ],
        vec![count_cell(0, real_count), count_cell(1, 0)],
        &query.cell_props,
    )
}

pub(crate) fn build_cchildren_for_measures<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    _result: &QueryResult,
    backend: &B,
) -> String {
    render_response(
        vec![
            single_member_axis("Axis0", measures_hierarchy(), measures_total_member()),
            single_member_axis("Axis1", measures_hierarchy(), cchildren_member()),
            full_slicer_axis_with_backend(query, backend),
        ],
        vec![count_cell(0, 0)],
        &query.cell_props,
    )
}

/// Route a classified query+result to the correct cellset builder.
fn empty_cellset<B: QueryBackend + ?Sized>(query: &SemanticQuery, backend: &B) -> String {
    render_response(
        vec![full_slicer_axis_with_backend(query, backend)],
        vec![],
        &query.cell_props,
    )
}

pub(crate) fn dispatch(query: &SemanticQuery, result: &QueryResult) -> String {
    dispatch_with_backend(query, result, crate::backend::Backend::get())
}

pub(crate) fn dispatch_with_backend<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    result: &QueryResult,
    backend: &B,
) -> String {
    if matches!(result, QueryResult::Empty)
        && !matches!(
            query.kind,
            SemanticQueryKind::MeasureMetadataProbe | SemanticQueryKind::MemberOnlyProbe
        )
    {
        return empty_cellset(query, backend);
    }
    if matches!(result, QueryResult::MultiGrouped(_)) {
        return build_multi_measure_by_category(query, result, backend);
    }
    if matches!(result, QueryResult::MultiGrouped2(_)) {
        return build_multi_measure_crossjoin(query, result, backend);
    }
    match query.kind {
        SemanticQueryKind::ChildrenCountForAll => build_cchildren_for_all(query, result, backend),
        SemanticQueryKind::ChildrenCountLeafProduct => {
            let name = query.cchildren_leaf_name.as_deref().unwrap_or("");
            build_cchildren_for_leaf_product(query, name, result, backend)
        }
        SemanticQueryKind::ChildrenCountMeasures => {
            build_cchildren_for_measures(query, result, backend)
        }
        SemanticQueryKind::SlicerAllAndMeasure => {
            build_slicer_all_and_measure(query, result, backend)
        }
        SemanticQueryKind::MeasureChildrenEmpty => {
            build_measure_children_empty(query, result, backend)
        }
        SemanticQueryKind::LeafChildrenEmpty => build_leaf_children_empty(query, result, backend),
        SemanticQueryKind::AllLevelMembers => build_all_level_members(query, result, backend),
        SemanticQueryKind::LeafLevelMembers => build_leaf_level_members(query, result, backend),
        SemanticQueryKind::MeasureByCategory => build_measure_by_category(query, result, backend),
        SemanticQueryKind::DrilldownCategories => build_drilldown(query, result, backend),
        SemanticQueryKind::SlicerOnly => match result {
            QueryResult::Multi(_) if query.axis_tuples.len() > 1 => {
                build_tuple_set(query, result, backend)
            }
            QueryResult::Multi(_) => build_multi_measure(query, result, backend),
            _ => build_slicer_only(query, result, backend),
        },
        SemanticQueryKind::DrilldownMemberProbe => match result {
            QueryResult::Grouped(_) => build_drilldown(query, result, backend),
            _ => build_drilldown_member(query, result, backend),
        },
        SemanticQueryKind::MeasureMetadataProbe => build_measure_metadata_probe(query, backend),
        SemanticQueryKind::MemberOnlyProbe => build_member_only_probe(query, backend),
    }
}

fn build_member_only_probe<B: QueryBackend + ?Sized>(query: &SemanticQuery, backend: &B) -> String {
    let mut members: Vec<cellset::MemberConfig> = Vec::new();
    let mut hier_name = "[Measures]".to_string();

    for (i, uname) in query.member_only_unames.iter().enumerate() {
        let caption = uname
            .split("&[")
            .nth(1)
            .and_then(|s| s.split(']').next())
            .unwrap_or(uname)
            .to_string();
        // Extract [Dim] and [Hier] by parsing the first two [...] segments.
        let parts: Vec<&str> = uname.splitn(3, ']').collect();
        let dim = parts.first().map(|s| format!("{s}]")).unwrap_or_default();
        let hier = parts
            .get(1)
            .map(|s| s.strip_prefix(".[").unwrap_or(s))
            .unwrap_or("");
        let hier_bracketed = format!("[{hier}]");
        if i == 0 {
            hier_name = format!("{dim}.{hier_bracketed}");
        }
        let lname = format!("{dim}.{hier_bracketed}.{hier_bracketed}");
        members.push(cellset::MemberConfig {
            hierarchy: format!("{dim}.{hier_bracketed}"),
            u_name: xml_escape(uname),
            caption: xml_escape(&caption),
            l_name: xml_escape(&lname),
            l_num: 1,
            display_info: 0,
            children_cardinality: 0,
            dim_props: vec![],
        });
    }

    let axis0 = member_list_axis(
        "Axis0",
        cellset::HierarchyConfig {
            name: hier_name,
            dim_prop_decls: vec![],
        },
        members,
    );
    let slicer = full_slicer_axis_with_backend(query, backend);

    let props = vec!["CELL_ORDINAL".to_string()];
    render_response(vec![axis0, slicer], vec![], &props)
}

fn build_measure_metadata_probe<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    backend: &B,
) -> String {
    let project = crate::proxy_project::project();

    let mut members: Vec<cellset::MemberConfig> = Vec::new();
    let mut cells: Vec<cellset::CellConfig> = Vec::new();
    let mut cell_ordinal: u32 = 0;

    for target in &query.metadata_probe_targets {
        let is_measure = target.starts_with("[Measures]");
        let (unique_name, caption, level_unique) = if is_measure {
            let measure_id = target
                .split("].[")
                .last()
                .map(|s| s.trim_end_matches(']'))
                .unwrap_or(target);
            let m = project.model.measures.iter().find(|m| {
                m.id == measure_id || m.caption == measure_id || m.display_name == measure_id
            });
            let un = m
                .map(|m| m.measure_unique_name())
                .unwrap_or_else(|| format!("[Measures].[{}]", measure_id));
            let cap = m.map(|m| m.display_name.as_str()).unwrap_or(measure_id);
            (
                un,
                cap.to_string(),
                "[Measures].[MeasuresLevel]".to_string(),
            )
        } else {
            let caption = target
                .split("&[")
                .nth(1)
                .and_then(|s| s.split(']').next())
                .unwrap_or("")
                .to_string();
            let level = extract_dim_hierarchy_name(target)
                .unwrap_or_else(|| "[Measures].[MeasuresLevel]".to_string());
            (target.to_string(), caption, level)
        };

        for prop in &query.metadata_probe_properties {
            let val = match prop.as_str() {
                "UniqueName" => unique_name.clone(),
                "caption" => caption.clone(),
                "level.UniqueName" => level_unique.clone(),
                _ => String::new(),
            };
            members.push(cellset::MemberConfig {
                hierarchy: "[Measures]".into(),
                u_name: format!("[Measures].[XL_SD{}]", cell_ordinal),
                caption: format!("XL_SD{}", cell_ordinal),
                l_name: "[Measures].[MeasuresLevel]".into(),
                l_num: 0,
                display_info: if cell_ordinal == 0 { 0 } else { 131072 },
                children_cardinality: 0,
                dim_props: vec![],
            });
            cells.push(cellset::CellConfig {
                ordinal: cell_ordinal,
                value: 0.0,
                fmt_value: String::new(),
                format_string: String::new(),
                back_color: String::new(),
                fore_color: String::new(),
                string_value: Some(xml_escape(&val)),
            });
            cell_ordinal += 1;
        }
    }

    let axis0 = member_list_axis("Axis0", measures_hierarchy(), members);
    let slicer = full_slicer_axis_with_backend(query, backend);

    render_response(vec![axis0, slicer], cells, &query.cell_props)
}

fn extract_dim_hierarchy_name(target: &str) -> Option<String> {
    let rest = target.strip_prefix('[')?;
    let close = rest.find(']')?;
    let dim = &rest[..close];
    let rest = &rest[close + 1..];
    let rest = rest.strip_prefix(".[")?;
    let close = rest.find(']')?;
    let hier = &rest[..close];
    Some(format!("[{}].[{}].[{}]", dim, hier, hier))
}
