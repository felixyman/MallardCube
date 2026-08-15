use crate::proxy_project;
use crate::response::{UUID_TYPE, discover_rowset_envelope, xml_escape};

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

pub fn get_tables_response() -> String {
    let project = proxy_project::project();
    let catalog = &project.config.catalog;
    let cube = &project.config.cube;
    let mut rows = String::new();

    for ft in &project.model.fact_tables {
        rows.push_str(&format!(
            r#"          <row>
            <TABLE_CATALOG>{catalog}</TABLE_CATALOG>
            <TABLE_SCHEMA>{cube}</TABLE_SCHEMA>
            <TABLE_NAME>{table}</TABLE_NAME>
            <TABLE_TYPE>SYSTEM TABLE</TABLE_TYPE>
            <TABLE_OLAP_TYPE>MEASURE_GROUP</TABLE_OLAP_TYPE>
            <CUBE_NAME>{cube}</CUBE_NAME>
          </row>
"#,
            table = xml_escape(&ft.table_name),
        ));
    }
    for d in &project.model.dimensions {
        rows.push_str(&format!(
            r#"          <row>
            <TABLE_CATALOG>{catalog}</TABLE_CATALOG>
            <TABLE_SCHEMA>{cube}</TABLE_SCHEMA>
            <TABLE_NAME>{table}</TABLE_NAME>
            <TABLE_TYPE>TABLE</TABLE_TYPE>
            <TABLE_OLAP_TYPE>CUBE_DIMENSION</TABLE_OLAP_TYPE>
            <CUBE_NAME>{cube}</CUBE_NAME>
          </row>
"#,
            table = xml_escape(project.model.dim_table_for_discovery(&d.id)),
        ));
    }

    discover_rowset_envelope(UUID_TYPE, TABLE_ROW_FIELDS, &rows)
}
