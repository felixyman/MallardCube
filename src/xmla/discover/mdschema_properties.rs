use crate::proxy_project;
use crate::response::discover_rowset_envelope;

use crate::engine::model::{DimensionDef, LevelDef};
use crate::xmla::parser::Restrictions;

// OLE DB DBTYPEs used for MEMBER_VALUE rows. Excel offers its Date Filters only
// on a field whose MEMBER_VALUE DATA_TYPE is a date type (7 = DBTYPE_DATE, the
// value the OOXML `memberValueDatatype` note names); the other levels keep
// their key types (plan 048).
const DBTYPE_R8: i32 = 5;
const DBTYPE_I4: i32 = 3;
const DBTYPE_DATE: i32 = 7;
const DBTYPE_WSTR: i32 = 130;

/// MEMBER_VALUE data type for one level: the level's key type.
fn level_member_value_type(d: &DimensionDef, level: &LevelDef) -> i32 {
    if !d.is_date_role {
        return DBTYPE_WSTR;
    }
    let deepest = d.levels.iter().map(|l| l.level_number).max();
    if Some(level.level_number) == deepest {
        DBTYPE_DATE
    } else {
        DBTYPE_I4
    }
}

/// One MEMBER_VALUE row target: `(hierarchy unique name, level unique name,
/// MEMBER_VALUE data type)`. A date role exposes its user hierarchy's levels
/// plus a single-level key attribute hierarchy; a flat dimension exposes its
/// (All) and leaf levels (plan 048).
fn member_value_targets(d: &DimensionDef) -> Vec<(String, String, i32)> {
    let mut out = Vec::new();
    if d.levels.is_empty() {
        out.push((
            d.hierarchy_unique_name(),
            d.all_level_unique_name(),
            DBTYPE_WSTR,
        ));
        out.push((
            d.hierarchy_unique_name(),
            d.leaf_level_unique_name(),
            if d.is_date_role {
                DBTYPE_DATE
            } else {
                DBTYPE_WSTR
            },
        ));
        return out;
    }

    let user = d.hierarchy_unique_name();
    out.push((user.clone(), d.all_level_unique_name(), DBTYPE_WSTR));
    for level in &d.levels {
        out.push((
            user.clone(),
            format!("{user}.[{}]", level.name),
            level_member_value_type(d, level),
        ));
    }
    if let (Some(key), Some(level)) = (d.key_hierarchy_unique_name(), d.key_level()) {
        out.push((
            key.clone(),
            format!("{key}.[{}]", d.all_level_name),
            DBTYPE_WSTR,
        ));
        out.push((key.clone(), format!("{key}.[{}]", level.name), DBTYPE_DATE));
    }
    out
}

const PROPERTIES_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string"/>
                <xsd:element sql:field="HIERARCHY_UNIQUE_NAME" name="HIERARCHY_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_UNIQUE_NAME" name="LEVEL_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEMBER_UNIQUE_NAME" name="MEMBER_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_NAME" name="PROPERTY_NAME" type="xsd:string"/>
                <xsd:element sql:field="PROPERTY_CAPTION" name="PROPERTY_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_DESCRIPTION" name="PROPERTY_DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_TYPE" name="PROPERTY_TYPE" type="xsd:short" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_CONTENT_TYPE" name="PROPERTY_CONTENT_TYPE" type="xsd:short" minOccurs="0"/>
                <xsd:element sql:field="DATA_TYPE" name="DATA_TYPE" type="xsd:unsignedShort" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_ORIGIN" name="PROPERTY_ORIGIN" type="xsd:int" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_IS_VISIBLE" name="PROPERTY_IS_VISIBLE" type="xsd:boolean" minOccurs="0"/>"#;

/// Dimension/hierarchy/level coordinates a member-property row is emitted for.
struct RowCoords<'a> {
    dim: &'a str,
    hier: &'a str,
    level: &'a str,
}

fn member_property_row(
    catalog: &str,
    cube: &str,
    coords: &RowCoords<'_>,
    prop_name: &str,
    content_type: u8,
    data_type: Option<i32>,
    origin: u32,
) -> String {
    let RowCoords { dim, hier, level } = coords;
    let data_type_xml = match data_type {
        Some(t) => format!("\n            <DATA_TYPE>{t}</DATA_TYPE>"),
        None => String::new(),
    };
    format!(
        r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>{dim}</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>{hier}</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>{level}</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>{prop_name}</PROPERTY_NAME>
            <PROPERTY_CAPTION>{prop_name}</PROPERTY_CAPTION>
            <PROPERTY_TYPE>1</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>{content_type}</PROPERTY_CONTENT_TYPE>{data_type_xml}
            <PROPERTY_ORIGIN>{origin}</PROPERTY_ORIGIN>
            <PROPERTY_IS_VISIBLE>true</PROPERTY_IS_VISIBLE>
          </row>"#,
    )
}

