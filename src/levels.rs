use crate::response::{discover_rowset_envelope, UUID_TYPE};

const LEVEL_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="HIERARCHY_UNIQUE_NAME" name="HIERARCHY_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="LEVEL_NAME" name="LEVEL_NAME" type="xsd:string"/>
                <xsd:element sql:field="LEVEL_UNIQUE_NAME" name="LEVEL_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="LEVEL_GUID" name="LEVEL_GUID" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_CAPTION" name="LEVEL_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_NUMBER" name="LEVEL_NUMBER" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_CARDINALITY" name="LEVEL_CARDINALITY" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_TYPE" name="LEVEL_TYPE" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUSTOM_ROLLUP_SETTINGS" name="CUSTOM_ROLLUP_SETTINGS" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_UNIQUE_SETTINGS" name="LEVEL_UNIQUE_SETTINGS" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_IS_VISIBLE" name="LEVEL_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_ORDERING_PROPERTY" name="LEVEL_ORDERING_PROPERTY" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_DBTYPE" name="LEVEL_DBTYPE" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_MASTER_UNIQUE_NAME" name="LEVEL_MASTER_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_NAME_SQL_COLUMN_NAME" name="LEVEL_NAME_SQL_COLUMN_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_KEY_SQL_COLUMN_NAME" name="LEVEL_KEY_SQL_COLUMN_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_UNIQUE_NAME_SQL_COLUMN_NAME" name="LEVEL_UNIQUE_NAME_SQL_COLUMN_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_ATTRIBUTE_HIERARCHY_NAME" name="LEVEL_ATTRIBUTE_HIERARCHY_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_KEY_CARDINALITY" name="LEVEL_KEY_CARDINALITY" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_ORIGIN" name="LEVEL_ORIGIN" type="xsd:unsignedShort" minOccurs="0"/>"#;

const LEVEL_ROWS: &str = r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Measures]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_NAME>Measures</LEVEL_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <LEVEL_CAPTION>Measures</LEVEL_CAPTION>
            <LEVEL_NUMBER>0</LEVEL_NUMBER>
            <LEVEL_CARDINALITY>1</LEVEL_CARDINALITY>
            <LEVEL_TYPE>0</LEVEL_TYPE>
            <CUSTOM_ROLLUP_SETTINGS>0</CUSTOM_ROLLUP_SETTINGS>
            <LEVEL_UNIQUE_SETTINGS>1</LEVEL_UNIQUE_SETTINGS>
            <LEVEL_IS_VISIBLE>true</LEVEL_IS_VISIBLE>
            <LEVEL_DBTYPE>5</LEVEL_DBTYPE>
            <LEVEL_KEY_CARDINALITY>1</LEVEL_KEY_CARDINALITY>
            <LEVEL_ORIGIN>6</LEVEL_ORIGIN>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_NAME>(All)</LEVEL_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[(All)]</LEVEL_UNIQUE_NAME>
            <LEVEL_CAPTION>(All)</LEVEL_CAPTION>
            <LEVEL_NUMBER>0</LEVEL_NUMBER>
            <LEVEL_CARDINALITY>1</LEVEL_CARDINALITY>
            <LEVEL_TYPE>1</LEVEL_TYPE>
            <CUSTOM_ROLLUP_SETTINGS>0</CUSTOM_ROLLUP_SETTINGS>
            <LEVEL_UNIQUE_SETTINGS>1</LEVEL_UNIQUE_SETTINGS>
            <LEVEL_IS_VISIBLE>true</LEVEL_IS_VISIBLE>
            <LEVEL_DBTYPE>130</LEVEL_DBTYPE>
            <LEVEL_KEY_CARDINALITY>1</LEVEL_KEY_CARDINALITY>
            <LEVEL_ORIGIN>1</LEVEL_ORIGIN>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_NAME>Produktkategori</LEVEL_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <LEVEL_CAPTION>Produktkategori</LEVEL_CAPTION>
            <LEVEL_NUMBER>1</LEVEL_NUMBER>
            <LEVEL_CARDINALITY>50</LEVEL_CARDINALITY>
            <LEVEL_TYPE>0</LEVEL_TYPE>
            <CUSTOM_ROLLUP_SETTINGS>0</CUSTOM_ROLLUP_SETTINGS>
            <LEVEL_UNIQUE_SETTINGS>1</LEVEL_UNIQUE_SETTINGS>
            <LEVEL_IS_VISIBLE>true</LEVEL_IS_VISIBLE>
            <LEVEL_DBTYPE>130</LEVEL_DBTYPE>
            <LEVEL_KEY_CARDINALITY>50</LEVEL_KEY_CARDINALITY>
            <LEVEL_ORIGIN>1</LEVEL_ORIGIN>
          </row>"#;

pub fn get_levels_response() -> String {
    discover_rowset_envelope(UUID_TYPE, LEVEL_ROW_FIELDS, LEVEL_ROWS)
}
