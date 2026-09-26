use crate::axis_members::{
    all_member_for_with_backend, cchildren_member, count_cell, dims_only_slicer_axis_with_backend,
    empty_member_list_axis, filter_dim_props, full_slicer_axis_with_backend, hierarchy_for,
    leaf_member_for, leaf_member_for_level, leaf_members_from, measurement_cell_for,
    measurement_cell_for_query, measures_hierarchy, measures_member, measures_total_member,
    measures_total_member_for_query, member_list_axis, render_response, row_dim,
    single_member_axis,
};
use crate::backend::QueryBackend;
use crate::cellset;
use crate::engine::plan::QueryResult;
/// Cellset render functions.
///
/// Converts a `SemanticQuery` + `QueryResult` into an XMLA cellset
/// XML string.  Each `build_*` function handles one query shape.
/// `dispatch()` routes by `SemanticQueryKind`.
use crate::mdx_semantic::{SemanticQuery, SemanticQueryKind, includes_prop};
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

/// The requested axis that cross-joins dimensions with measures, if any.
///
/// Excel writes `CrossJoin(<hierarchy>, {[Measures].…})` whenever a field and
/// the Values area sit on the same edge (a field in Columns, or measures in
/// Rows). The response has to keep both on that axis; the historical split
/// (measures on Axis0, dimensions on Axis1) is only correct when the statement
/// really does put them on different axes (plan 049).
fn measure_dim_axis(query: &SemanticQuery) -> Option<&crate::mdx::frontend::AxisSpec> {
    query.axis_specs.iter().find(|s| s.has_both())
}

/// The historical layout: measures alone on their own axis, the dimension on
/// its own axis. `measure_axis` is the ordinal the measures were requested on.
fn split_measure_axes(
    measure_axis: u32,
    measure_members: Vec<cellset::MemberConfig>,
    dim_axis: u32,
    dim_hierarchies: Vec<cellset::HierarchyConfig>,
    dim_tuples: Vec<cellset::TupleConfig>,
    slicer: cellset::AxisConfig,
) -> Vec<cellset::AxisConfig> {
    let measures = cellset::AxisConfig {
        name: format!("Axis{measure_axis}"),
        hierarchies: vec![measures_hierarchy()],
        tuples: dim_tuples_measures(measure_members),
    };
    let dims = cellset::AxisConfig {
        name: format!("Axis{dim_axis}"),
        hierarchies: dim_hierarchies,
        tuples: dim_tuples,
    };
    let (mut axes, second) = if measure_axis <= dim_axis {
        (vec![measures], dims)
    } else {
        (vec![dims], measures)
    };
    axes.push(second);
    axes.push(slicer);
    axes
}

fn dim_tuples_measures(members: Vec<cellset::MemberConfig>) -> Vec<cellset::TupleConfig> {
    members
        .into_iter()
        .map(|member| cellset::TupleConfig {
            members: vec![member],
        })
        .collect()
}

/// Merge the measures onto the dimensions' axis: one tuple per dimension tuple
/// per measure, members in the order the statement wrote them. The cells keep
/// their order (dimension tuple slowest, measure fastest), which is exactly
/// what the callers already produce.
fn merge_measures_into_tuples(
    dim_tuples: Vec<cellset::TupleConfig>,
    measure_members: &[cellset::MemberConfig],
    measures_first: bool,
) -> Vec<cellset::TupleConfig> {
    let mut out = Vec::with_capacity(dim_tuples.len() * measure_members.len());
    for t in &dim_tuples {
        for m in measure_members {
            let mut members = Vec::with_capacity(t.members.len() + 1);
            if measures_first {
                members.push(m.clone());
                members.extend(t.members.iter().cloned());
            } else {
                members.extend(t.members.iter().cloned());
                members.push(m.clone());
            }
            out.push(cellset::TupleConfig { members });
        }
    }
    out
}

/// One axis carrying the dimensions and the measures, as requested.
fn merged_measure_axis(
    ordinal: u32,
    measures_first: bool,
    dim_hierarchies: Vec<cellset::HierarchyConfig>,
    dim_tuples: Vec<cellset::TupleConfig>,
    measure_members: &[cellset::MemberConfig],
) -> cellset::AxisConfig {
    let tuples = merge_measures_into_tuples(dim_tuples, measure_members, measures_first);
    let mut hierarchies = Vec::new();
    if measures_first {
        hierarchies.push(measures_hierarchy());
    }
    hierarchies.extend(dim_hierarchies);
    if !measures_first {
        hierarchies.push(measures_hierarchy());
    }
    cellset::AxisConfig {
        name: format!("Axis{ordinal}"),
        hierarchies,
        tuples,
    }
}

