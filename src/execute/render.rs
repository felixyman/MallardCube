use crate::axis_members::{
    all_member_for_with_backend, cchildren_member, count_cell, empty_member_list_axis,
    full_slicer_axis_with_backend, hierarchy_for, leaf_member_for, leaf_members_from,
    measurement_cell_for_query, measures_axis_for_query, measures_hierarchy, measures_total_member,
    member_list_axis, render_response, row_dim, single_member_axis,
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
    data.sort_by(|a, b| a.0.cmp(&b.0));
    let parent_uname: Option<String> = query.drilldown_level.and_then(|dl| {
        if dl == 0 {
            return None;
        }
        let project = crate::proxy_project::project();
        let dim_def = project.model.dim_def_opt(dim)?;
        let parent_level = dim_def.levels.get(dl - 1)?;
        let key = query
            .filters
            .iter()
            .find(|f| f.dimension == *dim)
            .and_then(|f| f.members.first())?;
        Some(format!(
            "{}.[{}].&amp;[{}]",
            dim_def.hierarchy_unique_name(),
            parent_level.name,
            key
        ))
    });
    let mut members = leaf_members_from(
        dim,
        &data.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(),
        &query.dim_props,
        query.drilldown_level,
        parent_uname.as_deref(),
    );
    // Prepend the parent member so Excel can establish the
    // parent-child link for multi-level hierarchy expandability.
    if let Some(dl) = query.drilldown_level {
        let parent: Option<cellset::MemberConfig> = if dl == 0 {
            Some(all_member_for_with_backend(dim, &query.dim_props, backend))
        } else {
            let project = crate::proxy_project::project();
            let dim_def = project.model.dim_def_opt(dim);
            let filter_key = query
                .filters
                .iter()
                .find(|f| f.dimension == *dim)
                .and_then(|f| f.members.first());
            let parent_level = dim_def.and_then(|d| d.levels.get(dl - 1));
            if let (Some(def), Some(key), Some(pl)) = (dim_def, filter_key, parent_level) {
                let parent_u = format!(
                    "{}.[{}].&amp;[{}]",
                    def.hierarchy_unique_name(),
                    pl.name,
                    key
                );
                let parent_lname = format!("{}.[{}]", def.hierarchy_unique_name(), pl.name);
                Some(cellset::MemberConfig {
                    hierarchy: def.hierarchy_unique_name(),
                    u_name: parent_u,
                    caption: key.clone(),
                    l_name: parent_lname,
                    l_num: dl as i32,
                    display_info: 0,
                    children_cardinality: 0,
                    dim_props: vec![],
                })
            } else {
                None
            }
        };
        if let Some(p) = parent {
            members.insert(0, p);
        }
    }

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
        && !matches!(query.kind, SemanticQueryKind::MeasureMetadataProbe)
    {
        return empty_cellset(query, backend);
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
        SemanticQueryKind::SlicerOnly => build_slicer_only(query, result, backend),
        SemanticQueryKind::DrilldownMemberProbe => match result {
            QueryResult::Grouped(_) => build_drilldown(query, result, backend),
            _ => build_drilldown_member(query, result, backend),
        },
        SemanticQueryKind::MeasureMetadataProbe => build_measure_metadata_probe(query, backend),
    }
}

fn build_measure_metadata_probe<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    backend: &B,
) -> String {
    let project = crate::proxy_project::project();
    let measure_id = query.metadata_probe_measure.as_deref().unwrap_or("");
    let m =
        project.model.measures.iter().find(|m| {
            m.id == measure_id || m.caption == measure_id || m.display_name == measure_id
        });
    let unique_name = m
        .map(|m| m.measure_unique_name())
        .unwrap_or_else(|| format!("[Measures].[{}]", measure_id));
    let caption = m.map(|m| m.display_name.as_str()).unwrap_or(measure_id);
    let level_unique = "[Measures].[MeasuresLevel]";

    let mut members: Vec<cellset::MemberConfig> = Vec::new();
    let mut cells: Vec<cellset::CellConfig> = Vec::new();

    for (i, prop) in query.metadata_probe_properties.iter().enumerate() {
        let val = match prop.as_str() {
            "UniqueName" => unique_name.clone(),
            "caption" => caption.to_string(),
            "level.UniqueName" => level_unique.to_string(),
            _ => String::new(),
        };
        members.push(cellset::MemberConfig {
            hierarchy: "[Measures]".into(),
            u_name: format!("[Measures].[XL_SD{}]", i),
            caption: val.clone(),
            l_name: "[Measures].[MeasuresLevel]".into(),
            l_num: 0,
            display_info: 0,
            children_cardinality: 0,
            dim_props: vec![],
        });
        cells.push(cellset::CellConfig {
            ordinal: i as u32,
            value: 0.0,
            fmt_value: val.clone(),
            format_string: String::new(),
            back_color: String::new(),
            fore_color: String::new(),
            string_value: Some(val),
        });
    }

    let axis0 = member_list_axis("Axis0", measures_hierarchy(), members);
    let slicer = full_slicer_axis_with_backend(query, backend);

    render_response(vec![axis0, slicer], cells, &query.cell_props)
}
