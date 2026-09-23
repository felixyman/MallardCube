use crate::backend::QueryBackend;
use crate::engine::model::SemanticModel;
#[cfg(test)]
use crate::execute_builders::{
    get_execute_cellset_response, get_execute_dax_response, get_execute_mdx_response,
};
#[cfg(test)]
use crate::mdx_semantic::{is_dax, is_drillthrough, is_mdx_select};
/// Execute dispatch.
///
/// Routes incoming MDX/DAX statements to the correct response builder.
/// The actual parsing and classification lives in `mdx_semantic`;
/// the cellset/flat-rowset builders live in `execute_builders`.
use crate::response::wrap_in_soap_envelope;

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

/// Test seam: routes statements against the demo fixture. Production requests
/// are routed by `route_request` in `main.rs`, which passes the request's
/// backend explicitly.
#[cfg(test)]
pub fn get_execute_statement_response(statement: &str) -> String {
    if is_dax(statement) {
        get_execute_dax_response(statement)
    } else if is_drillthrough(statement) {
        get_execute_drillthrough_response(statement, crate::backend::Backend::test_fixture())
    } else if is_mdx_select(statement) {
        get_execute_cellset_response(statement)
    } else {
        get_execute_mdx_response(statement)
    }
}

pub fn get_execute_drillthrough_response<B: QueryBackend + ?Sized>(
    statement: &str,
    backend: &B,
) -> String {
    let project = crate::proxy_project::project();
    let model = &project.model;
    let table = model.primary_table_name();

    // Extract slicer filters from WHERE-clause members like
    // [Territory].[Territory].&[North], [Date].[Calendar].[Year].&[2024], or the
    // compound [Date].[Calendar].[Quarter].&[2024]&[1]. Filters are exact: flat
    // dimensions compare by equality, multi-level dimensions scope through
    // their dim table by level (plan 033 — the old CAST+LIKE prefix match
    // also returned Northeast/Northwest rows for North).
    let mut where_clauses: Vec<String> = Vec::new();
    let mut pos = 0usize;
    while let Some(rel) = statement[pos..].find(".&[") {
        let abs = pos + rel;
        // Backtrack to the opening [Dim] bracket. DRILLTHROUGH uses (...),
        // not {[...]}, so search for any [.
        let prefix = &statement[..abs];
        let bracket = prefix
            .rfind(",[")
            .or_else(|| prefix.rfind("(["))
            .unwrap_or(0);
        match parse_member_ref(&statement[bracket + 1..]) {
            Some((member, consumed)) => {
                if let Some(sql) = member_filter_sql(model, &member) {
                    where_clauses.push(sql);
                }
                pos = bracket + 1 + consumed;
            }
            None => pos = abs + 3,
        }
    }

    let sql = if where_clauses.is_empty() {
        format!("SELECT * FROM {table} LIMIT 1000")
    } else {
        format!(
            "SELECT * FROM {table} WHERE {} LIMIT 1000",
            where_clauses.join(" AND ")
        )
    };
    let rows = backend.query_rows(&sql);
    let col_names = backend.query_column_names(&sql);
    build_drillthrough_rowset(&col_names, rows, statement)
}

/// A DRILLTHROUGH slicer member: `[Dim].[Hier].[Level].&[k1]&[k2]`.
struct DrillMember {
    dim: String,
    /// The named level, when the member carries one (`[Date].[Calendar].[Year]`).
    level: Option<String>,
    keys: Vec<String>,
}

/// Parse a member from the start of `text` (`[Dim].[Hier]...`). Returns the
/// member and how many bytes were consumed (path plus every key).
fn parse_member_ref(text: &str) -> Option<(DrillMember, usize)> {
    let mut parts: Vec<&str> = Vec::new();
    let mut i = 0usize;
    while text[i..].starts_with('[') {
        let end = i + text[i..].find(']')?;
        parts.push(&text[i + 1..end]);
        i = end + 1;
        if text[i..].starts_with(".[") {
            i += 1; // another hierarchy level segment
        } else {
            break;
        }
    }
    if text[i..].starts_with('.') {
        i += 1; // the `.` of the first `.&[key]`
    }
    let mut keys = Vec::new();
    while text[i..].starts_with("&[") {
        let end = i + text[i..].find(']')?;
        keys.push(text[i + 2..end].to_string());
        i = end + 1;
    }
    if parts.is_empty() || keys.is_empty() {
        return None;
    }
    let level = (parts.len() >= 3).then(|| parts[2].to_string());
    Some((
        DrillMember {
            dim: parts[0].to_string(),
            level,
            keys,
        },
        i,
    ))
}

/// Build an exact WHERE predicate for a DRILLTHROUGH member.
///
/// - Multi-level dimensions scope through their dim table by level columns
///   (`date_key IN (SELECT date_key FROM date_dim WHERE year = '2024')`), so
///   coarse members filter exactly and leaf members match the leaf column.
/// - Flat dimensions compare the fact column by equality.
///
/// Never emits a prefix match: a member shape that cannot be aligned falls
/// back to exact equality on the fact column (fail closed).
fn member_filter_sql(model: &SemanticModel, m: &DrillMember) -> Option<String> {
    let dim = model.dim_def_opt(&m.dim)?;
    let rel = model.rel_for_dimension(&m.dim);
    let esc = |v: &str| v.replace('\'', "''");

    if !dim.levels.is_empty() {
        let target = match &m.level {
            Some(level) => dim
                .levels
                .iter()
                .position(|l| l.name.eq_ignore_ascii_case(level)),
            None => Some(dim.levels.len() - 1), // bare member = leaf level
        };
        if let (Some(target), Some(rel)) = (target, rel)
            && let Some(start) = (target + 1).checked_sub(m.keys.len())
        {
            let preds: Vec<String> = m
                .keys
                .iter()
                .enumerate()
                .map(|(i, key)| {
                    format!(
                        "CAST({} AS VARCHAR) = '{}'",
                        dim.levels[start + i].column,
                        esc(key)
                    )
                })
                .collect();
            return Some(format!(
                "{} IN (SELECT {} FROM {} WHERE {})",
                rel.fact_column,
                rel.dim_column,
                rel.dim_table,
                preds.join(" AND ")
            ));
        }
    }

    let key = m.keys.last()?;
    let col = rel
        .map(|r| r.fact_column.as_str())
        .unwrap_or(dim.physical_field.as_str());
    Some(format!("CAST({col} AS VARCHAR) = '{}'", esc(key)))
}

