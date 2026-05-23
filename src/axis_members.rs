/// Dimension/member/cell/slicer helpers for cellset responses.
///
/// Provides member constructors, hierarchy builders, axis assembly helpers,
/// cell constructors, and the `full_slicer_axis` builder.
/// Consumed by `execute_builders` for all cellset response construction.

use crate::cellset;
use crate::backend::Backend;
use crate::mdx_semantic::{
    includes_prop, SemanticQuery, DimensionFilter,
    PRODUKTKATEGORI_HIER, PRODUKTKATEGORI_ALL_U, PRODUKTKATEGORI_ALL_L,
    PRODUKTKATEGORI_LEAF_L, MEASURES_HIER, MEASURES_LEVEL,
    REGION_HIER, REGION_ALL_U, REGION_ALL_L, REGION_LEAF_L,
};

// ---- dimension property helpers ----

pub(crate) fn filter_dim_props(props: Vec<(String, String)>, requested: &[String]) -> Vec<(String, String)> {
    props.into_iter()
        .filter(|(tag, _)| includes_prop(requested, tag))
        .collect()
}

pub(crate) fn filter_dim_prop_decls(
    props: Vec<(String, String, String)>,
    requested: &[String],
) -> Vec<(String, String, String)> {
    props.into_iter()
        .filter(|(tag, _, _)| includes_prop(requested, tag))
        .collect()
}

// ---- Produktkategori member/property/hierarchy builders ----

pub(crate) fn produktkategori_dim_props_leaf(name: &str, requested: &[String]) -> Vec<(String, String)> {
    filter_dim_props(vec![
        ("PARENT_UNIQUE_NAME".into(), PRODUKTKATEGORI_ALL_U.into()),
        ("HIERARCHY_UNIQUE_NAME".into(), PRODUKTKATEGORI_HIER.into()),
        ("MEMBER_NAME".into(), name.to_string()),
        ("MEMBER_KEY".into(), name.to_string()),
        ("MEMBER_TYPE".into(), "1".into()),
        ("MEMBER_VALUE".into(), name.to_string()),
        ("PARENT_LEVEL".into(), "0".into()),
        ("PARENT_COUNT".into(), "1".into()),
        ("CHILDREN_CARDINALITY".into(), "0".into()),
    ], requested)
}

pub(crate) fn produktkategori_dim_props_all(requested: &[String]) -> Vec<(String, String)> {
    let count = Backend::get().category_count();
    filter_dim_props(vec![
        ("HIERARCHY_UNIQUE_NAME".into(), PRODUKTKATEGORI_HIER.into()),
        ("MEMBER_NAME".into(), "All".into()),
        ("MEMBER_KEY".into(), "All".into()),
        ("MEMBER_TYPE".into(), "2".into()),
        ("MEMBER_VALUE".into(), "All".into()),
        ("PARENT_LEVEL".into(), "0".into()),
        ("PARENT_COUNT".into(), "0".into()),
        ("CHILDREN_CARDINALITY".into(), count.to_string()),
    ], requested)
}

pub(crate) fn produktkategori_dim_decls() -> Vec<(String, String, String)> {
    let p = "[Produktkategori].[Produktkategori]";
    vec![
        ("PARENT_UNIQUE_NAME".into(),   format!("{p}.[PARENT_UNIQUE_NAME]"),   "xsd:string".into()),
        ("HIERARCHY_UNIQUE_NAME".into(),format!("{p}.[HIERARCHY_UNIQUE_NAME]"),"xsd:string".into()),
        ("MEMBER_NAME".into(),          format!("{p}.[MEMBER_NAME]"),          "xsd:string".into()),
        ("MEMBER_KEY".into(),           format!("{p}.[MEMBER_KEY]"),           "xsd:string".into()),
        ("MEMBER_TYPE".into(),          format!("{p}.[MEMBER_TYPE]"),          "xsd:int".into()),
        ("MEMBER_VALUE".into(),         format!("{p}.[MEMBER_VALUE]"),         "xsd:string".into()),
        ("PARENT_LEVEL".into(),         format!("{p}.[PARENT_LEVEL]"),         "xsd:int".into()),
        ("PARENT_COUNT".into(),         format!("{p}.[PARENT_COUNT]"),         "xsd:int".into()),
        ("CHILDREN_CARDINALITY".into(), format!("{p}.[CHILDREN_CARDINALITY]"), "xsd:unsignedInt".into()),
    ]
}

// ---- Region dimension property/member/hierarchy builders ----

pub(crate) fn region_dim_props_leaf(name: &str, requested: &[String]) -> Vec<(String, String)> {
    filter_dim_props(vec![
        ("PARENT_UNIQUE_NAME".into(), REGION_ALL_U.into()),
        ("HIERARCHY_UNIQUE_NAME".into(), REGION_HIER.into()),
        ("MEMBER_NAME".into(), name.to_string()),
        ("MEMBER_KEY".into(), name.to_string()),
        ("MEMBER_TYPE".into(), "1".into()),
        ("MEMBER_VALUE".into(), name.to_string()),
        ("PARENT_LEVEL".into(), "0".into()),
        ("PARENT_COUNT".into(), "1".into()),
        ("CHILDREN_CARDINALITY".into(), "0".into()),
    ], requested)
}

