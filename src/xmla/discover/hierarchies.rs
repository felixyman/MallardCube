use crate::proxy_project;
use crate::response::{UUID_TYPE, discover_rowset_envelope, xml_escape};
use crate::xmla::parser::Restrictions;

const HIER_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="HIERARCHY_NAME" name="HIERARCHY_NAME" type="xsd:string"/>
                <xsd:element sql:field="HIERARCHY_UNIQUE_NAME" name="HIERARCHY_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="HIERARCHY_GUID" name="HIERARCHY_GUID" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="HIERARCHY_CAPTION" name="HIERARCHY_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_TYPE" name="DIMENSION_TYPE" type="xsd:short" minOccurs="0"/>
                <xsd:element sql:field="HIERARCHY_CARDINALITY" name="HIERARCHY_CARDINALITY" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="DEFAULT_MEMBER" name="DEFAULT_MEMBER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="ALL_MEMBER" name="ALL_MEMBER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="STRUCTURE" name="STRUCTURE" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="IS_VIRTUAL" name="IS_VIRTUAL" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="IS_READWRITE" name="IS_READWRITE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_SETTINGS" name="DIMENSION_UNIQUE_SETTINGS" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_IS_VISIBLE" name="DIMENSION_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="HIERARCHY_ORDINAL" name="HIERARCHY_ORDINAL" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_IS_SHARED" name="DIMENSION_IS_SHARED" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="HIERARCHY_IS_VISIBLE" name="HIERARCHY_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="HIERARCHY_ORIGIN" name="HIERARCHY_ORIGIN" type="xsd:unsignedShort" minOccurs="0"/>
                <xsd:element sql:field="HIERARCHY_DISPLAY_FOLDER" name="HIERARCHY_DISPLAY_FOLDER" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="INSTANCE_SELECTION" name="INSTANCE_SELECTION" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="GROUPING_BEHAVIOR" name="GROUPING_BEHAVIOR" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="STRUCTURE_TYPE" name="STRUCTURE_TYPE" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_SOURCE" name="CUBE_SOURCE" type="xsd:unsignedShort" minOccurs="0"/>"#;

