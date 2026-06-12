/// Dimension/member/cell/slicer helpers for cellset responses.
///
/// Provides member constructors, hierarchy builders, axis assembly helpers,
/// cell constructors, and the `full_slicer_axis` builder.
/// Consumed by `execute_builders` for all cellset response construction.

use crate::cellset;
use crate::mdx_semantic::{includes_prop, DimensionFilter, SemanticQuery};
use crate::proxy_project;

const MEASURES_HIER: &str = "[Measures]";
const MEASURES_LEVEL: &str = "[Measures].[MeasuresLevel]";

// ---- dimension property helpers ----

pub(crate) fn filter_dim_props(
    props: Vec<(String, String)>,
    requested: &[String],
) -> Vec<(String, String)> {
    props
        .into_iter()
        .filter(|(tag, _)| includes_prop(requested, tag))
        .collect()
}

pub(crate) fn filter_dim_prop_decls(
    props: Vec<(String, String, String)>,
    requested: &[String],
) -> Vec<(String, String, String)> {
    props
        .into_iter()
        .filter(|(tag, _, _)| includes_prop(requested, tag))
        .collect()
}

fn dim_def(dim: &str) -> Option<&crate::engine::model::DimensionDef> {
    proxy_project::project().model.dim_def_opt(dim)
}

fn dim_children_count(dim: &crate::engine::model::DimensionDef) -> u32 {
    crate::backend::Backend::get()
        .distinct_count_in(proxy_project::project().model.dim_table(&dim.id), &dim.physical_field)
}

fn dim_props_leaf(
    dim: &crate::engine::model::DimensionDef,
    name: &str,
    requested: &[String],
) -> Vec<(String, String)> {
    filter_dim_props(
        vec![
            ("PARENT_UNIQUE_NAME".into(), dim.all_member_unique_name()),
            ("HIERARCHY_UNIQUE_NAME".into(), dim.hierarchy_unique_name()),
            ("MEMBER_NAME".into(), name.to_string()),
            ("MEMBER_KEY".into(), name.to_string()),
            ("MEMBER_TYPE".into(), "1".into()),
            ("MEMBER_VALUE".into(), name.to_string()),
            ("PARENT_LEVEL".into(), "0".into()),
            ("PARENT_COUNT".into(), "1".into()),
            ("CHILDREN_CARDINALITY".into(), "0".into()),
        ],
        requested,
    )
}

fn dim_props_all(
    dim: &crate::engine::model::DimensionDef,
    requested: &[String],
) -> Vec<(String, String)> {
    let count = dim_children_count(dim);
    filter_dim_props(
        vec![
            ("HIERARCHY_UNIQUE_NAME".into(), dim.hierarchy_unique_name()),
            ("MEMBER_NAME".into(), "All".into()),
            ("MEMBER_KEY".into(), "All".into()),
            ("MEMBER_TYPE".into(), "2".into()),
            ("MEMBER_VALUE".into(), "All".into()),
            ("PARENT_LEVEL".into(), "0".into()),
            ("PARENT_COUNT".into(), "0".into()),
            ("CHILDREN_CARDINALITY".into(), count.to_string()),
        ],
        requested,
    )
}

