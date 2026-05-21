// Project: xmla_proxy (v0.1.0)

// ./onefile.rs
// Project: xmla_proxy (v0.1.0)

// ./onefile.rs
// Project: xmla_proxy (v0.1.0)

// ./onefile.rs
// Project: xmla_proxy (v0.1.0)

// ./src/catalogs.rs
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

// ./src/cubes.rs
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
            <CUBE_NAME>Model</CUBE_NAME>
            <CUBE_TYPE>CUBE</CUBE_TYPE>
            <LAST_SCHEMA_UPDATE>2026-05-20T12:00:00.000000</LAST_SCHEMA_UPDATE>
            <LAST_DATA_UPDATE>2026-05-20T12:00:00.000000</LAST_DATA_UPDATE>
            <DESCRIPTION>Byggt med Rust och DuckDB!</DESCRIPTION>
            <IS_DRILLTHROUGH_ENABLED>true</IS_DRILLTHROUGH_ENABLED>
            <IS_LINKABLE>false</IS_LINKABLE>
            <IS_WRITE_ENABLED>false</IS_WRITE_ENABLED>
            <IS_SQL_ENABLED>false</IS_SQL_ENABLED>
            <CUBE_CAPTION>Malloy Analytics Cube</CUBE_CAPTION>
            <CUBE_SOURCE>1</CUBE_SOURCE>
            <PREFERRED_QUERY_PATTERNS>3</PREFERRED_QUERY_PATTERNS>
          </row>"#;

pub fn get_cubes_response() -> String {
    discover_rowset_envelope(UUID_TYPE, CUBE_ROW_FIELDS, CUBE_ROWS)
}

// ./src/dimensions.rs
use crate::response::discover_rowset_envelope;

const DIM_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_NAME" name="DIMENSION_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_CAPTION" name="DIMENSION_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_ORDINAL" name="DIMENSION_ORDINAL" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_TYPE" name="DIMENSION_TYPE" type="xsd:short" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_CARDINALITY" name="DIMENSION_CARDINALITY" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DEFAULT_HIERARCHY" name="DEFAULT_HIERARCHY" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="IS_VIRTUAL" name="IS_VIRTUAL" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="IS_READWRITE" name="IS_READWRITE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_SETTINGS" name="DIMENSION_UNIQUE_SETTINGS" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_IS_VISIBLE" name="DIMENSION_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>"#;

const DIM_ROWS: &str = r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <SCHEMA_NAME>Model</SCHEMA_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_NAME>Measures</DIMENSION_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_CAPTION>Measures</DIMENSION_CAPTION>
            <DIMENSION_ORDINAL>0</DIMENSION_ORDINAL>
            <DIMENSION_TYPE>2</DIMENSION_TYPE>
            <DIMENSION_CARDINALITY>1</DIMENSION_CARDINALITY>
            <DEFAULT_HIERARCHY>[Measures]</DEFAULT_HIERARCHY>
            <DESCRIPTION>Detta är mätvärdena</DESCRIPTION>
            <IS_VIRTUAL>false</IS_VIRTUAL>
            <IS_READWRITE>false</IS_READWRITE>
            <DIMENSION_UNIQUE_SETTINGS>0</DIMENSION_UNIQUE_SETTINGS>
            <DIMENSION_IS_VISIBLE>true</DIMENSION_IS_VISIBLE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <SCHEMA_NAME>Model</SCHEMA_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_NAME>Produktkategori</DIMENSION_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_CAPTION>Produktkategori</DIMENSION_CAPTION>
            <DIMENSION_ORDINAL>1</DIMENSION_ORDINAL>
            <DIMENSION_TYPE>0</DIMENSION_TYPE>
            <DIMENSION_CARDINALITY>50</DIMENSION_CARDINALITY>
            <DEFAULT_HIERARCHY>[Produktkategori]</DEFAULT_HIERARCHY>
            <DESCRIPTION>Våra olika produkter</DESCRIPTION>
            <IS_VIRTUAL>false</IS_VIRTUAL>
            <IS_READWRITE>false</IS_READWRITE>
            <DIMENSION_UNIQUE_SETTINGS>0</DIMENSION_UNIQUE_SETTINGS>
            <DIMENSION_IS_VISIBLE>true</DIMENSION_IS_VISIBLE>
          </row>"#;

pub fn get_dimensions_response() -> String {
    discover_rowset_envelope("", DIM_ROW_FIELDS, DIM_ROWS)
}

// ./src/execute.rs
use crate::response::wrap_in_soap_envelope;

pub fn get_empty_execute_response() -> String {
    wrap_in_soap_envelope(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:empty"/>
      </return>
    </ExecuteResponse>"#,
    )
}

pub fn get_execute_statement_response(mdx: &str) -> String {
    let has_measures = mdx.contains("Measures") || mdx.contains("measures");
    let measure_name = "Total_Försäljning";
    let measure_value = if has_measures { "1250000.5" } else { "" };

    let inner = format!(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence minOccurs="0" maxOccurs="unbounded"><xsd:element name="row" type="row"/></xsd:sequence></xsd:complexType>
            </xsd:element>
            <xsd:complexType name="row">
              <xsd:sequence>
                <xsd:element sql:field="{name}" name="{name}" type="xsd:double" minOccurs="0"/>
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
          <row>
            <{name}>{val}</{name}>
          </row>
        </root>
      </return>
    </ExecuteResponse>"#,
        name = measure_name,
        val = measure_value,
    );
    wrap_in_soap_envelope(&inner)
}

// ./src/hierarchies.rs
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

// ./src/kpis.rs
use crate::response::discover_rowset_envelope;

const KPIS_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="KPI_NAME" name="KPI_NAME" type="xsd:string"/>
                <xsd:element sql:field="KPI_CAPTION" name="KPI_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_DESCRIPTION" name="KPI_DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_DISPLAY_FOLDER" name="KPI_DISPLAY_FOLDER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_VALUE" name="KPI_VALUE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_GOAL" name="KPI_GOAL" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_STATUS" name="KPI_STATUS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_TREND" name="KPI_TREND" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_STATUS_GRAPHIC" name="KPI_STATUS_GRAPHIC" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_TREND_GRAPHIC" name="KPI_TREND_GRAPHIC" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_WEIGHT" name="KPI_WEIGHT" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_CURRENT_TIME_MEMBER" name="KPI_CURRENT_TIME_MEMBER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_PARENT_KPI_NAME" name="KPI_PARENT_KPI_NAME" type="xsd:string" minOccurs="0"/>"#;

pub fn get_kpis_response() -> String {
    discover_rowset_envelope("", KPIS_ROW_FIELDS, "")
}

// ./src/levels.rs
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

// ./src/literals.rs
use crate::response::discover_rowset_envelope;

const LITERAL_ROW_FIELDS: &str = r#"                <xsd:element sql:field="LITERAL_NAME" name="LITERAL_NAME" type="xsd:string"/>
                <xsd:element sql:field="LITERAL_VALUE" name="LITERAL_VALUE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LITERAL_INVALID_CHARS" name="LITERAL_INVALID_CHARS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LITERAL_INVALID_STARTING_CHARS" name="LITERAL_INVALID_STARTING_CHARS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LITERAL_MAX_LENGTH" name="LITERAL_MAX_LENGTH" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="LITERAL_SUFFIX" name="LITERAL_SUFFIX" type="xsd:string" minOccurs="0"/>"#;

const LITERAL_ROWS: &str = r#"          <row><LITERAL_NAME>DBLITERAL_CATALOG_NAME</LITERAL_NAME><LITERAL_VALUE>KTH_KEX_MALLOY_CUBE</LITERAL_VALUE><LITERAL_MAX_LENGTH>128</LITERAL_MAX_LENGTH></row>
          <row><LITERAL_NAME>DBLITERAL_CATALOG_SEPARATOR</LITERAL_NAME><LITERAL_VALUE>.</LITERAL_VALUE></row>
          <row><LITERAL_NAME>DBLITERAL_QUOTE_PREFIX</LITERAL_NAME><LITERAL_VALUE>[</LITERAL_VALUE></row>
          <row><LITERAL_NAME>DBLITERAL_QUOTE_SUFFIX</LITERAL_NAME><LITERAL_VALUE>]</LITERAL_VALUE></row>
          <row><LITERAL_NAME>DBLITERAL_PASS_THROUGH_COLUMNS</LITERAL_NAME><LITERAL_VALUE>true</LITERAL_VALUE></row>"#;

pub fn get_literals_response() -> String {
    discover_rowset_envelope("", LITERAL_ROW_FIELDS, LITERAL_ROWS)
}

// ./src/main.rs
use axum::{
    http::{header, HeaderMap, HeaderName, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use std::net::SocketAddr;

mod parser;
mod response;
mod properties;
mod schema_rowsets;
mod catalogs;
mod cubes;
mod tables;
mod dimensions;
mod measures;
mod hierarchies;
mod levels;
mod mdschema_properties;
mod members;
mod literals;
mod sets;
mod kpis;
mod measure_groups;
mod measuregroup_dimensions;
mod execute;

use parser::{parse_xmla, XmlaRequest};

#[tokio::main]
async fn main() {
    let app = Router::new().route("/xmla", post(handle_xmla));
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("🚀 Rust-XMLA Proxy (v3 - ModuleRefactor) snurrar på http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "text/xml; charset=utf-8".parse().unwrap());
    headers.insert(header::SERVER, "Rust-Malloy-Proxy/2.0".parse().unwrap());
    headers.insert(header::CONNECTION, "close".parse().unwrap());
    headers.insert(
        HeaderName::from_static("x-transport-caps-negotiation-flags"),
        "0,0,0,0,0".parse().unwrap(),
    );
    headers
}

async fn handle_xmla(body: String) -> impl IntoResponse {
    if body.contains("<RequestType>") {
        let req_start = body.find("<RequestType>").unwrap() + 13;
        let req_end = body.find("</RequestType>").unwrap();
        println!("🔍 Rå RequestType från Excel: {}", &body[req_start..req_end]);
    }

    let headers = default_headers();
    let request = parse_xmla(&body);
    println!("📥 Fick anrop, tolkade som: {:?}", request);

    if body.contains("<Execute") {
        println!("🔍 Rå Execute från Excel:\n{}", body);
    }

    let response_body = match request {
        XmlaRequest::BeginSession | XmlaRequest::ExecuteEmpty => {
            execute::get_empty_execute_response()
        }

        XmlaRequest::DiscoverProperties { property_names } => {
            if property_names.len() == 1 && property_names[0] == "Catalog" {
                println!("Excel frågar efter Catalog");
                properties::get_single_property_response("Catalog", "KTH_KEX_MALLOY_CUBE")
            } else {
                println!("Excel frågar efter egenskaper: {:?}", property_names);
                properties::get_properties_response(&property_names)
            }
        }

        XmlaRequest::DiscoverSchemaRowsets => schema_rowsets::get_schemas_response(),
        XmlaRequest::DbSchemaCatalogs => catalogs::get_catalogs_response(),
        XmlaRequest::MdschemaCubes => cubes::get_cubes_response(),
        XmlaRequest::DbschemaTables => tables::get_tables_response(),
        XmlaRequest::MdschemaDimensions => {
            println!("📥 Skickar Dimensioner till Excel!");
            dimensions::get_dimensions_response()
        }
        XmlaRequest::MdschemaMeasures => {
            println!("📥 Skickar Measures till Excel!");
            measures::get_measures_response()
        }
        XmlaRequest::MdschemaHierarchies => {
            println!("📥 Hierarchies");
            hierarchies::get_hierarchies_response()
        }
        XmlaRequest::MdschemaLevels => {
            println!("📥 Levels");
            levels::get_levels_response()
        }
        XmlaRequest::ExecuteStatement(mdx) => {
            println!("📥 MDX Statement: {}", mdx);
            execute::get_execute_statement_response(&mdx)
        }
        XmlaRequest::MdschemaProperties { property_type } => {
            println!("📥 MDSCHEMA_PROPERTIES (PROPERTY_TYPE={:?})", property_type);
            mdschema_properties::get_mdschema_properties_response(property_type)
        }
        XmlaRequest::MdschemaMembers => {
            println!("📥 MDSCHEMA_MEMBERS");
            members::get_members_response()
        }
        XmlaRequest::DiscoverLiterals => {
            println!("📥 DISCOVER_LITERALS");
            literals::get_literals_response()
        }
        XmlaRequest::MdschemaSets => {
            println!("📥 MDSCHEMA_SETS");
            sets::get_sets_response()
        }
        XmlaRequest::MdschemaKpis => {
            println!("📥 MDSCHEMA_KPIS");
            kpis::get_kpis_response()
        }
        XmlaRequest::MdschemaMeasureGroups => {
            println!("📥 MDSCHEMA_MEASUREGROUPS");
            measure_groups::get_measure_groups_response()
        }
        XmlaRequest::MdschemaMeasureGroupDimensions => {
            println!("📥 MDSCHEMA_MEASUREGROUP_DIMENSIONS");
            measuregroup_dimensions::get_measuregroup_dimensions_response()
        }

        XmlaRequest::Unknown => {
            println!("❌ Okänt anrop.");
            return (StatusCode::BAD_REQUEST, headers, "Okänt anrop".to_string());
        }
    };

    (StatusCode::OK, headers, response_body)
}

// ./src/mdschema_properties.rs
use crate::response::discover_rowset_envelope;

const PROPERTIES_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="HIERARCHY_UNIQUE_NAME" name="HIERARCHY_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_UNIQUE_NAME" name="LEVEL_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEMBER_UNIQUE_NAME" name="MEMBER_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_NAME" name="PROPERTY_NAME" type="xsd:string"/>
                <xsd:element sql:field="PROPERTY_CAPTION" name="PROPERTY_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_DESCRIPTION" name="PROPERTY_DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_TYPE" name="PROPERTY_TYPE" type="xsd:short" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_CONTENT_TYPE" name="PROPERTY_CONTENT_TYPE" type="xsd:short" minOccurs="0"/>"#;

fn member_property_rows() -> &'static str {
    r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_CAPTION</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_CAPTION</PROPERTY_CAPTION>
            <PROPERTY_TYPE>1</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_KEY</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_KEY</PROPERTY_CAPTION>
            <PROPERTY_TYPE>1</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>1</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>"#
}

fn system_property_rows() -> &'static str {
    r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>FORMATTED_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>FORMATTED_VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>1</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>FORMAT_STRING</PROPERTY_NAME>
            <PROPERTY_CAPTION>FORMAT_STRING</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>2</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>FORE_COLOR</PROPERTY_NAME>
            <PROPERTY_CAPTION>FORE_COLOR</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>2</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>BACK_COLOR</PROPERTY_NAME>
            <PROPERTY_CAPTION>BACK_COLOR</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>2</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>FONT_NAME</PROPERTY_NAME>
            <PROPERTY_CAPTION>FONT_NAME</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>2</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>FONT_SIZE</PROPERTY_NAME>
            <PROPERTY_CAPTION>FONT_SIZE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>2</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>CELL_ORDINAL</PROPERTY_NAME>
            <PROPERTY_CAPTION>CELL_ORDINAL</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>"#
}

fn member_value_rows() -> &'static str {
    r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Measures]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>5</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[(All)]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>5</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>5</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>"#
}

pub fn get_mdschema_properties_response(property_type: Option<i32>) -> String {
    let rows = match property_type {
        Some(1) => member_property_rows().to_string(),
        Some(2) => system_property_rows().to_string(),
        Some(5) => member_value_rows().to_string(),
        _ => format!("{}\n{}", system_property_rows(), member_value_rows()),
    };
    discover_rowset_envelope("", PROPERTIES_ROW_FIELDS, &rows)
}

// ./src/measure_groups.rs
use crate::response::discover_rowset_envelope;

const MEASUREGROUP_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASUREGROUP_NAME" name="MEASUREGROUP_NAME" type="xsd:string"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="IS_WRITE_ENABLED" name="IS_WRITE_ENABLED" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="MEASUREGROUP_CAPTION" name="MEASUREGROUP_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASUREGROUP_CARDINALITY" name="MEASUREGROUP_CARDINALITY" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="MEASUREGROUP_SIZE" name="MEASUREGROUP_SIZE" type="xsd:long" minOccurs="0"/>"#;

pub fn get_measure_groups_response() -> String {
    discover_rowset_envelope("", MEASUREGROUP_ROW_FIELDS,
        r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <MEASUREGROUP_NAME>Faktatabell</MEASUREGROUP_NAME>
            <MEASUREGROUP_CAPTION>Faktatabell</MEASUREGROUP_CAPTION>
          </row>"#
    )
}

// ./src/measuregroup_dimensions.rs
use crate::response::discover_rowset_envelope;

const MG_DIM_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASUREGROUP_NAME" name="MEASUREGROUP_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASUREGROUP_CARDINALITY" name="MEASUREGROUP_CARDINALITY" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_CARDINALITY" name="DIMENSION_CARDINALITY" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_IS_VISIBLE" name="DIMENSION_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_IS_FACT_DIMENSION" name="DIMENSION_IS_FACT_DIMENSION" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_GRANULARITY" name="DIMENSION_GRANULARITY" type="xsd:string" minOccurs="0"/>"#;

pub fn get_measuregroup_dimensions_response() -> String {
    discover_rowset_envelope("", MG_DIM_ROW_FIELDS,
        r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <MEASUREGROUP_NAME>Faktatabell</MEASUREGROUP_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_IS_VISIBLE>true</DIMENSION_IS_VISIBLE>
            <DIMENSION_IS_FACT_DIMENSION>false</DIMENSION_IS_FACT_DIMENSION>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <MEASUREGROUP_NAME>Faktatabell</MEASUREGROUP_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_IS_VISIBLE>true</DIMENSION_IS_VISIBLE>
            <DIMENSION_IS_FACT_DIMENSION>false</DIMENSION_IS_FACT_DIMENSION>
          </row>"#
    )
}

// ./src/measures.rs
use crate::response::{discover_rowset_envelope, UUID_TYPE};

const MEASURE_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASURE_NAME" name="MEASURE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASURE_UNIQUE_NAME" name="MEASURE_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASURE_CAPTION" name="MEASURE_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_GUID" name="MEASURE_GUID" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_AGGREGATOR" name="MEASURE_AGGREGATOR" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DATA_TYPE" name="DATA_TYPE" type="xsd:unsignedShort" minOccurs="0"/>
                <xsd:element sql:field="NUMERIC_PRECISION" name="NUMERIC_PRECISION" type="xsd:unsignedShort" minOccurs="0"/>
                <xsd:element sql:field="NUMERIC_SCALE" name="NUMERIC_SCALE" type="xsd:short" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_UNITS" name="MEASURE_UNITS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="EXPRESSION" name="EXPRESSION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_IS_VISIBLE" name="MEASURE_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="LEVELS_LIST" name="LEVELS_LIST" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_NAME_SQL_COLUMN_NAME" name="MEASURE_NAME_SQL_COLUMN_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_UNQUALIFIED_CAPTION" name="MEASURE_UNQUALIFIED_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASUREGROUP_NAME" name="MEASUREGROUP_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_DISPLAY_FOLDER" name="MEASURE_DISPLAY_FOLDER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DEFAULT_FORMAT_STRING" name="DEFAULT_FORMAT_STRING" type="xsd:string" minOccurs="0"/>"#;

const MEASURE_ROWS: &str = r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <SCHEMA_NAME>Model</SCHEMA_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <MEASURE_NAME>Total Försäljning</MEASURE_NAME>
            <MEASURE_UNIQUE_NAME>[Measures].[Total Försäljning]</MEASURE_UNIQUE_NAME>
            <MEASURE_CAPTION>Total Försäljning (SEK)</MEASURE_CAPTION>
            <MEASURE_AGGREGATOR>1</MEASURE_AGGREGATOR>
            <DATA_TYPE>5</DATA_TYPE>
            <NUMERIC_PRECISION>18</NUMERIC_PRECISION>
            <NUMERIC_SCALE>2</NUMERIC_SCALE>
            <MEASURE_UNITS>SEK</MEASURE_UNITS>
            <DESCRIPTION>Vår totala försäljning</DESCRIPTION>
            <MEASURE_IS_VISIBLE>true</MEASURE_IS_VISIBLE>
            <MEASURE_UNQUALIFIED_CAPTION>Total Försäljning (SEK)</MEASURE_UNQUALIFIED_CAPTION>
            <MEASUREGROUP_NAME>Faktatabell</MEASUREGROUP_NAME>
            <DEFAULT_FORMAT_STRING>#,##0.00 SEK</DEFAULT_FORMAT_STRING>
          </row>"#;

pub fn get_measures_response() -> String {
    discover_rowset_envelope(UUID_TYPE, MEASURE_ROW_FIELDS, MEASURE_ROWS)
}

// ./src/members.rs
use crate::response::discover_rowset_envelope;

const MEMBER_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="HIERARCHY_UNIQUE_NAME" name="HIERARCHY_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="LEVEL_UNIQUE_NAME" name="LEVEL_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="LEVEL_NUMBER" name="LEVEL_NUMBER" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="MEMBER_ORDINAL" name="MEMBER_ORDINAL" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="MEMBER_NAME" name="MEMBER_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEMBER_UNIQUE_NAME" name="MEMBER_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEMBER_TYPE" name="MEMBER_TYPE" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="MEMBER_CAPTION" name="MEMBER_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CHILDREN_CARDINALITY" name="CHILDREN_CARDINALITY" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="PARENT_LEVEL" name="PARENT_LEVEL" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="PARENT_COUNT" name="PARENT_COUNT" type="xsd:unsignedInt" minOccurs="0"/>"#;

const MEMBER_ROWS: &str = r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Measures]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>0</LEVEL_NUMBER>
            <MEMBER_ORDINAL>0</MEMBER_ORDINAL>
            <MEMBER_NAME>Total Försäljning</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>[Measures].[Total Försäljning]</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>2</MEMBER_TYPE>
            <MEMBER_CAPTION>Total Försäljning</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>0</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>0</PARENT_LEVEL>
            <PARENT_COUNT>0</PARENT_COUNT>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[(All)]</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>0</LEVEL_NUMBER>
            <MEMBER_ORDINAL>0</MEMBER_ORDINAL>
            <MEMBER_NAME>All Produktkategorier</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>[Produktkategori].[All Produktkategorier]</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>1</MEMBER_TYPE>
            <MEMBER_CAPTION>All Produktkategorier</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>50</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>0</PARENT_LEVEL>
            <PARENT_COUNT>0</PARENT_COUNT>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>1</LEVEL_NUMBER>
            <MEMBER_ORDINAL>1</MEMBER_ORDINAL>
            <MEMBER_NAME>Kategori A</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>[Produktkategori].[Kategori A]</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>3</MEMBER_TYPE>
            <MEMBER_CAPTION>Kategori A</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>0</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>0</PARENT_LEVEL>
            <PARENT_COUNT>1</PARENT_COUNT>
          </row>"#;

pub fn get_members_response() -> String {
    discover_rowset_envelope("", MEMBER_ROW_FIELDS, MEMBER_ROWS)
}

// ./src/parser.rs
use quick_xml::events::Event;
use quick_xml::Reader;

#[derive(Debug, PartialEq)]
pub enum XmlaRequest {
    DiscoverProperties { property_names: Vec<String> },
    DiscoverSchemaRowsets,
    DiscoverLiterals,
    DbSchemaCatalogs,
    MdschemaCubes,
    DbschemaTables,
    MdschemaDimensions,
    MdschemaMeasures,
    MdschemaHierarchies,
    MdschemaLevels,
    MdschemaProperties { property_type: Option<i32> },
    MdschemaMembers,
    MdschemaSets,
    MdschemaKpis,
    MdschemaMeasureGroups,
    MdschemaMeasureGroupDimensions,
    BeginSession,
    ExecuteEmpty,
    ExecuteStatement(String),
    Unknown,
}

