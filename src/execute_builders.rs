/// Cellset response builders.
///
/// Converts a `SemanticQuery` (from `mdx_semantic`) into a full
/// mddataset XML response, backed by the current `Backend`.
///
/// Also contains the flat-rowset fallback responses for MDX and DAX.

use crate::response::wrap_in_soap_envelope;
use crate::cellset;
use crate::backend::Backend;
use crate::mdx_semantic::{
    includes_prop, SemanticQuery, SemanticQueryKind, DimensionFilter,
    PRODUKTKATEGORI_HIER, PRODUKTKATEGORI_ALL_U, PRODUKTKATEGORI_ALL_L,
    PRODUKTKATEGORI_LEAF_L, MEASURES_HIER, MEASURES_LEVEL,
    REGION_HIER, REGION_ALL_U, REGION_ALL_L, REGION_LEAF_L,
};

// ---- dimension property helpers ----

fn filter_dim_props(props: Vec<(String, String)>, requested: &[String]) -> Vec<(String, String)> {
    props.into_iter()
        .filter(|(tag, _)| includes_prop(requested, tag))
        .collect()
}

fn filter_dim_prop_decls(
    props: Vec<(String, String, String)>,
    requested: &[String],
) -> Vec<(String, String, String)> {
    props.into_iter()
        .filter(|(tag, _, _)| includes_prop(requested, tag))
        .collect()
}

