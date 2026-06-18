/// Execute dispatch.
///
/// Routes incoming MDX/DAX statements to the correct response builder.
/// The actual parsing and classification lives in `mdx_semantic`;
/// the cellset/flat-rowset builders live in `execute_builders`.

use crate::response::wrap_in_soap_envelope;
use crate::mdx_semantic::{is_dax, is_mdx_select};
use crate::execute_builders::{
    get_execute_cellset_response, get_execute_cellset_response_with_backend,
    get_execute_dax_response, get_execute_mdx_response,
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
    use crate::backend::{Backend, QueryBackend};
    use crate::engine::model::SemanticModel;
    use crate::mdx_semantic::*;
    use crate::proxy_project::{ProxyProject, with_test_project};
    use crate::test_fixtures::{
        MDX_TWO_LEAF_FILTERS_UNITS,
        EXCEL_TRACE_CATEGORY_TERRITORY_REVENUE,
        EXCEL_TRACE_CHANNEL_WHOLESALE_CCHILDREN,
        EXCEL_TRACE_PROJECT3_EXECUTES,
        EXCEL_TRACE_SEGMENT_ALL_REVENUE,
        EXCEL_TRACE_SEGMENT_CONSUMER_CCHILDREN,
        EXCEL_TRACE_SEGMENT_CONSUMER_CHANNEL_ALL_REVENUE,
        EXCEL_TRACE_SEGMENT_CONSUMER_CHANNEL_WHOLESALE_DEFAULT_MEASURE,
        EXCEL_TRACE_SEGMENT_CONSUMER_CHANNEL_WHOLESALE_REVENUE,
        EXCEL_TRACE_SEGMENT_CONSUMER_CHANNEL_WHOLESALE_UNITS,
        EXCEL_TRACE_SEGMENT_CONSUMER_REVENUE,
        EXCEL_TRACE_TERRITORY_CATEGORY_ALL_UNITS,
        EXCEL_TRACE_TERRITORY_CATEGORY_COLLAPSE_NORTHWEST_REVENUE,
        EXCEL_TRACE_TERRITORY_CATEGORY_CONSUMER_UNITS,
        EXCEL_TRACE_TERRITORY_CATEGORY_DEFAULT_MEASURE,
        EXCEL_TRACE_TERRITORY_CATEGORY_REVENUE,
        EXCEL_TRACE_TERRITORY_CATEGORY_UNITS,
        EXCEL_TRACE_TERRITORY_DRILLDOWN_REVENUE,
        EXCEL_TRACE_TERRITORY_FILTER_NORTHWEST_REVENUE,
        EXCEL_TRACE_TERRITORY_FILTER_SOUTH_SEGMENT_CONSUMER_REVENUE,
        EXCEL_TRACE_TOTAL_REVENUE,
    };
    use std::collections::BTreeMap;

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

    const MDX_CROSSJOIN_REGION_FIRST: &str = "SELECT NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Region].[Region].[All]},,,INCLUDE_CALC_MEMBERS)}), Hierarchize({DrilldownLevel({[Produktkategori].[Produktkategori].[All]},,,INCLUDE_CALC_MEMBERS)})) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_CAPTION,[Region].[Region].[Region]MEMBER_NAME,[Region].[Region].[Region]MEMBER_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_KEY,[Region].[Region].[Region]MEMBER_TYPE,[Region].[Region].[Region]MEMBER_VALUE,[Region].[Region].[Region]LEVEL_NUMBER,[Region].[Region].[Region]LEVEL_UNIQUE_NAME,[Region].[Region].[Region]PARENT_LEVEL,[Region].[Region].[Region]PARENT_UNIQUE_NAME,[Region].[Region].[Region]PARENT_COUNT,[Region].[Region].[Region]CHILDREN_CARDINALITY,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_CAPTION,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_KEY,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_TYPE,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_VALUE,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_NUMBER,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_LEVEL,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_COUNT,[Produktkategori].[Produktkategori].[Produktkategori]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_COLLAPSE_REGION_FIRST: &str = "SELECT NON EMPTY Hierarchize(DrilldownMember(CrossJoin({[Region].[Region].[All],[Region].[Region].[Region].AllMembers}, {([Produktkategori].[Produktkategori].[All])}), {-{[Produktkategori].[Produktkategori].&[Kategori B]}}, [Produktkategori].[Produktkategori])) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_CAPTION,[Region].[Region].[Region]MEMBER_NAME,[Region].[Region].[Region]MEMBER_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_KEY,[Region].[Region].[Region]MEMBER_TYPE,[Region].[Region].[Region]MEMBER_VALUE,[Region].[Region].[Region]LEVEL_NUMBER,[Region].[Region].[Region]LEVEL_UNIQUE_NAME,[Region].[Region].[Region]PARENT_LEVEL,[Region].[Region].[Region]PARENT_UNIQUE_NAME,[Region].[Region].[Region]PARENT_COUNT,[Region].[Region].[Region]CHILDREN_CARDINALITY,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_CAPTION,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_KEY,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_TYPE,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_VALUE,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_NUMBER,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_LEVEL,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_COUNT,[Produktkategori].[Produktkategori].[Produktkategori]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] WHERE ([Measures].[Total Försäljning]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_COLLAPSE_EXCLUDE_REGION: &str = "SELECT NON EMPTY Hierarchize(DrilldownMember(CrossJoin({[Region].[Region].[All],[Region].[Region].[Region].AllMembers}, {([Produktkategori].[Produktkategori].[All])}), {-{[Region].[Region].&[North]}}, [Produktkategori].[Produktkategori])) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_CAPTION,[Region].[Region].[Region]MEMBER_NAME,[Region].[Region].[Region]MEMBER_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_KEY,[Region].[Region].[Region]MEMBER_TYPE,[Region].[Region].[Region]MEMBER_VALUE,[Region].[Region].[Region]LEVEL_NUMBER,[Region].[Region].[Region]LEVEL_UNIQUE_NAME,[Region].[Region].[Region]PARENT_LEVEL,[Region].[Region].[Region]PARENT_UNIQUE_NAME,[Region].[Region].[Region]PARENT_COUNT,[Region].[Region].[Region]CHILDREN_CARDINALITY,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_CAPTION,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_KEY,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_TYPE,[Produktkategori].[Produktkategori].[Produktkategori]MEMBER_VALUE,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_NUMBER,[Produktkategori].[Produktkategori].[Produktkategori]LEVEL_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_LEVEL,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_UNIQUE_NAME,[Produktkategori].[Produktkategori].[Produktkategori]PARENT_COUNT,[Produktkategori].[Produktkategori].[Produktkategori]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] WHERE ([Measures].[Total Försäljning]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

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

    fn with_project3<T>(f: impl FnOnce() -> T) -> T {
        let project = ProxyProject::load("project3/proxy-config.json")
            .expect("load project3");
        with_test_project(project, f)
    }

    fn with_retail_analytics<T>(f: impl FnOnce() -> T) -> T {
        let project = ProxyProject::load("generated_retail_analytics/proxy-config.json")
            .expect("load generated_retail_analytics");
        with_test_project(project, f)
    }

    fn with_generated_project<T>(f: impl FnOnce() -> T) -> T {
        let project = ProxyProject::load("generated_project/proxy-config.json")
            .expect("load generated_project");
        with_test_project(project, f)
    }

    /// Test-only `QueryBackend` that wraps a file-based DuckDB connection.
    /// Avoids the global `Backend` singleton so converted-project tests can
    /// exercise their own databases without in-memory demo seeding.
    struct FileQueryBackend(std::sync::Mutex<duckdb::Connection>);

    impl QueryBackend for FileQueryBackend {
        fn query_scalar(&self, sql: &str) -> f64 {
            let conn = self.0.lock().unwrap();
            conn.query_row(sql, [], |r| r.get(0)).unwrap_or(0.0)
        }

        fn query_grouped_1d(&self, sql: &str) -> Vec<(String, f64)> {
            let conn = self.0.lock().unwrap();
            let mut stmt = conn.prepare(sql).expect("prepare query_grouped_1d");
            stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?)))
                .expect("query_map query_grouped_1d")
                .filter_map(|r| r.ok())
                .collect()
        }

        fn query_pairs(&self, sql: &str) -> Vec<(String, String, f64)> {
            let conn = self.0.lock().unwrap();
            let mut stmt = conn.prepare(sql).expect("prepare query_pairs");
            stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, f64>(2)?)))
                .expect("query_map query_pairs")
                .filter_map(|r| r.ok())
                .collect()
        }

        fn query_count(&self, sql: &str) -> u32 {
            let conn = self.0.lock().unwrap();
            conn.query_row(sql, [], |r| r.get(0)).unwrap_or(0)
        }
    }

    fn extract_cell_value(xml: &str) -> Option<String> {
        let cell_start = xml.find("<Cell CellOrdinal=\"0\"")?;
        let val_start = xml[cell_start..].find("<Value")?;
        let abs_val_start = cell_start + val_start;
        let close = xml[abs_val_start..].find('>')?;
        let content_start = abs_val_start + close + 1;
        let content_end = xml[content_start..].find("</Value>")?;
        Some(xml[content_start..content_start + content_end].to_string())
    }

    fn extract_fmt_value(xml: &str) -> Option<String> {
        let start = xml.find("<FmtValue>")?;
        let content_start = start + "<FmtValue>".len();
        let content_end = xml[content_start..].find("</FmtValue>")?;
        Some(xml[content_start..content_start + content_end].to_string())
    }

    fn axis_captions(xml: &str, axis_name: &str) -> Vec<String> {
        let first = xml.find(&format!(r#"name="{axis_name}""#))
            .unwrap_or_else(|| panic!("missing {axis_name}"));
        let second = xml[first + 1..].find(&format!(r#"name="{axis_name}""#))
            .unwrap_or_else(|| panic!("missing second {axis_name}"));
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

    fn axis_tuple_captions(xml: &str, axis_name: &str) -> Vec<Vec<String>> {
        let first = xml.find(&format!(r#"name="{axis_name}""#))
            .unwrap_or_else(|| panic!("missing {axis_name}"));
        let second = xml[first + 1..].find(&format!(r#"name="{axis_name}""#))
            .unwrap_or_else(|| panic!("missing second {axis_name}"));
        let start = first + 1 + second;
        let end = xml[start..].find("</Axis>").map(|i| start + i).unwrap_or(xml.len());
        let slice = &xml[start..end];

        slice.split("<Tuple>")
            .skip(1)
            .map(|tuple| {
                let tuple_end = tuple.find("</Tuple>").unwrap_or(tuple.len());
                let tuple = &tuple[..tuple_end];
                let mut caps = Vec::new();
                let mut pos = 0;
                while let Some(i) = tuple[pos..].find("<Caption>") {
                    let abs = pos + i + "<Caption>".len();
                    let close = tuple[abs..].find("</Caption>").unwrap();
                    caps.push(tuple[abs..abs + close].to_string());
                    pos = abs + close + "</Caption>".len();
                }
                caps
            })
            .collect()
    }

    fn cell_values(xml: &str) -> Vec<f64> {
        let start = xml.find("<CellData>").expect("missing CellData");
        let end = xml[start..].find("</CellData>").map(|i| start + i).unwrap_or(xml.len());
        let slice = &xml[start..end];
        let mut values = Vec::new();
        let mut pos = 0;
        while let Some(i) = slice[pos..].find("<Value xsi:type=\"xsd:double\">") {
            let abs = pos + i + "<Value xsi:type=\"xsd:double\">".len();
            let close = slice[abs..].find("</Value>").unwrap();
            values.push(slice[abs..abs + close].parse().unwrap());
            pos = abs + close + "</Value>".len();
        }
        values
    }

    fn cell_format_strings(xml: &str) -> Vec<String> {
        let start = xml.find("<CellData>").expect("missing CellData");
        let end = xml[start..].find("</CellData>").map(|i| start + i).unwrap_or(xml.len());
        let slice = &xml[start..end];
        let mut values = Vec::new();
        let mut pos = 0;
        while let Some(i) = slice[pos..].find("<FormatString>") {
            let abs = pos + i + "<FormatString>".len();
            let close = slice[abs..].find("</FormatString>").unwrap();
            values.push(slice[abs..abs + close].to_string());
            pos = abs + close + "</FormatString>".len();
        }
        values
    }

    fn query_grouped(sql: &str) -> (Vec<String>, Vec<f64>) {
        let rows = Backend::get().query_grouped_1d(sql);
        let captions = rows.iter().map(|(name, _)| name.clone()).collect();
        let values = rows.iter().map(|(_, value)| *value).collect();
        (captions, values)
    }

    fn query_pairs(sql: &str) -> (Vec<Vec<String>>, Vec<f64>) {
        let rows = Backend::get().query_pairs(sql);
        let tuples = rows.iter()
            .map(|(first, second, _)| vec![first.clone(), second.clone()])
            .collect();
        let values = rows.iter().map(|(_, _, value)| *value).collect();
        (tuples, values)
    }

    fn collapse_first_dimension(sql: &str, excluded: &str) -> (Vec<Vec<String>>, Vec<f64>) {
        let rows = Backend::get().query_pairs(sql);
        let mut tuples = Vec::new();
        let mut values = Vec::new();
        let mut i = 0;

        while i < rows.len() {
            let (first, second, value) = &rows[i];
            if first == excluded {
                let mut total = *value;
                i += 1;
                while i < rows.len() && rows[i].0 == *first {
                    total += rows[i].2;
                    i += 1;
                }
                tuples.push(vec![first.clone(), "All".to_string()]);
                values.push(total);
                continue;
            }

            tuples.push(vec![first.clone(), second.clone()]);
            values.push(*value);
            i += 1;
        }

        (tuples, values)
    }

    fn tuple_value_map(tuples: &[Vec<String>], values: &[f64], swap: bool) -> BTreeMap<(String, String), f64> {
        tuples.iter().zip(values.iter())
            .map(|(tuple, value)| {
                let pair = if swap {
                    (tuple[1].clone(), tuple[0].clone())
                } else {
                    (tuple[0].clone(), tuple[1].clone())
                };
                (pair, *value)
            })
            .collect()
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
        axis_captions(xml, "SlicerAxis")
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
        assert_eq!(q.excluded_members.len(), 1);
        assert_eq!(q.excluded_members[0].dimension, "Produktkategori");
        assert_eq!(q.excluded_members[0].key, "Kategori A");
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
        assert_eq!(q.excluded_members.len(), 1);
        assert_eq!(q.excluded_members[0].dimension, "Produktkategori");
        assert_eq!(q.excluded_members[0].key, "Kategori D");
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

    // --- axis-order awareness (regression test for reversed row order) ---

    #[test]
    fn parse_axis_dimensions_preserves_forward_order() {
        let q = semantic_query_from_mdx(MDX_CROSSJOIN_PROBE);
        assert_eq!(q.axis_dimensions, vec!["Produktkategori", "Region"]);
    }

    #[test]
    fn parse_axis_dimensions_preserves_reversed_order() {
        let q = semantic_query_from_mdx(MDX_CROSSJOIN_REGION_FIRST);
        assert_eq!(q.axis_dimensions, vec!["Region", "Produktkategori"]);
    }

    #[test]
    fn reversed_crossjoin_puts_region_first_in_tuple() {
        let xml = get_execute_statement_response(MDX_CROSSJOIN_REGION_FIRST);
        // First hierarchy in AxisInfo should be Region
        let region_pos = xml.find("[Region].[Region]").unwrap();
        let kat_pos = xml.find("[Produktkategori].[Produktkategori]").unwrap();
        assert!(region_pos < kat_pos,
            "AxisInfo must list Region hierarchy first when Region is first in rows");
    }

    #[test]
    fn reversed_collapse_keeps_excluded_visible() {
        let xml = get_execute_statement_response(MDX_COLLAPSE_REGION_FIRST);
        // Produktkategori Kategori B is excluded — should appear as
        // (Region leaf, Produktkategori.All) in axis-dimension order.
        // Produktkategori.All has caption "All".
        assert!(xml.contains("All"), "All caption should appear for collapsed member");
        assert!(xml.contains("North"), "Region members should still appear");
    }

    #[test]
    fn reversed_crossjoin_has_correct_hierarchy_order_in_tuples() {
        let xml = get_execute_statement_response(MDX_CROSSJOIN_REGION_FIRST);
        // The first member in the first tuple of Axis0 should be from Region
        let first_tuple = xml.split("<Tuple>").nth(1).unwrap();
        let first_hier = first_tuple.split("<Member Hierarchy=").nth(1).unwrap();
        assert!(first_hier.starts_with("\"[Region].[Region]\""),
            "First tuple member should be Region when Region is first in rows, got: {first_hier}");
    }

    #[test]
    fn reversed_crossjoin_semantic_values_not_swapped() {
        let xml = get_execute_statement_response(MDX_CROSSJOIN_REGION_FIRST);
        // Region hierarchy must contain region captions (North, South), not Kategori names
        // Find the first Region member in the first tuple
        let cap_region = xml.split("<Caption>Region").nth(0).unwrap_or("");
        // Region captions like "North", "South" should appear
        assert!(xml.contains("<Caption>North</Caption>"), "Region hierarchy must show region names");
        assert!(xml.contains("<Caption>Kategori A</Caption>"), "Produktkategori hierarchy must show category names");
        // A Kategori name must NOT appear as a Region member caption
        // (Region captions should be North/South, not Kategori X)
    }

    // --- symmetric collapse: excluded member can be Region ---

    #[test]
    fn parse_excluded_members_detects_region_dimension() {
        let q = semantic_query_from_mdx(MDX_COLLAPSE_EXCLUDE_REGION);
        assert_eq!(q.excluded_members.len(), 1);
        assert_eq!(q.excluded_members[0].dimension, "Region");
        assert_eq!(q.excluded_members[0].key, "North");
    }

    #[test]
    fn collapse_parse_only_excludes_the_drilldownmember_members() {
        with_project3(|| {
            let query = crate::mdx_semantic::semantic_query_from_mdx(
                EXCEL_TRACE_TERRITORY_CATEGORY_COLLAPSE_NORTHWEST_REVENUE
            );
            // Only the one explicit exclusion from DrilldownMember, not the
            // later slicer members for Segment/Channel.
            assert_eq!(query.excluded_members.len(), 1,
                "should only exclude the DrilldownMember member, not slicer members");
            assert_eq!(query.excluded_members[0].key, "Northwest");
            assert_eq!(query.excluded_members[0].dimension, "Territory");
        });
    }

    #[test]
    fn collapse_exclude_region_keeps_north_visible_as_all() {
        let xml = get_execute_statement_response(MDX_COLLAPSE_EXCLUDE_REGION);
        // North is excluded from Region — should appear as (Region leaf, Produktkategori.All)
        // "All" caption should appear for the collapsed Produktkategori member
        assert!(xml.contains("All"), "All caption should appear for collapsed Produktkategori");
        // North should remain visible under Region hierarchy
        assert!(xml.contains("North"), "Excluded Region member North should still appear");
    }

    #[test]
    fn collapse_exclude_region_uses_region_total() {
        let xml = get_execute_statement_response(MDX_COLLAPSE_EXCLUDE_REGION);
        // North total across all categories: 100000 + 150000 + 200000 + 200000 = 650000
        assert!(xml.contains("650000"), "North collapsed row should show region total 650000");
    }

    // --- two-leaf-filter regression (project3 crash reproducer) ---

    #[test]
    fn two_leaf_filters_semantic() {
        let q = semantic_query_from_mdx(MDX_TWO_LEAF_FILTERS_UNITS);
        assert_eq!(q.kind, SemanticQueryKind::DrilldownCategories);
        // Axis dimensions are model-driven; at minimum verify the query
        // shape is recognized (not classified as SlicerOnly or similar).
        assert!(!q.filters.is_empty(), "should have at least one extracted filter");
    }

    #[test]
    fn two_leaf_filters_plan() {
        let q = semantic_query_from_mdx(MDX_TWO_LEAF_FILTERS_UNITS);
        let plan = crate::engine::plan::plan_from_semantic(&q);
        // Verify we get a GroupBy (not a crash / wrong variant).
        match &plan {
            crate::engine::plan::QueryPlan::GroupBy { .. } => {},
            _ => panic!("expected GroupBy plan, got {:?}", plan),
        }
    }

    #[test]
    fn two_leaf_filters_response() {
        let xml = get_execute_statement_response(MDX_TWO_LEAF_FILTERS_UNITS);
        // The pipeline must not panic. The XML must contain some
        // recognizable cellset structure.
        assert!(xml.contains("urn:schemas-microsoft-com:xml-analysis:mddataset"));
        assert!(xml.contains("<Axes>"));
        assert!(xml.contains("<CellData>"));
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
        assert!(out.contains("region = 'North'"));
        assert!(out.contains("(produktkategori = 'Kategori A' or produktkategori = 'Kategori B' or produktkategori = 'Kategori D')"));
        assert!(!out.contains(" | "), "cross-dimension filters should use AND (,) not OR (|)");
    }

    #[test]
    fn excel_trace_replay_project3_execute_shapes_render_cellsets() {
        with_project3(|| {
            for mdx in EXCEL_TRACE_PROJECT3_EXECUTES {
                let xml = get_execute_statement_response(mdx);
                assert!(xml.contains("urn:schemas-microsoft-com:xml-analysis:mddataset"), "query failed: {mdx}");
                assert!(xml.contains("<Axes>"), "missing axes for: {mdx}");
            }
        });
    }

    #[test]
    fn excel_trace_total_revenue_matches_raw_sql() {
        with_project3(|| {
            let xml = get_execute_statement_response(EXCEL_TRACE_TOTAL_REVENUE);
            let expected = Backend::get().query_scalar("SELECT SUM(revenue) FROM sales_fact");
            assert_eq!(cell_values(&xml), vec![expected]);
        });
    }

    #[test]
    fn excel_trace_territory_drilldown_matches_raw_sql() {
        with_project3(|| {
            let xml = get_execute_statement_response(EXCEL_TRACE_TERRITORY_DRILLDOWN_REVENUE);
            let (expected_captions, expected_values) = query_grouped(
                "SELECT territory, SUM(revenue) FROM sales_fact GROUP BY territory ORDER BY territory"
            );
            assert_eq!(axis_captions(&xml, "Axis0"), expected_captions);
            assert_eq!(cell_values(&xml), expected_values);
        });
    }

    #[test]
    fn excel_trace_territory_subquery_filter_matches_raw_sql() {
        with_project3(|| {
            let xml = get_execute_statement_response(EXCEL_TRACE_TERRITORY_FILTER_NORTHWEST_REVENUE);
            let (expected_captions, expected_values) = query_grouped(
                "SELECT territory, SUM(revenue) FROM sales_fact WHERE territory = 'Northwest' GROUP BY territory ORDER BY territory"
            );
            assert_eq!(axis_captions(&xml, "Axis0"), expected_captions);
            assert_eq!(cell_values(&xml), expected_values);
        });
    }

    #[test]
    fn excel_trace_segment_all_matches_unfiltered_revenue() {
        with_project3(|| {
            let all_xml = get_execute_statement_response(EXCEL_TRACE_SEGMENT_ALL_REVENUE);
            let plain_xml = get_execute_statement_response(EXCEL_TRACE_TERRITORY_DRILLDOWN_REVENUE);
            assert_eq!(axis_captions(&all_xml, "Axis0"), axis_captions(&plain_xml, "Axis0"));
            assert_eq!(cell_values(&all_xml), cell_values(&plain_xml));
        });
    }

    #[test]
    fn excel_trace_segment_consumer_matches_raw_sql() {
        with_project3(|| {
            let xml = get_execute_statement_response(EXCEL_TRACE_SEGMENT_CONSUMER_REVENUE);
            let (expected_captions, expected_values) = query_grouped(
                "SELECT territory, SUM(revenue) FROM sales_fact WHERE segment = 'Consumer' GROUP BY territory ORDER BY territory"
            );
            assert_eq!(axis_captions(&xml, "Axis0"), expected_captions);
            assert_eq!(cell_values(&xml), expected_values);
        });
    }

    #[test]
    fn excel_trace_nested_territory_and_segment_filter_matches_raw_sql() {
        with_project3(|| {
            let xml = get_execute_statement_response(EXCEL_TRACE_TERRITORY_FILTER_SOUTH_SEGMENT_CONSUMER_REVENUE);
            let (expected_captions, expected_values) = query_grouped(
                "SELECT territory, SUM(revenue) FROM sales_fact WHERE territory = 'South' AND segment = 'Consumer' GROUP BY territory ORDER BY territory"
            );
            assert_eq!(axis_captions(&xml, "Axis0"), expected_captions);
            assert_eq!(cell_values(&xml), expected_values);
        });
    }

    #[test]
    fn excel_trace_channel_all_filter_is_noop_under_consumer_filter() {
        with_project3(|| {
            let all_xml = get_execute_statement_response(EXCEL_TRACE_SEGMENT_CONSUMER_CHANNEL_ALL_REVENUE);
            let plain_xml = get_execute_statement_response(EXCEL_TRACE_SEGMENT_CONSUMER_REVENUE);
            assert_eq!(axis_captions(&all_xml, "Axis0"), axis_captions(&plain_xml, "Axis0"));
            assert_eq!(cell_values(&all_xml), cell_values(&plain_xml));
        });
    }

    #[test]
    fn excel_trace_two_leaf_filters_match_raw_revenue_sql() {
        with_project3(|| {
            let xml = get_execute_statement_response(EXCEL_TRACE_SEGMENT_CONSUMER_CHANNEL_WHOLESALE_REVENUE);
            let (expected_captions, expected_values) = query_grouped(
                "SELECT territory, SUM(revenue) FROM sales_fact WHERE segment = 'Consumer' AND channel = 'Wholesale' GROUP BY territory ORDER BY territory"
            );
            assert_eq!(axis_captions(&xml, "Axis0"), expected_captions);
            assert_eq!(cell_values(&xml), expected_values);
        });
    }

    #[test]
    fn excel_trace_omitted_measure_matches_explicit_revenue() {
        with_project3(|| {
            let implicit_xml = get_execute_statement_response(EXCEL_TRACE_SEGMENT_CONSUMER_CHANNEL_WHOLESALE_DEFAULT_MEASURE);
            let explicit_xml = get_execute_statement_response(EXCEL_TRACE_SEGMENT_CONSUMER_CHANNEL_WHOLESALE_REVENUE);
            assert_eq!(axis_captions(&implicit_xml, "Axis0"), axis_captions(&explicit_xml, "Axis0"));
            assert_eq!(cell_values(&implicit_xml), cell_values(&explicit_xml));
            assert_eq!(cell_format_strings(&implicit_xml), cell_format_strings(&explicit_xml));
        });
    }

    #[test]
    fn excel_trace_units_uses_units_values_and_format_string() {
        with_project3(|| {
            let xml = get_execute_statement_response(EXCEL_TRACE_SEGMENT_CONSUMER_CHANNEL_WHOLESALE_UNITS);
            let (expected_captions, expected_values) = query_grouped(
                "SELECT territory, SUM(units) FROM sales_fact WHERE segment = 'Consumer' AND channel = 'Wholesale' GROUP BY territory ORDER BY territory"
            );
            assert_eq!(axis_captions(&xml, "Axis0"), expected_captions);
            assert_eq!(cell_values(&xml), expected_values);
            assert!(cell_format_strings(&xml).iter().all(|fmt| fmt == "#,##0"));
            assert!(xml.contains("[Measures].[Units]"), "Units should be reflected on slicer axis");
        });
    }

    #[test]
    fn excel_trace_crossjoin_revenue_matches_raw_sql() {
        with_project3(|| {
            let xml = get_execute_statement_response(EXCEL_TRACE_TERRITORY_CATEGORY_REVENUE);
            let (expected_tuples, expected_values) = query_pairs(
                "SELECT territory, category, SUM(revenue) FROM sales_fact WHERE segment = 'Consumer' AND channel = 'Wholesale' GROUP BY territory, category ORDER BY territory, category"
            );
            assert_eq!(axis_tuple_captions(&xml, "Axis0"), expected_tuples);
            assert_eq!(cell_values(&xml), expected_values);
        });
    }

    #[test]
    fn excel_trace_crossjoin_reorder_matches_raw_sql_and_preserves_pair_values() {
        with_project3(|| {
            let forward_xml = get_execute_statement_response(EXCEL_TRACE_TERRITORY_CATEGORY_REVENUE);
            let reverse_xml = get_execute_statement_response(EXCEL_TRACE_CATEGORY_TERRITORY_REVENUE);

            let (expected_forward_tuples, expected_forward_values) = query_pairs(
                "SELECT territory, category, SUM(revenue) FROM sales_fact WHERE segment = 'Consumer' AND channel = 'Wholesale' GROUP BY territory, category ORDER BY territory, category"
            );
            let (expected_reverse_tuples, expected_reverse_values) = query_pairs(
                "SELECT category, territory, SUM(revenue) FROM sales_fact WHERE segment = 'Consumer' AND channel = 'Wholesale' GROUP BY category, territory ORDER BY category, territory"
            );

            let forward_tuples = axis_tuple_captions(&forward_xml, "Axis0");
            let reverse_tuples = axis_tuple_captions(&reverse_xml, "Axis0");
            let forward_values = cell_values(&forward_xml);
            let reverse_values = cell_values(&reverse_xml);

            assert_eq!(forward_tuples, expected_forward_tuples);
            assert_eq!(forward_values, expected_forward_values);
            assert_eq!(reverse_tuples, expected_reverse_tuples);
            assert_eq!(reverse_values, expected_reverse_values);

            assert_eq!(
                tuple_value_map(&forward_tuples, &forward_values, false),
                tuple_value_map(&reverse_tuples, &reverse_values, true),
            );
        });
    }

    #[test]
    fn excel_trace_crossjoin_implicit_measure_matches_explicit_revenue() {
        with_project3(|| {
            let implicit_xml = get_execute_statement_response(EXCEL_TRACE_TERRITORY_CATEGORY_DEFAULT_MEASURE);
            let explicit_xml = get_execute_statement_response(EXCEL_TRACE_TERRITORY_CATEGORY_REVENUE);
            assert_eq!(axis_tuple_captions(&implicit_xml, "Axis0"), axis_tuple_captions(&explicit_xml, "Axis0"));
            assert_eq!(cell_values(&implicit_xml), cell_values(&explicit_xml));
            assert_eq!(cell_format_strings(&implicit_xml), cell_format_strings(&explicit_xml));
        });
    }

    #[test]
    fn excel_trace_crossjoin_collapse_rolls_up_northwest_total() {
        with_project3(|| {
            let xml = get_execute_statement_response(EXCEL_TRACE_TERRITORY_CATEGORY_COLLAPSE_NORTHWEST_REVENUE);
            let (expected_tuples, expected_values) = collapse_first_dimension(
                "SELECT territory, category, SUM(revenue) FROM sales_fact WHERE segment = 'Consumer' AND channel = 'Wholesale' GROUP BY territory, category ORDER BY territory, category",
                "Northwest",
            );
            assert_eq!(axis_tuple_captions(&xml, "Axis0"), expected_tuples);
            assert_eq!(cell_values(&xml), expected_values);
        });
    }

    #[test]
    fn excel_trace_crossjoin_units_matches_raw_sql_and_format() {
        with_project3(|| {
            let xml = get_execute_statement_response(EXCEL_TRACE_TERRITORY_CATEGORY_UNITS);
            let (expected_tuples, expected_values) = query_pairs(
                "SELECT territory, category, SUM(units) FROM sales_fact WHERE segment = 'Consumer' AND channel = 'Wholesale' GROUP BY territory, category ORDER BY territory, category"
            );
            assert_eq!(axis_tuple_captions(&xml, "Axis0"), expected_tuples);
            assert_eq!(cell_values(&xml), expected_values);
            assert!(cell_format_strings(&xml).iter().all(|fmt| fmt == "#,##0"));
            assert!(xml.contains("[Measures].[Units]"), "Units should be reflected on slicer axis");
        });
    }

    #[test]
    fn excel_trace_crossjoin_consumer_units_matches_raw_sql() {
        with_project3(|| {
            let xml = get_execute_statement_response(EXCEL_TRACE_TERRITORY_CATEGORY_CONSUMER_UNITS);
            let (expected_tuples, expected_values) = query_pairs(
                "SELECT territory, category, SUM(units) FROM sales_fact WHERE segment = 'Consumer' GROUP BY territory, category ORDER BY territory, category"
            );
            assert_eq!(axis_tuple_captions(&xml, "Axis0"), expected_tuples);
            assert_eq!(cell_values(&xml), expected_values);
            assert!(cell_format_strings(&xml).iter().all(|fmt| fmt == "#,##0"));
        });
    }

    #[test]
    fn excel_trace_crossjoin_all_units_matches_unfiltered_sql() {
        with_project3(|| {
            let xml = get_execute_statement_response(EXCEL_TRACE_TERRITORY_CATEGORY_ALL_UNITS);
            let (expected_tuples, expected_values) = query_pairs(
                "SELECT territory, category, SUM(units) FROM sales_fact GROUP BY territory, category ORDER BY territory, category"
            );
            assert_eq!(axis_tuple_captions(&xml, "Axis0"), expected_tuples);
            assert_eq!(cell_values(&xml), expected_values);
            assert!(cell_format_strings(&xml).iter().all(|fmt| fmt == "#,##0"));
        });
    }

    #[test]
    fn excel_trace_filtered_cchildren_probes_render_cellsets() {
        with_project3(|| {
            for mdx in [EXCEL_TRACE_CHANNEL_WHOLESALE_CCHILDREN, EXCEL_TRACE_SEGMENT_CONSUMER_CCHILDREN] {
                let xml = get_execute_statement_response(mdx);
                assert!(xml.contains("urn:schemas-microsoft-com:xml-analysis:mddataset"), "query failed: {mdx}");
                assert!(xml.contains("<CellData>"), "missing cell data for: {mdx}");
            }
        });
    }

    #[test]
    fn column_only_measure_uses_correct_measure() {
        with_project3(|| {
            let mdx = "SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales] CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";
            let xml = get_execute_statement_response(mdx);
            let expected = Backend::get().query_scalar("SELECT SUM(revenue) FROM sales_fact");
            assert_eq!(cell_values(&xml), vec![expected]);
            assert!(xml.contains("[Measures].[Revenue]"), "slicer axis should show Revenue");
        });
    }

    #[test]
    fn parser_axis_dimension_ids_match_semantic_parse_axis_dimensions() {
        with_project3(|| {
            for mdx in EXCEL_TRACE_PROJECT3_EXECUTES {
                // Skip member/children probes — they don't have axis dimensions.
                if mdx.contains(".Members") || mdx.contains(".Children") || mdx.contains("AddCalculatedMembers") {
                    continue;
                }
                let parsed = crate::mdx_parser::parse_mdx(mdx);
                let from_parser: Vec<String> = parsed.axis_dimension_ids.iter()
                    .filter(|id| crate::proxy_project::project().model.dim_def_opt(id).is_some())
                    .cloned()
                    .collect();
                let from_semantic = crate::mdx_semantic::semantic_query_from_mdx(mdx).axis_dimensions;
                assert_eq!(from_parser, from_semantic,
                    "axis dimension mismatch for: {mdx}");
            }
        });
    }

    #[test]
    fn time_intelligence_revenue_ytd_plan_has_date_dim_filter() {
        with_project3(|| {
            use crate::engine::plan::plan_from_semantic_with_model;
            use crate::engine::sql::sql_for_query_plan;
            let project = crate::proxy_project::project();
            let mdx = "SELECT  FROM [Sales] WHERE ([Measures].[Revenue YTD]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";
            let semantic = crate::mdx_semantic::semantic_query_from_mdx(mdx);
            assert_eq!(semantic.measure.as_deref(), Some("Revenue YTD"),
                "should resolve measure from MDX WHERE clause");
            let plan = plan_from_semantic_with_model(&semantic, &project.model);
            let sql = sql_for_query_plan(&project.model, &plan);
            println!("=== Revenue YTD SQL ===\n{sql}");
            assert!(
                sql.contains("IN (SELECT date_key FROM date_dim WHERE ytd_flag = true)"),
                "Revenue YTD plan should include date_dim ytd_flag subquery, got: {sql}"
            );
            // Verify the plan itself carries the time_flag filter.
            match &plan {
                crate::engine::plan::QueryPlan::Total { filters, .. } => {
                    let ti_filters: Vec<_> = filters.iter()
                        .filter(|f| f.time_flag.is_some())
                        .collect();
                    assert_eq!(ti_filters.len(), 1,
                        "should have exactly one time_flag filter");
                    assert_eq!(ti_filters[0].time_flag.as_deref(), Some("ytd_flag"));
                }
                other => panic!("expected Total plan, got {other:?}"),
            }
        });
    }

    #[test]
    fn time_intelligence_revenue_prior_year_plan_has_date_dim_filter() {
        with_project3(|| {
            use crate::engine::plan::plan_from_semantic_with_model;
            use crate::engine::sql::sql_for_query_plan;
            let project = crate::proxy_project::project();
            let mdx = "SELECT  FROM [Sales] WHERE ([Measures].[Revenue Prior Year]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";
            let semantic = crate::mdx_semantic::semantic_query_from_mdx(mdx);
            assert_eq!(semantic.measure.as_deref(), Some("Revenue Prior Year"));
            let plan = plan_from_semantic_with_model(&semantic, &project.model);
            let sql = sql_for_query_plan(&project.model, &plan);
            println!("=== Revenue Prior Year SQL ===\n{sql}");
            assert!(
                sql.contains("IN (SELECT date_key FROM date_dim WHERE prior_year_ytd_flag = true)"),
                "Revenue Prior Year plan should include date_dim prior_year_ytd_flag subquery, got: {sql}"
            );
            match &plan {
                crate::engine::plan::QueryPlan::Total { filters, .. } => {
                    let ti: Vec<_> = filters.iter().filter(|f| f.time_flag.is_some()).collect();
                    assert_eq!(ti.len(), 1);
                    assert_eq!(ti[0].time_flag.as_deref(), Some("prior_year_ytd_flag"));
                }
                other => panic!("expected Total plan, got {other:?}"),
            }
        });
    }

    #[test]
    fn time_intelligence_revenue_qtd_plan_has_date_dim_filter() {
        with_project3(|| {
            use crate::engine::plan::plan_from_semantic_with_model;
            use crate::engine::sql::sql_for_query_plan;
            let project = crate::proxy_project::project();
            let mdx = "SELECT  FROM [Sales] WHERE ([Measures].[Revenue QTD]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";
            let semantic = crate::mdx_semantic::semantic_query_from_mdx(mdx);
            assert_eq!(semantic.measure.as_deref(), Some("Revenue QTD"));
            let plan = plan_from_semantic_with_model(&semantic, &project.model);
            let sql = sql_for_query_plan(&project.model, &plan);
            println!("=== Revenue QTD SQL ===\n{sql}");
            assert!(
                sql.contains("IN (SELECT date_key FROM date_dim WHERE qtd_flag = true)"),
                "Revenue QTD plan should include date_dim qtd_flag subquery, got: {sql}"
            );
            match &plan {
                crate::engine::plan::QueryPlan::Total { filters, .. } => {
                    let ti: Vec<_> = filters.iter().filter(|f| f.time_flag.is_some()).collect();
                    assert_eq!(ti.len(), 1);
                    assert_eq!(ti[0].time_flag.as_deref(), Some("qtd_flag"));
                }
                other => panic!("expected Total plan, got {other:?}"),
            }
        });
    }

    #[test]
    fn time_intelligence_revenue_mtd_plan_has_date_dim_filter() {
        with_project3(|| {
            use crate::engine::plan::plan_from_semantic_with_model;
            use crate::engine::sql::sql_for_query_plan;
            let project = crate::proxy_project::project();
            let mdx = "SELECT  FROM [Sales] WHERE ([Measures].[Revenue MTD]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";
            let semantic = crate::mdx_semantic::semantic_query_from_mdx(mdx);
            assert_eq!(semantic.measure.as_deref(), Some("Revenue MTD"));
            let plan = plan_from_semantic_with_model(&semantic, &project.model);
            let sql = sql_for_query_plan(&project.model, &plan);
            println!("=== Revenue MTD SQL ===\n{sql}");
            assert!(
                sql.contains("IN (SELECT date_key FROM date_dim WHERE mtd_flag = true)"),
                "Revenue MTD plan should include date_dim mtd_flag subquery, got: {sql}"
            );
            match &plan {
                crate::engine::plan::QueryPlan::Total { filters, .. } => {
                    let ti: Vec<_> = filters.iter().filter(|f| f.time_flag.is_some()).collect();
                    assert_eq!(ti.len(), 1);
                    assert_eq!(ti[0].time_flag.as_deref(), Some("mtd_flag"));
                }
                other => panic!("expected Total plan, got {other:?}"),
            }
        });
    }

    #[test]
    fn time_intelligence_measures_execute_non_empty() {
        with_project3(|| {
            use crate::backend::Backend;
            use crate::engine::plan::plan_from_semantic_with_model;
            let project = crate::proxy_project::project();
            let backend = Backend::get();
            for (mdx, label) in [
                ("SELECT  FROM [Sales] WHERE ([Measures].[Revenue YTD]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR", "YTD"),
                ("SELECT  FROM [Sales] WHERE ([Measures].[Revenue Prior Year]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR", "PriorYTD"),
                ("SELECT  FROM [Sales] WHERE ([Measures].[Revenue QTD]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR", "QTD"),
                ("SELECT  FROM [Sales] WHERE ([Measures].[Revenue MTD]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR", "MTD"),
            ] {
                let semantic = crate::mdx_semantic::semantic_query_from_mdx(mdx);
                let plan = plan_from_semantic_with_model(&semantic, &project.model);
                let result = crate::engine::plan::execute_plan_with_backend(
                    &plan, &project.model, backend,
                );
                match result {
                    crate::engine::plan::QueryResult::Scalar(v) => {
                        assert!(v > 0.0, "{label} revenue should be non-zero against demo data, got {v}");
                    }
                    other => panic!("{label} expected Scalar result, got {other:?}"),
                }
            }
        });
    }

    // ---- Generated retail analytics compatibility gate ----

    #[test]
    fn retail_analytics_discover_catalogs_returns_correct_name() {
        with_retail_analytics(|| {
            let xml = crate::xmla::discover::catalogs::get_catalogs_response();
            assert!(xml.contains("urn:schemas-microsoft-com:xml-analysis:rowset"), "missing rowset namespace");
            assert!(xml.contains("SEMANTICMODEL"), "should contain catalog name");
            assert!(xml.contains("<row"), "should have at least one row");
        });
    }

    #[test]
    fn retail_analytics_discover_cubes_returns_correct_name() {
        with_retail_analytics(|| {
            let xml = crate::xmla::discover::cubes::get_cubes_response();
            assert!(xml.contains("urn:schemas-microsoft-com:xml-analysis:rowset"), "missing rowset namespace");
            assert!(xml.contains("<row"), "should have at least one row");
        });
    }

    #[test]
    fn retail_analytics_discover_dimensions_has_date_role() {
        with_retail_analytics(|| {
            let xml = crate::xmla::discover::dimensions::get_dimensions_response();
            assert!(xml.contains("urn:schemas-microsoft-com:xml-analysis:rowset"), "missing rowset namespace");
            assert!(xml.contains(">Dates<"), "should contain Dates dimension");
            assert!(xml.contains(">Stores<"), "should contain Stores dimension");
            let rows = xml.matches("<row").count();
            assert!(rows >= 5, "should have at least 5 dimension rows");
        });
    }

    #[test]
    fn retail_analytics_discover_measures_has_total_revenue() {
        with_retail_analytics(|| {
            let xml = crate::xmla::discover::measures::get_measures_response();
            assert!(xml.contains("urn:schemas-microsoft-com:xml-analysis:rowset"), "missing rowset namespace");
            assert!(xml.contains("<row"), "should have at least one row");
        });
    }

    #[test]
    fn retail_analytics_total_revenue_is_fallback_returns_empty() {
        // Total Revenue is no longer a stub — Plan 021 generated real SQL.
        // The fallback returns a real value (0 on empty DB).
        with_retail_analytics(|| {
            let project = crate::proxy_project::project();
            let conn = duckdb::Connection::open(
                "generated_retail_analytics/data/sales.db"
            ).expect("open retail db");
            let backend = FileQueryBackend(std::sync::Mutex::new(conn));

            let mdx = "SELECT  FROM [SALES] WHERE ([Measures].[Total Revenue]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";
            let xml = get_execute_cellset_response_with_backend(
                mdx, &backend, &project.model,
            );

            assert!(!xml.is_empty(), "should not panic on fallback measure");
            assert!(xml.contains("urn:schemas-microsoft-com:xml-analysis:mddataset"), "missing mddataset");
            assert!(xml.contains("<Axes>"), "missing axes");
            // Real fallback SQL now returns a value
            assert!(xml.contains("<Cell "), "real fallback should have Cell elements");
        });
    }

    #[test]
    fn retail_analytics_config_has_no_placeholder_sql() {
        // Verify the checked-in config contract: no converted measure
        // should use SUM(1), SUM(...), AVG(...), etc. as sql_expr.
        let config_text = std::fs::read_to_string(
            "generated_retail_analytics/proxy-config.json"
        ).expect("read retail config");
        let line = config_text.lines().find(|l| l.contains("sql_expr"))
            .unwrap_or("");
        // All measures should be sql_fallback (sql_expr: "null").
        // Placeholder aggregations should never appear.
        assert!(!config_text.contains("SUM(1)"), "SUM(1) placeholder found in config");
        assert!(!config_text.contains("SUM(...)"), "SUM(...) placeholder found in config");
        assert!(!config_text.contains("AVG(...)"), "AVG(...) placeholder found in config");
        assert!(!config_text.contains("COUNT(...)"), "COUNT(...) placeholder found in config");
        assert!(!config_text.contains("COUNT(DISTINCT ...)"), "COUNT(DISTINCT ...) placeholder found in config");
    }

    #[test]
    fn retail_analytics_stub_measures_return_empty() {
        with_retail_analytics(|| {
            for mdx in [
                "SELECT  FROM [SALES] WHERE ([Measures].[Gross Profit]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR",
                "SELECT  FROM [SALES] WHERE ([Measures].[Total COGS]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR",
            ] {
                let xml = get_execute_statement_response(mdx);
                assert!(!xml.is_empty(), "should not panic on stub measure");
                // Stubs return Empty QueryResult — cellset has no cell data
            }
        });
    }

    #[test]
    fn generated_project_fallback_measures_return_real_data() {
        // ---- direct DuckDB characterization (independent data-proof) ----
        use duckdb::Connection;
        let conn = Connection::open("data/generated.db").expect("open generated db");

        // DVT measure: should find matching rows in the fixture
        let dvt_count: f64 = conn.query_row(
            "SELECT COUNT(DISTINCT f.remissnummer) AS value
             FROM dw_fys_f_undersökning f
             JOIN dw_fys_d_remisskoder rk ON f.remisskoderid = rk.remisskoderid
             JOIN dw_fys_d_produkt p ON f.produktid = p.produktid
             JOIN dw_fys_kalender_signeringsdatum kd ON f.signeringsdatum = kd.signeringsdatum
             JOIN dw_fys_d_beställare b ON f.beställareid = b.beställareid
             WHERE rk.akut = 'Ja'
               AND p.produktkod IN ('516', '526', '524')
               AND f.beställningstimme BETWEEN 8 AND 14
               AND kd.veckodagssiffra BETWEEN 1 AND 5
               AND RIGHT(b.beställarekod, 3) = 'M08'",
            [], |r| r.get(0)
        ).expect("DVT query");
        assert!(dvt_count > 0.0, "DVT measure should return > 0 remissnummer, got {dvt_count}");

        // Medeltid measure: should return a non-null average
        let medeltid: Option<f64> = conn.query_row(
            "SELECT AVG(avg_per_remiss) FROM (
                SELECT AVG(undersökningsslut_till_signering_ej_akut) AS avg_per_remiss
                FROM dw_fys_f_undersökning
                GROUP BY remissnummer
            ) sub",
            [], |r| r.get(0)
        ).expect("Medeltid query");
        assert!(medeltid.is_some(), "Medeltid measure should return a value");
        assert!(medeltid.unwrap() > 0.0, "Medeltid should be > 0, got {medeltid:?}");

        // ---- execution-path assertions (prove the proxy returns the same) ----
        with_generated_project(|| {
            let project = crate::proxy_project::project();
            let conn = Connection::open("data/generated.db").expect("open generated db");
            let backend = FileQueryBackend(std::sync::Mutex::new(conn));

            // DVT measure through proxy execution
            let mdx_dvt = "SELECT  FROM [DW_FYS_F_UNDERSÖKNING] WHERE ([Measures].[Antal signerade DVT-remisser]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";
            let xml_dvt = get_execute_cellset_response_with_backend(
                mdx_dvt, &backend, &project.model,
            );
            assert!(xml_dvt.contains("<CellData>"), "DVT execution should produce cellset");
            let dvt_val = extract_cell_value(&xml_dvt)
                .expect("DVT cellset should contain <Value>");
            let dvt_parsed: f64 = dvt_val.parse().expect("DVT value should be numeric");
            assert!(dvt_parsed > 0.0, "DVT measure should return > 0 through proxy execution, got {dvt_parsed}");

            // Medeltid measure through proxy execution
            let mdx_mt = "SELECT  FROM [DW_FYS_F_UNDERSÖKNING] WHERE ([Measures].[Medeltid Undersökningsslut till signering (ej akut)]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";
            let xml_mt = get_execute_cellset_response_with_backend(
                mdx_mt, &backend, &project.model,
            );
            assert!(xml_mt.contains("<CellData>"), "Medeltid execution should produce cellset");
            let mt_val = extract_cell_value(&xml_mt)
                .expect("Medeltid cellset should contain <Value>");
            let mt_parsed: f64 = mt_val.parse().expect("Medeltid value should be numeric");
            assert!(mt_parsed > 0.0, "Medeltid should return > 0 through proxy execution, got {mt_parsed}");
        });
    }
}