/// Does a row for these coordinates satisfy the request's restriction list?
/// Excel restricts `MDSCHEMA_PROPERTIES` to one hierarchy at a time while it
/// builds pivot cache fields; rows for other hierarchies corrupt the cache
/// field (plan 048).
fn matches_restrictions(restrictions: &Restrictions, dim: &str, hier: &str, level: &str) -> bool {
    if let Some(d) = &restrictions.dimension_unique_name
        && d != dim
    {
        return false;
    }
    if let Some(h) = &restrictions.hierarchy_unique_name
        && h != hier
    {
        return false;
    }
    if let Some(l) = &restrictions.level_unique_name
        && l != level
    {
        return false;
    }
    true
}

/// Is a property row with this name requested?
fn property_requested(restrictions: &Restrictions, prop_name: &str) -> bool {
    match &restrictions.property_name {
        Some(p) => p == prop_name,
        None => true,
    }
}

fn member_property_rows(restrictions: &Restrictions) -> String {
    const PROPS: &[(&str, u8)] = &[
        ("MEMBER_CAPTION", 0),
        ("MEMBER_NAME", 0),
        ("MEMBER_UNIQUE_NAME", 1),
        ("MEMBER_KEY", 1),
        ("MEMBER_TYPE", 0),
        ("MEMBER_VALUE", 0),
        ("LEVEL_NUMBER", 0),
        ("LEVEL_UNIQUE_NAME", 1),
        ("PARENT_LEVEL", 0),
        ("PARENT_UNIQUE_NAME", 1),
        ("PARENT_COUNT", 0),
        ("CHILDREN_CARDINALITY", 0),
    ];

    let project = proxy_project::project();
    let model = &project.model;
    let catalog = &project.config.catalog;
    let cube = &project.config.cube;
    let mut out = String::new();
    for d in &model.dimensions {
        let dim = &d.dimension_unique_name();
        for (hier, level, data_type) in member_value_targets(d) {
            if !matches_restrictions(restrictions, dim, &hier, &level) {
                continue;
            }
            let coords = RowCoords {
                dim,
                hier: &hier,
                level: &level,
            };
            let origin = if !d.levels.is_empty() && hier == d.hierarchy_unique_name() {
                1
            } else {
                2
            };
            for (name, content) in PROPS {
                if !property_requested(restrictions, name) {
                    continue;
                }
                out.push_str(&member_property_row(
                    catalog,
                    cube,
                    &coords,
                    name,
                    *content,
                    (*name == "MEMBER_VALUE").then_some(data_type),
                    origin,
                ));
                out.push('\n');
            }
        }
    }

    // [Measures] intrinsic member properties (special case)
    const M_PROPS: &[(&str, u8)] = &[
        ("MEMBER_CAPTION", 0),
        ("MEMBER_NAME", 0),
        ("MEMBER_UNIQUE_NAME", 1),
        ("MEMBER_VALUE", 0),
    ];
    let measures_coords = RowCoords {
        dim: "[Measures]",
        hier: "[Measures]",
        level: "[Measures].[MeasuresLevel]",
    };
    if matches_restrictions(
        restrictions,
        measures_coords.dim,
        measures_coords.hier,
        measures_coords.level,
    ) {
        for (name, content) in M_PROPS {
            if !property_requested(restrictions, name) {
                continue;
            }
            out.push_str(&member_property_row(
                catalog,
                cube,
                &measures_coords,
                name,
                *content,
                (*name == "MEMBER_VALUE").then_some(DBTYPE_R8),
                2,
            ));
            out.push('\n');
        }
    }

    out
}

fn system_property_rows() -> String {
    const PROPS: &[(&str, u8)] = &[
        ("VALUE", 0),
        ("FORMATTED_VALUE", 1),
        ("FORMAT_STRING", 2),
        ("FORE_COLOR", 2),
        ("BACK_COLOR", 2),
        ("FONT_NAME", 2),
        ("FONT_SIZE", 2),
        ("CELL_ORDINAL", 0),
    ];

    let project = proxy_project::project();
    let catalog = &project.config.catalog;
    let cube = &project.config.cube;
    let mut out = String::new();
    for (name, content) in PROPS {
        out.push_str(&format!(
            r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <PROPERTY_NAME>{}</PROPERTY_NAME>
            <PROPERTY_CAPTION>{}</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>{}</PROPERTY_CONTENT_TYPE>
          </row>
"#,
            name, name, content,
        ));
    }
    out
}

