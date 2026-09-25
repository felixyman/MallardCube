use crate::proxy_project;
use crate::response::discover_rowset_envelope;

const MG_DIM_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASUREGROUP_NAME" name="MEASUREGROUP_NAME" type="xsd:string"/>
                <xsd:element sql:field="MEASUREGROUP_CARDINALITY" name="MEASUREGROUP_CARDINALITY" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_CARDINALITY" name="DIMENSION_CARDINALITY" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_IS_VISIBLE" name="DIMENSION_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_IS_FACT_DIMENSION" name="DIMENSION_IS_FACT_DIMENSION" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_GRANULARITY" name="DIMENSION_GRANULARITY" type="xsd:string" minOccurs="0"/>"#;

pub fn get_measuregroup_dimensions_response(
    restrictions: &crate::xmla::parser::Restrictions,
    user: &crate::engine::model::UserContext,
    config: &crate::project::config::ProxyConfig,
) -> String {
    let project = proxy_project::project();
    if !super::in_scope(restrictions, &project.config.catalog, &project.config.cube) {
        // A request naming another catalog or cube is out of scope: the
        // reference answers an empty rowset in this rowset's shape, not a
        // fault (measured 2026-09-25).
        return discover_rowset_envelope("", MG_DIM_ROW_FIELDS, "");
    }

    let model = &project.model;
    let mut rows = String::new();

    let mut seen_groups = std::collections::BTreeSet::new();
    if super::hidden_by_visibility(restrictions.dimension_visibility) {
        return discover_rowset_envelope("", MG_DIM_ROW_FIELDS, "");
    }
    for ft in &model.fact_tables {
        if !super::name_matches(
            restrictions.measuregroup_name.as_deref(),
            &[&ft.measure_group_name],
        ) {
            continue;
        }
        if !super::table_visible(config, user, &ft.table_name) {
            continue;
        }
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
            <MEASUREGROUP_CARDINALITY>MANY</MEASUREGROUP_CARDINALITY>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_CARDINALITY>ONE</DIMENSION_CARDINALITY>
            <DIMENSION_IS_VISIBLE>false</DIMENSION_IS_VISIBLE>
            <DIMENSION_IS_FACT_DIMENSION>false</DIMENSION_IS_FACT_DIMENSION>
          </row>
"#,
            group = group_name,
            catalog = project.config.catalog,
            cube = project.config.cube,
        ));

        for d in &model.dimensions {
            if !super::dimension_visible(model, config, user, &d.id) {
                continue;
            }
            // The dimension's granularity is its key attribute — the key
            // attribute hierarchy for a date role, the single hierarchy
            // otherwise. Excel reads this to know which attribute identifies a
            // dimension member (the reference SSAS reports e.g.
            // `[DateDim].[DateKey]`); plan 048.
            let granularity = d
                .key_hierarchy_unique_name()
                .unwrap_or_else(|| d.hierarchy_unique_name());
            rows.push_str(&format!(
                r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <MEASUREGROUP_NAME>{group}</MEASUREGROUP_NAME>
            <MEASUREGROUP_CARDINALITY>MANY</MEASUREGROUP_CARDINALITY>
            <DIMENSION_UNIQUE_NAME>{dim_u}</DIMENSION_UNIQUE_NAME>
            <DIMENSION_CARDINALITY>ONE</DIMENSION_CARDINALITY>
            <DIMENSION_IS_VISIBLE>{vis}</DIMENSION_IS_VISIBLE>
            <DIMENSION_IS_FACT_DIMENSION>false</DIMENSION_IS_FACT_DIMENSION>
            <DIMENSION_GRANULARITY>{granularity}</DIMENSION_GRANULARITY>
          </row>
"#,
                group = group_name,
                dim_u = d.dimension_unique_name(),
                vis = d.visible,
                granularity = granularity,
                catalog = project.config.catalog,
                cube = project.config.cube,
            ));
        }
    }

    discover_rowset_envelope("", MG_DIM_ROW_FIELDS, &rows)
}