/// Finish a dimension axis: when the statement cross-joined the measures onto
/// the same axis (`CrossJoin(<hierarchy>, {[Measures].…})`, what Excel writes
/// for a field in Columns), the measure member joins every tuple and the slicer
/// drops it; otherwise the measure keeps its place on the slicer (plan 049).
fn finish_dim_axis<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    backend: &B,
    axis: cellset::AxisConfig,
) -> Vec<cellset::AxisConfig> {
    let dim_spec = query.axis_specs.iter().find(|s| !s.dims.is_empty());
    let measure_spec = query
        .axis_specs
        .iter()
        .find(|s| !s.measures.is_empty() && s.measures.len() == 1);
    let measure_member = measure_spec.map(|spec| {
        let project = crate::proxy_project::project();
        let name = spec.measures[0].as_str();
        match project.model.lookup_measure(name) {
            Some(m) => measures_member(&m.measure_unique_name(), &m.display_name),
            None => measures_total_member_for_query(query),
        }
    });
    match (dim_spec, measure_spec, measure_member) {
        // Measures and dimensions on the same edge: one axis, one member per
        // dimension tuple plus the measure member (plan 049).
        (Some(ds), Some(ms), Some(member)) if ds.ordinal == ms.ordinal => {
            let merged = merged_measure_axis(
                ds.ordinal,
                ms.measures_first(),
                axis.hierarchies,
                axis.tuples,
                &[member],
            );
            vec![merged, dims_only_slicer_axis_with_backend(query, backend)]
        }
        // Measures on their own edge (`{[Measures].…} ON COLUMNS`, the
        // dimensions on the other): one axis per requested ordinal.
        (Some(ds), Some(ms), Some(member)) => {
            let dim_axis = cellset::AxisConfig {
                name: format!("Axis{}", ds.ordinal),
                hierarchies: axis.hierarchies,
                tuples: axis.tuples,
            };
            let measure_axis = cellset::AxisConfig {
                name: format!("Axis{}", ms.ordinal),
                hierarchies: vec![measures_hierarchy()],
                tuples: vec![cellset::TupleConfig {
                    members: vec![member],
                }],
            };
            let mut axes = if ds.ordinal <= ms.ordinal {
                vec![dim_axis, measure_axis]
            } else {
                vec![measure_axis, dim_axis]
            };
            axes.push(dims_only_slicer_axis_with_backend(query, backend));
            axes
        }
        _ => vec![axis, full_slicer_axis_with_backend(query, backend)],
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

/// Render a pivot whose statement groups by three or more dimensions spread
/// over two edges — a field in Columns with two nested in Rows, the shape
/// Excel sends for that layout.
///
/// The plan groups by every axis dimension, so a cell is keyed by the full
/// coordinate. This builds one cellset axis per requested edge: `(All)` first,
/// nested parents before their children on a drilled edge, the measures joined
/// where the statement wrote them, and each cell looked up by its coordinate
/// (sparse, like the reference). Plan 049, phase 3.
fn build_multi_dim_pivot<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    result: &QueryResult,
    backend: &B,
) -> String {
    let project = crate::proxy_project::project();
    let specs: Vec<&crate::mdx::frontend::AxisSpec> = query
        .axis_specs
        .iter()
        .filter(|s| !s.dims.is_empty())
        .collect();
    if specs.len() != 2 {
        return empty_cellset(query, backend);
    }
    let (spec0, spec1) = (specs[0], specs[1]);

    let data = match result {
        QueryResult::MultiGroupedN(rows) => rows,
        _ => return empty_cellset(query, backend),
    };
    // Key columns follow `query.axis_dimensions`, which the plan mirrors
    // positionally. Resolve each dimension's column **by name**: the previous
    // arithmetic assumed the specs' dim order matched the plan's, which only
    // holds when the statement lists COLUMNS before ROWS. With `... ON ROWS,
    // CrossJoin(A, B) ON COLUMNS` the plan's columns follow the statement, so
    // every axis got the other edge's values — dimension names with foreign
    // member keys, and most cells missing (plan 051, mirror-measured).
    // Key columns come from the query shape — the same object the plan mirrors
    // for its group-by columns (plan 051).
    let key_of = |dim: &str| -> Option<usize> { query.shape.key_index(dim) };
    // The value of a dimension in a plan row, or None for a missing column.
    let value_of = |keys: &[String], dim: &str| -> Option<String> {
        key_of(dim).and_then(|i| keys.get(i)).cloned()
    };

    let measure_names: Vec<String> = specs
        .iter()
        .flat_map(|s| s.measures.iter().cloned())
        .collect();
    let measure_members: Vec<cellset::MemberConfig> = if measure_names.is_empty() {
        vec![measures_total_member_for_query(query)]
    } else {
        measure_names
            .iter()
            .filter_map(|name| project.model.lookup_measure(name))
            .map(|m| measures_member(&m.measure_unique_name(), &m.display_name))
            .collect()
    };
    let n_measures = measure_members.len().max(1);

    // Dedup through a set, not a linear scan of what we have already kept:
    // with a few thousand distinct values over a few hundred thousand rows the
    // scan was O(rows x distinct) — 12.7 s of a 14 s three-field pivot
    // (plan 051-C cells).
    let distinct = |idx: usize| -> Vec<String> {
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut values: Vec<String> = Vec::new();
        for (keys, _) in data {
            if let Some(k) = keys.get(idx)
                && seen.insert(k.as_str())
            {
                values.push(k.clone());
            }
        }
        values.sort();
        values
    };
    // The value for a coordinate: `None` marks an `(All)` slot, which
    // aggregates the matching rows (the measures the proxy serves are additive).
    // ---- edges -------------------------------------------------------------
    // Each edge contributes its coordinates: one dimension gives (All) +
    // members; a cross-joined pair gives the product with (All) first on each
    // side and the inner member varying fastest (mirror-measured 2026-09-23);
    // a drilled pair nests parents before their children instead. Both edges
    // are built the same way — the old code hardcoded "axis 0 is one
    // dimension", which silently dropped the second dimension of a
    // cross-joined COLUMNS edge.
    let edge_coords = |dims: &[String], nested: bool| -> Vec<Vec<Option<String>>> {
        let values: Vec<Vec<String>> = dims
            .iter()
            .map(|dim| key_of(dim).map(distinct).unwrap_or_default())
            .collect();
        let mut coords: Vec<Vec<Option<String>>> = Vec::new();
        if dims.len() < 2 {
            coords.push(vec![None]);
            if let Some(first) = values.first() {
                for value in first {
                    coords.push(vec![Some(value.clone())]);
                }
            }
            return coords;
        }
        let excluded: std::collections::HashSet<&str> = query
            .excluded_members
            .iter()
            .filter(|e| e.dimension == dims[0] || e.dimension == dims[1])
            .map(|e| e.key.as_str())
            .collect();
        if nested {
            coords.push(vec![None, None]);
            for parent in &values[0] {
                coords.push(vec![Some(parent.clone()), None]);
                if excluded.contains(parent.as_str()) {
                    continue;
                }
                for child in &values[1] {
                    let has_data = data.iter().any(|(keys, _)| {
                        value_of(keys, &dims[0]).as_ref() == Some(parent)
                            && value_of(keys, &dims[1]).as_ref() == Some(child)
                    });
                    if has_data {
                        coords.push(vec![Some(parent.clone()), Some(child.clone())]);
                    }
                }
            }
        } else {
            for parent in std::iter::once(None).chain(values[0].iter().map(|v| Some(v.clone()))) {
                for child in std::iter::once(None).chain(values[1].iter().map(|v| Some(v.clone())))
                {
                    coords.push(vec![parent.clone(), child.clone()]);
                }
            }
        }
        coords
    };
    let nested_edges = query.drilldown_member_hierarchy.is_some();
    let axis0_coords = edge_coords(&spec0.dims, nested_edges && spec0.dims.len() >= 2);
    let axis1_coords = edge_coords(&spec1.dims, nested_edges && spec1.dims.len() >= 2);

    // Coordinate lookups: exact coordinates plus one roll-up per `(All)` mask
    // the cell loop's coordinates actually use. The first version scanned every
    // row per cell — O(cells x rows), which cost 12.7 s on an 84k-cell
    // cross-tab (plan 051). Masks are collected from the same coordinates the
    // cell loop assembles, so every lookup is O(1).
    let mut needed_masks: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for row in &axis1_coords {
        for col in &axis0_coords {
            let coord = cell_coord(&query.shape.flat_dims(), spec0, spec1, col, row);
            let mask = crate::execute::coords::coord_mask(&coord);
            if mask != 0 {
                needed_masks.insert(mask);
            }
        }
    }
    let coord_index = crate::execute::coords::CoordIndex::build(data, n_measures, &needed_masks);
    let value_for =
        |coord: &[Option<&str>], mi: usize| -> Option<f64> { coord_index.get(coord, mi) };

    // ---- tuples ------------------------------------------------------------
    // One member per dimension of the edge, in edge order, plus the measures
    // when the statement cross-joined them onto that edge.
    let edge_tuples = |spec: &crate::mdx::frontend::AxisSpec,
                       coords: &[Vec<Option<String>>]|
     -> Vec<cellset::TupleConfig> {
        let mut tuples = Vec::new();
        for coord in coords {
            let members: Vec<cellset::MemberConfig> = spec
                .dims
                .iter()
                .zip(coord.iter())
                .map(|(dim, value)| member_or_all(dim, value.as_deref(), &query.dim_props, backend))
                .collect();
            if spec.measures.is_empty() {
                tuples.push(cellset::TupleConfig { members });
            } else {
                for m in measure_members.iter().take(n_measures) {
                    let mut with_measure = members.clone();
                    if spec.measures_first() {
                        with_measure.insert(0, m.clone());
                    } else {
                        with_measure.push(m.clone());
                    }
                    tuples.push(cellset::TupleConfig {
                        members: with_measure,
                    });
                }
            }
        }
        tuples
    };
    let axis0_tuples = edge_tuples(spec0, &axis0_coords);
    let axis1_tuples = edge_tuples(spec1, &axis1_coords);

    // Cells, row-major with axis0 fastest. `measure_ids_for` is resolved once:
    // it allocated a Vec per cell in the first version.
    let measure_ids = measure_ids_for(query, &specs, &measure_members);
    let cells_capacity = axis1_coords.len() * axis0_coords.len() * measure_ids.len();
    let mut cells = Vec::with_capacity(cells_capacity.min(1_000_000));
    let mut ordinal = 0u32;
    for row in &axis1_coords {
        for col in &axis0_coords {
            let coord = cell_coord(&query.axis_dimensions, spec0, spec1, col, row);
            for (mi, measure_id) in measure_ids.iter().enumerate() {
                if let Some(value) = value_for(&coord, mi) {
                    cells.push(measurement_cell_for(ordinal, value, measure_id));
                }
                ordinal += 1;
            }
        }
    }

    // Every dimension of the edge appears in its hierarchy list, in edge order,
    // with the measures where the statement put them.
    let edge_hierarchies =
        |spec: &crate::mdx::frontend::AxisSpec| -> Vec<cellset::HierarchyConfig> {
            let mut hierarchies = Vec::new();
            if spec.measures_first() {
                hierarchies.push(measures_hierarchy());
            }
            for dim in &spec.dims {
                hierarchies.push(hierarchy_for(dim, &query.dim_props));
            }
            if !spec.measures.is_empty() && !spec.measures_first() {
                hierarchies.push(measures_hierarchy());
            }
            hierarchies
        };
    let hierarchies0 = edge_hierarchies(spec0);
    let hierarchies1 = edge_hierarchies(spec1);
    let axis0 = cellset::AxisConfig {
        name: format!("Axis{}", spec0.ordinal),
        hierarchies: hierarchies0,
        tuples: axis0_tuples,
    };
    let axis1 = cellset::AxisConfig {
        name: format!("Axis{}", spec1.ordinal),
        hierarchies: hierarchies1,
        tuples: axis1_tuples,
    };
    let (mut axes, second) = if spec0.ordinal <= spec1.ordinal {
        (vec![axis0], axis1)
    } else {
        (vec![axis1], axis0)
    };
    axes.push(second);
    axes.push(full_slicer_axis_with_backend(query, backend));
    render_response(axes, cells, &query.cell_props)
}

fn member_or_all<B: QueryBackend + ?Sized>(
    dim: &str,
    value: Option<&str>,
    dim_props: &[String],
    backend: &B,
) -> cellset::MemberConfig {
    match value {
        Some(v) => leaf_members_from(dim, &[v.to_string()], dim_props, None, None).remove(0),
        None => all_member_for_with_backend(dim, dim_props, backend),
    }
}

/// Render a cross-tab: the statement put dimensions on two different axes (a
/// field in Columns and another in Rows — Excel's shape for a two-axis pivot).
///
/// The plan already returns the (dim0, dim1) cross-tab, so this splits it back
/// into one cellset axis per requested edge: each dimension contributes its
/// `(All)` member first (Excel's Grand Total) followed by its members, the
/// measures join the axis they were written on, and the cells are ordered
/// row-major with the first axis varying fastest, as SSAS returns them.
/// Without this the renderers collapsed both dimensions and the measures onto
/// one axis (plan 049).
fn build_cross_tab<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    result: &QueryResult,
    backend: &B,
) -> String {
    let project = crate::proxy_project::project();
    let specs: Vec<&crate::mdx::frontend::AxisSpec> = query
        .axis_specs
        .iter()
        .filter(|s| !s.dims.is_empty())
        .collect();
    if specs.len() < 2 {
        return empty_cellset(query, backend);
    }
    let (spec0, spec1) = (specs[0], specs[1]);
    let d0 = spec0.dims[0].clone();
    let d1 = spec1.dims[0].clone();

    // Data: one entry per (dim0, dim1) pair; `Vec<f64>` holds one value per
    // measure (a single entry when the measure came from the slicer).
    let rows: Vec<(String, String, Vec<f64>)> = match result {
        QueryResult::Pairs(pairs) => pairs
            .iter()
            .map(|(a, b, v)| (a.clone(), b.clone(), vec![*v]))
            .collect(),
        QueryResult::MultiGrouped2(pairs) => pairs.clone(),
        _ => return empty_cellset(query, backend),
    };
    let mut rows = rows;
    rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    // Measure members: the ones the statement put on an axis, else the slicer's.
    let measure_names: Vec<String> = specs
        .iter()
        .flat_map(|s| s.measures.iter().cloned())
        .collect();
    let measure_members: Vec<cellset::MemberConfig> = if measure_names.is_empty() {
        vec![measures_total_member_for_query(query)]
    } else {
        measure_names
            .iter()
            .filter_map(|name| project.model.lookup_measure(name))
            .map(|m| measures_member(&m.measure_unique_name(), &m.display_name))
            .collect()
    };
    let n_measures = measure_members.len().max(1);

    // Members in the dimension's own order (the labels arrive grouped by the
    // other axis, so dedupe then sort, as the single-axis renderers do).
    let mut dim0_values: Vec<String> = Vec::new();
    let mut dim1_values: Vec<String> = Vec::new();
    for (a, b, _) in &rows {
        if !dim0_values.iter().any(|v| v == a) {
            dim0_values.push(a.clone());
        }
        if !dim1_values.iter().any(|v| v == b) {
            dim1_values.push(b.clone());
        }
    }
    dim0_values.sort();
    dim1_values.sort();

    let member_for = |dim: &str, value: &str| -> cellset::MemberConfig {
        leaf_members_from(
            dim,
            std::slice::from_ref(&value.to_string()),
            &query.dim_props,
            query.drilldown_level(),
            None,
        )
        .remove(0)
    };
    let all_for = |dim: &str| -> cellset::MemberConfig {
        all_member_for_with_backend(dim, &query.dim_props, backend)
    };

    // Axis 0: (All) + every dim0 member, with the measures when they belong here.
    let mut axis0_members = vec![all_for(&d0)];
    axis0_members.extend(dim0_values.iter().map(|v| member_for(&d0, v)));
    let axis1_members = {
        let mut m = vec![all_for(&d1)];
        m.extend(dim1_values.iter().map(|v| member_for(&d1, v)));
        m
    };

    // Lookup tables built once. The first version scanned every result row per
    // cell (`rows.iter().find(..)` plus a per-cell `collect` for the totals),
    // which is O(cells x rows): an 84k-cell cross-tab spent 12.7 s rendering
    // (plan 051-C cells). Maps make the cell loop O(cells).
    let measure_count = measure_ids_for(query, &specs, &measure_members).len();
    let mut values_by_pair: std::collections::HashMap<(&str, &str), &[f64]> =
        std::collections::HashMap::with_capacity(rows.len());
    let mut total_a: std::collections::HashMap<&str, Vec<f64>> = std::collections::HashMap::new();
    let mut total_b: std::collections::HashMap<&str, Vec<f64>> = std::collections::HashMap::new();
    let mut grand_total = vec![0.0f64; measure_count];
    for (ra, rb, values) in &rows {
        values_by_pair.insert((ra.as_str(), rb.as_str()), values.as_slice());
        for (mi, value) in values.iter().enumerate().take(measure_count) {
            grand_total[mi] += *value;
            let a_bucket = total_a.entry(ra.as_str()).or_default();
            if a_bucket.len() < measure_count {
                a_bucket.resize(measure_count, 0.0);
            }
            a_bucket[mi] += *value;
            let b_bucket = total_b.entry(rb.as_str()).or_default();
            if b_bucket.len() < measure_count {
                b_bucket.resize(measure_count, 0.0);
            }
            b_bucket[mi] += *value;
        }
    }
    // (All, b) sums over dim0, (a, All) sums over dim1, (All, All) is the
    // total. `None` means the combination has no data: the reference omits
    // those cells (sparse cell data) instead of sending a zero, so Excel shows
    // a blank rather than 0.
    let cell_value = |a: Option<&str>, b: Option<&str>, mi: usize| -> Option<f64> {
        match (a, b) {
            (Some(a), Some(b)) => values_by_pair
                .get(&(a, b))
                .and_then(|values| values.get(mi).copied()),
            (Some(a), None) => total_a
                .get(a)
                .map(|values| values.get(mi).copied().unwrap_or(0.0)),
            (None, Some(b)) => total_b
                .get(b)
                .map(|values| values.get(mi).copied().unwrap_or(0.0)),
            (None, None) => (!rows.is_empty()).then(|| grand_total[mi]),
        }
    };

    // Tuples per axis: dimension member (and measure, when cross-joined).
    let axis0_tuples = build_axis_tuples(&axis0_members, spec0, &measure_members, n_measures);
    let axis1_tuples = build_axis_tuples(&axis1_members, spec1, &measure_members, n_measures);

    // Cells, row-major with axis0 fastest. `measure_ids_for` is resolved once:
    // it allocated a Vec per cell in the first version.
    let measure_ids = measure_ids_for(query, &specs, &measure_members);
    let cells_capacity = axis1_members.len() * axis0_members.len() * measure_ids.len();
    let mut cells = Vec::with_capacity(cells_capacity.min(1_000_000));
    let mut ordinal = 0u32;
    for (row_idx, _) in axis1_members.iter().enumerate() {
        for (col_idx, _) in axis0_members.iter().enumerate() {
            let a = (col_idx > 0).then(|| dim0_values[col_idx - 1].as_str());
            let b = (row_idx > 0).then(|| dim1_values[row_idx - 1].as_str());
            for (mi, measure_id) in measure_ids.iter().enumerate() {
                if let Some(value) = cell_value(a, b, mi) {
                    cells.push(measurement_cell_for(ordinal, value, measure_id));
                }
                ordinal += 1;
            }
        }
    }

    let hierarchies0 = axis_hierarchies(&d0, spec0, &query.dim_props);
    let hierarchies1 = axis_hierarchies(&d1, spec1, &query.dim_props);
    let axis0 = cellset::AxisConfig {
        name: format!("Axis{}", spec0.ordinal),
        hierarchies: hierarchies0,
        tuples: axis0_tuples,
    };
    let axis1 = cellset::AxisConfig {
        name: format!("Axis{}", spec1.ordinal),
        hierarchies: hierarchies1,
        tuples: axis1_tuples,
    };
    let (mut axes, second) = if spec0.ordinal <= spec1.ordinal {
        (vec![axis0], axis1)
    } else {
        (vec![axis1], axis0)
    };
    axes.push(second);
    axes.push(full_slicer_axis_with_backend(query, backend));
    render_response(axes, cells, &query.cell_props)
}

