use crate::response::wrap_in_soap_envelope;
use crate::cellset;
use crate::backend::Backend;

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
    let trimmed = mdx.trim_start();
    let upper = trimmed.to_uppercase();
    upper.starts_with("SELECT")
        || (upper.starts_with("WITH") && upper.contains("SELECT "))
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

fn parse_category_filter(mdx: &str) -> Option<String> {
    let start = mdx.find("[Produktkategori].[Produktkategori].")?;
    let rest = &mdx[start..];
    if rest.contains("[Produktkategori].[Produktkategori].[All]") {
        return None;
    }
    if let Some(amp) = rest.find("&amp;[") {
        let begin = amp + 5;
        let end = rest[begin..].find(']')? + begin;
        return Some(rest[begin..end].to_string());
    }
    if let Some(amp) = rest.find("&[") {
        let begin = amp + 2;
        let end = rest[begin..].find(']')? + begin;
        return Some(rest[begin..end].to_string());
    }
    None
}

fn parse_mdx_filters(mdx: &str) -> Vec<String> {
    if let Some(where_filter) = parse_category_filter(mdx) {
        return vec![where_filter];
    }
    let sub_start = match mdx.find("SELECT ({") {
        Some(p) => p,
        None => return vec![],
    };
    let sub_rest = &mdx[sub_start..];
    let sub_end = match sub_rest.find("})") {
        Some(p) => p,
        None => return vec![],
    };
    let members_str = &sub_rest["SELECT ({".len()..sub_end];
    let mut result = Vec::new();
    for member in members_str.split(',') {
        let member = member.trim();
        if let Some(amp_start) = member.find("&[") {
            let begin = amp_start + 2;
            if let Some(end) = member[begin..].find(']') {
                result.push(member[begin..begin + end].to_string());
            }
        } else if let Some(amp_start) = member.find("&amp;[") {
            let begin = amp_start + 5;
            if let Some(end) = member[begin..].find(']') {
                result.push(member[begin..begin + end].to_string());
            }
        }
    }
    result
}

fn cchildren_target_is_measures(mdx: &str) -> bool {
    if let Some(start) = mdx.find("FilteredMembers As '") {
        let after_open = &mdx[start + "FilteredMembers As '".len()..];
        if let Some(end) = after_open.find('\'') {
            let set = &after_open[..end];
            return set.contains("[Measures]") && !set.contains("[Produktkategori]");
        }
    }
    false
}

fn cchildren_target_is_product_leaf(mdx: &str) -> bool {
    if let Some(start) = mdx.find("FilteredMembers As '") {
        let after_open = &mdx[start + "FilteredMembers As '".len()..];
        if let Some(end) = after_open.find('\'') {
            let set = &after_open[..end];
            return set.contains("[Produktkategori]") && (set.contains("&[") || set.contains("&amp;["));
        }
    }
    false
}

fn cchildren_filtered_member_name(mdx: &str) -> Option<String> {
    let key_start = mdx.find("FilteredMembers As '")?;
    let after_open = &mdx[key_start + "FilteredMembers As '".len()..];
    let set_end = after_open.find('\'')?;
    let set = &after_open[..set_end];
    if let Some(amp_start) = set.find("&[") {
        let begin = amp_start + 2;
        let end = set[begin..].find(']')? + begin;
        return Some(set[begin..end].to_string());
    }
    if let Some(amp_start) = set.find("&amp;[") {
        let begin = amp_start + 5;
        let end = set[begin..].find(']')? + begin;
        return Some(set[begin..end].to_string());
    }
    None
}

