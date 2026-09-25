use crate::proxy_project;
use crate::response::discover_rowset_envelope;

use crate::engine::model::{DimensionDef, LevelDef};
use crate::xmla::parser::Restrictions;

// OLE DB DBTYPEs used for MEMBER_VALUE rows. Excel offers its Date Filters only
// on a field whose MEMBER_VALUE DATA_TYPE is a date type (7 = DBTYPE_DATE, the
// value the OOXML `memberValueDatatype` note names); the other levels keep
// their key types (plan 048).
const DBTYPE_I4: i32 = 3;
const DBTYPE_UI8: i32 = 20;
const DBTYPE_DATE: i32 = 7;
const DBTYPE_WSTR: i32 = 130;

/// MEMBER_VALUE / KEY0 data type for one level: the level's key type. The
/// tabular reference reports integer keys as `DBTYPE_UI8` (20) and the date
/// leaf as `DBTYPE_DATE` (7); Excel copies the key attribute's type into the
/// pivot cache's `memberValueDatatype` (plan 048).
fn level_member_value_type(d: &DimensionDef, level: &LevelDef) -> i32 {
    if !d.is_date_role {
        return DBTYPE_WSTR;
    }
    let deepest = d.levels.iter().map(|l| l.level_number).max();
    if Some(level.level_number) == deepest {
        DBTYPE_DATE
    } else {
        DBTYPE_UI8
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

// The MDSCHEMA_PROPERTIES row schema, byte-for-byte the reference's: every
// field is optional and the numeric types match. Excel validates the rows
// against it, and rows that omit a field the schema marks required are
// rejected outright — which aborted its whole metadata sweep and left the
// pivot cache without measure fields, so a measure could not be placed in
// Values at all (plan 049 regression).
const PROPERTIES_ROW_FIELDS: &str = r#"                <xsd:element sql:field="CATALOG_NAME" name="CATALOG_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="SCHEMA_NAME" name="SCHEMA_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="CUBE_NAME" name="CUBE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DIMENSION_UNIQUE_NAME" name="DIMENSION_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="HIERARCHY_UNIQUE_NAME" name="HIERARCHY_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LEVEL_UNIQUE_NAME" name="LEVEL_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MEMBER_UNIQUE_NAME" name="MEMBER_UNIQUE_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_TYPE" name="PROPERTY_TYPE" type="xsd:short" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_NAME" name="PROPERTY_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_CAPTION" name="PROPERTY_CAPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="DATA_TYPE" name="DATA_TYPE" type="xsd:unsignedShort" minOccurs="0"/>
                <xsd:element sql:field="CHARACTER_MAXIMUM_LENGTH" name="CHARACTER_MAXIMUM_LENGTH" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="CHARACTER_OCTET_LENGTH" name="CHARACTER_OCTET_LENGTH" type="xsd:unsignedInt" minOccurs="0"/>
                <xsd:element sql:field="NUMERIC_PRECISION" name="NUMERIC_PRECISION" type="xsd:unsignedShort" minOccurs="0"/>
                <xsd:element sql:field="NUMERIC_SCALE" name="NUMERIC_SCALE" type="xsd:short" minOccurs="0"/>
                <xsd:element sql:field="DESCRIPTION" name="DESCRIPTION" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_CONTENT_TYPE" name="PROPERTY_CONTENT_TYPE" type="xsd:short" minOccurs="0"/>
                <xsd:element sql:field="SQL_COLUMN_NAME" name="SQL_COLUMN_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="LANGUAGE" name="LANGUAGE" type="xsd:unsignedShort" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_ORIGIN" name="PROPERTY_ORIGIN" type="xsd:unsignedShort" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_ATTRIBUTE_HIERARCHY_NAME" name="PROPERTY_ATTRIBUTE_HIERARCHY_NAME" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="PROPERTY_CARDINALITY" name="PROPERTY_CARDINALITY" type="xsd:string" minOccurs="0"/>
                <xsd:element sql:field="MIME_TYPE" name="MIME_TYPE" type="xsd:string" minOccurs="0"/>
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
    property_type: u8,
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
            <PROPERTY_TYPE>{property_type}</PROPERTY_TYPE>
            <PROPERTY_NAME>{prop_name}</PROPERTY_NAME>
            <PROPERTY_CAPTION>{prop_name}</PROPERTY_CAPTION>{data_type_xml}
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

/// Is this level the hierarchy's `(All)` level?
fn is_all_level(d: &DimensionDef, level: &str) -> bool {
    level.ends_with(&format!(".[{}]", d.all_level_name))
}

/// The property rows the tabular reference returns for one hierarchy: per
/// level `KEY0` and `MEMBER_VALUE`, plus `NAME` on the `(All)` level, all
/// `PROPERTY_TYPE=5` (internal member property). Excel reads the key
/// attribute's `MEMBER_VALUE` from here for Date Filters (plan 048), and this
/// shape — rather than a fabricated list of the standard member properties —
/// is what keeps its pivot MDX on the short `DIMENSION PROPERTIES
/// PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME` form the reference sees.
fn hierarchy_property_rows(
    restrictions: &Restrictions,
    user: &crate::engine::model::UserContext,
    config: &crate::project::config::ProxyConfig,
) -> String {
    let project = proxy_project::project();
    if !super::in_scope(restrictions, &project.config.catalog, &project.config.cube) {
        // A request naming another catalog or cube is out of scope: the
        // reference answers an empty rowset in this rowset's shape, not a
        // fault (measured 2026-09-25).
        return discover_rowset_envelope("", PROPERTIES_ROW_FIELDS, "");
    }

    let model = &project.model;
    let catalog = &project.config.catalog;
    let cube = &project.config.cube;
    let mut out = String::new();

    for d in &model.dimensions {
        if !super::dimension_visible(model, config, user, &d.id) {
            continue;
        }
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
            // The reference emits KEY0, then NAME on the (All) level, then
            // MEMBER_VALUE; Excel reads these positionally.
            let mut names: Vec<&str> = vec!["KEY0"];
            if is_all_level(d, &level) {
                names.push("NAME");
            }
            names.push("MEMBER_VALUE");
            for name in names {
                if !property_requested(restrictions, name) {
                    continue;
                }
                // KEY0 carries the level's key type too — that is where Excel
                // reads the key attribute's type from when it writes
                // `memberValueDatatype` into the pivot cache (plan 048). The
                // `(All)` level reports I4 for KEY0 but WSTR for MEMBER_VALUE,
                // as the reference does.
                let is_all = is_all_level(d, &level);
                let data_type = match name {
                    // NAME is the caption: always a string.
                    "NAME" => DBTYPE_WSTR,
                    "MEMBER_VALUE" if is_all => data_type,
                    _ if is_all => DBTYPE_I4,
                    _ => data_type,
                };
                out.push_str(&member_property_row(
                    catalog,
                    cube,
                    &coords,
                    name,
                    5,
                    Some(data_type),
                    origin,
                ));
                out.push('\n');
            }
        }
    }

    // [Measures] intrinsic rows (special case); hidden with the last measure
    // when no measure's table is visible.
    let measures_coords = RowCoords {
        dim: "[Measures]",
        hier: "[Measures]",
        level: "[Measures].[MeasuresLevel]",
    };
    if super::measures_visible(model, config, user)
        && matches_restrictions(
            restrictions,
            measures_coords.dim,
            measures_coords.hier,
            measures_coords.level,
        )
    {
        for name in ["KEY0", "MEMBER_VALUE"] {
            if !property_requested(restrictions, name) {
                continue;
            }
            out.push_str(&member_property_row(
                catalog,
                cube,
                &measures_coords,
                name,
                5,
                (name == "MEMBER_VALUE").then_some(DBTYPE_WSTR),
                2,
            ));
            out.push('\n');
        }
    }

    out
}

/// The cell-property advertisement, exactly as the reference sends it: one row
/// per property with `PROPERTY_TYPE`, `PROPERTY_NAME`, `PROPERTY_CAPTION` and
/// `DATA_TYPE` — in that order, and with the reference's DBTYPEs. Excel reads
/// these positionally; our earlier rows carried `CATALOG_NAME`/`CUBE_NAME`/
/// `DIMENSION_UNIQUE_NAME` and a `PROPERTY_CONTENT_TYPE` the reference does not
/// have, and with the full property list that made Excel refuse to place a
/// measure in Values at all (found by bisecting plan 049's regression).
fn system_property_rows(restrictions: &Restrictions) -> String {
    const PROPS: &[(&str, i32)] = &[
        ("VALUE", 12),
        ("FORMAT_STRING", 130),
        ("BACK_COLOR", 19),
        ("FORE_COLOR", 19),
        ("FONT_NAME", 130),
        ("FONT_SIZE", 18),
        ("FONT_FLAGS", 3),
        ("LANGUAGE", 19),
        ("CELL_ORDINAL", 19),
        ("FORMATTED_VALUE", 130),
        ("ACTION_TYPE", 19),
        ("UPDATEABLE", 19),
    ];

    let mut out = String::new();
    for (name, data_type) in PROPS {
        if !property_requested(restrictions, name) {
            continue;
        }
        out.push_str(&format!(
            r#"          <row>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_NAME>{name}</PROPERTY_NAME>
            <PROPERTY_CAPTION>{name}</PROPERTY_CAPTION>
            <DATA_TYPE>{data_type}</DATA_TYPE>
          </row>
"#,
        ));
    }
    out
}

fn member_value_rows(
    restrictions: &Restrictions,
    user: &crate::engine::model::UserContext,
    config: &crate::project::config::ProxyConfig,
) -> String {
    let project = proxy_project::project();
    let model = &project.model;
    let catalog = &project.config.catalog;
    let cube = &project.config.cube;
    let mut out = String::new();

    // The reference lists member-value rows sorted by hierarchy and Excel's
    // cache marking (memberValueDatatype) follows that order; emitting
    // [Measures] first made every hierarchy inherit its type (plan 048).
    let mut targets: Vec<(String, String, String, i32, u32)> = Vec::new();
    for d in &model.dimensions {
        if !super::dimension_visible(model, config, user, &d.id) {
            continue;
        }
        let dim = d.dimension_unique_name();
        for (hier, level, data_type) in member_value_targets(d) {
            if !matches_restrictions(restrictions, &dim, &hier, &level) {
                continue;
            }
            let origin = if !d.levels.is_empty() && hier == d.hierarchy_unique_name() {
                1
            } else {
                2
            };
            targets.push((hier, level, dim.clone(), data_type, origin));
        }
    }
    targets.sort_by(|a, b| a.0.cmp(&b.0));
    for (hier, level, dim, data_type, origin) in targets {
        out.push_str(&format!(
            r#"          <row>
            <CATALOG_NAME>{catalog}</CATALOG_NAME>
            <CUBE_NAME>{cube}</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>{dim}</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>{hier}</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>{level}</LEVEL_UNIQUE_NAME>
            <PROPERTY_TYPE>5</PROPERTY_TYPE>
            <PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_VALUE</PROPERTY_CAPTION>
            <DATA_TYPE>{data_type}</DATA_TYPE>
            <PROPERTY_ORIGIN>{origin}</PROPERTY_ORIGIN>
            <PROPERTY_IS_VISIBLE>true</PROPERTY_IS_VISIBLE>
          </row>
"#,
        ));
    }

    // [Measures] last, as the reference does, and typed WSTR there. Hidden
    // with the last measure when no measure's table is visible.
    if super::measures_visible(model, config, user)
        && property_requested(restrictions, "MEMBER_VALUE")
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
            <PROPERTY_TYPE>5</PROPERTY_TYPE>
            <PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_VALUE</PROPERTY_CAPTION>
            <DATA_TYPE>{data_type}</DATA_TYPE>
            <PROPERTY_ORIGIN>2</PROPERTY_ORIGIN>
            <PROPERTY_IS_VISIBLE>true</PROPERTY_IS_VISIBLE>
          </row>
"#,
            data_type = DBTYPE_WSTR,
        ));
    }

    out
}