fn member_value_rows(restrictions: &Restrictions) -> String {
    let project = proxy_project::project();
    let model = &project.model;
    let catalog = &project.config.catalog;
    let cube = &project.config.cube;
    let mut out = String::new();

    // [Measures] MEMBER_VALUE row (special case). Excel's cache build expects
    // this row to be present; removing it made the key-attribute marking in
    // the pivot cache definition disappear (plan 048).
    if property_requested(restrictions, "MEMBER_VALUE")
        && matches_restrictions(
            restrictions,
            "[Measures]",
            "[Measures]",
            "[Measures].[MeasuresLevel]",
        )
    {
        out.push_str(&format!(
            r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>[Measures]</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>[Measures].[MeasuresLevel]</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>5</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
            <DATA_TYPE>{data_type}</DATA_TYPE>
            <PROPERTY_ORIGIN>2</PROPERTY_ORIGIN>
            <PROPERTY_IS_VISIBLE>true</PROPERTY_IS_VISIBLE>
          </row>
"#,
            data_type = DBTYPE_R8,
        ));
    }

    for d in &model.dimensions {
        let dim = &d.dimension_unique_name();
        for (hier, level, data_type) in member_value_targets(d) {
            if !matches_restrictions(restrictions, dim, &hier, &level) {
                continue;
            }
            let origin = if !d.levels.is_empty() && hier == d.hierarchy_unique_name() {
                1
            } else {
                2
            };
            out.push_str(&format!(
                r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>{dim}</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>{hier}</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>{level}</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>5</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
            <DATA_TYPE>{data_type}</DATA_TYPE>
            <PROPERTY_ORIGIN>{origin}</PROPERTY_ORIGIN>
            <PROPERTY_IS_VISIBLE>true</PROPERTY_IS_VISIBLE>
          </row>
"#,
            ));
        }
    }

    out
}

pub fn get_mdschema_properties_response(
    property_type: Option<i32>,
    restrictions: &Restrictions,
) -> String {
    let rows = match property_type {
        Some(1) => member_property_rows(restrictions),
        Some(2) => system_property_rows(),
        Some(5) => member_value_rows(restrictions),
        _ => format!(
            "{}\n{}",
            system_property_rows(),
            member_value_rows(restrictions)
        ),
    };
    discover_rowset_envelope("", PROPERTIES_ROW_FIELDS, &rows)
}

#[cfg(test)]
mod tests {
    use crate::project::project::ProxyProject;
    use crate::project::project::with_test_project;
    use crate::xmla::parser::Restrictions;

    fn member_value_row(resp: &str, level_unique_name: &str) -> String {
        let needle = format!("<LEVEL_UNIQUE_NAME>{level_unique_name}</LEVEL_UNIQUE_NAME>");
        resp.split("<row>")
            .find(|r| {
                r.contains(&needle) && r.contains("<PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>")
            })
            .unwrap_or_else(|| panic!("no MEMBER_VALUE row for {level_unique_name}"))
            .to_string()
    }

    #[test]
    fn date_key_attribute_member_value_is_a_date_type() {
        // Excel only offers date filters on an OLAP pivot when the key
        // attribute's MEMBER_VALUE DATA_TYPE is a date type (plan 048).
        let p = ProxyProject::load("projects/project3/proxy-config.json").expect("load project3");
        with_test_project(p, || {
            let resp = super::get_mdschema_properties_response(None, &Restrictions::default());
            let date_row = member_value_row(&resp, "[Date].[Calendar].[Full Date]");
            assert!(date_row.contains("<DATA_TYPE>7</DATA_TYPE>"), "{date_row}");
            let cat_row = member_value_row(&resp, "[Category].[Category].[Category]");
            assert!(cat_row.contains("<DATA_TYPE>130</DATA_TYPE>"), "{cat_row}");
        });
    }

    #[test]
    fn hierarchy_restriction_filters_rows() {
        // Excel asks for one hierarchy's member properties at a time while it
        // builds pivot cache fields; rows for other hierarchies corrupt the
        // cache field and make Excel refuse to add the field (plan 048).
        let p = ProxyProject::load("projects/project3/proxy-config.json").expect("load project3");
        with_test_project(p, || {
            let restrictions = Restrictions {
                hierarchy_unique_name: Some("[Date].[Full Date]".into()),
                ..Restrictions::default()
            };
            let resp = super::get_mdschema_properties_response(Some(1), &restrictions);
            assert!(
                resp.contains("<HIERARCHY_UNIQUE_NAME>[Date].[Full Date]</HIERARCHY_UNIQUE_NAME>"),
                "{resp}"
            );
            assert!(
                !resp.contains("<HIERARCHY_UNIQUE_NAME>[Date].[Calendar]</HIERARCHY_UNIQUE_NAME>"),
                "{resp}"
            );
            assert!(
                !resp.contains(
                    "<HIERARCHY_UNIQUE_NAME>[Category].[Category]</HIERARCHY_UNIQUE_NAME>"
                ),
                "{resp}"
            );
            assert!(
                !resp.contains("<DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>"),
                "{resp}"
            );
            let row = member_value_row(&resp, "[Date].[Full Date].[Full Date]");
            assert!(row.contains("<DATA_TYPE>7</DATA_TYPE>"), "{row}");
        });
    }
}