/// A cell's coordinate in the plan's key order: each dimension of each edge
/// contributes its slot's value (`None` = the `(All)` member). The old code
/// assumed one dimension per edge and a fixed column order, which cross-wired
/// the dimensions whenever the statement listed ROWS before COLUMNS (plan 051).
fn cell_coord<'a>(
    dims: &[String],
    spec0: &crate::mdx::frontend::AxisSpec,
    spec1: &crate::mdx::frontend::AxisSpec,
    axis0: &'a [Option<String>],
    axis1: &'a [Option<String>],
) -> Vec<Option<&'a str>> {
    dims.iter()
        .map(|dim| {
            spec0
                .dims
                .iter()
                .position(|d| d == dim)
                .and_then(|pos| axis0.get(pos))
                .or_else(|| {
                    spec1
                        .dims
                        .iter()
                        .position(|d| d == dim)
                        .and_then(|pos| axis1.get(pos))
                })
                .map(|value| value.as_deref())
                .unwrap_or_else(|| {
                    debug_assert!(false, "axis dimension {dim} is on neither edge");
                    eprintln!("!!! render: axis dimension {dim} is on neither edge");
                    None
                })
        })
        .collect()
}

/// The measure ids a cross-tab cell carries, in axis order.
fn measure_ids_for(
    query: &SemanticQuery,
    specs: &[&crate::mdx::frontend::AxisSpec],
    measure_members: &[cellset::MemberConfig],
) -> Vec<String> {
    let project = crate::proxy_project::project();
    let mut ids: Vec<String> = specs
        .iter()
        .flat_map(|s| s.measures.iter())
        .filter_map(|name| project.model.lookup_measure(name).map(|m| m.id.clone()))
        .collect();
    if ids.is_empty() {
        ids.push(crate::axis_members::measure_id_for_query(query));
    }
    let _ = measure_members;
    ids
}

/// The hierarchies of one cross-tab axis: its dimension, plus the measures when
/// the statement cross-joined them onto this edge.
fn axis_hierarchies(
    dim: &str,
    spec: &crate::mdx::frontend::AxisSpec,
    dim_props: &[String],
) -> Vec<cellset::HierarchyConfig> {
    let mut hierarchies = Vec::new();
    if spec.measures_first() {
        hierarchies.push(measures_hierarchy());
    }
    hierarchies.push(hierarchy_for(dim, dim_props));
    if !spec.measures.is_empty() && !spec.measures_first() {
        hierarchies.push(measures_hierarchy());
    }
    hierarchies
}

/// Cross-product of the axis members and the measure members (dimension member
/// slowest, measure fastest — the order the reference uses).
fn build_axis_tuples(
    dim_members: &[cellset::MemberConfig],
    spec: &crate::mdx::frontend::AxisSpec,
    measure_members: &[cellset::MemberConfig],
    n_measures: usize,
) -> Vec<cellset::TupleConfig> {
    let mut tuples = Vec::new();
    for member in dim_members {
        if spec.measures.is_empty() {
            tuples.push(cellset::TupleConfig {
                members: vec![member.clone()],
            });
            continue;
        }
        for m in measure_members.iter().take(n_measures) {
            let mut members = Vec::new();
            if spec.measures_first() {
                members.push(m.clone());
                members.push(member.clone());
            } else {
                members.push(member.clone());
                members.push(m.clone());
            }
            tuples.push(cellset::TupleConfig { members });
        }
    }
    tuples
}

/// Is the axis carrying `dim` marked `NON EMPTY`? Defaults to true — the
/// data-driven behaviour — when the axis cannot be found.
fn axis_non_empty(query: &SemanticQuery, dim: &str) -> bool {
    query
        .axis_specs
        .iter()
        .find(|spec| spec.dims.iter().any(|d| d == dim))
        .map(|spec| spec.non_empty)
        .unwrap_or(true)
}

