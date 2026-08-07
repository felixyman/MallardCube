use crate::response::{UUID_TYPE, discover_rowset_envelope};

const TABLE_ROW_FIELDS: &str = r#"                <xsd:element sql:field="TABLE_CATALOG" name="TABLE_CATALOG" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_SCHEMA" name="TABLE_SCHEMA" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_NAME" name="TABLE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_TYPE" name="TABLE_TYPE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_GUID" name="TABLE_GUID" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_PROPID" name="TABLE_PROPID" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="DATE_CREATED" name="DATE_CREATED" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="DATE_MODIFIED" name="DATE_MODIFIED" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="TABLE_OLAP_TYPE" name="TABLE_OLAP_TYPE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string" minOccurs="0"/>"#;

const TABLE_ROWS: &str = r#"          <row>
            <TABLE_CATALOG>KTH_KEX_MALLOY_CUBE</TABLE_CATALOG>
            <TABLE_SCHEMA>Model</TABLE_SCHEMA>
            <TABLE_NAME>Faktatabell</TABLE_NAME>
            <TABLE_TYPE>SYSTEM TABLE</TABLE_TYPE>
            <TABLE_OLAP_TYPE>MEASURE_GROUP</TABLE_OLAP_TYPE>
            <CUBE_NAME>Model</CUBE_NAME>
          </row>
          <row>
            <TABLE_CATALOG>KTH_KEX_MALLOY_CUBE</TABLE_CATALOG>
            <TABLE_SCHEMA>Model</TABLE_SCHEMA>
            <TABLE_NAME>Produktkategori</TABLE_NAME>
            <TABLE_TYPE>TABLE</TABLE_TYPE>
            <TABLE_OLAP_TYPE>CUBE_DIMENSION</TABLE_OLAP_TYPE>
            <CUBE_NAME>Model</CUBE_NAME>
          </row>
          <row>
            <TABLE_CATALOG>KTH_KEX_MALLOY_CUBE</TABLE_CATALOG>
            <TABLE_SCHEMA>Model</TABLE_SCHEMA>
            <TABLE_NAME>Region</TABLE_NAME>
            <TABLE_TYPE>TABLE</TABLE_TYPE>
            <TABLE_OLAP_TYPE>CUBE_DIMENSION</TABLE_OLAP_TYPE>
            <CUBE_NAME>Model</CUBE_NAME>
          </row>"#;

pub fn get_tables_response() -> String {
    discover_rowset_envelope(UUID_TYPE, TABLE_ROW_FIELDS, TABLE_ROWS)
}