fn produktkategori_dim_props_leaf(name: &str, requested: &[String]) -> Vec<(String, String)> {
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

fn produktkategori_dim_props_all(requested: &[String]) -> Vec<(String, String)> {
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

fn produktkategori_dim_decls() -> Vec<(String, String, String)> {
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

fn region_dim_props_leaf(name: &str, requested: &[String]) -> Vec<(String, String)> {
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

fn region_dim_props_all(requested: &[String]) -> Vec<(String, String)> {
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

fn region_dim_decls() -> Vec<(String, String, String)> {
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

fn region_hierarchy(requested: &[String]) -> cellset::HierarchyConfig {
    cellset::HierarchyConfig {
        name: REGION_HIER.into(),
        dim_prop_decls: filter_dim_prop_decls(region_dim_decls(), requested),
    }
}

fn region_leaf_member(name: &str, requested: &[String]) -> cellset::MemberConfig {
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

fn region_all_member(requested: &[String]) -> cellset::MemberConfig {
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

fn region_leaf_members_from(names: &[String], requested: &[String]) -> Vec<cellset::MemberConfig> {
    names.iter().map(|name| region_leaf_member(name, requested)).collect()
}

// ---- member/cell/axis constructors ----

fn produktkategori_hierarchy(requested: &[String]) -> cellset::HierarchyConfig {
    cellset::HierarchyConfig {
        name: PRODUKTKATEGORI_HIER.into(),
        dim_prop_decls: filter_dim_prop_decls(produktkategori_dim_decls(), requested),
    }
}

fn measures_hierarchy() -> cellset::HierarchyConfig {
    cellset::HierarchyConfig {
        name: MEASURES_HIER.into(),
        dim_prop_decls: vec![],
    }
}

fn produktkategori_leaf_member(name: &str, requested: &[String]) -> cellset::MemberConfig {
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

fn produktkategori_all_member(requested: &[String]) -> cellset::MemberConfig {
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

fn produktkategori_leaf_members_from(names: &[String], requested: &[String]) -> Vec<cellset::MemberConfig> {
    names
        .iter()
        .map(|name| produktkategori_leaf_member(name, requested))
        .collect()
}

fn measurement_cell(ordinal: u32, value: f64) -> cellset::CellConfig {
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

fn count_cell(ordinal: u32, value: u32) -> cellset::CellConfig {
    cellset::CellConfig {
        ordinal,
        value: value as f64,
        fmt_value: value.to_string(),
        format_string: "0".into(),
        back_color: String::new(),
        fore_color: String::new(),
    }
}

fn measures_member(unique_name: &str, caption: &str) -> cellset::MemberConfig {
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

fn measures_total_member() -> cellset::MemberConfig {
    measures_member("[Measures].[Total Försäljning]", "Total Försäljning (SEK)")
}

fn cchildren_member() -> cellset::MemberConfig {
    measures_member("[Measures].[cChildren]", "cChildren")
}

fn tuples_from_members(members: Vec<cellset::MemberConfig>) -> Vec<cellset::TupleConfig> {
    members
        .into_iter()
        .map(|member| cellset::TupleConfig { members: vec![member] })
        .collect()
}

fn render_response(axes: Vec<cellset::AxisConfig>, cells: Vec<cellset::CellConfig>, cell_props: &[String]) -> String {
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

fn slicer_axis_with_members(
    hierarchies: Vec<cellset::HierarchyConfig>,
    members: Vec<cellset::MemberConfig>,
) -> cellset::AxisConfig {
    cellset::AxisConfig {
        name: "SlicerAxis".into(),
        hierarchies,
        tuples: vec![cellset::TupleConfig { members }],
    }
}

fn empty_slicer_axis() -> cellset::AxisConfig {
    cellset::AxisConfig {
        name: "SlicerAxis".into(),
        hierarchies: vec![],
        tuples: vec![cellset::TupleConfig { members: vec![] }],
    }
}

fn measures_axis() -> cellset::AxisConfig {
    cellset::AxisConfig {
        name: "Axis0".into(),
        hierarchies: vec![measures_hierarchy()],
        tuples: vec![cellset::TupleConfig { members: vec![measures_total_member()] }],
    }
}

fn single_member_axis(name: &str, hierarchy: cellset::HierarchyConfig, member: cellset::MemberConfig) -> cellset::AxisConfig {
    cellset::AxisConfig {
        name: name.into(),
        hierarchies: vec![hierarchy],
        tuples: vec![cellset::TupleConfig { members: vec![member] }],
    }
}

fn member_list_axis(name: &str, hierarchy: cellset::HierarchyConfig, members: Vec<cellset::MemberConfig>) -> cellset::AxisConfig {
    cellset::AxisConfig {
        name: name.into(),
        hierarchies: vec![hierarchy],
        tuples: tuples_from_members(members),
    }
}

fn empty_member_list_axis(name: &str, hierarchy: cellset::HierarchyConfig) -> cellset::AxisConfig {
    cellset::AxisConfig {
        name: name.into(),
        hierarchies: vec![hierarchy],
        tuples: vec![],
    }
}

fn row_dim(query: &SemanticQuery) -> &str {
    query.row_dimension.as_deref().unwrap_or("Produktkategori")
}

fn leaf_member_for(dim: &str, name: &str, requested: &[String]) -> cellset::MemberConfig {
    match dim {
        "Region" => region_leaf_member(name, requested),
        _ => produktkategori_leaf_member(name, requested),
    }
}

fn all_member_for(dim: &str, requested: &[String]) -> cellset::MemberConfig {
    match dim {
        "Region" => region_all_member(requested),
        _ => produktkategori_all_member(requested),
    }
}

fn hierarchy_for(dim: &str, requested: &[String]) -> cellset::HierarchyConfig {
    match dim {
        "Region" => region_hierarchy(requested),
        _ => produktkategori_hierarchy(requested),
    }
}

fn leaf_members_from(dim: &str, names: &[String], requested: &[String]) -> Vec<cellset::MemberConfig> {
    match dim {
        "Region" => region_leaf_members_from(names, requested),
        _ => produktkategori_leaf_members_from(names, requested),
    }
}

fn kat_filter(filters: &[DimensionFilter]) -> Vec<String> {
    filters.iter().find(|f| f.dimension == "Produktkategori")
        .map(|f| f.members.clone()).unwrap_or_default()
}

fn region_filter(filters: &[DimensionFilter]) -> Vec<String> {
    filters.iter().find(|f| f.dimension == "Region")
        .map(|f| f.members.clone()).unwrap_or_default()
}

fn fetch_grouped(row_dim: &str, filters: &[DimensionFilter]) -> Vec<(String, f64)> {
    let backend = Backend::get();
    match row_dim {
        "Region" => backend.grouped_by_region(&kat_filter(filters)),
        _ => backend.grouped_by_produktkategori(&region_filter(filters)),
    }
}

fn fetch_total_with_filters(filters: &[DimensionFilter]) -> f64 {
    Backend::get().total_with_filters(&region_filter(filters), &kat_filter(filters))
}

fn dimension_count(dim: &str) -> u32 {
    let backend = Backend::get();
    match dim {
        "Region" => backend.region_count(),
        _ => backend.category_count(),
    }
}

fn filter_members_for(dim: &str, filters: &[DimensionFilter]) -> Vec<String> {
    filters.iter()
        .find(|f| f.dimension == dim)
        .map(|f| f.members.clone())
        .unwrap_or_default()
}

/// All cube dimensions in ordinal order (the order they must appear on SlicerAxis).
const ALL_DIMS: &[&str] = &["Measures", "Produktkategori", "Region"];

/// Build a SlicerAxis that includes every cube dimension not on the visible axis,
/// in stable metadata-ordinal order (Measures, Produktkategori, Region).
fn full_slicer_axis(query: &SemanticQuery) -> cellset::AxisConfig {
    let row_dim = row_dim(query);
    let mut hierarchies: Vec<cellset::HierarchyConfig> = Vec::new();
    let mut members: Vec<cellset::MemberConfig> = Vec::new();

    for &dim in ALL_DIMS {
        if dim == row_dim { continue; }

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

// ---- cellset response builders ----

fn build_slicer_only(query: &SemanticQuery) -> String {
    let total = fetch_total_with_filters(&query.filters);
    render_response(
        vec![full_slicer_axis(query)],
        vec![measurement_cell(0, total)],
        &query.cell_props,
    )
}

fn build_drilldown(query: &SemanticQuery) -> String {
    let dim = row_dim(query);
    let data = fetch_grouped(dim, &query.filters);
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

fn build_measure_by_category(query: &SemanticQuery) -> String {
    let dim = row_dim(query);
    let data = fetch_grouped(dim, &query.filters);
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

fn slicer_dim(query: &SemanticQuery) -> &str {
    query.filters.first()
        .map(|f| f.dimension.as_str())
        .unwrap_or_else(|| row_dim(query))
}

fn build_slicer_all_and_measure(query: &SemanticQuery) -> String {
    let total = fetch_total_with_filters(&query.filters);
    render_response(
        vec![full_slicer_axis(query)],
        vec![measurement_cell(0, total)],
        &query.cell_props,
    )
}

fn build_all_level_members(query: &SemanticQuery) -> String {
    let dim = row_dim(query);
    let total = Backend::get().total_sales();
    render_response(
        vec![
            single_member_axis("Axis0", hierarchy_for(dim, &query.dim_props), all_member_for(dim, &query.dim_props)),
            full_slicer_axis(query),
        ],
        vec![measurement_cell(0, total)],
        &query.cell_props,
    )
}

fn build_leaf_level_members(query: &SemanticQuery) -> String {
    let dim = row_dim(query);
    let data = fetch_grouped(dim, &query.filters);
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

fn build_leaf_children_empty(query: &SemanticQuery) -> String {
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

fn build_measure_children_empty(query: &SemanticQuery) -> String {
    render_response(
        vec![
            empty_member_list_axis("Axis0", measures_hierarchy()),
            full_slicer_axis(query),
        ],
        vec![],
        &query.cell_props,
    )
}

fn build_cchildren_for_all(query: &SemanticQuery) -> String {
    let dim = row_dim(query);
    let count = dimension_count(dim);
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

fn build_cchildren_for_leaf_product(query: &SemanticQuery, name: &str) -> String {
    let dim = row_dim(query);
    let leaf = leaf_member_for(dim, name, &query.dim_props);
    let all = all_member_for(dim, &query.dim_props);
    let real_count = dimension_count(dim);
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

fn build_cchildren_for_measures(query: &SemanticQuery) -> String {
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
    match query.kind {
        SemanticQueryKind::ChildrenCountForAll => build_cchildren_for_all(query),
        SemanticQueryKind::ChildrenCountLeafProduct => {
            let name = query.cchildren_leaf_name.as_deref().unwrap_or("");
            build_cchildren_for_leaf_product(query, name)
        }
        SemanticQueryKind::ChildrenCountMeasures => build_cchildren_for_measures(query),
        SemanticQueryKind::SlicerAllAndMeasure => build_slicer_all_and_measure(query),
        SemanticQueryKind::MeasureChildrenEmpty => build_measure_children_empty(query),
        SemanticQueryKind::LeafChildrenEmpty => build_leaf_children_empty(query),
        SemanticQueryKind::AllLevelMembers => build_all_level_members(query),
        SemanticQueryKind::LeafLevelMembers => build_leaf_level_members(query),
        SemanticQueryKind::MeasureByCategory => build_measure_by_category(query),
        SemanticQueryKind::DrilldownCategories => build_drilldown(query),
        SemanticQueryKind::SlicerOnly => build_slicer_only(query),
    }
}

pub fn get_execute_cellset_response(mdx: &str) -> String {
    let query = crate::mdx_semantic::semantic_query_from_mdx(mdx);
    execute_semantic_query(&query)
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
