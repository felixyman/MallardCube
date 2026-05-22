use crate::response::wrap_in_soap_envelope;
use crate::cellset;

pub fn get_empty_execute_response() -> String {
    wrap_in_soap_envelope(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:empty"/>
      </return>
    </ExecuteResponse>"#,
    )
}

fn is_dax(statement: &str) -> bool {
    let trimmed = statement.trim_start();
    let upper = trimmed.to_uppercase();
    upper.starts_with("EVALUATE") || upper.starts_with("DEFINE")
}

fn is_mdx_select(mdx: &str) -> bool {
    mdx.trim_start().to_uppercase().starts_with("SELECT")
}

pub fn get_execute_statement_response(statement: &str) -> String {
    if is_dax(statement) {
        get_execute_dax_response(statement)
    } else if is_mdx_select(statement) {
        get_execute_cellset_response(statement)
    } else {
        get_execute_mdx_response(statement)
    }
}

// ---- helpers for building cellset data ----

const PRODUKTKATEGORI_HIER: &str = "[Produktkategori].[Produktkategori]";
const PRODUKTKATEGORI_ALL_U: &str = "[Produktkategori].[Produktkategori].[All]";
const PRODUKTKATEGORI_ALL_L: &str = "[Produktkategori].[Produktkategori].[(All)]";
const PRODUKTKATEGORI_LEAF_L: &str = "[Produktkategori].[Produktkategori].[Produktkategori]";
const MEASURES_HIER: &str = "[Measures]";
const MEASURES_LEVEL: &str = "[Measures].[MeasuresLevel]";

const PRODUKTKATEGORI_PROP_NAMES: &[&str] = &[
    "PARENT_UNIQUE_NAME",
    "HIERARCHY_UNIQUE_NAME",
    "MEMBER_NAME",
    "MEMBER_KEY",
    "MEMBER_TYPE",
    "MEMBER_VALUE",
    "PARENT_LEVEL",
    "PARENT_COUNT",
    "CHILDREN_CARDINALITY",
];

fn clause_contents<'a>(mdx: &'a str, keyword: &str, terminators: &[&str]) -> Option<&'a str> {
    let upper = mdx.to_uppercase();
    let keyword_upper = keyword.to_uppercase();
    let start = upper.find(&keyword_upper)? + keyword_upper.len();

    let mut end = mdx.len();
    for term in terminators {
        let term_upper = term.to_uppercase();
        if let Some(idx) = upper[start..].find(&term_upper) {
            end = end.min(start + idx);
        }
    }

    Some(mdx[start..end].trim())
}

fn parse_dimension_properties(mdx: &str) -> Vec<String> {
    let Some(raw) = clause_contents(
        mdx,
        "DIMENSION PROPERTIES",
        &[" ON COLUMNS", " ON ROWS", " FROM ", " CELL PROPERTIES"],
    ) else {
        return vec![];
    };

    let mut props = Vec::new();
    for token in raw.split(',') {
        let token_upper = token.trim().to_uppercase();
        for prop in PRODUKTKATEGORI_PROP_NAMES {
            if token_upper.ends_with(prop) {
                if !props.iter().any(|p| p == prop) {
                    props.push((*prop).to_string());
                }
                break;
            }
        }
    }
    props
}

fn parse_cell_properties(mdx: &str) -> Vec<String> {
    let Some(raw) = clause_contents(mdx, "CELL PROPERTIES", &[]) else {
        return vec![];
    };

    raw.split(',')
        .map(|token| token.trim().to_uppercase())
        .filter(|token| !token.is_empty())
        .collect()
}

