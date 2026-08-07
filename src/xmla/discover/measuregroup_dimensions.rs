use crate::proxy_project;
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
    let project = proxy_project::project();
    let model = &project.model;
    let mut rows = String::new();

    let mut seen_groups = std::collections::BTreeSet::new();
    for ft in &model.fact_tables {
        if !seen_groups.insert(&ft.measure_group_name) {
            continue;
        }
        let group_name = &ft.measure_group_name;

        // [Measures] system dimension
        rows.push_str(&format!(
            r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <MEASUREGROUP_NAME>{group}</MEASUREGROUP_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_IS_VISIBLE>false</DIMENSION_IS_VISIBLE>
            <DIMENSION_IS_FACT_DIMENSION>false</DIMENSION_IS_FACT_DIMENSION>
          </row>
"#,
            group = group_name,
            catalog = project.config.catalog,
            cube = project.config.cube,
        ));

        for d in &model.dimensions {
            rows.push_str(&format!(
                r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <MEASUREGROUP_NAME>{group}</MEASUREGROUP_NAME>
            <DIMENSION_UNIQUE_NAME>{dim_u}</DIMENSION_UNIQUE_NAME>
            <DIMENSION_IS_VISIBLE>{vis}</DIMENSION_IS_VISIBLE>
            <DIMENSION_IS_FACT_DIMENSION>false</DIMENSION_IS_FACT_DIMENSION>
          </row>
"#,
                group = group_name,
                dim_u = d.dimension_unique_name(),
                vis = d.visible,
                catalog = project.config.catalog,
                cube = project.config.cube,
            ));
        }
    }

    discover_rowset_envelope("", MG_DIM_ROW_FIELDS, &rows)
}
