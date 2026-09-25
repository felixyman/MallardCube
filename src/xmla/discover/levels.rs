use crate::proxy_project;
use crate::response::{UUID_TYPE, discover_rowset_envelope, xml_escape};
use crate::xmla::parser::Restrictions;

// OLE DB DBTYPE values (oledb.h / mdstypes.h).
const DBTYPE_I4: i32 = 3;
const DBTYPE_DATE: i32 = 7;
const DBTYPE_WSTR: i32 = 130;

/// OLE DB `LEVEL_DBTYPE` for a level's member key. Date-role leaves report a
/// date type so Excel treats the hierarchy as dates; everything else stays a
/// string.
///
/// Verified against the reference SSAS 2025 (tabular) and the pivot cache Excel
/// writes: the date level reports `DBTYPE_DATE` (7), and Excel stores that as
/// the hierarchy's `memberValueDatatype="7"` — the flag that gates its Date
/// Filters. Reporting `DBTYPE_DBTIMESTAMP` (135) instead made Excel fall back to
/// `memberValueDatatype="5"` and withhold the date filters (plan 048).
fn level_db_type(
    d: &crate::engine::model::DimensionDef,
    level: &crate::engine::model::LevelDef,
) -> i32 {
    if !d.is_date_role {
        return DBTYPE_WSTR;
    }
    // The deepest level of a date role is the full date, the ones above it are
    // period parts (level names are project-defined, so go by level number).
    let deepest = d.levels.iter().map(|l| l.level_number).max();
    if Some(level.level_number) == deepest {
        DBTYPE_DATE
    } else {
        DBTYPE_I4
    }
}

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

