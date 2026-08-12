use crate::proxy_project;
use crate::response::{UUID_TYPE, discover_rowset_envelope, xml_escape};

const LEVEL_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="HIERARCHY_UNIQUE_NAME" name="HIERARCHY_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="LEVEL_NAME" name="LEVEL_NAME" type="xsd:string"/>
                <xsd:element sql:field="LEVEL_UNIQUE_NAME" name="LEVEL_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="LEVEL_GUID" name="LEVEL_GUID" type="uuid" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_CAPTION" name="LEVEL_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_NUMBER" name="LEVEL_NUMBER" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_CARDINALITY" name="LEVEL_CARDINALITY" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_TYPE" name="LEVEL_TYPE" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUSTOM_ROLLUP_SETTINGS" name="CUSTOM_ROLLUP_SETTINGS" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_UNIQUE_SETTINGS" name="LEVEL_UNIQUE_SETTINGS" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_IS_VISIBLE" name="LEVEL_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_ORDERING_PROPERTY" name="LEVEL_ORDERING_PROPERTY" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_DBTYPE" name="LEVEL_DBTYPE" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_MASTER_UNIQUE_NAME" name="LEVEL_MASTER_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_NAME_SQL_COLUMN_NAME" name="LEVEL_NAME_SQL_COLUMN_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_KEY_SQL_COLUMN_NAME" name="LEVEL_KEY_SQL_COLUMN_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_UNIQUE_NAME_SQL_COLUMN_NAME" name="LEVEL_UNIQUE_NAME_SQL_COLUMN_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_ATTRIBUTE_HIERARCHY_NAME" name="LEVEL_ATTRIBUTE_HIERARCHY_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_KEY_CARDINALITY" name="LEVEL_KEY_CARDINALITY" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_ORIGIN" name="LEVEL_ORIGIN" type="xsd:unsignedShort" minOccurs="0"/>
                <xsd:element sql:field="CUBE_SOURCE" name="CUBE_SOURCE" type="xsd:unsignedShort" minOccurs="0"/>"#;