fn dim_decls(dim: &crate::engine::model::DimensionDef) -> Vec<(String, String, String)> {
    let p = dim.hierarchy_unique_name();
    vec![
        (
            "PARENT_UNIQUE_NAME".into(),
            format!("{p}.[PARENT_UNIQUE_NAME]"),
            "xsd:string".into(),
        ),
        (
            "HIERARCHY_UNIQUE_NAME".into(),
            format!("{p}.[HIERARCHY_UNIQUE_NAME]"),
            "xsd:string".into(),
        ),
        (
            "MEMBER_NAME".into(),
            format!("{p}.[MEMBER_NAME]"),
            "xsd:string".into(),
        ),
        (
            "MEMBER_KEY".into(),
            format!("{p}.[MEMBER_KEY]"),
            "xsd:string".into(),
        ),
        (
            "MEMBER_TYPE".into(),
            format!("{p}.[MEMBER_TYPE]"),
            "xsd:int".into(),
        ),
        (
            "MEMBER_VALUE".into(),
            format!("{p}.[MEMBER_VALUE]"),
            "xsd:string".into(),
        ),
        (
            "PARENT_LEVEL".into(),
            format!("{p}.[PARENT_LEVEL]"),
            "xsd:int".into(),
        ),
        (
            "PARENT_COUNT".into(),
            format!("{p}.[PARENT_COUNT]"),
            "xsd:int".into(),
        ),
        (
            "CHILDREN_CARDINALITY".into(),
            format!("{p}.[CHILDREN_CARDINALITY]"),
            "xsd:unsignedInt".into(),
        ),
    ]
}

fn hierarchy_for_dim(
    dim: &crate::engine::model::DimensionDef,
    requested: &[String],
) -> cellset::HierarchyConfig {
    cellset::HierarchyConfig {
        name: dim.hierarchy_unique_name(),
        dim_prop_decls: filter_dim_prop_decls(dim_decls(dim), requested),
    }
}

fn leaf_member_for_dim(
    dim: &crate::engine::model::DimensionDef,
    name: &str,
    requested: &[String],
) -> cellset::MemberConfig {
    let u_name = format!("{}.&amp;[{}]", dim.hierarchy_unique_name(), name);
    cellset::MemberConfig {
        hierarchy: dim.hierarchy_unique_name(),
        u_name,
        caption: name.to_string(),
        l_name: dim.leaf_level_unique_name(),
        l_num: 1,
        display_info: 3,
        dim_props: dim_props_leaf(dim, name, requested),
    }
}

fn all_member_for_dim(
    dim: &crate::engine::model::DimensionDef,
    requested: &[String],
) -> cellset::MemberConfig {
    cellset::MemberConfig {
        hierarchy: dim.hierarchy_unique_name(),
        u_name: dim.all_member_unique_name(),
        caption: "All".into(),
        l_name: dim.all_level_unique_name(),
        l_num: 0,
        display_info: 5,
        dim_props: dim_props_all(dim, requested),
    }
}

fn leaf_members_from_dim(
    dim: &crate::engine::model::DimensionDef,
    names: &[String],
    requested: &[String],
) -> Vec<cellset::MemberConfig> {
    names.iter()
        .map(|name| leaf_member_for_dim(dim, name, requested))
        .collect()
}

fn default_measure() -> &'static crate::engine::model::MeasureDef {
    let project = proxy_project::project();
    let id = project
        .model
        .default_measure_id()
        .unwrap_or_else(|| project.model.measures.first().map(|m| m.id.clone()).unwrap_or_default());
    project.model.meas_def(&id)
}

fn measure_by_id(measure_id: &str) -> &crate::engine::model::MeasureDef {
    proxy_project::project().model.meas_def(measure_id)
}

// ---- cell constructors ----

pub(crate) fn measurement_cell(ordinal: u32, value: f64) -> cellset::CellConfig {
    measurement_cell_for_measure(ordinal, value, default_measure())
}

pub(crate) fn measurement_cell_for(ordinal: u32, value: f64, measure_id: &str) -> cellset::CellConfig {
    measurement_cell_for_measure(ordinal, value, measure_by_id(measure_id))
}

fn measurement_cell_for_measure(ordinal: u32, value: f64, m: &crate::engine::model::MeasureDef) -> cellset::CellConfig {
    let fmt = if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    };
    let fmt_value = if m.units.is_empty() {
        fmt.clone()
    } else {
        format!("{} {}", fmt, m.units)
    };
    cellset::CellConfig {
        ordinal,
        value,
        fmt_value,
        format_string: m.format_string.clone(),
        back_color: String::new(),
        fore_color: String::new(),
    }
}