pub fn get_levels_response(
    restrictions: &Restrictions,
    user: &crate::engine::model::UserContext,
    config: &crate::project::config::ProxyConfig,
) -> String {
    let project = proxy_project::project();
    if !super::in_scope(restrictions, &project.config.catalog, &project.config.cube) {
        // A request naming another catalog or cube is out of scope: the
        // reference answers an empty rowset in this rowset's shape, not a
        // fault (measured 2026-09-25).
        return discover_rowset_envelope(UUID_TYPE, LEVEL_ROW_FIELDS, "");
    }

    let model = &project.model;
    if super::hidden_by_visibility(restrictions.level_visibility) {
        return discover_rowset_envelope(UUID_TYPE, LEVEL_ROW_FIELDS, "");
    }
    let mut rows = String::new();

    // MeasuresLevel (special case, not in model); hidden when no measure's
    // table is visible.
    let measures_visible = super::measures_visible(model, config, user);
    if measures_visible
        && super::coordinates_match(
            restrictions,
            "[Measures]",
            Some("[Measures]"),
            Some("[Measures].[MeasuresLevel]"),
        )
    {
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
    }

    for (i, d) in model.dimensions.iter().enumerate() {
        if !super::dimension_visible(model, config, user, &d.id) {
            continue;
        }
        let base_guid = 30 + i as u32 * 2;

        // (All) level. A flat dimension exposes a single attribute hierarchy
        // (origin 2); a date role's user hierarchy is origin 1.
        let all_origin = if d.levels.is_empty() { 2 } else { 1 };
        if super::coordinates_match(
            restrictions,
            &d.dimension_unique_name(),
            Some(&d.hierarchy_unique_name()),
            Some(&d.all_level_unique_name()),
        ) {
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
            <LEVEL_UNIQUE_SETTINGS>0</LEVEL_UNIQUE_SETTINGS>
            <LEVEL_IS_VISIBLE>true</LEVEL_IS_VISIBLE>
            <LEVEL_ORDERING_PROPERTY>{all_name}</LEVEL_ORDERING_PROPERTY>
            <LEVEL_DBTYPE>3</LEVEL_DBTYPE>
            <LEVEL_KEY_CARDINALITY>1</LEVEL_KEY_CARDINALITY>
            <LEVEL_ORIGIN>{all_origin}</LEVEL_ORIGIN>
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>
"#,
                all_origin = all_origin,
                dim_u = xml_escape(&d.dimension_unique_name()),
                hier_u = xml_escape(&d.hierarchy_unique_name()),
                all_name = xml_escape(&d.all_level_name),
                all_unique = xml_escape(&d.all_level_unique_name()),
                guid = base_guid,
                catalog = project.config.catalog,
                cube = project.config.cube,
            ));
        }

        if !d.levels.is_empty() {
            // Levels of the user hierarchy: LEVEL_ORIGIN 1 (MS-SSAS bitmask:
            // 1 = user hierarchy level) and LEVEL_TYPE 0 — the tabular
            // presentation. Time level types (20/68/84/116) are a
            // multidimensional concept; the reference SSAS 2025 reports 0 for
            // every level but (All), and Excel still offers its Date Filters on
            // the date attribute hierarchy (plan 048).
            for level in &d.levels {
                let level_num = level.level_number + 1; // (All) is 0, first level is 1
                let level_unique = format!("{}.[{}]", d.hierarchy_unique_name(), level.name);
                let level_dbt = level_db_type(d, level);
                if !super::coordinates_match(
                    restrictions,
                    &d.dimension_unique_name(),
                    Some(&d.hierarchy_unique_name()),
                    Some(&level_unique),
                ) {
                    continue;
                }
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
            <LEVEL_UNIQUE_SETTINGS>0</LEVEL_UNIQUE_SETTINGS>
            <LEVEL_IS_VISIBLE>true</LEVEL_IS_VISIBLE>
            <LEVEL_ORDERING_PROPERTY>{lname}</LEVEL_ORDERING_PROPERTY>
            <LEVEL_DBTYPE>{ldbt}</LEVEL_DBTYPE>
            <LEVEL_ATTRIBUTE_HIERARCHY_NAME>{lname}</LEVEL_ATTRIBUTE_HIERARCHY_NAME>
            <LEVEL_KEY_CARDINALITY>1</LEVEL_KEY_CARDINALITY>
            <LEVEL_ORIGIN>1</LEVEL_ORIGIN>
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>
"#,
                    lname = xml_escape(&level.name),
                    lunique = xml_escape(&level_unique),
                    lnum = level_num,
                    lcard = level.cardinality.max(1),
                    ldbt = level_dbt,
                    dim_u = xml_escape(&d.dimension_unique_name()),
                    hier_u = xml_escape(&d.hierarchy_unique_name()),
                    guid = base_guid + 1 + level.level_number * 2,
                    catalog = project.config.catalog,
                    cube = project.config.cube,
                ));
            }
        } else {
            // Leaf level (single-level hierarchy, current behavior)
            if super::coordinates_match(
                restrictions,
                &d.dimension_unique_name(),
                Some(&d.hierarchy_unique_name()),
                Some(&d.leaf_level_unique_name()),
            ) {
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
            <LEVEL_UNIQUE_SETTINGS>0</LEVEL_UNIQUE_SETTINGS>
            <LEVEL_IS_VISIBLE>true</LEVEL_IS_VISIBLE>
            <LEVEL_ORDERING_PROPERTY>{leaf_name}</LEVEL_ORDERING_PROPERTY>
            <LEVEL_DBTYPE>{ldbt}</LEVEL_DBTYPE>
            <LEVEL_ATTRIBUTE_HIERARCHY_NAME>{leaf_name}</LEVEL_ATTRIBUTE_HIERARCHY_NAME>
            <LEVEL_KEY_CARDINALITY>1</LEVEL_KEY_CARDINALITY>
            <LEVEL_ORIGIN>2</LEVEL_ORIGIN>
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>
"#,
                    dim_u = xml_escape(&d.dimension_unique_name()),
                    hier_u = xml_escape(&d.hierarchy_unique_name()),
                    leaf_name = xml_escape(&d.leaf_level_name),
                    leaf_unique = xml_escape(&d.leaf_level_unique_name()),
                    guid = base_guid + 1,
                    cardinality = d.cardinality_hint,
                    ldbt = if d.is_date_role {
                        DBTYPE_DATE
                    } else {
                        DBTYPE_WSTR
                    },
                    catalog = project.config.catalog,
                    cube = project.config.cube,
                ));
            }
        }

        // The key attribute hierarchy of a date role: (All) + the full-date
        // level, shaped exactly like the reference SSAS 2025 date column
        // (attribute hierarchy, origin 2, plain LEVEL_TYPE 0 on the date level).
        // That shape is what makes Excel read the level's DBTYPE (7 = date) and
        // store memberValueDatatype="7" — the flag that gates its Date Filters
        // (plan 048).
        if let (Some(key_name), Some(level)) = (d.key_hierarchy_name(), d.key_level()) {
            let key_hier_u = format!("[{}].[{}]", d.caption, key_name);
            let level_unique = format!("{key_hier_u}.[{}]", level.name);
            let key_all_unique = format!("{key_hier_u}.[{}]", d.all_level_name);
            let cardinality = level.cardinality.max(1);
            if super::coordinates_match(
                restrictions,
                &d.dimension_unique_name(),
                Some(&key_hier_u),
                Some(&key_all_unique),
            ) {
                rows.push_str(&format!(
                    r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>{dim_u}</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>{hier_u}</HIERARCHY_UNIQUE_NAME>
            <LEVEL_NAME>{all_name}</LEVEL_NAME>
            <LEVEL_UNIQUE_NAME>{hier_u}.[{all_name}]</LEVEL_UNIQUE_NAME>
            <LEVEL_GUID>00000000-0000-0000-0000-{guid_all:012}</LEVEL_GUID>
            <LEVEL_CAPTION>{all_name}</LEVEL_CAPTION>
            <LEVEL_NUMBER>0</LEVEL_NUMBER>
            <LEVEL_CARDINALITY>1</LEVEL_CARDINALITY>
            <LEVEL_TYPE>1</LEVEL_TYPE>
            <CUSTOM_ROLLUP_SETTINGS>0</CUSTOM_ROLLUP_SETTINGS>
            <LEVEL_UNIQUE_SETTINGS>0</LEVEL_UNIQUE_SETTINGS>
            <LEVEL_IS_VISIBLE>true</LEVEL_IS_VISIBLE>
            <LEVEL_ORDERING_PROPERTY>{all_name}</LEVEL_ORDERING_PROPERTY>
            <LEVEL_DBTYPE>{all_dbtype}</LEVEL_DBTYPE>
            <LEVEL_KEY_CARDINALITY>1</LEVEL_KEY_CARDINALITY>
            <LEVEL_ORIGIN>2</LEVEL_ORIGIN>
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>
"#,
                    all_name = xml_escape(&d.all_level_name),
                    dim_u = xml_escape(&d.dimension_unique_name()),
                    hier_u = xml_escape(&key_hier_u),
                    guid_all = base_guid + 8,
                    all_dbtype = DBTYPE_I4,
                    catalog = project.config.catalog,
                    cube = project.config.cube,
                ));
            }
            if super::coordinates_match(
                restrictions,
                &d.dimension_unique_name(),
                Some(&key_hier_u),
                Some(&level_unique),
            ) {
                rows.push_str(&format!(
                    r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>{dim_u}</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>{hier_u}</HIERARCHY_UNIQUE_NAME>
            <LEVEL_NAME>{lname}</LEVEL_NAME>
            <LEVEL_UNIQUE_NAME>{lunique}</LEVEL_UNIQUE_NAME>
            <LEVEL_GUID>00000000-0000-0000-0000-{guid_level:012}</LEVEL_GUID>
            <LEVEL_CAPTION>{lname}</LEVEL_CAPTION>
            <LEVEL_NUMBER>1</LEVEL_NUMBER>
            <LEVEL_CARDINALITY>{cardinality}</LEVEL_CARDINALITY>
            <LEVEL_TYPE>0</LEVEL_TYPE>
            <CUSTOM_ROLLUP_SETTINGS>0</CUSTOM_ROLLUP_SETTINGS>
            <LEVEL_UNIQUE_SETTINGS>0</LEVEL_UNIQUE_SETTINGS>
            <LEVEL_IS_VISIBLE>true</LEVEL_IS_VISIBLE>
            <LEVEL_ORDERING_PROPERTY>{lname}</LEVEL_ORDERING_PROPERTY>
            <LEVEL_DBTYPE>{level_dbtype}</LEVEL_DBTYPE>
            <LEVEL_ATTRIBUTE_HIERARCHY_NAME>{lname}</LEVEL_ATTRIBUTE_HIERARCHY_NAME>
            <LEVEL_KEY_CARDINALITY>1</LEVEL_KEY_CARDINALITY>
            <LEVEL_ORIGIN>2</LEVEL_ORIGIN>
            <CUBE_SOURCE>1</CUBE_SOURCE>
          </row>
"#,
                    lname = xml_escape(&level.name),
                    lunique = xml_escape(&level_unique),
                    cardinality = cardinality,
                    level_dbtype = DBTYPE_DATE,
                    guid_level = base_guid + 9,
                    dim_u = xml_escape(&d.dimension_unique_name()),
                    hier_u = xml_escape(&key_hier_u),
                    catalog = project.config.catalog,
                    cube = project.config.cube,
                ));
            }
        }
    }

    discover_rowset_envelope(UUID_TYPE, LEVEL_ROW_FIELDS, &rows)
}

