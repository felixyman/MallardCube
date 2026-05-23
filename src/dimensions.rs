use crate::response::{discover_rowset_envelope, UUID_TYPE};

const DIM_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_NAME" name="DIMENSION_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_GUID" name="DIMENSION_GUID" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_CAPTION" name="DIMENSION_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_ORDINAL" name="DIMENSION_ORDINAL" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_TYPE" name="DIMENSION_TYPE" type="xsd:short" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_CARDINALITY" name="DIMENSION_CARDINALITY" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DEFAULT_HIERARCHY" name="DEFAULT_HIERARCHY" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="IS_VIRTUAL" name="IS_VIRTUAL" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="IS_READWRITE" name="IS_READWRITE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_SETTINGS" name="DIMENSION_UNIQUE_SETTINGS" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_MASTER_UNIQUE_NAME" name="DIMENSION_MASTER_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_IS_VISIBLE" name="DIMENSION_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="CUBE_SOURCE" name="CUBE_SOURCE" type="xsd:unsignedShort" minOccurs="0"/>"#;

const DIM_ROWS: &str = r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_NAME>Measures</DIMENSION_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_GUID>00000000-0000-0000-0000-000000000001</DIMENSION_GUID>
            <DIMENSION_CAPTION>Measures</DIMENSION_CAPTION>
            <DIMENSION_ORDINAL>0</DIMENSION_ORDINAL>
            <DIMENSION_TYPE>2</DIMENSION_TYPE>
            <DIMENSION_CARDINALITY>1</DIMENSION_CARDINALITY>
            <DEFAULT_HIERARCHY>[Measures]</DEFAULT_HIERARCHY>
            <DESCRIPTION>Measures system dimension</DESCRIPTION>
            <IS_VIRTUAL>false</IS_VIRTUAL>
            <IS_READWRITE>false</IS_READWRITE>
            <DIMENSION_UNIQUE_SETTINGS>0</DIMENSION_UNIQUE_SETTINGS>
            <DIMENSION_MASTER_UNIQUE_NAME>[Measures]</DIMENSION_MASTER_UNIQUE_NAME>
            <DIMENSION_IS_VISIBLE>false</DIMENSION_IS_VISIBLE>
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_NAME>Produktkategori</DIMENSION_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_GUID>00000000-0000-0000-0000-000000000002</DIMENSION_GUID>
            <DIMENSION_CAPTION>Produktkategori</DIMENSION_CAPTION>
            <DIMENSION_ORDINAL>1</DIMENSION_ORDINAL>
            <DIMENSION_TYPE>3</DIMENSION_TYPE>
            <DIMENSION_CARDINALITY>50</DIMENSION_CARDINALITY>
            <DEFAULT_HIERARCHY>[Produktkategori].[Produktkategori]</DEFAULT_HIERARCHY>
            <DESCRIPTION>Våra olika produkter</DESCRIPTION>
            <IS_VIRTUAL>false</IS_VIRTUAL>
            <IS_READWRITE>false</IS_READWRITE>
            <DIMENSION_UNIQUE_SETTINGS>0</DIMENSION_UNIQUE_SETTINGS>
            <DIMENSION_MASTER_UNIQUE_NAME>[Produktkategori]</DIMENSION_MASTER_UNIQUE_NAME>
            <DIMENSION_IS_VISIBLE>true</DIMENSION_IS_VISIBLE>
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_NAME>Region</DIMENSION_NAME>
            <DIMENSION_UNIQUE_NAME>[Region]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_GUID>00000000-0000-0000-0000-000000000003</DIMENSION_GUID>
            <DIMENSION_CAPTION>Region</DIMENSION_CAPTION>
            <DIMENSION_ORDINAL>2</DIMENSION_ORDINAL>
            <DIMENSION_TYPE>3</DIMENSION_TYPE>
            <DIMENSION_CARDINALITY>10</DIMENSION_CARDINALITY>
            <DEFAULT_HIERARCHY>[Region].[Region]</DEFAULT_HIERARCHY>
            <DESCRIPTION>Geografisk region</DESCRIPTION>
            <IS_VIRTUAL>false</IS_VIRTUAL>
            <IS_READWRITE>false</IS_READWRITE>
            <DIMENSION_UNIQUE_SETTINGS>0</DIMENSION_UNIQUE_SETTINGS>
            <DIMENSION_MASTER_UNIQUE_NAME>[Region]</DIMENSION_MASTER_UNIQUE_NAME>
            <DIMENSION_IS_VISIBLE>true</DIMENSION_IS_VISIBLE>
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>"#;

pub fn get_dimensions_response() -> String {
    discover_rowset_envelope(UUID_TYPE, DIM_ROW_FIELDS, DIM_ROWS)
}
