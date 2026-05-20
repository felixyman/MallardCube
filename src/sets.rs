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