#[cfg(test)]
mod tests {
    /// An OLS-hidden table disappears from the metadata rowsets; an
    /// administrator still sees it.
    #[test]
    fn hidden_tables_are_not_advertised() {
        use crate::engine::model::UserContext;
        use crate::project::config::{ModelPermission, RoleConfig, TablePermissionConfig};

        let project =
            crate::proxy_project::ProxyProject::load("projects/project3/proxy-config.json")
                .expect("load project3");
        crate::project::project::with_test_project(project, || {
            let project = crate::proxy_project::project();
            let mut config = project.config.clone();
            let table = project.model.dim_table_for_discovery("Date").to_string();
            config.roles = vec![RoleConfig {
                name: "OLS".into(),
                description: String::new(),
                model_permission: ModelPermission::Read,
                members: vec![],
                table_permissions: vec![TablePermissionConfig {
                    table,
                    filter_expression: String::new(),
                    dax_filter: None,
                    metadata_permission: ModelPermission::None,
                }],
            }];
            let mut user = UserContext::deny_all();
            user.roles = vec!["OLS".into()];

            let restricted = super::get_levels_response(&Restrictions::default(), &user, &config);
            assert!(
                !restricted.contains("[Date].[Calendar]"),
                "a hidden table must not be advertised: {restricted}"
            );
            let admin = super::get_levels_response(
                &Restrictions::default(),
                &UserContext::admin_default(),
                &project.config,
            );
            assert!(admin.contains("[Date].[Calendar]"), "{admin}");
        });
    }