/// The no-`NON EMPTY` drilldown: every member the dictionary knows, in its
/// order, with a cell only where the tuple has data.
fn build_drilldown_dictionary<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    result: &QueryResult,
    backend: &B,
) -> String {
    let project = crate::proxy_project::project();
    let model = &project.model;
    let dim = query
        .axis_dimensions
        .first()
        .map(|s| s.as_str())
        .unwrap_or_default();
    let Some(def) = model.dim_def_opt(dim) else {
        return render_response(
            finish_dim_axis(
                query,
                backend,
                empty_member_list_axis("Axis0", hierarchy_for(dim, &query.dim_props)),
            ),
            Vec::new(),
            &query.cell_props,
        );
    };
    let values: Vec<(String, f64)> = match result {
        QueryResult::Grouped(rows) => rows.clone(),
        _ => Vec::new(),
    };
    let members = model.dim_cache.get(model, def, backend);
    let key_view = query.key_hierarchy_view.is_some();
    let names: Vec<String> = if key_view {
        // The key-attribute level's paths carry their ancestors; the view it
        // renders as is single-level, so its member keys are the last segment
        // (the date itself) — `apply_key_hierarchy_view` rewrites the namespace
        // and prepends its (All) below.
        members
            .level_paths
            .last()
            .map(|paths| {
                paths
                    .iter()
                    .filter_map(|parts| parts.last().cloned())
                    .collect()
            })
            .unwrap_or_else(|| members.leaf_values.clone())
    } else {
        // The grain the axis asked for: a level set or drilldown names its level.
        let level_index = query.drilldown_level().unwrap_or(0);
        match members.level_paths.get(level_index) {
            Some(paths) if !paths.is_empty() => paths.iter().map(|parts| parts.join("|")).collect(),
            _ => members.leaf_values.clone(),
        }
    };

    // A drilldown's input set includes (All); a level set does not. The
    // key-attribute view gets its own (All) from `apply_key_hierarchy_view`.
    let mut axis_members = Vec::new();
    if !query.level_drag && !key_view {
        axis_members.push(all_member_for_with_backend(dim, &query.dim_props, backend));
    }
    axis_members.extend(leaf_members_from(
        dim,
        &names,
        &query.dim_props,
        query.drilldown_level(),
        None,
    ));
    if key_view {
        crate::axis_members::apply_key_hierarchy_view(
            &mut axis_members,
            def,
            &query.dim_props,
            backend,
        );
    }
    apply_member_display_info(&mut axis_members);

    // Sparse cells: the ordinal indexes the full member list, and tuples
    // without data simply have none (the reference's shape).
    let total: f64 = values.iter().map(|(_, value)| *value).sum();
    let mut cells: Vec<crate::cellset::CellConfig> = Vec::new();
    for (index, member) in axis_members.iter().enumerate() {
        let key = crate::axis_members::key_from_member_uname(&member.u_name).unwrap_or_default();
        let value = if member.u_name.ends_with(".[All]") {
            Some(total)
        } else {
            values
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| *value)
        };
        if let Some(value) = value {
            cells.push(measurement_cell_for_query(query, index as u32, value));
        }
    }

    let axis = member_list_axis("Axis0", hierarchy_for(dim, &query.dim_props), axis_members);
    render_response(
        finish_dim_axis(query, backend, axis),
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
    // Without NON EMPTY the axis lists the level's whole dictionary — the
    // reference answers 4,019 members for the date key hierarchy — while only
    // the tuples with data carry cells (its CellData is sparse: 758 cells for
    // 4,019 members, the ordinals indexing the full list; measured 2026-09-26).
    // Excel always sends NON EMPTY, so this exists for the other XMLA clients.
    if query.axis_set_op.is_none()
        && !axis_non_empty(query, dims.first().map(|s| s.as_str()).unwrap_or(""))
    {
        return build_drilldown_dictionary(query, result, backend);
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
    let filter_keys: Vec<String> = query
        .filters
        .iter()
        .find(|f| f.dimension == *dim)
        .map(|f| f.members.clone())
        .unwrap_or_default();
    // Path labels (`2026|1|1`) already carry their ancestors, so the renderer
    // must not prefix a parent key on top of them.
    let labels_are_paths = data.iter().any(|(n, _)| n.contains('|'));
    // Only a single expanded parent lets the renderer prefix the parent key;
    // with several, the SQL emits full ancestor paths per member.
    let single_parent = filter_keys.len() == 1 && !labels_are_paths;
    let parent_uname: Option<String> = query.drilldown_level().and_then(|dl| {
        if dl == 0 || query.level_drag || !single_parent {
            return None;
        }
        let project = crate::proxy_project::project();
        let dim_def = project.model.dim_def_opt(dim)?;
        let key = filter_keys.first()?;
        Some(level_member_uname(dim_def, dl - 1, key))
    });
    let mut members = leaf_members_from(
        dim,
        &data.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(),
        &query.dim_props,
        query.drilldown_level(),
        parent_uname.as_deref(),
    );
    // Multi-parent expansion ("Expand Entire Field"): each child's label is a
    // path (`2026|1`), so wire its PARENT_UNIQUE_NAME from that prefix. Level
    // drags stay flat — their parents are intentionally not on the axis.
    if !query.level_drag {
        attach_parent_keys(&mut members, dim, query.drilldown_level());
    }
    // Report the real per-member child count so Excel's expand indicators stay
    // consistent (a year has 4 quarters, a quarter has 3 months, a month has its
    // day count). Without this, Excel shows a missing "+" on years and a
    // mismatched expand state on quarters.
    if let Some(dl) = query.drilldown_level() {
        let key_path = filter_keys.first().cloned().unwrap_or_default();
        let cc_map = drill_children_cardinalities(backend, dim, dl, &key_path);
        for m in &mut members {
            // Path-labelled members aren't in the plain-value map; compute
            // their real count from the key path (a month shows ~30 day
            // children, not the static whole-level cardinality).
            let key = crate::axis_members::key_from_member_uname(&m.u_name);
            let cc = match key.as_deref() {
                Some(k) if k.contains('|') => member_child_count(backend, dim, dl, k),
                _ => match cc_map.get(&m.caption) {
                    Some(cc) => *cc,
                    None => continue,
                },
            };
            m.children_cardinality = cc;
            m.display_info = if cc > 0 { 131075 } else { 3 };
        }
    }
    // Whole-field / deep drills: emit the hierarchy in pre-order (parent
    // immediately before its children) so Excel's DRILLED_DOWN bookkeeping
    // stays consistent. Single-parent plain-value drills keep the chain path
    // below.
    if let Some(dl) = query.drilldown_level()
        && dl > 0
        && !query.level_drag
        && (filter_keys.is_empty() || labels_are_paths)
    {
        let labels: Vec<String> = data.iter().map(|(n, _)| n.clone()).collect();
        // SSAS returns the whole `DrilldownLevel({All})` input set (all level-0
        // members) alongside the expanded branch; include it.
        let extra_roots = level0_member_values(query, dim, backend);
        let tree = preorder_drill_members(query, dim, &labels, dl, &extra_roots, backend);
        let total: f64 = data.iter().map(|(_, v)| *v).sum();
        let mut members: Vec<cellset::MemberConfig> = Vec::new();
        let mut cells: Vec<crate::cellset::CellConfig> = Vec::new();
        for (member, data_idx) in tree {
            let value = match data_idx {
                Some(i) => data[i].1,
                // Ancestors aggregate their own subtree, not the whole branch.
                None => {
                    let key = crate::axis_members::key_from_member_uname(&member.u_name)
                        .unwrap_or_default();
                    if let Some((_, v)) = extra_roots.iter().find(|(k, _)| *k == key) {
                        // A root of the input set carries its own aggregate.
                        *v
                    } else if key.is_empty() {
                        total
                    } else {
                        let prefix = format!("{key}|");
                        data.iter()
                            .filter(|(n, _)| *n == key || n.starts_with(&prefix))
                            .map(|(_, v)| *v)
                            .sum()
                    }
                }
            };
            cells.push(measurement_cell_for_query(
                query,
                members.len() as u32,
                value,
            ));
            members.push(member);
        }
        apply_member_display_info(&mut members);
        let axis = member_list_axis("Axis0", hierarchy_for(dim, &query.dim_props), members);
        return render_response(
            finish_dim_axis(query, backend, axis),
            cells,
            &query.cell_props,
        );
    }

    // Prepend the full ancestor chain so every member's parent is either on the
    // axis or is the (All) root. Real SSAS DrilldownMember returns the whole
    // input set — `DrilldownLevel({All})` = (All) plus every level-0 member —
    // followed by the drilled branch's intermediate levels and its children.
    let mut prefix: Vec<(cellset::MemberConfig, f64)> = Vec::new();
    let total: f64 = data.iter().map(|(_, v)| *v).sum();
    if let Some(dl) = query.drilldown_level()
        && !query.level_drag
        && dl > 0
        && filter_keys.len() == 1
        && !labels_are_paths
    {
        let project = crate::proxy_project::project();
        if let Some(def) = project.model.dim_def_opt(dim) {
            let key = &filter_keys[0];
            let branch_year = key.split('|').next().unwrap_or("").to_string();
            // Level-0 members with their own aggregates (the input set).
            let mut roots = level0_member_values(query, dim, backend);
            roots.sort_by(|a, b| cmp_key_paths(&a.0, &b.0));
            prefix.push((
                all_member_for_with_backend(dim, &query.dim_props, backend),
                total,
            ));
            for (root_key, value) in &roots {
                if root_key.split('|').count() != 1 {
                    continue;
                }
                let is_branch = *root_key == branch_year;
                let u_name = level_member_uname(def, 0, root_key);
                let cc = member_child_count(backend, dim, 0, root_key);
                prefix.push((
                    cellset::MemberConfig {
                        hierarchy: def.hierarchy_unique_name(),
                        u_name: u_name.clone(),
                        caption: root_key.clone(),
                        l_name: format!("{}.[{}]", def.hierarchy_unique_name(), def.levels[0].name),
                        l_num: 1,
                        display_info: 0,
                        children_cardinality: cc,
                        dim_props: walk_dim_props(
                            def,
                            root_key,
                            &u_name,
                            &format!("{}.[{}]", def.hierarchy_unique_name(), def.levels[0].name),
                            root_key,
                            &def.all_member_unique_name(),
                            0,
                            &query.dim_props,
                        ),
                    },
                    *value,
                ));
                if !is_branch {
                    continue;
                }
                // The drilled branch: intermediate levels, then the children.
                let parent_uname = Some(level_member_uname(def, dl - 1, key));
                for m in ancestor_members(query, dim, def, key, dl, backend)
                    .into_iter()
                    .skip(1)
                    .filter(|m| m.l_num != 1)
                {
                    prefix.push((m, total));
                }
                for (name, value) in &data {
                    let mut m = leaf_members_from(
                        dim,
                        std::slice::from_ref(name),
                        &query.dim_props,
                        Some(dl),
                        parent_uname.as_deref(),
                    )
                    .remove(0);
                    // Real per-member child count: Excel's hierarchy walk reads
                    // the low bits of DisplayInfo as the child count, so the
                    // static whole-level value (e.g. 132 months for a quarter)
                    // corrupts its tree and the expand never renders.
                    m.children_cardinality =
                        member_child_count(backend, dim, dl, &format!("{key}|{name}"));
                    prefix.push((m, *value));
                }
            }
            // Fallback: the branch year missing from the roots (empty dim table).
            if !prefix.iter().any(|(m, _)| m.l_num == 1)
                || !prefix
                    .iter()
                    .any(|(m, _)| m.u_name == level_member_uname(def, 0, &branch_year))
            {
                for m in ancestor_members(query, dim, def, key, dl, backend) {
                    prefix.push((m, total));
                }
                for (name, value) in &data {
                    let mut m = leaf_members_from(
                        dim,
                        std::slice::from_ref(name),
                        &query.dim_props,
                        Some(dl),
                        None,
                    )
                    .remove(0);
                    m.children_cardinality =
                        member_child_count(backend, dim, dl, &format!("{key}|{name}"));
                    prefix.push((m, *value));
                }
            }
        }
    }

    if !prefix.is_empty() {
        let mut ms: Vec<cellset::MemberConfig> = Vec::new();
        let mut cs: Vec<crate::cellset::CellConfig> = Vec::new();
        for (m, v) in prefix {
            cs.push(measurement_cell_for_query(query, ms.len() as u32, v));
            ms.push(m);
        }
        apply_member_display_info(&mut ms);
        let axis = member_list_axis("Axis0", hierarchy_for(dim, &query.dim_props), ms);
        return render_response(finish_dim_axis(query, backend, axis), cs, &query.cell_props);
    }

    // Legacy chain prepend for shapes the prefix path doesn't cover.
    let mut num_ancestors: u32 = 0;
    if let Some(dl) = query.drilldown_level()
        && !query.level_drag
    {
        let project = crate::proxy_project::project();
        if let Some(def) = project.model.dim_def_opt(dim) {
            let mut ancestors: Vec<cellset::MemberConfig> = Vec::new();
            if filter_keys.is_empty() || labels_are_paths {
                let labels: Vec<String> = data.iter().map(|(n, _)| n.clone()).collect();
                ancestors = ancestor_members_from_labels(query, dim, def, &labels, dl, backend);
            } else {
                for (i, key) in filter_keys.iter().enumerate() {
                    let chain = ancestor_members(query, dim, def, key, dl, backend);
                    if i == 0 {
                        ancestors = chain;
                    } else {
                        ancestors.extend(chain.into_iter().skip(1));
                    }
                }
            }
            num_ancestors = ancestors.len() as u32;
            for a in ancestors.into_iter().rev() {
                members.insert(0, a);
            }
        }
    }

    // A top-level drag (`DrilldownLevel({All})`) returns the (All) member
    // first, as the reference does; Excel reads it as the Grand Total row or
    // column. The key-hierarchy view adds its own (All) below.
    // A set-op axis (TopCount/Order/Filter) keeps it too: the reference's
    // `(All)` for such an axis aggregates the *returned* subset (verified
    // against the mirror for Excel's Top-5 subselect), which is exactly what
    // summing the data here produces.
    if matches!(query.drilldown_level(), None | Some(0))
        && !query.level_drag
        && query.key_hierarchy_view.is_none()
        && members
            .first()
            .is_none_or(|m| !m.u_name.ends_with(".[All]") && !m.u_name.ends_with(".[(All)]"))
    {
        members.insert(
            0,
            all_member_for_with_backend(dim, &query.dim_props, backend),
        );
        num_ancestors += 1;
    }

    // The key attribute hierarchy renders as its own single-level hierarchy:
    // (All) plus the date members. Excel places axis members by the field's
    // hierarchy and level numbers, so the user hierarchy's namespace
    // (`[Date].[Calendar].[Full Date]`, level 4) left the field empty (plan 048).
    let key_view = query.key_hierarchy_view.as_deref().filter(|v| {
        crate::proxy_project::project()
            .model
            .dim_def_opt(dim)
            .and_then(|d| d.key_hierarchy_unique_name())
            .as_deref()
            == Some(*v)
    });
    if key_view.is_some()
        && let Some(def) = crate::proxy_project::project().model.dim_def_opt(dim)
    {
        crate::axis_members::apply_key_hierarchy_view(&mut members, def, &query.dim_props, backend);
        // The (All) root is an ancestor: it carries the branch total and the
        // drilled-down display flag.
        num_ancestors = 1;
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

    let axis = member_list_axis(
        "Axis0",
        crate::axis_members::hierarchy_for_view(dim, &query.dim_props, key_view),
        members,
    );
    render_response(
        finish_dim_axis(query, backend, axis),
        cells,
        &query.cell_props,
    )
}

/// Fill in `PARENT_UNIQUE_NAME` for members whose label is a compound path
/// (`2026|1`) — used when several parents are expanded at once, where the
/// renderer can't prefix a single parent key. Replaces the default `(All)`
/// parent that the member builder emits for path members. No-op when the path
/// has no ancestor segments.
fn attach_parent_keys(
    members: &mut [crate::cellset::MemberConfig],
    dim: &str,
    level: Option<usize>,
) {
    let Some(dl) = level else { return };
    // Level-0 members sit directly under (All); no parent uname is needed
    // (and `dl - 1` would underflow).
    if dl == 0 {
        return;
    }
    let Some(def) = crate::proxy_project::project().model.dim_def_opt(dim) else {
        return;
    };
    for m in members.iter_mut() {
        let key = crate::axis_members::key_from_member_uname(&m.u_name)
            .or_else(|| {
                m.dim_props
                    .iter()
                    .find(|(k, _)| k == "MEMBER_KEY")
                    .map(|(_, v)| v.clone())
            })
            .unwrap_or_default();
        let parts: Vec<&str> = key.split('|').collect();
        if parts.len() <= dl {
            continue;
        }
        let parent = level_member_uname(def, dl - 1, &parts[..dl].join("|"));
        match m
            .dim_props
            .iter_mut()
            .find(|(k, _)| k == "PARENT_UNIQUE_NAME")
        {
            Some(slot) => slot.1 = parent,
            None => m.dim_props.push(("PARENT_UNIQUE_NAME".into(), parent)),
        }
    }
}

/// Numeric-aware ordering for pipe-joined key paths: months sort 1..12 rather
/// than "1","10","11","12","2". Shorter paths (ancestors) come first.
fn cmp_key_paths(a: &str, b: &str) -> std::cmp::Ordering {
    let (pa, pb): (Vec<&str>, Vec<&str>) = (a.split('|').collect(), b.split('|').collect());
    for (x, y) in pa.iter().zip(pb.iter()) {
        let ord = match (x.parse::<i64>(), y.parse::<i64>()) {
            (Ok(nx), Ok(ny)) => nx.cmp(&ny),
            _ => x.cmp(y),
        };
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
    }
    pa.len().cmp(&pb.len())
}

/// Level-0 member values for a hierarchy: the `DrilldownLevel({All})` input
/// set that SSAS includes alongside every expanded branch. Values respect the
/// query's filters (slicers, subselects, time flags) because the level-0
/// grouping runs through the normal plan executor.
fn level0_member_values<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    dim: &str,
    backend: &B,
) -> Vec<(String, f64)> {
    use crate::engine::plan::{
        QueryPlan, QueryResult, execute_plan_with_backend, filters_with_time_flag, typed_filters,
    };
    let project = crate::proxy_project::project();
    let model = &project.model;
    if model.dim_def_opt(dim).is_none_or(|d| d.levels.is_empty()) {
        return Vec::new();
    }
    // Requests without a measure (e.g. a pivot with only the hierarchy on an
    // axis) still need the input set; fall back to the model's default measure.
    let meas = query
        .measures
        .first()
        .or(query.measure.as_ref())
        .and_then(|name| model.lookup_measure(name))
        .or_else(|| model.default_measure_id().map(|id| model.meas_def(&id)));
    let Some(meas) = meas else {
        return Vec::new();
    };
    // The drill's own member filter must not constrain the input set; real
    // slicers on the same dimension are kept.
    let is_drill_filter = |f: &crate::mdx_semantic::DimensionFilter| {
        query
            .drill_members
            .iter()
            .any(|(d, keys)| f.dimension == *d && f.members == *keys)
    };
    let slicers: Vec<crate::mdx_semantic::DimensionFilter> = query
        .filters
        .iter()
        .filter(|f| !is_drill_filter(f))
        .cloned()
        .collect();
    let plan = QueryPlan::GroupBy {
        measure: meas.id.clone(),
        group_by: vec![dim.to_string()],
        filters: filters_with_time_flag(model, &meas.id, &typed_filters(&slicers)),
        group_levels: vec![Some(0)],
        set_op: None,
    };
    match execute_plan_with_backend(&plan, model, backend) {
        QueryResult::Grouped(rows) => rows,
        _ => Vec::new(),
    }
}

/// Level-0 member values for two dimensions, paired: the `DrilldownMember`
/// input set (every level-0 member, not just the expanded target) with each
/// member's aggregate per member of the other slot. SSAS returns the whole
/// input set alongside the expanded branch; without it, expanding one member
/// drops every un-expanded sibling from the axis (plan 048).
fn pair_level0_values<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    d0: &str,
    d1: &str,
    backend: &B,
) -> Vec<(String, String, f64)> {
    use crate::engine::plan::{
        QueryPlan, QueryResult, execute_plan_with_backend, filters_with_time_flag, typed_filters,
    };
    let project = crate::proxy_project::project();
    let model = &project.model;
    let meas = query
        .measures
        .first()
        .or(query.measure.as_ref())
        .and_then(|name| model.lookup_measure(name))
        .or_else(|| model.default_measure_id().map(|id| model.meas_def(&id)));
    let Some(meas) = meas else {
        return Vec::new();
    };
    let is_drill_filter = |f: &crate::mdx_semantic::DimensionFilter| {
        query
            .drill_members
            .iter()
            .any(|(d, keys)| f.dimension == *d && f.members == *keys)
    };
    let slicers: Vec<crate::mdx_semantic::DimensionFilter> = query
        .filters
        .iter()
        .filter(|f| !is_drill_filter(f))
        .cloned()
        .collect();
    let plan = QueryPlan::GroupBy {
        measure: meas.id.clone(),
        group_by: vec![d0.to_string(), d1.to_string()],
        filters: filters_with_time_flag(model, &meas.id, &typed_filters(&slicers)),
        group_levels: vec![Some(0), Some(0)],
        set_op: None,
    };
    match execute_plan_with_backend(&plan, model, backend) {
        QueryResult::Pairs(rows) => rows,
        _ => Vec::new(),
    }
}