fn includes_prop(props: &[String], name: &str) -> bool {
    props.iter().any(|prop| prop == name)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SemanticQueryKind {
    ChildrenCountForAll,
    SlicerAllAndMeasure,
    MeasureChildrenEmpty,
    LeafChildrenEmpty,
    AllLevelMembers,
    LeafLevelMembers,
    MeasureByCategory,
    DrilldownCategories,
    SlicerOnly,
}

#[derive(Debug, Clone)]
struct SemanticQuery {
    kind: SemanticQueryKind,
    dim_props: Vec<String>,
    cell_props: Vec<String>,
}

fn semantic_query_from_mdx(mdx: &str) -> SemanticQuery {
    let upper = mdx.to_uppercase();
    let dim_props = parse_dimension_properties(mdx);
    let cell_props = parse_cell_properties(mdx);
    let has_axes = upper.contains("ON COLUMNS") || upper.contains("ON ROWS");
    let has_rows = upper.contains("ON ROWS");
    let has_cols = upper.contains("ON COLUMNS");
    let has_product = mdx.contains("[Produktkategori]");
    let has_measures = mdx.contains("[Measures]");
    let is_drilldown = has_product && (mdx.contains("DrilldownLevel") || mdx.contains(".Members"));

    let kind = if mdx.contains("WITH MEMBER [Measures].cChildren") {
        SemanticQueryKind::ChildrenCountForAll
    } else if mdx.contains("WHERE ([Produktkategori].[Produktkategori].[All],[Measures].[Total Försäljning])") {
        SemanticQueryKind::SlicerAllAndMeasure
    } else if mdx.contains("AddCalculatedMembers({[Measures].[Total Försäljning].Children})") {
        SemanticQueryKind::MeasureChildrenEmpty
    } else if mdx.contains("AddCalculatedMembers({[Produktkategori].[Produktkategori].&[") && mdx.contains("].Children})") {
        SemanticQueryKind::LeafChildrenEmpty
    } else if mdx.contains("AddCalculatedMembers({[Produktkategori].[Produktkategori].[(All)].Members})") {
        SemanticQueryKind::AllLevelMembers
    } else if mdx.contains("AddCalculatedMembers({[Produktkategori].[Produktkategori].[Produktkategori].Members})") {
        SemanticQueryKind::LeafLevelMembers
    } else if has_rows && has_cols && has_product && has_measures {
        SemanticQueryKind::MeasureByCategory
    } else if is_drilldown {
        SemanticQueryKind::DrilldownCategories
    } else if !has_axes {
        SemanticQueryKind::SlicerOnly
    } else {
        SemanticQueryKind::SlicerOnly
    };

    SemanticQuery {
        kind,
        dim_props,
        cell_props,
    }
}

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
    filter_dim_props(vec![
        ("HIERARCHY_UNIQUE_NAME".into(), PRODUKTKATEGORI_HIER.into()),
        ("MEMBER_NAME".into(), "All".into()),
        ("MEMBER_KEY".into(), "All".into()),
        ("MEMBER_TYPE".into(), "2".into()),
        ("MEMBER_VALUE".into(), "All".into()),
        ("PARENT_LEVEL".into(), "0".into()),
        ("PARENT_COUNT".into(), "0".into()),
        ("CHILDREN_CARDINALITY".into(), "4".into()),
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

fn produktkategori_leaf_members(requested: &[String]) -> Vec<cellset::MemberConfig> {
    ["Kategori A", "Kategori B", "Kategori C", "Kategori D"]
        .iter()
        .map(|name| produktkategori_leaf_member(name, requested))
        .collect()
}

fn measurement_cell(ordinal: u32) -> cellset::CellConfig {
    cellset::CellConfig {
        ordinal,
        value: 1250000.5,
        fmt_value: "1,250,000.50 SEK".into(),
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

/// Shape 1: slicer-only (e.g. dimension removed, measure stays).
/// `SELECT FROM [Model] WHERE ([Measures]...) CELL PROPERTIES ...`
fn build_slicer_only(cell_props: &[String]) -> String {
    render_response(
        vec![slicer_axis_with_members(vec![measures_hierarchy()], vec![measures_total_member()])],
        vec![measurement_cell(0)],
        cell_props,
    )
}

/// Shape 2: hierarchy drilldown (e.g. first drag of Produktkategori to Rows).
/// `SELECT ... DrilldownLevel({[All]}) ... ON COLUMNS ...`
fn build_drilldown(dim_props: &[String], cell_props: &[String]) -> String {
    let members = produktkategori_leaf_members(dim_props);

    let mut cells = Vec::new();
    for i in 0..members.len() {
        cells.push(measurement_cell(i as u32));
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

/// Shape 3: measure on columns + Produktkategori on rows.
/// Excel pivot with one Values measure and one Rows hierarchy.
fn build_measure_by_category(dim_props: &[String], cell_props: &[String]) -> String {
    let axis1_members = produktkategori_leaf_members(dim_props);
    let mut cells = Vec::new();
    for i in 0..axis1_members.len() {
        cells.push(measurement_cell(i as u32));
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

fn build_slicer_all_and_measure(dim_props: &[String], cell_props: &[String]) -> String {
    render_response(
        vec![slicer_axis_with_members(
            vec![produktkategori_hierarchy(dim_props), measures_hierarchy()],
            vec![produktkategori_all_member(dim_props), measures_total_member()],
        )],
        vec![measurement_cell(0)],
        cell_props,
    )
}

fn build_all_level_members(dim_props: &[String], cell_props: &[String]) -> String {
    render_response(
        vec![
            single_member_axis("Axis0", produktkategori_hierarchy(dim_props), produktkategori_all_member(dim_props)),
            slicer_axis_with_members(vec![measures_hierarchy()], vec![measures_total_member()]),
        ],
        vec![measurement_cell(0)],
        cell_props,
    )
}

fn build_leaf_level_members(dim_props: &[String], cell_props: &[String]) -> String {
    let members = produktkategori_leaf_members(dim_props);
    let mut cells = Vec::new();
    for i in 0..members.len() {
        cells.push(measurement_cell(i as u32));
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

fn build_leaf_children_empty(dim_props: &[String], cell_props: &[String]) -> String {
    render_response(
        vec![
            empty_member_list_axis("Axis0", produktkategori_hierarchy(dim_props)),
            slicer_axis_with_members(vec![measures_hierarchy()], vec![measures_total_member()]),
        ],
        vec![],
        cell_props,
    )
}

fn build_measure_children_empty(cell_props: &[String]) -> String {
    render_response(
        vec![
            empty_member_list_axis("Axis0", measures_hierarchy()),
            slicer_axis_with_members(vec![produktkategori_hierarchy(&[])], vec![produktkategori_all_member(&[])]),
        ],
        vec![],
        cell_props,
    )
}

fn build_cchildren_for_all(dim_props: &[String], cell_props: &[String]) -> String {
    render_response(
        vec![
            single_member_axis("Axis0", produktkategori_hierarchy(dim_props), produktkategori_all_member(dim_props)),
            single_member_axis("Axis1", measures_hierarchy(), cchildren_member()),
            empty_slicer_axis(),
        ],
        vec![count_cell(0, 4)],
        cell_props,
    )
}

fn execute_semantic_query(query: &SemanticQuery) -> String {
    match query.kind {
        SemanticQueryKind::ChildrenCountForAll => {
            build_cchildren_for_all(&query.dim_props, &query.cell_props)
        }
        SemanticQueryKind::SlicerAllAndMeasure => {
            build_slicer_all_and_measure(&query.dim_props, &query.cell_props)
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
            build_leaf_level_members(&query.dim_props, &query.cell_props)
        }
        SemanticQueryKind::MeasureByCategory => {
            build_measure_by_category(&query.dim_props, &query.cell_props)
        }
        SemanticQueryKind::DrilldownCategories => {
            build_drilldown(&query.dim_props, &query.cell_props)
        }
        SemanticQueryKind::SlicerOnly => {
            build_slicer_only(&query.cell_props)
        }
    }
}

fn get_execute_cellset_response(mdx: &str) -> String {
    let query = semantic_query_from_mdx(mdx);
    execute_semantic_query(&query)
}

fn get_execute_mdx_response(mdx: &str) -> String {
    let has_measures = mdx.contains("Measures") || mdx.contains("measures");
    let measure_name = "Total_Forsaljning";
    let measure_value = if has_measures { "1250000.5" } else { "" };

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

/// Minimal DAX EVALUATE response: returns a single-row rowset with the
/// `Faktatabell[Total Försäljning (SEK)]` measure column.
fn get_execute_dax_response(_dax: &str) -> String {
    // DAX result columns are normally named `'Table'[Column]` — Excel will
    // accept the bracketed form. We use a column name aligned with the
    // measure caption so a drag-to-Values renders the expected number.
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
            <{xname}>1250000.5</{xname}>
          </row>
        </root>
      </return>
    </ExecuteResponse>"#,
        sqlf = col_sql_field,
        xname = col_xml_name,
    );
    wrap_in_soap_envelope(&inner)
}
