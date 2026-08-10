/// DISCOVER_KEYWORDS — returns reserved XMLA/MDX keywords.
/// Required by Excel CUBE functions to validate the server's
/// MDX dialect support.
use crate::response::discover_rowset_envelope;

const KEYWORDS_FIELDS: &str =
    r#"                <xsd:element sql:field="Keyword" name="Keyword" type="xsd:string"/>"#;

pub fn get_keywords_response() -> String {
    let keywords = [
        "SELECT",
        "FROM",
        "WHERE",
        "NON",
        "EMPTY",
        "ON",
        "ROWS",
        "COLUMNS",
        "DIMENSION",
        "PROPERTIES",
        "MEMBER",
        "CELL",
        "PROPERTIES",
        "VALUE",
        "FORMAT_STRING",
        "BACK_COLOR",
        "FORE_COLOR",
        "FORMATTED_VALUE",
        "CELL_ORDINAL",
        "WITH",
        "SET",
        "AS",
        "MEMBERS",
        "CHILDREN",
        "DESCENDANTS",
        "HIERARCHIZE",
        "CROSSJOIN",
        "DRILLDOWNLEVEL",
        "DRILLDOWNMEMBER",
        "FILTER",
        "GENERATE",
        "ASCENDANTS",
        "ADDCALCULATEDMEMBERS",
        "INCLUDE_CALC_MEMBERS",
        "ALLMEMBERS",
    ];

    let xml_rows: String = keywords
        .iter()
        .map(|kw| format!("          <row>\n            <Keyword>{kw}</Keyword>\n          </row>"))
        .collect::<Vec<_>>()
        .join("\n");

    discover_rowset_envelope("", KEYWORDS_FIELDS, &xml_rows)
}
