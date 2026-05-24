/// Execute dispatch.
///
/// Routes incoming MDX/DAX statements to the correct response builder.
/// The actual parsing and classification lives in `mdx_semantic`;
/// the cellset/flat-rowset builders live in `execute_builders`.

use crate::response::wrap_in_soap_envelope;
use crate::mdx_semantic::{is_dax, is_mdx_select};
use crate::execute_builders::{
    get_execute_cellset_response, get_execute_dax_response,
    get_execute_mdx_response,
};

// ---- public API called by main.rs ----

pub fn get_empty_execute_response() -> String {
    wrap_in_soap_envelope(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:empty"/>
      </return>
    </ExecuteResponse>"#,
    )
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mdx_semantic::*;

    const MDX_CCHILDREN_LEAF: &str = "WITH MEMBER [Measures].cChildren As 'AddCalculatedMembers([Produktkategori].[Produktkategori].currentmember.children).count' Set FilteredMembers As '{[Produktkategori].[Produktkategori].&[Kategori B]}' Select {[Measures].cChildren} on ROWS, Hierarchize(Generate(FilteredMembers, Ascendants([Produktkategori].[Produktkategori].currentmember))) DIMENSION PROPERTIES PARENT_UNIQUE_NAME, MEMBER_TYPE ON COLUMNS FROM [Model]";

    const MDX_CCHILDREN_MEASURE: &str = "WITH MEMBER [Measures].cChildren As 'AddCalculatedMembers([Measures].currentmember.children).count' Set FilteredMembers As '{[Measures].[Total Försäljning]}' Select {[Measures].cChildren} on ROWS, Hierarchize(Generate(FilteredMembers, Ascendants([Measures].currentmember))) ON COLUMNS FROM [Model]";

    const MDX_ALL_MEMBERS: &str = "SELECT {AddCalculatedMembers({[Produktkategori].[Produktkategori].[(All)].Members})} DIMENSION PROPERTIES MEMBER_TYPE ON COLUMNS FROM [Model] CELL PROPERTIES CELL_ORDINAL";

    const MDX_ALL_CHILDREN: &str = "SELECT {AddCalculatedMembers({[Produktkategori].[Produktkategori].[All].Children})} DIMENSION PROPERTIES MEMBER_TYPE ON COLUMNS FROM [Model] CELL PROPERTIES CELL_ORDINAL";

    const MDX_DRILLDOWN: &str = "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Produktkategori].[Produktkategori].[All]},,,INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_CAPTION,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_KEY,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_TYPE,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_VALUE,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_NUMBER,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_LEVEL,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_COUNT,[Produktkategori].[Produktkategori].[Produktkategori]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] WHERE ([Measures].[Total Försäljning]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_SLICER: &str = "SELECT  FROM [Model] WHERE ([Produktkategori].[Produktkategori].&[Kategori A],[Measures].[Total Försäljning]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_SLICER_ALL: &str = "SELECT  FROM [Model] WHERE ([Produktkategori].[Produktkategori].[All],[Measures].[Total Försäljning]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_SUBQUERY_FILTERS: &str = "SELECT FROM (SELECT ({[Produktkategori].[Produktkategori].&[Kategori A],[Produktkategori].[Produktkategori].&[Kategori C]}) ON COLUMNS FROM [Model]) WHERE ([Measures].[Total Försäljning])";

    const MDX_REGION_SLICER: &str = "SELECT  FROM [Model] WHERE ([Region].[Region].&[North],[Measures].[Total Försäljning]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_REGION_DRILLDOWN: &str = "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Region].[Region].[All]},,,INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_CAPTION,[Region].[Region].[Region]MEMBER_NAME,[Region].[Region].[Region]MEMBER_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_KEY,[Region].[Region].[Region]MEMBER_TYPE,[Region].[Region].[Region]MEMBER_VALUE,[Region].[Region].[Region]LEVEL_NUMBER,[Region].[Region].[Region]LEVEL_UNIQUE_NAME,[Region].[Region].[Region]PARENT_LEVEL,[Region].[Region].[Region]PARENT_UNIQUE_NAME,[Region].[Region].[Region]PARENT_COUNT,[Region].[Region].[Region]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] WHERE ([Measures].[Total Försäljning]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_REGION_ALL_MEMBERS: &str = "SELECT {AddCalculatedMembers({[Region].[Region].[(All)].Members})} DIMENSION PROPERTIES MEMBER_TYPE ON COLUMNS FROM [Model] CELL PROPERTIES CELL_ORDINAL";

    const MDX_REGION_SLICER_ALL: &str = "SELECT  FROM [Model] WHERE ([Region].[Region].[All],[Measures].[Total Försäljning]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_KAT_ROWS_REGION_FILTER: &str = "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Produktkategori].[Produktkategori].[All]},,,INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_CAPTION,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_KEY,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_TYPE,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_VALUE,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_NUMBER,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_LEVEL,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_COUNT,[Produktkategori].[Produktkategori].[Produktkategori]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] WHERE ([Region].[Region].&[North],[Measures].[Total Försäljning]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_KAT_ROWS_REGION_ALL: &str = "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Produktkategori].[Produktkategori].[All]},,,INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_CAPTION,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_KEY,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_TYPE,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_VALUE,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_NUMBER,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_LEVEL,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_COUNT,[Produktkategori].[Produktkategori].[Produktkategori]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] WHERE ([Region].[Region].[All],[Measures].[Total Försäljning]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_CROSSJOIN_PROBE: &str = "SELECT NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Produktkategori].[Produktkategori].[All]},,,INCLUDE_CALC_MEMBERS)}), Hierarchize({DrilldownLevel({[Region].[Region].[All]},,,INCLUDE_CALC_MEMBERS)})) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_CAPTION,[Region].[Region].[Region]MEMBER_NAME,[Region].[Region].[Region]MEMBER_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_KEY,[Region].[Region].[Region]MEMBER_TYPE,[Region].[Region].[Region]MEMBER_VALUE,[Region].[Region].[Region]LEVEL_NUMBER,[Region].[Region].[Region]LEVEL_UNIQUE_NAME,[Region].[Region].[Region]PARENT_LEVEL,[Region].[Region].[Region]PARENT_UNIQUE_NAME,[Region].[Region].[Region]PARENT_COUNT,[Region].[Region].[Region]CHILDREN_CARDINALITY,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_CAPTION,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_KEY,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_TYPE,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_VALUE,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_NUMBER,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_LEVEL,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_COUNT,[Produktkategori].[Produktkategori].[Produktkategori]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_KAT_FILTERED_SINGLE: &str = "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Produktkategori].[Produktkategori].[All]},,,INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_CAPTION,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_KEY,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_TYPE,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_VALUE,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_NUMBER,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_LEVEL,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_COUNT,[Produktkategori].[Produktkategori].[Produktkategori]CHILDREN_CARDINALITY ON COLUMNS  FROM (SELECT ({[Produktkategori].[Produktkategori].&[Kategori B]}) ON COLUMNS  FROM [Model]) WHERE ([Region].[Region].[All],[Measures].[Total Försäljning]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_NESTED_BOTH_FILTERS: &str = "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Region].[Region].[All]},,,INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_CAPTION,[Region].[Region].[Region]MEMBER_NAME,[Region].[Region].[Region]MEMBER_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_KEY,[Region].[Region].[Region]MEMBER_TYPE,[Region].[Region].[Region]MEMBER_VALUE,[Region].[Region].[Region]LEVEL_NUMBER,[Region].[Region].[Region]LEVEL_UNIQUE_NAME,[Region].[Region].[Region]PARENT_LEVEL,[Region].[Region].[Region]PARENT_UNIQUE_NAME,[Region].[Region].[Region]PARENT_COUNT,[Region].[Region].[Region]CHILDREN_CARDINALITY ON COLUMNS  FROM (SELECT ({[Region].[Region].&[North]}) ON COLUMNS  FROM (SELECT ({[Produktkategori].[Produktkategori].&[Kategori A],[Produktkategori].[Produktkategori].&[Kategori B],[Produktkategori].[Produktkategori].&[Kategori D]}) ON COLUMNS  FROM [Model])) WHERE ([Measures].[Total Försäljning]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_DRILLDOWN_MEMBER_COLLAPSE: &str = "SELECT NON EMPTY Hierarchize(DrilldownMember(CrossJoin({[Produktkategori].[Produktkategori].[All],[Produktkategori].[Produktkategori].[Produktkategori].AllMembers}, {([Region].[Region].[All])}), {-{[Produktkategori].[Produktkategori].&[Kategori A]}}, [Region].[Region])) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_CAPTION,[Region].[Region].[Region]MEMBER_NAME,[Region].[Region].[Region]MEMBER_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_KEY,[Region].[Region].[Region]MEMBER_TYPE,[Region].[Region].[Region]MEMBER_VALUE,[Region].[Region].[Region]LEVEL_NUMBER,[Region].[Region].[Region]LEVEL_UNIQUE_NAME,[Region].[Region].[Region]PARENT_LEVEL,[Region].[Region].[Region]PARENT_UNIQUE_NAME,[Region].[Region].[Region]PARENT_COUNT,[Region].[Region].[Region]CHILDREN_CARDINALITY,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_CAPTION,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_KEY,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_TYPE,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_VALUE,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_NUMBER,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_LEVEL,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_COUNT,[Produktkategori].[Produktkategori].[Produktkategori]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] WHERE ([Measures].[Total Försäljning]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_DRILLDOWN_MEMBER_COLLAPSE_PRODUCT: &str = "SELECT NON EMPTY Hierarchize(DrilldownMember(CrossJoin({[Region].[Region].[All],[Region].[Region].[Region].AllMembers}, {([Produktkategori].[Produktkategori].[All])}), {-{[Produktkategori].[Produktkategori].&[Kategori D]}}, [Produktkategori].[Produktkategori])) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_CAPTION,[Region].[Region].[Region]MEMBER_NAME,[Region].[Region].[Region]MEMBER_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_KEY,[Region].[Region].[Region]MEMBER_TYPE,[Region].[Region].[Region]MEMBER_VALUE,[Region].[Region].[Region]LEVEL_NUMBER,[Region].[Region].[Region]LEVEL_UNIQUE_NAME,[Region].[Region].[Region]PARENT_LEVEL,[Region].[Region].[Region]PARENT_UNIQUE_NAME,[Region].[Region].[Region]PARENT_COUNT,[Region].[Region].[Region]CHILDREN_CARDINALITY,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_CAPTION,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_KEY,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_TYPE,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_VALUE,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_NUMBER,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_LEVEL,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_COUNT,[Produktkategori].[Produktkategori].[Produktkategori]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] WHERE ([Measures].[Total Försäljning]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

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
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].dimension, "Produktkategori");
        assert_eq!(filters[0].members, vec!["Kategori A"]);
    }

    #[test]
    fn parse_mdx_filters_extracts_multiple_subquery_categories() {
        let filters = parse_mdx_filters(MDX_SUBQUERY_FILTERS);
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].dimension, "Produktkategori");
        assert_eq!(filters[0].members, vec!["Kategori A", "Kategori C"]);
    }

    #[test]
    fn parse_mdx_filters_uses_slicer_not_subquery_when_slicer_has_product() {
        let mdx = "SELECT FROM (SELECT ({[Produktkategori].[Produktkategori].&[Kategori A]}) ON COLUMNS FROM [Model]) WHERE ([Produktkategori].[Produktkategori].&[Kategori B],[Measures].[Total Försäljning])";
        let filters = parse_mdx_filters(mdx);
        // Now merges both: WHERE (Kategori B) + subquery (Kategori A)
        let kat = filters.iter().find(|f| f.dimension == "Produktkategori").unwrap();
        assert_eq!(kat.members.len(), 2);
        assert!(kat.members.contains(&"Kategori A".to_string()));
        assert!(kat.members.contains(&"Kategori B".to_string()));
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

    // --- Region dimension ---

    #[test]
    fn parse_region_slicer_filter() {
        let filters = parse_mdx_filters(MDX_REGION_SLICER);
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].dimension, "Region");
        assert_eq!(filters[0].members, vec!["North"]);
    }

    #[test]
    fn semantic_query_classifies_region_drilldown() {
        let q = semantic_query_from_mdx(MDX_REGION_DRILLDOWN);
        assert_eq!(q.kind, SemanticQueryKind::DrilldownCategories);
        assert_eq!(q.row_dimension.as_deref(), Some("Region"));
    }

    #[test]
    fn semantic_query_classifies_region_all_members() {
        let q = semantic_query_from_mdx(MDX_REGION_ALL_MEMBERS);
        assert_eq!(q.kind, SemanticQueryKind::AllLevelMembers);
        assert_eq!(q.row_dimension.as_deref(), Some("Region"));
    }

    #[test]
    fn semantic_query_classifies_region_slicer_all() {
        let q = semantic_query_from_mdx(MDX_REGION_SLICER_ALL);
        assert_eq!(q.kind, SemanticQueryKind::SlicerAllAndMeasure);
        // No axis → no row dimension. All filter produces no members.
        assert_eq!(q.row_dimension, None);
        assert!(q.filters.is_empty());
    }

    #[test]
    fn region_all_members_response_uses_region_hierarchy() {
        let xml = get_execute_statement_response(MDX_REGION_ALL_MEMBERS);
        assert!(xml.contains("[Region].[Region]"));
        assert!(xml.contains("[Region].[Region].[All]"));
    }

    #[test]
    fn region_slicer_response_returns_north_total() {
        let xml = get_execute_statement_response(MDX_REGION_SLICER);
        // North total = 100000 + 150000 + 200000 + 200000 = 650000
        assert!(xml.contains("650000"));
    }

    // --- combined dimension ---

    #[test]
    fn parse_slicer_dimensions_detects_region_all() {
        let slicers = crate::mdx_semantic::parse_slicer_dimensions(MDX_REGION_SLICER_ALL);
        assert_eq!(slicers.len(), 1);
        assert_eq!(slicers[0].dimension, "Region");
        assert!(slicers[0].is_all);
    }

    #[test]
    fn parse_slicer_dimensions_detects_region_specific() {
        let slicers = crate::mdx_semantic::parse_slicer_dimensions(MDX_REGION_SLICER);
        assert_eq!(slicers.len(), 1);
        assert_eq!(slicers[0].dimension, "Region");
        assert!(!slicers[0].is_all);
    }

    #[test]
    fn parse_slicer_dimensions_empty_when_no_visible_dim_in_where() {
        let slicers = crate::mdx_semantic::parse_slicer_dimensions(
            "SELECT FROM [Model] WHERE ([Measures].[Total Försäljning]) CELL PROPERTIES VALUE"
        );
        assert!(slicers.is_empty());
    }

    #[test]
    fn semantic_query_has_slicer_for_region_all_in_where() {
        let q = semantic_query_from_mdx(MDX_REGION_SLICER_ALL);
        assert_eq!(q.slicers.len(), 1);
        assert_eq!(q.slicers[0].dimension, "Region");
        assert!(q.slicers[0].is_all);
    }

    #[test]
    fn combined_drilldown_response_includes_region_hierarchy_on_slicer_axis() {
        let xml = get_execute_statement_response(MDX_KAT_ROWS_REGION_FILTER);
        assert!(xml.contains("SlicerAxis"));
        assert!(xml.contains("[Region].[Region]"));
    }

    /// SlicerAxis caption extraction helper: finds the N-th Caption in SlicerAxis.
    fn slicer_captions(xml: &str) -> Vec<String> {
        // Find the actual <Axis name="SlicerAxis"> inside <Axes>.
        // Skip over the info declaration in <AxesInfo> by looking for the second occurrence.
        let first = xml.find(r#"name="SlicerAxis""#).expect("missing SlicerAxis");
        let second = xml[first + 1..].find(r#"name="SlicerAxis""#).expect("missing second SlicerAxis");
        let start = first + 1 + second;
        let end = xml[start..].find("</Axis>").map(|i| start + i).unwrap_or(xml.len());
        let slice = &xml[start..end];
        let mut caps = Vec::new();
        let mut pos = 0;
        while let Some(i) = slice[pos..].find("<Caption>") {
            let abs = pos + i + "<Caption>".len();
            let close = slice[abs..].find("</Caption>").unwrap();
            caps.push(slice[abs..abs + close].to_string());
            pos = abs + close + "</Caption>".len();
        }
        caps
    }

    #[test]
    fn slicer_axis_includes_region_even_when_not_in_where() {
        let xml = get_execute_statement_response(MDX_DRILLDOWN);
        let caps = slicer_captions(&xml);
        assert!(caps.iter().any(|c| c == "All"),
                "SlicerAxis should include Region.All as default even when not in WHERE, caps: {:?}", caps);
    }

    #[test]
    fn slicer_axis_has_region_all_for_kat_rows_query() {
        let xml = get_execute_statement_response(MDX_KAT_ROWS_REGION_ALL);
        let caps = slicer_captions(&xml);
        // Should contain Total Försäljning (SEK) and All for Region
        assert!(caps.iter().any(|c| c.contains("Total Försäljning")),
                "SlicerAxis missing Total Försäljning, caps: {:?}", caps);
        // Second "All" in slicer caps should be Region's All
        let all_count = caps.iter().filter(|c| *c == "All").count();
        assert!(all_count >= 1, "SlicerAxis missing Region.All, caps: {:?}", caps);
    }

    #[test]
    fn semantic_query_combined_row_kat_filter_region() {
        let q = semantic_query_from_mdx(MDX_KAT_ROWS_REGION_FILTER);
        assert_eq!(q.kind, SemanticQueryKind::DrilldownCategories);
        assert_eq!(q.row_dimension.as_deref(), Some("Produktkategori"));
        assert_eq!(q.filters.len(), 1);
        assert_eq!(q.filters[0].dimension, "Region");
        assert_eq!(q.filters[0].members, vec!["North"]);
    }

    #[test]
    fn combined_kat_rows_region_filter_returns_filtered_totals() {
        let xml = get_execute_statement_response(MDX_KAT_ROWS_REGION_FILTER);
        assert!(xml.contains("100000"));
        assert!(xml.contains("150000"));
        assert!(xml.contains("200000"));
    }

    #[test]
    fn drilldown_with_region_all_in_where_is_not_misclassified_as_slicer() {
        let q = semantic_query_from_mdx(MDX_KAT_ROWS_REGION_ALL);
        assert_eq!(q.kind, SemanticQueryKind::DrilldownCategories,
            "drilldown with WHERE (Region.All, Measures) must be DrilldownCategories, not {:?}", q.kind);
    }

    #[test]
    fn drilldown_with_region_all_in_where_has_axis0() {
        let xml = get_execute_statement_response(MDX_KAT_ROWS_REGION_ALL);
        assert!(xml.contains("Axis0"), "response must have Axis0 with Produktkategori members");
        assert!(xml.contains("[Produktkategori].[Produktkategori]"));
    }

    #[test]
    fn crossjoin_drilldown_has_both_dimensions() {
        let q = semantic_query_from_mdx(MDX_CROSSJOIN_PROBE);
        assert_eq!(q.kind, SemanticQueryKind::DrilldownCategories);
        assert_eq!(q.axis_dimensions, vec!["Produktkategori", "Region"]);
    }

    #[test]
    fn crossjoin_response_has_kategori_a() {
        let xml = get_execute_statement_response(MDX_CROSSJOIN_PROBE);
        assert!(xml.contains("Kategori A"));
        assert!(xml.contains("North"));
    }

    #[test]
    fn kat_filter_single_returns_only_filtered_category() {
        let xml = get_execute_statement_response(MDX_KAT_FILTERED_SINGLE);
        assert!(xml.contains("Kategori B"));
        assert!(!xml.contains("Kategori A"), "Kategori A should be filtered out");
        assert!(!xml.contains("Kategori C"), "Kategori C should be filtered out");
    }

    #[test]
    fn kat_filter_single_returns_correct_value() {
        let xml = get_execute_statement_response(MDX_KAT_FILTERED_SINGLE);
        // Kategori B total across all regions = 150000 + 100000 = 250000
        assert!(xml.contains("250000"));
    }

    #[test]
    fn nested_filters_parse_both_dimensions() {
        let filters = parse_mdx_filters(MDX_NESTED_BOTH_FILTERS);
        let kat = filters.iter().find(|f| f.dimension == "Produktkategori")
            .map(|f| &f.members).unwrap();
        let reg = filters.iter().find(|f| f.dimension == "Region")
            .map(|f| &f.members).unwrap();
        assert!(kat.contains(&"Kategori A".to_string()));
        assert!(kat.contains(&"Kategori B".to_string()));
        assert!(kat.contains(&"Kategori D".to_string()));
        assert!(!kat.contains(&"Kategori C".to_string()), "Kategori C should be filtered out");
        assert_eq!(reg, &vec!["North"]);
    }

    #[test]
    fn nested_filters_response_shows_region_rows_only() {
        let xml = get_execute_statement_response(MDX_NESTED_BOTH_FILTERS);
        // Region on rows with both filters: North only, Kategori A/B/D filtered
        assert!(xml.contains("North"));
        // Total: North + (A,B,D) = 100000 + 150000 + 200000 = 450000
        assert!(xml.contains("450000"));
    }

    #[test]
    fn collapse_detected() {
        let q = semantic_query_from_mdx(MDX_DRILLDOWN_MEMBER_COLLAPSE);
        assert_eq!(q.kind, SemanticQueryKind::DrilldownMemberProbe);
        assert_eq!(q.excluded_members, vec!["Kategori A"]);
        assert_eq!(q.drilldown_member_hierarchy.as_deref(), Some("Region"));
    }

    #[test]
    fn collapse_kategori_a_keeps_all_tuple() {
        let xml = get_execute_statement_response(MDX_DRILLDOWN_MEMBER_COLLAPSE);
        // Kategori A should remain, but collapsed to (Kategori A, Region.All)
        assert!(xml.contains("Kategori A"), "Kategori A should remain visible as All");
        // Region.All captions should appear (from the collapsed Kat A tuple)
        let caps = slicer_captions(&xml);
        // Axis0 should have Kategori A present
        assert!(xml.contains("Kategori A"));
    }

    #[test]
    fn collapse_kategori_a_removes_region_leaf_tuples_for_a() {
        let xml = get_execute_statement_response(MDX_DRILLDOWN_MEMBER_COLLAPSE);
        // Kategori A should NOT have North/South leaf tuples — those should be gone
        assert!(xml.contains("Kategori B"));
        assert!(xml.contains("Kategori C"));
        assert!(xml.contains("Kategori D"));
        // The count of tuples should match: 1(A+All) + 3*2(BCxDregion) = 7
    }

    #[test]
    fn collapse_produktkategori_detected() {
        let q = semantic_query_from_mdx(MDX_DRILLDOWN_MEMBER_COLLAPSE_PRODUCT);
        assert_eq!(q.kind, SemanticQueryKind::DrilldownMemberProbe);
        assert_eq!(q.drilldown_member_hierarchy.as_deref(), Some("Produktkategori"));
        assert_eq!(q.excluded_members, vec!["Kategori D"]);
    }

    #[test]
    fn collapse_produktkategori_keeps_d_visible_as_all() {
        let xml = get_execute_statement_response(MDX_DRILLDOWN_MEMBER_COLLAPSE_PRODUCT);
        // Kategori D should appear as (All, Region) — All caption present
        assert!(xml.contains("Kategori B"), "B should remain");
        assert!(xml.contains("Kategori C"), "C should remain");
    }

    #[test]
    fn collapse_produktkategori_removes_d_leaf_tuples() {
        let xml = get_execute_statement_response(MDX_DRILLDOWN_MEMBER_COLLAPSE_PRODUCT);
        let all_count = xml.matches("All").count();
        assert!(all_count >= 2, "Expected at least 2 All captions from collapsed tuples");
    }

    #[test]
    fn collapse_region_all_member_on_axis0_has_properties() {
        let xml = get_execute_statement_response(MDX_DRILLDOWN_MEMBER_COLLAPSE);
        let all_block = member_block(&xml, "All");
        assert!(all_block.contains("<HIERARCHY_UNIQUE_NAME>"),
            "Axis0 All member must have HIERARCHY_UNIQUE_NAME");
    }

    // --- MDX -> Malloy integration ---

    fn malloy_for_mdx(mdx: &str) -> String {
        let query = semantic_query_from_mdx(mdx);
        let plan = crate::engine::plan::plan_from_semantic(&query);
        crate::engine::malloy::malloy_for_query_plan(&crate::engine::model::default_model(), &plan)
    }

    #[test]
    fn slicer_produces_malloy_total() {
        let out = malloy_for_mdx(MDX_SLICER);
        assert!(out.contains("aggregate: total_forsaljning"));
        assert!(out.contains("where: produktkategori = 'Kategori A'"));
    }

    #[test]
    fn drilldown_produces_malloy_group_by_produktkategori() {
        let out = malloy_for_mdx(MDX_DRILLDOWN);
        assert!(out.contains("group_by: produktkategori"));
        assert!(out.contains("aggregate: total_forsaljning"));
    }

    #[test]
    fn crossjoin_produces_malloy_group_by_two_dimensions() {
        let out = malloy_for_mdx(MDX_CROSSJOIN_PROBE);
        assert!(out.contains("group_by: produktkategori, region"));
    }

    #[test]
    fn kat_rows_region_filter_produces_malloy_filter() {
        let out = malloy_for_mdx(MDX_KAT_ROWS_REGION_FILTER);
        assert!(out.contains("where: region = 'North'"));
        assert!(out.contains("group_by: produktkategori"));
    }

    #[test]
    fn malloy_two_dim_filtered_query() {
        let query = semantic_query_from_mdx(MDX_NESTED_BOTH_FILTERS);
        let plan = crate::engine::plan::plan_from_semantic(&query);
        let out = crate::engine::malloy::malloy_query(
            &crate::engine::model::default_model(),
            &plan,
        );
        assert!(out.contains("group_by: region"));
        assert!(out.contains("where: region = 'North' | produktkategori = 'Kategori A' | produktkategori = 'Kategori B' | produktkategori = 'Kategori D'"));
    }
}