pub(crate) fn region_dim_props_all(requested: &[String]) -> Vec<(String, String)> {
    let count = Backend::get().region_count();
    filter_dim_props(vec![
        ("HIERARCHY_UNIQUE_NAME".into(), REGION_HIER.into()),
        ("MEMBER_NAME".into(), "All".into()),
        ("MEMBER_KEY".into(), "All".into()),
        ("MEMBER_TYPE".into(), "2".into()),
        ("MEMBER_VALUE".into(), "All".into()),
        ("PARENT_LEVEL".into(), "0".into()),
        ("PARENT_COUNT".into(), "0".into()),
        ("CHILDREN_CARDINALITY".into(), count.to_string()),
    ], requested)
}

pub(crate) fn region_dim_decls() -> Vec<(String, String, String)> {
    let p = "[Region].[Region]";
    vec![
        ("PARENT_UNIQUE_NAME".into(),   format!("{p}.[PARENT_UNIQUE_NAME]"),   "xsd:string".into()),
        ("HIERARCHY_UNIQUE_NAME".into(),format!("{p}.[HIERARCHY_UNIQUE_NAME]"),"xsd:string".into()),
        ("MEMBER_NAME".into(),          format!("{p}.[MEMBER_NAME]"),          "xsd:string".into()),
        ("MEMBER_KEY".into(),           format!("{p}.[MEMBER_KEY]"),           "xsd:string".into()),
        ("MEMBER_TYPE".into(),          format!("{p}.[MEMBER_TYPE]"),          "xsd:int".into()),
        ("MEMBER_VALUE".into(),         format!("{p}.[MEMBER_VALUE]"),         "xsd:string".into()),
        ("PARENT_LEVEL".into(),         format!("{p}.[PARENT_LEVEL]"),         "xsd:int".into()),
        ("PARENT_COUNT".into(),         format!("{p}.[PARENT_COUNT]"),         "xsd:int".into()),
        ("CHILDREN_CARDINALITY".into(), format!("{p}.[CHILDREN_CARDINALITY]"), "xsd:unsignedInt".into()),
    ]
}

// ---- hierarchy builders ----

pub(crate) fn region_hierarchy(requested: &[String]) -> cellset::HierarchyConfig {
    cellset::HierarchyConfig {
        name: REGION_HIER.into(),
        dim_prop_decls: filter_dim_prop_decls(region_dim_decls(), requested),
    }
}

pub(crate) fn region_leaf_member(name: &str, requested: &[String]) -> cellset::MemberConfig {
    let u_name = format!("[Region].[Region].&amp;[{}]", name);
    cellset::MemberConfig {
        hierarchy: REGION_HIER.into(),
        u_name,
        caption: name.to_string(),
        l_name: REGION_LEAF_L.into(),
        l_num: 1,
        display_info: 3,
        dim_props: region_dim_props_leaf(name, requested),
    }
}

pub(crate) fn region_all_member(requested: &[String]) -> cellset::MemberConfig {
    cellset::MemberConfig {
        hierarchy: REGION_HIER.into(),
        u_name: REGION_ALL_U.into(),
        caption: "All".into(),
        l_name: REGION_ALL_L.into(),
        l_num: 0,
        display_info: 5,
        dim_props: region_dim_props_all(requested),
    }
}

pub(crate) fn region_leaf_members_from(names: &[String], requested: &[String]) -> Vec<cellset::MemberConfig> {
    names.iter().map(|name| region_leaf_member(name, requested)).collect()
}

pub(crate) fn produktkategori_hierarchy(requested: &[String]) -> cellset::HierarchyConfig {
    cellset::HierarchyConfig {
        name: PRODUKTKATEGORI_HIER.into(),
        dim_prop_decls: filter_dim_prop_decls(produktkategori_dim_decls(), requested),
    }
}

pub(crate) fn measures_hierarchy() -> cellset::HierarchyConfig {
    cellset::HierarchyConfig {
        name: MEASURES_HIER.into(),
        dim_prop_decls: vec![],
    }
}

pub(crate) fn produktkategori_leaf_member(name: &str, requested: &[String]) -> cellset::MemberConfig {
    let u_name = format!("[Produktkategori].[Produktkategori].&amp;[{}]", name);
    cellset::MemberConfig {
        hierarchy: PRODUKTKATEGORI_HIER.into(),
        u_name,
        caption: name.to_string(),
        l_name: PRODUKTKATEGORI_LEAF_L.into(),
        l_num: 1,
        display_info: 3,
        dim_props: produktkategori_dim_props_leaf(name, requested),
    }
}