/// `MDSCHEMA_PROPERTIES` as the tabular reference answers it (measured against
/// a processed SSAS 2025 tabular model, plan 048):
///
/// * `PROPERTY_TYPE=1` (member properties): the reference has none — every
///   property it exposes is `PROPERTY_TYPE=5`. Excel reads an empty answer as
///   "this field has no member properties" and shows *(No Properties
///   Retrieved)*; answering with the hierarchy's own rows instead made it
///   create `memberPropertyField` cache fields (`…KEY0`, `…MEMBER_VALUE`) and
///   ask for those in every pivot MDX.
/// * `PROPERTY_TYPE=2` (cell properties): only the provider-level list, i.e.
///   when the request names no cube or hierarchy. A cube-scoped cell-property
///   request answers empty.
/// * `PROPERTY_TYPE=5`: the hierarchy's `KEY0` / `NAME` / `MEMBER_VALUE` rows.
/// * No type but a cube or hierarchy: those rows, never the cell properties.
pub fn get_mdschema_properties_response(
    property_type: Option<i32>,
    restrictions: &Restrictions,
    user: &crate::engine::model::UserContext,
    config: &crate::project::config::ProxyConfig,
) -> String {
    // Every branch is object metadata for the local model, so the scope check
    // runs once here — a catalog-only mismatch used to reach the rows (plan 051
    // review).
    let project = proxy_project::project();
    if !super::in_scope(restrictions, &project.config.catalog, &project.config.cube) {
        return discover_rowset_envelope("", PROPERTIES_ROW_FIELDS, "");
    }
    if super::hidden_by_visibility(restrictions.property_visibility) {
        return discover_rowset_envelope("", PROPERTIES_ROW_FIELDS, "");
    }
    let cube_scoped = restrictions.cube_name.is_some()
        || restrictions.dimension_unique_name.is_some()
        || restrictions.hierarchy_unique_name.is_some()
        || restrictions.level_unique_name.is_some();
    let rows = match property_type {
        // Member properties: the reference has no type-1 rows to report.
        Some(1) | Some(3) | Some(4) => String::new(),
        Some(2) if !cube_scoped => system_property_rows(restrictions),
        Some(2) => String::new(),
        // Member properties: the reference answers with the hierarchy's own
        // rows (KEY0 / NAME / MEMBER_VALUE), never a standard member-property
        // list. That list made Excel request 38 properties in its pivot MDX
        // where the reference is asked for two.
        Some(5) => hierarchy_property_rows(restrictions, user, config),
        // A request that names one hierarchy gets only that hierarchy's rows;
        // mixing the cell properties in is what Excel rejects (plan 048).
        _ if cube_scoped => hierarchy_property_rows(restrictions, user, config),
        _ => format!(
            "{}\n{}",
            system_property_rows(restrictions),
            member_value_rows(restrictions, user, config)
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

    /// The row for one level and property, or empty when there is none.
    fn row_for(resp: &str, level_unique_name: &str, prop: &str) -> String {
        let level_needle = format!("<LEVEL_UNIQUE_NAME>{level_unique_name}</LEVEL_UNIQUE_NAME>");
        let prop_needle = format!("<PROPERTY_NAME>{prop}</PROPERTY_NAME>");
        resp.split("<row>")
            .find(|r| r.contains(&level_needle) && r.contains(&prop_needle))
            .map(|r| r.to_string())
            .unwrap_or_default()
    }

    #[test]
    fn hierarchy_rows_match_the_tabular_reference_shape() {
        // The reference answers a hierarchy-restricted request with, per level,
        // KEY0 and MEMBER_VALUE — plus NAME on the (All) level — all
        // PROPERTY_TYPE=5, and no cell properties. That shape is what keeps
        // Excel's pivot MDX on the short property list the reference sees.
        let p = ProxyProject::load("projects/project3/proxy-config.json").expect("load project3");
        with_test_project(p, || {
            let restrictions = Restrictions {
                hierarchy_unique_name: Some("[Date].[Calendar]".into()),
                ..Restrictions::default()
            };
            let resp = super::get_mdschema_properties_response(
                None,
                &restrictions,
                &crate::engine::model::UserContext::admin_default(),
                &crate::proxy_project::project().config,
            );
            assert!(
                resp.contains("<PROPERTY_NAME>KEY0</PROPERTY_NAME>"),
                "{resp}"
            );
            assert!(
                resp.contains("<PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>"),
                "{resp}"
            );
            assert!(resp.contains("<PROPERTY_TYPE>5</PROPERTY_TYPE>"), "{resp}");
            assert!(!resp.contains("<PROPERTY_TYPE>2</PROPERTY_TYPE>"), "{resp}");
            assert!(
                !resp.contains("<PROPERTY_NAME>MEMBER_CAPTION</PROPERTY_NAME>"),
                "{resp}"
            );
            // NAME only on the (All) level.
            let all_row = row_for(&resp, "[Date].[Calendar].[(All)]", "NAME");
            assert!(
                all_row.contains("<PROPERTY_NAME>NAME</PROPERTY_NAME>"),
                "{all_row}"
            );
            assert!(
                row_for(&resp, "[Date].[Calendar].[Year]", "NAME").is_empty(),
                "NAME should only be on the (All) level"
            );
            // KEY0 carries the level's key type — Excel reads the key
            // attribute's type from here for the cache's memberValueDatatype.
            let date_key = row_for(&resp, "[Date].[Calendar].[Full Date]", "KEY0");
            assert!(date_key.contains("<DATA_TYPE>7</DATA_TYPE>"), "{date_key}");
            let year_key = row_for(&resp, "[Date].[Calendar].[Year]", "KEY0");
            assert!(year_key.contains("<DATA_TYPE>20</DATA_TYPE>"), "{year_key}");
            assert!(all_row.contains("<DATA_TYPE>130</DATA_TYPE>"), "{all_row}");
        });
    }

    // Excel sends `PROPERTY_TYPE=5` with no `PROPERTY_NAME` when it wants a
    // hierarchy's member properties; the tabular reference answers with
    // KEY0 / NAME / MEMBER_VALUE. Returning member-value rows alone made Excel
    // ask for KEY0/MEMBER_VALUE in its pivot MDX where the reference is asked
    // for PARENT_UNIQUE_NAME/HIERARCHY_UNIQUE_NAME (found by diffing Excel's
    // requests to a mirror tabular model).
    #[test]
    fn member_property_type_returns_the_hierarchy_rows() {
        let p = ProxyProject::load("projects/project3/proxy-config.json").expect("load project3");
        with_test_project(p, || {
            let restrictions = Restrictions {
                hierarchy_unique_name: Some("[Category].[Category]".into()),
                ..Restrictions::default()
            };
            let resp = super::get_mdschema_properties_response(
                Some(5),
                &restrictions,
                &crate::engine::model::UserContext::admin_default(),
                &crate::proxy_project::project().config,
            );
            for name in ["KEY0", "NAME", "MEMBER_VALUE"] {
                assert!(
                    resp.contains(&format!("<PROPERTY_NAME>{name}</PROPERTY_NAME>")),
                    "{name} missing: {resp}"
                );
            }
            // Naming one property still returns just that property.
            let restrictions = Restrictions {
                hierarchy_unique_name: Some("[Category].[Category]".into()),
                property_name: Some("MEMBER_VALUE".into()),
                ..Restrictions::default()
            };
            let resp = super::get_mdschema_properties_response(
                Some(5),
                &restrictions,
                &crate::engine::model::UserContext::admin_default(),
                &crate::proxy_project::project().config,
            );
            assert!(
                resp.contains("<PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>"),
                "{resp}"
            );
            assert!(
                !resp.contains("<PROPERTY_NAME>KEY0</PROPERTY_NAME>"),
                "a named property must be the only one returned: {resp}"
            );
        });
    }

    #[test]
    fn date_key_attribute_member_value_is_a_date_type() {
        // Excel only offers date filters on an OLAP pivot when the key
        // attribute's MEMBER_VALUE DATA_TYPE is a date type (plan 048).
        let p = ProxyProject::load("projects/project3/proxy-config.json").expect("load project3");
        with_test_project(p, || {
            let resp = super::get_mdschema_properties_response(
                None,
                &Restrictions::default(),
                &crate::engine::model::UserContext::admin_default(),
                &crate::proxy_project::project().config,
            );
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
            let resp = super::get_mdschema_properties_response(
                Some(5),
                &restrictions,
                &crate::engine::model::UserContext::admin_default(),
                &crate::proxy_project::project().config,
            );
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

    #[test]
    fn member_property_requests_answer_empty_like_the_reference() {
        // The tabular reference exposes no PROPERTY_TYPE=1 rows at all; Excel
        // reads the empty answer as "(No Properties Retrieved)" and keeps its
        // pivot cache free of member-property fields. Answering with the
        // hierarchy rows made it write `…KEY0` / `…MEMBER_VALUE` cache fields
        // and request those in every pivot MDX (plan 048).
        let p = ProxyProject::load("projects/project3/proxy-config.json").expect("load project3");
        with_test_project(p, || {
            let restrictions = Restrictions {
                cube_name: Some("Sales".into()),
                hierarchy_unique_name: Some("[Category].[Category]".into()),
                ..Restrictions::default()
            };
            for property_type in [1, 3, 4] {
                let resp = super::get_mdschema_properties_response(
                    Some(property_type),
                    &restrictions,
                    &crate::engine::model::UserContext::admin_default(),
                    &crate::proxy_project::project().config,
                );
                assert!(
                    !resp.contains("<row>"),
                    "PROPERTY_TYPE={property_type} must answer empty: {resp}"
                );
            }
        });
    }

    #[test]
    fn cell_properties_only_answer_a_provider_level_request() {
        // Excel probes PROPERTY_TYPE=2 without a cube before it knows the
        // catalog; the reference answers with the 12 cell properties. Naming a
        // cube (or hierarchy) answers empty, as the reference does.
        let p = ProxyProject::load("projects/project3/proxy-config.json").expect("load project3");
        with_test_project(p, || {
            let resp = super::get_mdschema_properties_response(
                Some(2),
                &Restrictions::default(),
                &crate::engine::model::UserContext::admin_default(),
                &crate::proxy_project::project().config,
            );
            for name in ["VALUE", "FORMAT_STRING", "FONT_FLAGS", "UPDATEABLE"] {
                assert!(
                    resp.contains(&format!("<PROPERTY_NAME>{name}</PROPERTY_NAME>")),
                    "{name} missing: {resp}"
                );
            }
            assert_eq!(
                resp.matches("<PROPERTY_TYPE>2</PROPERTY_TYPE>").count(),
                12,
                "{resp}"
            );

            let scoped = Restrictions {
                cube_name: Some("Sales".into()),
                ..Restrictions::default()
            };
            let resp = super::get_mdschema_properties_response(
                Some(2),
                &scoped,
                &crate::engine::model::UserContext::admin_default(),
                &crate::proxy_project::project().config,
            );
            assert!(
                !resp.contains("<row>"),
                "cube-scoped cell properties: {resp}"
            );

            // A named cell property still filters the provider-level list.
            let named = Restrictions {
                property_name: Some("FORMAT_STRING".into()),
                ..Restrictions::default()
            };
            let resp = super::get_mdschema_properties_response(
                Some(2),
                &named,
                &crate::engine::model::UserContext::admin_default(),
                &crate::proxy_project::project().config,
            );
            assert!(
                resp.contains("<PROPERTY_NAME>FORMAT_STRING</PROPERTY_NAME>"),
                "{resp}"
            );
            assert!(
                !resp.contains("<PROPERTY_NAME>VALUE</PROPERTY_NAME>"),
                "{resp}"
            );
        });
    }
}
