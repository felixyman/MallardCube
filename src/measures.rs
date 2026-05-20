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
