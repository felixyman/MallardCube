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
