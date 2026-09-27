use crate::proxy_project;
use crate::response::{UUID_TYPE, discover_rowset_envelope, xml_escape};

const DIM_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_NAME" name="DIMENSION_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_GUID" name="DIMENSION_GUID" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_CAPTION" name="DIMENSION_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_ORDINAL" name="DIMENSION_ORDINAL" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_TYPE" name="DIMENSION_TYPE" type="xsd:short" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_CARDINALITY" name="DIMENSION_CARDINALITY" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DEFAULT_HIERARCHY" name="DEFAULT_HIERARCHY" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="IS_VIRTUAL" name="IS_VIRTUAL" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="IS_READWRITE" name="IS_READWRITE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_SETTINGS" name="DIMENSION_UNIQUE_SETTINGS" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_MASTER_UNIQUE_NAME" name="DIMENSION_MASTER_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_IS_VISIBLE" name="DIMENSION_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="CUBE_SOURCE" name="CUBE_SOURCE" type="xsd:unsignedShort" minOccurs="0"/>"#;

pub fn get_dimensions_response(
    restrictions: &crate::xmla::parser::Restrictions,
    user: &crate::engine::model::UserContext,
    config: &crate::project::config::ProxyConfig,
) -> String {
    let project = proxy_project::project();
    if !super::in_scope(restrictions, &project.config.catalog, &project.config.cube) {
        // A request naming another catalog or cube is out of scope: the
        // reference answers an empty rowset in this rowset's shape, not a
        // fault (measured 2026-09-25).
        return discover_rowset_envelope(UUID_TYPE, DIM_ROW_FIELDS, "");
    }

    let model = &project.model;
    let catalog = &project.config.catalog;
    let cube = &project.config.cube;
    let mut rows = String::new();

    // Measures system dimension (special case); hidden when no measure's table
    // is visible.
    let visibility = super::visibility(restrictions.dimension_visibility);
    match visibility {
        super::Visibility::None => return discover_rowset_envelope(UUID_TYPE, DIM_ROW_FIELDS, ""),
        super::Visibility::Invalid => return super::visibility_fault(),
        _ => {}
    }
    let measures_visible = super::measures_visible(model, config, user);
    if measures_visible
        && super::visibility_selects(visibility, true)
        && super::name_matches(restrictions.dimension_name.as_deref(), &["Measures"])
    {
        rows.push_str(&format!(
            r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <DIMENSION_NAME>Measures</DIMENSION_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <DIMENSION_GUID>00000000-0000-0000-0000-000000000001</DIMENSION_GUID>
            <DIMENSION_CAPTION>Measures</DIMENSION_CAPTION>
            <DIMENSION_ORDINAL>0</DIMENSION_ORDINAL>
            <DIMENSION_TYPE>2</DIMENSION_TYPE>
            <DIMENSION_CARDINALITY>1</DIMENSION_CARDINALITY>
            <DEFAULT_HIERARCHY>[Measures]</DEFAULT_HIERARCHY>
            <DESCRIPTION>Measures system dimension</DESCRIPTION>
            <IS_VIRTUAL>false</IS_VIRTUAL>
            <IS_READWRITE>false</IS_READWRITE>
            <DIMENSION_UNIQUE_SETTINGS>1</DIMENSION_UNIQUE_SETTINGS>
            <DIMENSION_MASTER_UNIQUE_NAME>[Measures]</DIMENSION_MASTER_UNIQUE_NAME>
            <DIMENSION_IS_VISIBLE>false</DIMENSION_IS_VISIBLE>
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>
"#,
        ));
    }

    for (i, d) in model.dimensions.iter().enumerate() {
        if !super::visibility_selects(visibility, d.visible) {
            continue;
        }
        if !super::dimension_visible(model, config, user, &d.id) {
            continue;
        }
        if !super::name_matches(
            restrictions.dimension_name.as_deref(),
            &[&d.caption, &d.id, &d.dimension_unique_name()],
        ) {
            continue;
        }
        rows.push_str(&format!(
            r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <DIMENSION_NAME>{caption}</DIMENSION_NAME>
            <DIMENSION_UNIQUE_NAME>{dim_u}</DIMENSION_UNIQUE_NAME>
            <DIMENSION_GUID>00000000-0000-0000-0000-{:012}</DIMENSION_GUID>
            <DIMENSION_CAPTION>{caption}</DIMENSION_CAPTION>
            <DIMENSION_ORDINAL>{ordinal}</DIMENSION_ORDINAL>
            <DIMENSION_TYPE>{dim_type}</DIMENSION_TYPE>
            <DIMENSION_CARDINALITY>{cardinality}</DIMENSION_CARDINALITY>
            <DEFAULT_HIERARCHY>{hier_u}</DEFAULT_HIERARCHY>
            <DESCRIPTION>{description}</DESCRIPTION>
            <IS_VIRTUAL>false</IS_VIRTUAL>
            <IS_READWRITE>false</IS_READWRITE>
            <DIMENSION_UNIQUE_SETTINGS>1</DIMENSION_UNIQUE_SETTINGS>
            <DIMENSION_MASTER_UNIQUE_NAME>{dim_u}</DIMENSION_MASTER_UNIQUE_NAME>
            <DIMENSION_IS_VISIBLE>{visible}</DIMENSION_IS_VISIBLE>
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>
"#,
            i + 2,
            caption = xml_escape(&d.caption),
            dim_u = xml_escape(&d.dimension_unique_name()),
            ordinal = d.ordinal,
            cardinality = d.cardinality_hint,
            hier_u = xml_escape(&d.hierarchy_unique_name()),
            description = xml_escape(&d.description),
            visible = d.visible,
            dim_type = if d.is_date_role { 1 } else { 3 },
        ));
    }

    discover_rowset_envelope(UUID_TYPE, DIM_ROW_FIELDS, &rows)
}

#[cfg(test)]
mod tests {
    use crate::xmla::parser::Restrictions;

    /// The `*_VISIBILITY` restriction is a bitmask: 1 visible, 2 hidden, 3
    /// both, 4 nothing; a negative value faults like the reference (measured
    /// 2026-09-27).
    #[test]
    fn dimension_visibility_is_a_bitmask() {
        let project =
            crate::proxy_project::ProxyProject::load("projects/project3/proxy-config.json")
                .expect("load project3");
        crate::project::project::with_test_project(project, || {
            let project = crate::proxy_project::project();
            let admin = crate::engine::model::UserContext::admin_default();
            let rows = |value: Option<i32>| {
                let restrictions = Restrictions {
                    dimension_visibility: value,
                    ..Default::default()
                };
                super::get_dimensions_response(&restrictions, &admin, &project.config)
                    .matches("<row>")
                    .count()
            };

            let all = rows(None);
            assert!(all > 0);
            assert_eq!(rows(Some(1)), all, "1 selects the visible objects");
            assert_eq!(rows(Some(3)), all, "3 selects both classes");
            assert_eq!(rows(Some(5)), all, "higher bits select nothing extra");
            assert_eq!(rows(Some(0)), 0, "0 selects nothing");
            assert_eq!(rows(Some(2)), 0, "this model has no hidden dimensions");
            assert_eq!(rows(Some(4)), 0, "no object carries bit 2");

            let restrictions = Restrictions {
                dimension_visibility: Some(-1),
                ..Default::default()
            };
            let fault = super::get_dimensions_response(&restrictions, &admin, &project.config);
            assert!(
                fault.contains("Out of present range"),
                "a negative visibility faults like the reference: {fault}"
            );
        });
    }
}
