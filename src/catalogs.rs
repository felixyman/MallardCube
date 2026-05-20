use crate::response::{discover_rowset_envelope, UUID_TYPE};

const CATALOG_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="ROLES" name="ROLES" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DATE_MODIFIED" name="DATE_MODIFIED" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="COMPATIBILITY_LEVEL" name="COMPATIBILITY_LEVEL" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="TYPE" name="TYPE" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="VERSION" name="VERSION" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="DATABASE_ID" name="DATABASE_ID" type="xsd:string" minOccurs="0"/>"#;

const CATALOG_ROWS: &str = r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <DESCRIPTION>Världens första Rust-till-Malloy proxy</DESCRIPTION>
            <ROLES>*</ROLES>
            <DATE_MODIFIED>2026-05-20T12:00:00.000000</DATE_MODIFIED>
            <COMPATIBILITY_LEVEL>1500</COMPATIBILITY_LEVEL>
            <TYPE>3</TYPE>
            <VERSION>1</VERSION>
            <DATABASE_ID>KTH_KEX_MALLOY_CUBE</DATABASE_ID>
          </row>"#;

pub fn get_catalogs_response() -> String {
    discover_rowset_envelope(UUID_TYPE, CATALOG_ROW_FIELDS, CATALOG_ROWS)
}
