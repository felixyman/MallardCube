use crate::response::{discover_rowset_envelope, UUID_TYPE};
use crate::engine::model::default_model;

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

pub fn get_measures_response() -> String {
    let model = default_model();
    let mut rows = String::new();
    for (i, m) in model.measures.iter().enumerate() {
        rows.push_str(&format!(
            r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <MEASURE_NAME>{}</MEASURE_NAME>
            <MEASURE_UNIQUE_NAME>{}</MEASURE_UNIQUE_NAME>
            <MEASURE_CAPTION>{}</MEASURE_CAPTION>
            <MEASURE_GUID>00000000-0000-0000-0000-{:012}</MEASURE_GUID>
            <MEASURE_AGGREGATOR>{}</MEASURE_AGGREGATOR>
            <DATA_TYPE>5</DATA_TYPE>
            <NUMERIC_PRECISION>{}</NUMERIC_PRECISION>
            <NUMERIC_SCALE>{}</NUMERIC_SCALE>
            <MEASURE_UNITS>{}</MEASURE_UNITS>
            <DESCRIPTION>{}</DESCRIPTION>
            <EXPRESSION>{}</EXPRESSION>
            <MEASURE_IS_VISIBLE>{}</MEASURE_IS_VISIBLE>
            <MEASURE_UNQUALIFIED_CAPTION>{}</MEASURE_UNQUALIFIED_CAPTION>
            <MEASUREGROUP_NAME>{}</MEASUREGROUP_NAME>
            <MEASURE_DISPLAY_FOLDER></MEASURE_DISPLAY_FOLDER>
            <DEFAULT_FORMAT_STRING>{}</DEFAULT_FORMAT_STRING>
          </row>
"#,
            m.caption,
            m.measure_unique_name(),
            m.display_name,
            40 + i,
            m.aggregator,
            m.numeric_precision,
            m.numeric_scale,
            m.units,
            m.description,
            m.expression,
            m.visible,
            m.display_name,
            m.measure_group_name,
            m.format_string,
        ));
    }

    discover_rowset_envelope(UUID_TYPE, MEASURE_ROW_FIELDS, &rows)
}
