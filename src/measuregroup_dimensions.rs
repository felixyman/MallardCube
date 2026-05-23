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
          </row>
          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <MEASUREGROUP_NAME>Faktatabell</MEASUREGROUP_NAME>
            <DIMENSION_UNIQUE_NAME>[Region]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_IS_VISIBLE>true</DIMENSION_IS_VISIBLE>
            <DIMENSION_IS_FACT_DIMENSION>false</DIMENSION_IS_FACT_DIMENSION>
          </row>"#
    )
}
