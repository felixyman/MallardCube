use crate::response::discover_rowset_envelope;
use crate::engine::model::default_model;
use std::collections::BTreeSet;

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
    let model = default_model();
    let mut rows = String::new();
    let mut seen = BTreeSet::new();
    for m in model.measures {
        if seen.insert(m.measure_group_name.clone()) {
            rows.push_str(&format!(
                r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <MEASUREGROUP_NAME>{}</MEASUREGROUP_NAME>
            <MEASUREGROUP_CAPTION>{}</MEASUREGROUP_CAPTION>
          </row>
"#,
                m.measure_group_name,
                m.measure_group_name,
            ));
        }
    }

    discover_rowset_envelope("", MEASUREGROUP_ROW_FIELDS, &rows)
}
