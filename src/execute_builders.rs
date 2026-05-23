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
    includes_prop, SemanticQuery, SemanticQueryKind,
    PRODUKTKATEGORI_HIER, PRODUKTKATEGORI_ALL_U, PRODUKTKATEGORI_ALL_L,
    PRODUKTKATEGORI_LEAF_L, MEASURES_HIER, MEASURES_LEVEL,
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

// ---- cellset response builders ----

fn build_slicer_only(cell_props: &[String], filters: &[String]) -> String {
    let total = Backend::get().total_for_categories(filters);
    render_response(
        vec![slicer_axis_with_members(vec![measures_hierarchy()], vec![measures_total_member()])],
        vec![measurement_cell(0, total)],
        cell_props,
    )
}

fn build_drilldown(dim_props: &[String], cell_props: &[String], filters: &[String]) -> String {
    let data = Backend::get().sales_for_categories(filters);
    let members = produktkategori_leaf_members_from(
        &data.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(),
        dim_props,
    );

    let mut cells = Vec::new();
    for (i, (_name, value)) in data.iter().enumerate() {
        cells.push(measurement_cell(i as u32, *value));
    }

    render_response(
        vec![
            member_list_axis("Axis0", produktkategori_hierarchy(dim_props), members),
            slicer_axis_with_members(vec![measures_hierarchy()], vec![measures_total_member()]),
        ],
        cells,
        cell_props,
    )
}

fn build_measure_by_category(dim_props: &[String], cell_props: &[String], filters: &[String]) -> String {
    let data = Backend::get().sales_for_categories(filters);
    let axis1_members = produktkategori_leaf_members_from(
        &data.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(),
        dim_props,
    );
    let mut cells = Vec::new();
    for (i, (_name, value)) in data.iter().enumerate() {
        cells.push(measurement_cell(i as u32, *value));
    }

    render_response(
        vec![
            measures_axis(),
            member_list_axis("Axis1", produktkategori_hierarchy(dim_props), axis1_members),
            empty_slicer_axis(),
        ],
        cells,
        cell_props,
    )
}

fn build_slicer_all_and_measure(dim_props: &[String], cell_props: &[String], filters: &[String]) -> String {
    let total = Backend::get().total_for_categories(filters);
    render_response(
        vec![slicer_axis_with_members(
            vec![produktkategori_hierarchy(dim_props), measures_hierarchy()],
            vec![produktkategori_all_member(dim_props), measures_total_member()],
        )],
        vec![measurement_cell(0, total)],
        cell_props,
    )
}

fn build_all_level_members(dim_props: &[String], cell_props: &[String]) -> String {
    let total = Backend::get().total_sales();
    render_response(
        vec![
            single_member_axis("Axis0", produktkategori_hierarchy(dim_props), produktkategori_all_member(dim_props)),
            empty_slicer_axis(),
        ],
        vec![measurement_cell(0, total)],
        cell_props,
    )
}

fn build_leaf_level_members(dim_props: &[String], cell_props: &[String], filters: &[String]) -> String {
    let data = Backend::get().sales_for_categories(filters);
    let members = produktkategori_leaf_members_from(
        &data.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(),
        dim_props,
    );
    let mut cells = Vec::new();
    for (i, (_name, value)) in data.iter().enumerate() {
        cells.push(measurement_cell(i as u32, *value));
    }

    render_response(
        vec![
            member_list_axis("Axis0", produktkategori_hierarchy(dim_props), members),
            empty_slicer_axis(),
        ],
        cells,
        cell_props,
    )
}

fn build_leaf_children_empty(dim_props: &[String], cell_props: &[String]) -> String {
    render_response(
        vec![
            empty_member_list_axis("Axis0", produktkategori_hierarchy(dim_props)),
            empty_slicer_axis(),
        ],
        vec![],
        cell_props,
    )
}

fn build_measure_children_empty(cell_props: &[String]) -> String {
    render_response(
        vec![
            empty_member_list_axis("Axis0", measures_hierarchy()),
            empty_slicer_axis(),
        ],
        vec![],
        cell_props,
    )
}

fn build_cchildren_for_all(dim_props: &[String], cell_props: &[String]) -> String {
    let count = Backend::get().category_count();
    render_response(
        vec![
            single_member_axis("Axis0", produktkategori_hierarchy(dim_props), produktkategori_all_member(dim_props)),
            single_member_axis("Axis1", measures_hierarchy(), cchildren_member()),
            empty_slicer_axis(),
        ],
        vec![count_cell(0, count)],
        cell_props,
    )
}

fn build_cchildren_for_leaf_product(dim_props: &[String], cell_props: &[String], name: &str) -> String {
    let leaf = produktkategori_leaf_member(name, dim_props);
    let all = produktkategori_all_member(dim_props);
    let real_count = Backend::get().category_count();
    render_response(
        vec![
            member_list_axis("Axis0", produktkategori_hierarchy(dim_props), vec![all, leaf]),
            single_member_axis("Axis1", measures_hierarchy(), cchildren_member()),
            empty_slicer_axis(),
        ],
        vec![count_cell(0, real_count), count_cell(1, 0)],
        cell_props,
    )
}

fn build_cchildren_for_measures(_dim_props: &[String], cell_props: &[String]) -> String {
    render_response(
        vec![
            single_member_axis("Axis0", measures_hierarchy(), measures_total_member()),
            single_member_axis("Axis1", measures_hierarchy(), cchildren_member()),
            empty_slicer_axis(),
        ],
        vec![count_cell(0, 0)],
        cell_props,
    )
}

// ---- public API consumed by execute.rs dispatch ----

pub fn execute_semantic_query(query: &SemanticQuery) -> String {
    let filters = &query.category_filters;
    match query.kind {
        SemanticQueryKind::ChildrenCountForAll => {
            build_cchildren_for_all(&query.dim_props, &query.cell_props)
        }
        SemanticQueryKind::ChildrenCountLeafProduct => {
            let name = query.cchildren_leaf_name.as_deref().unwrap_or("");
            build_cchildren_for_leaf_product(&query.dim_props, &query.cell_props, name)
        }
        SemanticQueryKind::ChildrenCountMeasures => {
            build_cchildren_for_measures(&query.dim_props, &query.cell_props)
        }
        SemanticQueryKind::SlicerAllAndMeasure => {
            build_slicer_all_and_measure(&query.dim_props, &query.cell_props, filters)
        }
        SemanticQueryKind::MeasureChildrenEmpty => {
            build_measure_children_empty(&query.cell_props)
        }
        SemanticQueryKind::LeafChildrenEmpty => {
            build_leaf_children_empty(&query.dim_props, &query.cell_props)
        }
        SemanticQueryKind::AllLevelMembers => {
            build_all_level_members(&query.dim_props, &query.cell_props)
        }
        SemanticQueryKind::LeafLevelMembers => {
            build_leaf_level_members(&query.dim_props, &query.cell_props, filters)
        }
        SemanticQueryKind::MeasureByCategory => {
            build_measure_by_category(&query.dim_props, &query.cell_props, filters)
        }
        SemanticQueryKind::DrilldownCategories => {
            build_drilldown(&query.dim_props, &query.cell_props, filters)
        }
        SemanticQueryKind::SlicerOnly => {
            build_slicer_only(&query.cell_props, filters)
        }
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
