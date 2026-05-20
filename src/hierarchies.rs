use crate::response::{discover_rowset_envelope, UUID_TYPE};

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
                <xsd:element sql:field="STRUCTURE_TYPE" name="STRUCTURE_TYPE" type="xsd:string" minOccurs="0"/>"#;

const HIER_ROWS: &str = r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_NAME>Measures</HIERARCHY_NAME>
            <HIERARCHY_UNIQUE_NAME>[Measures]</HIERARCHY_UNIQUE_NAME>
            <HIERARCHY_CAPTION>Measures</HIERARCHY_CAPTION>
            <DIMENSION_TYPE>2</DIMENSION_TYPE>
            <HIERARCHY_CARDINALITY>1</HIERARCHY_CARDINALITY>
            <DEFAULT_MEMBER>[Measures].[Total Försäljning]</DEFAULT_MEMBER>
            <STRUCTURE>3</STRUCTURE>
            <DIMENSION_IS_VISIBLE>true</DIMENSION_IS_VISIBLE>
            <HIERARCHY_ORDINAL>0</HIERARCHY_ORDINAL>
            <DIMENSION_IS_SHARED>true</DIMENSION_IS_SHARED>
            <HIERARCHY_IS_VISIBLE>true</HIERARCHY_IS_VISIBLE>
            <HIERARCHY_ORIGIN>2</HIERARCHY_ORIGIN>
            <INSTANCE_SELECTION>1</INSTANCE_SELECTION>
            <GROUPING_BEHAVIOR>0</GROUPING_BEHAVIOR>
            <STRUCTURE_TYPE>Flat</STRUCTURE_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_NAME>Produktkategori</HIERARCHY_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <HIERARCHY_CAPTION>Produktkategori</HIERARCHY_CAPTION>
            <DIMENSION_TYPE>0</DIMENSION_TYPE>
            <HIERARCHY_CARDINALITY>50</HIERARCHY_CARDINALITY>
            <DEFAULT_MEMBER>[Produktkategori].[All Produktkategorier]</DEFAULT_MEMBER>
            <ALL_MEMBER>[Produktkategori].[All Produktkategorier]</ALL_MEMBER>
            <STRUCTURE>3</STRUCTURE>
            <DIMENSION_IS_VISIBLE>true</DIMENSION_IS_VISIBLE>
            <HIERARCHY_ORDINAL>1</HIERARCHY_ORDINAL>
            <DIMENSION_IS_SHARED>true</DIMENSION_IS_SHARED>
            <HIERARCHY_IS_VISIBLE>true</HIERARCHY_IS_VISIBLE>
            <HIERARCHY_ORIGIN>1</HIERARCHY_ORIGIN>
            <INSTANCE_SELECTION>1</INSTANCE_SELECTION>
            <GROUPING_BEHAVIOR>0</GROUPING_BEHAVIOR>
            <STRUCTURE_TYPE>Flat</STRUCTURE_TYPE>
          </row>"#;

pub fn get_hierarchies_response() -> String {
    discover_rowset_envelope(UUID_TYPE, HIER_ROW_FIELDS, HIER_ROWS)
}
