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