pub fn get_hierarchies_response(restrictions: &Restrictions) -> String {
    let project = proxy_project::project();
    let model = &project.model;
    let mut rows = String::new();

    // Measures hierarchy (special case, not in model)
    if super::coordinates_match(restrictions, "[Measures]", Some("[Measures]"), None) {
        rows.push_str(&format!(
            r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_NAME>Measures</HIERARCHY_NAME>
            <HIERARCHY_UNIQUE_NAME>[Measures]</HIERARCHY_UNIQUE_NAME>
            <HIERARCHY_GUID>00000000-0000-0000-0000-000000000050</HIERARCHY_GUID>
            <HIERARCHY_CAPTION>Measures</HIERARCHY_CAPTION>
            <DIMENSION_TYPE>2</DIMENSION_TYPE>
            <HIERARCHY_CARDINALITY>1</HIERARCHY_CARDINALITY>
            <DEFAULT_MEMBER>{default_member}</DEFAULT_MEMBER>
            <STRUCTURE>0</STRUCTURE>
            <DIMENSION_IS_VISIBLE>false</DIMENSION_IS_VISIBLE>
            <HIERARCHY_ORDINAL>0</HIERARCHY_ORDINAL>
            <DIMENSION_IS_SHARED>true</DIMENSION_IS_SHARED>
            <HIERARCHY_IS_VISIBLE>false</HIERARCHY_IS_VISIBLE>
            <HIERARCHY_ORIGIN>6</HIERARCHY_ORIGIN>
            <HIERARCHY_DISPLAY_FOLDER></HIERARCHY_DISPLAY_FOLDER>
            <INSTANCE_SELECTION>0</INSTANCE_SELECTION>
            <GROUPING_BEHAVIOR>0</GROUPING_BEHAVIOR>
            <STRUCTURE_TYPE>Natural</STRUCTURE_TYPE>
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>
"#,
            catalog = project.config.catalog,
            cube = project.config.cube,
            // The mirror reports its internal `[Measures].[__Default measure]`
            // placeholder; a real first measure is resolvable and does not
            // borrow another project's measure name (plan 051 round 3).
            default_member = project
                .model
                .measures
                .first()
                .map(|m| m.measure_unique_name())
                .unwrap_or_default(),
        ));
    }

    let catalog = &project.config.catalog;
    let cube = &project.config.cube;
    // HIERARCHY_ORIGIN is a bitmask (MS-SSAS): 1 = user-defined, 2 = attribute,
    // 4 = key attribute. GROUPING_BEHAVIOR 1 (discourage grouping) and
    // STRUCTURE_TYPE Unnatural for user hierarchies / Natural for attribute
    // hierarchies match the reference SSAS 2025 (plan 048).
    let hier_row = |guid: u32,
                    dim_u: &str,
                    caption: &str,
                    hier_name: &str,
                    dim_type: u32,
                    origin: u32,
                    cardinality: u32,
                    ordinal: u32,
                    all_member: &str,
                    visible: bool| {
        let structure = if origin == 1 { "Unnatural" } else { "Natural" };
        format!(
            r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>{dim_u}</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_NAME>{hier_name}</HIERARCHY_NAME>
            <HIERARCHY_UNIQUE_NAME>[{caption}].[{hier_name}]</HIERARCHY_UNIQUE_NAME>
            <HIERARCHY_GUID>00000000-0000-0000-0000-{guid:012}</HIERARCHY_GUID>
            <HIERARCHY_CAPTION>{hier_name}</HIERARCHY_CAPTION>
            <DIMENSION_TYPE>{dim_type}</DIMENSION_TYPE>
            <HIERARCHY_CARDINALITY>{cardinality}</HIERARCHY_CARDINALITY>
            <DEFAULT_MEMBER>{all_member}</DEFAULT_MEMBER>
            <ALL_MEMBER>{all_member}</ALL_MEMBER>
            <STRUCTURE>0</STRUCTURE>
            <DIMENSION_IS_VISIBLE>{visible}</DIMENSION_IS_VISIBLE>
            <HIERARCHY_ORDINAL>{ordinal}</HIERARCHY_ORDINAL>
            <DIMENSION_IS_SHARED>true</DIMENSION_IS_SHARED>
            <HIERARCHY_IS_VISIBLE>{visible}</HIERARCHY_IS_VISIBLE>
            <HIERARCHY_ORIGIN>{origin}</HIERARCHY_ORIGIN>
            <HIERARCHY_DISPLAY_FOLDER></HIERARCHY_DISPLAY_FOLDER>
            <INSTANCE_SELECTION>0</INSTANCE_SELECTION>
            <GROUPING_BEHAVIOR>1</GROUPING_BEHAVIOR>
            <STRUCTURE_TYPE>{structure}</STRUCTURE_TYPE>
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>
"#,
            dim_u = xml_escape(dim_u),
            caption = xml_escape(caption),
            hier_name = xml_escape(hier_name),
            all_member = xml_escape(all_member),
        )
    };

    for (i, d) in model.dimensions.iter().enumerate() {
        let dim_type = if !d.levels.is_empty() {
            if d.is_date_role { 1 } else { 0 } // Time dim = 1, Regular = 0
        } else {
            3 // Other dim
        };
        let guid_base = 20 + i as u32 * 2;

        // The user hierarchy of a leveled dimension; the single attribute
        // hierarchy of a flat one.
        let user_origin = if !d.levels.is_empty() { 1 } else { 2 };
        let user_hier = format!("[{}].[{}]", d.caption, d.hierarchy_name);
        if super::coordinates_match(
            restrictions,
            &d.dimension_unique_name(),
            Some(&user_hier),
            None,
        ) {
            rows.push_str(&hier_row(
                guid_base,
                &d.dimension_unique_name(),
                &d.caption,
                &d.hierarchy_name,
                dim_type,
                user_origin,
                d.cardinality_hint,
                1,
                &d.all_member_unique_name(),
                d.visible,
            ));
        }

        // A date role's full-date level is exposed as the dimension's key
        // attribute hierarchy beside the user hierarchy (SSAS shape). Excel
        // only stores the MEMBER_VALUE data type — and therefore only offers
        // its Date Filters — for such a single-level attribute hierarchy
        // (plan 048).
        //
        // HIERARCHY_ORIGIN is 2 (attribute) and not 6 (attribute|key): the
        // reference SSAS 2025 reports 2 for a date column, and with the key bit
        // Excel wrote `memberValueDatatype="5"` (a double fallback) instead of
        // the date type 7 the level advertises.
        if let (Some(name), Some(level)) = (d.key_hierarchy_name(), d.key_level()) {
            let key_hier = format!("[{}].[{}]", d.caption, name);
            if super::coordinates_match(
                restrictions,
                &d.dimension_unique_name(),
                Some(&key_hier),
                None,
            ) {
                rows.push_str(&hier_row(
                    guid_base + 1,
                    &d.dimension_unique_name(),
                    &d.caption,
                    name,
                    dim_type,
                    2, // attribute hierarchy (matches the verified reference)
                    level.cardinality.max(1),
                    2,
                    &format!("[{}].[{}].[All]", d.caption, name),
                    d.visible,
                ));
            }
        }
    }

    discover_rowset_envelope(UUID_TYPE, HIER_ROW_FIELDS, &rows)
}