    use crate::project::project::ProxyProject;
    use crate::project::project::with_test_project;
    use crate::xmla::parser::Restrictions;

    #[test]
    fn date_dim_has_five_levels() {
        let p = ProxyProject::load("projects/project3/proxy-config.json").expect("load project3");
        with_test_project(p, || {
            let resp = super::get_levels_response(
                &Restrictions::default(),
                &crate::engine::model::UserContext::admin_default(),
                &crate::proxy_project::project().config,
            );
            let date_section = &resp[resp.find("[Date]").unwrap_or(0)..];
            let year = date_section.contains("<LEVEL_NAME>Year</LEVEL_NAME>");
            let quarter = date_section.contains("<LEVEL_NAME>Quarter</LEVEL_NAME>");
            let month = date_section.contains("<LEVEL_NAME>Month</LEVEL_NAME>");
            let date_leaf = date_section.contains("<LEVEL_NAME>Full Date</LEVEL_NAME>");
            assert!(year, "should have Year level");
            assert!(quarter, "should have Quarter level");
            assert!(month, "should have Month level");
            assert!(date_leaf, "should have Date leaf level");

            // Date levels report real data types (Excel needs a date type for
            // Date Filters): the leaves are dates (7 = DBTYPE_DATE, matching the
            // reference SSAS and what Excel stores as memberValueDatatype="7"),
            // the period parts are numeric.
            for (lvl, expected) in [
                ("[Date].[Calendar].[Year]", "3"),
                ("[Date].[Calendar].[Quarter]", "3"),
                ("[Date].[Calendar].[Month]", "3"),
                ("[Date].[Calendar].[Full Date]", "7"),
                ("[Date].[Full Date].[Full Date]", "7"),
            ] {
                let marker = format!("<LEVEL_UNIQUE_NAME>{lvl}</LEVEL_UNIQUE_NAME>");
                let start = resp
                    .find(&marker)
                    .unwrap_or_else(|| panic!("missing {lvl}"));
                let end = start + resp[start..].find("</row>").unwrap();
                let row = &resp[start..end];
                let dbt = row
                    .split("<LEVEL_DBTYPE>")
                    .nth(1)
                    .and_then(|s| s.split('<').next())
                    .unwrap_or("");
                assert_eq!(dbt, expected, "LEVEL_DBTYPE for {lvl}: {row}");
            }

            // Non-date dimensions stay strings at the leaf level.
            let cat_row = resp
                .split("<row>")
                .find(|r| {
                    r.contains(
                        "<LEVEL_UNIQUE_NAME>[Category].[Category].[Category]</LEVEL_UNIQUE_NAME>",
                    )
                })
                .expect("Category leaf level row");
            assert!(
                cat_row.contains("<LEVEL_DBTYPE>130</LEVEL_DBTYPE>"),
                "Category should stay a string level: {cat_row}"
            );

            // Tabular presentation, verified against the reference SSAS 2025:
            // (All) levels are visible with DBTYPE 3 and LEVEL_TYPE 1; every
            // other level reports LEVEL_TYPE 0. Time level types are a
            // multidimensional concept, and Excel still offers Date Filters on
            // the date attribute hierarchy without them (plan 048).
            let all_row = resp
                .split("<row>")
                .find(|r| {
                    r.contains("<LEVEL_UNIQUE_NAME>[Date].[Calendar].[(All)]</LEVEL_UNIQUE_NAME>")
                })
                .expect("Date (All) level row");
            assert!(all_row.contains("<LEVEL_TYPE>1</LEVEL_TYPE>"), "{all_row}");
            assert!(
                all_row.contains("<LEVEL_DBTYPE>3</LEVEL_DBTYPE>"),
                "{all_row}"
            );
            assert!(
                all_row.contains("<LEVEL_IS_VISIBLE>true</LEVEL_IS_VISIBLE>"),
                "{all_row}"
            );
            for lvl in [
                "[Date].[Calendar].[Year]",
                "[Date].[Calendar].[Full Date]",
                "[Date].[Full Date].[Full Date]",
            ] {
                let marker = format!("<LEVEL_UNIQUE_NAME>{lvl}</LEVEL_UNIQUE_NAME>");
                let start = resp
                    .find(&marker)
                    .unwrap_or_else(|| panic!("missing {lvl}"));
                let end = start + resp[start..].find("</row>").unwrap();
                let row = &resp[start..end];
                assert!(
                    row.contains("<LEVEL_TYPE>0</LEVEL_TYPE>"),
                    "tabular LEVEL_TYPE for {lvl}: {row}"
                );
            }
        });
    }

    #[test]
    fn single_dim_has_two_levels() {
        let p = ProxyProject::load("projects/project3/proxy-config.json").expect("load project3");
        with_test_project(p, || {
            let resp = super::get_levels_response(
                &Restrictions::default(),
                &crate::engine::model::UserContext::admin_default(),
                &crate::proxy_project::project().config,
            );
            let cat_section = resp
                .split("<DIMENSION_UNIQUE_NAME>[Category]")
                .collect::<Vec<_>>();
            assert!(cat_section.len() >= 2, "should find Category dimension");
        });
    }
}