fn includes_prop(props: &[String], name: &str) -> bool {
    props.iter().any(|prop| prop == name)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SemanticQueryKind {
    ChildrenCountForAll,
    ChildrenCountLeafProduct,
    ChildrenCountMeasures,
    SlicerAllAndMeasure,
    MeasureChildrenEmpty,
    LeafChildrenEmpty,
    AllLevelMembers,
    LeafLevelMembers,
    MeasureByCategory,
    DrilldownCategories,
    SlicerOnly,
}

#[derive(Debug, Clone, PartialEq)]
struct SemanticQuery {
    kind: SemanticQueryKind,
    dim_props: Vec<String>,
    cell_props: Vec<String>,
    category_filters: Vec<String>,
    cchildren_leaf_name: Option<String>,
}

fn semantic_query_from_mdx(mdx: &str) -> SemanticQuery {
    let upper = mdx.to_uppercase();
    let dim_props = parse_dimension_properties(mdx);
    let cell_props = parse_cell_properties(mdx);
    let category_filters = parse_mdx_filters(mdx);
    let has_axes = upper.contains("ON COLUMNS") || upper.contains("ON ROWS");
    let has_rows = upper.contains("ON ROWS");
    let has_cols = upper.contains("ON COLUMNS");
    let has_product = mdx.contains("[Produktkategori]");
    let has_measures = mdx.contains("[Measures]");
    let is_drilldown = has_product && (mdx.contains("DrilldownLevel") || mdx.contains(".Members"));

    let kind = if mdx.contains("WITH MEMBER [Measures].cChildren") {
        if cchildren_target_is_measures(mdx) {
            SemanticQueryKind::ChildrenCountMeasures
        } else if cchildren_target_is_product_leaf(mdx) {
            SemanticQueryKind::ChildrenCountLeafProduct
        } else {
            SemanticQueryKind::ChildrenCountForAll
        }
    } else if mdx.contains("WHERE ([Produktkategori].[Produktkategori].[All],[Measures].[Total Försäljning])") {
        SemanticQueryKind::SlicerAllAndMeasure
    } else if mdx.contains("AddCalculatedMembers({[Measures].[Total Försäljning].Children})") {
        SemanticQueryKind::MeasureChildrenEmpty
    } else if mdx.contains("AddCalculatedMembers({[Produktkategori].[Produktkategori].&[") && mdx.contains("].Children})") {
        SemanticQueryKind::LeafChildrenEmpty
    } else if mdx.contains("AddCalculatedMembers({[Produktkategori].[Produktkategori].[(All)].Members})") {
        SemanticQueryKind::AllLevelMembers
    } else if mdx.contains("AddCalculatedMembers({[Produktkategori].[Produktkategori].[All].Children})") {
        SemanticQueryKind::LeafLevelMembers
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
        category_filters,
        cchildren_leaf_name: cchildren_filtered_member_name(mdx),
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

fn execute_semantic_query(query: &SemanticQuery) -> String {
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

fn get_execute_cellset_response(mdx: &str) -> String {
    let query = semantic_query_from_mdx(mdx);
    execute_semantic_query(&query)
}

fn get_execute_mdx_response(mdx: &str) -> String {
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

fn get_execute_dax_response(_dax: &str) -> String {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const MDX_CCHILDREN_LEAF: &str = "WITH MEMBER [Measures].cChildren As 'AddCalculatedMembers([Produktkategori].[Produktkategori].currentmember.children).count' Set FilteredMembers As '{[Produktkategori].[Produktkategori].&[Kategori B]}' Select {[Measures].cChildren} on ROWS, Hierarchize(Generate(FilteredMembers, Ascendants([Produktkategori].[Produktkategori].currentmember))) DIMENSION PROPERTIES PARENT_UNIQUE_NAME, MEMBER_TYPE ON COLUMNS FROM [Model]";

    const MDX_CCHILDREN_MEASURE: &str = "WITH MEMBER [Measures].cChildren As 'AddCalculatedMembers([Measures].currentmember.children).count' Set FilteredMembers As '{[Measures].[Total Försäljning]}' Select {[Measures].cChildren} on ROWS, Hierarchize(Generate(FilteredMembers, Ascendants([Measures].currentmember))) ON COLUMNS FROM [Model]";

    const MDX_ALL_MEMBERS: &str = "SELECT {AddCalculatedMembers({[Produktkategori].[Produktkategori].[(All)].Members})} DIMENSION PROPERTIES MEMBER_TYPE ON COLUMNS FROM [Model] CELL PROPERTIES CELL_ORDINAL";

    const MDX_ALL_CHILDREN: &str = "SELECT {AddCalculatedMembers({[Produktkategori].[Produktkategori].[All].Children})} DIMENSION PROPERTIES MEMBER_TYPE ON COLUMNS FROM [Model] CELL PROPERTIES CELL_ORDINAL";

    const MDX_DRILLDOWN: &str = "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Produktkategori].[Produktkategori].[All]},,,INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_CAPTION,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_KEY,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_TYPE,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_VALUE,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_NUMBER,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_LEVEL,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_COUNT,[Produktkategori].[Produktkategori].[Produktkategori]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] WHERE ([Measures].[Total Försäljning]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_SLICER: &str = "SELECT  FROM [Model] WHERE ([Produktkategori].[Produktkategori].&[Kategori A],[Measures].[Total Försäljning]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_SLICER_ALL: &str = "SELECT  FROM [Model] WHERE ([Produktkategori].[Produktkategori].[All],[Measures].[Total Försäljning]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_SUBQUERY_FILTERS: &str = "SELECT FROM (SELECT ({[Produktkategori].[Produktkategori].&[Kategori A],[Produktkategori].[Produktkategori].&[Kategori C]}) ON COLUMNS FROM [Model]) WHERE ([Measures].[Total Försäljning])";

    fn assert_in_order(haystack: &str, first: &str, second: &str) {
        let f = haystack.find(first)
            .unwrap_or_else(|| panic!("missing substring: {first}"));
        let s = haystack.find(second)
            .unwrap_or_else(|| panic!("missing substring: {second}"));
        assert!(f < s, "expected '{first}' before '{second}'");
    }

    fn member_block<'a>(xml: &'a str, caption: &str) -> &'a str {
        let member_start = xml.find(&format!("<Caption>{caption}</Caption>"))
            .unwrap_or_else(|| panic!("missing Caption: {caption}"));
        let block_start = xml[..member_start].rfind("<Member Hierarchy=")
            .unwrap_or_else(|| panic!("no Member start before Caption: {caption}"));
        let block_end = xml[member_start..].find("</Member>")
            .unwrap_or_else(|| panic!("no </Member> after Caption: {caption}"));
        &xml[block_start..member_start + block_end + "</Member>".len()]
    }

    // --- routing ---

    #[test]
    fn with_member_query_is_treated_as_mdx_select() {
        assert!(is_mdx_select(MDX_CCHILDREN_LEAF));
    }

    #[test]
    fn with_member_cchildren_does_not_fall_back_to_rowset_response() {
        let xml = get_execute_statement_response(MDX_CCHILDREN_LEAF);
        assert!(xml.contains("urn:schemas-microsoft-com:xml-analysis:mddataset"),
                "must use mddataset, not flat rowset");
    }

    // --- parsing: dimension properties ---

    #[test]
    fn parse_dimension_properties_extracts_known_props_from_qualified_tokens() {
        let props = parse_dimension_properties(MDX_DRILLDOWN);
        for name in &[
            "PARENT_UNIQUE_NAME",
            "HIERARCHY_UNIQUE_NAME",
            "MEMBER_NAME",
            "MEMBER_KEY",
            "MEMBER_TYPE",
            "MEMBER_VALUE",
            "PARENT_LEVEL",
            "PARENT_COUNT",
            "CHILDREN_CARDINALITY",
        ] {
            assert!(props.iter().any(|p| p == name),
                    "missing dimension property: {name}");
        }
    }

    #[test]
    fn parse_dimension_properties_returns_empty_when_clause_absent() {
        let props = parse_dimension_properties(MDX_CCHILDREN_MEASURE);
        assert!(props.is_empty());
    }

    // --- parsing: cell properties ---

    #[test]
    fn parse_cell_properties_extracts_requested_props() {
        let props = parse_cell_properties(MDX_DRILLDOWN);
        assert_eq!(props, vec!["VALUE", "FORMAT_STRING", "BACK_COLOR", "FORE_COLOR"]);
    }

    #[test]
    fn parse_cell_properties_returns_empty_when_clause_absent() {
        let props = parse_cell_properties(MDX_CCHILDREN_LEAF);
        assert!(props.is_empty());
    }

    // --- parsing: filter extraction ---

    #[test]
    fn parse_mdx_filters_extracts_single_where_category() {
        let filters = parse_mdx_filters(MDX_SLICER);
        assert_eq!(filters, vec!["Kategori A"]);
    }

    #[test]
    fn parse_mdx_filters_extracts_single_where_category_from_subquery() {
        // Known limitation: parse_category_filter matches Produktkategori
        // anywhere in the MDX, so subqueries only yield the first category.
        let filters = parse_mdx_filters(MDX_SUBQUERY_FILTERS);
        assert_eq!(filters, vec!["Kategori A"]);
    }

    #[test]
    fn parse_mdx_filters_returns_empty_for_all_filter() {
        let filters = parse_mdx_filters(MDX_SLICER_ALL);
        assert!(filters.is_empty());
    }

    // --- parsing: cChildren helpers ---

    #[test]
    fn cchildren_filtered_member_name_extracts_leaf_name() {
        let name = cchildren_filtered_member_name(MDX_CCHILDREN_LEAF);
        assert_eq!(name, Some("Kategori B".to_string()));
    }

    #[test]
    fn cchildren_target_is_product_leaf_returns_true_for_leaf_filter() {
        assert!(cchildren_target_is_product_leaf(MDX_CCHILDREN_LEAF));
    }

    #[test]
    fn cchildren_target_is_product_leaf_returns_false_for_all_probe() {
        let mdx_all = "WITH MEMBER [Measures].cChildren As 'AddCalculatedMembers([Produktkategori].[Produktkategori].currentmember.children).count' Set FilteredMembers As '{[Produktkategori].[Produktkategori].[(All)].Members}' Select {[Measures].cChildren} on ROWS, Hierarchize(Generate(FilteredMembers, Ascendants([Produktkategori].[Produktkategori].currentmember))) ON COLUMNS FROM [Model]";
        assert!(!cchildren_target_is_product_leaf(mdx_all));
    }

    #[test]
    fn cchildren_target_is_measures_returns_true_for_measures_probe() {
        assert!(cchildren_target_is_measures(MDX_CCHILDREN_MEASURE));
    }

    // --- semantic classification ---

    #[test]
    fn semantic_query_classifies_leaf_cchildren_probe() {
        let q = semantic_query_from_mdx(MDX_CCHILDREN_LEAF);
        assert_eq!(q.kind, SemanticQueryKind::ChildrenCountLeafProduct);
        assert_eq!(q.cchildren_leaf_name.as_deref(), Some("Kategori B"));
    }

    #[test]
    fn semantic_query_classifies_measure_cchildren_probe() {
        let q = semantic_query_from_mdx(MDX_CCHILDREN_MEASURE);
        assert_eq!(q.kind, SemanticQueryKind::ChildrenCountMeasures);
    }

    #[test]
    fn semantic_query_classifies_all_members_probe() {
        let q = semantic_query_from_mdx(MDX_ALL_MEMBERS);
        assert_eq!(q.kind, SemanticQueryKind::AllLevelMembers);
    }

    #[test]
    fn semantic_query_classifies_all_children_probe() {
        let q = semantic_query_from_mdx(MDX_ALL_CHILDREN);
        assert_eq!(q.kind, SemanticQueryKind::LeafLevelMembers);
    }

    #[test]
    fn semantic_query_classifies_drilldown_query() {
        let q = semantic_query_from_mdx(MDX_DRILLDOWN);
        assert_eq!(q.kind, SemanticQueryKind::DrilldownCategories);
    }

    #[test]
    fn semantic_query_classifies_slicer_all_and_measure() {
        let q = semantic_query_from_mdx(MDX_SLICER_ALL);
        assert_eq!(q.kind, SemanticQueryKind::SlicerAllAndMeasure);
    }

    // --- response shape: fragile cChildren + Ascendants probe ---

    #[test]
    fn leaf_cchildren_response_puts_all_before_leaf() {
        let xml = get_execute_statement_response(MDX_CCHILDREN_LEAF);
        assert_in_order(&xml,
            "[Produktkategori].[Produktkategori].[All]",
            "[Produktkategori].[Produktkategori].&amp;[Kategori B]");
    }

    #[test]
    fn leaf_cchildren_response_omits_parent_unique_name_for_all_member() {
        let xml = get_execute_statement_response(MDX_CCHILDREN_LEAF);
        let block = member_block(&xml, "All");
        assert!(!block.contains("<PARENT_UNIQUE_NAME>"),
                "All member must NOT emit PARENT_UNIQUE_NAME");
    }

    #[test]
    fn leaf_cchildren_response_includes_parent_unique_name_for_leaf_member() {
        let xml = get_execute_statement_response(MDX_CCHILDREN_LEAF);
        let block = member_block(&xml, "Kategori B");
        assert!(block.contains(
            "<PARENT_UNIQUE_NAME>[Produktkategori].[Produktkategori].[All]</PARENT_UNIQUE_NAME>"
        ));
    }

    #[test]
    fn leaf_cchildren_response_contains_two_count_cells() {
        let xml = get_execute_statement_response(MDX_CCHILDREN_LEAF);
        assert!(xml.contains(r#"<Cell CellOrdinal="0">"#));
        assert!(xml.contains(r#"<Cell CellOrdinal="1">"#));
    }
}
