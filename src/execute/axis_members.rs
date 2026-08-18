use crate::backend::QueryBackend;
/// Dimension/member/cell/slicer helpers for cellset responses.
///
/// Provides member constructors, hierarchy builders, axis assembly helpers,
/// and cell constructors.
/// Consumed by `execute_builders` for all cellset response construction.
use crate::cellset;
use crate::mdx_semantic::{DimensionFilter, SemanticQuery, includes_prop};
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

fn dim_props_leaf(
    dim: &crate::engine::model::DimensionDef,
    name: &str,
    member_key: &str,
    requested: &[String],
    parent_uname: Option<&str>,
) -> Vec<(String, String)> {
    filter_dim_props(
        vec![
            (
                "PARENT_UNIQUE_NAME".into(),
                parent_uname
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| dim.all_member_unique_name()),
            ),
            ("HIERARCHY_UNIQUE_NAME".into(), dim.hierarchy_unique_name()),
            ("MEMBER_NAME".into(), name.to_string()),
            ("MEMBER_KEY".into(), member_key.to_string()),
            ("MEMBER_TYPE".into(), "1".into()),
            ("MEMBER_VALUE".into(), name.to_string()),
            ("PARENT_LEVEL".into(), "0".into()),
            ("PARENT_COUNT".into(), "1".into()),
        ],
        requested,
    )
}