/// Pre-order member walk for one axis slot's drill: `(All)`, then every
/// ancestor level and the data members themselves, each parent immediately
/// before its children. Excel's DRILLED_DOWN / PARENT_SAME_AS_PREV bookkeeping
/// depends on that order; ancestors grouped by level instead corrupt its
/// hierarchy tree. Returns `(member, Some(label index))` for data rows and
/// `(member, None)` for ancestors.
fn preorder_drill_members<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    dim: &str,
    labels: &[String],
    level_idx: usize,
    extra_roots: &[(String, f64)],
    backend: &B,
) -> Vec<(cellset::MemberConfig, Option<usize>)> {
    let project = crate::proxy_project::project();
    let Some(def) = project.model.dim_def_opt(dim) else {
        return Vec::new();
    };
    let mut order: Vec<usize> = (0..labels.len()).collect();
    order.sort_by(|&a, &b| cmp_key_paths(&labels[a], &labels[b]));

    let mut out: Vec<(cellset::MemberConfig, Option<usize>)> = Vec::new();
    out.push((
        all_member_for_with_backend(dim, &query.dim_props, backend),
        None,
    ));

    #[allow(clippy::too_many_arguments)]
    fn walk<B: QueryBackend + ?Sized>(
        out: &mut Vec<(cellset::MemberConfig, Option<usize>)>,
        query: &SemanticQuery,
        dim: &str,
        def: &crate::engine::model::DimensionDef,
        labels: &[String],
        order: &[usize],
        level_idx: usize,
        level: usize,
        prefix: &str,
        extra_roots: &[(String, f64)],
        backend: &B,
    ) {
        if level == level_idx {
            for &idx in order {
                let label = &labels[idx];
                let parts: Vec<&str> = label.split('|').collect();
                if parts.len() != level_idx + 1 {
                    continue;
                }
                if level_idx > 0 && parts[..level_idx].join("|") != prefix {
                    continue;
                }
                let mut m = leaf_members_from(
                    dim,
                    std::slice::from_ref(label),
                    &query.dim_props,
                    Some(level_idx),
                    None,
                )
                .remove(0);
                attach_parent_keys(std::slice::from_mut(&mut m), dim, Some(level_idx));
                let cc = member_child_count(backend, dim, level_idx, label);
                m.children_cardinality = cc;
                m.display_info = if cc > 0 { 131075 } else { 3 };
                out.push((m, Some(idx)));
            }
            return;
        }
        // Distinct keys at this level under `prefix`, in label order. The
        // level-0 set also seeds the full `DrilldownLevel({All})` input set
        // (every year), which SSAS returns alongside the expanded branch.
        let mut seen: Vec<String> = Vec::new();
        if level == 0 {
            for (key, _) in extra_roots {
                if !seen.contains(key) {
                    seen.push(key.clone());
                }
            }
        }
        for &idx in order {
            let parts: Vec<&str> = labels[idx].split('|').collect();
            if parts.len() <= level {
                continue;
            }
            if level > 0 && parts[..level].join("|") != prefix {
                continue;
            }
            let key = parts[..=level].join("|");
            if !seen.contains(&key) {
                seen.push(key);
            }
        }
        seen.sort_by(|a, b| cmp_key_paths(a, b));
        for key in seen {
            let parts: Vec<&str> = key.split('|').collect();
            let caption = parts.last().copied().unwrap_or(&key).to_string();
            let parent_uname = if level == 0 {
                def.all_member_unique_name()
            } else {
                level_member_uname(def, level - 1, &parts[..level].join("|"))
            };
            let cc = member_child_count(backend, dim, level, &key);
            out.push((
                cellset::MemberConfig {
                    hierarchy: def.hierarchy_unique_name(),
                    u_name: level_member_uname(def, level, &key),
                    caption: caption.clone(),
                    l_name: def
                        .levels
                        .get(level)
                        .map(|l| format!("{}.[{}]", def.hierarchy_unique_name(), l.name))
                        .unwrap_or_default(),
                    l_num: (level + 1) as i32,
                    display_info: 0,
                    children_cardinality: cc,
                    dim_props: walk_dim_props(
                        def,
                        &caption,
                        &level_member_uname(def, level, &key),
                        &def.levels
                            .get(level)
                            .map(|l| format!("{}.[{}]", def.hierarchy_unique_name(), l.name))
                            .unwrap_or_default(),
                        &key,
                        &parent_uname,
                        level,
                        &query.dim_props,
                    ),
                },
                None,
            ));
            walk(
                out,
                query,
                dim,
                def,
                labels,
                order,
                level_idx,
                level + 1,
                &key,
                extra_roots,
                backend,
            );
        }
    }

    walk(
        &mut out,
        query,
        dim,
        def,
        labels,
        &order,
        level_idx,
        0,
        "",
        extra_roots,
        backend,
    );
    out
}

/// DRILLED_DOWN / PARENT_SAME_AS_PREV flags for a single flat member list in
/// pre-order (each parent immediately followed by its children).
fn apply_member_display_info(members: &mut [cellset::MemberConfig]) {
    let parents: Vec<Option<String>> = members
        .iter()
        .map(|m| {
            m.dim_props
                .iter()
                .find(|(k, _)| k == "PARENT_UNIQUE_NAME")
                .map(|(_, v)| v.clone())
        })
        .collect();
    let unames: Vec<String> = members.iter().map(|m| m.u_name.clone()).collect();
    for i in 0..members.len() {
        let mut di = members[i].children_cardinality.min(65535);
        if i + 1 < members.len() && parents[i + 1].as_deref() == Some(unames[i].as_str()) {
            di |= 0x10000;
        }
        if i > 0 && parents[i] == parents[i - 1] && parents[i].is_some() {
            di |= 0x20000;
        }
        members[i].display_info = di;
    }
}

/// Ancestor members derived from the data labels themselves: for a drill to
/// `level_idx` with no single parent, every label path contributes its
/// ancestors (years and quarters for a month drill). Returns `(All)` followed
/// by the distinct members of each level `0..level_idx`, sorted by key.
/// Without this, a whole-field "Expand to Month" returns months whose parents
/// are absent from the axis — an inconsistent hierarchy that Excel dislikes.
fn ancestor_members_from_labels<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    dim: &str,
    def: &crate::engine::model::DimensionDef,
    labels: &[String],
    level_idx: usize,
    backend: &B,
) -> Vec<cellset::MemberConfig> {
    // Distinct ancestor key paths per level.
    let mut keys_by_level: Vec<Vec<String>> = vec![Vec::new(); level_idx];
    for label in labels {
        let parts: Vec<&str> = label.split('|').collect();
        for (i, keys) in keys_by_level.iter_mut().enumerate() {
            if parts.len() > i {
                let key = parts[..=i].join("|");
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
        }
    }
    let mut out: Vec<cellset::MemberConfig> = Vec::new();
    out.push(all_member_for_with_backend(dim, &query.dim_props, backend));
    for (i, keys) in keys_by_level.iter_mut().enumerate() {
        keys.sort();
        let Some(level) = def.levels.get(i) else {
            continue;
        };
        for key in keys.iter() {
            let parts: Vec<&str> = key.split('|').collect();
            let caption = parts.last().copied().unwrap_or(key).to_string();
            let parent_uname = if i == 0 {
                def.all_member_unique_name()
            } else {
                level_member_uname(def, i - 1, &parts[..i].join("|"))
            };
            let cc = member_child_count(backend, dim, i, key);
            out.push(cellset::MemberConfig {
                hierarchy: def.hierarchy_unique_name(),
                u_name: level_member_uname(def, i, key),
                caption: caption.clone(),
                l_name: format!("{}.[{}]", def.hierarchy_unique_name(), level.name),
                l_num: (i + 1) as i32,
                display_info: 0,
                children_cardinality: cc,
                dim_props: walk_dim_props(
                    def,
                    &caption,
                    &level_member_uname(def, i, key),
                    &format!("{}.[{}]", def.hierarchy_unique_name(), level.name),
                    key,
                    &parent_uname,
                    i,
                    &query.dim_props,
                ),
            });
        }
    }
    out
}

/// Dimension properties for members built by the renderer's ancestor/level
/// walks: mirrors the leaf property set plus level identity, so every property
/// Excel requests is actually emitted.
#[allow(clippy::too_many_arguments)]
fn walk_dim_props(
    def: &crate::engine::model::DimensionDef,
    caption: &str,
    u_name: &str,
    l_name: &str,
    key: &str,
    parent_uname: &str,
    level: usize,
    requested: &[String],
) -> Vec<(String, String)> {
    filter_dim_props(
        vec![
            ("PARENT_UNIQUE_NAME".into(), parent_uname.to_string()),
            ("HIERARCHY_UNIQUE_NAME".into(), def.hierarchy_unique_name()),
            ("MEMBER_NAME".into(), caption.to_string()),
            ("MEMBER_CAPTION".into(), caption.to_string()),
            ("MEMBER_UNIQUE_NAME".into(), u_name.to_string()),
            ("MEMBER_KEY".into(), key.to_string()),
            ("MEMBER_TYPE".into(), "1".into()),
            ("MEMBER_VALUE".into(), caption.to_string()),
            ("LEVEL_NUMBER".into(), (level + 1).to_string()),
            ("LEVEL_UNIQUE_NAME".into(), l_name.to_string()),
            ("PARENT_LEVEL".into(), level.to_string()),
            ("PARENT_COUNT".into(), "1".into()),
        ],
        requested,
    )
}

/// The ancestor chain — `(All)` plus levels `0..level_idx` — for a member of
/// `dim` identified by the pipe-joined ancestor path `key_path` (e.g. `2026`).
/// A member whose parent is missing from the axis corrupts Excel's
/// `MDDSAxis::MoveToHierProperty` walk and crashes it, so every expanded
/// member (and every crossjoin slot) must carry its full chain.
fn ancestor_members<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    dim: &str,
    def: &crate::engine::model::DimensionDef,
    key_path: &str,
    level_idx: usize,
    backend: &B,
) -> Vec<cellset::MemberConfig> {
    let key_parts: Vec<&str> = key_path.split('|').filter(|s| !s.is_empty()).collect();
    let mut out: Vec<cellset::MemberConfig> = Vec::new();
    // (All) at hierarchy level 0.
    out.push(all_member_for_with_backend(dim, &query.dim_props, backend));
    for i in 0..level_idx {
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
        let dim_props = walk_dim_props(
            def,
            &caption,
            &u_name,
            &l_name,
            &anc_key,
            &parent_uname,
            i,
            &query.dim_props,
        );
        out.push(cellset::MemberConfig {
            hierarchy: def.hierarchy_unique_name(),
            u_name,
            caption: caption.clone(),
            l_name,
            l_num: (i + 1) as i32,
            display_info: 0,
            children_cardinality: cc,
            dim_props,
        });
    }
    out
}

/// Member unique name for a level, converting an internal pipe path to the
/// SSAS compound-key form: `2026|4` at level `Quarter` becomes
/// `[Date].[Calendar].[Quarter].&amp;[2026]&amp;[4]`.
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
    if dim_def.levels.get(level_idx + 1).is_none() {
        return 0;
    }
    // Cached dictionary (plan 031): one lookup instead of one query per axis
    // member.
    model
        .dim_cache
        .get(model, dim_def, backend)
        .child_count(level_idx, key_path)
}

