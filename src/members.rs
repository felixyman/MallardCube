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