fn dim_props_all<B: QueryBackend + ?Sized>(
    dim: &crate::engine::model::DimensionDef,
    requested: &[String],
    _backend: &B,
) -> Vec<(String, String)> {
    filter_dim_props(
        vec![
            ("HIERARCHY_UNIQUE_NAME".into(), dim.hierarchy_unique_name()),
            ("MEMBER_NAME".into(), "All".into()),
            ("MEMBER_KEY".into(), "All".into()),
            ("MEMBER_TYPE".into(), "2".into()),
            ("MEMBER_VALUE".into(), "All".into()),
            ("PARENT_LEVEL".into(), "0".into()),
            ("PARENT_COUNT".into(), "0".into()),
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
    drilldown_level: Option<usize>,
    parent_uname: Option<&str>,
) -> cellset::MemberConfig {
    // A member at a deeper level needs a key unique within the hierarchy, so
    // prefix its ancestor path (e.g. a quarter under 2026 is &[2026]&[1]).
    let member_key = parent_uname
        .and_then(key_from_member_uname)
        .map(|path| format!("{path}|{name}"))
        .unwrap_or_else(|| name.to_string());
    let (u_name, l_name, l_num) =
        if let (Some(level_idx), true) = (drilldown_level, !dim.levels.is_empty()) {
            if let Some(level) = dim.levels.get(level_idx) {
                (
                    format!(
                        "{}.[{}].{}",
                        dim.hierarchy_unique_name(),
                        level.name,
                        member_key_suffix(&member_key)
                    ),
                    format!("{}.[{}]", dim.hierarchy_unique_name(), level.name),
                    (level_idx + 1) as i32,
                )
            } else {
                (
                    format!("{}.&amp;[{}]", dim.hierarchy_unique_name(), name),
                    dim.leaf_level_unique_name(),
                    1,
                )
            }
        } else {
            (
                format!("{}.&amp;[{}]", dim.hierarchy_unique_name(), name),
                dim.leaf_level_unique_name(),
                1,
            )
        };
    let cc = dim.children_cardinality_at(drilldown_level);
    cellset::MemberConfig {
        hierarchy: dim.hierarchy_unique_name(),
        u_name,
        caption: name.to_string(),
        l_name,
        l_num,
        display_info: if cc > 0 { 131075 } else { 3 },
        children_cardinality: cc,
        dim_props: dim_props_leaf(dim, name, &member_key, requested, parent_uname),
    }
}

/// Extract the key path from a member UName (compound aware), e.g.
/// `[Date].[Date].[Year].&[2026]` -> `2026`.
fn key_from_member_uname(uname: &str) -> Option<String> {
    let start = uname.rfind(".&amp;[")? + 7;
    let mut rest = &uname[start..];
    let mut parts = Vec::new();
    loop {
        let end = rest.find(']')?;
        parts.push(rest[..end].to_string());
        rest = &rest[end + 1..];
        if let Some(next) = rest.strip_prefix("&amp;[") {
            rest = next;
        } else {
            break;
        }
    }
    Some(parts.join("|"))
}

fn member_key_suffix(key: &str) -> String {
    key.split('|')
        .map(|part| format!("&amp;[{}]", part))
        .collect()
}

fn all_member_for_dim<B: QueryBackend + ?Sized>(
    dim: &crate::engine::model::DimensionDef,
    requested: &[String],
    backend: &B,
) -> cellset::MemberConfig {
    let cc = dim.levels.first().map(|l| l.cardinality).unwrap_or(0);
    cellset::MemberConfig {
        hierarchy: dim.hierarchy_unique_name(),
        u_name: dim.all_member_unique_name(),
        caption: "All".into(),
        l_name: dim.all_level_unique_name(),
        l_num: 0,
        display_info: if cc > 0 { 131075 } else { 5 },
        children_cardinality: cc,
        dim_props: dim_props_all(dim, requested, backend),
    }
}

fn leaf_members_from_dim(
    dim: &crate::engine::model::DimensionDef,
    names: &[String],
    requested: &[String],
    drilldown_level: Option<usize>,
    parent_uname: Option<&str>,
) -> Vec<cellset::MemberConfig> {
    names
        .iter()
        .map(|name| leaf_member_for_dim(dim, name, requested, drilldown_level, parent_uname))
        .collect()
}

fn default_measure() -> &'static crate::engine::model::MeasureDef {
    let project = proxy_project::project();
    let id = project.model.default_measure_id().unwrap_or_else(|| {
        project
            .model
            .measures
            .first()
            .map(|m| m.id.clone())
            .unwrap_or_default()
    });
    project.model.meas_def(&id)
}

fn measure_by_id(measure_id: &str) -> &crate::engine::model::MeasureDef {
    proxy_project::project().model.meas_def(measure_id)
}

pub(crate) fn measure_id_for_query(query: &SemanticQuery) -> String {
    let project = proxy_project::project();
    query
        .measure
        .as_deref()
        .and_then(|name| {
            project
                .model
                .measures
                .iter()
                .find(|m| m.id == name || m.caption == name || m.display_name == name)
        })
        .map(|m| m.id.clone())
        .or_else(|| project.model.default_measure_id())
        .unwrap_or_else(|| {
            project
                .model
                .measures
                .first()
                .map(|m| m.id.clone())
                .unwrap_or_default()
        })
}

// ---- cell constructors ----

pub(crate) fn measurement_cell_for(
    ordinal: u32,
    value: f64,
    measure_id: &str,
) -> cellset::CellConfig {
    measurement_cell_for_measure(ordinal, value, measure_by_id(measure_id))
}

pub(crate) fn measurement_cell_for_query(
    query: &SemanticQuery,
    ordinal: u32,
    value: f64,
) -> cellset::CellConfig {
    let measure_id = measure_id_for_query(query);
    measurement_cell_for(ordinal, value, &measure_id)
}

fn measurement_cell_for_measure(
    ordinal: u32,
    value: f64,
    m: &crate::engine::model::MeasureDef,
) -> cellset::CellConfig {
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
        string_value: None,
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
        string_value: None,
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
        children_cardinality: 0,
        dim_props: vec![],
    }
}

pub(crate) fn measures_total_member() -> cellset::MemberConfig {
    let m = default_measure();
    measures_member(&m.measure_unique_name(), &m.display_name)
}

