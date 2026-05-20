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