pub(crate) fn produktkategori_all_member(requested: &[String]) -> cellset::MemberConfig {
    cellset::MemberConfig {
        hierarchy: PRODUKTKATEGORI_HIER.into(),
        u_name: PRODUKTKATEGORI_ALL_U.into(),
        caption: "All".into(),
        l_name: PRODUKTKATEGORI_ALL_L.into(),
        l_num: 0,
        display_info: 5,
        dim_props: produktkategori_dim_props_all(requested),
    }
}

pub(crate) fn produktkategori_leaf_members_from(names: &[String], requested: &[String]) -> Vec<cellset::MemberConfig> {
    names
        .iter()
        .map(|name| produktkategori_leaf_member(name, requested))
        .collect()
}

// ---- cell constructors ----

pub(crate) fn measurement_cell(ordinal: u32, value: f64) -> cellset::CellConfig {
    let fmt = if value.fract() == 0.0 {
        format!("{:.0}", value)
    } else {
        format!("{:.2}", value)
    };
    cellset::CellConfig {
        ordinal,
        value,
        fmt_value: format!("{} SEK", fmt),
        format_string: "#,##0.00 SEK".into(),
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
    measures_member("[Measures].[Total Försäljning]", "Total Försäljning (SEK)")
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

pub(crate) fn render_response(axes: Vec<cellset::AxisConfig>, cells: Vec<cellset::CellConfig>, cell_props: &[String]) -> String {
    let resp = cellset::CellsetResponse {
        cube_name: "Model".into(),
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
        tuples: vec![cellset::TupleConfig { members: vec![measures_total_member()] }],
    }
}

pub(crate) fn single_member_axis(name: &str, hierarchy: cellset::HierarchyConfig, member: cellset::MemberConfig) -> cellset::AxisConfig {
    cellset::AxisConfig {
        name: name.into(),
        hierarchies: vec![hierarchy],
        tuples: vec![cellset::TupleConfig { members: vec![member] }],
    }
}

pub(crate) fn member_list_axis(name: &str, hierarchy: cellset::HierarchyConfig, members: Vec<cellset::MemberConfig>) -> cellset::AxisConfig {
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
    query.axis_dimensions.first().map(|s| s.as_str()).unwrap_or("Produktkategori")
}

pub(crate) fn leaf_member_for(dim: &str, name: &str, requested: &[String]) -> cellset::MemberConfig {
    match dim {
        "Region" => region_leaf_member(name, requested),
        _ => produktkategori_leaf_member(name, requested),
    }
}

pub(crate) fn all_member_for(dim: &str, requested: &[String]) -> cellset::MemberConfig {
    match dim {
        "Region" => region_all_member(requested),
        _ => produktkategori_all_member(requested),
    }
}

pub(crate) fn hierarchy_for(dim: &str, requested: &[String]) -> cellset::HierarchyConfig {
    match dim {
        "Region" => region_hierarchy(requested),
        _ => produktkategori_hierarchy(requested),
    }
}

pub(crate) fn leaf_members_from(dim: &str, names: &[String], requested: &[String]) -> Vec<cellset::MemberConfig> {
    match dim {
        "Region" => region_leaf_members_from(names, requested),
        _ => produktkategori_leaf_members_from(names, requested),
    }
}

// ---- slicer helpers ----

pub(crate) fn filter_members_for(dim: &str, filters: &[DimensionFilter]) -> Vec<String> {
    filters.iter()
        .find(|f| f.dimension == dim)
        .map(|f| f.members.clone())
        .unwrap_or_default()
}

/// All cube dimensions in ordinal order (the order they must appear on SlicerAxis).
pub(crate) const ALL_DIMS: &[&str] = &["Measures", "Produktkategori", "Region"];

/// Build a SlicerAxis that includes every cube dimension not on the visible axis,
/// in stable metadata-ordinal order (Measures, Produktkategori, Region).
pub(crate) fn full_slicer_axis(query: &SemanticQuery) -> cellset::AxisConfig {
    let mut hierarchies: Vec<cellset::HierarchyConfig> = Vec::new();
    let mut members: Vec<cellset::MemberConfig> = Vec::new();

    for &dim in ALL_DIMS {
        if query.axis_dimensions.contains(&dim.to_string()) { continue; }

        if dim == "Measures" {
            hierarchies.push(measures_hierarchy());
            members.push(measures_total_member());
            continue;
        }

        hierarchies.push(hierarchy_for(dim, &[]));

        let slc = query.slicers.iter().find(|s| s.dimension == dim);
        if slc.map(|s| s.is_all).unwrap_or(true) {
            members.push(all_member_for(dim, &[]));
        } else {
            let dim_members = filter_members_for(dim, &query.filters);
            for name in &dim_members {
                members.push(leaf_member_for(dim, name, &[]));
            }
        }
    }

    cellset::AxisConfig {
        name: "SlicerAxis".into(),
        hierarchies,
        tuples: vec![cellset::TupleConfig { members }],
    }
}