pub(crate) fn measures_total_member_for_query(query: &SemanticQuery) -> cellset::MemberConfig {
    let measure_id = measure_id_for_query(query);
    let m = measure_by_id(&measure_id);
    measures_member(&m.measure_unique_name(), &m.display_name)
}

pub(crate) fn cchildren_member() -> cellset::MemberConfig {
    measures_member("[Measures].[cChildren]", "cChildren")
}

// ---- axis helpers ----

pub(crate) fn tuples_from_members(
    members: Vec<cellset::MemberConfig>,
) -> Vec<cellset::TupleConfig> {
    members
        .into_iter()
        .map(|member| cellset::TupleConfig {
            members: vec![member],
        })
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
        include_cell_ordinal: includes_prop(cell_props, "CELL_ORDINAL"),
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

pub(crate) fn measures_axis_for_query(query: &SemanticQuery) -> cellset::AxisConfig {
    cellset::AxisConfig {
        name: "Axis0".into(),
        hierarchies: vec![measures_hierarchy()],
        tuples: vec![cellset::TupleConfig {
            members: vec![measures_total_member_for_query(query)],
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
        tuples: vec![cellset::TupleConfig {
            members: vec![member],
        }],
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

pub(crate) fn empty_member_list_axis(
    name: &str,
    hierarchy: cellset::HierarchyConfig,
) -> cellset::AxisConfig {
    cellset::AxisConfig {
        name: name.into(),
        hierarchies: vec![hierarchy],
        tuples: vec![],
    }
}

// ---- dimension dispatch helpers ----

pub(crate) fn row_dim(query: &SemanticQuery) -> &str {
    if let Some(dim) = query.axis_dimensions.first() {
        return dim.as_str();
    }
    if let Some(dim) = query.row_dimension.as_deref() {
        return dim;
    }
    ""
}

pub(crate) fn leaf_member_for(
    dim: &str,
    name: &str,
    requested: &[String],
) -> cellset::MemberConfig {
    match dim_def(dim) {
        Some(d) => leaf_member_for_dim(d, name, requested, None, None),
        None => unknown_dim_member(dim, name),
    }
}

/// Leaf member for a specific hierarchy level (e.g. `[Date].[Date].[Year].&[2024]`).
pub(crate) fn leaf_member_for_level(
    dim: &str,
    name: &str,
    requested: &[String],
    level_name: Option<&str>,
) -> cellset::MemberConfig {
    let d = match dim_def(dim) {
        Some(d) => d,
        None => return unknown_dim_member(dim, name),
    };
    let level_idx = level_name.and_then(|ln| d.levels.iter().position(|l| l.name == ln));
    leaf_member_for_dim(d, name, requested, level_idx, None)
}

pub(crate) fn all_member_for_with_backend<B: QueryBackend + ?Sized>(
    dim: &str,
    requested: &[String],
    backend: &B,
) -> cellset::MemberConfig {
    match dim_def(dim) {
        Some(d) => all_member_for_dim(d, requested, backend),
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

pub(crate) fn leaf_members_from(
    dim: &str,
    names: &[String],
    requested: &[String],
    drilldown_level: Option<usize>,
    parent_uname: Option<&str>,
) -> Vec<cellset::MemberConfig> {
    match dim_def(dim) {
        Some(d) => leaf_members_from_dim(d, names, requested, drilldown_level, parent_uname),
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
        children_cardinality: 0,
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

pub(crate) fn full_slicer_axis_with_backend<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    backend: &B,
) -> cellset::AxisConfig {
    let project = proxy_project::project();
    let mut hierarchies: Vec<cellset::HierarchyConfig> = Vec::new();
    let mut members: Vec<cellset::MemberConfig> = Vec::new();

    // Measures always appear first on the slicer axis.
    hierarchies.push(measures_hierarchy());
    members.push(measures_total_member_for_query(query));

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
            members.push(all_member_for_dim(dim, &[], backend));
        } else {
            let dim_members = filter_members_for(&dim.id, &query.filters);
            for name in &dim_members {
                members.push(leaf_member_for_dim(dim, name, &[], None, None));
            }
        }
    }

    cellset::AxisConfig {
        name: "SlicerAxis".into(),
        hierarchies,
        tuples: vec![cellset::TupleConfig { members }],
    }
}

/// Slicer axis with only the dimension All/leaf members — no measure. Used for
/// multi-measure queries where the measures live on Axis0.
pub(crate) fn dims_only_slicer_axis_with_backend<B: QueryBackend + ?Sized>(
    query: &SemanticQuery,
    backend: &B,
) -> cellset::AxisConfig {
    let project = proxy_project::project();
    let mut hierarchies: Vec<cellset::HierarchyConfig> = Vec::new();
    let mut members: Vec<cellset::MemberConfig> = Vec::new();

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
            members.push(all_member_for_dim(dim, &[], backend));
        } else {
            let dim_members = filter_members_for(&dim.id, &query.filters);
            for name in &dim_members {
                members.push(leaf_member_for_dim(dim, name, &[], None, None));
            }
        }
    }

    cellset::AxisConfig {
        name: "SlicerAxis".into(),
        hierarchies,
        tuples: vec![cellset::TupleConfig { members }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::model::LevelDef;

    fn date_dim_with_levels() -> crate::engine::model::DimensionDef {
        crate::engine::model::DimensionDef {
            id: "Date".into(),
            physical_field: "full_date".into(),
            table_name: Some("date_dim".into()),
            shared: false,
            caption: "Date".into(),
            description: String::new(),
            visible: true,
            ordinal: 5,
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
        }
    }

    #[test]
    fn leaf_member_di_drillable() {
        let d = date_dim_with_levels();
        let m = leaf_member_for_dim(&d, "2020", &[], Some(0), None);
        assert_eq!(m.display_info, 131075);
        assert_eq!(m.children_cardinality, 44);
    }

    #[test]
    fn leaf_member_di_leaf() {
        let d = date_dim_with_levels();
        let m = leaf_member_for_dim(&d, "1", &[], Some(2), None);
        assert_eq!(m.display_info, 3);
        assert_eq!(m.children_cardinality, 0);
    }

    #[test]
    fn leaf_member_uname_level_qualified() {
        let d = date_dim_with_levels();
        let m = leaf_member_for_dim(&d, "2020", &[], Some(0), None);
        assert_eq!(m.u_name, "[Date].[Date].[Year].&amp;[2020]");
    }

    #[test]
    fn leaf_member_lname_level_qualified() {
        let d = date_dim_with_levels();
        let m = leaf_member_for_dim(&d, "2020", &[], Some(0), None);
        assert_eq!(m.l_name, "[Date].[Date].[Year]");
    }

    #[test]
    fn leaf_member_parent_uname_override() {
        let d = date_dim_with_levels();
        let req: Vec<String> = vec!["PARENT_UNIQUE_NAME".into()];
        let m = leaf_member_for_dim(
            &d,
            "1",
            &req,
            Some(1),
            Some("[Date].[Date].[Year].&amp;[2024]"),
        );
        let pun = m.dim_props.iter().find(|(k, _)| k == "PARENT_UNIQUE_NAME");
        assert!(pun.is_some(), "should have PARENT_UNIQUE_NAME prop");
        assert!(
            pun.unwrap().1.contains("Year"),
            "should contain Year in parent: {:?}",
            pun
        );
    }

    #[test]
    fn leaf_member_parent_uname_default() {
        let d = date_dim_with_levels();
        let req: Vec<String> = vec!["PARENT_UNIQUE_NAME".into()];
        let m = leaf_member_for_dim(&d, "2020", &req, Some(0), None);
        let pun = m.dim_props.iter().find(|(k, _)| k == "PARENT_UNIQUE_NAME");
        assert!(pun.is_some());
        assert_eq!(pun.unwrap().1, "[Date].[Date].[All]");
    }
}