/// Apply OLE DB for OLAP "Axis Rowsets" DISPLAY_INFO conventions across an
/// axis of tuples (crossjoin shapes). The low 16 bits carry the member's child
/// count; DRILLED_DOWN (0x10000) marks a member whose child appears in the
/// next tuple at the same hierarchy position; PARENT_SAME_AS_PREV (0x20000)
/// marks a member whose parent equals the previous tuple's member parent in
/// the same position. Excel's MDDSAxis::MoveToHierProperty walks the axis on
/// these fields; stale static values corrupt its hierarchy tree.
pub(crate) fn apply_axis_display_info(tuples: &mut [cellset::TupleConfig]) {
    fn parent_of(m: &cellset::MemberConfig) -> Option<String> {
        m.dim_props
            .iter()
            .find(|(tag, _)| tag == "PARENT_UNIQUE_NAME")
            .map(|(_, v)| v.clone())
    }
    let n = tuples.len();
    let parents: Vec<Vec<Option<String>>> = tuples
        .iter()
        .map(|t| t.members.iter().map(parent_of).collect())
        .collect();
    let unames: Vec<Vec<String>> = tuples
        .iter()
        .map(|t| t.members.iter().map(|m| m.u_name.clone()).collect())
        .collect();
    for (i, tuple) in tuples.iter_mut().enumerate() {
        for (s, m) in tuple.members.iter_mut().enumerate() {
            let mut di = m.children_cardinality.min(65535);
            // A child of this member occupies the same slot of the next tuple.
            if i + 1 < n && matches!(parents[i + 1].get(s), Some(Some(p)) if *p == unames[i][s]) {
                di |= 0x10000;
            }
            // Same parent as the previous tuple's member in this slot. Only
            // compare real parents: two parentless (All) roots are not
            // "same as previous" in a way Excel relies on.
            if i > 0 && parents[i - 1].get(s) == parents[i].get(s) && parents[i].get(s).is_some() {
                di |= 0x20000;
            }
            m.display_info = di;
        }
    }
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

    // Per-dimension hierarchy levels: a whole-hierarchy drag (`DrilldownLevel`)
    // puts every leveled dimension on the axis at its top level, so both slots
    // must be built at their grouped level — not just the first.
    let lvl0 = query.drilldown_levels.first().copied().flatten();
    let lvl1 = query.drilldown_levels.get(1).copied().flatten();
    let project = crate::proxy_project::project();

    // A slot is "expanded" (DrilldownMember) when it sits below the top level
    // *and* carries a filter on that dimension: its children must be keyed by
    // the parent and the parent chain must appear on the axis, or Excel's
    // MDDSAxis::MoveToHierProperty walk breaks and crashes.
    let filter_keys = |dim: &str| -> Vec<String> {
        query
            .filters
            .iter()
            .find(|f| f.dimension == dim)
            .map(|f| f.members.clone())
            .unwrap_or_default()
    };
    let slot_drill = |dim: &str, lvl: Option<usize>| -> Option<(usize, Vec<String>)> {
        let lvl = lvl?;
        if lvl == 0 || query.level_drag {
            return None;
        }
        let keys = filter_keys(dim);
        if keys.is_empty() {
            None
        } else {
            Some((lvl, keys))
        }
    };
    let drill0 = slot_drill(d0, lvl0);
    let drill1 = slot_drill(d1, lvl1);
    // With several expanded parents the SQL emits full ancestor paths, so the
    // renderer cannot prefix one parent key; per-member keys are attached later.
    let parent_uname = |dim: &str, drill: &Option<(usize, Vec<String>)>| -> Option<String> {
        let (lvl, keys) = drill.as_ref()?;
        if keys.len() != 1 {
            return None;
        }
        let def = project.model.dim_def_opt(dim)?;
        Some(level_member_uname(def, lvl - 1, &keys[0]))
    };
    let d0_parent_uname = parent_uname(d0, &drill0);
    let d1_parent_uname = parent_uname(d1, &drill1);

    let d0_filter_key = filter_keys(d0).first().cloned().unwrap_or_default();
    let d1_filter_key = filter_keys(d1).first().cloned().unwrap_or_default();
    let cc_map = lvl0.map(|dl| drill_children_cardinalities(backend, d0, dl, &d0_filter_key));
    let cc_map1 = lvl1.map(|dl| drill_children_cardinalities(backend, d1, dl, &d1_filter_key));

    let slot_member = |slot: usize, value: &str| -> crate::cellset::MemberConfig {
        let (dim, lvl, parent, cc_map) = if slot == 0 {
            (d0, lvl0, d0_parent_uname.as_deref(), cc_map.as_ref())
        } else {
            (d1, lvl1, d1_parent_uname.as_deref(), cc_map1.as_ref())
        };
        let v = value.to_string();
        let mut m = leaf_members_from(dim, std::slice::from_ref(&v), &query.dim_props, lvl, parent)
            .remove(0);
        // Path-labelled members aren't in the plain-value cc map; compute their
        // real child count from the key path. Plain labels use the map.
        let label_key = value
            .contains('|')
            .then(|| crate::axis_members::key_from_member_uname(&m.u_name))
            .flatten();
        match (label_key, lvl) {
            (Some(k), Some(lv)) => {
                let cc = member_child_count(backend, dim, lv, &k);
                m.children_cardinality = cc;
                m.display_info = if cc > 0 { 131075 } else { 3 };
            }
            _ => {
                if let Some(cc) = cc_map.and_then(|m| m.get(value)) {
                    m.children_cardinality = *cc;
                }
            }
        }
        // Multi-parent expansion: the label is a path, so derive the parent
        // unique name from it (single-parent members already carry one).
        if !query.level_drag {
            attach_parent_keys(std::slice::from_mut(&mut m), dim, lvl);
        }
        m
    };

    // Distinct values per slot (for ancestor cross-products) and branch totals
    // (ancestor cells aggregate their branch).
    let mut first_values: Vec<String> = Vec::new();
    let mut second_values: Vec<String> = Vec::new();
    let mut first_totals: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    let mut second_totals: std::collections::HashMap<String, f64> =
        std::collections::HashMap::new();
    for (first, second, value) in &all_data {
        if has_exclusions
            && query
                .excluded_members
                .iter()
                .any(|e| e.key == *first || e.key == *second)
        {
            continue;
        }
        if !first_values.contains(first) {
            first_values.push(first.clone());
        }
        if !second_values.contains(second) {
            second_values.push(second.clone());
        }
        *first_totals.entry(first.clone()).or_insert(0.0) += *value;
        *second_totals.entry(second.clone()).or_insert(0.0) += *value;
    }

    let mut tuples: Vec<crate::cellset::TupleConfig> = Vec::new();
    let mut cells = Vec::new();
    let mut ordinal = 0u32;

    // Emit the axis in the crossjoin's natural order: the first slot varies
    // slowest, and each expanded member's ancestor chain comes immediately
    // before that member's children (Excel's DRILLED_DOWN flag depends on the
    // child following its parent on the axis).
    #[allow(clippy::too_many_arguments)]
    fn push(
        tuples: &mut Vec<crate::cellset::TupleConfig>,
        cells: &mut Vec<crate::cellset::CellConfig>,
        ordinal: &mut u32,
        dims: &[String],
        d0: &str,
        d1: &str,
        slot0: crate::cellset::MemberConfig,
        slot1: crate::cellset::MemberConfig,
        query: &SemanticQuery,
        value: f64,
    ) {
        tuples.push(ordered_pair(dims, d0, slot0, d1, slot1));
        cells.push(measurement_cell_for_query(query, *ordinal, value));
        *ordinal += 1;
    }

    // Ancestors per expanded slot. A single plain parent keeps its one chain
    // (the renderer prefixes that parent key on the children); with path
    // labels or several/mixed-level parents, one flat list derived from the
    // labels carries every intermediate level.
    let paths0 = first_values.iter().any(|v| v.contains('|'));
    let paths1 = second_values.iter().any(|v| v.contains('|'));
    let flat0 = drill0
        .as_ref()
        .is_some_and(|(_, keys)| paths0 || keys.len() > 1);
    let flat1 = drill1
        .as_ref()
        .is_some_and(|(_, keys)| paths1 || keys.len() > 1);
    let ancestors_for =
        |dim: &str, drill: &Option<(usize, Vec<String>)>, values: &[String], flat: bool| {
            let Some((lvl, keys)) = drill else {
                return Vec::new();
            };
            let Some(def) = project.model.dim_def_opt(dim) else {
                return Vec::new();
            };
            if flat {
                vec![ancestor_members_from_labels(
                    query, dim, def, values, *lvl, backend,
                )]
            } else {
                keys.iter()
                    .map(|key| ancestor_members(query, dim, def, key, *lvl, backend))
                    .collect::<Vec<_>>()
            }
        };
    let ancestors0 = ancestors_for(d0, &drill0, &first_values, flat0);
    let ancestors1 = ancestors_for(d1, &drill1, &second_values, flat1);
    let all0: Vec<crate::cellset::MemberConfig> = ancestors0
        .first()
        .and_then(|c| c.first().cloned())
        .into_iter()
        .collect();
    let all1: Vec<crate::cellset::MemberConfig> = ancestors1
        .first()
        .and_then(|c| c.first().cloned())
        .into_iter()
        .collect();
    let excluded = |a: &str, b: &str| {
        has_exclusions
            && query
                .excluded_members
                .iter()
                .any(|e| e.key == *a || e.key == *b)
    };
    let value_for = |a: &str, b: &str| -> f64 {
        all_data
            .iter()
            .find(|(x, y, _)| x == a && y == b)
            .map(|(_, _, v)| *v)
            .unwrap_or(0.0)
    };
    // Pre-order trees for flat slots (computed once, cloned per tuple).
    let tree0 = flat0
        .then(|| {
            lvl0.map(|lvl| preorder_drill_members(query, d0, &first_values, lvl, &[], backend))
        })
        .flatten();
    let tree1 = flat1
        .then(|| {
            lvl1.map(|lvl| preorder_drill_members(query, d1, &second_values, lvl, &[], backend))
        })
        .flatten();

    // The DrilldownMember input set for a single-target slot: every level-0
    // member of the drilled hierarchy with its value per other-slot member.
    // SSAS returns them all, with only the target expanded; without them,
    // expanding one year drops every sibling year from the axis (plan 048).
    let roots_pairs = pair_level0_values(query, d0, d1, backend);
    let single_target = |drill: &Option<(usize, Vec<String>)>| -> Option<String> {
        drill
            .as_ref()
            .and_then(|(_, keys)| (keys.len() == 1).then(|| keys[0].clone()))
    };
    let target0 = single_target(&drill0);
    let target1 = single_target(&drill1);
    let roots_for = |slot: usize| -> Vec<String> {
        let mut keys: Vec<String> = roots_pairs
            .iter()
            .map(|(a, b, _)| if slot == 0 { a.clone() } else { b.clone() })
            .collect();
        keys.sort();
        keys.dedup();
        keys
    };
    let roots0 = if target0.is_some() {
        roots_for(0)
    } else {
        Vec::new()
    };
    let roots1 = if target1.is_some() {
        roots_for(1)
    } else {
        Vec::new()
    };
    // Real per-member child counts for the un-expanded roots (a year has 4
    // quarters, not the whole next level's cardinality). Excel's hierarchy walk
    // reads the low bits of DisplayInfo as the child count, so a static
    // whole-level value corrupts its tree.
    let roots_cc = |slot: usize| -> std::collections::HashMap<String, u32> {
        let dim = if slot == 0 { d0 } else { d1 };
        drill_children_cardinalities(backend, dim, 0, "")
    };
    let roots_cc0 = if target0.is_some() {
        roots_cc(0)
    } else {
        std::collections::HashMap::new()
    };
    let roots_cc1 = if target1.is_some() {
        roots_cc(1)
    } else {
        std::collections::HashMap::new()
    };
    let root_value = |first: &str, second: &str| -> f64 {
        roots_pairs
            .iter()
            .find(|(a, b, _)| a == first && b == second)
            .map(|(_, _, v)| *v)
            .unwrap_or(0.0)
    };
    let slot_member_at = |slot: usize, value: &str, level: usize| -> crate::cellset::MemberConfig {
        let dim = if slot == 0 { d0 } else { d1 };
        let v = value.to_string();
        let mut m = leaf_members_from(
            dim,
            std::slice::from_ref(&v),
            &query.dim_props,
            Some(level),
            None,
        )
        .remove(0);
        let cc_map = if slot == 0 { &roots_cc0 } else { &roots_cc1 };
        if let Some(cc) = cc_map.get(value) {
            m.children_cardinality = *cc;
        }
        if !query.level_drag {
            attach_parent_keys(std::slice::from_mut(&mut m), dim, Some(level));
        }
        m
    };

    match (drill0.is_some(), drill1.is_some()) {
        // Second slot expanded (the common Excel shape, e.g. Category x Date):
        // emit each parent immediately before its own children so Excel's
        // DRILLED_DOWN flag lines up.
        (false, true) => {
            for first in &first_values {
                // (All) once per other-slot member (the flat list starts with it).
                if !flat1 {
                    for anc in &all1 {
                        push(
                            &mut tuples,
                            &mut cells,
                            &mut ordinal,
                            dims,
                            d0,
                            d1,
                            slot_member(0, first),
                            anc.clone(),
                            query,
                            *first_totals.get(first).unwrap_or(&0.0),
                        );
                    }
                }
                if !flat1 {
                    // Un-expanded members of the DrilldownMember input set that
                    // sort before the target (plan 048).
                    if let Some(target) = &target1 {
                        for root in &roots1 {
                            if root == target {
                                break;
                            }
                            push(
                                &mut tuples,
                                &mut cells,
                                &mut ordinal,
                                dims,
                                d0,
                                d1,
                                slot_member(0, first),
                                slot_member_at(1, root, 0),
                                query,
                                root_value(first, root),
                            );
                        }
                    }
                    // Single parent: children labels are plain level values.
                    for chain in &ancestors1 {
                        for anc in chain.iter().skip(1) {
                            push(
                                &mut tuples,
                                &mut cells,
                                &mut ordinal,
                                dims,
                                d0,
                                d1,
                                slot_member(0, first),
                                anc.clone(),
                                query,
                                *first_totals.get(first).unwrap_or(&0.0),
                            );
                        }
                    }
                    for (a, b, v) in &all_data {
                        if a == first && !excluded(a, b) {
                            push(
                                &mut tuples,
                                &mut cells,
                                &mut ordinal,
                                dims,
                                d0,
                                d1,
                                slot_member(0, a),
                                slot_member(1, b),
                                query,
                                *v,
                            );
                        }
                    }
                    // Un-expanded members that sort after the target.
                    if let Some(target) = &target1 {
                        let mut seen_target = false;
                        for root in &roots1 {
                            if !seen_target {
                                seen_target = root == target;
                                continue;
                            }
                            push(
                                &mut tuples,
                                &mut cells,
                                &mut ordinal,
                                dims,
                                d0,
                                d1,
                                slot_member(0, first),
                                slot_member_at(1, root, 0),
                                query,
                                root_value(first, root),
                            );
                        }
                    }
                } else {
                    // Pre-order tree: (All), year, its quarters and months,
                    // then the next year — parents immediately before children.
                    if let Some(tree) = &tree1 {
                        for (member, data_idx) in tree {
                            let value = match data_idx {
                                Some(i) => value_for(first, &second_values[*i]),
                                None => {
                                    // Ancestors aggregate their own subtree.
                                    let key =
                                        crate::axis_members::key_from_member_uname(&member.u_name)
                                            .unwrap_or_default();
                                    let prefix = format!("{key}|");
                                    all_data
                                        .iter()
                                        .filter(|(a, b, _)| {
                                            a == first && (*b == key || b.starts_with(&prefix))
                                        })
                                        .map(|(_, _, v)| *v)
                                        .sum()
                                }
                            };
                            push(
                                &mut tuples,
                                &mut cells,
                                &mut ordinal,
                                dims,
                                d0,
                                d1,
                                slot_member(0, first),
                                member.clone(),
                                query,
                                value,
                            );
                        }
                    }
                }
            }
        }
        // First slot expanded (e.g. Date x Segment).
        (true, false) => {
            for second in &second_values {
                if !flat0 {
                    for anc in &all0 {
                        push(
                            &mut tuples,
                            &mut cells,
                            &mut ordinal,
                            dims,
                            d0,
                            d1,
                            anc.clone(),
                            slot_member(1, second),
                            query,
                            *second_totals.get(second).unwrap_or(&0.0),
                        );
                    }
                }
                if !flat0 {
                    // Un-expanded members of the DrilldownMember input set that
                    // sort before the target (plan 048).
                    if let Some(target) = &target0 {
                        for root in &roots0 {
                            if root == target {
                                break;
                            }
                            push(
                                &mut tuples,
                                &mut cells,
                                &mut ordinal,
                                dims,
                                d0,
                                d1,
                                slot_member_at(0, root, 0),
                                slot_member(1, second),
                                query,
                                root_value(root, second),
                            );
                        }
                    }
                    for chain in &ancestors0 {
                        for anc in chain.iter().skip(1) {
                            push(
                                &mut tuples,
                                &mut cells,
                                &mut ordinal,
                                dims,
                                d0,
                                d1,
                                anc.clone(),
                                slot_member(1, second),
                                query,
                                *second_totals.get(second).unwrap_or(&0.0),
                            );
                        }
                    }
                    for (a, b, v) in &all_data {
                        if b == second && !excluded(a, b) {
                            push(
                                &mut tuples,
                                &mut cells,
                                &mut ordinal,
                                dims,
                                d0,
                                d1,
                                slot_member(0, a),
                                slot_member(1, b),
                                query,
                                *v,
                            );
                        }
                    }
                    // Un-expanded members that sort after the target.
                    if let Some(target) = &target0 {
                        let mut seen_target = false;
                        for root in &roots0 {
                            if !seen_target {
                                seen_target = root == target;
                                continue;
                            }
                            push(
                                &mut tuples,
                                &mut cells,
                                &mut ordinal,
                                dims,
                                d0,
                                d1,
                                slot_member_at(0, root, 0),
                                slot_member(1, second),
                                query,
                                root_value(root, second),
                            );
                        }
                    }
                } else {
                    // Pre-order tree for the first slot.
                    if let Some(tree) = &tree0 {
                        for (member, data_idx) in tree {
                            let value = match data_idx {
                                Some(i) => value_for(&first_values[*i], second),
                                None => {
                                    // Ancestors aggregate their own subtree.
                                    let key =
                                        crate::axis_members::key_from_member_uname(&member.u_name)
                                            .unwrap_or_default();
                                    let prefix = format!("{key}|");
                                    all_data
                                        .iter()
                                        .filter(|(a, b, _)| {
                                            b == second && (*a == key || a.starts_with(&prefix))
                                        })
                                        .map(|(_, _, v)| *v)
                                        .sum()
                                }
                            };
                            push(
                                &mut tuples,
                                &mut cells,
                                &mut ordinal,
                                dims,
                                d0,
                                d1,
                                member.clone(),
                                slot_member(1, second),
                                query,
                                value,
                            );
                        }
                    }
                }
            }
        }
        // Both slots expanded (unusual): ancestors first, then the data.
        (true, true) => {
            for chain in &ancestors0 {
                for anc in chain {
                    for second in &second_values {
                        push(
                            &mut tuples,
                            &mut cells,
                            &mut ordinal,
                            dims,
                            d0,
                            d1,
                            anc.clone(),
                            slot_member(1, second),
                            query,
                            *second_totals.get(second).unwrap_or(&0.0),
                        );
                    }
                }
            }
            for chain in &ancestors1 {
                for anc in chain {
                    for first in &first_values {
                        push(
                            &mut tuples,
                            &mut cells,
                            &mut ordinal,
                            dims,
                            d0,
                            d1,
                            slot_member(0, first),
                            anc.clone(),
                            query,
                            *first_totals.get(first).unwrap_or(&0.0),
                        );
                    }
                }
            }
            for (a, b, v) in &all_data {
                if excluded(a, b) {
                    continue;
                }
                push(
                    &mut tuples,
                    &mut cells,
                    &mut ordinal,
                    dims,
                    d0,
                    d1,
                    slot_member(0, a),
                    slot_member(1, b),
                    query,
                    *v,
                );
            }
        }
        // No expanded slot: plain hierarchy levels / leaves.
        (false, false) => {
            for (a, b, v) in &all_data {
                if excluded(a, b) {
                    continue;
                }
                push(
                    &mut tuples,
                    &mut cells,
                    &mut ordinal,
                    dims,
                    d0,
                    d1,
                    slot_member(0, a),
                    slot_member(1, b),
                    query,
                    *v,
                );
            }
        }
    }

    apply_axis_display_info(&mut tuples);

    let axis = crate::cellset::AxisConfig {
        name: "Axis0".into(),
        hierarchies,
        tuples,
    };

    render_response(
        finish_dim_axis(query, backend, axis),
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
    let mut seen_d1_col: std::collections::HashSet<String> = std::collections::HashSet::new();

    // The reference returns the whole `DrilldownLevel({All})` input set: the
    // root tuple first (Excel's Grand Total), then for every parent its own
    // `(parent, child.All)` aggregate followed by the children with data.
    // Emitting only the leaf pairs left Excel with no parent rows and no
    // totals (plan 049).
    let grand_total: f64 = all_data.iter().map(|(_, _, v)| *v).sum();
    let root = ordered_pair(
        dims,
        d0,
        all_member_for_with_backend(d0, &query.dim_props, backend),
        d1,
        all_member_for_with_backend(d1, &query.dim_props, backend),
    );
    tuples.push(root);
    cells.push(measurement_cell_for_query(query, ordinal, grand_total));
    ordinal += 1;

    let mut i = 0usize;
    while i < all_data.len() {
        let parent = all_data[i].0.clone();
        let start = i;
        while i < all_data.len() && all_data[i].0 == parent {
            i += 1;
        }
        let group = &all_data[start..i];
        let collapsed = excluded_d0.contains(parent.as_str());
        let parent_total: f64 = group.iter().map(|(_, _, v)| *v).sum();
        let m0 = leaf_member_for(d0, &parent, &query.dim_props);
        let m1 = all_member_for_with_backend(d1, &query.dim_props, backend);
        tuples.push(ordered_pair(dims, d0, m0, d1, m1));
        cells.push(measurement_cell_for_query(query, ordinal, parent_total));
        ordinal += 1;
        if collapsed {
            continue;
        }
        for (_, second, value) in group {
            if excluded_d1.contains(second.as_str()) {
                if seen_d1_col.insert(second.clone()) {
                    let total = col_d1_totals.get(second).copied().unwrap_or(0.0);
                    let m0 = all_member_for_with_backend(d0, &query.dim_props, backend);
                    let m1 = leaf_member_for(d1, second, &query.dim_props);
                    tuples.push(ordered_pair(dims, d0, m0, d1, m1));
                    cells.push(measurement_cell_for_query(query, ordinal, total));
                    ordinal += 1;
                }
                continue;
            }
            let m0 = leaf_member_for(d0, &parent, &query.dim_props);
            let m1 = leaf_member_for(d1, second, &query.dim_props);
            tuples.push(ordered_pair(dims, d0, m0, d1, m1));
            cells.push(measurement_cell_for_query(query, ordinal, *value));
            ordinal += 1;
        }
    }

    apply_axis_display_info(&mut tuples);

    let axis = crate::cellset::AxisConfig {
        name: "Axis0".into(),
        hierarchies,
        tuples,
    };

    render_response(
        finish_dim_axis(query, backend, axis),
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
        query.drilldown_level(),
        None,
    );
    let mut cells = Vec::new();
    for (i, (_name, value)) in data.iter().enumerate() {
        cells.push(measurement_cell_for_query(query, i as u32, *value));
    }

    let dim_hierarchy = hierarchy_for(dim, &query.dim_props);
    let dim_tuples = dim_tuples_measures(axis1_members);
    let measure_members = vec![measures_total_member_for_query(query)];
    let axes = match measure_dim_axis(query) {
        Some(spec) => vec![
            merged_measure_axis(
                spec.ordinal,
                spec.measures_first(),
                vec![dim_hierarchy],
                dim_tuples,
                &measure_members,
            ),
            full_slicer_axis_with_backend(query, backend),
        ],
        None => split_measure_axes(
            0,
            measure_members,
            1,
            vec![dim_hierarchy],
            dim_tuples,
            full_slicer_axis_with_backend(query, backend),
        ),
    };

    render_response(axes, cells, &query.cell_props)
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

    let mut axis1_members = leaf_members_from(
        dim,
        &merged.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(),
        &query.dim_props,
        query.drilldown_level(),
        None,
    );

    let mut cells = Vec::new();
    let mut ordinal = 0u32;
    // `DrilldownLevel({All})` returns the (All) member first — Excel's Grand
    // Total column/row. Summing the groups is exact for the additive measures
    // the proxy serves (plan 049).
    if !query.level_drag {
        let all = all_member_for_with_backend(dim, &query.dim_props, backend);
        for (mi, measure_id) in measure_ids.iter().enumerate() {
            let total: f64 = merged
                .iter()
                .map(|(_, values)| values.get(mi).copied().unwrap_or(0.0))
                .sum();
            cells.push(measurement_cell_for(ordinal, total, measure_id));
            ordinal += 1;
        }
        axis1_members.insert(0, all);
    }
    // SSAS CellOrdinal is row-major: `row * num_columns + column`. Here the
    // columns are the measures (Axis0) and the rows are the dimension members
    // (Axis1), so cells interleave measure values per group.
    for (_label, values) in merged.iter() {
        for (mi, measure_id) in measure_ids.iter().enumerate() {
            let value = values.get(mi).copied().unwrap_or(0.0);
            cells.push(measurement_cell_for(ordinal, value, measure_id));
            ordinal += 1;
        }
    }

    let dim_hierarchy = hierarchy_for(dim, &query.dim_props);
    let dim_tuples = dim_tuples_measures(axis1_members);
    let axes = match measure_dim_axis(query) {
        Some(spec) => vec![
            merged_measure_axis(
                spec.ordinal,
                spec.measures_first(),
                vec![dim_hierarchy],
                dim_tuples,
                &measure_members,
            ),
            dims_only_slicer_axis_with_backend(query, backend),
        ],
        None => split_measure_axes(
            0,
            measure_members,
            1,
            vec![dim_hierarchy],
            dim_tuples,
            dims_only_slicer_axis_with_backend(query, backend),
        ),
    };

    render_response(axes, cells, &query.cell_props)
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
    apply_axis_display_info(&mut tuples);

    let n_measures = measure_ids.len();
    let mut cells = Vec::new();
    for (pi, (_a, _b, values)) in merged.iter().enumerate() {
        for (mi, measure_id) in measure_ids.iter().enumerate() {
            let value = values.get(mi).copied().unwrap_or(0.0);
            let ordinal = (pi * n_measures + mi) as u32;
            cells.push(measurement_cell_for(ordinal, value, measure_id));
        }
    }

    let axes = match measure_dim_axis(query) {
        Some(spec) => vec![
            merged_measure_axis(
                spec.ordinal,
                spec.measures_first(),
                hierarchies,
                tuples,
                &measure_members,
            ),
            dims_only_slicer_axis_with_backend(query, backend),
        ],
        None => split_measure_axes(
            0,
            measure_members,
            1,
            hierarchies,
            tuples,
            dims_only_slicer_axis_with_backend(query, backend),
        ),
    };

    render_response(axes, cells, &query.cell_props)
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

/// Test seam: route a classified query+result to the correct cellset builder
/// using the demo fixture. Production code calls `dispatch_with_backend` with
/// the request's backend (see `execute/runtime.rs`).
#[cfg(test)]
pub(crate) fn dispatch(query: &SemanticQuery, result: &QueryResult) -> String {
    dispatch_with_backend(query, result, crate::backend::Backend::test_fixture())
}

pub(crate) fn dispatch_with_backend<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    result: &QueryResult,
    backend: &B,
) -> String {
    if matches!(result, QueryResult::Empty)
        && !matches!(
            query.kind,
            SemanticQueryKind::MeasureMetadataProbe
                | SemanticQueryKind::MemberOnlyProbe
                | SemanticQueryKind::SetProbe
        )
    {
        return empty_cellset(query, backend);
    }
    // Three or more grouping dimensions (a field in Columns with two nested in
    // Rows): the N-key result needs the multi-dimension renderer.
    if matches!(result, QueryResult::MultiGroupedN(_)) {
        return build_multi_dim_pivot(query, result, backend);
    }
    // A cross-tab (dimensions on two different axes) needs one axis per edge.
    let dim_axes = query
        .axis_specs
        .iter()
        .filter(|s| !s.dims.is_empty())
        .count();
    if dim_axes >= 2 {
        return build_cross_tab(query, result, backend);
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
        SemanticQueryKind::SetProbe => {
            if let Some(cc) = &query.set_count {
                build_set_count(query, result, cc, backend)
            } else if let Some(se) = &query.set_probe {
                build_set_members(query, result, se, backend)
            } else {
                empty_cellset(query, backend)
            }
        }
    }
}

/// Render an Excel CUBECOUNT probe: one `[Measures].[name]` member on Axis0
/// with the set's member count as the single cell value. A non-count result
/// means the plan failed closed (e.g. an unknown level) — render an empty
/// cellset rather than a misleading `0`.
fn build_set_count<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    result: &QueryResult,
    cc: &crate::mdx_parser::CalculatedCount,
    backend: &B,
) -> String {
    let QueryResult::Count(count) = result else {
        return empty_cellset(query, backend);
    };
    let member = measures_member(&format!("[Measures].[{}]", cc.member_name), &cc.member_name);
    render_response(
        vec![
            single_member_axis("Axis0", measures_hierarchy(), member),
            crate::axis_members::dims_only_slicer_axis_with_backend(query, backend),
        ],
        vec![count_cell(0, *count)],
        &query.cell_props,
    )
}