pub fn parse_xmla(xml: &str) -> XmlaRequest {
    let mut reader = Reader::from_str(xml);

    let mut in_request_type = false;
    let mut is_execute = false;
    let mut in_property_name = false;
    let mut in_statement = false;
    let mut is_begin_session = false;
    let mut in_property_type = false;

    let mut parsed_request_type = String::new();
    let mut requested_properties: Vec<String> = Vec::new();
    let mut statement_text = String::new();
    let mut property_type: Option<i32> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                match e.local_name().as_ref() {
                    b"RequestType" => in_request_type = true,
                    b"PropertyName" => in_property_name = true,
                    b"Statement" => in_statement = true,
                    b"BeginSession" | b"BeginGetSessionToken" => is_begin_session = true,
                    b"Execute" => is_execute = true,
                    b"PROPERTY_TYPE" => in_property_type = true,
                    _ => (),
                }
            }
            Ok(Event::Empty(ref e)) => {
                match e.local_name().as_ref() {
                    b"Execute" => is_execute = true,
                    _ => (),
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().trim().to_string();

                if !text.is_empty() {
                    if in_request_type {
                        parsed_request_type = text;
                    } else if in_property_name {
                        requested_properties.push(text);
                    } else if in_statement {
                        statement_text = text;
                    } else if in_property_type {
                        if let Ok(v) = text.parse::<i32>() {
                            property_type = Some(v);
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                match e.local_name().as_ref() {
                    b"RequestType" => in_request_type = false,
                    b"PropertyName" => in_property_name = false,
                    b"Statement" => in_statement = false,
                    b"PROPERTY_TYPE" => in_property_type = false,
                    _ => (),
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => (),
        }
    }

    match parsed_request_type.as_str() {
        "DISCOVER_PROPERTIES" => {
            return XmlaRequest::DiscoverProperties {
                property_names: requested_properties,
            }
        }
        "DISCOVER_SCHEMA_ROWSETS" => return XmlaRequest::DiscoverSchemaRowsets,
        "DISCOVER_LITERALS" => return XmlaRequest::DiscoverLiterals,
        "DBSCHEMA_CATALOGS" => return XmlaRequest::DbSchemaCatalogs,
        "MDSCHEMA_CUBES" => return XmlaRequest::MdschemaCubes,
        "DBSCHEMA_TABLES" => return XmlaRequest::DbschemaTables,
        "MDSCHEMA_DIMENSIONS" => return XmlaRequest::MdschemaDimensions,
        "MDSCHEMA_MEASURES" => return XmlaRequest::MdschemaMeasures,
        "MDSCHEMA_HIERARCHIES" => return XmlaRequest::MdschemaHierarchies,
        "MDSCHEMA_LEVELS" => return XmlaRequest::MdschemaLevels,
        "MDSCHEMA_PROPERTIES" => return XmlaRequest::MdschemaProperties { property_type },
        "MDSCHEMA_MEMBERS" => return XmlaRequest::MdschemaMembers,
        "MDSCHEMA_SETS" => return XmlaRequest::MdschemaSets,
        "MDSCHEMA_KPIS" => return XmlaRequest::MdschemaKpis,
        "MDSCHEMA_MEASUREGROUPS" => return XmlaRequest::MdschemaMeasureGroups,
        "MDSCHEMA_MEASUREGROUP_DIMENSIONS" => return XmlaRequest::MdschemaMeasureGroupDimensions,
        _ => (),
    };

    if is_execute {
        if !statement_text.is_empty() {
            return XmlaRequest::ExecuteStatement(statement_text);
        } else if is_begin_session {
            return XmlaRequest::BeginSession;
        } else {
            return XmlaRequest::ExecuteEmpty;
        }
    }

    XmlaRequest::Unknown
}

// ./src/properties.rs
use crate::response::{discover_rowset_envelope, UUID_TYPE};

struct Property {
    name: &'static str,
    description: &'static str,
    prop_type: &'static str,
    access_type: &'static str,
    is_required: bool,
    value: Option<&'static str>,
}

const PROPERTIES: &[Property] = &[
    Property {
        name: "ProviderName",
        description: "ProviderName",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("Min Riktiga Rust Proxy"),
    },
    Property {
        name: "DbpropMsmdSubqueries",
        description: "DbpropMsmdSubqueries",
        prop_type: "int",
        access_type: "ReadWrite",
        is_required: false,
        value: Some("2"),
    },
    Property {
        name: "DbpropMsmdOptimizeResponse",
        description: "DbpropMsmdOptimizeResponse",
        prop_type: "long",
        access_type: "ReadWrite",
        is_required: false,
        value: Some("0"),
    },
    Property {
        name: "DbpropMsmdActivityID",
        description: "DbpropMsmdActivityID",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: None,
    },
    Property {
        name: "DbpropMsmdCurrentActivityID",
        description: "DbpropMsmdCurrentActivityID",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: None,
    },
    Property {
        name: "ApplicationContext",
        description: "ApplicationContext",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: None,
    },
    Property {
        name: "Catalog",
        description: "Catalog",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: Some("KTH_KEX_MALLOY_CUBE"),
    },
    Property {
        name: "ServerName",
        description: "ServerName",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("rust-proxy"),
    },
    Property {
        name: "ProviderVersion",
        description: "ProviderVersion",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("1.0.0"),
    },
    Property {
        name: "MdpropMdxSubqueries",
        description: "MdpropMdxSubqueries",
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        value: Some("63"),
    },
    Property {
        name: "MdpropMdxDrillFunctions",
        description: "MdpropMdxDrillFunctions",
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        value: Some("7"),
    },
    Property {
        name: "MdpropMdxNamedSets",
        description: "MdpropMdxNamedSets",
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        value: Some("15"),
    },
    Property {
        name: "MdpropMdxDdlExtensions",
        description: "MdpropMdxDdlExtensions",
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        value: Some("23"),
    },
    Property {
        name: "MDXSupport",
        description: "MDXSupport",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("Core"),
    },
];

const PROPERTY_ROW_FIELDS: &str = r#"                <xsd:element sql:field="PropertyName" name="PropertyName" type="xsd:string"/>
                <xsd:element sql:field="PropertyDescription" name="PropertyDescription" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PropertyType" name="PropertyType" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PropertyAccessType" name="PropertyAccessType" type="xsd:string"/>
                <xsd:element sql:field="IsRequired" name="IsRequired" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="Value" name="Value" type="xsd:string" minOccurs="0"/>"#;

fn format_row(p: &Property) -> String {
    format!(
        r#"          <row>
            <PropertyName>{name}</PropertyName>
            <PropertyDescription>{desc}</PropertyDescription>
            <PropertyType>{ptype}</PropertyType>
            <PropertyAccessType>{access}</PropertyAccessType>
            <IsRequired>{req}</IsRequired>
            <Value>{val}</Value>
          </row>"#,
        name = p.name,
        desc = p.description,
        ptype = p.prop_type,
        access = p.access_type,
        req = p.is_required,
        val = p.value.unwrap_or(""),
    )
}

pub fn get_properties_response(filter: &[String]) -> String {
    let filtered: Vec<String> = PROPERTIES
        .iter()
        .filter(|p| filter.is_empty() || filter.iter().any(|f| f == p.name))
        .map(format_row)
        .collect();

    discover_rowset_envelope(UUID_TYPE, PROPERTY_ROW_FIELDS, &filtered.join("\n"))
}

pub fn get_single_property_response(name: &str, value: &str) -> String {
    let row = format!(
        r#"          <row>
            <PropertyName>{name}</PropertyName>
            <PropertyDescription>{name}</PropertyDescription>
            <PropertyType>string</PropertyType>
            <PropertyAccessType>ReadWrite</PropertyAccessType>
            <IsRequired>false</IsRequired>
            <Value>{value}</Value>
          </row>"#,
    );
    discover_rowset_envelope(UUID_TYPE, PROPERTY_ROW_FIELDS, &row)
}

// ./src/response.rs
pub fn wrap_in_soap_envelope(inner_xml: &str) -> String {
    format!(
        r#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Header>
    <Session xmlns="urn:schemas-microsoft-com:xml-analysis" SessionId="RUST-SESSION-456" />
  </soap:Header>
  <soap:Body>
{}
  </soap:Body>
</soap:Envelope>"#,
        inner_xml
    )
}

pub const UUID_TYPE: &str = r#"<xsd:simpleType name="uuid">
              <xsd:restriction base="xsd:string">
                <xsd:pattern value="[0-9a-zA-Z]{8}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{12}"/>
              </xsd:restriction>
            </xsd:simpleType>"#;

pub fn empty_discover_response() -> String {
    let inner = r#"    <DiscoverResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" />
        </root>
      </return>
    </DiscoverResponse>"#;
    wrap_in_soap_envelope(inner)
}

pub fn discover_rowset_envelope(extra_schema: &str, row_fields: &str, rows: &str) -> String {
    let inner = format!(
        r#"    <DiscoverResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence minOccurs="0" maxOccurs="unbounded"><xsd:element name="row" type="row"/></xsd:sequence></xsd:complexType>
            </xsd:element>
{extra_schema}
            <xsd:complexType name="row">
              <xsd:sequence>
{row_fields}
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
{rows}
        </root>
      </return>
    </DiscoverResponse>"#,
    );
    wrap_in_soap_envelope(&inner)
}

// ./src/schema_rowsets.rs
use crate::response::{discover_rowset_envelope, UUID_TYPE};

const SCHEMA_ROW_FIELDS: &str = r#"                <xsd:element sql:field="SchemaName" name="SchemaName" type="xsd:string"/>
                <xsd:element sql:field="SchemaGuid" name="SchemaGuid" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="Restrictions" name="Restrictions" minOccurs="0" maxOccurs="unbounded">
                  <xsd:complexType>
                    <xsd:sequence>
                      <xsd:element sql:field="Name" name="Name" type="xsd:string" minOccurs="0"/>
                      <xsd:element sql:field="Type" name="Type" type="xsd:string" minOccurs="0"/>
                    </xsd:sequence>
                  </xsd:complexType>
                </xsd:element>
                <xsd:element sql:field="RestrictionsMask" name="RestrictionsMask" type="xsd:unsignedLong" minOccurs="0"/>"#;

const SCHEMA_ROWSET_DATA: &str = r#"          <row>
            <SchemaName>DBSCHEMA_CATALOGS</SchemaName>
            <SchemaGuid>C8B52211-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DBSCHEMA_TABLES</SchemaName>
            <SchemaGuid>C8B52229-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>TABLE_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_OLAP_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DBSCHEMA_COLUMNS</SchemaName>
            <SchemaGuid>C8B52214-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>TABLE_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>COLUMN_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>COLUMN_OLAP_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DBSCHEMA_PROVIDER_TYPES</SchemaName>
            <SchemaGuid>C8B5222C-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>DATA_TYPE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>BEST_MATCH</Name><Type>xsd:boolean</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_CUBES</SchemaName>
            <SchemaGuid>C8B522D8-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>BASE_CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_DIMENSIONS</SchemaName>
            <SchemaGuid>C8B522D9-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>DIMENSION_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>127</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_HIERARCHIES</SchemaName>
            <SchemaGuid>C8B522DA-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_ORIGIN</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>511</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_LEVELS</SchemaName>
            <SchemaGuid>C8B522DB-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_ORIGIN</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>LEVEL_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>1023</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_MEASURES</SchemaName>
            <SchemaGuid>C8B522DC-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEASURE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEASURE_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEASUREGROUP_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>MEASURE_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>255</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_PROPERTIES</SchemaName>
            <SchemaGuid>C8B522DD-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEMBER_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>PROPERTY_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>PROPERTY_TYPE</Name><Type>xsd:short</Type></Restrictions>
            <Restrictions><Name>PROPERTY_CONTENT_TYPE</Name><Type>xsd:short</Type></Restrictions>
            <Restrictions><Name>PROPERTY_ORIGIN</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>PROPERTY_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>8191</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_MEMBERS</SchemaName>
            <SchemaGuid>C8B522DE-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_NUMBER</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <Restrictions><Name>MEMBER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEMBER_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEMBER_CAPTION</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEMBER_TYPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>TREE_OP</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>SCOPE</Name><Type>xsd:int</Type></Restrictions>
            <RestrictionsMask>16383</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_FUNCTIONS</SchemaName>
            <SchemaGuid>A07CCD07-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>LIBRARY_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>INTERFACE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>FUNCTION_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ORIGIN</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_ACTIONS</SchemaName>
            <SchemaGuid>A07CCD08-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ACTION_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ACTION_TYPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>COORDINATE</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>COORDINATE_TYPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>INVOCATION</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>511</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_SETS</SchemaName>
            <SchemaGuid>A07CCD0B-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SET_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCOPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>SET_EVALUATION_CONTEXT</Name><Type>xsd:int</Type></Restrictions>
            <RestrictionsMask>255</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_INSTANCES</SchemaName>
            <SchemaGuid>20518699-2474-4C15-9885-0E947EC7A7E3</SchemaGuid>
            <Restrictions><Name>INSTANCE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_KPIS</SchemaName>
            <SchemaGuid>2AE44109-ED3D-4842-B16F-B694D1CB0E3F</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>KPI_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>SCOPE</Name><Type>xsd:int</Type></Restrictions>
            <RestrictionsMask>63</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_MEASUREGROUPS</SchemaName>
            <SchemaGuid>E1625EBF-FA96-42FD-BEA6-DB90ADAFD96B</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEASUREGROUP_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_MEASUREGROUP_DIMENSIONS</SchemaName>
            <SchemaGuid>A07CCD33-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEASUREGROUP_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>63</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_INPUT_DATASOURCES</SchemaName>
            <SchemaGuid>A07CCD32-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DATASOURCE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DATASOURCE_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_SERVICES</SchemaName>
            <SchemaGuid>3ADD8A95-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>SERVICE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SERVICE_TYPE_ID</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_SERVICE_PARAMETERS</SchemaName>
            <SchemaGuid>3ADD8A75-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>SERVICE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>PARAMETER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_FUNCTIONS</SchemaName>
            <SchemaGuid>3ADD8A79-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>SERVICE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>FUNCTION_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_MODEL_CONTENT</SchemaName>
            <SchemaGuid>3ADD8A76-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>MODEL_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ATTRIBUTE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>NODE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>NODE_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>NODE_TYPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>NODE_GUID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>NODE_CAPTION</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TREE_OPERATION</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <RestrictionsMask>1023</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_MODEL_XML</SchemaName>
            <SchemaGuid>4290B2D5-0E9C-4AA7-9369-98C95CFD9D13</SchemaGuid>
            <Restrictions><Name>MODEL_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_MODEL_CONTENT_PMML</SchemaName>
            <SchemaGuid>4290B2D5-0E9C-4AA7-9369-98C95CFD9D13</SchemaGuid>
            <Restrictions><Name>MODEL_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_MODELS</SchemaName>
            <SchemaGuid>3ADD8A77-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>MODEL_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SERVICE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SERVICE_TYPE_ID</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <Restrictions><Name>MINING_STRUCTURE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>127</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_COLUMNS</SchemaName>
            <SchemaGuid>3ADD8A78-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>MODEL_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>COLUMN_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_STRUCTURES</SchemaName>
            <SchemaGuid>883269F3-0CAD-462F-B6F5-E88A72418C4B</SchemaGuid>
            <Restrictions><Name>STRUCTURE_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>STRUCTURE_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>STRUCTURE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>7</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_STRUCTURE_COLUMNS</SchemaName>
            <SchemaGuid>9952E836-BFBF-4D1F-8535-9B67DBD9DDFE</SchemaGuid>
            <Restrictions><Name>STRUCTURE_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>STRUCTURE_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>STRUCTURE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>COLUMN_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_DATASOURCES</SchemaName>
            <SchemaGuid>06C03D41-F66D-49F3-B1B8-987F7AF4CF18</SchemaGuid>
            <Restrictions><Name>DataSourceName</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>URL</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ProviderName</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ProviderType</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>AuthenticationMode</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_PROPERTIES</SchemaName>
            <SchemaGuid>4B40ADFB-8B09-4758-97BB-636E8AE97BCF</SchemaGuid>
            <Restrictions><Name>PropertyName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_SCHEMA_ROWSETS</SchemaName>
            <SchemaGuid>EEA0302B-7922-4992-8991-0E605D0E5593</SchemaGuid>
            <Restrictions><Name>SchemaName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_ENUMERATORS</SchemaName>
            <SchemaGuid>55A9E78B-ACCB-45B4-95A6-94C5065617A7</SchemaGuid>
            <Restrictions><Name>EnumName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_KEYWORDS</SchemaName>
            <SchemaGuid>1426C443-4CDD-4A40-8F45-572FAB9BBAA1</SchemaGuid>
            <Restrictions><Name>Keyword</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_LITERALS</SchemaName>
            <SchemaGuid>C3EF5ECB-0A07-4665-A140-B075722DBDC2</SchemaGuid>
            <Restrictions><Name>LiteralName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XML_METADATA</SchemaName>
            <SchemaGuid>3444B255-171E-4CB9-AD98-19E57888A75F</SchemaGuid>
            <Restrictions><Name>DatabaseID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DimensionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CubeID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MeasureGroupID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>PartitionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>PerspectiveID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DimensionPermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>RoleID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DatabasePermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MiningModelID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MiningModelPermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DataSourceID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MiningStructureID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>AggregationDesignID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TraceID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MiningStructurePermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CubePermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>AssemblyID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MdxScriptID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DataSourceViewID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DataSourcePermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CalculatedColumns</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ObjectExpansion</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DBWorkloadGroupID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ResourcePoolID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ModifiedAfter</Name><Type>xsd:dateTime</Type></Restrictions>
            <RestrictionsMask>67108863</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_TRACES</SchemaName>
            <SchemaGuid>A07CCD1A-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>TraceID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>Type</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_TRACE_DEFINITION_PROVIDERINFO</SchemaName>
            <SchemaGuid>A07CCD1B-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>Data</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XEVENT_PACKAGES</SchemaName>
            <SchemaGuid>A07CCD1C-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ID</Name><Type>uuid</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XEVENT_OBJECTS</SchemaName>
            <SchemaGuid>A07CCD1D-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>OBJECT_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XEVENT_OBJECT_COLUMNS</SchemaName>
            <SchemaGuid>A07CCD1E-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>OBJECT_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XEVENT_SESSION_TARGETS</SchemaName>
            <SchemaGuid>A07CCD1F-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>XESessionName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XEVENT_SESSIONS</SchemaName>
            <SchemaGuid>A07CCD20-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>XESessionName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_TRACE_COLUMNS</SchemaName>
            <SchemaGuid>A07CCD18-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>Data</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_TRACE_EVENT_CATEGORIES</SchemaName>
            <SchemaGuid>A07CCD19-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>Data</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_MEMORYUSAGE</SchemaName>
            <SchemaGuid>A07CCD21-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>SPID</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <Restrictions><Name>MemoryUsed</Name><Type>xsd:long</Type></Restrictions>
            <Restrictions><Name>BaseObjectType</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <Restrictions><Name>Shrinkable</Name><Type>xsd:boolean</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_MEMORYGRANT</SchemaName>
            <SchemaGuid>A07CCD23-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>SPID</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_LOCKS</SchemaName>
            <SchemaGuid>A07CCD24-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>SPID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>LOCK_TRANSACTION_ID</Name><Type>uuid</Type></Restrictions>
            <Restrictions><Name>LOCK_OBJECT_ID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LOCK_STATUS</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>LOCK_TYPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>LOCK_MIN_TOTAL_MS</Name><Type>xsd:long</Type></Restrictions>
            <RestrictionsMask>63</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_CONNECTIONS</SchemaName>
            <SchemaGuid>A07CCD25-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CONNECTION_ID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CONNECTION_USER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CONNECTION_IMPERSONATED_USER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CONNECTION_HOST_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CONNECTION_ELAPSED_TIME_MS</Name><Type>xsd:long</Type></Restrictions>
            <Restrictions><Name>CONNECTION_LAST_COMMAND_ELAPSED_TIME_MS</Name><Type>xsd:long</Type></Restrictions>
            <Restrictions><Name>CONNECTION_IDLE_TIME_MS</Name><Type>xsd:long</Type></Restrictions>
            <RestrictionsMask>127</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_SESSIONS</SchemaName>
            <SchemaGuid>A07CCD26-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>SESSION_ID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SESSION_SPID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>SESSION_CONNECTION_ID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>SESSION_USER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SESSION_CURRENT_DATABASE</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SESSION_ELAPSED_TIME_MS</Name><Type>xsd:unsignedLong</Type></Restrictions>
            <Restrictions><Name>SESSION_CPU_TIME_MS</Name><Type>xsd:unsignedLong</Type></Restrictions>
            <Restrictions><Name>SESSION_IDLE_TIME_MS</Name><Type>xsd:unsignedLong</Type></Restrictions>
            <Restrictions><Name>SESSION_STATUS</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>RESTRICT_CATALOG_ID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>REQUEST_ACTIVITY_ID</Name><Type>uuid</Type></Restrictions>
            <Restrictions><Name>CLIENT_ACTIVITY_ID</Name><Type>uuid</Type></Restrictions>
            <RestrictionsMask>4095</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_JOBS</SchemaName>
            <SchemaGuid>A07CCD27-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>SPID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>JOB_ID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>JOB_DESCRIPTION</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>JOB_THREADPOOL_ID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>JOB_MIN_TOTAL_TIME_MS</Name><Type>xsd:long</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_TRANSACTIONS</SchemaName>
            <SchemaGuid>A07CCD28-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>TRANSACTION_ID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TRANSACTION_SESSION_ID</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_DB_CONNECTIONS</SchemaName>
            <SchemaGuid>A07CCD2A-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CONNECTION_ID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CONNECTION_IN_USE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CONNECTION_SERVER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CONNECTION_CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CONNECTION_SPID</Name><Type>xsd:int</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_MASTER_KEY</SchemaName>
            <SchemaGuid>A07CCD29-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>KEY</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
"#;

pub fn get_schemas_response() -> String {
    discover_rowset_envelope(UUID_TYPE, SCHEMA_ROW_FIELDS, SCHEMA_ROWSET_DATA)
}

// ./src/sets.rs
use crate::response::discover_rowset_envelope;

const SETS_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="SET_NAME" name="SET_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCOPE" name="SCOPE" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="EXPRESSION" name="EXPRESSION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DIMENSIONS" name="DIMENSIONS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="SET_CAPTION" name="SET_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="SET_DISPLAY_FOLDER" name="SET_DISPLAY_FOLDER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="SET_EVALUATION_CONTEXT" name="SET_EVALUATION_CONTEXT" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="SET_IS_VISIBLE" name="SET_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>"#;

pub fn get_sets_response() -> String {
    discover_rowset_envelope("", SETS_ROW_FIELDS, "")
}

// ./src/tables.rs
use crate::response::{discover_rowset_envelope, UUID_TYPE};

const TABLE_ROW_FIELDS: &str = r#"                <xsd:element sql:field="TABLE_CATALOG" name="TABLE_CATALOG" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_SCHEMA" name="TABLE_SCHEMA" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_NAME" name="TABLE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_TYPE" name="TABLE_TYPE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_GUID" name="TABLE_GUID" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_PROPID" name="TABLE_PROPID" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="DATE_CREATED" name="DATE_CREATED" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="DATE_MODIFIED" name="DATE_MODIFIED" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="TABLE_OLAP_TYPE" name="TABLE_OLAP_TYPE" type="xsd:string" minOccurs="0"/>"#;

const TABLE_ROWS: &str = r#"          <row>
            <TABLE_CATALOG>KTH_KEX_MALLOY_CUBE</TABLE_CATALOG>
            <TABLE_SCHEMA>Model</TABLE_SCHEMA>
            <TABLE_NAME>Faktatabell</TABLE_NAME>
            <TABLE_TYPE>SYSTEM TABLE</TABLE_TYPE>
            <TABLE_OLAP_TYPE>MEASURE_GROUP</TABLE_OLAP_TYPE>
          </row>
          <row>
            <TABLE_CATALOG>KTH_KEX_MALLOY_CUBE</TABLE_CATALOG>
            <TABLE_SCHEMA>Model</TABLE_SCHEMA>
            <TABLE_NAME>Produktkategori</TABLE_NAME>
            <TABLE_TYPE>TABLE</TABLE_TYPE>
            <TABLE_OLAP_TYPE>CUBE_DIMENSION</TABLE_OLAP_TYPE>
          </row>"#;

pub fn get_tables_response() -> String {
    discover_rowset_envelope(UUID_TYPE, TABLE_ROW_FIELDS, TABLE_ROWS)
}


// ./src/catalogs.rs
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

// ./src/cubes.rs
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
            <CUBE_NAME>Model</CUBE_NAME>
            <CUBE_TYPE>CUBE</CUBE_TYPE>
            <LAST_SCHEMA_UPDATE>2026-05-20T12:00:00.000000</LAST_SCHEMA_UPDATE>
            <LAST_DATA_UPDATE>2026-05-20T12:00:00.000000</LAST_DATA_UPDATE>
            <DESCRIPTION>Byggt med Rust och DuckDB!</DESCRIPTION>
            <IS_DRILLTHROUGH_ENABLED>true</IS_DRILLTHROUGH_ENABLED>
            <IS_LINKABLE>false</IS_LINKABLE>
            <IS_WRITE_ENABLED>false</IS_WRITE_ENABLED>
            <IS_SQL_ENABLED>false</IS_SQL_ENABLED>
            <CUBE_CAPTION>Malloy Analytics Cube</CUBE_CAPTION>
            <CUBE_SOURCE>1</CUBE_SOURCE>
            <PREFERRED_QUERY_PATTERNS>3</PREFERRED_QUERY_PATTERNS>
          </row>"#;

pub fn get_cubes_response() -> String {
    discover_rowset_envelope(UUID_TYPE, CUBE_ROW_FIELDS, CUBE_ROWS)
}

// ./src/dimensions.rs
use crate::response::discover_rowset_envelope;

const DIM_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_NAME" name="DIMENSION_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_CAPTION" name="DIMENSION_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_ORDINAL" name="DIMENSION_ORDINAL" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_TYPE" name="DIMENSION_TYPE" type="xsd:short" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_CARDINALITY" name="DIMENSION_CARDINALITY" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DEFAULT_HIERARCHY" name="DEFAULT_HIERARCHY" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="IS_VIRTUAL" name="IS_VIRTUAL" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="IS_READWRITE" name="IS_READWRITE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_SETTINGS" name="DIMENSION_UNIQUE_SETTINGS" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_IS_VISIBLE" name="DIMENSION_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>"#;

const DIM_ROWS: &str = r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_NAME>Measures</DIMENSION_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_CAPTION>Measures</DIMENSION_CAPTION>
            <DIMENSION_ORDINAL>0</DIMENSION_ORDINAL>
            <DIMENSION_TYPE>2</DIMENSION_TYPE>
            <DIMENSION_CARDINALITY>1</DIMENSION_CARDINALITY>
            <DEFAULT_HIERARCHY>[Measures]</DEFAULT_HIERARCHY>
            <DESCRIPTION>Detta är mätvärdena</DESCRIPTION>
            <IS_VIRTUAL>false</IS_VIRTUAL>
            <IS_READWRITE>false</IS_READWRITE>
            <DIMENSION_UNIQUE_SETTINGS>0</DIMENSION_UNIQUE_SETTINGS>
            <DIMENSION_IS_VISIBLE>false</DIMENSION_IS_VISIBLE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_NAME>Produktkategori</DIMENSION_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_CAPTION>Produktkategori</DIMENSION_CAPTION>
            <DIMENSION_ORDINAL>1</DIMENSION_ORDINAL>
            <DIMENSION_TYPE>0</DIMENSION_TYPE>
            <DIMENSION_CARDINALITY>50</DIMENSION_CARDINALITY>
            <DEFAULT_HIERARCHY>[Produktkategori]</DEFAULT_HIERARCHY>
            <DESCRIPTION>Våra olika produkter</DESCRIPTION>
            <IS_VIRTUAL>false</IS_VIRTUAL>
            <IS_READWRITE>false</IS_READWRITE>
            <DIMENSION_UNIQUE_SETTINGS>0</DIMENSION_UNIQUE_SETTINGS>
            <DIMENSION_IS_VISIBLE>true</DIMENSION_IS_VISIBLE>
          </row>"#;

pub fn get_dimensions_response() -> String {
    discover_rowset_envelope("", DIM_ROW_FIELDS, DIM_ROWS)
}

// ./src/execute.rs
use crate::response::wrap_in_soap_envelope;

pub fn get_empty_execute_response() -> String {
    wrap_in_soap_envelope(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:empty"/>
      </return>
    </ExecuteResponse>"#,
    )
}

pub fn get_execute_statement_response(mdx: &str) -> String {
    let has_measures = mdx.contains("Measures") || mdx.contains("measures");
    let measure_name = "Total_Försäljning";
    let measure_value = if has_measures { "1250000.5" } else { "" };

    let inner = format!(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence minOccurs="0" maxOccurs="unbounded"><xsd:element name="row" type="row"/></xsd:sequence></xsd:complexType>
            </xsd:element>
            <xsd:complexType name="row">
              <xsd:sequence>
                <xsd:element sql:field="{name}" name="{name}" type="xsd:double" minOccurs="0"/>
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
          <row>
            <{name}>{val}</{name}>
          </row>
        </root>
      </return>
    </ExecuteResponse>"#,
        name = measure_name,
        val = measure_value,
    );
    wrap_in_soap_envelope(&inner)
}

// ./src/hierarchies.rs
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

// ./src/kpis.rs
use crate::response::discover_rowset_envelope;

const KPIS_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="KPI_NAME" name="KPI_NAME" type="xsd:string"/>
                <xsd:element sql:field="KPI_CAPTION" name="KPI_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_DESCRIPTION" name="KPI_DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_DISPLAY_FOLDER" name="KPI_DISPLAY_FOLDER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_VALUE" name="KPI_VALUE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_GOAL" name="KPI_GOAL" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_STATUS" name="KPI_STATUS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_TREND" name="KPI_TREND" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_STATUS_GRAPHIC" name="KPI_STATUS_GRAPHIC" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_TREND_GRAPHIC" name="KPI_TREND_GRAPHIC" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_WEIGHT" name="KPI_WEIGHT" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_CURRENT_TIME_MEMBER" name="KPI_CURRENT_TIME_MEMBER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_PARENT_KPI_NAME" name="KPI_PARENT_KPI_NAME" type="xsd:string" minOccurs="0"/>"#;

pub fn get_kpis_response() -> String {
    discover_rowset_envelope("", KPIS_ROW_FIELDS, "")
}

// ./src/levels.rs
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

// ./src/literals.rs
use crate::response::discover_rowset_envelope;

const LITERAL_ROW_FIELDS: &str = r#"                <xsd:element sql:field="LITERAL_NAME" name="LITERAL_NAME" type="xsd:string"/>
                <xsd:element sql:field="LITERAL_VALUE" name="LITERAL_VALUE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LITERAL_INVALID_CHARS" name="LITERAL_INVALID_CHARS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LITERAL_INVALID_STARTING_CHARS" name="LITERAL_INVALID_STARTING_CHARS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LITERAL_MAX_LENGTH" name="LITERAL_MAX_LENGTH" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="LITERAL_SUFFIX" name="LITERAL_SUFFIX" type="xsd:string" minOccurs="0"/>"#;

const LITERAL_ROWS: &str = r#"          <row><LITERAL_NAME>DBLITERAL_CATALOG_NAME</LITERAL_NAME><LITERAL_VALUE>KTH_KEX_MALLOY_CUBE</LITERAL_VALUE><LITERAL_MAX_LENGTH>128</LITERAL_MAX_LENGTH></row>
          <row><LITERAL_NAME>DBLITERAL_CATALOG_SEPARATOR</LITERAL_NAME><LITERAL_VALUE>.</LITERAL_VALUE></row>
          <row><LITERAL_NAME>DBLITERAL_QUOTE_PREFIX</LITERAL_NAME><LITERAL_VALUE>[</LITERAL_VALUE></row>
          <row><LITERAL_NAME>DBLITERAL_QUOTE_SUFFIX</LITERAL_NAME><LITERAL_VALUE>]</LITERAL_VALUE></row>
          <row><LITERAL_NAME>DBLITERAL_PASS_THROUGH_COLUMNS</LITERAL_NAME><LITERAL_VALUE>true</LITERAL_VALUE></row>"#;

pub fn get_literals_response() -> String {
    discover_rowset_envelope("", LITERAL_ROW_FIELDS, LITERAL_ROWS)
}

// ./src/main.rs
use axum::{
    http::{header, HeaderMap, HeaderName, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use std::net::SocketAddr;

mod parser;
mod response;
mod properties;
mod schema_rowsets;
mod catalogs;
mod cubes;
mod tables;
mod dimensions;
mod measures;
mod hierarchies;
mod levels;
mod mdschema_properties;
mod members;
mod literals;
mod sets;
mod kpis;
mod measure_groups;
mod measuregroup_dimensions;
mod execute;

use parser::{parse_xmla, XmlaRequest};

#[tokio::main]
async fn main() {
    let app = Router::new().route("/xmla", post(handle_xmla));
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("🚀 Rust-XMLA Proxy (v3 - ModuleRefactor) snurrar på http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "text/xml; charset=utf-8".parse().unwrap());
    headers.insert(header::SERVER, "Rust-Malloy-Proxy/2.0".parse().unwrap());
    headers.insert(header::CONNECTION, "close".parse().unwrap());
    headers.insert(
        HeaderName::from_static("x-transport-caps-negotiation-flags"),
        "0,0,0,0,0".parse().unwrap(),
    );
    headers
}

async fn handle_xmla(body: String) -> impl IntoResponse {
    if body.contains("<RequestType>") {
        let req_start = body.find("<RequestType>").unwrap() + 13;
        let req_end = body.find("</RequestType>").unwrap();
        println!("🔍 Rå RequestType från Excel: {}", &body[req_start..req_end]);
    }

    let headers = default_headers();
    let request = parse_xmla(&body);
    println!("📥 Fick anrop, tolkade som: {:?}", request);

    if body.contains("<Execute") {
        println!("🔍 Rå Execute från Excel:\n{}", body);
    }

    let response_body = match request {
        XmlaRequest::BeginSession | XmlaRequest::ExecuteEmpty => {
            execute::get_empty_execute_response()
        }

        XmlaRequest::DiscoverProperties { property_names } => {
            if property_names.len() == 1 && property_names[0] == "Catalog" {
                println!("Excel frågar efter Catalog");
                properties::get_single_property_response("Catalog", "KTH_KEX_MALLOY_CUBE")
            } else {
                println!("Excel frågar efter egenskaper: {:?}", property_names);
                properties::get_properties_response(&property_names)
            }
        }

        XmlaRequest::DiscoverSchemaRowsets => schema_rowsets::get_schemas_response(),
        XmlaRequest::DbSchemaCatalogs => catalogs::get_catalogs_response(),
        XmlaRequest::MdschemaCubes => cubes::get_cubes_response(),
        XmlaRequest::DbschemaTables => tables::get_tables_response(),
        XmlaRequest::MdschemaDimensions => {
            println!("📥 Skickar Dimensioner till Excel!");
            dimensions::get_dimensions_response()
        }
        XmlaRequest::MdschemaMeasures => {
            println!("📥 Skickar Measures till Excel!");
            measures::get_measures_response()
        }
        XmlaRequest::MdschemaHierarchies => {
            println!("📥 Hierarchies");
            hierarchies::get_hierarchies_response()
        }
        XmlaRequest::MdschemaLevels => {
            println!("📥 Levels");
            levels::get_levels_response()
        }
        XmlaRequest::ExecuteStatement(mdx) => {
            println!("📥 MDX Statement: {}", mdx);
            execute::get_execute_statement_response(&mdx)
        }
        XmlaRequest::MdschemaProperties { property_type } => {
            println!("📥 MDSCHEMA_PROPERTIES (PROPERTY_TYPE={:?})", property_type);
            mdschema_properties::get_mdschema_properties_response(property_type)
        }
        XmlaRequest::MdschemaMembers => {
            println!("📥 MDSCHEMA_MEMBERS");
            members::get_members_response()
        }
        XmlaRequest::DiscoverLiterals => {
            println!("📥 DISCOVER_LITERALS");
            literals::get_literals_response()
        }
        XmlaRequest::MdschemaSets => {
            println!("📥 MDSCHEMA_SETS");
            sets::get_sets_response()
        }
        XmlaRequest::MdschemaKpis => {
            println!("📥 MDSCHEMA_KPIS");
            kpis::get_kpis_response()
        }
        XmlaRequest::MdschemaMeasureGroups => {
            println!("📥 MDSCHEMA_MEASUREGROUPS");
            measure_groups::get_measure_groups_response()
        }
        XmlaRequest::MdschemaMeasureGroupDimensions => {
            println!("📥 MDSCHEMA_MEASUREGROUP_DIMENSIONS");
            measuregroup_dimensions::get_measuregroup_dimensions_response()
        }

        XmlaRequest::Unknown => {
            println!("❌ Okänt anrop.");
            return (StatusCode::BAD_REQUEST, headers, "Okänt anrop".to_string());
        }
    };

    (StatusCode::OK, headers, response_body)
}

// ./src/mdschema_properties.rs
use crate::response::discover_rowset_envelope;

const PROPERTIES_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="HIERARCHY_UNIQUE_NAME" name="HIERARCHY_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_UNIQUE_NAME" name="LEVEL_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEMBER_UNIQUE_NAME" name="MEMBER_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_NAME" name="PROPERTY_NAME" type="xsd:string"/>
                <xsd:element sql:field="PROPERTY_CAPTION" name="PROPERTY_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_DESCRIPTION" name="PROPERTY_DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_TYPE" name="PROPERTY_TYPE" type="xsd:short" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_CONTENT_TYPE" name="PROPERTY_CONTENT_TYPE" type="xsd:short" minOccurs="0"/>"#;

fn member_property_rows() -> &'static str {
    r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_CAPTION</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_CAPTION</PROPERTY_CAPTION>
            <PROPERTY_TYPE>1</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_KEY</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_KEY</PROPERTY_CAPTION>
            <PROPERTY_TYPE>1</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>1</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>"#
}

fn system_property_rows() -> &'static str {
    r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>FORMATTED_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>FORMATTED_VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>1</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>FORMAT_STRING</PROPERTY_NAME>
            <PROPERTY_CAPTION>FORMAT_STRING</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>2</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>FORE_COLOR</PROPERTY_NAME>
            <PROPERTY_CAPTION>FORE_COLOR</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>2</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>BACK_COLOR</PROPERTY_NAME>
            <PROPERTY_CAPTION>BACK_COLOR</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>2</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>FONT_NAME</PROPERTY_NAME>
            <PROPERTY_CAPTION>FONT_NAME</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>2</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>FONT_SIZE</PROPERTY_NAME>
            <PROPERTY_CAPTION>FONT_SIZE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>2</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>CELL_ORDINAL</PROPERTY_NAME>
            <PROPERTY_CAPTION>CELL_ORDINAL</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>"#
}

fn member_value_rows() -> &'static str {
    r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Measures]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>5</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[(All)]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>5</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>5</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>"#
}

pub fn get_mdschema_properties_response(property_type: Option<i32>) -> String {
    let rows = match property_type {
        Some(1) => member_property_rows().to_string(),
        Some(2) => system_property_rows().to_string(),
        Some(5) => member_value_rows().to_string(),
        _ => format!("{}\n{}", system_property_rows(), member_value_rows()),
    };
    discover_rowset_envelope("", PROPERTIES_ROW_FIELDS, &rows)
}

// ./src/measure_groups.rs
use crate::response::discover_rowset_envelope;

const MEASUREGROUP_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASUREGROUP_NAME" name="MEASUREGROUP_NAME" type="xsd:string"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="IS_WRITE_ENABLED" name="IS_WRITE_ENABLED" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="MEASUREGROUP_CAPTION" name="MEASUREGROUP_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASUREGROUP_CARDINALITY" name="MEASUREGROUP_CARDINALITY" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="MEASUREGROUP_SIZE" name="MEASUREGROUP_SIZE" type="xsd:long" minOccurs="0"/>"#;

pub fn get_measure_groups_response() -> String {
    discover_rowset_envelope("", MEASUREGROUP_ROW_FIELDS,
        r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <MEASUREGROUP_NAME>Faktatabell</MEASUREGROUP_NAME>
            <MEASUREGROUP_CAPTION>Faktatabell</MEASUREGROUP_CAPTION>
          </row>"#
    )
}

// ./src/measuregroup_dimensions.rs
use crate::response::discover_rowset_envelope;

const MG_DIM_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASUREGROUP_NAME" name="MEASUREGROUP_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASUREGROUP_CARDINALITY" name="MEASUREGROUP_CARDINALITY" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_CARDINALITY" name="DIMENSION_CARDINALITY" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_IS_VISIBLE" name="DIMENSION_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_IS_FACT_DIMENSION" name="DIMENSION_IS_FACT_DIMENSION" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_GRANULARITY" name="DIMENSION_GRANULARITY" type="xsd:string" minOccurs="0"/>"#;

pub fn get_measuregroup_dimensions_response() -> String {
    discover_rowset_envelope("", MG_DIM_ROW_FIELDS,
        r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <MEASUREGROUP_NAME>Faktatabell</MEASUREGROUP_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_IS_VISIBLE>true</DIMENSION_IS_VISIBLE>
            <DIMENSION_IS_FACT_DIMENSION>false</DIMENSION_IS_FACT_DIMENSION>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <MEASUREGROUP_NAME>Faktatabell</MEASUREGROUP_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_IS_VISIBLE>true</DIMENSION_IS_VISIBLE>
            <DIMENSION_IS_FACT_DIMENSION>false</DIMENSION_IS_FACT_DIMENSION>
          </row>"#
    )
}

// ./src/measures.rs
use crate::response::{discover_rowset_envelope, UUID_TYPE};

const MEASURE_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASURE_NAME" name="MEASURE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASURE_UNIQUE_NAME" name="MEASURE_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASURE_CAPTION" name="MEASURE_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_GUID" name="MEASURE_GUID" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_AGGREGATOR" name="MEASURE_AGGREGATOR" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DATA_TYPE" name="DATA_TYPE" type="xsd:unsignedShort" minOccurs="0"/>
                <xsd:element sql:field="NUMERIC_PRECISION" name="NUMERIC_PRECISION" type="xsd:unsignedShort" minOccurs="0"/>
                <xsd:element sql:field="NUMERIC_SCALE" name="NUMERIC_SCALE" type="xsd:short" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_UNITS" name="MEASURE_UNITS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="EXPRESSION" name="EXPRESSION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_IS_VISIBLE" name="MEASURE_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="LEVELS_LIST" name="LEVELS_LIST" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_NAME_SQL_COLUMN_NAME" name="MEASURE_NAME_SQL_COLUMN_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_UNQUALIFIED_CAPTION" name="MEASURE_UNQUALIFIED_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASUREGROUP_NAME" name="MEASUREGROUP_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_DISPLAY_FOLDER" name="MEASURE_DISPLAY_FOLDER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DEFAULT_FORMAT_STRING" name="DEFAULT_FORMAT_STRING" type="xsd:string" minOccurs="0"/>"#;

const MEASURE_ROWS: &str = r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <MEASURE_NAME>Total Försäljning</MEASURE_NAME>
            <MEASURE_UNIQUE_NAME>[Measures].[Total Försäljning]</MEASURE_UNIQUE_NAME>
            <MEASURE_CAPTION>Total Försäljning (SEK)</MEASURE_CAPTION>
            <MEASURE_AGGREGATOR>1</MEASURE_AGGREGATOR>
            <DATA_TYPE>5</DATA_TYPE>
            <NUMERIC_PRECISION>18</NUMERIC_PRECISION>
            <NUMERIC_SCALE>2</NUMERIC_SCALE>
            <MEASURE_UNITS>SEK</MEASURE_UNITS>
            <DESCRIPTION>Vår totala försäljning</DESCRIPTION>
            <MEASURE_IS_VISIBLE>true</MEASURE_IS_VISIBLE>
            <MEASURE_UNQUALIFIED_CAPTION>Total Försäljning (SEK)</MEASURE_UNQUALIFIED_CAPTION>
            <MEASUREGROUP_NAME>Faktatabell</MEASUREGROUP_NAME>
            <DEFAULT_FORMAT_STRING>#,##0.00 SEK</DEFAULT_FORMAT_STRING>
          </row>"#;

pub fn get_measures_response() -> String {
    discover_rowset_envelope(UUID_TYPE, MEASURE_ROW_FIELDS, MEASURE_ROWS)
}

// ./src/members.rs
use crate::response::discover_rowset_envelope;

const MEMBER_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="HIERARCHY_UNIQUE_NAME" name="HIERARCHY_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="LEVEL_UNIQUE_NAME" name="LEVEL_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="LEVEL_NUMBER" name="LEVEL_NUMBER" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="MEMBER_ORDINAL" name="MEMBER_ORDINAL" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="MEMBER_NAME" name="MEMBER_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEMBER_UNIQUE_NAME" name="MEMBER_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEMBER_TYPE" name="MEMBER_TYPE" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="MEMBER_CAPTION" name="MEMBER_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CHILDREN_CARDINALITY" name="CHILDREN_CARDINALITY" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="PARENT_LEVEL" name="PARENT_LEVEL" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="PARENT_COUNT" name="PARENT_COUNT" type="xsd:unsignedInt" minOccurs="0"/>"#;

const MEMBER_ROWS: &str = r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Measures]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>0</LEVEL_NUMBER>
            <MEMBER_ORDINAL>0</MEMBER_ORDINAL>
            <MEMBER_NAME>Total Försäljning</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>[Measures].[Total Försäljning]</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>2</MEMBER_TYPE>
            <MEMBER_CAPTION>Total Försäljning</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>0</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>0</PARENT_LEVEL>
            <PARENT_COUNT>0</PARENT_COUNT>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[(All)]</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>0</LEVEL_NUMBER>
            <MEMBER_ORDINAL>0</MEMBER_ORDINAL>
            <MEMBER_NAME>All Produktkategorier</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>[Produktkategori].[All Produktkategorier]</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>1</MEMBER_TYPE>
            <MEMBER_CAPTION>All Produktkategorier</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>50</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>0</PARENT_LEVEL>
            <PARENT_COUNT>0</PARENT_COUNT>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>1</LEVEL_NUMBER>
            <MEMBER_ORDINAL>1</MEMBER_ORDINAL>
            <MEMBER_NAME>Kategori A</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>[Produktkategori].[Kategori A]</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>3</MEMBER_TYPE>
            <MEMBER_CAPTION>Kategori A</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>0</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>0</PARENT_LEVEL>
            <PARENT_COUNT>1</PARENT_COUNT>
          </row>"#;

pub fn get_members_response() -> String {
    discover_rowset_envelope("", MEMBER_ROW_FIELDS, MEMBER_ROWS)
}

// ./src/parser.rs
use quick_xml::events::Event;
use quick_xml::Reader;

#[derive(Debug, PartialEq)]
pub enum XmlaRequest {
    DiscoverProperties { property_names: Vec<String> },
    DiscoverSchemaRowsets,
    DiscoverLiterals,
    DbSchemaCatalogs,
    MdschemaCubes,
    DbschemaTables,
    MdschemaDimensions,
    MdschemaMeasures,
    MdschemaHierarchies,
    MdschemaLevels,
    MdschemaProperties { property_type: Option<i32> },
    MdschemaMembers,
    MdschemaSets,
    MdschemaKpis,
    MdschemaMeasureGroups,
    MdschemaMeasureGroupDimensions,
    BeginSession,
    ExecuteEmpty,
    ExecuteStatement(String),
    Unknown,
}

pub fn parse_xmla(xml: &str) -> XmlaRequest {
    let mut reader = Reader::from_str(xml);

    let mut in_request_type = false;
    let mut is_execute = false;
    let mut in_property_name = false;
    let mut in_statement = false;
    let mut is_begin_session = false;
    let mut in_property_type = false;

    let mut parsed_request_type = String::new();
    let mut requested_properties: Vec<String> = Vec::new();
    let mut statement_text = String::new();
    let mut property_type: Option<i32> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                match e.local_name().as_ref() {
                    b"RequestType" => in_request_type = true,
                    b"PropertyName" => in_property_name = true,
                    b"Statement" => in_statement = true,
                    b"BeginSession" | b"BeginGetSessionToken" => is_begin_session = true,
                    b"Execute" => is_execute = true,
                    b"PROPERTY_TYPE" => in_property_type = true,
                    _ => (),
                }
            }
            Ok(Event::Empty(ref e)) => {
                match e.local_name().as_ref() {
                    b"Execute" => is_execute = true,
                    _ => (),
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().trim().to_string();

                if !text.is_empty() {
                    if in_request_type {
                        parsed_request_type = text;
                    } else if in_property_name {
                        requested_properties.push(text);
                    } else if in_statement {
                        statement_text = text;
                    } else if in_property_type {
                        if let Ok(v) = text.parse::<i32>() {
                            property_type = Some(v);
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                match e.local_name().as_ref() {
                    b"RequestType" => in_request_type = false,
                    b"PropertyName" => in_property_name = false,
                    b"Statement" => in_statement = false,
                    b"PROPERTY_TYPE" => in_property_type = false,
                    _ => (),
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => (),
        }
    }

    match parsed_request_type.as_str() {
        "DISCOVER_PROPERTIES" => {
            return XmlaRequest::DiscoverProperties {
                property_names: requested_properties,
            }
        }
        "DISCOVER_SCHEMA_ROWSETS" => return XmlaRequest::DiscoverSchemaRowsets,
        "DISCOVER_LITERALS" => return XmlaRequest::DiscoverLiterals,
        "DBSCHEMA_CATALOGS" => return XmlaRequest::DbSchemaCatalogs,
        "MDSCHEMA_CUBES" => return XmlaRequest::MdschemaCubes,
        "DBSCHEMA_TABLES" => return XmlaRequest::DbschemaTables,
        "MDSCHEMA_DIMENSIONS" => return XmlaRequest::MdschemaDimensions,
        "MDSCHEMA_MEASURES" => return XmlaRequest::MdschemaMeasures,
        "MDSCHEMA_HIERARCHIES" => return XmlaRequest::MdschemaHierarchies,
        "MDSCHEMA_LEVELS" => return XmlaRequest::MdschemaLevels,
        "MDSCHEMA_PROPERTIES" => return XmlaRequest::MdschemaProperties { property_type },
        "MDSCHEMA_MEMBERS" => return XmlaRequest::MdschemaMembers,
        "MDSCHEMA_SETS" => return XmlaRequest::MdschemaSets,
        "MDSCHEMA_KPIS" => return XmlaRequest::MdschemaKpis,
        "MDSCHEMA_MEASUREGROUPS" => return XmlaRequest::MdschemaMeasureGroups,
        "MDSCHEMA_MEASUREGROUP_DIMENSIONS" => return XmlaRequest::MdschemaMeasureGroupDimensions,
        _ => (),
    };

    if is_execute {
        if !statement_text.is_empty() {
            return XmlaRequest::ExecuteStatement(statement_text);
        } else if is_begin_session {
            return XmlaRequest::BeginSession;
        } else {
            return XmlaRequest::ExecuteEmpty;
        }
    }

    XmlaRequest::Unknown
}

// ./src/properties.rs
use crate::response::{discover_rowset_envelope, UUID_TYPE};

struct Property {
    name: &'static str,
    description: &'static str,
    prop_type: &'static str,
    access_type: &'static str,
    is_required: bool,
    value: Option<&'static str>,
}

const PROPERTIES: &[Property] = &[
    Property {
        name: "ProviderName",
        description: "ProviderName",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("Min Riktiga Rust Proxy"),
    },
    Property {
        name: "DbpropMsmdSubqueries",
        description: "DbpropMsmdSubqueries",
        prop_type: "int",
        access_type: "ReadWrite",
        is_required: false,
        value: Some("2"),
    },
    Property {
        name: "DbpropMsmdOptimizeResponse",
        description: "DbpropMsmdOptimizeResponse",
        prop_type: "long",
        access_type: "ReadWrite",
        is_required: false,
        value: Some("0"),
    },
    Property {
        name: "DbpropMsmdActivityID",
        description: "DbpropMsmdActivityID",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: None,
    },
    Property {
        name: "DbpropMsmdCurrentActivityID",
        description: "DbpropMsmdCurrentActivityID",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: None,
    },
    Property {
        name: "ApplicationContext",
        description: "ApplicationContext",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: None,
    },
    Property {
        name: "Catalog",
        description: "Catalog",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: Some("KTH_KEX_MALLOY_CUBE"),
    },
    Property {
        name: "ServerName",
        description: "ServerName",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("rust-proxy"),
    },
    Property {
        name: "ProviderVersion",
        description: "ProviderVersion",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("1.0.0"),
    },
    Property {
        name: "MdpropMdxSubqueries",
        description: "MdpropMdxSubqueries",
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        value: Some("63"),
    },
    Property {
        name: "MdpropMdxDrillFunctions",
        description: "MdpropMdxDrillFunctions",
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        value: Some("7"),
    },
    Property {
        name: "MdpropMdxNamedSets",
        description: "MdpropMdxNamedSets",
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        value: Some("15"),
    },
    Property {
        name: "MdpropMdxDdlExtensions",
        description: "MdpropMdxDdlExtensions",
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        value: Some("23"),
    },
    Property {
        name: "MDXSupport",
        description: "MDXSupport",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("Core"),
    },
];

const PROPERTY_ROW_FIELDS: &str = r#"                <xsd:element sql:field="PropertyName" name="PropertyName" type="xsd:string"/>
                <xsd:element sql:field="PropertyDescription" name="PropertyDescription" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PropertyType" name="PropertyType" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PropertyAccessType" name="PropertyAccessType" type="xsd:string"/>
                <xsd:element sql:field="IsRequired" name="IsRequired" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="Value" name="Value" type="xsd:string" minOccurs="0"/>"#;

fn format_row(p: &Property) -> String {
    format!(
        r#"          <row>
            <PropertyName>{name}</PropertyName>
            <PropertyDescription>{desc}</PropertyDescription>
            <PropertyType>{ptype}</PropertyType>
            <PropertyAccessType>{access}</PropertyAccessType>
            <IsRequired>{req}</IsRequired>
            <Value>{val}</Value>
          </row>"#,
        name = p.name,
        desc = p.description,
        ptype = p.prop_type,
        access = p.access_type,
        req = p.is_required,
        val = p.value.unwrap_or(""),
    )
}

pub fn get_properties_response(filter: &[String]) -> String {
    let filtered: Vec<String> = PROPERTIES
        .iter()
        .filter(|p| filter.is_empty() || filter.iter().any(|f| f == p.name))
        .map(format_row)
        .collect();

    discover_rowset_envelope(UUID_TYPE, PROPERTY_ROW_FIELDS, &filtered.join("\n"))
}

pub fn get_single_property_response(name: &str, value: &str) -> String {
    let row = format!(
        r#"          <row>
            <PropertyName>{name}</PropertyName>
            <PropertyDescription>{name}</PropertyDescription>
            <PropertyType>string</PropertyType>
            <PropertyAccessType>ReadWrite</PropertyAccessType>
            <IsRequired>false</IsRequired>
            <Value>{value}</Value>
          </row>"#,
    );
    discover_rowset_envelope(UUID_TYPE, PROPERTY_ROW_FIELDS, &row)
}

// ./src/response.rs
pub fn wrap_in_soap_envelope(inner_xml: &str) -> String {
    format!(
        r#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Header>
    <Session xmlns="urn:schemas-microsoft-com:xml-analysis" SessionId="RUST-SESSION-456" />
  </soap:Header>
  <soap:Body>
{}
  </soap:Body>
</soap:Envelope>"#,
        inner_xml
    )
}

pub const UUID_TYPE: &str = r#"<xsd:simpleType name="uuid">
              <xsd:restriction base="xsd:string">
                <xsd:pattern value="[0-9a-zA-Z]{8}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{12}"/>
              </xsd:restriction>
            </xsd:simpleType>"#;

pub fn empty_discover_response() -> String {
    let inner = r#"    <DiscoverResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" />
        </root>
      </return>
    </DiscoverResponse>"#;
    wrap_in_soap_envelope(inner)
}

pub fn discover_rowset_envelope(extra_schema: &str, row_fields: &str, rows: &str) -> String {
    let inner = format!(
        r#"    <DiscoverResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence minOccurs="0" maxOccurs="unbounded"><xsd:element name="row" type="row"/></xsd:sequence></xsd:complexType>
            </xsd:element>
{extra_schema}
            <xsd:complexType name="row">
              <xsd:sequence>
{row_fields}
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
{rows}
        </root>
      </return>
    </DiscoverResponse>"#,
    );
    wrap_in_soap_envelope(&inner)
}

// ./src/schema_rowsets.rs
use crate::response::{discover_rowset_envelope, UUID_TYPE};

const SCHEMA_ROW_FIELDS: &str = r#"                <xsd:element sql:field="SchemaName" name="SchemaName" type="xsd:string"/>
                <xsd:element sql:field="SchemaGuid" name="SchemaGuid" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="Restrictions" name="Restrictions" minOccurs="0" maxOccurs="unbounded">
                  <xsd:complexType>
                    <xsd:sequence>
                      <xsd:element sql:field="Name" name="Name" type="xsd:string" minOccurs="0"/>
                      <xsd:element sql:field="Type" name="Type" type="xsd:string" minOccurs="0"/>
                    </xsd:sequence>
                  </xsd:complexType>
                </xsd:element>
                <xsd:element sql:field="RestrictionsMask" name="RestrictionsMask" type="xsd:unsignedLong" minOccurs="0"/>"#;

const SCHEMA_ROWSET_DATA: &str = r#"          <row>
            <SchemaName>DBSCHEMA_CATALOGS</SchemaName>
            <SchemaGuid>C8B52211-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DBSCHEMA_TABLES</SchemaName>
            <SchemaGuid>C8B52229-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>TABLE_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_OLAP_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DBSCHEMA_COLUMNS</SchemaName>
            <SchemaGuid>C8B52214-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>TABLE_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>COLUMN_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>COLUMN_OLAP_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DBSCHEMA_PROVIDER_TYPES</SchemaName>
            <SchemaGuid>C8B5222C-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>DATA_TYPE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>BEST_MATCH</Name><Type>xsd:boolean</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_CUBES</SchemaName>
            <SchemaGuid>C8B522D8-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>BASE_CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_DIMENSIONS</SchemaName>
            <SchemaGuid>C8B522D9-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>DIMENSION_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>127</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_HIERARCHIES</SchemaName>
            <SchemaGuid>C8B522DA-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_ORIGIN</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>511</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_LEVELS</SchemaName>
            <SchemaGuid>C8B522DB-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_ORIGIN</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>LEVEL_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>1023</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_MEASURES</SchemaName>
            <SchemaGuid>C8B522DC-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEASURE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEASURE_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEASUREGROUP_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>MEASURE_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>255</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_PROPERTIES</SchemaName>
            <SchemaGuid>C8B522DD-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEMBER_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>PROPERTY_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>PROPERTY_TYPE</Name><Type>xsd:short</Type></Restrictions>
            <Restrictions><Name>PROPERTY_CONTENT_TYPE</Name><Type>xsd:short</Type></Restrictions>
            <Restrictions><Name>PROPERTY_ORIGIN</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>PROPERTY_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>8191</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_MEMBERS</SchemaName>
            <SchemaGuid>C8B522DE-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_NUMBER</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <Restrictions><Name>MEMBER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEMBER_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEMBER_CAPTION</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEMBER_TYPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>TREE_OP</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>SCOPE</Name><Type>xsd:int</Type></Restrictions>
            <RestrictionsMask>16383</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_FUNCTIONS</SchemaName>
            <SchemaGuid>A07CCD07-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>LIBRARY_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>INTERFACE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>FUNCTION_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ORIGIN</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_ACTIONS</SchemaName>
            <SchemaGuid>A07CCD08-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ACTION_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ACTION_TYPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>COORDINATE</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>COORDINATE_TYPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>INVOCATION</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>511</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_SETS</SchemaName>
            <SchemaGuid>A07CCD0B-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SET_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCOPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>SET_EVALUATION_CONTEXT</Name><Type>xsd:int</Type></Restrictions>
            <RestrictionsMask>255</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_INSTANCES</SchemaName>
            <SchemaGuid>20518699-2474-4C15-9885-0E947EC7A7E3</SchemaGuid>
            <Restrictions><Name>INSTANCE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_KPIS</SchemaName>
            <SchemaGuid>2AE44109-ED3D-4842-B16F-B694D1CB0E3F</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>KPI_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>SCOPE</Name><Type>xsd:int</Type></Restrictions>
            <RestrictionsMask>63</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_MEASUREGROUPS</SchemaName>
            <SchemaGuid>E1625EBF-FA96-42FD-BEA6-DB90ADAFD96B</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEASUREGROUP_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_MEASUREGROUP_DIMENSIONS</SchemaName>
            <SchemaGuid>A07CCD33-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEASUREGROUP_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>63</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_INPUT_DATASOURCES</SchemaName>
            <SchemaGuid>A07CCD32-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DATASOURCE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DATASOURCE_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_SERVICES</SchemaName>
            <SchemaGuid>3ADD8A95-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>SERVICE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SERVICE_TYPE_ID</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_SERVICE_PARAMETERS</SchemaName>
            <SchemaGuid>3ADD8A75-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>SERVICE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>PARAMETER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_FUNCTIONS</SchemaName>
            <SchemaGuid>3ADD8A79-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>SERVICE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>FUNCTION_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_MODEL_CONTENT</SchemaName>
            <SchemaGuid>3ADD8A76-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>MODEL_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ATTRIBUTE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>NODE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>NODE_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>NODE_TYPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>NODE_GUID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>NODE_CAPTION</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TREE_OPERATION</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <RestrictionsMask>1023</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_MODEL_XML</SchemaName>
            <SchemaGuid>4290B2D5-0E9C-4AA7-9369-98C95CFD9D13</SchemaGuid>
            <Restrictions><Name>MODEL_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_MODEL_CONTENT_PMML</SchemaName>
            <SchemaGuid>4290B2D5-0E9C-4AA7-9369-98C95CFD9D13</SchemaGuid>
            <Restrictions><Name>MODEL_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_MODELS</SchemaName>
            <SchemaGuid>3ADD8A77-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>MODEL_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SERVICE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SERVICE_TYPE_ID</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <Restrictions><Name>MINING_STRUCTURE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>127</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_COLUMNS</SchemaName>
            <SchemaGuid>3ADD8A78-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>MODEL_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>COLUMN_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_STRUCTURES</SchemaName>
            <SchemaGuid>883269F3-0CAD-462F-B6F5-E88A72418C4B</SchemaGuid>
            <Restrictions><Name>STRUCTURE_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>STRUCTURE_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>STRUCTURE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>7</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_STRUCTURE_COLUMNS</SchemaName>
            <SchemaGuid>9952E836-BFBF-4D1F-8535-9B67DBD9DDFE</SchemaGuid>
            <Restrictions><Name>STRUCTURE_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>STRUCTURE_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>STRUCTURE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>COLUMN_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_DATASOURCES</SchemaName>
            <SchemaGuid>06C03D41-F66D-49F3-B1B8-987F7AF4CF18</SchemaGuid>
            <Restrictions><Name>DataSourceName</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>URL</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ProviderName</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ProviderType</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>AuthenticationMode</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_PROPERTIES</SchemaName>
            <SchemaGuid>4B40ADFB-8B09-4758-97BB-636E8AE97BCF</SchemaGuid>
            <Restrictions><Name>PropertyName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_SCHEMA_ROWSETS</SchemaName>
            <SchemaGuid>EEA0302B-7922-4992-8991-0E605D0E5593</SchemaGuid>
            <Restrictions><Name>SchemaName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_ENUMERATORS</SchemaName>
            <SchemaGuid>55A9E78B-ACCB-45B4-95A6-94C5065617A7</SchemaGuid>
            <Restrictions><Name>EnumName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_KEYWORDS</SchemaName>
            <SchemaGuid>1426C443-4CDD-4A40-8F45-572FAB9BBAA1</SchemaGuid>
            <Restrictions><Name>Keyword</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_LITERALS</SchemaName>
            <SchemaGuid>C3EF5ECB-0A07-4665-A140-B075722DBDC2</SchemaGuid>
            <Restrictions><Name>LiteralName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XML_METADATA</SchemaName>
            <SchemaGuid>3444B255-171E-4CB9-AD98-19E57888A75F</SchemaGuid>
            <Restrictions><Name>DatabaseID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DimensionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CubeID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MeasureGroupID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>PartitionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>PerspectiveID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DimensionPermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>RoleID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DatabasePermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MiningModelID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MiningModelPermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DataSourceID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MiningStructureID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>AggregationDesignID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TraceID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MiningStructurePermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CubePermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>AssemblyID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MdxScriptID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DataSourceViewID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DataSourcePermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CalculatedColumns</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ObjectExpansion</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DBWorkloadGroupID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ResourcePoolID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ModifiedAfter</Name><Type>xsd:dateTime</Type></Restrictions>
            <RestrictionsMask>67108863</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_TRACES</SchemaName>
            <SchemaGuid>A07CCD1A-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>TraceID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>Type</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_TRACE_DEFINITION_PROVIDERINFO</SchemaName>
            <SchemaGuid>A07CCD1B-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>Data</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XEVENT_PACKAGES</SchemaName>
            <SchemaGuid>A07CCD1C-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ID</Name><Type>uuid</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XEVENT_OBJECTS</SchemaName>
            <SchemaGuid>A07CCD1D-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>OBJECT_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XEVENT_OBJECT_COLUMNS</SchemaName>
            <SchemaGuid>A07CCD1E-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>OBJECT_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XEVENT_SESSION_TARGETS</SchemaName>
            <SchemaGuid>A07CCD1F-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>XESessionName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XEVENT_SESSIONS</SchemaName>
            <SchemaGuid>A07CCD20-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>XESessionName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_TRACE_COLUMNS</SchemaName>
            <SchemaGuid>A07CCD18-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>Data</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_TRACE_EVENT_CATEGORIES</SchemaName>
            <SchemaGuid>A07CCD19-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>Data</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_MEMORYUSAGE</SchemaName>
            <SchemaGuid>A07CCD21-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>SPID</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <Restrictions><Name>MemoryUsed</Name><Type>xsd:long</Type></Restrictions>
            <Restrictions><Name>BaseObjectType</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <Restrictions><Name>Shrinkable</Name><Type>xsd:boolean</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_MEMORYGRANT</SchemaName>
            <SchemaGuid>A07CCD23-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>SPID</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_LOCKS</SchemaName>
            <SchemaGuid>A07CCD24-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>SPID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>LOCK_TRANSACTION_ID</Name><Type>uuid</Type></Restrictions>
            <Restrictions><Name>LOCK_OBJECT_ID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LOCK_STATUS</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>LOCK_TYPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>LOCK_MIN_TOTAL_MS</Name><Type>xsd:long</Type></Restrictions>
            <RestrictionsMask>63</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_CONNECTIONS</SchemaName>
            <SchemaGuid>A07CCD25-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CONNECTION_ID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CONNECTION_USER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CONNECTION_IMPERSONATED_USER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CONNECTION_HOST_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CONNECTION_ELAPSED_TIME_MS</Name><Type>xsd:long</Type></Restrictions>
            <Restrictions><Name>CONNECTION_LAST_COMMAND_ELAPSED_TIME_MS</Name><Type>xsd:long</Type></Restrictions>
            <Restrictions><Name>CONNECTION_IDLE_TIME_MS</Name><Type>xsd:long</Type></Restrictions>
            <RestrictionsMask>127</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_SESSIONS</SchemaName>
            <SchemaGuid>A07CCD26-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>SESSION_ID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SESSION_SPID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>SESSION_CONNECTION_ID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>SESSION_USER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SESSION_CURRENT_DATABASE</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SESSION_ELAPSED_TIME_MS</Name><Type>xsd:unsignedLong</Type></Restrictions>
            <Restrictions><Name>SESSION_CPU_TIME_MS</Name><Type>xsd:unsignedLong</Type></Restrictions>
            <Restrictions><Name>SESSION_IDLE_TIME_MS</Name><Type>xsd:unsignedLong</Type></Restrictions>
            <Restrictions><Name>SESSION_STATUS</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>RESTRICT_CATALOG_ID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>REQUEST_ACTIVITY_ID</Name><Type>uuid</Type></Restrictions>
            <Restrictions><Name>CLIENT_ACTIVITY_ID</Name><Type>uuid</Type></Restrictions>
            <RestrictionsMask>4095</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_JOBS</SchemaName>
            <SchemaGuid>A07CCD27-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>SPID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>JOB_ID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>JOB_DESCRIPTION</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>JOB_THREADPOOL_ID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>JOB_MIN_TOTAL_TIME_MS</Name><Type>xsd:long</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_TRANSACTIONS</SchemaName>
            <SchemaGuid>A07CCD28-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>TRANSACTION_ID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TRANSACTION_SESSION_ID</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_DB_CONNECTIONS</SchemaName>
            <SchemaGuid>A07CCD2A-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CONNECTION_ID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CONNECTION_IN_USE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CONNECTION_SERVER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CONNECTION_CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CONNECTION_SPID</Name><Type>xsd:int</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_MASTER_KEY</SchemaName>
            <SchemaGuid>A07CCD29-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>KEY</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
"#;

pub fn get_schemas_response() -> String {
    discover_rowset_envelope(UUID_TYPE, SCHEMA_ROW_FIELDS, SCHEMA_ROWSET_DATA)
}

// ./src/sets.rs
use crate::response::discover_rowset_envelope;

const SETS_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="SET_NAME" name="SET_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCOPE" name="SCOPE" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="EXPRESSION" name="EXPRESSION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DIMENSIONS" name="DIMENSIONS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="SET_CAPTION" name="SET_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="SET_DISPLAY_FOLDER" name="SET_DISPLAY_FOLDER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="SET_EVALUATION_CONTEXT" name="SET_EVALUATION_CONTEXT" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="SET_IS_VISIBLE" name="SET_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>"#;

pub fn get_sets_response() -> String {
    discover_rowset_envelope("", SETS_ROW_FIELDS, "")
}

// ./src/tables.rs
use crate::response::{discover_rowset_envelope, UUID_TYPE};

const TABLE_ROW_FIELDS: &str = r#"                <xsd:element sql:field="TABLE_CATALOG" name="TABLE_CATALOG" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_SCHEMA" name="TABLE_SCHEMA" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_NAME" name="TABLE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_TYPE" name="TABLE_TYPE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_GUID" name="TABLE_GUID" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_PROPID" name="TABLE_PROPID" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="DATE_CREATED" name="DATE_CREATED" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="DATE_MODIFIED" name="DATE_MODIFIED" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="TABLE_OLAP_TYPE" name="TABLE_OLAP_TYPE" type="xsd:string" minOccurs="0"/>"#;

const TABLE_ROWS: &str = r#"          <row>
            <TABLE_CATALOG>KTH_KEX_MALLOY_CUBE</TABLE_CATALOG>
            <TABLE_NAME>Faktatabell</TABLE_NAME>
            <TABLE_TYPE>TABLE</TABLE_TYPE>
            <TABLE_OLAP_TYPE>MEASURE_GROUP</TABLE_OLAP_TYPE>
          </row>
          <row>
            <TABLE_CATALOG>KTH_KEX_MALLOY_CUBE</TABLE_CATALOG>
            <TABLE_NAME>Produktkategori</TABLE_NAME>
            <TABLE_TYPE>TABLE</TABLE_TYPE>
            <TABLE_OLAP_TYPE>CUBE_DIMENSION</TABLE_OLAP_TYPE>
          </row>"#;

pub fn get_tables_response() -> String {
    discover_rowset_envelope(UUID_TYPE, TABLE_ROW_FIELDS, TABLE_ROWS)
}


// ./src/catalogs.rs
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

// ./src/cubes.rs
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
            <CUBE_NAME>Model</CUBE_NAME>
            <CUBE_TYPE>CUBE</CUBE_TYPE>
            <LAST_SCHEMA_UPDATE>2026-05-20T12:00:00.000000</LAST_SCHEMA_UPDATE>
            <LAST_DATA_UPDATE>2026-05-20T12:00:00.000000</LAST_DATA_UPDATE>
            <DESCRIPTION>Byggt med Rust och DuckDB!</DESCRIPTION>
            <IS_DRILLTHROUGH_ENABLED>true</IS_DRILLTHROUGH_ENABLED>
            <IS_LINKABLE>false</IS_LINKABLE>
            <IS_WRITE_ENABLED>false</IS_WRITE_ENABLED>
            <IS_SQL_ENABLED>false</IS_SQL_ENABLED>
            <CUBE_CAPTION>Malloy Analytics Cube</CUBE_CAPTION>
            <CUBE_SOURCE>1</CUBE_SOURCE>
            <PREFERRED_QUERY_PATTERNS>3</PREFERRED_QUERY_PATTERNS>
          </row>"#;

pub fn get_cubes_response() -> String {
    discover_rowset_envelope(UUID_TYPE, CUBE_ROW_FIELDS, CUBE_ROWS)
}

// ./src/dimensions.rs
use crate::response::discover_rowset_envelope;

const DIM_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_NAME" name="DIMENSION_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_CAPTION" name="DIMENSION_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_ORDINAL" name="DIMENSION_ORDINAL" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_TYPE" name="DIMENSION_TYPE" type="xsd:short" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_CARDINALITY" name="DIMENSION_CARDINALITY" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DEFAULT_HIERARCHY" name="DEFAULT_HIERARCHY" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="IS_VIRTUAL" name="IS_VIRTUAL" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="IS_READWRITE" name="IS_READWRITE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_SETTINGS" name="DIMENSION_UNIQUE_SETTINGS" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_IS_VISIBLE" name="DIMENSION_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>"#;

const DIM_ROWS: &str = r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_NAME>Measures</DIMENSION_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_CAPTION>Measures</DIMENSION_CAPTION>
            <DIMENSION_ORDINAL>0</DIMENSION_ORDINAL>
            <DIMENSION_TYPE>2</DIMENSION_TYPE>
            <DIMENSION_CARDINALITY>1</DIMENSION_CARDINALITY>
            <DEFAULT_HIERARCHY>[Measures]</DEFAULT_HIERARCHY>
            <DESCRIPTION>Detta är mätvärdena</DESCRIPTION>
            <IS_VIRTUAL>false</IS_VIRTUAL>
            <IS_READWRITE>false</IS_READWRITE>
            <DIMENSION_UNIQUE_SETTINGS>0</DIMENSION_UNIQUE_SETTINGS>
            <DIMENSION_IS_VISIBLE>false</DIMENSION_IS_VISIBLE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_NAME>Produktkategori</DIMENSION_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_CAPTION>Produktkategori</DIMENSION_CAPTION>
            <DIMENSION_ORDINAL>1</DIMENSION_ORDINAL>
            <DIMENSION_TYPE>0</DIMENSION_TYPE>
            <DIMENSION_CARDINALITY>50</DIMENSION_CARDINALITY>
            <DEFAULT_HIERARCHY>[Produktkategori]</DEFAULT_HIERARCHY>
            <DESCRIPTION>Våra olika produkter</DESCRIPTION>
            <IS_VIRTUAL>false</IS_VIRTUAL>
            <IS_READWRITE>false</IS_READWRITE>
            <DIMENSION_UNIQUE_SETTINGS>0</DIMENSION_UNIQUE_SETTINGS>
            <DIMENSION_IS_VISIBLE>true</DIMENSION_IS_VISIBLE>
          </row>"#;

pub fn get_dimensions_response() -> String {
    discover_rowset_envelope("", DIM_ROW_FIELDS, DIM_ROWS)
}

// ./src/execute.rs
use crate::response::wrap_in_soap_envelope;

pub fn get_empty_execute_response() -> String {
    wrap_in_soap_envelope(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:empty"/>
      </return>
    </ExecuteResponse>"#,
    )
}

pub fn get_execute_statement_response(mdx: &str) -> String {
    let has_measures = mdx.contains("Measures") || mdx.contains("measures");
    let measure_name = "Total_Försäljning";
    let measure_value = if has_measures { "1250000.5" } else { "" };

    let inner = format!(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence minOccurs="0" maxOccurs="unbounded"><xsd:element name="row" type="row"/></xsd:sequence></xsd:complexType>
            </xsd:element>
            <xsd:complexType name="row">
              <xsd:sequence>
                <xsd:element sql:field="{name}" name="{name}" type="xsd:double" minOccurs="0"/>
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
          <row>
            <{name}>{val}</{name}>
          </row>
        </root>
      </return>
    </ExecuteResponse>"#,
        name = measure_name,
        val = measure_value,
    );
    wrap_in_soap_envelope(&inner)
}

// ./src/hierarchies.rs
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
            <DIMENSION_IS_VISIBLE>false</DIMENSION_IS_VISIBLE>
            <HIERARCHY_ORDINAL>0</HIERARCHY_ORDINAL>
            <DIMENSION_IS_SHARED>true</DIMENSION_IS_SHARED>
            <HIERARCHY_IS_VISIBLE>false</HIERARCHY_IS_VISIBLE>
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

// ./src/kpis.rs
use crate::response::discover_rowset_envelope;

const KPIS_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="KPI_NAME" name="KPI_NAME" type="xsd:string"/>
                <xsd:element sql:field="KPI_CAPTION" name="KPI_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_DESCRIPTION" name="KPI_DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_DISPLAY_FOLDER" name="KPI_DISPLAY_FOLDER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_VALUE" name="KPI_VALUE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_GOAL" name="KPI_GOAL" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_STATUS" name="KPI_STATUS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_TREND" name="KPI_TREND" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_STATUS_GRAPHIC" name="KPI_STATUS_GRAPHIC" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_TREND_GRAPHIC" name="KPI_TREND_GRAPHIC" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_WEIGHT" name="KPI_WEIGHT" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_CURRENT_TIME_MEMBER" name="KPI_CURRENT_TIME_MEMBER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_PARENT_KPI_NAME" name="KPI_PARENT_KPI_NAME" type="xsd:string" minOccurs="0"/>"#;

pub fn get_kpis_response() -> String {
    discover_rowset_envelope("", KPIS_ROW_FIELDS, "")
}

// ./src/levels.rs
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
            <LEVEL_IS_VISIBLE>false</LEVEL_IS_VISIBLE>
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

// ./src/literals.rs
use crate::response::discover_rowset_envelope;

const LITERAL_ROW_FIELDS: &str = r#"                <xsd:element sql:field="LITERAL_NAME" name="LITERAL_NAME" type="xsd:string"/>
                <xsd:element sql:field="LITERAL_VALUE" name="LITERAL_VALUE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LITERAL_INVALID_CHARS" name="LITERAL_INVALID_CHARS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LITERAL_INVALID_STARTING_CHARS" name="LITERAL_INVALID_STARTING_CHARS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LITERAL_MAX_LENGTH" name="LITERAL_MAX_LENGTH" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="LITERAL_SUFFIX" name="LITERAL_SUFFIX" type="xsd:string" minOccurs="0"/>"#;

const LITERAL_ROWS: &str = r#"          <row><LITERAL_NAME>DBLITERAL_CATALOG_NAME</LITERAL_NAME><LITERAL_VALUE>KTH_KEX_MALLOY_CUBE</LITERAL_VALUE><LITERAL_MAX_LENGTH>128</LITERAL_MAX_LENGTH></row>
          <row><LITERAL_NAME>DBLITERAL_CATALOG_SEPARATOR</LITERAL_NAME><LITERAL_VALUE>.</LITERAL_VALUE></row>
          <row><LITERAL_NAME>DBLITERAL_QUOTE_PREFIX</LITERAL_NAME><LITERAL_VALUE>[</LITERAL_VALUE></row>
          <row><LITERAL_NAME>DBLITERAL_QUOTE_SUFFIX</LITERAL_NAME><LITERAL_VALUE>]</LITERAL_VALUE></row>
          <row><LITERAL_NAME>DBLITERAL_PASS_THROUGH_COLUMNS</LITERAL_NAME><LITERAL_VALUE>true</LITERAL_VALUE></row>"#;

pub fn get_literals_response() -> String {
    discover_rowset_envelope("", LITERAL_ROW_FIELDS, LITERAL_ROWS)
}

// ./src/main.rs
use axum::{
    http::{header, HeaderMap, HeaderName, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use std::net::SocketAddr;

mod parser;
mod response;
mod properties;
mod schema_rowsets;
mod catalogs;
mod cubes;
mod tables;
mod dimensions;
mod measures;
mod hierarchies;
mod levels;
mod mdschema_properties;
mod members;
mod literals;
mod sets;
mod kpis;
mod measure_groups;
mod measuregroup_dimensions;
mod execute;

use parser::{parse_xmla, XmlaRequest};

#[tokio::main]
async fn main() {
    let app = Router::new().route("/xmla", post(handle_xmla));
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("🚀 Rust-XMLA Proxy (v3 - ModuleRefactor) snurrar på http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "text/xml; charset=utf-8".parse().unwrap());
    headers.insert(header::SERVER, "Rust-Malloy-Proxy/2.0".parse().unwrap());
    headers.insert(header::CONNECTION, "close".parse().unwrap());
    headers.insert(
        HeaderName::from_static("x-transport-caps-negotiation-flags"),
        "0,0,0,0,0".parse().unwrap(),
    );
    headers
}

async fn handle_xmla(body: String) -> impl IntoResponse {
    if body.contains("<RequestType>") {
        let req_start = body.find("<RequestType>").unwrap() + 13;
        let req_end = body.find("</RequestType>").unwrap();
        println!("🔍 Rå RequestType från Excel: {}", &body[req_start..req_end]);
    }

    let headers = default_headers();
    let request = parse_xmla(&body);
    println!("📥 Fick anrop, tolkade som: {:?}", request);

    if body.contains("<Execute") {
        println!("🔍 Rå Execute från Excel:\n{}", body);
    }

    let response_body = match request {
        XmlaRequest::BeginSession | XmlaRequest::ExecuteEmpty => {
            execute::get_empty_execute_response()
        }

        XmlaRequest::DiscoverProperties { property_names } => {
            if property_names.len() == 1 && property_names[0] == "Catalog" {
                println!("Excel frågar efter Catalog");
                properties::get_single_property_response("Catalog", "KTH_KEX_MALLOY_CUBE")
            } else {
                println!("Excel frågar efter egenskaper: {:?}", property_names);
                properties::get_properties_response(&property_names)
            }
        }

        XmlaRequest::DiscoverSchemaRowsets => schema_rowsets::get_schemas_response(),
        XmlaRequest::DbSchemaCatalogs => catalogs::get_catalogs_response(),
        XmlaRequest::MdschemaCubes => cubes::get_cubes_response(),
        XmlaRequest::DbschemaTables => tables::get_tables_response(),
        XmlaRequest::MdschemaDimensions => {
            println!("📥 Skickar Dimensioner till Excel!");
            dimensions::get_dimensions_response()
        }
        XmlaRequest::MdschemaMeasures => {
            println!("📥 Skickar Measures till Excel!");
            measures::get_measures_response()
        }
        XmlaRequest::MdschemaHierarchies => {
            println!("📥 Hierarchies");
            hierarchies::get_hierarchies_response()
        }
        XmlaRequest::MdschemaLevels => {
            println!("📥 Levels");
            levels::get_levels_response()
        }
        XmlaRequest::ExecuteStatement(mdx) => {
            println!("📥 MDX Statement: {}", mdx);
            execute::get_execute_statement_response(&mdx)
        }
        XmlaRequest::MdschemaProperties { property_type } => {
            println!("📥 MDSCHEMA_PROPERTIES (PROPERTY_TYPE={:?})", property_type);
            mdschema_properties::get_mdschema_properties_response(property_type)
        }
        XmlaRequest::MdschemaMembers => {
            println!("📥 MDSCHEMA_MEMBERS");
            members::get_members_response()
        }
        XmlaRequest::DiscoverLiterals => {
            println!("📥 DISCOVER_LITERALS");
            literals::get_literals_response()
        }
        XmlaRequest::MdschemaSets => {
            println!("📥 MDSCHEMA_SETS");
            sets::get_sets_response()
        }
        XmlaRequest::MdschemaKpis => {
            println!("📥 MDSCHEMA_KPIS");
            kpis::get_kpis_response()
        }
        XmlaRequest::MdschemaMeasureGroups => {
            println!("📥 MDSCHEMA_MEASUREGROUPS");
            measure_groups::get_measure_groups_response()
        }
        XmlaRequest::MdschemaMeasureGroupDimensions => {
            println!("📥 MDSCHEMA_MEASUREGROUP_DIMENSIONS");
            measuregroup_dimensions::get_measuregroup_dimensions_response()
        }

        XmlaRequest::Unknown => {
            println!("❌ Okänt anrop.");
            return (StatusCode::BAD_REQUEST, headers, "Okänt anrop".to_string());
        }
    };

    (StatusCode::OK, headers, response_body)
}

// ./src/mdschema_properties.rs
use crate::response::discover_rowset_envelope;

const PROPERTIES_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="HIERARCHY_UNIQUE_NAME" name="HIERARCHY_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_UNIQUE_NAME" name="LEVEL_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEMBER_UNIQUE_NAME" name="MEMBER_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_NAME" name="PROPERTY_NAME" type="xsd:string"/>
                <xsd:element sql:field="PROPERTY_CAPTION" name="PROPERTY_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_DESCRIPTION" name="PROPERTY_DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_TYPE" name="PROPERTY_TYPE" type="xsd:short" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_CONTENT_TYPE" name="PROPERTY_CONTENT_TYPE" type="xsd:short" minOccurs="0"/>"#;

fn member_property_rows() -> &'static str {
    r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_CAPTION</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_CAPTION</PROPERTY_CAPTION>
            <PROPERTY_TYPE>1</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_KEY</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_KEY</PROPERTY_CAPTION>
            <PROPERTY_TYPE>1</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>1</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>"#
}

fn system_property_rows() -> &'static str {
    r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>FORMATTED_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>FORMATTED_VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>1</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>FORMAT_STRING</PROPERTY_NAME>
            <PROPERTY_CAPTION>FORMAT_STRING</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>2</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>FORE_COLOR</PROPERTY_NAME>
            <PROPERTY_CAPTION>FORE_COLOR</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>2</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>BACK_COLOR</PROPERTY_NAME>
            <PROPERTY_CAPTION>BACK_COLOR</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>2</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>FONT_NAME</PROPERTY_NAME>
            <PROPERTY_CAPTION>FONT_NAME</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>2</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>FONT_SIZE</PROPERTY_NAME>
            <PROPERTY_CAPTION>FONT_SIZE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>2</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>CELL_ORDINAL</PROPERTY_NAME>
            <PROPERTY_CAPTION>CELL_ORDINAL</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>"#
}

fn member_value_rows() -> &'static str {
    r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Measures]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>5</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[(All)]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>5</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>5</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>"#
}

pub fn get_mdschema_properties_response(property_type: Option<i32>) -> String {
    let rows = match property_type {
        Some(1) => member_property_rows().to_string(),
        Some(2) => system_property_rows().to_string(),
        Some(5) => member_value_rows().to_string(),
        _ => format!("{}\n{}", system_property_rows(), member_value_rows()),
    };
    discover_rowset_envelope("", PROPERTIES_ROW_FIELDS, &rows)
}

// ./src/measure_groups.rs
use crate::response::discover_rowset_envelope;

const MEASUREGROUP_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASUREGROUP_NAME" name="MEASUREGROUP_NAME" type="xsd:string"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="IS_WRITE_ENABLED" name="IS_WRITE_ENABLED" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="MEASUREGROUP_CAPTION" name="MEASUREGROUP_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASUREGROUP_CARDINALITY" name="MEASUREGROUP_CARDINALITY" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="MEASUREGROUP_SIZE" name="MEASUREGROUP_SIZE" type="xsd:long" minOccurs="0"/>"#;

pub fn get_measure_groups_response() -> String {
    discover_rowset_envelope("", MEASUREGROUP_ROW_FIELDS,
        r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <MEASUREGROUP_NAME>Faktatabell</MEASUREGROUP_NAME>
            <MEASUREGROUP_CAPTION>Faktatabell</MEASUREGROUP_CAPTION>
          </row>"#
    )
}

// ./src/measuregroup_dimensions.rs
use crate::response::discover_rowset_envelope;

const MG_DIM_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASUREGROUP_NAME" name="MEASUREGROUP_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASUREGROUP_CARDINALITY" name="MEASUREGROUP_CARDINALITY" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_CARDINALITY" name="DIMENSION_CARDINALITY" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_IS_VISIBLE" name="DIMENSION_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_IS_FACT_DIMENSION" name="DIMENSION_IS_FACT_DIMENSION" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_GRANULARITY" name="DIMENSION_GRANULARITY" type="xsd:string" minOccurs="0"/>"#;

pub fn get_measuregroup_dimensions_response() -> String {
    discover_rowset_envelope("", MG_DIM_ROW_FIELDS,
        r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <MEASUREGROUP_NAME>Faktatabell</MEASUREGROUP_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_IS_VISIBLE>true</DIMENSION_IS_VISIBLE>
            <DIMENSION_IS_FACT_DIMENSION>false</DIMENSION_IS_FACT_DIMENSION>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <MEASUREGROUP_NAME>Faktatabell</MEASUREGROUP_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_IS_VISIBLE>true</DIMENSION_IS_VISIBLE>
            <DIMENSION_IS_FACT_DIMENSION>false</DIMENSION_IS_FACT_DIMENSION>
          </row>"#
    )
}

// ./src/measures.rs
use crate::response::{discover_rowset_envelope, UUID_TYPE};

const MEASURE_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASURE_NAME" name="MEASURE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASURE_UNIQUE_NAME" name="MEASURE_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASURE_CAPTION" name="MEASURE_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_GUID" name="MEASURE_GUID" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_AGGREGATOR" name="MEASURE_AGGREGATOR" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DATA_TYPE" name="DATA_TYPE" type="xsd:unsignedShort" minOccurs="0"/>
                <xsd:element sql:field="NUMERIC_PRECISION" name="NUMERIC_PRECISION" type="xsd:unsignedShort" minOccurs="0"/>
                <xsd:element sql:field="NUMERIC_SCALE" name="NUMERIC_SCALE" type="xsd:short" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_UNITS" name="MEASURE_UNITS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="EXPRESSION" name="EXPRESSION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_IS_VISIBLE" name="MEASURE_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="LEVELS_LIST" name="LEVELS_LIST" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_NAME_SQL_COLUMN_NAME" name="MEASURE_NAME_SQL_COLUMN_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_UNQUALIFIED_CAPTION" name="MEASURE_UNQUALIFIED_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASUREGROUP_NAME" name="MEASUREGROUP_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_DISPLAY_FOLDER" name="MEASURE_DISPLAY_FOLDER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DEFAULT_FORMAT_STRING" name="DEFAULT_FORMAT_STRING" type="xsd:string" minOccurs="0"/>"#;

const MEASURE_ROWS: &str = r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <MEASURE_NAME>Total Försäljning</MEASURE_NAME>
            <MEASURE_UNIQUE_NAME>[Measures].[Total Försäljning]</MEASURE_UNIQUE_NAME>
            <MEASURE_CAPTION>Total Försäljning (SEK)</MEASURE_CAPTION>
            <MEASURE_AGGREGATOR>1</MEASURE_AGGREGATOR>
            <DATA_TYPE>5</DATA_TYPE>
            <NUMERIC_PRECISION>18</NUMERIC_PRECISION>
            <NUMERIC_SCALE>2</NUMERIC_SCALE>
            <MEASURE_UNITS>SEK</MEASURE_UNITS>
            <DESCRIPTION>Vår totala försäljning</DESCRIPTION>
            <MEASURE_IS_VISIBLE>true</MEASURE_IS_VISIBLE>
            <MEASURE_UNQUALIFIED_CAPTION>Total Försäljning (SEK)</MEASURE_UNQUALIFIED_CAPTION>
            <MEASUREGROUP_NAME>Faktatabell</MEASUREGROUP_NAME>
            <DEFAULT_FORMAT_STRING>#,##0.00 SEK</DEFAULT_FORMAT_STRING>
          </row>"#;

pub fn get_measures_response() -> String {
    discover_rowset_envelope(UUID_TYPE, MEASURE_ROW_FIELDS, MEASURE_ROWS)
}

// ./src/members.rs
use crate::response::discover_rowset_envelope;

const MEMBER_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="HIERARCHY_UNIQUE_NAME" name="HIERARCHY_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="LEVEL_UNIQUE_NAME" name="LEVEL_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="LEVEL_NUMBER" name="LEVEL_NUMBER" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="MEMBER_ORDINAL" name="MEMBER_ORDINAL" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="MEMBER_NAME" name="MEMBER_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEMBER_UNIQUE_NAME" name="MEMBER_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEMBER_TYPE" name="MEMBER_TYPE" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="MEMBER_CAPTION" name="MEMBER_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CHILDREN_CARDINALITY" name="CHILDREN_CARDINALITY" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="PARENT_LEVEL" name="PARENT_LEVEL" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="PARENT_COUNT" name="PARENT_COUNT" type="xsd:unsignedInt" minOccurs="0"/>"#;

const MEMBER_ROWS: &str = r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Measures]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[Measures]</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>0</LEVEL_NUMBER>
            <MEMBER_ORDINAL>0</MEMBER_ORDINAL>
            <MEMBER_NAME>Total Försäljning</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>[Measures].[Total Försäljning]</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>2</MEMBER_TYPE>
            <MEMBER_CAPTION>Total Försäljning</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>0</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>0</PARENT_LEVEL>
            <PARENT_COUNT>0</PARENT_COUNT>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[(All)]</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>0</LEVEL_NUMBER>
            <MEMBER_ORDINAL>0</MEMBER_ORDINAL>
            <MEMBER_NAME>All Produktkategorier</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>[Produktkategori].[All Produktkategorier]</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>1</MEMBER_TYPE>
            <MEMBER_CAPTION>All Produktkategorier</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>50</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>0</PARENT_LEVEL>
            <PARENT_COUNT>0</PARENT_COUNT>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>1</LEVEL_NUMBER>
            <MEMBER_ORDINAL>1</MEMBER_ORDINAL>
            <MEMBER_NAME>Kategori A</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>[Produktkategori].[Kategori A]</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>3</MEMBER_TYPE>
            <MEMBER_CAPTION>Kategori A</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>0</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>0</PARENT_LEVEL>
            <PARENT_COUNT>1</PARENT_COUNT>
          </row>"#;

pub fn get_members_response() -> String {
    discover_rowset_envelope("", MEMBER_ROW_FIELDS, MEMBER_ROWS)
}

// ./src/parser.rs
use quick_xml::events::Event;
use quick_xml::Reader;

#[derive(Debug, PartialEq)]
pub enum XmlaRequest {
    DiscoverProperties { property_names: Vec<String> },
    DiscoverSchemaRowsets,
    DiscoverLiterals,
    DbSchemaCatalogs,
    MdschemaCubes,
    DbschemaTables,
    MdschemaDimensions,
    MdschemaMeasures,
    MdschemaHierarchies,
    MdschemaLevels,
    MdschemaProperties { property_type: Option<i32> },
    MdschemaMembers,
    MdschemaSets,
    MdschemaKpis,
    MdschemaMeasureGroups,
    MdschemaMeasureGroupDimensions,
    BeginSession,
    ExecuteEmpty,
    ExecuteStatement(String),
    Unknown,
}

pub fn parse_xmla(xml: &str) -> XmlaRequest {
    let mut reader = Reader::from_str(xml);

    let mut in_request_type = false;
    let mut is_execute = false;
    let mut in_property_name = false;
    let mut in_statement = false;
    let mut is_begin_session = false;
    let mut in_property_type = false;

    let mut parsed_request_type = String::new();
    let mut requested_properties: Vec<String> = Vec::new();
    let mut statement_text = String::new();
    let mut property_type: Option<i32> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                match e.local_name().as_ref() {
                    b"RequestType" => in_request_type = true,
                    b"PropertyName" => in_property_name = true,
                    b"Statement" => in_statement = true,
                    b"BeginSession" | b"BeginGetSessionToken" => is_begin_session = true,
                    b"Execute" => is_execute = true,
                    b"PROPERTY_TYPE" => in_property_type = true,
                    _ => (),
                }
            }
            Ok(Event::Empty(ref e)) => {
                match e.local_name().as_ref() {
                    b"Execute" => is_execute = true,
                    _ => (),
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().trim().to_string();

                if !text.is_empty() {
                    if in_request_type {
                        parsed_request_type = text;
                    } else if in_property_name {
                        requested_properties.push(text);
                    } else if in_statement {
                        statement_text = text;
                    } else if in_property_type {
                        if let Ok(v) = text.parse::<i32>() {
                            property_type = Some(v);
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                match e.local_name().as_ref() {
                    b"RequestType" => in_request_type = false,
                    b"PropertyName" => in_property_name = false,
                    b"Statement" => in_statement = false,
                    b"PROPERTY_TYPE" => in_property_type = false,
                    _ => (),
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => (),
        }
    }

    match parsed_request_type.as_str() {
        "DISCOVER_PROPERTIES" => {
            return XmlaRequest::DiscoverProperties {
                property_names: requested_properties,
            }
        }
        "DISCOVER_SCHEMA_ROWSETS" => return XmlaRequest::DiscoverSchemaRowsets,
        "DISCOVER_LITERALS" => return XmlaRequest::DiscoverLiterals,
        "DBSCHEMA_CATALOGS" => return XmlaRequest::DbSchemaCatalogs,
        "MDSCHEMA_CUBES" => return XmlaRequest::MdschemaCubes,
        "DBSCHEMA_TABLES" => return XmlaRequest::DbschemaTables,
        "MDSCHEMA_DIMENSIONS" => return XmlaRequest::MdschemaDimensions,
        "MDSCHEMA_MEASURES" => return XmlaRequest::MdschemaMeasures,
        "MDSCHEMA_HIERARCHIES" => return XmlaRequest::MdschemaHierarchies,
        "MDSCHEMA_LEVELS" => return XmlaRequest::MdschemaLevels,
        "MDSCHEMA_PROPERTIES" => return XmlaRequest::MdschemaProperties { property_type },
        "MDSCHEMA_MEMBERS" => return XmlaRequest::MdschemaMembers,
        "MDSCHEMA_SETS" => return XmlaRequest::MdschemaSets,
        "MDSCHEMA_KPIS" => return XmlaRequest::MdschemaKpis,
        "MDSCHEMA_MEASUREGROUPS" => return XmlaRequest::MdschemaMeasureGroups,
        "MDSCHEMA_MEASUREGROUP_DIMENSIONS" => return XmlaRequest::MdschemaMeasureGroupDimensions,
        _ => (),
    };

    if is_execute {
        if !statement_text.is_empty() {
            return XmlaRequest::ExecuteStatement(statement_text);
        } else if is_begin_session {
            return XmlaRequest::BeginSession;
        } else {
            return XmlaRequest::ExecuteEmpty;
        }
    }

    XmlaRequest::Unknown
}

// ./src/properties.rs
use crate::response::{discover_rowset_envelope, UUID_TYPE};

struct Property {
    name: &'static str,
    description: &'static str,
    prop_type: &'static str,
    access_type: &'static str,
    is_required: bool,
    value: Option<&'static str>,
}

const PROPERTIES: &[Property] = &[
    Property {
        name: "ProviderName",
        description: "ProviderName",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("Min Riktiga Rust Proxy"),
    },
    Property {
        name: "DbpropMsmdSubqueries",
        description: "DbpropMsmdSubqueries",
        prop_type: "int",
        access_type: "ReadWrite",
        is_required: false,
        value: Some("2"),
    },
    Property {
        name: "DbpropMsmdOptimizeResponse",
        description: "DbpropMsmdOptimizeResponse",
        prop_type: "long",
        access_type: "ReadWrite",
        is_required: false,
        value: Some("0"),
    },
    Property {
        name: "DbpropMsmdActivityID",
        description: "DbpropMsmdActivityID",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: None,
    },
    Property {
        name: "DbpropMsmdCurrentActivityID",
        description: "DbpropMsmdCurrentActivityID",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: None,
    },
    Property {
        name: "ApplicationContext",
        description: "ApplicationContext",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: None,
    },
    Property {
        name: "Catalog",
        description: "Catalog",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: Some("KTH_KEX_MALLOY_CUBE"),
    },
    Property {
        name: "ServerName",
        description: "ServerName",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("rust-proxy"),
    },
    Property {
        name: "ProviderVersion",
        description: "ProviderVersion",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("1.0.0"),
    },
    Property {
        name: "MdpropMdxSubqueries",
        description: "MdpropMdxSubqueries",
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        value: Some("63"),
    },
    Property {
        name: "MdpropMdxDrillFunctions",
        description: "MdpropMdxDrillFunctions",
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        value: Some("7"),
    },
    Property {
        name: "MdpropMdxNamedSets",
        description: "MdpropMdxNamedSets",
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        value: Some("15"),
    },
    Property {
        name: "MdpropMdxDdlExtensions",
        description: "MdpropMdxDdlExtensions",
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        value: Some("23"),
    },
    Property {
        name: "MDXSupport",
        description: "MDXSupport",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("Core"),
    },
];

const PROPERTY_ROW_FIELDS: &str = r#"                <xsd:element sql:field="PropertyName" name="PropertyName" type="xsd:string"/>
                <xsd:element sql:field="PropertyDescription" name="PropertyDescription" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PropertyType" name="PropertyType" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PropertyAccessType" name="PropertyAccessType" type="xsd:string"/>
                <xsd:element sql:field="IsRequired" name="IsRequired" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="Value" name="Value" type="xsd:string" minOccurs="0"/>"#;

fn format_row(p: &Property) -> String {
    format!(
        r#"          <row>
            <PropertyName>{name}</PropertyName>
            <PropertyDescription>{desc}</PropertyDescription>
            <PropertyType>{ptype}</PropertyType>
            <PropertyAccessType>{access}</PropertyAccessType>
            <IsRequired>{req}</IsRequired>
            <Value>{val}</Value>
          </row>"#,
        name = p.name,
        desc = p.description,
        ptype = p.prop_type,
        access = p.access_type,
        req = p.is_required,
        val = p.value.unwrap_or(""),
    )
}

pub fn get_properties_response(filter: &[String]) -> String {
    let filtered: Vec<String> = PROPERTIES
        .iter()
        .filter(|p| filter.is_empty() || filter.iter().any(|f| f == p.name))
        .map(format_row)
        .collect();

    discover_rowset_envelope(UUID_TYPE, PROPERTY_ROW_FIELDS, &filtered.join("\n"))
}

pub fn get_single_property_response(name: &str, value: &str) -> String {
    let row = format!(
        r#"          <row>
            <PropertyName>{name}</PropertyName>
            <PropertyDescription>{name}</PropertyDescription>
            <PropertyType>string</PropertyType>
            <PropertyAccessType>ReadWrite</PropertyAccessType>
            <IsRequired>false</IsRequired>
            <Value>{value}</Value>
          </row>"#,
    );
    discover_rowset_envelope(UUID_TYPE, PROPERTY_ROW_FIELDS, &row)
}

// ./src/response.rs
pub fn wrap_in_soap_envelope(inner_xml: &str) -> String {
    format!(
        r#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Header>
    <Session xmlns="urn:schemas-microsoft-com:xml-analysis" SessionId="RUST-SESSION-456" />
  </soap:Header>
  <soap:Body>
{}
  </soap:Body>
</soap:Envelope>"#,
        inner_xml
    )
}

pub const UUID_TYPE: &str = r#"<xsd:simpleType name="uuid">
              <xsd:restriction base="xsd:string">
                <xsd:pattern value="[0-9a-zA-Z]{8}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{12}"/>
              </xsd:restriction>
            </xsd:simpleType>"#;

pub fn empty_discover_response() -> String {
    let inner = r#"    <DiscoverResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" />
        </root>
      </return>
    </DiscoverResponse>"#;
    wrap_in_soap_envelope(inner)
}

pub fn discover_rowset_envelope(extra_schema: &str, row_fields: &str, rows: &str) -> String {
    let inner = format!(
        r#"    <DiscoverResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence minOccurs="0" maxOccurs="unbounded"><xsd:element name="row" type="row"/></xsd:sequence></xsd:complexType>
            </xsd:element>
{extra_schema}
            <xsd:complexType name="row">
              <xsd:sequence>
{row_fields}
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
{rows}
        </root>
      </return>
    </DiscoverResponse>"#,
    );
    wrap_in_soap_envelope(&inner)
}

// ./src/schema_rowsets.rs
use crate::response::{discover_rowset_envelope, UUID_TYPE};

const SCHEMA_ROW_FIELDS: &str = r#"                <xsd:element sql:field="SchemaName" name="SchemaName" type="xsd:string"/>
                <xsd:element sql:field="SchemaGuid" name="SchemaGuid" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="Restrictions" name="Restrictions" minOccurs="0" maxOccurs="unbounded">
                  <xsd:complexType>
                    <xsd:sequence>
                      <xsd:element sql:field="Name" name="Name" type="xsd:string" minOccurs="0"/>
                      <xsd:element sql:field="Type" name="Type" type="xsd:string" minOccurs="0"/>
                    </xsd:sequence>
                  </xsd:complexType>
                </xsd:element>
                <xsd:element sql:field="RestrictionsMask" name="RestrictionsMask" type="xsd:unsignedLong" minOccurs="0"/>"#;

const SCHEMA_ROWSET_DATA: &str = r#"          <row>
            <SchemaName>DBSCHEMA_CATALOGS</SchemaName>
            <SchemaGuid>C8B52211-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DBSCHEMA_TABLES</SchemaName>
            <SchemaGuid>C8B52229-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>TABLE_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_OLAP_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DBSCHEMA_COLUMNS</SchemaName>
            <SchemaGuid>C8B52214-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>TABLE_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>COLUMN_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>COLUMN_OLAP_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DBSCHEMA_PROVIDER_TYPES</SchemaName>
            <SchemaGuid>C8B5222C-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>DATA_TYPE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>BEST_MATCH</Name><Type>xsd:boolean</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_CUBES</SchemaName>
            <SchemaGuid>C8B522D8-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>BASE_CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_DIMENSIONS</SchemaName>
            <SchemaGuid>C8B522D9-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>DIMENSION_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>127</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_HIERARCHIES</SchemaName>
            <SchemaGuid>C8B522DA-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_ORIGIN</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>511</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_LEVELS</SchemaName>
            <SchemaGuid>C8B522DB-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_ORIGIN</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>LEVEL_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>1023</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_MEASURES</SchemaName>
            <SchemaGuid>C8B522DC-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEASURE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEASURE_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEASUREGROUP_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>MEASURE_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>255</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_PROPERTIES</SchemaName>
            <SchemaGuid>C8B522DD-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEMBER_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>PROPERTY_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>PROPERTY_TYPE</Name><Type>xsd:short</Type></Restrictions>
            <Restrictions><Name>PROPERTY_CONTENT_TYPE</Name><Type>xsd:short</Type></Restrictions>
            <Restrictions><Name>PROPERTY_ORIGIN</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>PROPERTY_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>8191</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_MEMBERS</SchemaName>
            <SchemaGuid>C8B522DE-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_NUMBER</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <Restrictions><Name>MEMBER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEMBER_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEMBER_CAPTION</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEMBER_TYPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>TREE_OP</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>SCOPE</Name><Type>xsd:int</Type></Restrictions>
            <RestrictionsMask>16383</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_FUNCTIONS</SchemaName>
            <SchemaGuid>A07CCD07-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>LIBRARY_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>INTERFACE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>FUNCTION_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ORIGIN</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_ACTIONS</SchemaName>
            <SchemaGuid>A07CCD08-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ACTION_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ACTION_TYPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>COORDINATE</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>COORDINATE_TYPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>INVOCATION</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>511</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_SETS</SchemaName>
            <SchemaGuid>A07CCD0B-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SET_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCOPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>SET_EVALUATION_CONTEXT</Name><Type>xsd:int</Type></Restrictions>
            <RestrictionsMask>255</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_INSTANCES</SchemaName>
            <SchemaGuid>20518699-2474-4C15-9885-0E947EC7A7E3</SchemaGuid>
            <Restrictions><Name>INSTANCE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_KPIS</SchemaName>
            <SchemaGuid>2AE44109-ED3D-4842-B16F-B694D1CB0E3F</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>KPI_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>SCOPE</Name><Type>xsd:int</Type></Restrictions>
            <RestrictionsMask>63</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_MEASUREGROUPS</SchemaName>
            <SchemaGuid>E1625EBF-FA96-42FD-BEA6-DB90ADAFD96B</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEASUREGROUP_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_MEASUREGROUP_DIMENSIONS</SchemaName>
            <SchemaGuid>A07CCD33-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEASUREGROUP_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>63</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_INPUT_DATASOURCES</SchemaName>
            <SchemaGuid>A07CCD32-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DATASOURCE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DATASOURCE_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_SERVICES</SchemaName>
            <SchemaGuid>3ADD8A95-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>SERVICE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SERVICE_TYPE_ID</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_SERVICE_PARAMETERS</SchemaName>
            <SchemaGuid>3ADD8A75-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>SERVICE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>PARAMETER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_FUNCTIONS</SchemaName>
            <SchemaGuid>3ADD8A79-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>SERVICE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>FUNCTION_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_MODEL_CONTENT</SchemaName>
            <SchemaGuid>3ADD8A76-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>MODEL_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ATTRIBUTE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>NODE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>NODE_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>NODE_TYPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>NODE_GUID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>NODE_CAPTION</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TREE_OPERATION</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <RestrictionsMask>1023</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_MODEL_XML</SchemaName>
            <SchemaGuid>4290B2D5-0E9C-4AA7-9369-98C95CFD9D13</SchemaGuid>
            <Restrictions><Name>MODEL_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_MODEL_CONTENT_PMML</SchemaName>
            <SchemaGuid>4290B2D5-0E9C-4AA7-9369-98C95CFD9D13</SchemaGuid>
            <Restrictions><Name>MODEL_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_MODELS</SchemaName>
            <SchemaGuid>3ADD8A77-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>MODEL_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SERVICE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SERVICE_TYPE_ID</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <Restrictions><Name>MINING_STRUCTURE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>127</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_COLUMNS</SchemaName>
            <SchemaGuid>3ADD8A78-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>MODEL_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>COLUMN_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_STRUCTURES</SchemaName>
            <SchemaGuid>883269F3-0CAD-462F-B6F5-E88A72418C4B</SchemaGuid>
            <Restrictions><Name>STRUCTURE_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>STRUCTURE_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>STRUCTURE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>7</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_STRUCTURE_COLUMNS</SchemaName>
            <SchemaGuid>9952E836-BFBF-4D1F-8535-9B67DBD9DDFE</SchemaGuid>
            <Restrictions><Name>STRUCTURE_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>STRUCTURE_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>STRUCTURE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>COLUMN_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_DATASOURCES</SchemaName>
            <SchemaGuid>06C03D41-F66D-49F3-B1B8-987F7AF4CF18</SchemaGuid>
            <Restrictions><Name>DataSourceName</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>URL</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ProviderName</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ProviderType</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>AuthenticationMode</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_PROPERTIES</SchemaName>
            <SchemaGuid>4B40ADFB-8B09-4758-97BB-636E8AE97BCF</SchemaGuid>
            <Restrictions><Name>PropertyName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_SCHEMA_ROWSETS</SchemaName>
            <SchemaGuid>EEA0302B-7922-4992-8991-0E605D0E5593</SchemaGuid>
            <Restrictions><Name>SchemaName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_ENUMERATORS</SchemaName>
            <SchemaGuid>55A9E78B-ACCB-45B4-95A6-94C5065617A7</SchemaGuid>
            <Restrictions><Name>EnumName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_KEYWORDS</SchemaName>
            <SchemaGuid>1426C443-4CDD-4A40-8F45-572FAB9BBAA1</SchemaGuid>
            <Restrictions><Name>Keyword</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_LITERALS</SchemaName>
            <SchemaGuid>C3EF5ECB-0A07-4665-A140-B075722DBDC2</SchemaGuid>
            <Restrictions><Name>LiteralName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XML_METADATA</SchemaName>
            <SchemaGuid>3444B255-171E-4CB9-AD98-19E57888A75F</SchemaGuid>
            <Restrictions><Name>DatabaseID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DimensionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CubeID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MeasureGroupID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>PartitionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>PerspectiveID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DimensionPermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>RoleID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DatabasePermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MiningModelID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MiningModelPermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DataSourceID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MiningStructureID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>AggregationDesignID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TraceID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MiningStructurePermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CubePermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>AssemblyID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MdxScriptID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DataSourceViewID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DataSourcePermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CalculatedColumns</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ObjectExpansion</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DBWorkloadGroupID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ResourcePoolID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ModifiedAfter</Name><Type>xsd:dateTime</Type></Restrictions>
            <RestrictionsMask>67108863</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_TRACES</SchemaName>
            <SchemaGuid>A07CCD1A-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>TraceID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>Type</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_TRACE_DEFINITION_PROVIDERINFO</SchemaName>
            <SchemaGuid>A07CCD1B-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>Data</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XEVENT_PACKAGES</SchemaName>
            <SchemaGuid>A07CCD1C-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ID</Name><Type>uuid</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XEVENT_OBJECTS</SchemaName>
            <SchemaGuid>A07CCD1D-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>OBJECT_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XEVENT_OBJECT_COLUMNS</SchemaName>
            <SchemaGuid>A07CCD1E-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>OBJECT_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XEVENT_SESSION_TARGETS</SchemaName>
            <SchemaGuid>A07CCD1F-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>XESessionName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XEVENT_SESSIONS</SchemaName>
            <SchemaGuid>A07CCD20-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>XESessionName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_TRACE_COLUMNS</SchemaName>
            <SchemaGuid>A07CCD18-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>Data</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_TRACE_EVENT_CATEGORIES</SchemaName>
            <SchemaGuid>A07CCD19-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>Data</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_MEMORYUSAGE</SchemaName>
            <SchemaGuid>A07CCD21-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>SPID</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <Restrictions><Name>MemoryUsed</Name><Type>xsd:long</Type></Restrictions>
            <Restrictions><Name>BaseObjectType</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <Restrictions><Name>Shrinkable</Name><Type>xsd:boolean</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_MEMORYGRANT</SchemaName>
            <SchemaGuid>A07CCD23-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>SPID</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_LOCKS</SchemaName>
            <SchemaGuid>A07CCD24-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>SPID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>LOCK_TRANSACTION_ID</Name><Type>uuid</Type></Restrictions>
            <Restrictions><Name>LOCK_OBJECT_ID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LOCK_STATUS</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>LOCK_TYPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>LOCK_MIN_TOTAL_MS</Name><Type>xsd:long</Type></Restrictions>
            <RestrictionsMask>63</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_CONNECTIONS</SchemaName>
            <SchemaGuid>A07CCD25-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CONNECTION_ID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CONNECTION_USER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CONNECTION_IMPERSONATED_USER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CONNECTION_HOST_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CONNECTION_ELAPSED_TIME_MS</Name><Type>xsd:long</Type></Restrictions>
            <Restrictions><Name>CONNECTION_LAST_COMMAND_ELAPSED_TIME_MS</Name><Type>xsd:long</Type></Restrictions>
            <Restrictions><Name>CONNECTION_IDLE_TIME_MS</Name><Type>xsd:long</Type></Restrictions>
            <RestrictionsMask>127</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_SESSIONS</SchemaName>
            <SchemaGuid>A07CCD26-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>SESSION_ID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SESSION_SPID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>SESSION_CONNECTION_ID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>SESSION_USER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SESSION_CURRENT_DATABASE</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SESSION_ELAPSED_TIME_MS</Name><Type>xsd:unsignedLong</Type></Restrictions>
            <Restrictions><Name>SESSION_CPU_TIME_MS</Name><Type>xsd:unsignedLong</Type></Restrictions>
            <Restrictions><Name>SESSION_IDLE_TIME_MS</Name><Type>xsd:unsignedLong</Type></Restrictions>
            <Restrictions><Name>SESSION_STATUS</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>RESTRICT_CATALOG_ID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>REQUEST_ACTIVITY_ID</Name><Type>uuid</Type></Restrictions>
            <Restrictions><Name>CLIENT_ACTIVITY_ID</Name><Type>uuid</Type></Restrictions>
            <RestrictionsMask>4095</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_JOBS</SchemaName>
            <SchemaGuid>A07CCD27-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>SPID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>JOB_ID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>JOB_DESCRIPTION</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>JOB_THREADPOOL_ID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>JOB_MIN_TOTAL_TIME_MS</Name><Type>xsd:long</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_TRANSACTIONS</SchemaName>
            <SchemaGuid>A07CCD28-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>TRANSACTION_ID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TRANSACTION_SESSION_ID</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_DB_CONNECTIONS</SchemaName>
            <SchemaGuid>A07CCD2A-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CONNECTION_ID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CONNECTION_IN_USE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CONNECTION_SERVER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CONNECTION_CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CONNECTION_SPID</Name><Type>xsd:int</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_MASTER_KEY</SchemaName>
            <SchemaGuid>A07CCD29-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>KEY</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
"#;

pub fn get_schemas_response() -> String {
    discover_rowset_envelope(UUID_TYPE, SCHEMA_ROW_FIELDS, SCHEMA_ROWSET_DATA)
}

// ./src/sets.rs
use crate::response::discover_rowset_envelope;

const SETS_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="SET_NAME" name="SET_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCOPE" name="SCOPE" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="EXPRESSION" name="EXPRESSION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DIMENSIONS" name="DIMENSIONS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="SET_CAPTION" name="SET_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="SET_DISPLAY_FOLDER" name="SET_DISPLAY_FOLDER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="SET_EVALUATION_CONTEXT" name="SET_EVALUATION_CONTEXT" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="SET_IS_VISIBLE" name="SET_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>"#;

pub fn get_sets_response() -> String {
    discover_rowset_envelope("", SETS_ROW_FIELDS, "")
}

// ./src/tables.rs
use crate::response::{discover_rowset_envelope, UUID_TYPE};

const TABLE_ROW_FIELDS: &str = r#"                <xsd:element sql:field="TABLE_CATALOG" name="TABLE_CATALOG" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_SCHEMA" name="TABLE_SCHEMA" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_NAME" name="TABLE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_TYPE" name="TABLE_TYPE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_GUID" name="TABLE_GUID" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE_PROPID" name="TABLE_PROPID" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="DATE_CREATED" name="DATE_CREATED" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="DATE_MODIFIED" name="DATE_MODIFIED" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="TABLE_OLAP_TYPE" name="TABLE_OLAP_TYPE" type="xsd:string" minOccurs="0"/>"#;

const TABLE_ROWS: &str = r#"          <row>
            <TABLE_CATALOG>KTH_KEX_MALLOY_CUBE</TABLE_CATALOG>
            <TABLE_NAME>Faktatabell</TABLE_NAME>
            <TABLE_TYPE>TABLE</TABLE_TYPE>
            <TABLE_OLAP_TYPE>MEASURE_GROUP</TABLE_OLAP_TYPE>
          </row>
          <row>
            <TABLE_CATALOG>KTH_KEX_MALLOY_CUBE</TABLE_CATALOG>
            <TABLE_NAME>Produktkategori</TABLE_NAME>
            <TABLE_TYPE>TABLE</TABLE_TYPE>
            <TABLE_OLAP_TYPE>CUBE_DIMENSION</TABLE_OLAP_TYPE>
          </row>"#;

pub fn get_tables_response() -> String {
    discover_rowset_envelope(UUID_TYPE, TABLE_ROW_FIELDS, TABLE_ROWS)
}


// ./src/catalogs.rs
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

// ./src/cubes.rs
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

// ./src/dimensions.rs
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
          </row>"#;

pub fn get_dimensions_response() -> String {
    discover_rowset_envelope(UUID_TYPE, DIM_ROW_FIELDS, DIM_ROWS)
}

// ./src/execute.rs
use crate::response::wrap_in_soap_envelope;

pub fn get_empty_execute_response() -> String {
    wrap_in_soap_envelope(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:empty"/>
      </return>
    </ExecuteResponse>"#,
    )
}

/// Returns true when the statement looks like a DAX query (starts with EVALUATE,
/// optionally after DEFINE blocks/whitespace).
fn is_dax(statement: &str) -> bool {
    let trimmed = statement.trim_start();
    let upper = trimmed.to_uppercase();
    upper.starts_with("EVALUATE") || upper.starts_with("DEFINE")
}

pub fn get_execute_statement_response(statement: &str) -> String {
    if is_dax(statement) {
        get_execute_dax_response(statement)
    } else {
        get_execute_mdx_response(statement)
    }
}

fn get_execute_mdx_response(mdx: &str) -> String {
    let has_measures = mdx.contains("Measures") || mdx.contains("measures");
    let measure_name = "Total_Försäljning";
    let measure_value = if has_measures { "1250000.5" } else { "" };

    let inner = format!(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence minOccurs="0" maxOccurs="unbounded"><xsd:element name="row" type="row"/></xsd:sequence></xsd:complexType>
            </xsd:element>
            <xsd:complexType name="row">
              <xsd:sequence>
                <xsd:element sql:field="{name}" name="{name}" type="xsd:double" minOccurs="0"/>
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
          <row>
            <{name}>{val}</{name}>
          </row>
        </root>
      </return>
    </ExecuteResponse>"#,
        name = measure_name,
        val = measure_value,
    );
    wrap_in_soap_envelope(&inner)
}

/// Minimal DAX EVALUATE response: returns a single-row rowset with the
/// `Faktatabell[Total Försäljning (SEK)]` measure column.
fn get_execute_dax_response(_dax: &str) -> String {
    // DAX result columns are normally named `'Table'[Column]` — Excel will
    // accept the bracketed form. We use a column name aligned with the
    // measure caption so a drag-to-Values renders the expected number.
    let col_xml_name = "Faktatabell_x005B_Total_x0020_Försäljning_x0020__x0028_SEK_x0029__x005D_";
    let col_sql_field = "[Faktatabell].[Total Försäljning (SEK)]";

    let inner = format!(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence minOccurs="0" maxOccurs="unbounded"><xsd:element name="row" type="row"/></xsd:sequence></xsd:complexType>
            </xsd:element>
            <xsd:complexType name="row">
              <xsd:sequence>
                <xsd:element sql:field="{sqlf}" name="{xname}" type="xsd:double" minOccurs="0"/>
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
          <row>
            <{xname}>1250000.5</{xname}>
          </row>
        </root>
      </return>
    </ExecuteResponse>"#,
        sqlf = col_sql_field,
        xname = col_xml_name,
    );
    wrap_in_soap_envelope(&inner)
}

// ./src/hierarchies.rs
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
                <xsd:element sql:field="STRUCTURE_TYPE" name="STRUCTURE_TYPE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_SOURCE" name="CUBE_SOURCE" type="xsd:unsignedShort" minOccurs="0"/>"#;

const HIER_ROWS: &str = r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_NAME>Produktkategori</HIERARCHY_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori].[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <HIERARCHY_GUID>00000000-0000-0000-0000-000000000020</HIERARCHY_GUID>
            <HIERARCHY_CAPTION>Produktkategori</HIERARCHY_CAPTION>
            <DIMENSION_TYPE>3</DIMENSION_TYPE>
            <HIERARCHY_CARDINALITY>50</HIERARCHY_CARDINALITY>
            <DEFAULT_MEMBER>[Produktkategori].[Produktkategori].[All]</DEFAULT_MEMBER>
            <ALL_MEMBER>[Produktkategori].[Produktkategori].[All]</ALL_MEMBER>
            <STRUCTURE>0</STRUCTURE>
            <DIMENSION_IS_VISIBLE>true</DIMENSION_IS_VISIBLE>
            <HIERARCHY_ORDINAL>0</HIERARCHY_ORDINAL>
            <DIMENSION_IS_SHARED>true</DIMENSION_IS_SHARED>
            <HIERARCHY_IS_VISIBLE>true</HIERARCHY_IS_VISIBLE>
            <HIERARCHY_ORIGIN>2</HIERARCHY_ORIGIN>
            <HIERARCHY_DISPLAY_FOLDER></HIERARCHY_DISPLAY_FOLDER>
            <INSTANCE_SELECTION>0</INSTANCE_SELECTION>
            <GROUPING_BEHAVIOR>0</GROUPING_BEHAVIOR>
            <STRUCTURE_TYPE>Natural</STRUCTURE_TYPE>
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>"#;

pub fn get_hierarchies_response() -> String {
    discover_rowset_envelope(UUID_TYPE, HIER_ROW_FIELDS, HIER_ROWS)
}

// ./src/kpis.rs
use crate::response::discover_rowset_envelope;

const KPIS_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="KPI_NAME" name="KPI_NAME" type="xsd:string"/>
                <xsd:element sql:field="KPI_CAPTION" name="KPI_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_DESCRIPTION" name="KPI_DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_DISPLAY_FOLDER" name="KPI_DISPLAY_FOLDER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_VALUE" name="KPI_VALUE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_GOAL" name="KPI_GOAL" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_STATUS" name="KPI_STATUS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_TREND" name="KPI_TREND" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_STATUS_GRAPHIC" name="KPI_STATUS_GRAPHIC" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_TREND_GRAPHIC" name="KPI_TREND_GRAPHIC" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_WEIGHT" name="KPI_WEIGHT" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_CURRENT_TIME_MEMBER" name="KPI_CURRENT_TIME_MEMBER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="KPI_PARENT_KPI_NAME" name="KPI_PARENT_KPI_NAME" type="xsd:string" minOccurs="0"/>"#;

pub fn get_kpis_response() -> String {
    discover_rowset_envelope("", KPIS_ROW_FIELDS, "")
}

// ./src/levels.rs
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
                <xsd:element sql:field="LEVEL_ORIGIN" name="LEVEL_ORIGIN" type="xsd:unsignedShort" minOccurs="0"/>
                <xsd:element sql:field="CUBE_SOURCE" name="CUBE_SOURCE" type="xsd:unsignedShort" minOccurs="0"/>"#;

const LEVEL_ROWS: &str = r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori].[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_NAME>(All)</LEVEL_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori].[(All)]</LEVEL_UNIQUE_NAME>
            <LEVEL_GUID>00000000-0000-0000-0000-000000000030</LEVEL_GUID>
            <LEVEL_CAPTION>(All)</LEVEL_CAPTION>
            <LEVEL_NUMBER>0</LEVEL_NUMBER>
            <LEVEL_CARDINALITY>1</LEVEL_CARDINALITY>
            <LEVEL_TYPE>1</LEVEL_TYPE>
            <CUSTOM_ROLLUP_SETTINGS>0</CUSTOM_ROLLUP_SETTINGS>
            <LEVEL_UNIQUE_SETTINGS>1</LEVEL_UNIQUE_SETTINGS>
            <LEVEL_IS_VISIBLE>false</LEVEL_IS_VISIBLE>
            <LEVEL_DBTYPE>130</LEVEL_DBTYPE>
            <LEVEL_KEY_CARDINALITY>1</LEVEL_KEY_CARDINALITY>
            <LEVEL_ORIGIN>1</LEVEL_ORIGIN>
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori].[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_NAME>Produktkategori</LEVEL_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <LEVEL_GUID>00000000-0000-0000-0000-000000000031</LEVEL_GUID>
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
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>"#;

pub fn get_levels_response() -> String {
    discover_rowset_envelope(UUID_TYPE, LEVEL_ROW_FIELDS, LEVEL_ROWS)
}

// ./src/literals.rs
use crate::response::discover_rowset_envelope;

const LITERAL_ROW_FIELDS: &str = r#"                <xsd:element sql:field="LITERAL_NAME" name="LITERAL_NAME" type="xsd:string"/>
                <xsd:element sql:field="LITERAL_VALUE" name="LITERAL_VALUE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LITERAL_INVALID_CHARS" name="LITERAL_INVALID_CHARS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LITERAL_INVALID_STARTING_CHARS" name="LITERAL_INVALID_STARTING_CHARS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LITERAL_MAX_LENGTH" name="LITERAL_MAX_LENGTH" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="LITERAL_SUFFIX" name="LITERAL_SUFFIX" type="xsd:string" minOccurs="0"/>"#;

const LITERAL_ROWS: &str = r#"          <row><LITERAL_NAME>DBLITERAL_CATALOG_NAME</LITERAL_NAME><LITERAL_VALUE>KTH_KEX_MALLOY_CUBE</LITERAL_VALUE><LITERAL_MAX_LENGTH>128</LITERAL_MAX_LENGTH></row>
          <row><LITERAL_NAME>DBLITERAL_CATALOG_SEPARATOR</LITERAL_NAME><LITERAL_VALUE>.</LITERAL_VALUE></row>
          <row><LITERAL_NAME>DBLITERAL_QUOTE_PREFIX</LITERAL_NAME><LITERAL_VALUE>[</LITERAL_VALUE></row>
          <row><LITERAL_NAME>DBLITERAL_QUOTE_SUFFIX</LITERAL_NAME><LITERAL_VALUE>]</LITERAL_VALUE></row>
          <row><LITERAL_NAME>DBLITERAL_PASS_THROUGH_COLUMNS</LITERAL_NAME><LITERAL_VALUE>true</LITERAL_VALUE></row>"#;

pub fn get_literals_response() -> String {
    discover_rowset_envelope("", LITERAL_ROW_FIELDS, LITERAL_ROWS)
}

// ./src/main.rs
use axum::{
    http::{header, HeaderMap, HeaderName, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use std::net::SocketAddr;

mod parser;
mod response;
mod properties;
mod schema_rowsets;
mod catalogs;
mod cubes;
mod tables;
mod dimensions;
mod measures;
mod hierarchies;
mod levels;
mod mdschema_properties;
mod members;
mod literals;
mod sets;
mod kpis;
mod measure_groups;
mod measuregroup_dimensions;
mod execute;
mod tmschema;

use parser::{parse_xmla, XmlaRequest};

#[tokio::main]
async fn main() {
    let app = Router::new().route("/xmla", post(handle_xmla));
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("🚀 Rust-XMLA Proxy (v3 - ModuleRefactor) snurrar på http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "text/xml; charset=utf-8".parse().unwrap());
    headers.insert(header::SERVER, "Rust-Malloy-Proxy/2.0".parse().unwrap());
    headers.insert(header::CONNECTION, "close".parse().unwrap());
    headers.insert(
        HeaderName::from_static("x-transport-caps-negotiation-flags"),
        "0,0,0,0,0".parse().unwrap(),
    );
    headers
}

/// Extracts `<open>...</close>` (first occurrence) verbatim from `body`.
/// Returns the trimmed inner contents, or None if either tag is missing.
fn extract_block<'a>(body: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = body.find(open)? + open.len();
    let end = body[start..].find(close)? + start;
    Some(body[start..end].trim())
}

/// Print the `<RestrictionList>` and `<PropertyList>` blocks from a Discover
/// request body, when present. Helps us see what Excel is actually asking for.
fn log_discover_context(body: &str) {
    if let Some(restrictions) = extract_block(body, "<RestrictionList", "</RestrictionList>") {
        // <RestrictionList ...> — strip leading attrs up to the first '>' so we
        // print just the inner XML.
        let inner = match restrictions.find('>') {
            Some(idx) => restrictions[idx + 1..].trim(),
            None => restrictions,
        };
        if !inner.is_empty() {
            println!("🎯 RestrictionList:\n{}", inner);
        } else {
            println!("🎯 RestrictionList: (empty)");
        }
    }
    if let Some(properties) = extract_block(body, "<PropertyList", "</PropertyList>") {
        let inner = match properties.find('>') {
            Some(idx) => properties[idx + 1..].trim(),
            None => properties,
        };
        if !inner.is_empty() {
            println!("⚙️  PropertyList:\n{}", inner);
        }
    }
}

async fn handle_xmla(body: String) -> impl IntoResponse {
    if body.contains("<RequestType>") {
        let req_start = body.find("<RequestType>").unwrap() + 13;
        let req_end = body.find("</RequestType>").unwrap();
        println!("🔍 Rå RequestType från Excel: {}", &body[req_start..req_end]);
    }

    let headers = default_headers();
    let request = parse_xmla(&body);
    println!("📥 Fick anrop, tolkade som: {:?}", request);

    log_discover_context(&body);

    if body.contains("<Execute") {
        println!("🔍 Rå Execute från Excel:\n{}", body);
    }

    let response_body = match request {
        XmlaRequest::BeginSession | XmlaRequest::ExecuteEmpty => {
            execute::get_empty_execute_response()
        }

        XmlaRequest::DiscoverProperties { property_names } => {
            if property_names.len() == 1 && property_names[0] == "Catalog" {
                println!("Excel frågar efter Catalog");
                properties::get_single_property_response("Catalog", "KTH_KEX_MALLOY_CUBE")
            } else {
                println!("Excel frågar efter egenskaper: {:?}", property_names);
                properties::get_properties_response(&property_names)
            }
        }

        XmlaRequest::DiscoverSchemaRowsets => schema_rowsets::get_schemas_response(),
        XmlaRequest::DbSchemaCatalogs => catalogs::get_catalogs_response(),
        XmlaRequest::MdschemaCubes => cubes::get_cubes_response(),
        XmlaRequest::DbschemaTables => tables::get_tables_response(),
        XmlaRequest::MdschemaDimensions => {
            println!("📥 Skickar Dimensioner till Excel!");
            dimensions::get_dimensions_response()
        }
        XmlaRequest::MdschemaMeasures => {
            println!("📥 Skickar Measures till Excel!");
            measures::get_measures_response()
        }
        XmlaRequest::MdschemaHierarchies => {
            println!("📥 Hierarchies");
            hierarchies::get_hierarchies_response()
        }
        XmlaRequest::MdschemaLevels => {
            println!("📥 Levels");
            levels::get_levels_response()
        }
        XmlaRequest::ExecuteStatement(mdx) => {
            println!("📥 MDX Statement: {}", mdx);
            execute::get_execute_statement_response(&mdx)
        }
        XmlaRequest::MdschemaProperties { property_type } => {
            println!("📥 MDSCHEMA_PROPERTIES (PROPERTY_TYPE={:?})", property_type);
            mdschema_properties::get_mdschema_properties_response(property_type)
        }
        XmlaRequest::MdschemaMembers => {
            println!("📥 MDSCHEMA_MEMBERS");
            members::get_members_response()
        }
        XmlaRequest::DiscoverLiterals => {
            println!("📥 DISCOVER_LITERALS");
            literals::get_literals_response()
        }
        XmlaRequest::MdschemaSets => {
            println!("📥 MDSCHEMA_SETS");
            sets::get_sets_response()
        }
        XmlaRequest::MdschemaKpis => {
            println!("📥 MDSCHEMA_KPIS");
            kpis::get_kpis_response()
        }
        XmlaRequest::MdschemaMeasureGroups => {
            println!("📥 MDSCHEMA_MEASUREGROUPS");
            measure_groups::get_measure_groups_response()
        }
        XmlaRequest::MdschemaMeasureGroupDimensions => {
            println!("📥 MDSCHEMA_MEASUREGROUP_DIMENSIONS");
            measuregroup_dimensions::get_measuregroup_dimensions_response()
        }

        XmlaRequest::TmschemaModel => {
            println!("📥 TMSCHEMA_MODEL");
            tmschema::get_tmschema_model_response()
        }
        XmlaRequest::TmschemaTables => {
            println!("📥 TMSCHEMA_TABLES");
            tmschema::get_tmschema_tables_response()
        }
        XmlaRequest::TmschemaColumns => {
            println!("📥 TMSCHEMA_COLUMNS");
            tmschema::get_tmschema_columns_response()
        }
        XmlaRequest::TmschemaMeasures => {
            println!("📥 TMSCHEMA_MEASURES");
            tmschema::get_tmschema_measures_response()
        }
        XmlaRequest::TmschemaHierarchies => {
            println!("📥 TMSCHEMA_HIERARCHIES");
            tmschema::get_tmschema_hierarchies_response()
        }
        XmlaRequest::TmschemaLevels => {
            println!("📥 TMSCHEMA_LEVELS");
            tmschema::get_tmschema_levels_response()
        }
        XmlaRequest::TmschemaRelationships => {
            println!("📥 TMSCHEMA_RELATIONSHIPS");
            tmschema::get_tmschema_relationships_response()
        }
        XmlaRequest::TmschemaPartitions => {
            println!("📥 TMSCHEMA_PARTITIONS");
            tmschema::get_tmschema_partitions_response()
        }
        XmlaRequest::DiscoverXmlMetadata => {
            println!("📥 DISCOVER_XML_METADATA");
            tmschema::get_discover_xml_metadata_response()
        }
        XmlaRequest::DiscoverCalcDependency => {
            println!("📥 DISCOVER_CALC_DEPENDENCY");
            tmschema::get_discover_calc_dependency_response()
        }

        XmlaRequest::Unknown => {
            println!("❌ Okänt anrop.");
            return (StatusCode::BAD_REQUEST, headers, "Okänt anrop".to_string());
        }
    };

    (StatusCode::OK, headers, response_body)
}

// ./src/mdschema_properties.rs
use crate::response::discover_rowset_envelope;

const PROPERTIES_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="HIERARCHY_UNIQUE_NAME" name="HIERARCHY_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_UNIQUE_NAME" name="LEVEL_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEMBER_UNIQUE_NAME" name="MEMBER_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_NAME" name="PROPERTY_NAME" type="xsd:string"/>
                <xsd:element sql:field="PROPERTY_CAPTION" name="PROPERTY_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_DESCRIPTION" name="PROPERTY_DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_TYPE" name="PROPERTY_TYPE" type="xsd:short" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_CONTENT_TYPE" name="PROPERTY_CONTENT_TYPE" type="xsd:short" minOccurs="0"/>"#;

/// Helper: emit a property row anchored at a (dim, hier, level) for PROPERTY_TYPE=1
/// (intrinsic MEMBER properties). Filters in MDSCHEMA_PROPERTIES use
/// HIERARCHY_UNIQUE_NAME, so it MUST be present on every row.
fn member_property_row(
    dim: &str,
    hier: &str,
    level: &str,
    prop_name: &str,
    content_type: u8,
) -> String {
    format!(
        r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>{dim}</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>{hier}</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>{level}</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>{prop_name}</PROPERTY_NAME>
            <PROPERTY_CAPTION>{prop_name}</PROPERTY_CAPTION>
            <PROPERTY_TYPE>1</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>{content_type}</PROPERTY_CONTENT_TYPE>
          </row>"#,
    )
}

/// Intrinsic MEMBER properties (PROPERTY_TYPE=1) for the Produktkategori
/// hierarchy. Emitted at both levels of the hierarchy so Excel can construct
/// axis queries against either the (All) level or the leaf level.
fn member_property_rows() -> String {
    const HIER: &str = "[Produktkategori].[Produktkategori]";
    const DIM: &str = "[Produktkategori]";
    const LEVEL_ALL: &str = "[Produktkategori].[Produktkategori].[(All)]";
    const LEVEL_LEAF: &str = "[Produktkategori].[Produktkategori].[Produktkategori]";

    // Standard intrinsic member property names + their PROPERTY_CONTENT_TYPE.
    // Content types: 0 = Regular, 1 = Id, 2 = Relation_to_parent, 4 = Property_formatting.
    // We use 0 throughout; Excel doesn't appear to enforce subtypes here.
    let props: [(&str, u8); 12] = [
        ("MEMBER_CAPTION", 0),
        ("MEMBER_NAME", 0),
        ("MEMBER_UNIQUE_NAME", 0),
        ("MEMBER_KEY", 0),
        ("MEMBER_TYPE", 0),
        ("MEMBER_VALUE", 0),
        ("LEVEL_NUMBER", 0),
        ("LEVEL_UNIQUE_NAME", 0),
        ("PARENT_LEVEL", 0),
        ("PARENT_UNIQUE_NAME", 0),
        ("PARENT_COUNT", 0),
        ("CHILDREN_CARDINALITY", 0),
    ];

    let mut out = String::new();
    for level in [LEVEL_ALL, LEVEL_LEAF] {
        for (name, content) in props.iter() {
            out.push_str(&member_property_row(DIM, HIER, level, name, *content));
            out.push('\n');
        }
    }
    out
}

/// CELL properties (PROPERTY_TYPE=2). These are cube-scoped, not level-scoped,
/// so we omit `LEVEL_UNIQUE_NAME` entirely (the schema declares it minOccurs=0).
fn system_property_rows() -> String {
    let props: [(&str, u8); 8] = [
        ("VALUE", 0),
        ("FORMATTED_VALUE", 1),
        ("FORMAT_STRING", 2),
        ("FORE_COLOR", 2),
        ("BACK_COLOR", 2),
        ("FONT_NAME", 2),
        ("FONT_SIZE", 2),
        ("CELL_ORDINAL", 0),
    ];

    let mut out = String::new();
    for (name, content) in props.iter() {
        out.push_str(&format!(
            r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <PROPERTY_NAME>{name}</PROPERTY_NAME>
            <PROPERTY_CAPTION>{name}</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>{content}</PROPERTY_CONTENT_TYPE>
          </row>
"#,
        ));
    }
    out
}

/// MEMBER_VALUE-style rows (PROPERTY_TYPE=5). Two rows for the Produktkategori
/// hierarchy at the (All) and leaf levels. The Measures row was removed since
/// the corresponding level no longer exists.
fn member_value_rows() -> String {
    const DIM: &str = "[Produktkategori]";
    const HIER: &str = "[Produktkategori].[Produktkategori]";
    const LEVEL_ALL: &str = "[Produktkategori].[Produktkategori].[(All)]";
    const LEVEL_LEAF: &str = "[Produktkategori].[Produktkategori].[Produktkategori]";

    let mut out = String::new();
    for level in [LEVEL_ALL, LEVEL_LEAF] {
        out.push_str(&format!(
            r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>{DIM}</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>{HIER}</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>{level}</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>5</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>
"#,
        ));
    }
    out
}

pub fn get_mdschema_properties_response(property_type: Option<i32>) -> String {
    let rows = match property_type {
        Some(1) => member_property_rows(),
        Some(2) => system_property_rows(),
        Some(5) => member_value_rows(),
        _ => format!("{}\n{}", system_property_rows(), member_value_rows()),
    };
    discover_rowset_envelope("", PROPERTIES_ROW_FIELDS, &rows)
}

// ./src/measure_groups.rs
use crate::response::discover_rowset_envelope;

const MEASUREGROUP_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASUREGROUP_NAME" name="MEASUREGROUP_NAME" type="xsd:string"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="IS_WRITE_ENABLED" name="IS_WRITE_ENABLED" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="MEASUREGROUP_CAPTION" name="MEASUREGROUP_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASUREGROUP_CARDINALITY" name="MEASUREGROUP_CARDINALITY" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="MEASUREGROUP_SIZE" name="MEASUREGROUP_SIZE" type="xsd:long" minOccurs="0"/>"#;

pub fn get_measure_groups_response() -> String {
    discover_rowset_envelope("", MEASUREGROUP_ROW_FIELDS,
        r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <MEASUREGROUP_NAME>Faktatabell</MEASUREGROUP_NAME>
            <MEASUREGROUP_CAPTION>Faktatabell</MEASUREGROUP_CAPTION>
          </row>"#
    )
}

// ./src/measuregroup_dimensions.rs
use crate::response::discover_rowset_envelope;

const MG_DIM_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASUREGROUP_NAME" name="MEASUREGROUP_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASUREGROUP_CARDINALITY" name="MEASUREGROUP_CARDINALITY" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_CARDINALITY" name="DIMENSION_CARDINALITY" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_IS_VISIBLE" name="DIMENSION_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_IS_FACT_DIMENSION" name="DIMENSION_IS_FACT_DIMENSION" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_GRANULARITY" name="DIMENSION_GRANULARITY" type="xsd:string" minOccurs="0"/>"#;

pub fn get_measuregroup_dimensions_response() -> String {
    discover_rowset_envelope("", MG_DIM_ROW_FIELDS,
        r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <MEASUREGROUP_NAME>Faktatabell</MEASUREGROUP_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_IS_VISIBLE>false</DIMENSION_IS_VISIBLE>
            <DIMENSION_IS_FACT_DIMENSION>false</DIMENSION_IS_FACT_DIMENSION>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <MEASUREGROUP_NAME>Faktatabell</MEASUREGROUP_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_IS_VISIBLE>true</DIMENSION_IS_VISIBLE>
            <DIMENSION_IS_FACT_DIMENSION>false</DIMENSION_IS_FACT_DIMENSION>
          </row>"#
    )
}

// ./src/measures.rs
use crate::response::{discover_rowset_envelope, UUID_TYPE};

const MEASURE_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASURE_NAME" name="MEASURE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASURE_UNIQUE_NAME" name="MEASURE_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASURE_CAPTION" name="MEASURE_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_GUID" name="MEASURE_GUID" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_AGGREGATOR" name="MEASURE_AGGREGATOR" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DATA_TYPE" name="DATA_TYPE" type="xsd:unsignedShort" minOccurs="0"/>
                <xsd:element sql:field="NUMERIC_PRECISION" name="NUMERIC_PRECISION" type="xsd:unsignedShort" minOccurs="0"/>
                <xsd:element sql:field="NUMERIC_SCALE" name="NUMERIC_SCALE" type="xsd:short" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_UNITS" name="MEASURE_UNITS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="EXPRESSION" name="EXPRESSION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_IS_VISIBLE" name="MEASURE_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="LEVELS_LIST" name="LEVELS_LIST" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_NAME_SQL_COLUMN_NAME" name="MEASURE_NAME_SQL_COLUMN_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_UNQUALIFIED_CAPTION" name="MEASURE_UNQUALIFIED_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASUREGROUP_NAME" name="MEASUREGROUP_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEASURE_DISPLAY_FOLDER" name="MEASURE_DISPLAY_FOLDER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DEFAULT_FORMAT_STRING" name="DEFAULT_FORMAT_STRING" type="xsd:string" minOccurs="0"/>"#;

const MEASURE_ROWS: &str = r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <MEASURE_NAME>Total Försäljning</MEASURE_NAME>
            <MEASURE_UNIQUE_NAME>[Measures].[Total Försäljning]</MEASURE_UNIQUE_NAME>
            <MEASURE_CAPTION>Total Försäljning (SEK)</MEASURE_CAPTION>
            <MEASURE_GUID>00000000-0000-0000-0000-000000000040</MEASURE_GUID>
            <MEASURE_AGGREGATOR>1</MEASURE_AGGREGATOR>
            <DATA_TYPE>5</DATA_TYPE>
            <NUMERIC_PRECISION>18</NUMERIC_PRECISION>
            <NUMERIC_SCALE>2</NUMERIC_SCALE>
            <MEASURE_UNITS>SEK</MEASURE_UNITS>
            <DESCRIPTION>Vår totala försäljning</DESCRIPTION>
            <EXPRESSION>SUM('Faktatabell'[Sales])</EXPRESSION>
            <MEASURE_IS_VISIBLE>true</MEASURE_IS_VISIBLE>
            <MEASURE_UNQUALIFIED_CAPTION>Total Försäljning (SEK)</MEASURE_UNQUALIFIED_CAPTION>
            <MEASUREGROUP_NAME>Faktatabell</MEASUREGROUP_NAME>
            <MEASURE_DISPLAY_FOLDER></MEASURE_DISPLAY_FOLDER>
            <DEFAULT_FORMAT_STRING>#,##0.00 SEK</DEFAULT_FORMAT_STRING>
          </row>"#;

pub fn get_measures_response() -> String {
    discover_rowset_envelope(UUID_TYPE, MEASURE_ROW_FIELDS, MEASURE_ROWS)
}

// ./src/members.rs
use crate::response::discover_rowset_envelope;

const MEMBER_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="HIERARCHY_UNIQUE_NAME" name="HIERARCHY_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="LEVEL_UNIQUE_NAME" name="LEVEL_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="LEVEL_NUMBER" name="LEVEL_NUMBER" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="MEMBER_ORDINAL" name="MEMBER_ORDINAL" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="MEMBER_NAME" name="MEMBER_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEMBER_UNIQUE_NAME" name="MEMBER_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEMBER_TYPE" name="MEMBER_TYPE" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="MEMBER_CAPTION" name="MEMBER_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CHILDREN_CARDINALITY" name="CHILDREN_CARDINALITY" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="PARENT_LEVEL" name="PARENT_LEVEL" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="PARENT_COUNT" name="PARENT_COUNT" type="xsd:unsignedInt" minOccurs="0"/>"#;

const MEMBER_ROWS: &str = r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori].[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori].[(All)]</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>0</LEVEL_NUMBER>
            <MEMBER_ORDINAL>0</MEMBER_ORDINAL>
            <MEMBER_NAME>All</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>[Produktkategori].[Produktkategori].[All]</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>1</MEMBER_TYPE>
            <MEMBER_CAPTION>All</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>50</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>0</PARENT_LEVEL>
            <PARENT_COUNT>0</PARENT_COUNT>
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Produktkategori]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Produktkategori].[Produktkategori]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Produktkategori].[Produktkategori].[Produktkategori]</LEVEL_UNIQUE_NAME>
            <LEVEL_NUMBER>1</LEVEL_NUMBER>
            <MEMBER_ORDINAL>1</MEMBER_ORDINAL>
            <MEMBER_NAME>Kategori A</MEMBER_NAME>
            <MEMBER_UNIQUE_NAME>[Produktkategori].[Produktkategori].&amp;[Kategori A]</MEMBER_UNIQUE_NAME>
            <MEMBER_TYPE>3</MEMBER_TYPE>
            <MEMBER_CAPTION>Kategori A</MEMBER_CAPTION>
            <CHILDREN_CARDINALITY>0</CHILDREN_CARDINALITY>
            <PARENT_LEVEL>0</PARENT_LEVEL>
            <PARENT_COUNT>1</PARENT_COUNT>
          </row>"#;

pub fn get_members_response() -> String {
    discover_rowset_envelope("", MEMBER_ROW_FIELDS, MEMBER_ROWS)
}

// ./src/parser.rs
use quick_xml::events::Event;
use quick_xml::Reader;

#[derive(Debug, PartialEq)]
pub enum XmlaRequest {
    DiscoverProperties { property_names: Vec<String> },
    DiscoverSchemaRowsets,
    DiscoverLiterals,
    DbSchemaCatalogs,
    MdschemaCubes,
    DbschemaTables,
    MdschemaDimensions,
    MdschemaMeasures,
    MdschemaHierarchies,
    MdschemaLevels,
    MdschemaProperties { property_type: Option<i32> },
    MdschemaMembers,
    MdschemaSets,
    MdschemaKpis,
    MdschemaMeasureGroups,
    MdschemaMeasureGroupDimensions,
    TmschemaModel,
    TmschemaTables,
    TmschemaColumns,
    TmschemaMeasures,
    TmschemaHierarchies,
    TmschemaLevels,
    TmschemaRelationships,
    TmschemaPartitions,
    DiscoverXmlMetadata,
    DiscoverCalcDependency,
    BeginSession,
    ExecuteEmpty,
    ExecuteStatement(String),
    Unknown,
}

pub fn parse_xmla(xml: &str) -> XmlaRequest {
    let mut reader = Reader::from_str(xml);

    let mut in_request_type = false;
    let mut is_execute = false;
    let mut in_property_name = false;
    let mut in_statement = false;
    let mut is_begin_session = false;
    let mut in_property_type = false;

    let mut parsed_request_type = String::new();
    let mut requested_properties: Vec<String> = Vec::new();
    let mut statement_text = String::new();
    let mut property_type: Option<i32> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                match e.local_name().as_ref() {
                    b"RequestType" => in_request_type = true,
                    b"PropertyName" => in_property_name = true,
                    b"Statement" => in_statement = true,
                    b"BeginSession" | b"BeginGetSessionToken" => is_begin_session = true,
                    b"Execute" => is_execute = true,
                    b"PROPERTY_TYPE" => in_property_type = true,
                    _ => (),
                }
            }
            Ok(Event::Empty(ref e)) => {
                match e.local_name().as_ref() {
                    b"Execute" => is_execute = true,
                    _ => (),
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().trim().to_string();

                if !text.is_empty() {
                    if in_request_type {
                        parsed_request_type = text;
                    } else if in_property_name {
                        requested_properties.push(text);
                    } else if in_statement {
                        statement_text = text;
                    } else if in_property_type {
                        if let Ok(v) = text.parse::<i32>() {
                            property_type = Some(v);
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                match e.local_name().as_ref() {
                    b"RequestType" => in_request_type = false,
                    b"PropertyName" => in_property_name = false,
                    b"Statement" => in_statement = false,
                    b"PROPERTY_TYPE" => in_property_type = false,
                    _ => (),
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => (),
        }
    }

    match parsed_request_type.as_str() {
        "DISCOVER_PROPERTIES" => {
            return XmlaRequest::DiscoverProperties {
                property_names: requested_properties,
            }
        }
        "DISCOVER_SCHEMA_ROWSETS" => return XmlaRequest::DiscoverSchemaRowsets,
        "DISCOVER_LITERALS" => return XmlaRequest::DiscoverLiterals,
        "DBSCHEMA_CATALOGS" => return XmlaRequest::DbSchemaCatalogs,
        "MDSCHEMA_CUBES" => return XmlaRequest::MdschemaCubes,
        "DBSCHEMA_TABLES" => return XmlaRequest::DbschemaTables,
        "MDSCHEMA_DIMENSIONS" => return XmlaRequest::MdschemaDimensions,
        "MDSCHEMA_MEASURES" => return XmlaRequest::MdschemaMeasures,
        "MDSCHEMA_HIERARCHIES" => return XmlaRequest::MdschemaHierarchies,
        "MDSCHEMA_LEVELS" => return XmlaRequest::MdschemaLevels,
        "MDSCHEMA_PROPERTIES" => return XmlaRequest::MdschemaProperties { property_type },
        "MDSCHEMA_MEMBERS" => return XmlaRequest::MdschemaMembers,
        "MDSCHEMA_SETS" => return XmlaRequest::MdschemaSets,
        "MDSCHEMA_KPIS" => return XmlaRequest::MdschemaKpis,
        "MDSCHEMA_MEASUREGROUPS" => return XmlaRequest::MdschemaMeasureGroups,
        "MDSCHEMA_MEASUREGROUP_DIMENSIONS" => return XmlaRequest::MdschemaMeasureGroupDimensions,
        "TMSCHEMA_MODEL" => return XmlaRequest::TmschemaModel,
        "TMSCHEMA_TABLES" => return XmlaRequest::TmschemaTables,
        "TMSCHEMA_COLUMNS" => return XmlaRequest::TmschemaColumns,
        "TMSCHEMA_MEASURES" => return XmlaRequest::TmschemaMeasures,
        "TMSCHEMA_HIERARCHIES" => return XmlaRequest::TmschemaHierarchies,
        "TMSCHEMA_LEVELS" => return XmlaRequest::TmschemaLevels,
        "TMSCHEMA_RELATIONSHIPS" => return XmlaRequest::TmschemaRelationships,
        "TMSCHEMA_PARTITIONS" => return XmlaRequest::TmschemaPartitions,
        "DISCOVER_XML_METADATA" => return XmlaRequest::DiscoverXmlMetadata,
        "DISCOVER_CALC_DEPENDENCY" => return XmlaRequest::DiscoverCalcDependency,
        _ => (),
    };

    if is_execute {
        if !statement_text.is_empty() {
            return XmlaRequest::ExecuteStatement(statement_text);
        } else if is_begin_session {
            return XmlaRequest::BeginSession;
        } else {
            return XmlaRequest::ExecuteEmpty;
        }
    }

    XmlaRequest::Unknown
}

// ./src/properties.rs
use crate::response::{discover_rowset_envelope, UUID_TYPE};

struct Property {
    name: &'static str,
    description: &'static str,
    prop_type: &'static str,
    access_type: &'static str,
    is_required: bool,
    value: Option<&'static str>,
}

const PROPERTIES: &[Property] = &[
    Property {
        name: "ProviderName",
        description: "ProviderName",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("Min Riktiga Rust Proxy"),
    },
    Property {
        name: "DbpropMsmdSubqueries",
        description: "DbpropMsmdSubqueries",
        prop_type: "int",
        access_type: "ReadWrite",
        is_required: false,
        value: Some("2"),
    },
    Property {
        name: "DbpropMsmdOptimizeResponse",
        description: "DbpropMsmdOptimizeResponse",
        prop_type: "long",
        access_type: "ReadWrite",
        is_required: false,
        value: Some("0"),
    },
    Property {
        name: "DbpropMsmdActivityID",
        description: "DbpropMsmdActivityID",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: None,
    },
    Property {
        name: "DbpropMsmdCurrentActivityID",
        description: "DbpropMsmdCurrentActivityID",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: None,
    },
    Property {
        name: "ApplicationContext",
        description: "ApplicationContext",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: None,
    },
    Property {
        name: "Catalog",
        description: "Catalog",
        prop_type: "string",
        access_type: "ReadWrite",
        is_required: false,
        value: Some("KTH_KEX_MALLOY_CUBE"),
    },
    Property {
        name: "ServerName",
        description: "ServerName",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("rust-proxy"),
    },
    Property {
        name: "ProviderVersion",
        description: "ProviderVersion",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("1.0.0"),
    },
    Property {
        name: "MdpropMdxSubqueries",
        description: "MdpropMdxSubqueries",
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        value: Some("63"),
    },
    Property {
        name: "MdpropMdxDrillFunctions",
        description: "MdpropMdxDrillFunctions",
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        value: Some("7"),
    },
    Property {
        name: "MdpropMdxNamedSets",
        description: "MdpropMdxNamedSets",
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        value: Some("15"),
    },
    Property {
        name: "MdpropMdxDdlExtensions",
        description: "MdpropMdxDdlExtensions",
        prop_type: "int",
        access_type: "Read",
        is_required: false,
        value: Some("23"),
    },
    Property {
        name: "MDXSupport",
        description: "MDXSupport",
        prop_type: "string",
        access_type: "Read",
        is_required: false,
        value: Some("Core"),
    },
];

const PROPERTY_ROW_FIELDS: &str = r#"                <xsd:element sql:field="PropertyName" name="PropertyName" type="xsd:string"/>
                <xsd:element sql:field="PropertyDescription" name="PropertyDescription" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PropertyType" name="PropertyType" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PropertyAccessType" name="PropertyAccessType" type="xsd:string"/>
                <xsd:element sql:field="IsRequired" name="IsRequired" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="Value" name="Value" type="xsd:string" minOccurs="0"/>"#;

fn format_row(p: &Property) -> String {
    format!(
        r#"          <row>
            <PropertyName>{name}</PropertyName>
            <PropertyDescription>{desc}</PropertyDescription>
            <PropertyType>{ptype}</PropertyType>
            <PropertyAccessType>{access}</PropertyAccessType>
            <IsRequired>{req}</IsRequired>
            <Value>{val}</Value>
          </row>"#,
        name = p.name,
        desc = p.description,
        ptype = p.prop_type,
        access = p.access_type,
        req = p.is_required,
        val = p.value.unwrap_or(""),
    )
}

pub fn get_properties_response(filter: &[String]) -> String {
    let filtered: Vec<String> = PROPERTIES
        .iter()
        .filter(|p| filter.is_empty() || filter.iter().any(|f| f == p.name))
        .map(format_row)
        .collect();

    discover_rowset_envelope(UUID_TYPE, PROPERTY_ROW_FIELDS, &filtered.join("\n"))
}

pub fn get_single_property_response(name: &str, value: &str) -> String {
    let row = format!(
        r#"          <row>
            <PropertyName>{name}</PropertyName>
            <PropertyDescription>{name}</PropertyDescription>
            <PropertyType>string</PropertyType>
            <PropertyAccessType>ReadWrite</PropertyAccessType>
            <IsRequired>false</IsRequired>
            <Value>{value}</Value>
          </row>"#,
    );
    discover_rowset_envelope(UUID_TYPE, PROPERTY_ROW_FIELDS, &row)
}

// ./src/response.rs
pub fn wrap_in_soap_envelope(inner_xml: &str) -> String {
    format!(
        r#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Header>
    <Session xmlns="urn:schemas-microsoft-com:xml-analysis" SessionId="RUST-SESSION-456" />
  </soap:Header>
  <soap:Body>
{}
  </soap:Body>
</soap:Envelope>"#,
        inner_xml
    )
}

pub const UUID_TYPE: &str = r#"<xsd:simpleType name="uuid">
              <xsd:restriction base="xsd:string">
                <xsd:pattern value="[0-9a-zA-Z]{8}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{12}"/>
              </xsd:restriction>
            </xsd:simpleType>"#;

pub fn empty_discover_response() -> String {
    let inner = r#"    <DiscoverResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" />
        </root>
      </return>
    </DiscoverResponse>"#;
    wrap_in_soap_envelope(inner)
}

pub fn discover_rowset_envelope(extra_schema: &str, row_fields: &str, rows: &str) -> String {
    let inner = format!(
        r#"    <DiscoverResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence minOccurs="0" maxOccurs="unbounded"><xsd:element name="row" type="row"/></xsd:sequence></xsd:complexType>
            </xsd:element>
{extra_schema}
            <xsd:complexType name="row">
              <xsd:sequence>
{row_fields}
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
{rows}
        </root>
      </return>
    </DiscoverResponse>"#,
    );
    wrap_in_soap_envelope(&inner)
}

// ./src/schema_rowsets.rs
use crate::response::{discover_rowset_envelope, UUID_TYPE};

const SCHEMA_ROW_FIELDS: &str = r#"                <xsd:element sql:field="SchemaName" name="SchemaName" type="xsd:string"/>
                <xsd:element sql:field="SchemaGuid" name="SchemaGuid" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="Restrictions" name="Restrictions" minOccurs="0" maxOccurs="unbounded">
                  <xsd:complexType>
                    <xsd:sequence>
                      <xsd:element sql:field="Name" name="Name" type="xsd:string" minOccurs="0"/>
                      <xsd:element sql:field="Type" name="Type" type="xsd:string" minOccurs="0"/>
                    </xsd:sequence>
                  </xsd:complexType>
                </xsd:element>
                <xsd:element sql:field="RestrictionsMask" name="RestrictionsMask" type="xsd:unsignedLong" minOccurs="0"/>"#;

const SCHEMA_ROWSET_DATA: &str = r#"          <row>
            <SchemaName>DBSCHEMA_CATALOGS</SchemaName>
            <SchemaGuid>C8B52211-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DBSCHEMA_TABLES</SchemaName>
            <SchemaGuid>C8B52229-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>TABLE_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_OLAP_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DBSCHEMA_COLUMNS</SchemaName>
            <SchemaGuid>C8B52214-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>TABLE_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TABLE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>COLUMN_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>COLUMN_OLAP_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DBSCHEMA_PROVIDER_TYPES</SchemaName>
            <SchemaGuid>C8B5222C-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>DATA_TYPE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>BEST_MATCH</Name><Type>xsd:boolean</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_CUBES</SchemaName>
            <SchemaGuid>C8B522D8-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>BASE_CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_DIMENSIONS</SchemaName>
            <SchemaGuid>C8B522D9-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>DIMENSION_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>127</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_HIERARCHIES</SchemaName>
            <SchemaGuid>C8B522DA-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_ORIGIN</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>511</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_LEVELS</SchemaName>
            <SchemaGuid>C8B522DB-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_ORIGIN</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>LEVEL_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>1023</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_MEASURES</SchemaName>
            <SchemaGuid>C8B522DC-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEASURE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEASURE_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEASUREGROUP_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>MEASURE_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>255</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_PROPERTIES</SchemaName>
            <SchemaGuid>C8B522DD-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEMBER_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>PROPERTY_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>PROPERTY_TYPE</Name><Type>xsd:short</Type></Restrictions>
            <Restrictions><Name>PROPERTY_CONTENT_TYPE</Name><Type>xsd:short</Type></Restrictions>
            <Restrictions><Name>PROPERTY_ORIGIN</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>PROPERTY_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>8191</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_MEMBERS</SchemaName>
            <SchemaGuid>C8B522DE-5CF3-11CE-ADE5-00AA0044773D</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LEVEL_NUMBER</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <Restrictions><Name>MEMBER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEMBER_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEMBER_CAPTION</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEMBER_TYPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>TREE_OP</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>SCOPE</Name><Type>xsd:int</Type></Restrictions>
            <RestrictionsMask>16383</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_FUNCTIONS</SchemaName>
            <SchemaGuid>A07CCD07-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>LIBRARY_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>INTERFACE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>FUNCTION_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ORIGIN</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_ACTIONS</SchemaName>
            <SchemaGuid>A07CCD08-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ACTION_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ACTION_TYPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>COORDINATE</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>COORDINATE_TYPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>INVOCATION</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>511</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_SETS</SchemaName>
            <SchemaGuid>A07CCD0B-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SET_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCOPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>HIERARCHY_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>SET_EVALUATION_CONTEXT</Name><Type>xsd:int</Type></Restrictions>
            <RestrictionsMask>255</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_INSTANCES</SchemaName>
            <SchemaGuid>20518699-2474-4C15-9885-0E947EC7A7E3</SchemaGuid>
            <Restrictions><Name>INSTANCE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_KPIS</SchemaName>
            <SchemaGuid>2AE44109-ED3D-4842-B16F-B694D1CB0E3F</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>KPI_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_SOURCE</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <Restrictions><Name>SCOPE</Name><Type>xsd:int</Type></Restrictions>
            <RestrictionsMask>63</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_MEASUREGROUPS</SchemaName>
            <SchemaGuid>E1625EBF-FA96-42FD-BEA6-DB90ADAFD96B</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEASUREGROUP_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_MEASUREGROUP_DIMENSIONS</SchemaName>
            <SchemaGuid>A07CCD33-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CUBE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MEASUREGROUP_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DIMENSION_VISIBILITY</Name><Type>xsd:unsignedShort</Type></Restrictions>
            <RestrictionsMask>63</RestrictionsMask>
          </row>
          <row>
            <SchemaName>MDSCHEMA_INPUT_DATASOURCES</SchemaName>
            <SchemaGuid>A07CCD32-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SCHEMA_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DATASOURCE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DATASOURCE_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_SERVICES</SchemaName>
            <SchemaGuid>3ADD8A95-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>SERVICE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SERVICE_TYPE_ID</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_SERVICE_PARAMETERS</SchemaName>
            <SchemaGuid>3ADD8A75-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>SERVICE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>PARAMETER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_FUNCTIONS</SchemaName>
            <SchemaGuid>3ADD8A79-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>SERVICE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>FUNCTION_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_MODEL_CONTENT</SchemaName>
            <SchemaGuid>3ADD8A76-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>MODEL_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ATTRIBUTE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>NODE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>NODE_UNIQUE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>NODE_TYPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>NODE_GUID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>NODE_CAPTION</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TREE_OPERATION</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <RestrictionsMask>1023</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_MODEL_XML</SchemaName>
            <SchemaGuid>4290B2D5-0E9C-4AA7-9369-98C95CFD9D13</SchemaGuid>
            <Restrictions><Name>MODEL_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_MODEL_CONTENT_PMML</SchemaName>
            <SchemaGuid>4290B2D5-0E9C-4AA7-9369-98C95CFD9D13</SchemaGuid>
            <Restrictions><Name>MODEL_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_MODELS</SchemaName>
            <SchemaGuid>3ADD8A77-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>MODEL_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SERVICE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SERVICE_TYPE_ID</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <Restrictions><Name>MINING_STRUCTURE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>127</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_COLUMNS</SchemaName>
            <SchemaGuid>3ADD8A78-D8B9-11D2-8D2A-00E029154FDE</SchemaGuid>
            <Restrictions><Name>MODEL_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MODEL_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>COLUMN_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_STRUCTURES</SchemaName>
            <SchemaGuid>883269F3-0CAD-462F-B6F5-E88A72418C4B</SchemaGuid>
            <Restrictions><Name>STRUCTURE_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>STRUCTURE_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>STRUCTURE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>7</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DMSCHEMA_MINING_STRUCTURE_COLUMNS</SchemaName>
            <SchemaGuid>9952E836-BFBF-4D1F-8535-9B67DBD9DDFE</SchemaGuid>
            <Restrictions><Name>STRUCTURE_CATALOG</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>STRUCTURE_SCHEMA</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>STRUCTURE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>COLUMN_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_DATASOURCES</SchemaName>
            <SchemaGuid>06C03D41-F66D-49F3-B1B8-987F7AF4CF18</SchemaGuid>
            <Restrictions><Name>DataSourceName</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>URL</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ProviderName</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ProviderType</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>AuthenticationMode</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_PROPERTIES</SchemaName>
            <SchemaGuid>4B40ADFB-8B09-4758-97BB-636E8AE97BCF</SchemaGuid>
            <Restrictions><Name>PropertyName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_SCHEMA_ROWSETS</SchemaName>
            <SchemaGuid>EEA0302B-7922-4992-8991-0E605D0E5593</SchemaGuid>
            <Restrictions><Name>SchemaName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_ENUMERATORS</SchemaName>
            <SchemaGuid>55A9E78B-ACCB-45B4-95A6-94C5065617A7</SchemaGuid>
            <Restrictions><Name>EnumName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_KEYWORDS</SchemaName>
            <SchemaGuid>1426C443-4CDD-4A40-8F45-572FAB9BBAA1</SchemaGuid>
            <Restrictions><Name>Keyword</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_LITERALS</SchemaName>
            <SchemaGuid>C3EF5ECB-0A07-4665-A140-B075722DBDC2</SchemaGuid>
            <Restrictions><Name>LiteralName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XML_METADATA</SchemaName>
            <SchemaGuid>3444B255-171E-4CB9-AD98-19E57888A75F</SchemaGuid>
            <Restrictions><Name>DatabaseID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DimensionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CubeID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MeasureGroupID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>PartitionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>PerspectiveID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DimensionPermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>RoleID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DatabasePermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MiningModelID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MiningModelPermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DataSourceID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MiningStructureID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>AggregationDesignID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TraceID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MiningStructurePermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CubePermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>AssemblyID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>MdxScriptID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DataSourceViewID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DataSourcePermissionID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CalculatedColumns</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ObjectExpansion</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>DBWorkloadGroupID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ResourcePoolID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ModifiedAfter</Name><Type>xsd:dateTime</Type></Restrictions>
            <RestrictionsMask>67108863</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_TRACES</SchemaName>
            <SchemaGuid>A07CCD1A-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>TraceID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>Type</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_TRACE_DEFINITION_PROVIDERINFO</SchemaName>
            <SchemaGuid>A07CCD1B-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>Data</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XEVENT_PACKAGES</SchemaName>
            <SchemaGuid>A07CCD1C-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>ID</Name><Type>uuid</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XEVENT_OBJECTS</SchemaName>
            <SchemaGuid>A07CCD1D-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>OBJECT_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XEVENT_OBJECT_COLUMNS</SchemaName>
            <SchemaGuid>A07CCD1E-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>OBJECT_NAME</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XEVENT_SESSION_TARGETS</SchemaName>
            <SchemaGuid>A07CCD1F-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>XESessionName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_XEVENT_SESSIONS</SchemaName>
            <SchemaGuid>A07CCD20-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>XESessionName</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_TRACE_COLUMNS</SchemaName>
            <SchemaGuid>A07CCD18-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>Data</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_TRACE_EVENT_CATEGORIES</SchemaName>
            <SchemaGuid>A07CCD19-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>Data</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_MEMORYUSAGE</SchemaName>
            <SchemaGuid>A07CCD21-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>SPID</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <Restrictions><Name>MemoryUsed</Name><Type>xsd:long</Type></Restrictions>
            <Restrictions><Name>BaseObjectType</Name><Type>xsd:unsignedInt</Type></Restrictions>
            <Restrictions><Name>Shrinkable</Name><Type>xsd:boolean</Type></Restrictions>
            <RestrictionsMask>15</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_MEMORYGRANT</SchemaName>
            <SchemaGuid>A07CCD23-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>SPID</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_LOCKS</SchemaName>
            <SchemaGuid>A07CCD24-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>SPID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>LOCK_TRANSACTION_ID</Name><Type>uuid</Type></Restrictions>
            <Restrictions><Name>LOCK_OBJECT_ID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>LOCK_STATUS</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>LOCK_TYPE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>LOCK_MIN_TOTAL_MS</Name><Type>xsd:long</Type></Restrictions>
            <RestrictionsMask>63</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_CONNECTIONS</SchemaName>
            <SchemaGuid>A07CCD25-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CONNECTION_ID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CONNECTION_USER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CONNECTION_IMPERSONATED_USER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CONNECTION_HOST_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CONNECTION_ELAPSED_TIME_MS</Name><Type>xsd:long</Type></Restrictions>
            <Restrictions><Name>CONNECTION_LAST_COMMAND_ELAPSED_TIME_MS</Name><Type>xsd:long</Type></Restrictions>
            <Restrictions><Name>CONNECTION_IDLE_TIME_MS</Name><Type>xsd:long</Type></Restrictions>
            <RestrictionsMask>127</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_SESSIONS</SchemaName>
            <SchemaGuid>A07CCD26-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>SESSION_ID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SESSION_SPID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>SESSION_CONNECTION_ID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>SESSION_USER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SESSION_CURRENT_DATABASE</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>SESSION_ELAPSED_TIME_MS</Name><Type>xsd:unsignedLong</Type></Restrictions>
            <Restrictions><Name>SESSION_CPU_TIME_MS</Name><Type>xsd:unsignedLong</Type></Restrictions>
            <Restrictions><Name>SESSION_IDLE_TIME_MS</Name><Type>xsd:unsignedLong</Type></Restrictions>
            <Restrictions><Name>SESSION_STATUS</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>RESTRICT_CATALOG_ID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>REQUEST_ACTIVITY_ID</Name><Type>uuid</Type></Restrictions>
            <Restrictions><Name>CLIENT_ACTIVITY_ID</Name><Type>uuid</Type></Restrictions>
            <RestrictionsMask>4095</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_JOBS</SchemaName>
            <SchemaGuid>A07CCD27-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>SPID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>JOB_ID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>JOB_DESCRIPTION</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>JOB_THREADPOOL_ID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>JOB_MIN_TOTAL_TIME_MS</Name><Type>xsd:long</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_TRANSACTIONS</SchemaName>
            <SchemaGuid>A07CCD28-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>TRANSACTION_ID</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>TRANSACTION_SESSION_ID</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_DB_CONNECTIONS</SchemaName>
            <SchemaGuid>A07CCD2A-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>CONNECTION_ID</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CONNECTION_IN_USE</Name><Type>xsd:int</Type></Restrictions>
            <Restrictions><Name>CONNECTION_SERVER_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CONNECTION_CATALOG_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>CONNECTION_SPID</Name><Type>xsd:int</Type></Restrictions>
            <RestrictionsMask>31</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_MASTER_KEY</SchemaName>
            <SchemaGuid>A07CCD29-8148-11D0-87BB-00C04FC33942</SchemaGuid>
            <Restrictions><Name>KEY</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>TMSCHEMA_MODEL</SchemaName>
            <SchemaGuid>F1B5C3AB-7CD1-4F77-89A8-9DE3D0C9DBC0</SchemaGuid>
            <Restrictions><Name>ID</Name><Type>xsd:long</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>TMSCHEMA_TABLES</SchemaName>
            <SchemaGuid>F1B5C3AC-7CD1-4F77-89A8-9DE3D0C9DBC0</SchemaGuid>
            <Restrictions><Name>ID</Name><Type>xsd:long</Type></Restrictions>
            <Restrictions><Name>Name</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>TMSCHEMA_COLUMNS</SchemaName>
            <SchemaGuid>F1B5C3AD-7CD1-4F77-89A8-9DE3D0C9DBC0</SchemaGuid>
            <Restrictions><Name>ID</Name><Type>xsd:long</Type></Restrictions>
            <Restrictions><Name>TableID</Name><Type>xsd:long</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>TMSCHEMA_MEASURES</SchemaName>
            <SchemaGuid>F1B5C3AE-7CD1-4F77-89A8-9DE3D0C9DBC0</SchemaGuid>
            <Restrictions><Name>ID</Name><Type>xsd:long</Type></Restrictions>
            <Restrictions><Name>TableID</Name><Type>xsd:long</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>TMSCHEMA_HIERARCHIES</SchemaName>
            <SchemaGuid>F1B5C3AF-7CD1-4F77-89A8-9DE3D0C9DBC0</SchemaGuid>
            <Restrictions><Name>ID</Name><Type>xsd:long</Type></Restrictions>
            <Restrictions><Name>TableID</Name><Type>xsd:long</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>TMSCHEMA_LEVELS</SchemaName>
            <SchemaGuid>F1B5C3B0-7CD1-4F77-89A8-9DE3D0C9DBC0</SchemaGuid>
            <Restrictions><Name>ID</Name><Type>xsd:long</Type></Restrictions>
            <Restrictions><Name>HierarchyID</Name><Type>xsd:long</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>TMSCHEMA_RELATIONSHIPS</SchemaName>
            <SchemaGuid>F1B5C3B1-7CD1-4F77-89A8-9DE3D0C9DBC0</SchemaGuid>
            <Restrictions><Name>ID</Name><Type>xsd:long</Type></Restrictions>
            <RestrictionsMask>1</RestrictionsMask>
          </row>
          <row>
            <SchemaName>TMSCHEMA_PARTITIONS</SchemaName>
            <SchemaGuid>F1B5C3B2-7CD1-4F77-89A8-9DE3D0C9DBC0</SchemaGuid>
            <Restrictions><Name>ID</Name><Type>xsd:long</Type></Restrictions>
            <Restrictions><Name>TableID</Name><Type>xsd:long</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
          <row>
            <SchemaName>DISCOVER_CALC_DEPENDENCY</SchemaName>
            <SchemaGuid>F1B5C3B3-7CD1-4F77-89A8-9DE3D0C9DBC0</SchemaGuid>
            <Restrictions><Name>DATABASE_NAME</Name><Type>xsd:string</Type></Restrictions>
            <Restrictions><Name>OBJECT_TYPE</Name><Type>xsd:string</Type></Restrictions>
            <RestrictionsMask>3</RestrictionsMask>
          </row>
"#;

pub fn get_schemas_response() -> String {
    discover_rowset_envelope(UUID_TYPE, SCHEMA_ROW_FIELDS, SCHEMA_ROWSET_DATA)
}

// ./src/sets.rs
use crate::response::discover_rowset_envelope;

const SETS_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="SET_NAME" name="SET_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCOPE" name="SCOPE" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="EXPRESSION" name="EXPRESSION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DIMENSIONS" name="DIMENSIONS" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="SET_CAPTION" name="SET_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="SET_DISPLAY_FOLDER" name="SET_DISPLAY_FOLDER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="SET_EVALUATION_CONTEXT" name="SET_EVALUATION_CONTEXT" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="SET_IS_VISIBLE" name="SET_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>"#;

pub fn get_sets_response() -> String {
    discover_rowset_envelope("", SETS_ROW_FIELDS, "")
}

// ./src/tables.rs
use crate::response::{discover_rowset_envelope, UUID_TYPE};

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
          </row>"#;

pub fn get_tables_response() -> String {
    discover_rowset_envelope(UUID_TYPE, TABLE_ROW_FIELDS, TABLE_ROWS)
}

// ./src/tmschema.rs
use crate::response::discover_rowset_envelope;

/// Helper to build a TMSCHEMA_* envelope with one column declared (ID) and arbitrary rows.
fn tm_envelope(row_fields: &str, rows: &str) -> String {
    discover_rowset_envelope("", row_fields, rows)
}

const ID_ONLY_FIELDS: &str = r#"                <xsd:element sql:field="ID" name="ID" type="xsd:long" minOccurs="0"/>"#;

// -------- TMSCHEMA_MODEL: 1 row --------
pub fn get_tmschema_model_response() -> String {
    let row_fields = r#"                <xsd:element sql:field="ID" name="ID" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="Name" name="Name" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="Description" name="Description" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="Culture" name="Culture" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="ModifiedTime" name="ModifiedTime" type="xsd:dateTime" minOccurs="0"/>"#;
    let rows = r#"          <row>
            <ID>1</ID>
            <Name>Model</Name>
            <Description>Tabular model exposed by Rust XMLA proxy</Description>
            <Culture>sv-SE</Culture>
            <ModifiedTime>2026-05-20T12:00:00</ModifiedTime>
          </row>"#;
    tm_envelope(row_fields, rows)
}

// -------- TMSCHEMA_TABLES: Faktatabell + Produktkategori --------
pub fn get_tmschema_tables_response() -> String {
    let row_fields = r#"                <xsd:element sql:field="ID" name="ID" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="ModelID" name="ModelID" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="Name" name="Name" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DataCategory" name="DataCategory" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="Description" name="Description" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="IsHidden" name="IsHidden" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="TableStorageID" name="TableStorageID" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="ModifiedTime" name="ModifiedTime" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="StructureModifiedTime" name="StructureModifiedTime" type="xsd:dateTime" minOccurs="0"/>
                <xsd:element sql:field="IsPrivate" name="IsPrivate" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="ShowAsVariationsOnly" name="ShowAsVariationsOnly" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="SystemFlags" name="SystemFlags" type="xsd:long" minOccurs="0"/>"#;
    let rows = r#"          <row>
            <ID>2</ID>
            <ModelID>1</ModelID>
            <Name>Faktatabell</Name>
            <Description>Fact table</Description>
            <IsHidden>false</IsHidden>
            <ModifiedTime>2026-05-20T12:00:00</ModifiedTime>
            <StructureModifiedTime>2026-05-20T12:00:00</StructureModifiedTime>
            <IsPrivate>false</IsPrivate>
            <ShowAsVariationsOnly>false</ShowAsVariationsOnly>
            <SystemFlags>0</SystemFlags>
          </row>
          <row>
            <ID>3</ID>
            <ModelID>1</ModelID>
            <Name>Produktkategori</Name>
            <Description>Product category dimension</Description>
            <IsHidden>false</IsHidden>
            <ModifiedTime>2026-05-20T12:00:00</ModifiedTime>
            <StructureModifiedTime>2026-05-20T12:00:00</StructureModifiedTime>
            <IsPrivate>false</IsPrivate>
            <ShowAsVariationsOnly>false</ShowAsVariationsOnly>
            <SystemFlags>0</SystemFlags>
          </row>"#;
    tm_envelope(row_fields, rows)
}

// -------- TMSCHEMA_COLUMNS: empty stub --------
pub fn get_tmschema_columns_response() -> String {
    tm_envelope(ID_ONLY_FIELDS, "")
}

// -------- TMSCHEMA_MEASURES: empty stub (real measure lives in MDSCHEMA_MEASURES) --------
pub fn get_tmschema_measures_response() -> String {
    tm_envelope(ID_ONLY_FIELDS, "")
}

// -------- TMSCHEMA_HIERARCHIES: empty stub --------
pub fn get_tmschema_hierarchies_response() -> String {
    tm_envelope(ID_ONLY_FIELDS, "")
}

// -------- TMSCHEMA_LEVELS: empty stub --------
pub fn get_tmschema_levels_response() -> String {
    tm_envelope(ID_ONLY_FIELDS, "")
}

// -------- TMSCHEMA_RELATIONSHIPS: 1 row (Faktatabell.ProductKey → Produktkategori.ProductKey) --------
pub fn get_tmschema_relationships_response() -> String {
    let row_fields = r#"                <xsd:element sql:field="ID" name="ID" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="ModelID" name="ModelID" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="Name" name="Name" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="FromTableID" name="FromTableID" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="FromColumnID" name="FromColumnID" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="FromCardinality" name="FromCardinality" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="ToTableID" name="ToTableID" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="ToColumnID" name="ToColumnID" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="ToCardinality" name="ToCardinality" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="IsActive" name="IsActive" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="CrossFilteringBehavior" name="CrossFilteringBehavior" type="xsd:long" minOccurs="0"/>
                <xsd:element sql:field="ModifiedTime" name="ModifiedTime" type="xsd:dateTime" minOccurs="0"/>"#;
    let rows = r#"          <row>
            <ID>10</ID>
            <ModelID>1</ModelID>
            <Name>Faktatabell_Produktkategori</Name>
            <FromTableID>2</FromTableID>
            <FromColumnID>20</FromColumnID>
            <FromCardinality>2</FromCardinality>
            <ToTableID>3</ToTableID>
            <ToColumnID>30</ToColumnID>
            <ToCardinality>1</ToCardinality>
            <IsActive>true</IsActive>
            <CrossFilteringBehavior>1</CrossFilteringBehavior>
            <ModifiedTime>2026-05-20T12:00:00</ModifiedTime>
          </row>"#;
    tm_envelope(row_fields, rows)
}

// -------- TMSCHEMA_PARTITIONS: empty stub --------
pub fn get_tmschema_partitions_response() -> String {
    tm_envelope(ID_ONLY_FIELDS, "")
}

// -------- DISCOVER_XML_METADATA: empty rowset stub --------
pub fn get_discover_xml_metadata_response() -> String {
    let row_fields = r#"                <xsd:element sql:field="METADATA" name="METADATA" type="xsd:string" minOccurs="0"/>"#;
    tm_envelope(row_fields, "")
}

// -------- DISCOVER_CALC_DEPENDENCY: empty stub --------
pub fn get_discover_calc_dependency_response() -> String {
    let row_fields = r#"                <xsd:element sql:field="DATABASE_NAME" name="DATABASE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="OBJECT_TYPE" name="OBJECT_TYPE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="TABLE" name="TABLE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="OBJECT" name="OBJECT" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="EXPRESSION" name="EXPRESSION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="REFERENCED_OBJECT_TYPE" name="REFERENCED_OBJECT_TYPE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="REFERENCED_TABLE" name="REFERENCED_TABLE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="REFERENCED_OBJECT" name="REFERENCED_OBJECT" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="REFERENCED_EXPRESSION" name="REFERENCED_EXPRESSION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="QUERY" name="QUERY" type="xsd:string" minOccurs="0"/>"#;
    tm_envelope(row_fields, "")
}