pub(crate) fn count_cell(ordinal: u32, value: u32) -> cellset::CellConfig {
    cellset::CellConfig {
        ordinal,
        value: value as f64,
        fmt_value: value.to_string(),
        format_string: "0".into(),
        back_color: String::new(),
        fore_color: String::new(),
    }
}

pub(crate) fn measures_member(unique_name: &str, caption: &str) -> cellset::MemberConfig {
    cellset::MemberConfig {
        hierarchy: MEASURES_HIER.into(),
        u_name: unique_name.into(),
        caption: caption.into(),
        l_name: MEASURES_LEVEL.into(),
        l_num: 0,
        display_info: 131072,
        dim_props: vec![],
    }
}

pub(crate) fn measures_total_member() -> cellset::MemberConfig {
    let m = default_measure();
    measures_member(&m.measure_unique_name(), &m.display_name)
}

pub(crate) fn cchildren_member() -> cellset::MemberConfig {
    measures_member("[Measures].[cChildren]", "cChildren")
}

// ---- axis helpers ----

pub(crate) fn tuples_from_members(members: Vec<cellset::MemberConfig>) -> Vec<cellset::TupleConfig> {
    members
        .into_iter()
        .map(|member| cellset::TupleConfig { members: vec![member] })
        .collect()
}

pub(crate) fn render_response(
    axes: Vec<cellset::AxisConfig>,
    cells: Vec<cellset::CellConfig>,
    cell_props: &[String],
) -> String {
    let resp = cellset::CellsetResponse {
        cube_name: proxy_project::project().config.cube.clone(),
        axes,
        cells,
        include_value: cell_props.is_empty() || includes_prop(cell_props, "VALUE"),
        include_fmt_value: includes_prop(cell_props, "FORMATTED_VALUE"),
        include_format_string: includes_prop(cell_props, "FORMAT_STRING"),
        include_back_color: includes_prop(cell_props, "BACK_COLOR"),
        include_fore_color: includes_prop(cell_props, "FORE_COLOR"),
    };
    cellset::render_cellset(&resp)
}

#[allow(dead_code)]
pub(crate) fn slicer_axis_with_members(
    hierarchies: Vec<cellset::HierarchyConfig>,
    members: Vec<cellset::MemberConfig>,
) -> cellset::AxisConfig {
    cellset::AxisConfig {
        name: "SlicerAxis".into(),
        hierarchies,
        tuples: vec![cellset::TupleConfig { members }],
    }
}

#[allow(dead_code)]
pub(crate) fn empty_slicer_axis() -> cellset::AxisConfig {
    cellset::AxisConfig {
        name: "SlicerAxis".into(),
        hierarchies: vec![],
        tuples: vec![cellset::TupleConfig { members: vec![] }],
    }
}

pub(crate) fn measures_axis() -> cellset::AxisConfig {
    cellset::AxisConfig {
        name: "Axis0".into(),
        hierarchies: vec![measures_hierarchy()],
        tuples: vec![cellset::TupleConfig {
            members: vec![measures_total_member()],
        }],
    }
}

pub(crate) fn measures_hierarchy() -> cellset::HierarchyConfig {
    cellset::HierarchyConfig {
        name: MEASURES_HIER.into(),
        dim_prop_decls: vec![],
    }
}

pub(crate) fn single_member_axis(
    name: &str,
    hierarchy: cellset::HierarchyConfig,
    member: cellset::MemberConfig,
) -> cellset::AxisConfig {
    cellset::AxisConfig {
        name: name.into(),
        hierarchies: vec![hierarchy],
        tuples: vec![cellset::TupleConfig { members: vec![member] }],
    }
}

pub(crate) fn member_list_axis(
    name: &str,
    hierarchy: cellset::HierarchyConfig,
    members: Vec<cellset::MemberConfig>,
) -> cellset::AxisConfig {
    cellset::AxisConfig {
        name: name.into(),
        hierarchies: vec![hierarchy],
        tuples: tuples_from_members(members),
    }
}