/// Render a CUBESET validation probe: the set's members on Axis0 with real
/// values, pruned per Head/Tail wrappers.
fn build_set_members<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    result: &QueryResult,
    se: &crate::mdx_parser::SetExpr,
    backend: &B,
) -> String {
    let data = match result {
        QueryResult::Grouped(data) => data.clone(),
        _ => vec![],
    };
    // Unwrap pruning wrappers, remembering their order; the innermost source
    // defines the planned level (or an explicit member list).
    let mut prunes: Vec<(usize, bool)> = Vec::new(); // (n, is_head)
    let (dim, group_level, member_list) = {
        let mut cursor = se;
        loop {
            match cursor {
                crate::mdx_parser::SetExpr::Head(inner, n) => {
                    prunes.push((*n, true));
                    cursor = inner;
                }
                crate::mdx_parser::SetExpr::Tail(inner, n) => {
                    prunes.push((*n, false));
                    cursor = inner;
                }
                crate::mdx_parser::SetExpr::LevelMembers { dim, level } => {
                    let gl = level.as_ref().and_then(|ln| {
                        crate::proxy_project::project()
                            .model
                            .dim_def_opt(dim)
                            .and_then(|def| def.levels.iter().position(|l| l.name == *ln))
                    });
                    break (dim.clone(), gl, None);
                }
                crate::mdx_parser::SetExpr::MemberRange { from, .. } => {
                    let (dim, level, _) =
                        crate::mdx_parser::parse_level_member(from).unwrap_or_default();
                    let gl = crate::proxy_project::project()
                        .model
                        .dim_def_opt(&dim)
                        .and_then(|def| def.levels.iter().position(|l| l.name == level));
                    break (dim, gl, None);
                }
                crate::mdx_parser::SetExpr::AllMembers { dim } => {
                    break (dim.clone(), Some(0), None);
                }
                crate::mdx_parser::SetExpr::Measures => {
                    break (String::new(), None, None);
                }
                crate::mdx_parser::SetExpr::MemberList { unames } => {
                    break (String::new(), None, Some(unames.clone()));
                }
            }
        }
    };

    // Materialize (uname, caption, value) triples for the set's members.
    let is_measures = matches!(se, crate::mdx_parser::SetExpr::Measures);
    let mut entries: Vec<(String, String, f64)> = if is_measures {
        crate::proxy_project::project()
            .model
            .measures
            .iter()
            .enumerate()
            .map(|(i, m)| {
                (
                    m.measure_unique_name(),
                    m.display_name.clone(),
                    data.get(i).map(|(_, v)| *v).unwrap_or(0.0),
                )
            })
            .collect()
    } else if let Some(unames) = &member_list {
        let lookup: std::collections::HashMap<String, f64> = data.iter().cloned().collect();
        unames
            .iter()
            .map(|u| {
                let decoded = u.replace("&amp;", "&");
                let caption = decoded
                    .rsplit("&[")
                    .next()
                    .unwrap_or(&decoded)
                    .trim_end_matches(']')
                    .to_string();
                let v = lookup.get(&caption).copied().unwrap_or(0.0);
                (decoded, caption, v)
            })
            .collect()
    } else {
        data.iter()
            .map(|(n, v)| (n.clone(), n.clone(), *v))
            .collect()
    };
    // Apply Head/Tail wrappers in reverse declaration order.
    for (n, is_head) in prunes.into_iter().rev() {
        if is_head {
            entries.truncate(n);
        } else {
            let start = entries.len().saturating_sub(n);
            entries = entries[start..].to_vec();
        }
    }

    let (members, axis_hier) = if is_measures {
        let ms: Vec<cellset::MemberConfig> = entries
            .iter()
            .map(|(uname, caption, _)| measures_member(uname, caption))
            .collect();
        (ms, measures_hierarchy())
    } else if let Some(unames) = &member_list {
        // Render explicit members straight from their unique names.
        let dim_tok = unames
            .first()
            .and_then(|u| u.split(']').next())
            .map(|d| format!("[{d}]"))
            .unwrap_or_default();
        let hier_tok = unames
            .first()
            .map(|u| {
                u.trim_start_matches('[')
                    .split("].[")
                    .nth(1)
                    .and_then(|h| h.split(']').next())
                    .unwrap_or("")
            })
            .unwrap_or("");
        let hier_u = format!("{dim_tok}.[{hier_tok}]");
        let ms = entries
            .iter()
            .map(|(uname, caption, _)| cellset::MemberConfig {
                hierarchy: hier_u.clone(),
                u_name: xml_escape(uname),
                caption: xml_escape(caption),
                l_name: xml_escape(&format!("{hier_u}.[{hier_tok}]")),
                l_num: 1,
                display_info: 3,
                children_cardinality: 0,
                dim_props: vec![],
            })
            .collect();
        let hc = cellset::HierarchyConfig {
            name: hier_u,
            dim_prop_decls: vec![],
            include_children_cardinality: includes_prop(&query.dim_props, "CHILDREN_CARDINALITY"),
        };
        (ms, hc)
    } else {
        let names: Vec<String> = entries.iter().map(|(_, c, _)| c.clone()).collect();
        let ms = leaf_members_from(dim.as_str(), &names, &query.dim_props, group_level, None);
        let hc = hierarchy_for(dim.as_str(), &query.dim_props);
        (ms, hc)
    };
    let cells: Vec<cellset::CellConfig> = entries
        .iter()
        .enumerate()
        .map(|(i, (_, _, v))| measurement_cell_for_query(query, i as u32, *v))
        .collect();

    render_response(
        vec![
            member_list_axis("Axis0", axis_hier, members),
            full_slicer_axis_with_backend(query, backend),
        ],
        cells,
        &query.cell_props,
    )
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
            include_children_cardinality: includes_prop(&query.dim_props, "CHILDREN_CARDINALITY"),
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
            let level = level_unique_name_for_member(target)
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
                int_value: None,
            });
            cell_ordinal += 1;
        }
    }

    let axis0 = member_list_axis("Axis0", measures_hierarchy(), members);
    let slicer = full_slicer_axis_with_backend(query, backend);

    render_response(vec![axis0, slicer], cells, &query.cell_props)
}