#[cfg(test)]
mod tests {
    use crate::project::project::ProxyProject;
    use crate::project::project::with_test_project;
    use crate::xmla::parser::Restrictions;

    #[test]
    fn date_dim_exposes_key_attribute_hierarchy() {
        // project3 has Date with hierarchy_levels + is_date_role=true. SSAS
        // shape: the user hierarchy [Date].[Calendar] (origin 1) plus the
        // single-level key attribute hierarchy [Date].[Full Date] carrying the
        // attribute|key bits (origin 6) — the latter is what makes Excel offer
        // its Date Filters (plan 048).
        let p = ProxyProject::load("projects/project3/proxy-config.json").expect("load project3");
        with_test_project(p, || {
            let resp = super::get_hierarchies_response(&Restrictions::default());
            assert!(
                resp.contains("<HIERARCHY_UNIQUE_NAME>[Date].[Calendar]</HIERARCHY_UNIQUE_NAME>"),
                "{resp}"
            );
            assert!(
                resp.contains("<HIERARCHY_UNIQUE_NAME>[Date].[Full Date]</HIERARCHY_UNIQUE_NAME>"),
                "{resp}"
            );
            // The key attribute hierarchy is a plain attribute hierarchy
            // (origin 2), like the verified reference. With the key bit (6)
            // Excel fell back to memberValueDatatype="5" instead of the date
            // type the level advertises (plan 048).
            let key_row = resp
                .split("<row>")
                .find(|r| {
                    r.contains("<HIERARCHY_UNIQUE_NAME>[Date].[Full Date]</HIERARCHY_UNIQUE_NAME>")
                })
                .expect("key hierarchy row");
            assert!(
                key_row.contains("<HIERARCHY_ORIGIN>2</HIERARCHY_ORIGIN>"),
                "key attribute hierarchy is origin 2: {key_row}"
            );
            assert!(
                resp.contains("<DIMENSION_TYPE>1</DIMENSION_TYPE>"),
                "Date should have DIMENSION_TYPE=1 (Time)"
            );
        });
    }

    #[test]
    fn regular_dim_has_default_origin() {
        let p = ProxyProject::load("projects/project3/proxy-config.json").expect("load project3");
        with_test_project(p, || {
            let resp = super::get_hierarchies_response(&Restrictions::default());
            // Category hierarchy should have HIERARCHY_ORIGIN=2, DIMENSION_TYPE=3
            assert!(
                resp.contains("<HIERARCHY_ORIGIN>2</HIERARCHY_ORIGIN>"),
                "Regular dims should have HIERARCHY_ORIGIN=2"
            );
            assert!(
                resp.contains("<DIMENSION_TYPE>3</DIMENSION_TYPE>"),
                "Regular dims should have DIMENSION_TYPE=3"
            );
        });
    }
}
