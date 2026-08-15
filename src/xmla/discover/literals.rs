use crate::proxy_project;
use crate::response::discover_rowset_envelope;

const LITERAL_ROW_FIELDS: &str = r#"                <xsd:element sql:field="LITERAL_NAME" name="LITERAL_NAME" type="xsd:string"/>
                <xsd:element sql:field="LITERAL_VALUE" name="LITERAL_VALUE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LITERAL_INVALID_CHARS" name="LITERAL_INVALID_CHARS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LITERAL_INVALID_STARTING_CHARS" name="LITERAL_INVALID_STARTING_CHARS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LITERAL_MAX_LENGTH" name="LITERAL_MAX_LENGTH" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="LITERAL_SUFFIX" name="LITERAL_SUFFIX" type="xsd:string" minOccurs="0"/>"#;

pub fn get_literals_response() -> String {
    let catalog = &proxy_project::project().config.catalog;
    let rows = format!(
        r#"          <row><LITERAL_NAME>DBLITERAL_CATALOG_NAME</LITERAL_NAME><LITERAL_VALUE>{catalog}</LITERAL_VALUE><LITERAL_MAX_LENGTH>128</LITERAL_MAX_LENGTH></row>
          <row><LITERAL_NAME>DBLITERAL_CATALOG_SEPARATOR</LITERAL_NAME><LITERAL_VALUE>.</LITERAL_VALUE></row>
          <row><LITERAL_NAME>DBLITERAL_QUOTE_PREFIX</LITERAL_NAME><LITERAL_VALUE>[</LITERAL_VALUE></row>
          <row><LITERAL_NAME>DBLITERAL_QUOTE_SUFFIX</LITERAL_NAME><LITERAL_VALUE>]</LITERAL_VALUE></row>
          <row><LITERAL_NAME>DBLITERAL_PASS_THROUGH_COLUMNS</LITERAL_NAME><LITERAL_VALUE>true</LITERAL_VALUE></row>"#
    );
    discover_rowset_envelope("", LITERAL_ROW_FIELDS, &rows)
}