/// Split `[Date].[Calendar].[Year]` into its bracket-delimited parts.
fn bracket_segments(input: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = input.trim();
    while let Some(after_open) = rest.strip_prefix('[') {
        let Some(close) = after_open.find(']') else {
            break;
        };
        out.push(&after_open[..close]);
        rest = after_open[close + 1..].trim_start_matches('.');
    }
    out
}

/// Level unique name for a member unique name, the way the mirror answers
/// `strtomember(...).level.UniqueName` (measured 2026-09-23):
///
/// - `[Date].[Calendar].[Year].&[2026]` → `[Date].[Calendar].[Year]` (the
///   level-qualified prefix),
/// - `[Category].[Category].&[Books]` → the dimension's leaf level
///   (`[Category].[Category].[Category]`),
/// - `[Date].[Full Date].&[2020-01-01]` → `[Date].[Full Date].[Full Date]`
///   (the date role's key hierarchy is single-level).
///
/// Using the hierarchy name as the level made Excel answer `#N/A` for
/// level-qualified `CUBEVALUE` tuples: its probe returned a level that matched
/// no advertised level, so it never asked for the value (plan 049 follow-up).
fn level_unique_name_for_member(target: &str) -> Option<String> {
    let head = target.split(".&[").next().unwrap_or(target).trim();
    let segments = bracket_segments(head);
    match segments.len() {
        0 | 1 => None,
        2 => {
            // The measures hierarchy is answered by the caller's measure
            // branch; there is no dimension level to name.
            if segments[0].eq_ignore_ascii_case("Measures") {
                return None;
            }
            let model = &crate::proxy_project::project().model;
            let dim = model
                .dimensions
                .iter()
                .find(|d| d.hierarchy_unique_name() == head);
            Some(dim.map(|d| d.leaf_level_unique_name()).unwrap_or_else(|| {
                format!("[{}].[{}].[{}]", segments[0], segments[1], segments[1])
            }))
        }
        _ => Some(format!(
            "[{}].[{}].[{}]",
            segments[0], segments[1], segments[2]
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::level_unique_name_for_member;

    /// Mirror-measured (2026-09-23): the level of a level-qualified member is
    /// the qualified prefix; a flat member reports its dimension's leaf level.
    #[test]
    fn level_unique_name_matches_the_mirror() {
        let p = crate::proxy_project::ProxyProject::load("projects/project3/proxy-config.json")
            .expect("load project3");
        crate::project::project::with_test_project(p, || {
            let cases = [
                (
                    "[Date].[Calendar].[Year].&[2026]",
                    "[Date].[Calendar].[Year]",
                ),
                (
                    "[Date].[Calendar].[Quarter].&[2026]&[3]",
                    "[Date].[Calendar].[Quarter]",
                ),
                (
                    "[Category].[Category].&[Electronics]",
                    "[Category].[Category].[Category]",
                ),
                (
                    "[Territory].[Territory].&[North]",
                    "[Territory].[Territory].[Territory]",
                ),
                (
                    "[Date].[Full Date].&[2020-01-01]",
                    "[Date].[Full Date].[Full Date]",
                ),
            ];
            for (member, expected) in cases {
                assert_eq!(
                    level_unique_name_for_member(member).as_deref(),
                    Some(expected),
                    "level of {member}"
                );
            }
            assert_eq!(level_unique_name_for_member("[Measures].[Revenue]"), None);
        });
    }
}