fn build_drillthrough_rowset(
    col_names: &[String],
    rows: Vec<Vec<String>>,
    _statement: &str,
) -> String {
    let mut xml_rows = String::new();
    for row in &rows {
        xml_rows.push_str("          <row>\n");
        for (i, col) in col_names.iter().enumerate() {
            let val = row.get(i).map(|s| s.as_str()).unwrap_or("");
            let safe_col = col.replace(' ', "_x0020_").replace('.', "_x002E_");
            xml_rows.push_str(&format!(
                "            <{safe_col}>{}</{safe_col}>\n",
                crate::response::xml_escape(val),
            ));
        }
        xml_rows.push_str("          </row>\n");
    }

    let mut schema = String::new();
    schema.push_str(r#"              <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
                <xsd:element name="root">
                  <xsd:complexType><xsd:sequence minOccurs="0" maxOccurs="unbounded"><xsd:element name="row" type="row"/></xsd:sequence></xsd:complexType>
                </xsd:element>
                <xsd:complexType name="row">
                  <xsd:sequence>
"#);
    for col in col_names {
        let safe_col = col.replace(' ', "_x0020_").replace('.', "_x002E_");
        schema.push_str(&format!(
            r#"                    <xsd:element sql:field="{col}" name="{safe_col}" type="xsd:string" minOccurs="0"/>
"#,
        ));
    }
    schema.push_str(
        r#"                  </xsd:sequence>
                </xsd:complexType>
              </xsd:schema>
"#,
    );

    let inner = format!(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
{schema}
{xml_rows}
        </root>
      </return>
    </ExecuteResponse>"#
    );
    wrap_in_soap_envelope(&inner)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Backend, QueryBackend};

    use crate::mdx_semantic::*;
    use crate::proxy_project::{ProxyProject, with_test_project};
    use crate::test_fixtures::{
        EXCEL_TRACE_CATEGORY_TERRITORY_REVENUE, EXCEL_TRACE_CHANNEL_WHOLESALE_CCHILDREN,
        EXCEL_TRACE_PROJECT3_EXECUTES, EXCEL_TRACE_SEGMENT_ALL_REVENUE,
        EXCEL_TRACE_SEGMENT_CONSUMER_CCHILDREN, EXCEL_TRACE_SEGMENT_CONSUMER_CHANNEL_ALL_REVENUE,
        EXCEL_TRACE_SEGMENT_CONSUMER_CHANNEL_WHOLESALE_DEFAULT_MEASURE,
        EXCEL_TRACE_SEGMENT_CONSUMER_CHANNEL_WHOLESALE_REVENUE,
        EXCEL_TRACE_SEGMENT_CONSUMER_CHANNEL_WHOLESALE_UNITS, EXCEL_TRACE_SEGMENT_CONSUMER_REVENUE,
        EXCEL_TRACE_TERRITORY_CATEGORY_ALL_UNITS,
        EXCEL_TRACE_TERRITORY_CATEGORY_COLLAPSE_NORTHWEST_REVENUE,
        EXCEL_TRACE_TERRITORY_CATEGORY_CONSUMER_UNITS,
        EXCEL_TRACE_TERRITORY_CATEGORY_DEFAULT_MEASURE, EXCEL_TRACE_TERRITORY_CATEGORY_REVENUE,
        EXCEL_TRACE_TERRITORY_CATEGORY_UNITS, EXCEL_TRACE_TERRITORY_DRILLDOWN_REVENUE,
        EXCEL_TRACE_TERRITORY_FILTER_NORTHWEST_REVENUE,
        EXCEL_TRACE_TERRITORY_FILTER_SOUTH_SEGMENT_CONSUMER_REVENUE, EXCEL_TRACE_TOTAL_REVENUE,
        MDX_TWO_LEAF_FILTERS_UNITS,
    };
    use std::collections::BTreeMap;

    const MDX_CCHILDREN_LEAF: &str = "WITH MEMBER [Measures].cChildren As 'AddCalculatedMembers([ProductCategory].[ProductCategory].currentmember.children).count' Set FilteredMembers As '{[ProductCategory].[ProductCategory].&[Category B]}' Select {[Measures].cChildren} on ROWS, Hierarchize(Generate(FilteredMembers, Ascendants([ProductCategory].[ProductCategory].currentmember))) DIMENSION PROPERTIES PARENT_UNIQUE_NAME, MEMBER_TYPE ON COLUMNS FROM [Model]";

    const MDX_CCHILDREN_MEASURE: &str = "WITH MEMBER [Measures].cChildren As 'AddCalculatedMembers([Measures].currentmember.children).count' Set FilteredMembers As '{[Measures].[Total Sales]}' Select {[Measures].cChildren} on ROWS, Hierarchize(Generate(FilteredMembers, Ascendants([Measures].currentmember))) ON COLUMNS FROM [Model]";

    const MDX_ALL_MEMBERS: &str = "SELECT {AddCalculatedMembers({[ProductCategory].[ProductCategory].[(All)].Members})} DIMENSION PROPERTIES MEMBER_TYPE ON COLUMNS FROM [Model] CELL PROPERTIES CELL_ORDINAL";

    const MDX_ALL_CHILDREN: &str = "SELECT {AddCalculatedMembers({[ProductCategory].[ProductCategory].[All].Children})} DIMENSION PROPERTIES MEMBER_TYPE ON COLUMNS FROM [Model] CELL PROPERTIES CELL_ORDINAL";

    const MDX_DRILLDOWN: &str = "SELECT NON EMPTY Hierarchize({DrilldownLevel({[ProductCategory].[ProductCategory].[All]},,,INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_CAPTION,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_KEY,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_TYPE,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_VALUE,[ProductCategory].[ProductCategory].[ProductCategory]LEVEL_NUMBER,[ProductCategory].[ProductCategory].[ProductCategory]LEVEL_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_LEVEL,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_COUNT,[ProductCategory].[ProductCategory].[ProductCategory]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] WHERE ([Measures].[Total Sales]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_SLICER: &str = "SELECT  FROM [Model] WHERE ([ProductCategory].[ProductCategory].&[Category A],[Measures].[Total Sales]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_SLICER_ALL: &str = "SELECT  FROM [Model] WHERE ([ProductCategory].[ProductCategory].[All],[Measures].[Total Sales]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_SUBQUERY_FILTERS: &str = "SELECT FROM (SELECT ({[ProductCategory].[ProductCategory].&[Category A],[ProductCategory].[ProductCategory].&[Category C]}) ON COLUMNS FROM [Model]) WHERE ([Measures].[Total Sales])";

    const MDX_REGION_SLICER: &str = "SELECT  FROM [Model] WHERE ([Region].[Region].&[North],[Measures].[Total Sales]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_REGION_DRILLDOWN: &str = "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Region].[Region].[All]},,,INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_CAPTION,[Region].[Region].[Region]MEMBER_NAME,[Region].[Region].[Region]MEMBER_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_KEY,[Region].[Region].[Region]MEMBER_TYPE,[Region].[Region].[Region]MEMBER_VALUE,[Region].[Region].[Region]LEVEL_NUMBER,[Region].[Region].[Region]LEVEL_UNIQUE_NAME,[Region].[Region].[Region]PARENT_LEVEL,[Region].[Region].[Region]PARENT_UNIQUE_NAME,[Region].[Region].[Region]PARENT_COUNT,[Region].[Region].[Region]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] WHERE ([Measures].[Total Sales]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_REGION_ALL_MEMBERS: &str = "SELECT {AddCalculatedMembers({[Region].[Region].[(All)].Members})} DIMENSION PROPERTIES MEMBER_TYPE ON COLUMNS FROM [Model] CELL PROPERTIES CELL_ORDINAL";

    const MDX_REGION_SLICER_ALL: &str = "SELECT  FROM [Model] WHERE ([Region].[Region].[All],[Measures].[Total Sales]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_KAT_ROWS_REGION_FILTER: &str = "SELECT NON EMPTY Hierarchize({DrilldownLevel({[ProductCategory].[ProductCategory].[All]},,,INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_CAPTION,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_KEY,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_TYPE,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_VALUE,[ProductCategory].[ProductCategory].[ProductCategory]LEVEL_NUMBER,[ProductCategory].[ProductCategory].[ProductCategory]LEVEL_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_LEVEL,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_COUNT,[ProductCategory].[ProductCategory].[ProductCategory]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] WHERE ([Region].[Region].&[North],[Measures].[Total Sales]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_KAT_ROWS_REGION_ALL: &str = "SELECT NON EMPTY Hierarchize({DrilldownLevel({[ProductCategory].[ProductCategory].[All]},,,INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_CAPTION,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_KEY,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_TYPE,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_VALUE,[ProductCategory].[ProductCategory].[ProductCategory]LEVEL_NUMBER,[ProductCategory].[ProductCategory].[ProductCategory]LEVEL_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_LEVEL,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_COUNT,[ProductCategory].[ProductCategory].[ProductCategory]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] WHERE ([Region].[Region].[All],[Measures].[Total Sales]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_CROSSJOIN_PROBE: &str = "SELECT NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[ProductCategory].[ProductCategory].[All]},,,INCLUDE_CALC_MEMBERS)}), Hierarchize({DrilldownLevel({[Region].[Region].[All]},,,INCLUDE_CALC_MEMBERS)})) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_CAPTION,[Region].[Region].[Region]MEMBER_NAME,[Region].[Region].[Region]MEMBER_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_KEY,[Region].[Region].[Region]MEMBER_TYPE,[Region].[Region].[Region]MEMBER_VALUE,[Region].[Region].[Region]LEVEL_NUMBER,[Region].[Region].[Region]LEVEL_UNIQUE_NAME,[Region].[Region].[Region]PARENT_LEVEL,[Region].[Region].[Region]PARENT_UNIQUE_NAME,[Region].[Region].[Region]PARENT_COUNT,[Region].[Region].[Region]CHILDREN_CARDINALITY,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_CAPTION,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_KEY,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_TYPE,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_VALUE,[ProductCategory].[ProductCategory].[ProductCategory]LEVEL_NUMBER,[ProductCategory].[ProductCategory].[ProductCategory]LEVEL_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_LEVEL,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_COUNT,[ProductCategory].[ProductCategory].[ProductCategory]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_KAT_FILTERED_SINGLE: &str = "SELECT NON EMPTY Hierarchize({DrilldownLevel({[ProductCategory].[ProductCategory].[All]},,,INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_CAPTION,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_KEY,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_TYPE,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_VALUE,[ProductCategory].[ProductCategory].[ProductCategory]LEVEL_NUMBER,[ProductCategory].[ProductCategory].[ProductCategory]LEVEL_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_LEVEL,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_COUNT,[ProductCategory].[ProductCategory].[ProductCategory]CHILDREN_CARDINALITY ON COLUMNS  FROM (SELECT ({[ProductCategory].[ProductCategory].&[Category B]}) ON COLUMNS  FROM [Model]) WHERE ([Region].[Region].[All],[Measures].[Total Sales]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_NESTED_BOTH_FILTERS: &str = "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Region].[Region].[All]},,,INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_CAPTION,[Region].[Region].[Region]MEMBER_NAME,[Region].[Region].[Region]MEMBER_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_KEY,[Region].[Region].[Region]MEMBER_TYPE,[Region].[Region].[Region]MEMBER_VALUE,[Region].[Region].[Region]LEVEL_NUMBER,[Region].[Region].[Region]LEVEL_UNIQUE_NAME,[Region].[Region].[Region]PARENT_LEVEL,[Region].[Region].[Region]PARENT_UNIQUE_NAME,[Region].[Region].[Region]PARENT_COUNT,[Region].[Region].[Region]CHILDREN_CARDINALITY ON COLUMNS  FROM (SELECT ({[Region].[Region].&[North]}) ON COLUMNS  FROM (SELECT ({[ProductCategory].[ProductCategory].&[Category A],[ProductCategory].[ProductCategory].&[Category B],[ProductCategory].[ProductCategory].&[Category D]}) ON COLUMNS  FROM [Model])) WHERE ([Measures].[Total Sales]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_DRILLDOWN_MEMBER_COLLAPSE: &str = "SELECT NON EMPTY Hierarchize(DrilldownMember(CrossJoin({[ProductCategory].[ProductCategory].[All],[ProductCategory].[ProductCategory].[ProductCategory].AllMembers}, {([Region].[Region].[All])}), {-{[ProductCategory].[ProductCategory].&[Category A]}}, [Region].[Region])) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_CAPTION,[Region].[Region].[Region]MEMBER_NAME,[Region].[Region].[Region]MEMBER_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_KEY,[Region].[Region].[Region]MEMBER_TYPE,[Region].[Region].[Region]MEMBER_VALUE,[Region].[Region].[Region]LEVEL_NUMBER,[Region].[Region].[Region]LEVEL_UNIQUE_NAME,[Region].[Region].[Region]PARENT_LEVEL,[Region].[Region].[Region]PARENT_UNIQUE_NAME,[Region].[Region].[Region]PARENT_COUNT,[Region].[Region].[Region]CHILDREN_CARDINALITY,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_CAPTION,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_KEY,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_TYPE,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_VALUE,[ProductCategory].[ProductCategory].[ProductCategory]LEVEL_NUMBER,[ProductCategory].[ProductCategory].[ProductCategory]LEVEL_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_LEVEL,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_COUNT,[ProductCategory].[ProductCategory].[ProductCategory]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] WHERE ([Measures].[Total Sales]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_DRILLDOWN_MEMBER_COLLAPSE_PRODUCT: &str = "SELECT NON EMPTY Hierarchize(DrilldownMember(CrossJoin({[Region].[Region].[All],[Region].[Region].[Region].AllMembers}, {([ProductCategory].[ProductCategory].[All])}), {-{[ProductCategory].[ProductCategory].&[Category D]}}, [ProductCategory].[ProductCategory])) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_CAPTION,[Region].[Region].[Region]MEMBER_NAME,[Region].[Region].[Region]MEMBER_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_KEY,[Region].[Region].[Region]MEMBER_TYPE,[Region].[Region].[Region]MEMBER_VALUE,[Region].[Region].[Region]LEVEL_NUMBER,[Region].[Region].[Region]LEVEL_UNIQUE_NAME,[Region].[Region].[Region]PARENT_LEVEL,[Region].[Region].[Region]PARENT_UNIQUE_NAME,[Region].[Region].[Region]PARENT_COUNT,[Region].[Region].[Region]CHILDREN_CARDINALITY,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_CAPTION,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_KEY,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_TYPE,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_VALUE,[ProductCategory].[ProductCategory].[ProductCategory]LEVEL_NUMBER,[ProductCategory].[ProductCategory].[ProductCategory]LEVEL_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_LEVEL,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_COUNT,[ProductCategory].[ProductCategory].[ProductCategory]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] WHERE ([Measures].[Total Sales]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_CROSSJOIN_REGION_FIRST: &str = "SELECT NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Region].[Region].[All]},,,INCLUDE_CALC_MEMBERS)}), Hierarchize({DrilldownLevel({[ProductCategory].[ProductCategory].[All]},,,INCLUDE_CALC_MEMBERS)})) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_CAPTION,[Region].[Region].[Region]MEMBER_NAME,[Region].[Region].[Region]MEMBER_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_KEY,[Region].[Region].[Region]MEMBER_TYPE,[Region].[Region].[Region]MEMBER_VALUE,[Region].[Region].[Region]LEVEL_NUMBER,[Region].[Region].[Region]LEVEL_UNIQUE_NAME,[Region].[Region].[Region]PARENT_LEVEL,[Region].[Region].[Region]PARENT_UNIQUE_NAME,[Region].[Region].[Region]PARENT_COUNT,[Region].[Region].[Region]CHILDREN_CARDINALITY,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_CAPTION,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_KEY,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_TYPE,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_VALUE,[ProductCategory].[ProductCategory].[ProductCategory]LEVEL_NUMBER,[ProductCategory].[ProductCategory].[ProductCategory]LEVEL_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_LEVEL,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_COUNT,[ProductCategory].[ProductCategory].[ProductCategory]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_COLLAPSE_REGION_FIRST: &str = "SELECT NON EMPTY Hierarchize(DrilldownMember(CrossJoin({[Region].[Region].[All],[Region].[Region].[Region].AllMembers}, {([ProductCategory].[ProductCategory].[All])}), {-{[ProductCategory].[ProductCategory].&[Category B]}}, [ProductCategory].[ProductCategory])) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_CAPTION,[Region].[Region].[Region]MEMBER_NAME,[Region].[Region].[Region]MEMBER_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_KEY,[Region].[Region].[Region]MEMBER_TYPE,[Region].[Region].[Region]MEMBER_VALUE,[Region].[Region].[Region]LEVEL_NUMBER,[Region].[Region].[Region]LEVEL_UNIQUE_NAME,[Region].[Region].[Region]PARENT_LEVEL,[Region].[Region].[Region]PARENT_UNIQUE_NAME,[Region].[Region].[Region]PARENT_COUNT,[Region].[Region].[Region]CHILDREN_CARDINALITY,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_CAPTION,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_KEY,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_TYPE,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_VALUE,[ProductCategory].[ProductCategory].[ProductCategory]LEVEL_NUMBER,[ProductCategory].[ProductCategory].[ProductCategory]LEVEL_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_LEVEL,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_COUNT,[ProductCategory].[ProductCategory].[ProductCategory]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] WHERE ([Measures].[Total Sales]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    const MDX_COLLAPSE_EXCLUDE_REGION: &str = "SELECT NON EMPTY Hierarchize(DrilldownMember(CrossJoin({[Region].[Region].[All],[Region].[Region].[Region].AllMembers}, {([ProductCategory].[ProductCategory].[All])}), {-{[Region].[Region].&[North]}}, [ProductCategory].[ProductCategory])) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_CAPTION,[Region].[Region].[Region]MEMBER_NAME,[Region].[Region].[Region]MEMBER_UNIQUE_NAME,[Region].[Region].[Region]MEMBER_KEY,[Region].[Region].[Region]MEMBER_TYPE,[Region].[Region].[Region]MEMBER_VALUE,[Region].[Region].[Region]LEVEL_NUMBER,[Region].[Region].[Region]LEVEL_UNIQUE_NAME,[Region].[Region].[Region]PARENT_LEVEL,[Region].[Region].[Region]PARENT_UNIQUE_NAME,[Region].[Region].[Region]PARENT_COUNT,[Region].[Region].[Region]CHILDREN_CARDINALITY,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_CAPTION,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_KEY,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_TYPE,[ProductCategory].[ProductCategory].[ProductCategory]MEMBER_VALUE,[ProductCategory].[ProductCategory].[ProductCategory]LEVEL_NUMBER,[ProductCategory].[ProductCategory].[ProductCategory]LEVEL_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_LEVEL,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_UNIQUE_NAME,[ProductCategory].[ProductCategory].[ProductCategory]PARENT_COUNT,[ProductCategory].[ProductCategory].[ProductCategory]CHILDREN_CARDINALITY ON COLUMNS  FROM [Model] WHERE ([Measures].[Total Sales]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";

    fn assert_in_order(haystack: &str, first: &str, second: &str) {
        let f = haystack
            .find(first)
            .unwrap_or_else(|| panic!("missing substring: {first}"));
        let s = haystack
            .find(second)
            .unwrap_or_else(|| panic!("missing substring: {second}"));
        assert!(f < s, "expected '{first}' before '{second}'");
    }

    fn member_block<'a>(xml: &'a str, caption: &str) -> &'a str {
        let member_start = xml
            .find(&format!("<Caption>{caption}</Caption>"))
            .unwrap_or_else(|| panic!("missing Caption: {caption}"));
        let block_start = xml[..member_start]
            .rfind("<Member Hierarchy=")
            .unwrap_or_else(|| panic!("no Member start before Caption: {caption}"));
        let block_end = xml[member_start..]
            .find("</Member>")
            .unwrap_or_else(|| panic!("no </Member> after Caption: {caption}"));
        &xml[block_start..member_start + block_end + "</Member>".len()]
    }

    fn tag_value(block: &str, tag: &str) -> String {
        let open = format!("<{tag}>");
        block
            .find(&open)
            .map(|i| i + open.len())
            .and_then(|start| {
                block[start..]
                    .find(&format!("</{tag}>"))
                    .map(|end| block[start..start + end].to_string())
            })
            .unwrap_or_default()
    }

    /// `(caption, uname, display_info, children_cardinality)` per `<Member>`
    /// on the named axis, in tuple order.
    fn axis_member_infos(xml: &str, axis: &str) -> Vec<(String, String, u32, u32)> {
        let marker = format!("<Axis name=\"{axis}\">");
        let start = xml.find(&marker).expect("missing axis");
        let end = xml[start..]
            .find("</Axis>")
            .map(|i| start + i)
            .unwrap_or(xml.len());
        let slice = &xml[start..end];
        let mut out = Vec::new();
        let mut pos = 0;
        while let Some(i) = slice[pos..].find("<Member Hierarchy=") {
            let abs = pos + i;
            let close = abs + slice[abs..].find("</Member>").unwrap() + "</Member>".len();
            let block = &slice[abs..close];
            let display_info: u32 = tag_value(block, "DisplayInfo").parse().unwrap_or(0);
            // CHILDREN_CARDINALITY is only emitted when the query asks for it
            // (the reference behaves the same); Excel reads the child count
            // from DisplayInfo's low 16 bits, so fall back to those.
            let children: u32 = tag_value(block, "CHILDREN_CARDINALITY")
                .parse()
                .unwrap_or(display_info & 0xFFFF);
            out.push((
                tag_value(block, "Caption"),
                tag_value(block, "UName"),
                display_info,
                children,
            ));
            pos = close;
        }
        out
    }

    fn axis0_member_infos(xml: &str) -> Vec<(String, String, u32, u32)> {
        axis_member_infos(xml, "Axis0")
    }

    // ---- demo-data expectations ----
    //
    // The demo dates are bounded to the past (CURRENT_DATE) and spread evenly
    // across months, so tests derive the data-bearing member sets and values
    // from the seeded database instead of hardcoding year ranges.

    fn demo_scalar(sql: &str) -> f64 {
        crate::backend::Backend::test_fixture().query_scalar(sql)
    }

    fn demo_count(sql: &str) -> u32 {
        crate::backend::Backend::test_fixture().query_count(sql)
    }

    /// Distinct years with facts, ascending.
    fn data_year_keys() -> Vec<String> {
        crate::backend::Backend::test_fixture().query_strings(
            "SELECT DISTINCT CAST(d.year AS VARCHAR) FROM sales_fact f \
             JOIN date_dim d ON f.date_key = d.date_key ORDER BY d.year",
        )
    }

    /// Distinct `year|quarter` paths with facts, ascending.
    fn data_quarter_keys() -> Vec<String> {
        crate::backend::Backend::test_fixture().query_strings(
            "SELECT DISTINCT CAST(d.year AS VARCHAR) || '|' || CAST(d.quarter AS VARCHAR) \
             FROM sales_fact f JOIN date_dim d ON f.date_key = d.date_key \
             ORDER BY d.year, d.quarter",
        )
    }

    /// Distinct `year|quarter|month` paths with facts, ascending.
    fn data_month_keys() -> Vec<String> {
        crate::backend::Backend::test_fixture().query_strings(
            "SELECT DISTINCT CAST(d.year AS VARCHAR) || '|' || CAST(d.quarter AS VARCHAR) \
             || '|' || CAST(d.month AS VARCHAR) \
             FROM sales_fact f JOIN date_dim d ON f.date_key = d.date_key \
             ORDER BY d.year, d.quarter, d.month",
        )
    }

    fn demo_year_revenue(year: i32) -> f64 {
        demo_scalar(&format!(
            "SELECT COALESCE(SUM(f.revenue),0) FROM sales_fact f \
             JOIN date_dim d ON f.date_key = d.date_key WHERE d.year = {year}"
        ))
    }

    fn demo_quarter_revenue(year: i32, quarter: i32) -> f64 {
        demo_scalar(&format!(
            "SELECT COALESCE(SUM(f.revenue),0) FROM sales_fact f \
             JOIN date_dim d ON f.date_key = d.date_key \
             WHERE d.year = {year} AND d.quarter = {quarter}"
        ))
    }

    fn demo_month_revenue(year: i32, month: i32) -> f64 {
        demo_scalar(&format!(
            "SELECT COALESCE(SUM(f.revenue),0) FROM sales_fact f \
             JOIN date_dim d ON f.date_key = d.date_key \
             WHERE d.year = {year} AND d.month = {month}"
        ))
    }

    fn demo_quarter_value_revenue(quarter: i32) -> f64 {
        demo_scalar(&format!(
            "SELECT COALESCE(SUM(f.revenue),0) FROM sales_fact f \
             JOIN date_dim d ON f.date_key = d.date_key WHERE d.quarter = {quarter}"
        ))
    }

    fn demo_month_value_revenue(month: i32) -> f64 {
        demo_scalar(&format!(
            "SELECT COALESCE(SUM(f.revenue),0) FROM sales_fact f \
             JOIN date_dim d ON f.date_key = d.date_key WHERE d.month = {month}"
        ))
    }

    fn with_project3<T>(f: impl FnOnce() -> T) -> T {
        let project =
            ProxyProject::load("projects/project3/proxy-config.json").expect("load project3");
        with_test_project(project, f)
    }

    fn with_retail_analytics<T>(f: impl FnOnce() -> T) -> T {
        let project = ProxyProject::load("projects/generated_retail_analytics/proxy-config.json")
            .expect("load generated_retail_analytics");
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
            stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, f64>(2)?,
                ))
            })
            .expect("query_map query_pairs")
            .filter_map(|r| r.ok())
            .collect()
        }

        fn query_count(&self, sql: &str) -> u32 {
            let conn = self.0.lock().unwrap();
            conn.query_row(sql, [], |r| r.get(0)).unwrap_or(0)
        }

        fn query_strings(&self, sql: &str) -> Vec<String> {
            let conn = self.0.lock().unwrap();
            let mut stmt = conn.prepare(sql).expect("prepare query_strings");
            stmt.query_map([], |r| r.get::<_, String>(0))
                .expect("query_map query_strings")
                .filter_map(|r| r.ok())
                .collect()
        }

        fn query_rows(&self, sql: &str) -> Vec<Vec<String>> {
            let conn = self.0.lock().unwrap();
            let upper = sql.to_uppercase();
            let from_pos = upper.find("FROM ").unwrap_or(0);
            let after_from = &sql[from_pos + 5..].trim();
            let table = after_from.split_whitespace().next().unwrap_or("?");
            let pragma = format!("SELECT count(*) FROM pragma_table_info('{table}')");
            let col_count: usize = conn.query_row(&pragma, [], |r| r.get(0)).unwrap_or(0);
            let mut stmt = conn.prepare(sql).expect("prepare query_rows");
            if col_count > 0 {
                stmt.query_map([], move |r| {
                    let mut cols = Vec::with_capacity(col_count);
                    for i in 0..col_count {
                        cols.push(crate::backend::val_to_string(
                            r.get::<_, duckdb::types::Value>(i)
                                .unwrap_or(duckdb::types::Value::Null),
                        ));
                    }
                    Ok(cols)
                })
                .expect("query_map query_rows")
                .filter_map(|r| r.ok())
                .collect()
            } else {
                vec![]
            }
        }

        fn query_column_names(&self, sql: &str) -> Vec<String> {
            let conn = self.0.lock().unwrap();
            let upper = sql.to_uppercase();
            let from_pos = upper.find("FROM ").unwrap_or(0);
            let after_from = &sql[from_pos + 5..].trim();
            let table = after_from.split_whitespace().next().unwrap_or("?");
            let pragma = format!("SELECT name FROM pragma_table_info('{table}') ORDER BY cid");
            let mut stmt = conn.prepare(&pragma).expect("prepare pragma_table_info");
            stmt.query_map([], |r| r.get::<_, String>(0))
                .expect("query_map pragma")
                .filter_map(|r| r.ok())
                .collect()
        }
    }

    fn axis_captions(xml: &str, axis_name: &str) -> Vec<String> {
        let first = xml
            .find(&format!(r#"name="{axis_name}""#))
            .unwrap_or_else(|| panic!("missing {axis_name}"));
        let second = xml[first + 1..]
            .find(&format!(r#"name="{axis_name}""#))
            .unwrap_or_else(|| panic!("missing second {axis_name}"));
        let start = first + 1 + second;
        let end = xml[start..]
            .find("</Axis>")
            .map(|i| start + i)
            .unwrap_or(xml.len());
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
        let first = xml
            .find(&format!(r#"name="{axis_name}""#))
            .unwrap_or_else(|| panic!("missing {axis_name}"));
        let second = xml[first + 1..]
            .find(&format!(r#"name="{axis_name}""#))
            .unwrap_or_else(|| panic!("missing second {axis_name}"));
        let start = first + 1 + second;
        let end = xml[start..]
            .find("</Axis>")
            .map(|i| start + i)
            .unwrap_or(xml.len());
        let slice = &xml[start..end];

        slice
            .split("<Tuple>")
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
        let end = xml[start..]
            .find("</CellData>")
            .map(|i| start + i)
            .unwrap_or(xml.len());
        let slice = &xml[start..end];
        let mut values = Vec::new();
        let mut pos = 0;
        while let Some(i) = slice[pos..].find("<Value") {
            let open_end = slice[pos + i..].find(">").unwrap() + pos + i + 1;
            let close = slice[open_end..].find("</Value>").unwrap() + open_end;
            values.push(slice[open_end..close].trim().parse().unwrap_or(f64::NAN));
            pos = close + "</Value>".len();
        }
        values
    }

    fn cell_format_strings(xml: &str) -> Vec<String> {
        let start = xml.find("<CellData>").expect("missing CellData");
        let end = xml[start..]
            .find("</CellData>")
            .map(|i| start + i)
            .unwrap_or(xml.len());
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

    /// Raw-SQL groups plus the `(All)` member the reference returns first for a
    /// `DrilldownLevel({All})` axis (Excel's Grand Total).
    fn query_grouped(sql: &str) -> (Vec<String>, Vec<f64>) {
        let rows = Backend::test_fixture().query_grouped_1d(sql);
        let mut captions: Vec<String> = vec!["All".to_string()];
        let mut values: Vec<f64> = vec![rows.iter().map(|(_, value)| *value).sum()];
        captions.extend(rows.iter().map(|(name, _)| name.clone()));
        values.extend(rows.iter().map(|(_, value)| *value));
        (captions, values)
    }

    fn query_pairs(sql: &str) -> (Vec<Vec<String>>, Vec<f64>) {
        let rows = Backend::test_fixture().query_pairs(sql);
        let tuples = rows
            .iter()
            .map(|(first, second, _)| vec![first.clone(), second.clone()])
            .collect();
        let values = rows.iter().map(|(_, _, value)| *value).collect();
        (tuples, values)
    }

    #[test]
    fn concurrent_execute_cellset_with_injected_backends() {
        with_project3(|| {
            let source = crate::backend::BackendSource::demo().expect("create demo source");
            let mut handles = Vec::new();
            for mdx in [
                MDX_SLICER_ALL,
                MDX_DRILLDOWN,
                MDX_REGION_DRILLDOWN,
                MDX_CROSSJOIN_PROBE,
            ] {
                let source = source.clone();
                handles.push(std::thread::spawn(move || {
                    with_project3(|| {
                        let backend = source.checkout();
                        crate::execute_builders::get_execute_cellset_response_with_backend(
                            mdx,
                            backend.as_ref(),
                            &crate::proxy_project::project().model,
                        )
                    })
                }));
            }

            for handle in handles {
                let xml = handle.join().expect("join execute worker");
                assert!(xml.contains("<CellData>"), "missing cell data in {xml}");
            }
            let _ = std::fs::remove_file(source.path());
        });
    }

    /// The reference shape for `DrilldownMember(CrossJoin({All, members},
    /// {(child.All)}), {-{excluded}}, child)`: the root `(All, All)` tuple, then
    /// per parent its own `(parent, All)` aggregate followed by the children —
    /// a collapsed parent keeps only its aggregate (plan 049).
    fn collapse_first_dimension(sql: &str, excluded: &str) -> (Vec<Vec<String>>, Vec<f64>) {
        let rows = Backend::test_fixture().query_pairs(sql);
        let mut tuples = Vec::new();
        let mut values = Vec::new();
        let grand_total: f64 = rows.iter().map(|(_, _, v)| *v).sum();
        tuples.push(vec!["All".to_string(), "All".to_string()]);
        values.push(grand_total);

        let mut i = 0;
        while i < rows.len() {
            let parent = rows[i].0.clone();
            let start = i;
            let mut total = 0.0;
            while i < rows.len() && rows[i].0 == parent {
                total += rows[i].2;
                i += 1;
            }
            tuples.push(vec![parent.clone(), "All".to_string()]);
            values.push(total);
            if parent == excluded {
                continue;
            }
            for (_, second, value) in &rows[start..i] {
                tuples.push(vec![parent.clone(), second.clone()]);
                values.push(*value);
            }
        }

        (tuples, values)
    }

    fn tuple_value_map(
        tuples: &[Vec<String>],
        values: &[f64],
        swap: bool,
    ) -> BTreeMap<(String, String), f64> {
        tuples
            .iter()
            .zip(values.iter())
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
        assert!(
            xml.contains("urn:schemas-microsoft-com:xml-analysis:mddataset"),
            "must use mddataset, not flat rowset"
        );
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
            assert!(
                props.iter().any(|p| p == name),
                "missing dimension property: {name}"
            );
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
        assert_eq!(
            props,
            vec!["VALUE", "FORMAT_STRING", "BACK_COLOR", "FORE_COLOR"]
        );
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
        assert_eq!(filters[0].dimension, "ProductCategory");
        assert_eq!(filters[0].members, vec!["Category A"]);
    }

    #[test]
    fn parse_mdx_filters_extracts_multiple_subquery_categories() {
        let filters = parse_mdx_filters(MDX_SUBQUERY_FILTERS);
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].dimension, "ProductCategory");
        assert_eq!(filters[0].members, vec!["Category A", "Category C"]);
    }

    #[test]
    fn parse_mdx_filters_uses_slicer_not_subquery_when_slicer_has_product() {
        let mdx = "SELECT FROM (SELECT ({[ProductCategory].[ProductCategory].&[Category A]}) ON COLUMNS FROM [Model]) WHERE ([ProductCategory].[ProductCategory].&[Category B],[Measures].[Total Sales])";
        let filters = parse_mdx_filters(mdx);
        // Now merges both: WHERE (Category B) + subquery (Category A)
        let kat = filters
            .iter()
            .find(|f| f.dimension == "ProductCategory")
            .unwrap();
        assert_eq!(kat.members.len(), 2);
        assert!(kat.members.contains(&"Category A".to_string()));
        assert!(kat.members.contains(&"Category B".to_string()));
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
        assert_eq!(name, Some("Category B".to_string()));
    }

    #[test]
    fn cchildren_target_is_product_leaf_returns_true_for_leaf_filter() {
        assert!(cchildren_target_is_product_leaf(MDX_CCHILDREN_LEAF));
    }

    #[test]
    fn cchildren_target_is_product_leaf_returns_false_for_all_probe() {
        let mdx_all = "WITH MEMBER [Measures].cChildren As 'AddCalculatedMembers([ProductCategory].[ProductCategory].currentmember.children).count' Set FilteredMembers As '{[ProductCategory].[ProductCategory].[(All)].Members}' Select {[Measures].cChildren} on ROWS, Hierarchize(Generate(FilteredMembers, Ascendants([ProductCategory].[ProductCategory].currentmember))) ON COLUMNS FROM [Model]";
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
        assert_eq!(q.cchildren_leaf_name.as_deref(), Some("Category B"));
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
        assert_in_order(
            &xml,
            "[ProductCategory].[ProductCategory].[All]",
            "[ProductCategory].[ProductCategory].&amp;[Category B]",
        );
    }

    #[test]
    fn leaf_cchildren_response_omits_parent_unique_name_for_all_member() {
        let xml = get_execute_statement_response(MDX_CCHILDREN_LEAF);
        let block = member_block(&xml, "All");
        assert!(
            !block.contains("<PARENT_UNIQUE_NAME>"),
            "All member must NOT emit PARENT_UNIQUE_NAME"
        );
    }

    #[test]
    fn leaf_cchildren_response_includes_parent_unique_name_for_leaf_member() {
        let xml = get_execute_statement_response(MDX_CCHILDREN_LEAF);
        let block = member_block(&xml, "Category B");
        assert!(block.contains(
            "<PARENT_UNIQUE_NAME>[ProductCategory].[ProductCategory].[All]</PARENT_UNIQUE_NAME>"
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
            "SELECT FROM [Model] WHERE ([Measures].[Total Sales]) CELL PROPERTIES VALUE",
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
        assert!(
            caps.iter().any(|c| c == "All"),
            "SlicerAxis should include Region.All as default even when not in WHERE, caps: {:?}",
            caps
        );
    }

    #[test]
    fn slicer_axis_has_region_all_for_kat_rows_query() {
        let xml = get_execute_statement_response(MDX_KAT_ROWS_REGION_ALL);
        let caps = slicer_captions(&xml);
        // Should contain Total Sales (SEK) and All for Region
        assert!(
            caps.iter().any(|c| c.contains("Total Sales")),
            "SlicerAxis missing Total Sales, caps: {:?}",
            caps
        );
        // Second "All" in slicer caps should be Region's All
        let all_count = caps.iter().filter(|c| *c == "All").count();
        assert!(
            all_count >= 1,
            "SlicerAxis missing Region.All, caps: {:?}",
            caps
        );
    }

    #[test]
    fn semantic_query_combined_row_kat_filter_region() {
        let q = semantic_query_from_mdx(MDX_KAT_ROWS_REGION_FILTER);
        assert_eq!(q.kind, SemanticQueryKind::DrilldownCategories);
        assert_eq!(q.row_dimension.as_deref(), Some("ProductCategory"));
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
        assert_eq!(
            q.kind,
            SemanticQueryKind::DrilldownCategories,
            "drilldown with WHERE (Region.All, Measures) must be DrilldownCategories, not {:?}",
            q.kind
        );
    }

    #[test]
    fn drilldown_with_region_all_in_where_has_axis0() {
        let xml = get_execute_statement_response(MDX_KAT_ROWS_REGION_ALL);
        assert!(
            xml.contains("Axis0"),
            "response must have Axis0 with ProductCategory members"
        );
        assert!(xml.contains("[ProductCategory].[ProductCategory]"));
    }

    #[test]
    fn crossjoin_drilldown_has_both_dimensions() {
        let q = semantic_query_from_mdx(MDX_CROSSJOIN_PROBE);
        assert_eq!(q.kind, SemanticQueryKind::DrilldownCategories);
        assert_eq!(q.axis_dimensions, vec!["ProductCategory", "Region"]);
    }

    #[test]
    fn crossjoin_response_has_kategori_a() {
        let xml = get_execute_statement_response(MDX_CROSSJOIN_PROBE);
        assert!(xml.contains("Category A"));
        assert!(xml.contains("North"));
    }

    // Excel CUBECOUNT: WITH MEMBER ... AS 'COUNT(<set>)' must evaluate to the
    // set's member count, not fall into an aggregate grid.
    #[test]
    fn set_count_probe_returns_member_count() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "WITH MEMBER [Measures].[XL_SD] AS 'COUNT([Date].[Calendar].[Year].Members)' SELECT {[Measures].[XL_SD]} ON 0 FROM [Sales] CELL PROPERTIES VALUE",
            );
            assert_eq!(cell_values(&xml), vec![11.0]);
            assert!(xml.contains("XL_SD"), "calculated member name on axis");
        });
    }

    // Excel CUBESET validation probe: HEAD(set,1) must prune to exactly one
    // level-qualified member (the first year).
    #[test]
    fn head_probe_returns_first_member_only() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT {HEAD([Date].[Calendar].[Year].Members,1)} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            );
            let infos = axis0_member_infos(&xml);
            assert_eq!(infos.len(), 1, "HEAD(...,1) yields one tuple: {infos:?}");
            assert!(
                infos[0].1.contains("[Date].[Calendar].[Year].&amp;[2020]"),
                "first year, level-qualified: {:?}",
                infos[0]
            );
        });
    }

    #[test]
    fn tail_probe_returns_last_members() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT {TAIL([Date].[Calendar].[Year].Members,2)} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            );
            let infos = axis0_member_infos(&xml);
            let years = data_year_keys();
            assert_eq!(infos.len(), 2, "TAIL(...,2) yields two tuples: {infos:?}");
            let (last, prev) = (
                years[years.len() - 1].clone(),
                years[years.len() - 2].clone(),
            );
            assert!(infos[0].1.contains(&format!("&amp;[{prev}]")), "{infos:?}");
            assert!(infos[1].1.contains(&format!("&amp;[{last}]")), "{infos:?}");
        });
    }

    // An unknown level in a set source must fail closed: counting the physical
    // grain instead would silently return a plausible-but-wrong number.
    #[test]
    fn unknown_set_level_fails_closed() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "WITH MEMBER [Measures].[XL_SD] AS 'COUNT([Date].[Full Date].[Bogus].Members)' SELECT {[Measures].[XL_SD]} ON 0 FROM [Sales] CELL PROPERTIES VALUE",
            );
            // The CellData block must be empty: counting the physical grain
            // would emit a misleading cell value.
            let start = xml.find("<CellData>").expect("CellData element") + "<CellData>".len();
            let end = xml[start..].find("</CellData>").expect("closing CellData") + start;
            assert!(
                xml[start..end].trim().is_empty(),
                "no cell may be emitted for an unknown level: {}",
                &xml[start..end]
            );
        });
    }

    /// Temp project with a parent-child org chart. Returns `(dir, db_path)`;
    /// the caller removes `dir`. The DB is NOT materialized.
    fn parent_child_fixture() -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "mallardcube-pc-{}-{:#x}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("org.duckdb");
        {
            let conn = duckdb::Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE employee_dim (k INT, p INT, name VARCHAR);
                 INSERT INTO employee_dim VALUES
                   (1,NULL,'CEO'),(2,1,'CTO'),(3,1,'CFO'),
                   (4,2,'DevA'),(5,2,'DevB'),(6,3,'FinA'),(9,NULL,'COO');
                 CREATE TABLE fact_emp (employee_key INT, revenue DOUBLE);
                 INSERT INTO fact_emp VALUES (1,1000),(2,10),(4,100),(5,50),(6,30),(9,20);",
            )
            .unwrap();
        }
        let cfg = serde_json::json!({
            "catalog": "ORG",
            "cube": "Org",
            "source_name": "org",
            "table_name": "fact_emp",
            "dialect": "duckdb",
            "db_path": "org.duckdb",
            "relationships": [{
                "fact_table": "default",
                "fact_column": "employee_key",
                "dimension_id": "Employee",
                "dim_table": "employee_dim",
                "dim_column": "k"
            }],
            "dimensions": [{
                "id": "Employee",
                "physical_field": "k",
                "caption": "Employee",
                "description": "",
                "hierarchy_name": "Employee",
                "all_level_name": "(All)",
                "leaf_level_name": "Employee",
                "ordinal": 1,
                "visible": true,
                "has_all": true,
                "cardinality_hint": 10,
                "parent_child": {"key_column": "k", "parent_column": "p"}
            }],
            "measures": [{
                "id": "Revenue",
                "sql_expr": "SUM(revenue)",
                "caption": "Revenue",
                "display_name": "Revenue",
                "description": "",
                "format_string": "0",
                "units": "",
                "ordinal": 1,
                "visible": true,
                "measure_group_name": "Org"
            }]
        });
        std::fs::write(
            dir.join("proxy-config.json"),
            serde_json::to_string_pretty(&cfg).unwrap(),
        )
        .unwrap();
        (dir, db_path)
    }

    /// Number of materialized `Employee__pc_*` columns on the dimension table.
    fn pc_materialized_columns(db_path: &std::path::Path) -> u32 {
        let conn = duckdb::Connection::open(db_path).unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('employee_dim') WHERE name LIKE 'Employee__pc%'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    }

    /// Read-only loads (`qualify`, replay) must never materialize; the serving
    /// load builds the hierarchy once and later starts reuse it.
    #[test]
    fn parent_child_materializes_only_when_serving() {
        let (dir, db_path) = parent_child_fixture();
        let config_path = dir.join("proxy-config.json");
        let config = config_path.to_str().unwrap();

        // Read-only: no columns added, the dimension degrades to flat.
        let p = crate::proxy_project::ProxyProject::load(config).expect("read-only load");
        assert_eq!(pc_materialized_columns(&db_path), 0, "load must not write");
        assert!(
            p.model
                .dim_def_opt("Employee")
                .expect("Employee dim")
                .levels
                .is_empty(),
            "an unmaterialized parent-child dimension stays flat"
        );

        // Serving: materializes the synthetic levels.
        let p = crate::proxy_project::ProxyProject::load_for_serving(config).expect("serving load");
        let levels = p
            .model
            .dim_def_opt("Employee")
            .expect("Employee dim")
            .levels
            .clone();
        assert_eq!(levels.len(), 3, "{levels:?}");
        assert_eq!(
            pc_materialized_columns(&db_path),
            5,
            "path + depth + l1..l3"
        );

        // Second serving start: read-back fast path returns identical levels.
        let again = crate::proxy_project::ProxyProject::load_for_serving(config).expect("reload");
        let again_levels: Vec<(String, String, u32)> = again
            .model
            .dim_def_opt("Employee")
            .expect("Employee dim")
            .levels
            .iter()
            .map(|l| (l.name.clone(), l.column.clone(), l.cardinality))
            .collect();
        let expected: Vec<(String, String, u32)> = levels
            .iter()
            .map(|l| (l.name.clone(), l.column.clone(), l.cardinality))
            .collect();
        assert_eq!(again_levels, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // Parent-child hierarchy: org chart materialized into Level 01..NN
    // columns at project load; probes, rollups and drilldown must behave like
    // an explicit hierarchy (SSAS subtree-sum semantics).
    #[test]
    fn parent_child_org_chart_end_to_end() {
        let (dir, db_path) = parent_child_fixture();
        let config_path = dir.join("proxy-config.json");

        let p = crate::proxy_project::ProxyProject::load_for_serving(config_path.to_str().unwrap())
            .expect("load pc project");

        crate::project::project::with_test_project(p, || {
            let project = crate::proxy_project::project();

            // Model: synthetic levels injected from the recursion.
            let emp = project.model.dim_def_opt("Employee").expect("Employee dim");
            assert_eq!(emp.levels.len(), 3, "{:?}", emp.levels);
            assert_eq!(emp.levels[0].name, "Level 01");
            assert_eq!(emp.levels[0].column, "Employee__pc_l1");
            assert_eq!(emp.levels[0].cardinality, 2, "roots CEO + COO");
            assert_eq!(emp.levels[2].cardinality, 3, "leaves DevA/DevB/FinA");

            let conn = duckdb::Connection::open(&db_path).unwrap();
            let backend = FileQueryBackend(std::sync::Mutex::new(conn));

            // CUBECOUNT probe over Level 02 members.
            let cnt = crate::execute_builders::get_execute_cellset_response_with_backend(
                "WITH MEMBER [Measures].[XL_SD] AS 'COUNT([Employee].[Employee].[Level 02].Members)' SELECT {[Measures].[XL_SD]} ON 0 FROM [Org] CELL PROPERTIES VALUE",
                &backend,
                &project.model,
            );
            assert!(cnt.contains(">2<"), "Level02 count=2: {cnt}");

            // Drilldown from All: roots with SSAS subtree-rollup sums
            // (root 1 subtree = 1000+10+100+50+30+? ; root 9 = 20).
            let dd = crate::execute_builders::get_execute_cellset_response_with_backend(
                "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Employee].[Employee].[All]},,,INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME ON COLUMNS FROM [Org] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE",
                &backend,
                &project.model,
            );
            assert!(
                dd.contains("[Employee].[Employee].[Level 01].&amp;[1]"),
                "compound root uname: {dd}"
            );
            let vals = cell_values(&dd);
            // (All) branch total + one cell per root.
            assert_eq!(
                vals,
                vec![1210.0, 1190.0, 20.0],
                "(All)=1210; CEO subtree=1000+10+100+50+30? no—FinA(6) sits under CFO: 1000+10+100+50=1160+30=1190; COO=20 {dd}"
            );

            // SELF probe on a compound child member.
            let self_xml = crate::xmla::discover::members::get_members_response_with_backend(
                Some("[Employee].[Employee].[Level 02].&[1]&[2]"),
                Some(8),
                &crate::xmla::parser::Restrictions::default(),
                &backend,
                &crate::engine::model::UserContext::admin_default(),
                &project.config,
            );
            assert_eq!(self_xml.matches("<row>").count(), 1, "{self_xml}");
            assert!(
                self_xml.contains("CTO") || self_xml.contains(">2<"),
                "child of root 1: {self_xml}"
            );
        });

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn crossjoin_display_info_is_positionally_correct() {
        let xml = get_execute_statement_response(MDX_CROSSJOIN_PROBE);
        let members = axis0_member_infos(&xml);
        assert!(members.len() >= 4, "expected crossjoin tuples: {members:?}");
        // Both dimensions are single-level: every axis member is a leaf, so
        // nothing may claim children (the old static value claimed 3) and no
        // member claims a child on the axis.
        for (caption, _, di, cc) in &members {
            assert_eq!(*cc, 0, "{caption} is a leaf");
            assert_eq!(di & 0xFFFF, 0, "{caption} must not claim children");
            assert_eq!(di & 0x10000, 0, "{caption} must not claim drilled-down");
        }
        // All members share the (All) parent: PARENT_SAME_AS_PREV on every
        // tuple after the first two members, never on the first tuple.
        for (i, (caption, _, di, _)) in members.iter().enumerate() {
            assert_eq!(
                di & 0x20000 != 0,
                i >= 2,
                "{caption} at position {i}: PARENT_SAME_AS_PREV wrong (di={di})"
            );
        }
    }

    // A subselect restricts the query like a slicer (SSAS semantics).
    #[test]
    fn subselect_restricts_slicer_axis() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT {[Measures].[Revenue]} ON COLUMNS FROM (SELECT {[Date].[Calendar].[Year].&[2024]} ON COLUMNS FROM [Sales])",
            );
            assert_eq!(cell_values(&xml), vec![demo_year_revenue(2024)]);
        });
    }

    // Excel 2016 names a level directly with `.AllMembers`.
    #[test]
    fn all_members_level_set_returns_level_members() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT {[Measures].[Revenue]} ON COLUMNS, {[Date].[Calendar].[Quarter].AllMembers} ON ROWS FROM [Sales]",
            );
            let infos = axis_member_infos(&xml, "Axis1");
            // Level listings are data-driven today: only members with facts
            // are listed (a known gap vs SSAS, which also lists empty members).
            assert_eq!(
                infos.len(),
                data_quarter_keys().len(),
                "{:?}",
                &infos[..2.min(infos.len())]
            );
            assert!(
                infos[0]
                    .1
                    .contains("[Date].[Calendar].[Quarter].&amp;[2020]&amp;[1]"),
                "compound quarter: {}",
                infos[0].1
            );
        });
    }

    // `DrilldownLevel(set, <level>)` starts the drill at that level, with the
    // intermediate levels present so every parent is reachable.
    #[test]
    fn drilldown_level_expression_starts_at_that_level() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Date].[Calendar].[All]}, [Date].[Calendar].[Quarter])}) ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES CELL_ORDINAL",
            );
            let infos = axis0_member_infos(&xml);
            assert_eq!(
                infos.len(),
                1 + data_year_keys().len() + data_quarter_keys().len(),
                "(All) + years + quarters"
            );
            assert!(
                infos[0].1.contains("[Date].[Calendar].[All]"),
                "{:?}",
                infos[0]
            );
            let unames: Vec<&str> = infos.iter().map(|(_, u, _, _)| u.as_str()).collect();
            assert!(unames.contains(&"[Date].[Calendar].[Year].&amp;[2020]"));
            assert!(unames.contains(&"[Date].[Calendar].[Quarter].&amp;[2020]&amp;[1]"));
            assert!(
                !unames.iter().any(|u| u.contains("[Month]")),
                "no months at a quarter-level drill"
            );
        });
    }

    // The key attribute hierarchy is a single-level hierarchy in the tabular
    // shape. Excel drags it as a field and drills it from `(All)`: the axis must
    // list the dates in that hierarchy's own namespace — never the user
    // hierarchy's years (plan 048).
    #[test]
    fn key_attribute_hierarchy_drill_lists_dates_in_its_own_namespace() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Date].[Full Date].[All]},,,INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES CELL_ORDINAL",
            );
            assert!(
                xml.contains("<HierarchyInfo name=\"[Date].[Full Date]\">"),
                "the axis is the attribute hierarchy"
            );
            let infos = axis0_member_infos(&xml);
            let unames: Vec<&str> = infos.iter().map(|(_, u, _, _)| u.as_str()).collect();
            assert_eq!(
                unames.first().copied(),
                Some("[Date].[Full Date].[All]"),
                "(All) root first: {unames:?}"
            );
            assert!(
                unames
                    .iter()
                    .any(|u| u.starts_with("[Date].[Full Date].&amp;[")),
                "date members in the attribute hierarchy namespace: {unames:?}"
            );
            assert!(
                !unames.iter().any(|u| u.contains("[Year]")
                    || u.contains("[Quarter]")
                    || u.contains("[Month]")),
                "no user-hierarchy levels: {unames:?}"
            );
        });
    }

    // Whole-field "Expand to Month": the axis must include years and quarters
    // (the months' parents), or Excel's hierarchy walk has dangling parents.
    #[test]
    fn nested_drill_to_month_keeps_intermediate_levels() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY Hierarchize({DrilldownLevel({DrilldownLevel({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)},[Date].[Calendar].[Year],INCLUDE_CALC_MEMBERS)},[Date].[Calendar].[Month],INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES CELL_ORDINAL",
            );
            let infos = axis0_member_infos(&xml);
            assert_eq!(
                infos.len(),
                1 + data_year_keys().len() + data_quarter_keys().len() + data_month_keys().len(),
                "All + years + quarters + months"
            );
            let unames: Vec<&str> = infos.iter().map(|(_, u, _, _)| u.as_str()).collect();
            assert!(unames.contains(&"[Date].[Calendar].[Year].&amp;[2020]"));
            assert!(unames.contains(&"[Date].[Calendar].[Quarter].&amp;[2020]&amp;[1]"));
            assert!(unames.contains(&"[Date].[Calendar].[Month].&amp;[2020]&amp;[1]&amp;[1]"));

            // Every member's parent must be on the axis.
            let mut pairs: Vec<(String, Option<String>)> = Vec::new();
            let mut pos = 0;
            while let Some(i) = xml[pos..].find("<Member Hierarchy=\"[Date].[Calendar]\">") {
                let start = pos + i;
                let end = start + xml[start..].find("</Member>").unwrap();
                let block = &xml[start..end];
                let u = tag_value(block, "UName");
                let p = tag_value(block, "PARENT_UNIQUE_NAME");
                if !pairs.iter().any(|(u2, _)| *u2 == u) {
                    pairs.push((u, if p.is_empty() { None } else { Some(p) }));
                }
                pos = end;
            }
            for (u, p) in &pairs {
                if let Some(p) = p {
                    assert!(
                        unames.contains(&p.as_str()),
                        "parent {p} of {u} must be on the axis"
                    );
                }
            }
        });
    }

    // Excel probes each level with `AddCalculatedMembers({[D].[H].[Level].Members})`
    // while building the field list's level tree. The members must be
    // level-qualified, or Excel can't place them and collapses to the top level.
    #[test]
    fn add_calculated_members_level_probe_is_level_qualified() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT {AddCalculatedMembers({[Date].[Calendar].[Year].Members})} DIMENSION PROPERTIES MEMBER_TYPE ON COLUMNS FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            );
            let infos = axis0_member_infos(&xml);
            assert_eq!(
                infos.len(),
                data_year_keys().len(),
                "one row per year with data"
            );
            assert!(
                infos[0].1.contains("[Date].[Calendar].[Year].&amp;[2020]"),
                "level-qualified year: {}",
                infos[0].1
            );

            // An unqualified `.Members` set stays at the leaf grain.
            // TODO(plan 048): a set queried from the key attribute hierarchy
            // should render its members under [Date].[Full Date]; today the leaf
            // renders under the user hierarchy (and without the level segment).
            let leaf = get_execute_statement_response(
                "SELECT {AddCalculatedMembers({[Date].[Full Date].Members})} DIMENSION PROPERTIES MEMBER_TYPE ON COLUMNS FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            );
            let leaf_infos = axis0_member_infos(&leaf);
            assert!(
                leaf_infos[0].1.contains("[Date].[Calendar].&amp;[20"),
                "leaf member carries a date key: {}",
                leaf_infos[0].1
            );
        });
    }

    // "Expand Entire Field" expands every member at once — Excel sends all
    // parent members in the DrilldownMember set.
    #[test]
    fn crossjoin_expand_entire_field_expands_every_parent() {
        with_project3(|| {
            let mdx = r#"SELECT NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)}), Hierarchize(DrilldownMember({{DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}}, {[Date].[Calendar].[Year].&[2020],[Date].[Calendar].[Year].&[2021]},,,INCLUDE_CALC_MEMBERS))) DIMENSION PROPERTIES PARENT_UNIQUE_NAME ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE"#;
            let xml = get_execute_statement_response(mdx);
            let infos = axis_member_infos(&xml, "Axis0");
            let date_unames: Vec<&str> = infos
                .iter()
                .skip(1)
                .step_by(2)
                .map(|(_, u, _, _)| u.as_str())
                .collect();
            for year in [
                "[Date].[Calendar].[Year].&amp;[2020]",
                "[Date].[Calendar].[Year].&amp;[2021]",
            ] {
                assert!(date_unames.contains(&year), "missing {year}");
            }
            for quarter in [
                "[Date].[Calendar].[Quarter].&amp;[2020]&amp;[1]",
                "[Date].[Calendar].[Quarter].&amp;[2020]&amp;[4]",
                "[Date].[Calendar].[Quarter].&amp;[2021]&amp;[1]",
                "[Date].[Calendar].[Quarter].&amp;[2021]&amp;[4]",
            ] {
                assert!(date_unames.contains(&quarter), "missing {quarter}");
            }
            // Every quarter's parent must be its own year.
            let mut pairs: Vec<(String, Option<String>)> = Vec::new();
            let mut pos = 0;
            while let Some(i) = xml[pos..].find("<Member Hierarchy=\"[Date].[Calendar]\">") {
                let start = pos + i;
                let end = start + xml[start..].find("</Member>").unwrap();
                let block = &xml[start..end];
                let u = tag_value(block, "UName");
                let p = tag_value(block, "PARENT_UNIQUE_NAME");
                if !pairs.iter().any(|(u2, _)| *u2 == u) {
                    pairs.push((u, if p.is_empty() { None } else { Some(p) }));
                }
                pos = end;
            }
            for (u, p) in &pairs {
                for year in ["2020", "2021"] {
                    if u.contains(&format!("[Quarter].&amp;[{year}]")) {
                        assert_eq!(
                            p.as_deref(),
                            Some(format!("[Date].[Calendar].[Year].&amp;[{year}]").as_str()),
                            "quarter {u} must be parented by year {year}"
                        );
                    }
                }
            }
        });
    }

    #[test]
    fn single_dim_expand_two_years_lists_both_branches() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY Hierarchize(DrilldownMember({{DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}}, {[Date].[Calendar].[Year].&[2020],[Date].[Calendar].[Year].&[2021]},,,INCLUDE_CALC_MEMBERS)) ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE",
            );
            let infos = axis0_member_infos(&xml);
            let unames: Vec<&str> = infos.iter().map(|(_, u, _, _)| u.as_str()).collect();
            assert!(unames.contains(&"[Date].[Calendar].[Year].&amp;[2020]"));
            assert!(unames.contains(&"[Date].[Calendar].[Year].&amp;[2021]"));
            assert!(unames.contains(&"[Date].[Calendar].[Quarter].&amp;[2020]&amp;[1]"));
            assert!(unames.contains(&"[Date].[Calendar].[Quarter].&amp;[2021]&amp;[4]"));
        });
    }

    // A deep drill scoped by a slicer/compound member ("Expand to Month" on a
    // year) must also keep the intermediate quarters on the axis.
    #[test]
    fn deep_drill_with_year_slice_keeps_intermediate_levels() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY Hierarchize({DrilldownLevel({DrilldownLevel({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)},[Date].[Calendar].[Year],INCLUDE_CALC_MEMBERS)},[Date].[Calendar].[Month],INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME ON COLUMNS FROM (SELECT ({[Date].[Calendar].[Year].&[2024]}) ON COLUMNS FROM [Sales]) WHERE ([Measures].[Revenue]) CELL PROPERTIES CELL_ORDINAL",
            );
            let infos = axis0_member_infos(&xml);
            // The subselect slices the cube to 2024, so the input set
            // (`DrilldownLevel({All})`) contains only 2024 plus the expanded
            // branch: All + 2024 + its quarters + its months.
            let quarters = demo_count(
                "SELECT COUNT(DISTINCT d.quarter) FROM sales_fact f JOIN date_dim d ON f.date_key = d.date_key WHERE d.year = 2024",
            ) as usize;
            let months = demo_count(
                "SELECT COUNT(DISTINCT d.month) FROM sales_fact f JOIN date_dim d ON f.date_key = d.date_key WHERE d.year = 2024",
            ) as usize;
            assert_eq!(
                infos.len(),
                1 + 1 + quarters + months,
                "All + 2024 + its quarters + its months"
            );
            let unames: Vec<&str> = infos.iter().map(|(_, u, _, _)| u.as_str()).collect();
            assert!(unames.contains(&"[Date].[Calendar].[Year].&amp;[2024]"));
            assert!(
                !unames.contains(&"[Date].[Calendar].[Year].&amp;[2020]"),
                "years outside the slice are empty and omitted"
            );
            assert!(unames.contains(&"[Date].[Calendar].[Quarter].&amp;[2024]&amp;[1]"));
            assert!(unames.contains(&"[Date].[Calendar].[Month].&amp;[2024]&amp;[1]&amp;[1]"));
            assert!(
                !unames.iter().any(|u| u.matches("[2024]").count() > 1),
                "keys must not repeat the year: {unames:?}"
            );
        });
    }

    // The query Excel sends for "Expand to Month" on a year inside a crossjoin
    // (trace seq 68 of a live session): nested DrilldownMember whose outer set
    // mixes the year with its quarters. Used to panic on mixed-level keys.
    #[test]
    fn mixed_level_member_expand_to_month_inside_crossjoin() {
        with_project3(|| {
            let mdx = r##"SELECT NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Segment].[Segment].[All]},,,INCLUDE_CALC_MEMBERS)}), Hierarchize(DrilldownMember({{DrilldownMember({{DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}}, {[Date].[Calendar].[Year].&[2026]},,,INCLUDE_CALC_MEMBERS)}}, {[Date].[Calendar].[Year].&[2026],[Date].[Calendar].[Quarter].&[2026]&[1],[Date].[Calendar].[Quarter].&[2026]&[2],[Date].[Calendar].[Quarter].&[2026]&[3],[Date].[Calendar].[Quarter].&[2026]&[4]},,,INCLUDE_CALC_MEMBERS))) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,[Date].[Calendar].[Month]CHILDREN_CARDINALITY ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE"##;
            let xml = get_execute_statement_response(mdx);

            let mut pairs: Vec<(String, Option<String>, u32)> = Vec::new();
            let mut pos = 0;
            while let Some(i) = xml[pos..].find("<Member Hierarchy=\"[Date].[Calendar]\">") {
                let start = pos + i;
                let end = start + xml[start..].find("</Member>").unwrap();
                let block = &xml[start..end];
                let u = tag_value(block, "UName");
                let p = tag_value(block, "PARENT_UNIQUE_NAME");
                let cc = tag_value(block, "CHILDREN_CARDINALITY")
                    .parse()
                    .unwrap_or(0);
                if !pairs.iter().any(|(u2, _, _)| *u2 == u) {
                    pairs.push((u, if p.is_empty() { None } else { Some(p) }, cc));
                }
                pos = end;
            }
            let unames: Vec<&str> = pairs.iter().map(|(u, _, _)| u.as_str()).collect();
            assert!(
                unames.contains(&"[Date].[Calendar].[Year].&amp;[2026]"),
                "{unames:?}"
            );
            assert!(
                unames.contains(&"[Date].[Calendar].[Quarter].&amp;[2026]&amp;[1]"),
                "{unames:?}"
            );
            assert!(
                unames.contains(&"[Date].[Calendar].[Month].&amp;[2026]&amp;[1]&amp;[1]"),
                "months are expanded: {unames:?}"
            );
            for (u, p, cc) in &pairs {
                if let Some(p) = p {
                    assert!(
                        unames.contains(&p.as_str()),
                        "parent {p} of {u} must be on the axis"
                    );
                }
                // A month's children are days, never the static whole-level
                // cardinality (a wrong count corrupts Excel's hierarchy tree).
                if u.contains("[Month]") {
                    assert!((1..=31).contains(cc), "month {u} claims {cc} children");
                }
            }
        });
    }

    // Excel asks for member properties qualified by the level unique name
    // (`[Category].[Category].[Category]MEMBER_CAPTION`); a hardcoded whitelist
    // used to drop four of them, leaving requested properties missing.
    #[test]
    fn standard_member_properties_are_not_duplicated() {
        // Excel's cellset parser rejects a member that carries MEMBER_CAPTION /
        // MEMBER_UNIQUE_NAME / LEVEL_NUMBER / LEVEL_UNIQUE_NAME as extra
        // property elements: those values already travel in the standard
        // Caption / UName / LNum / LName tags, and the duplicates are not
        // declared in HierarchyInfo. Excel then refuses to add a pivot field
        // (plan 048: found by bisecting a working August build).
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT {[Measures].[Revenue]} ON COLUMNS, [Category].[Category].Members DIMENSION PROPERTIES PARENT_UNIQUE_NAME,[Category].[Category].[Category]MEMBER_CAPTION,[Category].[Category].[Category]MEMBER_UNIQUE_NAME,[Category].[Category].[Category]LEVEL_NUMBER,[Category].[Category].[Category]LEVEL_UNIQUE_NAME ON ROWS FROM [Sales]",
            );
            assert!(xml.contains("<Caption>Automotive</Caption>"), "{xml}");
            assert!(xml.contains("<LNum>1</LNum>"), "{xml}");
            assert!(
                xml.contains("<LName>[Category].[Category].[Category]</LName>"),
                "{xml}"
            );
            assert!(!xml.contains("<MEMBER_CAPTION>"), "{xml}");
            assert!(!xml.contains("<MEMBER_UNIQUE_NAME>"), "{xml}");
            assert!(!xml.contains("<LEVEL_NUMBER>"), "{xml}");
            assert!(!xml.contains("<LEVEL_UNIQUE_NAME>"), "{xml}");
        });
    }

    #[test]
    fn crossjoin_expand_year_keeps_ancestor_chain() {
        // The shape Excel sends when a year is expanded inside a crossjoin
        // (trace seq 50 from a live session). Quarters must be keyed by their
        // year and every parent must be present on the axis — a missing parent
        // crashes Excel's MDDSAxis walk.
        with_project3(|| {
            let mdx = r#"SELECT NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)}), Hierarchize(DrilldownMember({{DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}}, {[Date].[Calendar].[Year].&[2026]},,,INCLUDE_CALC_MEMBERS))) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,Hierarchy_UNIQUE_NAME ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE"#;
            let xml = get_execute_statement_response(mdx);

            let mut date_members: Vec<(String, Option<String>)> = Vec::new();
            let mut pos = 0;
            while let Some(i) = xml[pos..].find("<Member Hierarchy=\"[Date].[Calendar]\">") {
                let start = pos + i;
                let end = start + xml[start..].find("</Member>").unwrap();
                let block = &xml[start..end];
                let u = tag_value(block, "UName");
                let p = tag_value(block, "PARENT_UNIQUE_NAME");
                date_members.push((u, if p.is_empty() { None } else { Some(p) }));
                pos = end;
            }
            let unames: Vec<&str> = date_members.iter().map(|(u, _)| u.as_str()).collect();
            assert!(
                unames.contains(&"[Date].[Calendar].[All]"),
                "the (All) root is on the axis: {unames:?}"
            );
            assert!(
                unames.contains(&"[Date].[Calendar].[Year].&amp;[2026]"),
                "the expanded year is on the axis: {unames:?}"
            );
            assert!(
                unames.contains(&"[Date].[Calendar].[Quarter].&amp;[2026]&amp;[1]"),
                "quarters are keyed by their year: {unames:?}"
            );
            for (u, p) in &date_members {
                if let Some(p) = p {
                    assert!(
                        unames.contains(&p.as_str()),
                        "parent {p} of {u} must be on the axis"
                    );
                }
            }
            // The expanded year claims DRILLED_DOWN (a child follows it on the
            // axis) so Excel renders the expand state instead of flat years.
            let year_pos = xml
                .find("<UName>[Date].[Calendar].[Year].&amp;[2026]</UName>")
                .expect("year member");
            let block_start = xml[..year_pos].rfind("<Member ").expect("member start");
            let block_end = block_start + xml[block_start..].find("</Member>").unwrap();
            let di: u32 = tag_value(&xml[block_start..block_end], "DisplayInfo")
                .parse()
                .unwrap_or(0);
            assert_eq!(di & 0x10000, 0x10000, "year must be drilled down (di={di})");
        });
    }

    #[test]
    fn crossjoin_leveled_second_dim_is_at_top_level() {
        // Excel emits the same CrossJoin(DrilldownLevel, DrilldownLevel) shape
        // whatever the dimension order; a leveled dimension in the *second*
        // slot must also be served at its top level (years), not the leaf grain.
        with_project3(|| {
            let mdx = r#"SELECT NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)}), Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)})) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,Hierarchy_UNIQUE_NAME ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE"#;
            let xml = get_execute_statement_response(mdx);
            let members = axis0_member_infos(&xml);
            // 20 categories x (years with facts).
            assert_eq!(
                members.len(),
                20 * data_year_keys().len() * 2,
                "category x year tuples"
            );

            for (caption, uname, _, _) in members.iter().step_by(2) {
                assert!(
                    uname.contains("[Category].[Category].&amp;["),
                    "{caption}: first slot must be a category, got {uname}"
                );
            }
            for (caption, uname, _, cc) in members.iter().skip(1).step_by(2) {
                assert!(
                    uname.contains("[Date].[Calendar].[Year].&amp;["),
                    "{caption}: second slot must be a year, got {uname}"
                );
                assert!(*cc > 0, "{caption}: year reports quarter children");
            }
        });
    }

    #[test]
    fn quarter_level_drag_returns_compound_members() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT {[Measures].[Revenue]} ON COLUMNS, [Date].[Calendar].[Quarter].Members ON ROWS FROM [Sales]",
            );
            // The measures sit on Axis0 (the edge the statement asked for), so
            // the quarter members are Axis1 (plan 049).
            assert_eq!(axis0_member_infos(&xml).len(), 1, "measures axis");
            let infos = axis_member_infos(&xml, "Axis1");
            let quarters = data_quarter_keys();
            assert_eq!(
                infos.len(),
                quarters.len(),
                "one member per quarter with facts"
            );
            assert_eq!(infos[0].0, "1", "caption is the level value");
            assert!(
                infos[0]
                    .1
                    .contains("[Date].[Calendar].[Quarter].&amp;[2020]&amp;[1]"),
                "compound unique name: {}",
                infos[0].1
            );
            let last = quarters.last().expect("at least one quarter");
            let parts: Vec<&str> = last.split('|').collect();
            let last_uname = &infos[infos.len() - 1].1;
            assert!(
                last_uname.contains(&format!("&amp;[{}]&amp;[{}]", parts[0], parts[1])),
                "last quarter: {last_uname}"
            );
            assert_eq!(
                cell_values(&xml).len(),
                quarters.len(),
                "one value per quarter"
            );
        });
    }

    #[test]
    fn month_level_drag_orders_numerically() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT {[Measures].[Revenue]} ON COLUMNS, [Date].[Calendar].[Month].Members ON ROWS FROM [Sales]",
            );
            assert_eq!(axis0_member_infos(&xml).len(), 1, "measures axis");
            let infos = axis_member_infos(&xml, "Axis1");
            assert_eq!(
                infos.len(),
                data_month_keys().len(),
                "one member per month with facts"
            );
            // Months order 1..12, not lexicographically ("1","10","11","12","2").
            assert!(
                infos[11].1.contains("&amp;[12]"),
                "12th member is the first year's month 12: {}",
                infos[11].1
            );
            assert!(
                infos[12].1.contains("[2021]") && infos[12].1.contains("&amp;[1]"),
                "13th member starts the next year: {}",
                infos[12].1
            );
        });
    }

    #[test]
    fn crossjoin_leveled_first_dim_emits_level_qualified_members() {
        with_project3(|| {
            let mdx = r#"SELECT NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}), Hierarchize({DrilldownLevel({[Territory].[Territory].[All]},,,INCLUDE_CALC_MEMBERS)})) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,[Date].[Calendar].[Full Date]MEMBER_CAPTION,[Date].[Calendar].[Full Date]MEMBER_UNIQUE_NAME,[Date].[Calendar].[Full Date]LEVEL_NUMBER,[Date].[Calendar].[Full Date]LEVEL_UNIQUE_NAME,[Date].[Calendar].[Full Date]PARENT_LEVEL,[Date].[Calendar].[Full Date]CHILDREN_CARDINALITY,[Territory].[Territory].[Territory]MEMBER_CAPTION,[Territory].[Territory].[Territory]MEMBER_UNIQUE_NAME ON COLUMNS  FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE, FORMAT_STRING"#;
            let xml = get_execute_statement_response(mdx);
            let members = axis0_member_infos(&xml);
            assert!(
                members.len() >= 2,
                "expected year x territory tuples: {members:?}"
            );

            // Slot 0 = Date years: level-qualified unique names matching the
            // grouped data grain, real quarter child counts in the DISPLAY_INFO
            // low word, no drilled-down claim.
            for (caption, uname, di, cc) in members.iter().step_by(2) {
                assert!(
                    uname.contains("[Date].[Calendar].[Year].&amp;["),
                    "year uname must be level-qualified: {uname}"
                );
                assert!(*cc > 0, "a year reports its quarter count: {caption}");
                assert_eq!(di & 0xFFFF, *cc, "{caption}: low word carries cc");
                assert_eq!(di & 0x10000, 0, "{caption}: no child follows on axis");
            }

            // Slot 1 = territories: plain leaves, no phantom "+".
            for (caption, _, di, cc) in members.iter().skip(1).step_by(2) {
                assert_eq!(*cc, 0, "{caption} is a territory leaf");
                assert_eq!(di & 0xFFFF, 0, "{caption} must not claim children");
            }

            // The first year's parent metadata points at the (All) root.
            let (ycap, _, _, _) = members.first().expect("non-empty axis");
            let block = member_block(&xml, ycap);
            assert_eq!(tag_value(block, "PARENT_LEVEL"), "0");
            assert_eq!(
                tag_value(block, "PARENT_UNIQUE_NAME"),
                "[Date].[Calendar].[All]"
            );
        });
    }

    #[test]
    fn kat_filter_single_returns_only_filtered_category() {
        let xml = get_execute_statement_response(MDX_KAT_FILTERED_SINGLE);
        assert!(xml.contains("Category B"));
        assert!(
            !xml.contains("Category A"),
            "Category A should be filtered out"
        );
        assert!(
            !xml.contains("Category C"),
            "Category C should be filtered out"
        );
    }

    #[test]
    fn kat_filter_single_returns_correct_value() {
        let xml = get_execute_statement_response(MDX_KAT_FILTERED_SINGLE);
        // Category B total across all regions = 150000 + 100000 = 250000
        assert!(xml.contains("250000"));
    }

    #[test]
    fn nested_filters_parse_both_dimensions() {
        let filters = parse_mdx_filters(MDX_NESTED_BOTH_FILTERS);
        let kat = filters
            .iter()
            .find(|f| f.dimension == "ProductCategory")
            .map(|f| &f.members)
            .unwrap();
        let reg = filters
            .iter()
            .find(|f| f.dimension == "Region")
            .map(|f| &f.members)
            .unwrap();
        assert!(kat.contains(&"Category A".to_string()));
        assert!(kat.contains(&"Category B".to_string()));
        assert!(kat.contains(&"Category D".to_string()));
        assert!(
            !kat.contains(&"Category C".to_string()),
            "Category C should be filtered out"
        );
        assert_eq!(reg, &vec!["North"]);
    }

    #[test]
    fn nested_filters_response_shows_region_rows_only() {
        let xml = get_execute_statement_response(MDX_NESTED_BOTH_FILTERS);
        // Region on rows with both filters: North only, Category A/B/D filtered
        assert!(xml.contains("North"));
        // Total: North + (A,B,D) = 100000 + 150000 + 200000 = 450000
        assert!(xml.contains("450000"));
    }

    #[test]
    fn collapse_detected() {
        let q = semantic_query_from_mdx(MDX_DRILLDOWN_MEMBER_COLLAPSE);
        assert_eq!(q.kind, SemanticQueryKind::DrilldownMemberProbe);
        assert_eq!(q.excluded_members.len(), 1);
        assert_eq!(q.excluded_members[0].dimension, "ProductCategory");
        assert_eq!(q.excluded_members[0].key, "Category A");
        assert_eq!(q.drilldown_member_hierarchy.as_deref(), Some("Region"));
    }

    #[test]
    fn collapse_kategori_a_keeps_all_tuple() {
        let xml = get_execute_statement_response(MDX_DRILLDOWN_MEMBER_COLLAPSE);
        // Category A should remain, but collapsed to (Category A, Region.All)
        assert!(
            xml.contains("Category A"),
            "Category A should remain visible as All"
        );
        // Region.All captions should appear (from the collapsed Kat A tuple)
        let _caps = slicer_captions(&xml);
        // Axis0 should have Category A present
        assert!(xml.contains("Category A"));
    }

    #[test]
    fn collapse_kategori_a_removes_region_leaf_tuples_for_a() {
        let xml = get_execute_statement_response(MDX_DRILLDOWN_MEMBER_COLLAPSE);
        // Category A should NOT have North/South leaf tuples — those should be gone
        assert!(xml.contains("Category B"));
        assert!(xml.contains("Category C"));
        assert!(xml.contains("Category D"));
        // The count of tuples should match: 1(A+All) + 3*2(BCxDregion) = 7
    }

    #[test]
    fn collapse_product_category_detected() {
        let q = semantic_query_from_mdx(MDX_DRILLDOWN_MEMBER_COLLAPSE_PRODUCT);
        assert_eq!(q.kind, SemanticQueryKind::DrilldownMemberProbe);
        assert_eq!(
            q.drilldown_member_hierarchy.as_deref(),
            Some("ProductCategory")
        );
        assert_eq!(q.excluded_members.len(), 1);
        assert_eq!(q.excluded_members[0].dimension, "ProductCategory");
        assert_eq!(q.excluded_members[0].key, "Category D");
    }

    #[test]
    fn collapse_product_category_keeps_d_visible_as_all() {
        let xml = get_execute_statement_response(MDX_DRILLDOWN_MEMBER_COLLAPSE_PRODUCT);
        // Category D should appear as (All, Region) — All caption present
        assert!(xml.contains("Category B"), "B should remain");
        assert!(xml.contains("Category C"), "C should remain");
    }

    #[test]
    fn collapse_product_category_removes_d_leaf_tuples() {
        let xml = get_execute_statement_response(MDX_DRILLDOWN_MEMBER_COLLAPSE_PRODUCT);
        let all_count = xml.matches("All").count();
        assert!(
            all_count >= 2,
            "Expected at least 2 All captions from collapsed tuples"
        );
    }

    #[test]
    fn collapse_region_all_member_on_axis0_has_properties() {
        let xml = get_execute_statement_response(MDX_DRILLDOWN_MEMBER_COLLAPSE);
        let all_block = member_block(&xml, "All");
        assert!(
            all_block.contains("<HIERARCHY_UNIQUE_NAME>"),
            "Axis0 All member must have HIERARCHY_UNIQUE_NAME"
        );
    }

    // --- axis-order awareness (regression test for reversed row order) ---

    #[test]
    fn parse_axis_dimensions_preserves_forward_order() {
        let q = semantic_query_from_mdx(MDX_CROSSJOIN_PROBE);
        assert_eq!(q.axis_dimensions, vec!["ProductCategory", "Region"]);
    }

    #[test]
    fn parse_axis_dimensions_preserves_reversed_order() {
        let q = semantic_query_from_mdx(MDX_CROSSJOIN_REGION_FIRST);
        assert_eq!(q.axis_dimensions, vec!["Region", "ProductCategory"]);
    }

    #[test]
    fn reversed_crossjoin_puts_region_first_in_tuple() {
        let xml = get_execute_statement_response(MDX_CROSSJOIN_REGION_FIRST);
        // First hierarchy in AxisInfo should be Region
        let region_pos = xml.find("[Region].[Region]").unwrap();
        let kat_pos = xml.find("[ProductCategory].[ProductCategory]").unwrap();
        assert!(
            region_pos < kat_pos,
            "AxisInfo must list Region hierarchy first when Region is first in rows"
        );
    }

    #[test]
    fn reversed_collapse_keeps_excluded_visible() {
        let xml = get_execute_statement_response(MDX_COLLAPSE_REGION_FIRST);
        // ProductCategory Category B is excluded — should appear as
        // (Region leaf, ProductCategory.All) in axis-dimension order.
        // ProductCategory.All has caption "All".
        assert!(
            xml.contains("All"),
            "All caption should appear for collapsed member"
        );
        assert!(xml.contains("North"), "Region members should still appear");
    }

    #[test]
    fn reversed_crossjoin_has_correct_hierarchy_order_in_tuples() {
        let xml = get_execute_statement_response(MDX_CROSSJOIN_REGION_FIRST);
        // The first member in the first tuple of Axis0 should be from Region
        let first_tuple = xml.split("<Tuple>").nth(1).unwrap();
        let first_hier = first_tuple.split("<Member Hierarchy=").nth(1).unwrap();
        assert!(
            first_hier.starts_with("\"[Region].[Region]\""),
            "First tuple member should be Region when Region is first in rows, got: {first_hier}"
        );
    }

    #[test]
    fn reversed_crossjoin_semantic_values_not_swapped() {
        let xml = get_execute_statement_response(MDX_CROSSJOIN_REGION_FIRST);
        // Region hierarchy must contain region captions (North, South), not Category names
        // Find the first Region member in the first tuple
        let _cap_region = xml.split("<Caption>Region").next().unwrap_or("");
        // Region captions like "North", "South" should appear
        assert!(
            xml.contains("<Caption>North</Caption>"),
            "Region hierarchy must show region names"
        );
        assert!(
            xml.contains("<Caption>Category A</Caption>"),
            "ProductCategory hierarchy must show category names"
        );
        // A Category name must NOT appear as a Region member caption
        // (Region captions should be North/South, not Category X)
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
                EXCEL_TRACE_TERRITORY_CATEGORY_COLLAPSE_NORTHWEST_REVENUE,
            );
            // Only the one explicit exclusion from DrilldownMember, not the
            // later slicer members for Segment/Channel.
            assert_eq!(
                query.excluded_members.len(),
                1,
                "should only exclude the DrilldownMember member, not slicer members"
            );
            assert_eq!(query.excluded_members[0].key, "Northwest");
            assert_eq!(query.excluded_members[0].dimension, "Territory");
        });
    }

    #[test]
    fn collapse_exclude_region_keeps_north_visible_as_all() {
        let xml = get_execute_statement_response(MDX_COLLAPSE_EXCLUDE_REGION);
        // North is excluded from Region — should appear as (Region leaf, ProductCategory.All)
        // "All" caption should appear for the collapsed ProductCategory member
        assert!(
            xml.contains("All"),
            "All caption should appear for collapsed ProductCategory"
        );
        // North should remain visible under Region hierarchy
        assert!(
            xml.contains("North"),
            "Excluded Region member North should still appear"
        );
    }

    #[test]
    fn collapse_exclude_region_uses_region_total() {
        let xml = get_execute_statement_response(MDX_COLLAPSE_EXCLUDE_REGION);
        // North total across all categories: 100000 + 150000 + 200000 + 200000 = 650000
        assert!(
            xml.contains("650000"),
            "North collapsed row should show region total 650000"
        );
    }

    // --- two-leaf-filter regression (project3 crash reproducer) ---

    #[test]
    fn two_leaf_filters_semantic() {
        let q = semantic_query_from_mdx(MDX_TWO_LEAF_FILTERS_UNITS);
        assert_eq!(q.kind, SemanticQueryKind::DrilldownCategories);
        // Axis dimensions are model-driven; at minimum verify the query
        // shape is recognized (not classified as SlicerOnly or similar).
        assert!(
            !q.filters.is_empty(),
            "should have at least one extracted filter"
        );
    }

    #[test]
    fn two_leaf_filters_plan() {
        let q = semantic_query_from_mdx(MDX_TWO_LEAF_FILTERS_UNITS);
        let plan = crate::engine::plan::plan_from_semantic(&q);
        // Verify we get a GroupBy (not a crash / wrong variant).
        match &plan {
            crate::engine::plan::QueryPlan::GroupBy { .. } => {}
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

    #[test]
    fn excel_trace_replay_project3_execute_shapes_render_cellsets() {
        with_project3(|| {
            for mdx in EXCEL_TRACE_PROJECT3_EXECUTES {
                let xml = get_execute_statement_response(mdx);
                assert!(
                    xml.contains("urn:schemas-microsoft-com:xml-analysis:mddataset"),
                    "query failed: {mdx}"
                );
                assert!(xml.contains("<Axes>"), "missing axes for: {mdx}");
            }
        });
    }

    #[test]
    fn excel_trace_total_revenue_matches_raw_sql() {
        with_project3(|| {
            let xml = get_execute_statement_response(EXCEL_TRACE_TOTAL_REVENUE);
            let expected =
                Backend::test_fixture().query_scalar("SELECT SUM(revenue) FROM sales_fact");
            assert_eq!(cell_values(&xml), vec![expected]);
        });
    }

    // Excel sends each PivotTable query once per CELL PROPERTIES variant. The
    // short-lived result cache (plan 032) must collapse the repeats into one
    // DuckDB execution without changing the rendered cellset.
    #[test]
    fn repeated_mdx_variants_are_served_from_the_result_cache() {
        with_project3(|| {
            let base = "SELECT {[Measures].[Revenue]} ON COLUMNS, \
                        {[Category].[Category].Members} ON ROWS FROM [Sales]";
            let backend = Backend::test_fixture();
            let user = crate::engine::model::UserContext::admin_default();
            let config = crate::proxy_project::project().config.clone();
            let run = |mdx: &str| {
                crate::execute::runtime::get_execute_cellset_response_with_backend_and_context(
                    mdx, backend, &user, &config,
                )
            };

            let (first_xml, first) = run(&format!("{base} CELL PROPERTIES VALUE"));
            assert!(
                !first.cache_hit,
                "the first variant executes against DuckDB"
            );
            assert!(first.sql_execute_us > 0, "first variant runs a query");
            assert!(first_xml.contains("<Value"), "cellset renders values");

            let (second_xml, second) = run(&format!(
                "{base} CELL PROPERTIES CELL_ORDINAL, FORMAT_STRING"
            ));
            assert!(
                second.cache_hit,
                "the repeat variant is served from the cache"
            );
            assert!(
                second.sql_execute_us < first.sql_execute_us,
                "a cache hit must not re-execute ({}us vs {}us)",
                second.sql_execute_us,
                first.sql_execute_us
            );
            assert!(
                second_xml.contains("<Value") && second_xml.contains("<Axes>"),
                "the cached result still renders a full cellset"
            );

            let (_, other) =
                run("SELECT {[Measures].[Units]} ON COLUMNS FROM [Sales] CELL PROPERTIES VALUE");
            assert!(!other.cache_hit, "a different query must not hit the entry");
        });
    }

    // Regression: DRILLTHROUGH must read the backend it is given — a
    // file-backed project's rows, not a demo fallback. Contoso's `sales`
    // table has 7,794 rows; the response is capped at the 1,000-row LIMIT.
    #[test]
    fn drillthrough_reads_the_backend_it_is_given() {
        let project = ProxyProject::load("projects/generated_contoso/proxy-config.json")
            .expect("load generated_contoso");
        with_test_project(project, || {
            let conn = duckdb::Connection::open("projects/generated_contoso/data/sales.db")
                .expect("open contoso db");
            let backend = FileQueryBackend(std::sync::Mutex::new(conn));

            let xml = crate::execute::dispatch::get_execute_drillthrough_response(
                "DRILLTHROUGH",
                &backend,
            );
            assert_eq!(
                xml.matches("<row>").count(),
                1000,
                "rows must come from the file-backed backend (LIMIT 1000): {xml}"
            );
        });
    }

    // Plan 033: DRILLTHROUGH filters are exact. A flat-dimension member used
    // to match by prefix, so "North" also returned Northeast/Northwest rows.
    #[test]
    fn drillthrough_flat_dimension_member_is_exact() {
        with_project3(|| {
            let xml = crate::execute::dispatch::get_execute_drillthrough_response(
                "DRILLTHROUGH SELECT FROM [Sales] WHERE ([Territory].[Territory].&[North])",
                Backend::test_fixture(),
            );
            let territories: std::collections::BTreeSet<&str> = xml
                .split("<territory>")
                .skip(1)
                .filter_map(|s| s.split("</territory>").next())
                .collect();
            assert!(!territories.is_empty(), "North has rows: {xml}");
            assert_eq!(
                territories,
                std::collections::BTreeSet::from(["North"]),
                "only North rows may be returned"
            );
        });
    }

    // Coarse date members scope through the dim table, so Year 2024 returns
    // exactly the 2024 rows.
    #[test]
    fn drillthrough_coarse_date_member_scopes_exactly() {
        with_project3(|| {
            let xml = crate::execute::dispatch::get_execute_drillthrough_response(
                "DRILLTHROUGH SELECT FROM [Sales] WHERE ([Date].[Calendar].[Year].&[2024])",
                Backend::test_fixture(),
            );
            let keys: Vec<&str> = xml
                .split("<date_key>")
                .skip(1)
                .filter_map(|s| s.split("</date_key>").next())
                .collect();
            assert!(!keys.is_empty(), "2024 has rows: {xml}");
            assert!(
                keys.iter().all(|k| k.starts_with("2024")),
                "every row must be in 2024: {keys:?}"
            );
        });
    }

    // Compound members align their key parts to the level chain: quarter
    // &[2024]&[1] must not pick up other years' Q1s.
    #[test]
    fn drillthrough_compound_quarter_member_scopes_exactly() {
        with_project3(|| {
            let xml = crate::execute::dispatch::get_execute_drillthrough_response(
                "DRILLTHROUGH SELECT FROM [Sales] WHERE ([Date].[Calendar].[Quarter].&[2024]&[1])",
                Backend::test_fixture(),
            );
            let keys: Vec<&str> = xml
                .split("<date_key>")
                .skip(1)
                .filter_map(|s| s.split("</date_key>").next())
                .collect();
            assert!(!keys.is_empty(), "2024 Q1 has rows: {xml}");
            assert!(
                keys.iter().all(|k| k.starts_with("202401")
                    || k.starts_with("202402")
                    || k.starts_with("202403")),
                "every row must be in 2024 Q1: {keys:?}"
            );
        });
    }

    #[test]
    fn excel_trace_territory_drilldown_matches_raw_sql() {
        with_project3(|| {
            let xml = get_execute_statement_response(EXCEL_TRACE_TERRITORY_DRILLDOWN_REVENUE);
            let (expected_captions, expected_values) = query_grouped(
                "SELECT territory, SUM(revenue) FROM sales_fact GROUP BY territory ORDER BY territory",
            );
            assert_eq!(axis_captions(&xml, "Axis0"), expected_captions);
            assert_eq!(cell_values(&xml), expected_values);
        });
    }

    // Regression: the [All] parent member prepended to a multi-level drilldown
    // axis must get its own grand-total cell. Without it the cellset has one
    // fewer value than members and Excel shifts every year's value down one row
    // (Grand Total shows 2020, 2020 shows 2021, … 2030 shows empty).
    #[test]
    fn date_hierarchy_drilldown_has_aligned_all_total_cell() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}) ON COLUMNS FROM [Sales]",
            );
            let captions = axis_captions(&xml, "Axis0");
            let values = cell_values(&xml);
            let years = data_year_keys();
            assert_eq!(
                captions.len(),
                1 + years.len(),
                "expected All + years with facts on Axis0, got {captions:?}"
            );
            assert_eq!(
                values.len(),
                captions.len(),
                "must have one cell per axis member (off-by-one guard)"
            );
            let total = demo_scalar("SELECT COALESCE(SUM(revenue),0) FROM sales_fact");
            assert_eq!(values[0], total, "[All] grand total");
            let first_year: i32 = years[0].parse().expect("year key");
            assert_eq!(
                values[1],
                demo_year_revenue(first_year),
                "first year must not be shifted"
            );
            let sum_years: f64 = values[1..].iter().sum();
            assert!(
                (values[0] - sum_years).abs() < 1.0,
                "[All] {} should equal sum of years {sum_years}",
                values[0]
            );
        });
    }

    #[test]
    fn excel_trace_territory_subquery_filter_matches_raw_sql() {
        with_project3(|| {
            let xml =
                get_execute_statement_response(EXCEL_TRACE_TERRITORY_FILTER_NORTHWEST_REVENUE);
            let (expected_captions, expected_values) = query_grouped(
                "SELECT territory, SUM(revenue) FROM sales_fact WHERE territory = 'Northwest' GROUP BY territory ORDER BY territory",
            );
            assert_eq!(axis_captions(&xml, "Axis0"), expected_captions);
            assert_eq!(cell_values(&xml), expected_values);
        });
    }

    // Regression: name-based member references (no `&` key qualifier) in the
    // WHERE clause must filter, not fall through to the unfiltered grand total.
    #[test]
    fn report_filter_name_form_filters_correctly() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales] WHERE ([Category].[Category].[Electronics])",
            );
            assert_eq!(
                cell_values(&xml),
                vec![24_719_896.0],
                "name-form report filter must return the Electronics subset"
            );
        });
    }

    #[test]
    fn excel_trace_segment_all_matches_unfiltered_revenue() {
        with_project3(|| {
            let all_xml = get_execute_statement_response(EXCEL_TRACE_SEGMENT_ALL_REVENUE);
            let plain_xml = get_execute_statement_response(EXCEL_TRACE_TERRITORY_DRILLDOWN_REVENUE);
            assert_eq!(
                axis_captions(&all_xml, "Axis0"),
                axis_captions(&plain_xml, "Axis0")
            );
            assert_eq!(cell_values(&all_xml), cell_values(&plain_xml));
        });
    }

    #[test]
    fn excel_trace_segment_consumer_matches_raw_sql() {
        with_project3(|| {
            let xml = get_execute_statement_response(EXCEL_TRACE_SEGMENT_CONSUMER_REVENUE);
            let (expected_captions, expected_values) = query_grouped(
                "SELECT territory, SUM(revenue) FROM sales_fact WHERE segment = 'Consumer' GROUP BY territory ORDER BY territory",
            );
            assert_eq!(axis_captions(&xml, "Axis0"), expected_captions);
            assert_eq!(cell_values(&xml), expected_values);
        });
    }

    #[test]
    fn excel_trace_nested_territory_and_segment_filter_matches_raw_sql() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                EXCEL_TRACE_TERRITORY_FILTER_SOUTH_SEGMENT_CONSUMER_REVENUE,
            );
            let (expected_captions, expected_values) = query_grouped(
                "SELECT territory, SUM(revenue) FROM sales_fact WHERE territory = 'South' AND segment = 'Consumer' GROUP BY territory ORDER BY territory",
            );
            assert_eq!(axis_captions(&xml, "Axis0"), expected_captions);
            assert_eq!(cell_values(&xml), expected_values);
        });
    }

    #[test]
    fn excel_trace_channel_all_filter_is_noop_under_consumer_filter() {
        with_project3(|| {
            let all_xml =
                get_execute_statement_response(EXCEL_TRACE_SEGMENT_CONSUMER_CHANNEL_ALL_REVENUE);
            let plain_xml = get_execute_statement_response(EXCEL_TRACE_SEGMENT_CONSUMER_REVENUE);
            assert_eq!(
                axis_captions(&all_xml, "Axis0"),
                axis_captions(&plain_xml, "Axis0")
            );
            assert_eq!(cell_values(&all_xml), cell_values(&plain_xml));
        });
    }

    #[test]
    fn excel_trace_two_leaf_filters_match_raw_revenue_sql() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                EXCEL_TRACE_SEGMENT_CONSUMER_CHANNEL_WHOLESALE_REVENUE,
            );
            let (expected_captions, expected_values) = query_grouped(
                "SELECT territory, SUM(revenue) FROM sales_fact WHERE segment = 'Consumer' AND channel = 'Wholesale' GROUP BY territory ORDER BY territory",
            );
            assert_eq!(axis_captions(&xml, "Axis0"), expected_captions);
            assert_eq!(cell_values(&xml), expected_values);
        });
    }

    #[test]
    fn excel_trace_omitted_measure_matches_explicit_revenue() {
        with_project3(|| {
            let implicit_xml = get_execute_statement_response(
                EXCEL_TRACE_SEGMENT_CONSUMER_CHANNEL_WHOLESALE_DEFAULT_MEASURE,
            );
            let explicit_xml = get_execute_statement_response(
                EXCEL_TRACE_SEGMENT_CONSUMER_CHANNEL_WHOLESALE_REVENUE,
            );
            assert_eq!(
                axis_captions(&implicit_xml, "Axis0"),
                axis_captions(&explicit_xml, "Axis0")
            );
            assert_eq!(cell_values(&implicit_xml), cell_values(&explicit_xml));
            assert_eq!(
                cell_format_strings(&implicit_xml),
                cell_format_strings(&explicit_xml)
            );
        });
    }

    #[test]
    fn excel_trace_units_uses_units_values_and_format_string() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                EXCEL_TRACE_SEGMENT_CONSUMER_CHANNEL_WHOLESALE_UNITS,
            );
            let (expected_captions, expected_values) = query_grouped(
                "SELECT territory, SUM(units) FROM sales_fact WHERE segment = 'Consumer' AND channel = 'Wholesale' GROUP BY territory ORDER BY territory",
            );
            assert_eq!(axis_captions(&xml, "Axis0"), expected_captions);
            assert_eq!(cell_values(&xml), expected_values);
            assert!(cell_format_strings(&xml).iter().all(|fmt| fmt == "#,##0"));
            assert!(
                xml.contains("[Measures].[Units]"),
                "Units should be reflected on slicer axis"
            );
        });
    }

    #[test]
    fn excel_trace_crossjoin_revenue_matches_raw_sql() {
        with_project3(|| {
            let xml = get_execute_statement_response(EXCEL_TRACE_TERRITORY_CATEGORY_REVENUE);
            let (expected_tuples, expected_values) = query_pairs(
                "SELECT territory, category, SUM(revenue) FROM sales_fact WHERE segment = 'Consumer' AND channel = 'Wholesale' GROUP BY territory, category ORDER BY territory, category",
            );
            assert_eq!(axis_tuple_captions(&xml, "Axis0"), expected_tuples);
            assert_eq!(cell_values(&xml), expected_values);
        });
    }

    #[test]
    fn excel_trace_crossjoin_reorder_matches_raw_sql_and_preserves_pair_values() {
        with_project3(|| {
            let forward_xml =
                get_execute_statement_response(EXCEL_TRACE_TERRITORY_CATEGORY_REVENUE);
            let reverse_xml =
                get_execute_statement_response(EXCEL_TRACE_CATEGORY_TERRITORY_REVENUE);

            let (expected_forward_tuples, expected_forward_values) = query_pairs(
                "SELECT territory, category, SUM(revenue) FROM sales_fact WHERE segment = 'Consumer' AND channel = 'Wholesale' GROUP BY territory, category ORDER BY territory, category",
            );
            let (expected_reverse_tuples, expected_reverse_values) = query_pairs(
                "SELECT category, territory, SUM(revenue) FROM sales_fact WHERE segment = 'Consumer' AND channel = 'Wholesale' GROUP BY category, territory ORDER BY category, territory",
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
            let implicit_xml =
                get_execute_statement_response(EXCEL_TRACE_TERRITORY_CATEGORY_DEFAULT_MEASURE);
            let explicit_xml =
                get_execute_statement_response(EXCEL_TRACE_TERRITORY_CATEGORY_REVENUE);
            assert_eq!(
                axis_tuple_captions(&implicit_xml, "Axis0"),
                axis_tuple_captions(&explicit_xml, "Axis0")
            );
            assert_eq!(cell_values(&implicit_xml), cell_values(&explicit_xml));
            assert_eq!(
                cell_format_strings(&implicit_xml),
                cell_format_strings(&explicit_xml)
            );
        });
    }

    #[test]
    fn excel_trace_crossjoin_collapse_rolls_up_northwest_total() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                EXCEL_TRACE_TERRITORY_CATEGORY_COLLAPSE_NORTHWEST_REVENUE,
            );
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
                "SELECT territory, category, SUM(units) FROM sales_fact WHERE segment = 'Consumer' AND channel = 'Wholesale' GROUP BY territory, category ORDER BY territory, category",
            );
            assert_eq!(axis_tuple_captions(&xml, "Axis0"), expected_tuples);
            assert_eq!(cell_values(&xml), expected_values);
            assert!(cell_format_strings(&xml).iter().all(|fmt| fmt == "#,##0"));
            assert!(
                xml.contains("[Measures].[Units]"),
                "Units should be reflected on slicer axis"
            );
        });
    }

    #[test]
    fn excel_trace_crossjoin_consumer_units_matches_raw_sql() {
        with_project3(|| {
            let xml = get_execute_statement_response(EXCEL_TRACE_TERRITORY_CATEGORY_CONSUMER_UNITS);
            let (expected_tuples, expected_values) = query_pairs(
                "SELECT territory, category, SUM(units) FROM sales_fact WHERE segment = 'Consumer' GROUP BY territory, category ORDER BY territory, category",
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
                "SELECT territory, category, SUM(units) FROM sales_fact GROUP BY territory, category ORDER BY territory, category",
            );
            assert_eq!(axis_tuple_captions(&xml, "Axis0"), expected_tuples);
            assert_eq!(cell_values(&xml), expected_values);
            assert!(cell_format_strings(&xml).iter().all(|fmt| fmt == "#,##0"));
        });
    }

    #[test]
    fn excel_trace_filtered_cchildren_probes_render_cellsets() {
        with_project3(|| {
            for mdx in [
                EXCEL_TRACE_CHANNEL_WHOLESALE_CCHILDREN,
                EXCEL_TRACE_SEGMENT_CONSUMER_CCHILDREN,
            ] {
                let xml = get_execute_statement_response(mdx);
                assert!(
                    xml.contains("urn:schemas-microsoft-com:xml-analysis:mddataset"),
                    "query failed: {mdx}"
                );
                assert!(xml.contains("<CellData>"), "missing cell data for: {mdx}");
            }
        });
    }

    #[test]
    fn column_only_measure_uses_correct_measure() {
        with_project3(|| {
            let mdx = "SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales] CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";
            let xml = get_execute_statement_response(mdx);
            let expected =
                Backend::test_fixture().query_scalar("SELECT SUM(revenue) FROM sales_fact");
            assert_eq!(cell_values(&xml), vec![expected]);
            assert!(
                xml.contains("[Measures].[Revenue]"),
                "slicer axis should show Revenue"
            );
        });
    }

    #[test]
    fn parser_axis_dimension_ids_match_semantic_parse_axis_dimensions() {
        with_project3(|| {
            for mdx in EXCEL_TRACE_PROJECT3_EXECUTES {
                // Skip member/children probes — they don't have axis dimensions.
                if mdx.contains(".Members")
                    || mdx.contains(".Children")
                    || mdx.contains("AddCalculatedMembers")
                {
                    continue;
                }
                let parsed = crate::mdx_parser::parse_mdx(mdx);
                let from_parser: Vec<String> = parsed
                    .axis_dimension_ids
                    .iter()
                    .filter(|id| {
                        crate::proxy_project::project()
                            .model
                            .dim_def_opt(id)
                            .is_some()
                    })
                    .cloned()
                    .collect();
                let from_semantic =
                    crate::mdx_semantic::semantic_query_from_mdx(mdx).axis_dimensions;
                assert_eq!(
                    from_parser, from_semantic,
                    "axis dimension mismatch for: {mdx}"
                );
            }
        });
    }

    // Plan 046: unsupported set expressions fault with a named reason instead
    // of returning a dropped axis or a wrong-hierarchy cellset.
    // Plan 046 slice 3: a bare `{a : b}` set probe lists the members between
    // the two keys (not just the endpoints, not the whole hierarchy).
    #[test]
    fn member_range_set_probe_returns_the_members_between() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT {[Date].[Calendar].[Year].&[2022] : [Date].[Calendar].[Year].&[2024]} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            );
            for year in ["2022", "2023", "2024"] {
                assert!(
                    xml.contains(&format!(">{year}<")),
                    "range includes {year}: {xml}"
                );
            }
            for year in ["2020", "2021", "2025"] {
                assert!(
                    !xml.contains(&format!(">{year}<")),
                    "range excludes {year}: {xml}"
                );
            }
        });
    }

    // The CUBESETCOUNT probe over a windowed set (`COUNT(Filter(…))`).
    #[test]
    fn count_probe_over_a_windowed_set() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "WITH MEMBER [Measures].[XL_SD] AS 'COUNT(Filter([Date].[Calendar].[Full Date].Members, [Date].[Calendar].CurrentMember.Member_Value >= DateAdd(\"d\", -30, VBA![Date]())))' SELECT {[Measures].[XL_SD]} ON 0 FROM [Sales] CELL PROPERTIES VALUE",
            );
            assert!(!xml.contains("<faultstring>"), "{xml}");
            let values = cell_values(&xml);
            assert_eq!(values.len(), 1);
            // The fixture's `full_date` is an integer date key (the live demo
            // uses a DATE), so assert the window *restricts* here; the live
            // check verifies the exact ~30-day span.
            let total = Backend::test_fixture()
                .query_count("SELECT COUNT(DISTINCT CONCAT_WS('|', year, quarter, month, full_date)) FROM date_dim");
            assert!(
                values[0] > 0.0 && values[0] < f64::from(total),
                "the window must restrict the count: {} of {total}",
                values[0]
            );
        });
    }

    // Slicer ranges restrict the aggregate (the same range filter as axes).
    #[test]
    fn slicer_member_range_restricts_the_aggregate() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT {[Measures].[Revenue]} ON 0 FROM [Sales] WHERE ({[Date].[Calendar].[Year].&[2022] : [Date].[Calendar].[Year].&[2024]}) CELL PROPERTIES VALUE",
            );
            assert!(!xml.contains("<faultstring>"), "{xml}");
            let expected = Backend::test_fixture().query_scalar(
                "SELECT SUM(f.revenue) FROM sales_fact f JOIN date_dim d ON f.date_key = d.date_key WHERE d.year BETWEEN 2022 AND 2024",
            );
            assert_eq!(cell_values(&xml), vec![expected]);
        });
    }

    // Plan 046 slice 4: the documented Excel named-set sliding window
    // (`Filter(…, Member_Value >= DateAdd("d", -30, VBA![Date]()))`).
    #[test]
    fn member_value_windows_lower_to_date_filters() {
        with_project3(|| {
            // Inline filter — the captured CUBESET probe shape.
            let xml = get_execute_statement_response(
                "SELECT {HEAD(Filter([Date].[Calendar].[Full Date].Members, [Date].[Calendar].CurrentMember.Member_Value >= DateAdd(\"d\", -30, VBA![Date]())), 1)} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            );
            assert!(!xml.contains("<faultstring>"), "{xml}");
            let caps: Vec<String> = xml
                .split("<Caption>")
                .skip(1)
                .filter_map(|s| s.split("</Caption>").next().map(str::to_string))
                .collect();
            assert!(
                !caps.is_empty() && caps[0] != "All" && caps[0] != "Revenue",
                "one window member: {caps:?}"
            );

            // Named set (`WITH SET`) referenced on the axis.
            let xml = get_execute_statement_response(
                "WITH SET [Last30] AS 'Filter([Date].[Calendar].[Full Date].Members, [Date].[Calendar].CurrentMember.Member_Value >= DateAdd(\"d\", -30, VBA![Date]()))' SELECT {[Last30]} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            );
            assert!(!xml.contains("<faultstring>"), "{xml}");
            let members = xml.matches("<Member ").count();
            assert!(
                members > 1 && members < 40,
                "the window spans about 30 days: {members} members"
            );

            // An upper-bound window (`Member_Value <= DateAdd("yyyy", -1, …)`).
            let xml = get_execute_statement_response(
                "SELECT {HEAD(Filter([Date].[Calendar].[Full Date].Members, [Date].[Calendar].CurrentMember.Member_Value <= DateAdd(\"yyyy\", -1, VBA![Date]())), 1)} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            );
            assert!(!xml.contains("<faultstring>"), "{xml}");
        });
    }

    // Plan 046 slice 5: period-to-date functions lower to date windows on the
    // anchor's date role.
    #[test]
    fn period_to_date_functions_return_the_window_members() {
        with_project3(|| {
            // YTD of a year member is the year itself.
            let xml = get_execute_statement_response(
                "SELECT {YTD([Date].[Calendar].[Year].&[2024])} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            );
            assert!(!xml.contains("<faultstring>"), "{xml}");
            assert!(xml.contains("<Caption>2024</Caption>"), "{xml}");
            assert!(
                !xml.contains("<Caption>2023</Caption>"),
                "the window starts at the year: {xml}"
            );

            // YTD of a month member is that year's months up to it.
            let xml = get_execute_statement_response(
                "SELECT {YTD([Date].[Calendar].[Month].&[2024]&[2]&[6])} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            );
            assert!(!xml.contains("<faultstring>"), "{xml}");
            for month in ["1", "2", "3", "4", "5", "6"] {
                assert!(
                    xml.contains(&format!("<Caption>{month}</Caption>")),
                    "YTD includes month {month}: {xml}"
                );
            }
            for month in ["7", "8"] {
                assert!(
                    !xml.contains(&format!("<Caption>{month}</Caption>")),
                    "YTD excludes month {month}: {xml}"
                );
            }

            // QTD of a month member is that quarter's months up to it.
            let xml = get_execute_statement_response(
                "SELECT {QTD([Date].[Calendar].[Month].&[2024]&[2]&[6])} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            );
            assert!(!xml.contains("<faultstring>"), "{xml}");
            for month in ["4", "5", "6"] {
                assert!(
                    xml.contains(&format!("<Caption>{month}</Caption>")),
                    "QTD includes {month}: {xml}"
                );
            }
            assert!(
                !xml.contains("<Caption>3</Caption>"),
                "QTD excludes the previous quarter: {xml}"
            );

            // MTD of a month member is the month itself.
            let xml = get_execute_statement_response(
                "SELECT {MTD([Date].[Calendar].[Month].&[2024]&[2]&[6])} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            );
            assert!(!xml.contains("<faultstring>"), "{xml}");
            assert!(xml.contains("<Caption>6</Caption>"), "{xml}");
            assert!(
                !xml.contains("<Caption>5</Caption>"),
                "MTD starts at the month: {xml}"
            );

            // ParallelPeriod(Year, -1, 2024) → the previous year.
            let xml = get_execute_statement_response(
                "SELECT {ParallelPeriod([Date].[Calendar].[Year], -1, [Date].[Calendar].[Year].&[2024])} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            );
            assert!(!xml.contains("<faultstring>"), "{xml}");
            assert!(xml.contains("<Caption>2023</Caption>"), "{xml}");
            assert!(!xml.contains("<Caption>2024</Caption>"), "{xml}");

            // LastPeriods(3, 2024) → 2022, 2023, 2024.
            let xml = get_execute_statement_response(
                "SELECT {LastPeriods(3, [Date].[Calendar].[Year].&[2024])} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            );
            assert!(!xml.contains("<faultstring>"), "{xml}");
            for year in ["2022", "2023", "2024"] {
                assert!(
                    xml.contains(&format!("<Caption>{year}</Caption>")),
                    "LastPeriods includes {year}: {xml}"
                );
            }
            assert!(
                !xml.contains("<Caption>2021</Caption>"),
                "LastPeriods stops after three: {xml}"
            );

            // PeriodsToDate(Year, month) matches YTD.
            let xml = get_execute_statement_response(
                "SELECT {PeriodsToDate([Date].[Calendar].[Year], [Date].[Calendar].[Month].&[2024]&[2]&[6])} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            );
            assert!(!xml.contains("<faultstring>"), "{xml}");
            assert!(
                xml.contains("<Caption>1</Caption>") && xml.contains("<Caption>6</Caption>"),
                "{xml}"
            );
        });
    }

    // Plan 047 increment 4: structural `has_cols`/`has_rows` (from the AST, not
    // a spacing-sensitive text scan) fixed the classification of comma-separated
    // axes, so a range on a pivot axis beside a measure set now returns the
    // members between the keys.
    #[test]
    fn pivot_axis_member_range_returns_the_members_between() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT {[Measures].[Revenue]} ON 0, {[Date].[Calendar].[Year].&[2022] : [Date].[Calendar].[Year].&[2024]} ON 1 FROM [Sales]",
            );
            assert!(!xml.contains("<faultstring>"), "{xml}");
            for year in ["2022", "2023", "2024"] {
                assert!(
                    xml.contains(&format!(">{year}<")),
                    "range includes {year}: {xml}"
                );
            }
            for year in ["2020", "2021", "2025"] {
                assert!(
                    !xml.contains(&format!(">{year}<")),
                    "range excludes {year}: {xml}"
                );
            }
        });
    }

    #[test]
    fn head_of_a_member_range_takes_the_first_member() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT {HEAD({[Date].[Calendar].[Year].&[2022] : [Date].[Calendar].[Year].&[2024]},1)} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            );
            assert!(xml.contains(">2022<"), "HEAD takes the first member: {xml}");
            assert!(!xml.contains(">2023<"), "HEAD prunes the rest: {xml}");
        });
    }

    #[test]
    fn unsupported_set_expressions_fault_loudly() {
        with_project3(|| {
            for (mdx, needle) in [
                (
                    "SELECT {ClosingPeriod([Date].[Calendar].[Year], [Date].[Calendar].[Month].&[2024]&[2]&[6])} ON 1 FROM [Sales]",
                    "ClosingPeriod()",
                ),
                (
                    "WITH MEMBER [Measures].[XL_SD] AS 'COUNT({[Date].[Calendar].[Year].&[2022] : [Date].[Calendar].[Year].&[2024]})' SELECT {[Measures].[XL_SD]} ON 0 FROM [Sales]",
                    "member ranges",
                ),
                (
                    "WITH SET [Last30] AS 'Filter([Date].[Calendar].[Full Date].Members, [Date].[Calendar].CurrentMember.Member_Value >= 1)' SELECT {[Measures].[Revenue]} ON 0 FROM [Sales]",
                    "member-property filters",
                ),
            ] {
                let xml = crate::execute_builders::get_execute_cellset_response(mdx);
                assert!(xml.contains("<faultstring>"), "{mdx} → {xml}");
                assert!(xml.contains(needle), "{mdx} → {xml}");
            }

            // Supported shapes are untouched.
            let xml = crate::execute_builders::get_execute_cellset_response(
                "SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales]",
            );
            assert!(!xml.contains("faultstring"), "{xml}");
        });
    }

    #[test]
    fn refresh_cube_succeeds_as_noop() {
        with_project3(|| {
            // Excel's pivot Refresh sends `REFRESH CUBE [<cube>]`; a fault here
            // aborts the UI refresh with "The query did not run" (plan 048).
            for mdx in ["REFRESH CUBE [Sales]", "  refresh  cube [Sales]"] {
                let xml = crate::execute_builders::get_execute_cellset_response(mdx);
                assert!(!xml.contains("faultstring"), "{mdx} → {xml}");
                assert!(xml.contains("xml-analysis:empty"), "{mdx} → {xml}");
            }
        });
    }

    #[test]
    fn calculated_member_ddl_faults_with_an_actionable_message() {
        with_project3(|| {
            // Excel's OLAP Tools can send CREATE/DROP DDL. The proxy is a
            // read-only adapter, so the fault must name the reason instead of
            // leaking a parser error (plan 048).
            for (mdx, kind) in [
                (
                    "CREATE MEMBER [Sales].[Measures].[Test Calc] AS 1",
                    "calculated members",
                ),
                (
                    "DROP MEMBER [Sales].[Measures].[Test Calc]",
                    "calculated members",
                ),
                (
                    "CREATE SET [Sales].[Test Set] AS {[Category].[Category].&[Books]}",
                    "named sets",
                ),
                ("CREATE SESSION SET [Sales].[Test Set] AS {}", "named sets"),
                ("DROP SET [Sales].[Test Set]", "named sets"),
                ("ALTER CUBE [Sales] COMPILE", "cube alterations"),
            ] {
                let xml = crate::execute_builders::get_execute_cellset_response(mdx);
                assert!(xml.contains("faultstring"), "{mdx} → {xml}");
                assert!(xml.contains("read-only adapter"), "{mdx} → {xml}");
                assert!(xml.contains(kind), "{mdx} → {xml}");
                assert!(!xml.contains("expected `SELECT`"), "{mdx} → {xml}");
            }
        });
    }

    // Excel reads member elements positionally: an unrequested
    // CHILDREN_CARDINALITY element (or its declaration) shifts the properties
    // it reads, corrupting its hierarchy walk — this crashed Excel on
    // "Expand to Month" and "Expand to Full Date". The reference emits it only
    // when asked (plan 048).
    // Excel's Label Filters, captured live from "Filter → Label Filters →
    // begins with B": a subquery predicate over the member caption, with
    // Left/Right/InStr for begins-with / ends-with / contains (plan 048).
    #[test]
    fn excel_label_filters_lower_to_caption_predicates() {
        with_project3(|| {
            let captions = |condition: &str| {
                let mdx = format!(
                    "SELECT NON EMPTY Hierarchize({{DrilldownLevel({{[Category].[Category].[All]}},,,INCLUDE_CALC_MEMBERS)}}) \
                     ON COLUMNS FROM (SELECT Filter([Category].[Category].[Category].AllMembers, ({condition})) \
                     ON COLUMNS FROM [Sales]) WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE"
                );
                let xml = get_execute_statement_response(&mdx);
                assert!(!xml.contains("faultstring"), "{condition} → {xml}");
                axis0_member_infos(&xml)
                    .into_iter()
                    .map(|(caption, _, _, _)| caption)
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                captions(r#"Left([Category].[Category].CurrentMember.member_caption,1)="B""#),
                // The reference keeps (All) first on a filtered axis.
                vec!["All", "Baby", "Beauty", "Books"]
            );
            // Excel sends a leading start position for contains.
            let contains =
                captions(r#"InStr(1,[Category].[Category].CurrentMember.member_caption,"oo")>0"#);
            assert!(
                contains.len() > 1
                    && contains[0] == "All"
                    && contains[1..].iter().all(|c| c.contains("oo")),
                "{contains:?}"
            );
            let ends =
                captions(r#"Right([Category].[Category].CurrentMember.member_caption,1)="s""#);
            assert!(
                ends.len() > 1 && ends[0] == "All" && ends[1..].iter().all(|c| c.ends_with('s')),
                "{ends:?}"
            );
        });
    }

    // Excel's Date Filters arrive as a subquery predicate:
    // `Filter(<hierarchy>.Levels(n).AllMembers, CurrentMember.MemberValue <op>
    // CDate("YYYY-MM-DD"))`. It lowers to an absolute window on the date role's
    // full-date column — the exact MDX Excel sent when a Date Filter was
    // applied against the proxy (plan 048).
    #[test]
    fn excel_date_filters_lower_to_absolute_date_windows() {
        with_project3(|| {
            let set = "[Date].[Full Date].Levels(1).AllMembers";
            let members = |condition: &str| {
                let mdx = format!(
                    "SELECT NON EMPTY Hierarchize({{DrilldownLevel({{[Date].[Full Date].[All]}},,,INCLUDE_CALC_MEMBERS)}}) \
                     ON COLUMNS FROM (SELECT Filter({set}, ([Date].[Full Date].CurrentMember.MemberValue{condition})) \
                     ON COLUMNS FROM [Sales]) WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE"
                );
                let xml = get_execute_statement_response(&mdx);
                assert!(!xml.contains("faultstring"), "{condition} → {xml}");
                axis0_member_infos(&xml).len()
            };
            // (All) plus exactly the matching date.
            assert_eq!(members("=CDate(\"2020-01-02\")"), 2);
            // A strict comparison keeps a subset (two days before 2020-01-03).
            assert_eq!(members("<CDate(\"2020-01-03\")"), 3);
        });
    }

    #[test]
    fn children_cardinality_is_only_emitted_when_requested() {
        with_project3(|| {
            let two_level = "SELECT NON EMPTY Hierarchize(DrilldownMember({{DrilldownMember({{DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}}, {[Date].[Calendar].[Year].&[2024]},,,INCLUDE_CALC_MEMBERS)}}, {[Date].[Calendar].[Year].&[2024],[Date].[Calendar].[Quarter].&[2024]&[1]},,,INCLUDE_CALC_MEMBERS)) DIMENSION PROPERTIES PARENT_UNIQUE_NAME ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE";
            let xml = get_execute_statement_response(two_level);
            assert!(
                !xml.contains("CHILDREN_CARDINALITY"),
                "must not emit or declare it unrequested: {xml}"
            );
            // The requested properties still follow the standard five, in the
            // reference's order (check a level-1 member; (All) has no parent).
            let member = xml
                .split("<Member Hierarchy=\"[Date].[Calendar]\">")
                .find(|m| m.contains("<LNum>1</LNum>"))
                .expect("a level-1 calendar member");
            let elements = [
                "UName",
                "Caption",
                "LName",
                "LNum",
                "DisplayInfo",
                "PARENT_UNIQUE_NAME",
            ];
            let mut cursor = 0;
            for el in elements {
                let at = member[cursor..]
                    .find(&format!("<{el}>"))
                    .unwrap_or_else(|| panic!("{el} missing or out of order in {member}"));
                cursor += at;
            }

            let requested = "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,[Date].[Calendar].[Month]CHILDREN_CARDINALITY ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE";
            let xml = get_execute_statement_response(requested);
            assert!(xml.contains("<CHILDREN_CARDINALITY>"), "{xml}");
            assert!(
                xml.contains("[CHILDREN_CARDINALITY]"),
                "declared when requested: {xml}"
            );
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
            assert_eq!(
                semantic.measure.as_deref(),
                Some("Revenue YTD"),
                "should resolve measure from MDX WHERE clause"
            );
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
                    let ti_filters: Vec<_> =
                        filters.iter().filter(|f| f.time_flag.is_some()).collect();
                    assert_eq!(
                        ti_filters.len(),
                        1,
                        "should have exactly one time_flag filter"
                    );
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
            let backend = Backend::test_fixture();
            for (mdx, label) in [
                (
                    "SELECT  FROM [Sales] WHERE ([Measures].[Revenue YTD]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR",
                    "YTD",
                ),
                (
                    "SELECT  FROM [Sales] WHERE ([Measures].[Revenue Prior Year]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR",
                    "PriorYTD",
                ),
                (
                    "SELECT  FROM [Sales] WHERE ([Measures].[Revenue QTD]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR",
                    "QTD",
                ),
                (
                    "SELECT  FROM [Sales] WHERE ([Measures].[Revenue MTD]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR",
                    "MTD",
                ),
            ] {
                let semantic = crate::mdx_semantic::semantic_query_from_mdx(mdx);
                let plan = plan_from_semantic_with_model(&semantic, &project.model);
                let result =
                    crate::engine::plan::execute_plan_with_backend(&plan, &project.model, backend);
                match result {
                    crate::engine::plan::QueryResult::Scalar(v) => {
                        assert!(
                            v > 0.0,
                            "{label} revenue should be non-zero against demo data, got {v}"
                        );
                    }
                    other => panic!("{label} expected Scalar result, got {other:?}"),
                }
            }
        });
    }

    // Regression: time-flag measures selected on an AXIS (the MDX shape Excel
    // actually emits for a YTD/QTD/MTD measure) must resolve to the time-flag
    // measure and apply the date-dimension flag filter. Previously the measure
    // was looked up by caption only, so `[Measures].[RevenueYTD]` (id) failed
    // to match caption "Revenue YTD" and fell back to the plain Revenue measure.
    #[test]
    fn time_intelligence_measure_on_axis_applies_flag_filter() {
        with_project3(|| {
            use crate::backend::Backend;
            use crate::engine::plan::{
                QueryResult, execute_plan_with_backend, plan_from_semantic_with_model,
            };
            use crate::engine::sql::sql_for_query_plan;
            let project = crate::proxy_project::project();
            let backend = Backend::test_fixture();

            let total_plan = plan_from_semantic_with_model(
                &crate::mdx_semantic::semantic_query_from_mdx(
                    "SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales]",
                ),
                &project.model,
            );
            let total = match execute_plan_with_backend(&total_plan, &project.model, backend) {
                QueryResult::Scalar(v) => v,
                other => panic!("expected scalar total, got {other:?}"),
            };

            for (mdx, label, flag) in [
                (
                    "SELECT {[Measures].[RevenueYTD]} ON COLUMNS FROM [Sales]",
                    "YTD",
                    "ytd_flag",
                ),
                (
                    "SELECT {[Measures].[RevenueQTD]} ON COLUMNS FROM [Sales]",
                    "QTD",
                    "qtd_flag",
                ),
                (
                    "SELECT {[Measures].[RevenueMTD]} ON COLUMNS FROM [Sales]",
                    "MTD",
                    "mtd_flag",
                ),
                (
                    "SELECT {[Measures].[RevenuePriorYearYTD]} ON COLUMNS FROM [Sales]",
                    "PriorYear",
                    "prior_year_ytd_flag",
                ),
            ] {
                let semantic = crate::mdx_semantic::semantic_query_from_mdx(mdx);
                let plan = plan_from_semantic_with_model(&semantic, &project.model);
                let sql = sql_for_query_plan(&project.model, &plan);
                assert!(
                    sql.contains(&format!("WHERE {flag} = true")),
                    "{label} SQL should filter by {flag}, got: {sql}"
                );
                match &plan {
                    crate::engine::plan::QueryPlan::Total { measure, .. } => {
                        assert!(
                            project.model.meas_def(measure).time_flag.is_some(),
                            "{label} should resolve to a time-flag measure, got {measure}"
                        );
                    }
                    other => panic!("{label} expected Total plan, got {other:?}"),
                }
                match execute_plan_with_backend(&plan, &project.model, backend) {
                    QueryResult::Scalar(v) => {
                        assert!(v > 0.0, "{label} should be non-zero, got {v}");
                        assert!(
                            v < total,
                            "{label} should be a strict subset of total {total}, got {v}"
                        );
                    }
                    other => panic!("{label} expected Scalar, got {other:?}"),
                }
            }
        });
    }

    // Regression: a time-flag measure grouped by the date dimension on the rows
    // axis (e.g. Year) must also apply the flag filter, so every year except the
    // current one is zero.
    #[test]
    fn time_intelligence_measure_grouped_by_year_applies_flag_filter() {
        with_project3(|| {
            use crate::backend::Backend;
            use crate::engine::plan::{
                QueryResult, execute_plan_with_backend, plan_from_semantic_with_model,
            };
            use crate::engine::sql::sql_for_query_plan;
            let project = crate::proxy_project::project();
            let backend = Backend::test_fixture();

            let mdx = "SELECT [Date].[Calendar].[Year].Members ON ROWS, {[Measures].[RevenueYTD]} ON COLUMNS FROM [Sales]";
            let semantic = crate::mdx_semantic::semantic_query_from_mdx(mdx);
            let plan = plan_from_semantic_with_model(&semantic, &project.model);
            let sql = sql_for_query_plan(&project.model, &plan);
            assert!(
                sql.contains("WHERE ytd_flag = true"),
                "grouped YTD SQL should filter by ytd_flag, got: {sql}"
            );
            match execute_plan_with_backend(&plan, &project.model, backend) {
                QueryResult::Grouped(rows) => {
                    let nonzero = rows.iter().filter(|(_, v)| *v > 0.0).count();
                    assert!(
                        nonzero <= 1,
                        "only the current year should be non-zero, got {nonzero} non-zero rows: {rows:?}"
                    );
                }
                other => panic!("expected Grouped, got {other:?}"),
            }
        });
    }

    // Regression: batched CUBEVALUE cells produce a multi-measure query
    // (`SELECT {([Measures].[A]),([Measures].[B])} ON 0`). The proxy must
    // return one cell per measure instead of a single cell for the last measure.
    #[test]
    fn multi_measure_axis_returns_one_cell_per_measure() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT {([Measures].[Revenue]),([Measures].[Units]),([Measures].[Revenue QTD])} ON 0 FROM [Sales]",
            );
            let values = cell_values(&xml);
            // Revenue and Units are stable demo totals; the QTD measure drifts
            // with the current date, so compute its expectation from the same
            // seeded flag the engine filters on.
            let qtd = crate::backend::Backend::test_fixture()
                .query_scalar("SELECT SUM(f.revenue) FROM sales_fact f JOIN date_dim d ON f.date_key = d.date_key WHERE d.qtd_flag = true");
            assert_eq!(
                values,
                vec![521_586_767.0, 4_931_640.0, qtd],
                "one cell per measure, in order"
            );
        });
    }

    // ---- Oracle corpus (plan 049, phase 1) --------------------------------
    //
    // Axis structures captured from the reference SSAS 2025 tabular mirror
    // (`MallardDemo`, 2026-09-22) for the layouts Excel actually sends. The
    // tuples are the mirror's captions joined per tuple; the cell lists are
    // checked against the mirror's values. This is the safety net for the
    // parser/render refactor: it pins the *reference's* shapes, not ours.

    /// Axis tuples as `cap/cap,cap/cap,…` (the mirror's caption strings).
    fn axis_signature(xml: &str, axis: &str) -> String {
        axis_tuple_captions(xml, axis)
            .iter()
            .map(|tuple| tuple.join("/"))
            .collect::<Vec<_>>()
            .join(",")
    }

    const NESTED_ROWS_TUPLES: &str = "All/All,Automotive/All,Automotive/Retail,Baby/All,Baby/Retail,Beauty/All,Beauty/Direct,Books/All,Books/Direct,Clothing/All,Clothing/Direct,Electronics/All,Electronics/Wholesale,Food/All,Food/Online,Furniture/All,Furniture/Retail,Garden/All,Garden/Online,Health/All,Health/Wholesale,Home/All,Home/Online,Jewelry/All,Jewelry/Direct,Music/All,Music/Direct,Office/All,Office/Retail,Outdoors/All,Outdoors/Retail,Pet Supplies/All,Pet Supplies/Wholesale,Shoes/All,Shoes/Online,Sports/All,Sports/Wholesale,Tools/All,Tools/Wholesale,Toys/All,Toys/Online";

    // Excel's Value Filters arrive as a subselect too, with the condition
    // wrapped in parentheses:
    // `FROM (SELECT Filter(<set>, ([Measures].[X] > 26000000)) ON COLUMNS …)`.
    // Verified against the mirror: same members, same order, same values.
    #[test]
    fn oracle_excel_value_filter_subselect_matches_the_reference() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME ON COLUMNS FROM (SELECT Filter(Hierarchize([Category].[Category].[Category].AllMembers), ([Measures].[Revenue]>26000000)) ON COLUMNS FROM [Sales]) WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE, FORMAT_STRING",
            );
            assert!(!xml.contains("faultstring"), "{xml}");
            let captions: Vec<String> = axis0_member_infos(&xml)
                .into_iter()
                .map(|(caption, _, _, _)| caption)
                .collect();
            assert_eq!(captions[0], "All", "the reference keeps (All) first");
            assert!(
                captions.len() > 1 && captions.len() < 21,
                "only the members above the threshold survive: {captions:?}"
            );
            let values = cell_values(&xml);
            assert_eq!(values.len(), captions.len());
            assert!(
                values[1..].iter().all(|v| *v > 26_000_000.0),
                "every surviving member is above the threshold: {values:?}"
            );
            let subset: f64 = values[1..].iter().sum();
            assert_eq!(values[0], subset, "(All) aggregates the returned subset");
        });
    }

    // Excel's current Top/Bottom N dialog wraps the set in a subselect
    // (`FROM (SELECT Generate(<set> AS [XL_Filter_Set_0],
    //  TopCount(Filter(Except(DrilldownLevel(<set>.Current …), …),
    //            Not IsEmpty(<measure>)), n, <measure>)) ON COLUMNS …)`).
    // The reference answers with the surviving members in the outer axis's own
    // order and an `(All)` carrying the subset total — verified against the
    // mirror, member for member (plan 049).
    #[test]
    fn oracle_excel_top_n_subselect_matches_the_reference() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME ON COLUMNS FROM (SELECT Generate(Hierarchize({[Category].[Category].[All]}) AS [XL_Filter_Set_0], TopCount(Filter(Except(DrilldownLevel([XL_Filter_Set_0].Current AS [XL_Filter_HelperSet_0], , 0,INCLUDE_CALC_MEMBERS), [XL_Filter_HelperSet_0]), Not IsEmpty([Measures].[Revenue])), 5, [Measures].[Revenue])) ON COLUMNS FROM [Sales]) WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE, FORMAT_STRING",
            );
            assert!(!xml.contains("faultstring"), "{xml}");
            let captions: Vec<String> = axis0_member_infos(&xml)
                .into_iter()
                .map(|(caption, _, _, _)| caption)
                .collect();
            assert_eq!(captions[0], "All", "the reference keeps (All) first");
            assert_eq!(captions.len(), 6, "All + the top 5 categories");
            let members = &captions[1..];
            assert!(
                members.windows(2).all(|w| w[0] <= w[1]),
                "the reference returns them in the axis's own (hierarchy) order: {members:?}"
            );
            let values = cell_values(&xml);
            assert_eq!(values.len(), 6);
            let subset: f64 = values[1..].iter().sum();
            assert_eq!(values[0], subset, "(All) aggregates the returned subset");
        });
    }

    #[test]
    fn oracle_nested_rows_returns_parents_and_channels() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY {[Measures].[Revenue]} ON COLUMNS, NON EMPTY Hierarchize(DrilldownMember(CrossJoin({[Category].[Category].[All],[Category].[Category].[Category].AllMembers}, {([Channel].[Channel].[All])}), [Category].[Category].[Category].AllMembers, [Channel].[Channel])) ON ROWS FROM [Sales]",
            );
            assert_eq!(axis_signature(&xml, "Axis0"), "Revenue");
            assert_eq!(axis_signature(&xml, "Axis1"), NESTED_ROWS_TUPLES);
            let values = cell_values(&xml);
            assert_eq!(values.len(), 41, "mirror: 41 cells");
            let total = demo_scalar("SELECT SUM(revenue) FROM sales_fact");
            assert_eq!(values[0], total, "root = grand total");
            let automotive =
                demo_scalar("SELECT SUM(revenue) FROM sales_fact WHERE category = 'Automotive'");
            assert_eq!(values[1], automotive, "Automotive total");
            assert_eq!(values[2], automotive, "Automotive's only channel");
        });
    }

    #[test]
    fn oracle_nested_rows_collapse_keeps_the_parent_aggregate() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY Hierarchize(DrilldownMember(CrossJoin({[Category].[Category].[All],[Category].[Category].[Category].AllMembers}, {([Channel].[Channel].[All])}), {-{[Category].[Category].&[Baby]}}, [Channel].[Channel])) ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue])",
            );
            let expected = NESTED_ROWS_TUPLES.replace("Baby/All,Baby/Retail", "Baby/All");
            assert_eq!(axis_signature(&xml, "Axis0"), expected);
            let values = cell_values(&xml);
            assert_eq!(values.len(), 40, "mirror: 40 cells (Baby collapsed)");
            let total = demo_scalar("SELECT SUM(revenue) FROM sales_fact");
            assert_eq!(values[0], total);
        });
    }

    const CATEGORY_2MEAS_TUPLES: &str = "All/Revenue,All/Units,Automotive/Revenue,Automotive/Units,Baby/Revenue,Baby/Units,Beauty/Revenue,Beauty/Units,Books/Revenue,Books/Units,Clothing/Revenue,Clothing/Units,Electronics/Revenue,Electronics/Units,Food/Revenue,Food/Units,Furniture/Revenue,Furniture/Units,Garden/Revenue,Garden/Units,Health/Revenue,Health/Units,Home/Revenue,Home/Units,Jewelry/Revenue,Jewelry/Units,Music/Revenue,Music/Units,Office/Revenue,Office/Units,Outdoors/Revenue,Outdoors/Units,Pet Supplies/Revenue,Pet Supplies/Units,Shoes/Revenue,Shoes/Units,Sports/Revenue,Sports/Units,Tools/Revenue,Tools/Units,Toys/Revenue,Toys/Units";

    #[test]
    fn oracle_category_with_two_measures_keeps_them_on_one_axis() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)}), {[Measures].[Revenue],[Measures].[Units]}) ON COLUMNS FROM [Sales]",
            );
            assert_eq!(axis_signature(&xml, "Axis0"), CATEGORY_2MEAS_TUPLES);
            let values = cell_values(&xml);
            assert_eq!(values.len(), 42, "21 rows × 2 measures");
            let revenue = demo_scalar("SELECT SUM(revenue) FROM sales_fact");
            let units = demo_scalar("SELECT SUM(units) FROM sales_fact");
            assert_eq!(
                &values[0..2],
                &[revenue, units],
                "All row: Revenue then Units"
            );
        });
    }

    #[test]
    fn oracle_channel_by_category_cross_tab_is_sparse_like_the_reference() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Channel].[Channel].[All]},,,INCLUDE_CALC_MEMBERS)}), {[Measures].[Revenue]}) ON COLUMNS, NON EMPTY Hierarchize({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)}) ON ROWS FROM [Sales]",
            );
            assert_eq!(
                axis_signature(&xml, "Axis0"),
                "All/Revenue,Direct/Revenue,Online/Revenue,Retail/Revenue,Wholesale/Revenue"
            );
            assert_eq!(
                axis_signature(&xml, "Axis1"),
                "All,Automotive,Baby,Beauty,Books,Clothing,Electronics,Food,Furniture,Garden,Health,Home,Jewelry,Music,Office,Outdoors,Pet Supplies,Shoes,Sports,Tools,Toys"
            );
            // The reference sends only the combinations with data (45 of 105).
            let values = cell_values(&xml);
            assert_eq!(values.len(), 45, "mirror: 45 cells, sparse");
            let total = demo_scalar("SELECT SUM(revenue) FROM sales_fact");
            assert_eq!(values[0], total, "All × All");
        });
    }

    /// Excel's value filter on a nested pivot arrives as a subselect —
    /// `FROM (SELECT Filter(<categories>, ([Measures].[Revenue] >= n)) ON COLUMNS ...)`
    /// (captured from the VM sweep, 2026-09-23). The reference keeps the
    /// categories whose total passes, with all of their channels; the
    /// multi-dimension plan path used to drop the op entirely and return every
    /// category.
    #[test]
    fn oracle_subselect_value_filter_keeps_only_qualifying_members() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY Hierarchize(DrilldownMember(CrossJoin({[Category].[Category].[All],[Category].[Category].[Category].AllMembers}, {([Channel].[Channel].[All])}), [Category].[Category].[Category].AllMembers, [Channel].[Channel])) DIMENSION PROPERTIES PARENT_UNIQUE_NAME ON COLUMNS FROM (SELECT Filter(Hierarchize([Category].[Category].[Category].AllMembers), ([Measures].[Revenue]>=26000000)) ON COLUMNS FROM [Sales]) CELL PROPERTIES VALUE, FORMAT_STRING",
            );
            let captions = axis_tuple_captions(&xml, "Axis0");
            let shown: std::collections::BTreeSet<String> = captions
                .iter()
                .filter_map(|tuple| tuple.first().cloned())
                .filter(|name| name != "All")
                .collect();
            // The fixture's own SQL decides who passes: the filter is per
            // category over all channels, exactly what the subselect asks.
            let expected = demo_count(
                "SELECT COUNT(*) FROM (SELECT category FROM sales_fact \
                 GROUP BY category HAVING SUM(revenue) >= 26000000)",
            ) as usize;
            assert!(
                expected > 0 && expected < 20,
                "the filter is selective: {expected}"
            );
            assert_eq!(shown.len(), expected, "the surviving categories: {shown:?}");
            assert!(
                shown.contains("Clothing"),
                "a qualifying category is present: {shown:?}"
            );
            assert!(
                !shown.contains("Automotive"),
                "Automotive (25.1M) must be filtered out: {shown:?}"
            );
            // The survivor keeps its own total on the (All) channel row.
            let values = cell_values(&xml);
            let clothing =
                demo_scalar("SELECT SUM(revenue) FROM sales_fact WHERE category = 'Clothing'");
            assert!(
                values.contains(&clothing),
                "the surviving category keeps its total"
            );
        });
    }

    /// Mirror-measured (2026-09-23): measures cross-joined on the Rows edge stay
    /// there — 21 `(Category, Revenue)` tuples with the measure second, the
    /// columns edge unchanged at 5 Channel tuples, and the grid of cells.
    #[test]
    fn oracle_measures_on_rows_match_the_reference() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)}), {[Measures].[Revenue]}) ON ROWS, \
                 NON EMPTY Hierarchize({DrilldownLevel({[Channel].[Channel].[All]},,,INCLUDE_CALC_MEMBERS)}) ON COLUMNS \
                 FROM [Sales] CELL PROPERTIES VALUE, FORMAT_STRING",
            );
            let rows = axis_tuple_captions(&xml, "Axis1");
            assert_eq!(rows.len(), 21, "All + categories");
            assert_eq!(
                rows[0],
                vec!["All", "Revenue"],
                "measure second, as written"
            );
            assert_eq!(rows[1], vec!["Automotive", "Revenue"]);
            let cols = axis_tuple_captions(&xml, "Axis0");
            assert_eq!(cols.len(), 5, "All + channels");
            assert_eq!(cols[0], vec!["All"]);

            let values = cell_values(&xml);
            let revenue = demo_scalar("SELECT SUM(revenue) FROM sales_fact");
            assert_eq!(values[0], revenue, "All x All");
            let direct =
                demo_scalar("SELECT SUM(revenue) FROM sales_fact WHERE channel = 'Direct'");
            assert_eq!(values[1], direct, "All category x first channel");
            let automotive =
                demo_scalar("SELECT SUM(revenue) FROM sales_fact WHERE category = 'Automotive'");
            assert_eq!(values[5], automotive, "first category x All channels");
        });
    }

    /// Mirror-measured (2026-09-23): two fields on **both** edges, measures
    /// cross-joined on one — 105 `(Category, Channel)` tuples on Rows, 8
    /// `(Date, Revenue)` on Columns, and the grid of cells.
    #[test]
    fn oracle_two_fields_on_both_edges_match_the_reference() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)}), Hierarchize({DrilldownLevel({[Channel].[Channel].[All]},,,INCLUDE_CALC_MEMBERS)})) ON ROWS, \
                 NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}), {[Measures].[Revenue]}) ON COLUMNS \
                 FROM [Sales] CELL PROPERTIES VALUE, FORMAT_STRING",
            );
            let rows = axis_tuple_captions(&xml, "Axis1");
            assert_eq!(rows.len(), 21 * 5, "category x channel");
            assert_eq!(rows[0], vec!["All", "All"]);
            assert_eq!(rows[1], vec!["All", "Direct"], "inner member fastest");
            let cols = axis_tuple_captions(&xml, "Axis0");
            assert_eq!(cols.len(), data_year_keys().len() + 1, "All + years");
            assert_eq!(cols[0], vec!["All", "Revenue"]);

            let values = cell_values(&xml);
            let dense: usize = demo_scalar(
                "SELECT COUNT(*) FROM (SELECT DISTINCT d.year, f.category, f.channel \
                 FROM sales_fact f JOIN date_dim d ON f.date_key = d.date_key)",
            ) as usize;
            assert!(dense > 0, "the fixture has combinations");
            assert!(
                values.len() >= dense,
                "{} cells must cover the {dense} existing combinations",
                values.len()
            );
            let revenue = demo_scalar("SELECT SUM(revenue) FROM sales_fact");
            assert_eq!(values[0], revenue, "All x All");
        });
    }

    /// The reference rejects a measure that appears on an axis *and* in the
    /// slicer (mirror-measured 2026-09-23: "The Measures hierarchy already
    /// appears in the Axis1 axis."); the same statement without the slicer
    /// measure renders. We used to render the invalid form silently.
    #[test]
    fn measure_on_axis_and_slicer_faults_like_the_reference() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)}), {[Measures].[Revenue]}) ON ROWS, \
                 NON EMPTY Hierarchize({DrilldownLevel({[Channel].[Channel].[All]},,,INCLUDE_CALC_MEMBERS)}) ON COLUMNS \
                 FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE, FORMAT_STRING",
            );
            assert!(
                xml.contains("The Measures hierarchy already appears in the Axis1 axis."),
                "mirror message: {xml}"
            );
        });
    }

    /// Clause order is not part of the query: `ON ROWS, ... ON COLUMNS` and
    /// `... ON COLUMNS, ... ON ROWS` describe the same cellset and must produce
    /// the same response. This was the untested axis of variation behind the
    /// cross-wired axes (plan 051): the plan's key columns followed the
    /// statement's order while the renderer assumed axis order.
    #[test]
    fn clause_order_does_not_change_the_cellset() {
        with_project3(|| {
            let body = |xml: &str| xml.split("<soap:Body>").nth(1).unwrap_or("").to_string();
            let cases = [
                // Two-field cross-tab.
                (
                    "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)}) ON ROWS, \
                     NON EMPTY Hierarchize({DrilldownLevel({[Channel].[Channel].[All]},,,INCLUDE_CALC_MEMBERS)}) ON COLUMNS \
                     FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE, FORMAT_STRING",
                    "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Channel].[Channel].[All]},,,INCLUDE_CALC_MEMBERS)}) ON COLUMNS, \
                     NON EMPTY Hierarchize({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)}) ON ROWS \
                     FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE, FORMAT_STRING",
                ),
                // Cross-joined pair on Columns beside a Rows edge.
                (
                    "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}) ON ROWS, \
                     NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)}), \
                     Hierarchize({DrilldownLevel({[Channel].[Channel].[All]},,,INCLUDE_CALC_MEMBERS)})) ON COLUMNS \
                     FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE, FORMAT_STRING",
                    "SELECT NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)}), \
                     Hierarchize({DrilldownLevel({[Channel].[Channel].[All]},,,INCLUDE_CALC_MEMBERS)})) ON COLUMNS, \
                     NON EMPTY Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}) ON ROWS \
                     FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE, FORMAT_STRING",
                ),
            ];
            for (rows_first, columns_first) in cases {
                let a = get_execute_statement_response(rows_first);
                let b = get_execute_statement_response(columns_first);
                assert_eq!(
                    body(&a),
                    body(&b),
                    "clause order changed the cellset\nROWS-first: {rows_first}\nCOLUMNS-first: {columns_first}"
                );
                assert!(a.contains("<Cell "), "the statement produced cells: {a}");
            }
        });
    }

    /// Mirror-measured (2026-09-23): a cross-joined pair on COLUMNS beside a
    /// ROWS edge keeps **both** dimensions on that axis — the reference returns
    /// the product with `(All)` first on each side and the inner member varying
    /// fastest (45 tuples = 9 x 5 on its smaller model; 105 = 21 x 5 here), the
    /// rows edge unchanged, and the full grid of cells.
    ///
    /// Writing `ON ROWS` before `ON COLUMNS` used to cross-wire the edges: the
    /// plan's key columns follow the statement's order, while the renderer
    /// derived them from the specs' order, so each axis carried the other's
    /// member keys and most cells were missing — silently, with no fault.
    #[test]
    fn oracle_crossjoined_pair_on_columns_matches_the_reference() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}) ON ROWS, \
                 NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)}), \
                 Hierarchize({DrilldownLevel({[Channel].[Channel].[All]},,,INCLUDE_CALC_MEMBERS)})) ON COLUMNS \
                 FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE, FORMAT_STRING",
            );

            let years = data_year_keys();
            let columns = axis_tuple_captions(&xml, "Axis0");
            assert_eq!(
                columns.len(),
                21 * 5,
                "category x channel tuples: {columns:?}"
            );
            assert_eq!(columns[0], vec!["All", "All"], "outer then inner member");
            assert_eq!(
                columns[1],
                vec!["All", "Direct"],
                "inner member varies fastest"
            );
            assert_eq!(
                columns[4],
                vec!["All", "Wholesale"],
                "the inner set wraps before the outer advances"
            );
            assert_eq!(
                columns[5],
                vec!["Automotive", "All"],
                "the outer member advances after its inner set"
            );

            let rows = axis_tuple_captions(&xml, "Axis1");
            assert_eq!(rows.len(), years.len() + 1, "All + years");
            assert_eq!(rows[0], vec!["All"]);
            assert_eq!(rows[1], vec![years[0].clone()]);

            let values = cell_values(&xml);
            // Every combination that exists in the data has a cell, plus the
            // (All) combinations; the grid is sparse where the data is (the
            // reference omits empty cells rather than sending zeros). The old
            // cross-wiring produced 8 cells here — one per row, not a grid.
            let dense: usize = demo_scalar(
                "SELECT COUNT(*) FROM (SELECT DISTINCT d.year, f.category, f.channel \
                 FROM sales_fact f JOIN date_dim d ON f.date_key = d.date_key)",
            ) as usize;
            assert!(dense > 0, "the fixture has combinations");
            assert!(
                values.len() >= dense,
                "{} cells must cover the {dense} existing combinations",
                values.len()
            );
            // Row-major with the columns edge varying fastest.
            let revenue = demo_scalar("SELECT SUM(revenue) FROM sales_fact");
            assert_eq!(values[0], revenue, "All x All");
            let direct =
                demo_scalar("SELECT SUM(revenue) FROM sales_fact WHERE channel = 'Direct'");
            assert_eq!(values[1], direct, "All x first channel");
            let wholesale =
                demo_scalar("SELECT SUM(revenue) FROM sales_fact WHERE channel = 'Wholesale'");
            assert_eq!(values[4], wholesale, "All x last channel");
            let automotive =
                demo_scalar("SELECT SUM(revenue) FROM sales_fact WHERE category = 'Automotive'");
            assert_eq!(
                values[5], automotive,
                "the outer member advances after its inner set"
            );
        });
    }

    // Plan 049, phase 3: a field in Columns with two nested in Rows groups by
    // three dimensions. The reference returns the nested Rows structure (41
    // tuples) crossed with the Columns field × the measures.
    #[test]
    fn oracle_three_dimension_layout_matches_the_reference() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}), {[Measures].[Revenue],[Measures].[Units]}) ON COLUMNS, NON EMPTY Hierarchize(DrilldownMember(CrossJoin({[Category].[Category].[All],[Category].[Category].[Category].AllMembers}, {([Channel].[Channel].[All])}), [Category].[Category].[Category].AllMembers, [Channel].[Channel])) ON ROWS FROM [Sales]",
            );
            let mut expected_cols = vec!["All/Revenue".to_string(), "All/Units".to_string()];
            for year in data_year_keys() {
                expected_cols.push(format!("{year}/Revenue"));
                expected_cols.push(format!("{year}/Units"));
            }
            assert_eq!(axis_signature(&xml, "Axis0"), expected_cols.join(","));
            assert_eq!(axis_signature(&xml, "Axis1"), NESTED_ROWS_TUPLES);
            let values = cell_values(&xml);
            assert_eq!(
                values.len(),
                expected_cols.len() * 41,
                "columns × nested rows"
            );
            // Axis 0 varies fastest: (All/Revenue, All/All) first.
            let revenue = demo_scalar("SELECT SUM(revenue) FROM sales_fact");
            let units = demo_scalar("SELECT SUM(units) FROM sales_fact");
            assert_eq!(values[0], revenue, "All revenue");
            assert_eq!(values[1], units, "All units");
            let first_year = data_year_keys().first().cloned().expect("a year");
            let year_revenue = demo_scalar(&format!(
                "SELECT SUM(f.revenue) FROM sales_fact f JOIN date_dim d ON f.date_key = d.date_key WHERE d.year = {first_year}"
            ));
            assert_eq!(values[2], year_revenue, "first year revenue");
        });
    }

    #[test]
    fn oracle_year_by_category_cross_tab_keeps_the_all_column() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}), {[Measures].[Revenue]}) ON COLUMNS, NON EMPTY Hierarchize({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)}) ON ROWS FROM [Sales]",
            );
            let mut expected = vec!["All/Revenue".to_string()];
            expected.extend(data_year_keys().iter().map(|y| format!("{y}/Revenue")));
            assert_eq!(axis_signature(&xml, "Axis0"), expected.join(","));
            assert_eq!(
                axis_tuple_captions(&xml, "Axis1").len(),
                21,
                "All + categories"
            );
            let values = cell_values(&xml);
            assert_eq!(values.len(), (data_year_keys().len() + 1) * 21);
            let total = demo_scalar("SELECT SUM(revenue) FROM sales_fact");
            assert_eq!(values[0], total, "All × All");
            let first_year = data_year_keys().first().cloned().expect("a year");
            let year_total = demo_scalar(&format!(
                "SELECT SUM(f.revenue) FROM sales_fact f JOIN date_dim d ON f.date_key = d.date_key WHERE d.year = {first_year}"
            ));
            assert_eq!(values[1], year_total, "first year × All");
        });
    }

    // Plan 049: Excel cross-joins the Values area with whatever field sits on
    // the same edge. The response must keep both on that axis — splitting the
    // measures onto their own axis broke every layout with a field in Columns
    // or measures in Rows.
    #[test]
    fn crossjoined_measures_stay_on_the_requested_axis() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}), {[Measures].[Revenue],[Measures].[Units]}) ON COLUMNS FROM [Sales]",
            );
            let tuples = axis_tuple_captions(&xml, "Axis0");
            assert_eq!(
                tuples.len(),
                (data_year_keys().len() + 1) * 2,
                "years × measures"
            );
            assert_eq!(tuples[0], vec!["All", "Revenue"]);
            assert_eq!(tuples[1], vec!["All", "Units"]);
            assert_eq!(tuples[2], vec!["2020", "Revenue"]);
            assert_eq!(cell_values(&xml).len(), tuples.len());
            assert!(
                !xml.contains(r#"<AxisInfo name="Axis1">"#),
                "no separate measures axis: {xml}"
            );
        });
    }

    // Plan 049: a field in Columns with another in Rows is a cross-tab — one
    // cellset axis per requested edge, each with its (All) member first, and
    // cells ordered row-major with the first axis varying fastest.
    #[test]
    fn two_axis_cross_tab_keeps_one_axis_per_edge() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}), {[Measures].[Revenue]}) ON COLUMNS, NON EMPTY Hierarchize({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)}) ON ROWS FROM [Sales]",
            );
            let cols = axis_tuple_captions(&xml, "Axis0");
            let rows = axis_tuple_captions(&xml, "Axis1");
            assert_eq!(cols.len(), data_year_keys().len() + 1, "All + years");
            assert_eq!(rows.len(), 21, "All + 20 categories");
            assert_eq!(cols[0], vec!["All", "Revenue"]);
            assert_eq!(rows[0], vec!["All"]);
            assert_eq!(rows[1], vec!["Automotive"]);
            let values = cell_values(&xml);
            assert_eq!(values.len(), cols.len() * rows.len());
            assert_eq!(values[0], 521_586_767.0, "All × All = grand total");
            let first_year = data_year_keys().first().cloned().expect("a year");
            let year_total = demo_scalar(&format!(
                "SELECT SUM(f.revenue) FROM sales_fact f JOIN date_dim d ON f.date_key = d.date_key WHERE d.year = {first_year}"
            ));
            assert_eq!(values[1], year_total, "first year × All");
        });
    }

    // Plan 049: `DrilldownMember(CrossJoin(...))` — two fields in Rows —
    // returns the whole input set: the root tuple, then for every parent its
    // own `(parent, All)` aggregate followed by its children with data.
    #[test]
    fn nested_rows_drilldown_returns_parents_and_totals() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY {[Measures].[Revenue]} ON COLUMNS, NON EMPTY Hierarchize(DrilldownMember(CrossJoin({[Category].[Category].[All],[Category].[Category].[Category].AllMembers}, {([Channel].[Channel].[All])}), [Category].[Category].[Category].AllMembers, [Channel].[Channel])) ON ROWS FROM [Sales]",
            );
            assert_eq!(axis_tuple_captions(&xml, "Axis0").len(), 1, "measures axis");
            let rows = axis_tuple_captions(&xml, "Axis1");
            assert_eq!(rows[0], vec!["All", "All"]);
            assert_eq!(rows[1], vec!["Automotive", "All"]);
            assert_eq!(rows[2], vec!["Automotive", "Retail"]);
            assert_eq!(rows[3], vec!["Baby", "All"]);
            let values = cell_values(&xml);
            assert_eq!(values[0], 521_586_767.0, "root = grand total");
            assert_eq!(values[1], 25_102_648.0, "Automotive total");
            assert_eq!(values[2], 25_102_648.0, "Automotive's only channel");
        });
    }

    // Plan 049: the measures axis sits at the ordinal the statement asked for.
    #[test]
    fn measures_axis_keeps_its_ordinal() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT NON EMPTY {[Measures].[Revenue]} ON COLUMNS, NON EMPTY Hierarchize(DrilldownMember(CrossJoin({[Category].[Category].[All],[Category].[Category].[Category].AllMembers}, {([Channel].[Channel].[All])}), [Category].[Category].[Category].AllMembers, [Channel].[Channel])) ON ROWS FROM [Sales]",
            );
            assert_eq!(axis_tuple_captions(&xml, "Axis0"), vec![vec!["Revenue"]]);
            assert!(
                axis_tuple_captions(&xml, "Axis1").len() > 1,
                "rows on Axis1"
            );
        });
    }

    // Regression: a PivotTable with multiple measures in Values and a dimension
    // on Rows cross-joins them; the proxy must return N measures × M rows cells,
    // ordered row-major (columns = measures, rows = dimension members).
    #[test]
    fn multi_measure_crossjoin_returns_row_major_cells() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT [Category].[Category].Members ON ROWS, {[Measures].[Revenue],[Measures].[Units]} ON COLUMNS FROM [Sales]",
            );
            let values = cell_values(&xml);
            // `<hierarchy>.Members` includes the (All) member, as the reference
            // returns it: 21 rows (All + 20 categories) × 2 measures (plan 049).
            assert_eq!(values.len(), 42, "21 rows × 2 measures");
            assert_eq!(values[0], 521_586_767.0, "All revenue");
            assert_eq!(values[1], 4_931_640.0, "All units");
            assert_eq!(
                values[2], 25_102_648.0,
                "first category = Automotive revenue"
            );
            assert!(
                values[3] < 1_000_000.0,
                "Automotive units (small): {}",
                values[3]
            );
            let revenue: f64 = values.iter().skip(2).step_by(2).sum();
            let units: f64 = values.iter().skip(3).step_by(2).sum();
            assert!(
                (revenue - 521_586_767.0).abs() < 1.0,
                "category revenue sums to the total: {revenue}"
            );
            assert!((units - 4_931_640.0).abs() < 1.0, "category units: {units}");
        });
    }

    // Regression: level-qualified slicers like [Date].[Calendar].[Year].&[2024]
    // must filter on the hierarchy level's column (year), not the leaf column.
    #[test]
    fn level_qualified_slicer_filters_by_level_column() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales] WHERE ([Date].[Calendar].[Year].&[2024])",
            );
            assert_eq!(
                cell_values(&xml),
                vec![demo_year_revenue(2024)],
                "2024 revenue, not the unfiltered total"
            );
        });
    }

    // Regression: a compound quarter member (&[2024]&[4]) must scope its months
    // to that year (not all years' Q4s), carry the year-encoded keys, and the
    // correct per-month child counts.
    #[test]
    fn compound_quarter_drilldown_scopes_to_year() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                r##"SELECT NON EMPTY Hierarchize({DrilldownLevel({DrilldownLevel({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)},[Date].[Calendar].[Year],INCLUDE_CALC_MEMBERS)},[Date].[Calendar].[Quarter],INCLUDE_CALC_MEMBERS)}) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME ON COLUMNS FROM (SELECT ({[Date].[Calendar].[Quarter].&[2024]&[4]}) ON COLUMNS FROM [Sales]) WHERE ([Measures].[Revenue])"##,
            );
            let q4 = demo_quarter_revenue(2024, 4);
            assert_eq!(
                cell_values(&xml),
                vec![
                    q4,
                    q4,
                    q4,
                    demo_month_revenue(2024, 10),
                    demo_month_revenue(2024, 11),
                    demo_month_revenue(2024, 12)
                ],
                "(All), Year 2024, and Q4 2024 totals, then October, November, December"
            );
            assert!(
                xml.contains(
                    "<UName>[Date].[Calendar].[Month].&amp;[2024]&amp;[4]&amp;[10]</UName>"
                ),
                "month must carry the compound year path"
            );
            // The full ancestor chain must be on the axis so Excel can resolve
            // each member's parent (a quarter without its year crashes).
            assert!(
                xml.contains("<UName>[Date].[Calendar].[Year].&amp;[2024]</UName>"),
                "the year ancestor must be on the axis"
            );
            assert!(
                xml.contains("<UName>[Date].[Calendar].[All]</UName>"),
                "the (All) ancestor must be on the axis"
            );
            assert!(
                xml.contains(
                    "<PARENT_UNIQUE_NAME>[Date].[Calendar].[Quarter].&amp;[2024]&amp;[4]</PARENT_UNIQUE_NAME>"
                ),
                "month parent must be the compound quarter"
            );
            assert!(
                xml.contains("<Caption>4</Caption>"),
                "quarter caption should be the bare value, not the path"
            );
        });
    }

    // Regression: axis set functions (TopCount/Order/Filter) must actually
    // transform the row set instead of being ignored.
    #[test]
    fn topcount_returns_top_n_sorted_by_measure() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT TopCount([Category].[Category].Members, 3, [Measures].[Revenue]) ON ROWS, {[Measures].[Revenue]} ON COLUMNS FROM [Sales]",
            );
            // The reference answers a set-op axis with (All) first, carrying the
            // returned subset's total, then the top 3 sorted by the measure.
            assert_eq!(
                cell_values(&xml),
                vec![83_447_530.0, 28_502_160.0, 27_702_858.0, 27_242_512.0],
                "All (subset total) + top 3 categories, descending"
            );
        });
    }

    #[test]
    fn order_by_measure_sorts_rows() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT Order([Category].[Category].Members, [Measures].[Revenue], DESC) ON ROWS, {[Measures].[Revenue]} ON COLUMNS FROM [Sales]",
            );
            let values = cell_values(&xml);
            // (All) first (the reference keeps it on a set-op axis), then the
            // categories sorted by revenue descending.
            assert_eq!(values.len(), 21, "All + 20 categories");
            assert_eq!(values[1], 28_502_160.0, "first is the max");
            assert_eq!(values[20], 24_440_800.0, "last is the min");
        });
    }

    #[test]
    fn filter_by_measure_keeps_matching_rows() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT Filter([Category].[Category].Members, [Measures].[Revenue] > 26000000) ON ROWS, {[Measures].[Revenue]} ON COLUMNS FROM [Sales]",
            );
            let values = cell_values(&xml);
            assert!(!values.is_empty(), "some categories exceed 26M");
            assert!(
                values.iter().all(|v| *v > 26_000_000.0),
                "all rows must exceed the filter threshold: {values:?}"
            );
        });
    }

    // Regression: two row fields × two measures (a 3-axis cellset). The proxy
    // must emit one cell per (pair, measure), not empty.
    #[test]
    fn multi_measure_two_dim_crossjoin_returns_cells() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT CrossJoin([Category].[Category].Members, [Territory].[Territory].Members) ON ROWS, {[Measures].[Revenue],[Measures].[Units]} ON COLUMNS FROM [Sales]",
            );
            let values = cell_values(&xml);
            // Demo data has 40 distinct (category, territory) pairs; 2 measures.
            assert_eq!(values.len(), 80, "40 pairs × 2 measures");
            let revenue: f64 = values.iter().step_by(2).sum();
            let units: f64 = values.iter().skip(1).step_by(2).sum();
            assert!(
                (revenue - 521_586_767.0).abs() < 1.0,
                "revenue column: {revenue}"
            );
            assert!((units - 4_931_640.0).abs() < 1.0, "units column: {units}");
        });
    }

    // Regression: batched CUBEVALUE cells with different level slicers produce a
    // multi-tuple query where each tuple has its own filter. Previously all
    // tuples shared the first tuple's filter.
    #[test]
    fn batched_multi_tuple_slicers_return_per_tuple_values() {
        with_project3(|| {
            let xml = get_execute_statement_response(
                "SELECT {([Measures].[Revenue],[Date].[Calendar].[Year].&[2024]),([Measures].[Revenue],[Date].[Calendar].[Month].&[6]),([Measures].[Revenue],[Date].[Calendar].[Quarter].&[2])} ON 0 FROM [Sales]",
            );
            let values = cell_values(&xml);
            assert_eq!(values.len(), 3, "one cell per tuple");
            assert_eq!(values[0], demo_year_revenue(2024), "Year 2024");
            assert_eq!(
                values[1],
                demo_month_value_revenue(6),
                "Month 6 totals every year"
            );
            assert_eq!(
                values[2],
                demo_quarter_value_revenue(2),
                "Quarter 2 totals every year"
            );
            assert!(
                values.iter().all(|v| *v > 0.0),
                "all tuples non-zero: {values:?}"
            );
            assert_ne!(values[0], values[1], "Month 6 must differ from Year 2024");
            assert_ne!(values[0], values[2], "Quarter 2 must differ from Year 2024");
        });
    }

    #[test]
    fn retail_analytics_discover_catalogs_returns_correct_name() {
        with_retail_analytics(|| {
            let xml = crate::xmla::discover::catalogs::get_catalogs_response();
            assert!(
                xml.contains("urn:schemas-microsoft-com:xml-analysis:rowset"),
                "missing rowset namespace"
            );
            assert!(xml.contains("SEMANTICMODEL"), "should contain catalog name");
            assert!(xml.contains("<row"), "should have at least one row");
        });
    }

    #[test]
    fn retail_analytics_discover_cubes_returns_correct_name() {
        with_retail_analytics(|| {
            let xml = crate::xmla::discover::cubes::get_cubes_response();
            assert!(
                xml.contains("urn:schemas-microsoft-com:xml-analysis:rowset"),
                "missing rowset namespace"
            );
            assert!(xml.contains("<row"), "should have at least one row");
        });
    }

    #[test]
    fn retail_analytics_discover_dimensions_has_date_role() {
        with_retail_analytics(|| {
            let xml = crate::xmla::discover::dimensions::get_dimensions_response();
            assert!(
                xml.contains("urn:schemas-microsoft-com:xml-analysis:rowset"),
                "missing rowset namespace"
            );
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
            assert!(
                xml.contains("urn:schemas-microsoft-com:xml-analysis:rowset"),
                "missing rowset namespace"
            );
            assert!(xml.contains("<row"), "should have at least one row");
        });
    }

    #[test]
    fn retail_analytics_total_revenue_is_fallback_returns_empty() {
        // Total Revenue is no longer a stub — Plan 021 generated real SQL.
        // The fallback returns a real value (0 on empty DB).
        with_retail_analytics(|| {
            let project = crate::proxy_project::project();
            let conn =
                duckdb::Connection::open("projects/generated_retail_analytics/data/sales.db")
                    .expect("open retail db");
            let backend = FileQueryBackend(std::sync::Mutex::new(conn));

            let mdx = "SELECT  FROM [SALES] WHERE ([Measures].[Total Revenue]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";
            let xml = crate::execute_builders::get_execute_cellset_response_with_backend(
                mdx,
                &backend,
                &project.model,
            );

            assert!(!xml.is_empty(), "should not panic on fallback measure");
            assert!(
                xml.contains("urn:schemas-microsoft-com:xml-analysis:mddataset"),
                "missing mddataset"
            );
            assert!(xml.contains("<Axes>"), "missing axes");
            // Real fallback SQL now returns a value
            assert!(
                xml.contains("<Cell "),
                "real fallback should have Cell elements"
            );
        });
    }

    #[test]
    fn retail_analytics_config_has_no_placeholder_sql() {
        // Verify the checked-in config contract: no converted measure
        // should use SUM(1), SUM(...), AVG(...), etc. as sql_expr.
        let config_text =
            std::fs::read_to_string("projects/generated_retail_analytics/proxy-config.json")
                .expect("read retail config");
        let _line = config_text
            .lines()
            .find(|l| l.contains("sql_expr"))
            .unwrap_or("");
        // All measures should be sql_fallback (sql_expr: "null").
        // Placeholder aggregations should never appear.
        assert!(
            !config_text.contains("SUM(1)"),
            "SUM(1) placeholder found in config"
        );
        assert!(
            !config_text.contains("SUM(...)"),
            "SUM(...) placeholder found in config"
        );
        assert!(
            !config_text.contains("AVG(...)"),
            "AVG(...) placeholder found in config"
        );
        assert!(
            !config_text.contains("COUNT(...)"),
            "COUNT(...) placeholder found in config"
        );
        assert!(
            !config_text.contains("COUNT(DISTINCT ...)"),
            "COUNT(DISTINCT ...) placeholder found in config"
        );
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
    fn contoso_sales_amount_returns_data() {
        use duckdb::Connection;
        let project = crate::project::project::ProxyProject::load(
            "projects/generated_contoso/proxy-config.json",
        )
        .expect("load contoso config");
        let conn =
            Connection::open("projects/generated_contoso/data/sales.db").expect("open contoso db");
        crate::project::project::with_test_project(project, || {
            let backend = FileQueryBackend(std::sync::Mutex::new(conn));
            let mdx = "SELECT FROM [SALES] WHERE ([Measures].[Sales Amount]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";
            let xml = crate::execute_builders::get_execute_cellset_response_with_backend(
                mdx,
                &backend,
                &crate::proxy_project::project().model,
            );
            assert!(
                xml.contains("<CellData>"),
                "Contoso Sales Amount should produce cellset"
            );
            assert!(
                xml.contains("7305939"),
                "Sales Amount should be > 7M, got {:?}",
                &xml[..xml.len().min(500)]
            );
        });
    }

    #[test]
    fn drilldown_member_year_to_quarter() {
        with_project3(|| {
            let mdx = r##"SELECT NON EMPTY Hierarchize(DrilldownMember({{DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}}, {[Date].[Calendar].[Year].&[2024]},,,INCLUDE_CALC_MEMBERS)) ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR"##;
            let xml = get_execute_statement_response(mdx);
            assert!(xml.contains("<CellData>"), "should produce cellset");
            assert!(
                xml.contains("<Caption>2024</Caption>"),
                "should have year as parent"
            );
            assert!(
                xml.contains("<Caption>1</Caption>"),
                "should have quarter 1"
            );
            assert!(
                xml.contains("<Caption>2</Caption>"),
                "should have quarter 2"
            );
            assert!(
                xml.contains("<Caption>3</Caption>"),
                "should have quarter 3"
            );
            assert!(
                xml.contains("<Caption>4</Caption>"),
                "should have quarter 4"
            );
            // Should have real values
            assert!(xml.contains("xsd:double"), "should have numeric values");
        });
    }

    #[test]
    fn drilldown_member_keeps_the_whole_input_set() {
        // Excel's "expand this year to quarters" in a Channel x Date pivot.
        // SSAS returns the full DrilldownLevel({All}) input set with only the
        // target expanded (verified against the reference engine: All, every
        // year in order, the target's four quarters right after the target).
        // Returning just the expanded branch made Excel error out (plan 048).
        with_project3(|| {
            let years = data_year_keys();
            let target = years.get(1).cloned().unwrap_or_else(|| years[0].clone());
            let mdx = format!(
                "SELECT NON EMPTY CrossJoin(Hierarchize({{DrilldownLevel({{[Channel].[Channel].[All]}},,,INCLUDE_CALC_MEMBERS)}}), Hierarchize(DrilldownMember({{{{DrilldownLevel({{[Date].[Calendar].[All]}},,,INCLUDE_CALC_MEMBERS)}}}}, {{[Date].[Calendar].[Year].&[{target}]}},,,INCLUDE_CALC_MEMBERS))) DIMENSION PROPERTIES PARENT_UNIQUE_NAME ON COLUMNS FROM [Sales] CELL PROPERTIES VALUE"
            );
            let xml = get_execute_statement_response(&mdx);
            let axis = xml
                .split("<Axis name=\"Axis0\">")
                .nth(1)
                .and_then(|s| s.split("</Axis>").next())
                .unwrap_or_default();
            let mut date_members: Vec<String> = Vec::new();
            for tuple in axis.split("<Tuple>").skip(1) {
                let names: Vec<&str> = tuple
                    .match_indices("<UName>")
                    .map(|(i, _)| {
                        let rest = &tuple[i + "<UName>".len()..];
                        &rest[..rest.find('<').unwrap_or(0)]
                    })
                    .collect();
                if names.len() == 2 && !date_members.iter().any(|d| d == names[1]) {
                    date_members.push(names[1].to_string());
                }
            }
            let mut expected = vec!["[Date].[Calendar].[All]".to_string()];
            for year in &years {
                expected.push(format!("[Date].[Calendar].[Year].&amp;[{year}]"));
                if *year == target {
                    for quarter in ["1", "2", "3", "4"] {
                        expected.push(format!(
                            "[Date].[Calendar].[Quarter].&amp;[{year}]&amp;[{quarter}]"
                        ));
                    }
                }
            }
            assert_eq!(date_members, expected, "{date_members:?}");
        });
    }

    #[test]
    fn cubevalue_metadata_probe_returns_measure_info() {
        with_project3(|| {
            let mdx = r##"WITH MEMBER [Measures].[XL_SD0] AS 'strtomember("[Measures].[Revenue]").UniqueName' MEMBER [Measures].[XL_SD1] AS 'strtomember("[Measures].[Revenue]").properties("caption")' MEMBER [Measures].[XL_SD2] AS '{strtomember("[Measures].[Revenue]")}.item(0).item(0).level.UniqueName' SELECT {[Measures].[XL_SD0],[Measures].[XL_SD1],[Measures].[XL_SD2]} ON 0 FROM  CELL PROPERTIES VALUE"##;
            let xml = get_execute_statement_response(mdx);
            assert!(
                xml.contains("[Measures].[Revenue]"),
                "should contain measure unique name"
            );
            assert!(
                xml.contains("<Caption>XL_SD1</Caption>"),
                "metadata probe members should be captioned with the XL_SD label"
            );
            assert!(
                xml.contains("[Measures].[MeasuresLevel]"),
                "should contain measures level"
            );
            assert!(
                xml.contains("<Value>Revenue</Value>"),
                "should emit plain string Value with measure caption"
            );
        });
    }

    #[test]
    fn cubevalue_metadata_probe_handles_multiple_targets() {
        with_project3(|| {
            let mdx = r##"WITH MEMBER [Measures].[XL_SD0] AS 'strtomember("[Category].[Category].&[Electronics]").UniqueName' MEMBER [Measures].[XL_SD1] AS 'strtomember("[Category].[Category].&[Electronics]").properties("caption")' MEMBER [Measures].[XL_SD2] AS '{strtomember("[Category].[Category].&[Electronics]")}.item(0).item(0).level.UniqueName' MEMBER [Measures].[XL_SD3] AS 'strtomember("[Measures].[Revenue]").UniqueName' MEMBER [Measures].[XL_SD4] AS 'strtomember("[Measures].[Revenue]").properties("caption")' MEMBER [Measures].[XL_SD5] AS '{strtomember("[Measures].[Revenue]")}.item(0).item(0).level.UniqueName' SELECT {[Measures].[XL_SD0],[Measures].[XL_SD1],[Measures].[XL_SD2],[Measures].[XL_SD3],[Measures].[XL_SD4],[Measures].[XL_SD5]} ON 0 FROM  CELL PROPERTIES VALUE"##;
            let xml = get_execute_statement_response(mdx);
            assert!(
                xml.contains("<Value>[Category].[Category].&amp;[Electronics]</Value>"),
                "member unique name should be present and escaped"
            );
            assert!(
                xml.contains("<Value>Electronics</Value>"),
                "member caption should be present"
            );
            assert!(
                xml.contains("<Value>[Measures].[Revenue]</Value>"),
                "measure unique name should be present"
            );
            assert!(
                xml.contains("<Value>Revenue</Value>"),
                "measure caption should be present"
            );
            assert!(
                xml.contains("<Caption>XL_SD5</Caption>"),
                "sixth XL_SD member should be captioned XL_SD5"
            );
        });
    }

    #[test]
    fn cubevalue_tuple_filters_measure_by_member() {
        with_project3(|| {
            let mdx = "SELECT {([Measures].[Revenue],[Category].[Category].&[Electronics])} ON 0 FROM [Sales] CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";
            let xml = get_execute_statement_response(mdx);
            assert!(
                xml.contains("[Category].[Category].&amp;[Electronics]"),
                "member should appear in the slicer, got no member"
            );
            assert!(
                xml.contains("24719896"),
                "should return the Electronics-filtered total, not the grand total"
            );
            assert!(
                !xml.contains("521586767"),
                "must not return the unfiltered grand total"
            );
        });
    }
}