pub(crate) fn empty_member_list_axis(name: &str, hierarchy: cellset::HierarchyConfig) -> cellset::AxisConfig {
    cellset::AxisConfig {
        name: name.into(),
        hierarchies: vec![hierarchy],
        tuples: vec![],
    }
}

// ---- dimension dispatch helpers ----

pub(crate) fn row_dim(query: &SemanticQuery) -> &str {
    if let Some(dim) = query.axis_dimensions.first() {
        dim.as_str()
    } else if let Some(dim) = query.row_dimension.as_deref() {
        dim
    } else {
        "Produktkategori"
    }
}

pub(crate) fn leaf_member_for(dim: &str, name: &str, requested: &[String]) -> cellset::MemberConfig {
    match dim_def(dim) {
        Some(d) => leaf_member_for_dim(d, name, requested),
        None => unknown_dim_member(dim, name),
    }
}

pub(crate) fn all_member_for(dim: &str, requested: &[String]) -> cellset::MemberConfig {
    match dim_def(dim) {
        Some(d) => all_member_for_dim(d, requested),
        None => unknown_dim_member(dim, "All"),
    }
}

pub(crate) fn hierarchy_for(dim: &str, requested: &[String]) -> cellset::HierarchyConfig {
    match dim_def(dim) {
        Some(d) => hierarchy_for_dim(d, requested),
        None => cellset::HierarchyConfig {
            name: format!("[{dim}].[{dim}]"),
            dim_prop_decls: vec![],
        },
    }
}

pub(crate) fn leaf_members_from(dim: &str, names: &[String], requested: &[String]) -> Vec<cellset::MemberConfig> {
    match dim_def(dim) {
        Some(d) => leaf_members_from_dim(d, names, requested),
        None => names.iter().map(|n| unknown_dim_member(dim, n)).collect(),
    }
}

fn unknown_dim_member(dim: &str, name: &str) -> cellset::MemberConfig {
    cellset::MemberConfig {
        hierarchy: format!("[{dim}].[{dim}]"),
        u_name: format!("[{dim}].[{dim}].&[{name}]"),
        caption: name.to_string(),
        l_name: format!("[{dim}].[{dim}].[{dim}]"),
        l_num: 1,
        display_info: 3,
        dim_props: vec![],
    }
}

// ---- slicer helpers ----

pub(crate) fn filter_members_for(dim: &str, filters: &[DimensionFilter]) -> Vec<String> {
    filters
        .iter()
        .find(|f| f.dimension == dim)
        .map(|f| f.members.clone())
        .unwrap_or_default()
}

pub(crate) fn full_slicer_axis(query: &SemanticQuery) -> cellset::AxisConfig {
    let project = proxy_project::project();
    let mut hierarchies: Vec<cellset::HierarchyConfig> = Vec::new();
    let mut members: Vec<cellset::MemberConfig> = Vec::new();

    // Measures always appear first on the slicer axis.
    hierarchies.push(measures_hierarchy());
    members.push(measures_total_member());

    let mut dims: Vec<&crate::engine::model::DimensionDef> = project
        .model
        .dimensions
        .iter()
        .filter(|d| d.visible)
        .collect();
    dims.sort_by_key(|d| d.ordinal);

    for dim in dims {
        if query.axis_dimensions.contains(&dim.id) {
            continue;
        }

        hierarchies.push(hierarchy_for_dim(dim, &[]));

        let slc = query.slicers.iter().find(|s| s.dimension == dim.id);
        if slc.map(|s| s.is_all).unwrap_or(true) {
            members.push(all_member_for_dim(dim, &[]));
        } else {
            let dim_members = filter_members_for(&dim.id, &query.filters);
            for name in &dim_members {
                members.push(leaf_member_for_dim(dim, name, &[]));
            }
        }
    }

    cellset::AxisConfig {
        name: "SlicerAxis".into(),
        hierarchies,
        tuples: vec![cellset::TupleConfig { members }],
    }
}
