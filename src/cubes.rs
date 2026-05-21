use crate::response::{discover_rowset_envelope, UUID_TYPE};

const CUBE_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_TYPE" name="CUBE_TYPE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_GUID" name="CUBE_GUID" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="CREATED_ON" name="CREATED_ON" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="LAST_SCHEMA_UPDATE" name="LAST_SCHEMA_UPDATE" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="SCHEMA_UPDATED_BY" name="SCHEMA_UPDATED_BY" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LAST_DATA_UPDATE" name="LAST_DATA_UPDATE" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="DATA_UPDATED_BY" name="DATA_UPDATED_BY" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="IS_DRILLTHROUGH_ENABLED" name="IS_DRILLTHROUGH_ENABLED" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="IS_LINKABLE" name="IS_LINKABLE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="IS_WRITE_ENABLED" name="IS_WRITE_ENABLED" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="IS_SQL_ENABLED" name="IS_SQL_ENABLED" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="CUBE_CAPTION" name="CUBE_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="BASE_CUBE_NAME" name="BASE_CUBE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_SOURCE" name="CUBE_SOURCE" type="xsd:unsignedShort" minOccurs="0"/>
                <xsd:element sql:field="PREFERRED_QUERY_PATTERNS" name="PREFERRED_QUERY_PATTERNS" type="xsd:unsignedShort" minOccurs="0"/>"#;

const CUBE_ROWS: &str = r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <SCHEMA_NAME>Model</SCHEMA_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <CUBE_TYPE>CUBE</CUBE_TYPE>
            <CUBE_GUID>00000000-0000-0000-0000-000000000010</CUBE_GUID>
            <CREATED_ON>2026-05-20T12:00:00.000000</CREATED_ON>
            <LAST_SCHEMA_UPDATE>2026-05-20T12:00:00.000000</LAST_SCHEMA_UPDATE>
            <SCHEMA_UPDATED_BY>RustProxy</SCHEMA_UPDATED_BY>
            <LAST_DATA_UPDATE>2026-05-20T12:00:00.000000</LAST_DATA_UPDATE>
            <DATA_UPDATED_BY>RustProxy</DATA_UPDATED_BY>
            <DESCRIPTION>Byggt med Rust och DuckDB!</DESCRIPTION>
            <IS_DRILLTHROUGH_ENABLED>true</IS_DRILLTHROUGH_ENABLED>
            <IS_LINKABLE>false</IS_LINKABLE>
            <IS_WRITE_ENABLED>false</IS_WRITE_ENABLED>
            <IS_SQL_ENABLED>false</IS_SQL_ENABLED>
            <CUBE_CAPTION>Model</CUBE_CAPTION>
            <BASE_CUBE_NAME>Model</BASE_CUBE_NAME>
            <CUBE_SOURCE>1</CUBE_SOURCE>
            <PREFERRED_QUERY_PATTERNS>0</PREFERRED_QUERY_PATTERNS>
          </row>"#;

pub fn get_cubes_response() -> String {
    discover_rowset_envelope(UUID_TYPE, CUBE_ROW_FIELDS, CUBE_ROWS)
}