pub fn get_levels_response() -> String {
    let project = proxy_project::project();
    let model = &project.model;
    let mut rows = String::new();

    // MeasuresLevel (special case, not in model)
    rows.push_str(&format!(
        r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Measures]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_NAME>MeasuresLevel</LEVEL_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[MeasuresLevel]</LEVEL_UNIQUE_NAME>
            <LEVEL_GUID>00000000-0000-0000-0000-000000000060</LEVEL_GUID>
            <LEVEL_CAPTION>MeasuresLevel</LEVEL_CAPTION>
            <LEVEL_NUMBER>0</LEVEL_NUMBER>
            <LEVEL_CARDINALITY>1</LEVEL_CARDINALITY>
            <LEVEL_TYPE>0</LEVEL_TYPE>
            <CUSTOM_ROLLUP_SETTINGS>0</CUSTOM_ROLLUP_SETTINGS>
            <LEVEL_UNIQUE_SETTINGS>1</LEVEL_UNIQUE_SETTINGS>
            <LEVEL_IS_VISIBLE>false</LEVEL_IS_VISIBLE>
            <LEVEL_DBTYPE>5</LEVEL_DBTYPE>
            <LEVEL_KEY_CARDINALITY>1</LEVEL_KEY_CARDINALITY>
            <LEVEL_ORIGIN>6</LEVEL_ORIGIN>
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>
"#,
        catalog = project.config.catalog,
        cube = project.config.cube,
    ));

    for (i, d) in model.dimensions.iter().enumerate() {
        let base_guid = 30 + i as u32 * 2;

        // (All) level
        rows.push_str(&format!(
            r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>{dim_u}</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>{hier_u}</HIERARCHY_UNIQUE_NAME>
            <LEVEL_NAME>{all_name}</LEVEL_NAME>
            <LEVEL_UNIQUE_NAME>{all_unique}</LEVEL_UNIQUE_NAME>
            <LEVEL_GUID>00000000-0000-0000-0000-{guid:012}</LEVEL_GUID>
            <LEVEL_CAPTION>{all_name}</LEVEL_CAPTION>
            <LEVEL_NUMBER>0</LEVEL_NUMBER>
            <LEVEL_CARDINALITY>1</LEVEL_CARDINALITY>
            <LEVEL_TYPE>1</LEVEL_TYPE>
            <CUSTOM_ROLLUP_SETTINGS>0</CUSTOM_ROLLUP_SETTINGS>
            <LEVEL_UNIQUE_SETTINGS>1</LEVEL_UNIQUE_SETTINGS>
            <LEVEL_IS_VISIBLE>false</LEVEL_IS_VISIBLE>
            <LEVEL_DBTYPE>130</LEVEL_DBTYPE>
            <LEVEL_KEY_CARDINALITY>1</LEVEL_KEY_CARDINALITY>
            <LEVEL_ORIGIN>1</LEVEL_ORIGIN>
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>
"#,
            dim_u = xml_escape(&d.dimension_unique_name()),
            hier_u = xml_escape(&d.hierarchy_unique_name()),
            all_name = xml_escape(&d.all_level_name),
            all_unique = xml_escape(&d.all_level_unique_name()),
            guid = base_guid,
            catalog = project.config.catalog,
            cube = project.config.cube,
        ));

        if !d.levels.is_empty() {
            for level in &d.levels {
                let level_num = level.level_number + 1; // (All) is 0, first level is 1
                let level_unique = format!("{}.[{}]", d.hierarchy_unique_name(), level.name);
                rows.push_str(&format!(
                    r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>{dim_u}</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>{hier_u}</HIERARCHY_UNIQUE_NAME>
            <LEVEL_NAME>{lname}</LEVEL_NAME>
            <LEVEL_UNIQUE_NAME>{lunique}</LEVEL_UNIQUE_NAME>
            <LEVEL_GUID>00000000-0000-0000-0000-{guid:012}</LEVEL_GUID>
            <LEVEL_CAPTION>{lname}</LEVEL_CAPTION>
            <LEVEL_NUMBER>{lnum}</LEVEL_NUMBER>
            <LEVEL_CARDINALITY>{lcard}</LEVEL_CARDINALITY>
            <LEVEL_TYPE>0</LEVEL_TYPE>
            <CUSTOM_ROLLUP_SETTINGS>0</CUSTOM_ROLLUP_SETTINGS>
            <LEVEL_UNIQUE_SETTINGS>1</LEVEL_UNIQUE_SETTINGS>
            <LEVEL_IS_VISIBLE>true</LEVEL_IS_VISIBLE>
            <LEVEL_DBTYPE>130</LEVEL_DBTYPE>
            <LEVEL_KEY_CARDINALITY>{lcard}</LEVEL_KEY_CARDINALITY>
            <LEVEL_ORIGIN>1</LEVEL_ORIGIN>
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>
"#,
                    lname = xml_escape(&level.name),
                    lunique = xml_escape(&level_unique),
                    lnum = level_num,
                    lcard = level.cardinality.max(1),
                    dim_u = xml_escape(&d.dimension_unique_name()),
                    hier_u = xml_escape(&d.hierarchy_unique_name()),
                    guid = base_guid + 1 + level.level_number * 2,
                    catalog = project.config.catalog,
                    cube = project.config.cube,
                ));
            }
        } else {
            // Leaf level (single-level hierarchy, current behavior)
            rows.push_str(&format!(
                r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>{dim_u}</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>{hier_u}</HIERARCHY_UNIQUE_NAME>
            <LEVEL_NAME>{leaf_name}</LEVEL_NAME>
            <LEVEL_UNIQUE_NAME>{leaf_unique}</LEVEL_UNIQUE_NAME>
            <LEVEL_GUID>00000000-0000-0000-0000-{guid:012}</LEVEL_GUID>
            <LEVEL_CAPTION>{leaf_name}</LEVEL_CAPTION>
            <LEVEL_NUMBER>1</LEVEL_NUMBER>
            <LEVEL_CARDINALITY>{cardinality}</LEVEL_CARDINALITY>
            <LEVEL_TYPE>0</LEVEL_TYPE>
            <CUSTOM_ROLLUP_SETTINGS>0</CUSTOM_ROLLUP_SETTINGS>
            <LEVEL_UNIQUE_SETTINGS>1</LEVEL_UNIQUE_SETTINGS>
            <LEVEL_IS_VISIBLE>true</LEVEL_IS_VISIBLE>
            <LEVEL_DBTYPE>130</LEVEL_DBTYPE>
            <LEVEL_KEY_CARDINALITY>{cardinality}</LEVEL_KEY_CARDINALITY>
            <LEVEL_ORIGIN>1</LEVEL_ORIGIN>
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>
"#,
                dim_u = xml_escape(&d.dimension_unique_name()),
                hier_u = xml_escape(&d.hierarchy_unique_name()),
                leaf_name = xml_escape(&d.leaf_level_name),
                leaf_unique = xml_escape(&d.leaf_level_unique_name()),
                guid = base_guid + 1,
                cardinality = d.cardinality_hint,
                catalog = project.config.catalog,
                cube = project.config.cube,
            ));
        }
    }

    discover_rowset_envelope(UUID_TYPE, LEVEL_ROW_FIELDS, &rows)
}

#[cfg(test)]
mod tests {
    use crate::project::project::ProxyProject;
    use crate::project::project::with_test_project;

    #[test]
    fn date_dim_has_five_levels() {
        let p = ProxyProject::load("projects/project3/proxy-config.json").expect("load project3");
        with_test_project(p, || {
            let resp = super::get_levels_response();
            let date_section = &resp[resp.find("[Date]").unwrap_or(0)..];
            let year = date_section.contains("<LEVEL_NAME>Year</LEVEL_NAME>");
            let quarter = date_section.contains("<LEVEL_NAME>Quarter</LEVEL_NAME>");
            let month = date_section.contains("<LEVEL_NAME>Month</LEVEL_NAME>");
            let date_leaf = date_section.contains("<LEVEL_NAME>Date</LEVEL_NAME>");
            assert!(year, "should have Year level");
            assert!(quarter, "should have Quarter level");
            assert!(month, "should have Month level");
            assert!(date_leaf, "should have Date leaf level");
        });
    }

    #[test]
    fn single_dim_has_two_levels() {
        let p = ProxyProject::load("projects/project3/proxy-config.json").expect("load project3");
        with_test_project(p, || {
            let resp = super::get_levels_response();
            let cat_section = resp
                .split("<DIMENSION_UNIQUE_NAME>[Category]")
                .collect::<Vec<_>>();
            assert!(cat_section.len() >= 2, "should find Category dimension");
        });
    }
}
