use crate::proxy_project;
use crate::response::{UUID_TYPE, discover_rowset_envelope, xml_escape};

const HIER_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="HIERARCHY_NAME" name="HIERARCHY_NAME" type="xsd:string"/>
                <xsd:element sql:field="HIERARCHY_UNIQUE_NAME" name="HIERARCHY_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="HIERARCHY_GUID" name="HIERARCHY_GUID" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="HIERARCHY_CAPTION" name="HIERARCHY_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_TYPE" name="DIMENSION_TYPE" type="xsd:short" minOccurs="0"/>
                <xsd:element sql:field="HIERARCHY_CARDINALITY" name="HIERARCHY_CARDINALITY" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="DEFAULT_MEMBER" name="DEFAULT_MEMBER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="ALL_MEMBER" name="ALL_MEMBER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="STRUCTURE" name="STRUCTURE" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="IS_VIRTUAL" name="IS_VIRTUAL" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="IS_READWRITE" name="IS_READWRITE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_SETTINGS" name="DIMENSION_UNIQUE_SETTINGS" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_IS_VISIBLE" name="DIMENSION_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="HIERARCHY_ORDINAL" name="HIERARCHY_ORDINAL" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_IS_SHARED" name="DIMENSION_IS_SHARED" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="HIERARCHY_IS_VISIBLE" name="HIERARCHY_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="HIERARCHY_ORIGIN" name="HIERARCHY_ORIGIN" type="xsd:unsignedShort" minOccurs="0"/>
                <xsd:element sql:field="HIERARCHY_DISPLAY_FOLDER" name="HIERARCHY_DISPLAY_FOLDER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="INSTANCE_SELECTION" name="INSTANCE_SELECTION" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="GROUPING_BEHAVIOR" name="GROUPING_BEHAVIOR" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="STRUCTURE_TYPE" name="STRUCTURE_TYPE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_SOURCE" name="CUBE_SOURCE" type="xsd:unsignedShort" minOccurs="0"/>"#;

pub fn get_hierarchies_response() -> String {
    let project = proxy_project::project();
    let model = &project.model;
    let mut rows = String::new();

    // Measures hierarchy (special case, not in model)
    rows.push_str(&format!(
        r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_NAME>Measures</HIERARCHY_NAME>
            <HIERARCHY_UNIQUE_NAME>[Measures]</HIERARCHY_UNIQUE_NAME>
            <HIERARCHY_GUID>00000000-0000-0000-0000-000000000050</HIERARCHY_GUID>
            <HIERARCHY_CAPTION>Measures</HIERARCHY_CAPTION>
            <DIMENSION_TYPE>2</DIMENSION_TYPE>
            <HIERARCHY_CARDINALITY>1</HIERARCHY_CARDINALITY>
            <DEFAULT_MEMBER>[Measures].[Total Försäljning]</DEFAULT_MEMBER>
            <STRUCTURE>0</STRUCTURE>
            <DIMENSION_IS_VISIBLE>false</DIMENSION_IS_VISIBLE>
            <HIERARCHY_ORDINAL>0</HIERARCHY_ORDINAL>
            <DIMENSION_IS_SHARED>true</DIMENSION_IS_SHARED>
            <HIERARCHY_IS_VISIBLE>false</HIERARCHY_IS_VISIBLE>
            <HIERARCHY_ORIGIN>2</HIERARCHY_ORIGIN>
            <HIERARCHY_DISPLAY_FOLDER></HIERARCHY_DISPLAY_FOLDER>
            <INSTANCE_SELECTION>0</INSTANCE_SELECTION>
            <GROUPING_BEHAVIOR>0</GROUPING_BEHAVIOR>
            <STRUCTURE_TYPE>Natural</STRUCTURE_TYPE>
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>
"#,
        catalog = project.config.catalog,
        cube = project.config.cube,
    ));

    for (i, d) in model.dimensions.iter().enumerate() {
        rows.push_str(&format!(
            r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>{dim_u}</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_NAME>{caption}</HIERARCHY_NAME>
            <HIERARCHY_UNIQUE_NAME>{hier_u}</HIERARCHY_UNIQUE_NAME>
            <HIERARCHY_GUID>00000000-0000-0000-0000-{guid:012}</HIERARCHY_GUID>
            <HIERARCHY_CAPTION>{caption}</HIERARCHY_CAPTION>
            <DIMENSION_TYPE>3</DIMENSION_TYPE>
            <HIERARCHY_CARDINALITY>{cardinality}</HIERARCHY_CARDINALITY>
            <DEFAULT_MEMBER>{all_member}</DEFAULT_MEMBER>
            <ALL_MEMBER>{all_member}</ALL_MEMBER>
            <STRUCTURE>0</STRUCTURE>
            <DIMENSION_IS_VISIBLE>{visible}</DIMENSION_IS_VISIBLE>
            <HIERARCHY_ORDINAL>0</HIERARCHY_ORDINAL>
            <DIMENSION_IS_SHARED>true</DIMENSION_IS_SHARED>
            <HIERARCHY_IS_VISIBLE>{visible}</HIERARCHY_IS_VISIBLE>
            <HIERARCHY_ORIGIN>2</HIERARCHY_ORIGIN>
            <HIERARCHY_DISPLAY_FOLDER></HIERARCHY_DISPLAY_FOLDER>
            <INSTANCE_SELECTION>0</INSTANCE_SELECTION>
            <GROUPING_BEHAVIOR>0</GROUPING_BEHAVIOR>
            <STRUCTURE_TYPE>Natural</STRUCTURE_TYPE>
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>
"#,
            dim_u = xml_escape(&d.dimension_unique_name()),
            caption = xml_escape(&d.caption),
            hier_u = xml_escape(&d.hierarchy_unique_name()),
            guid = 20 + i as u32,
            cardinality = d.cardinality_hint,
            all_member = xml_escape(&d.all_member_unique_name()),
            visible = d.visible,
            catalog = project.config.catalog,
            cube = project.config.cube,
        ));
    }

    discover_rowset_envelope(UUID_TYPE, HIER_ROW_FIELDS, &rows)
}
