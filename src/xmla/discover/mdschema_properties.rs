use crate::proxy_project;
use crate::response::discover_rowset_envelope;

// OLE DB DBTYPEs used for MEMBER_VALUE rows. Excel only offers its date
// filters on an OLAP pivot when the key attribute's MEMBER_VALUE DATA_TYPE is
// a date type (the OOXML `memberValueDatatype` note names 7); with no
// DATA_TYPE it falls back to label filters only (plan 048).
const DBTYPE_R8: i32 = 5;
const DBTYPE_DATE: i32 = 7;
const DBTYPE_WSTR: i32 = 130;

/// MEMBER_VALUE data type for a dimension's key attribute.
fn member_value_data_type(d: &crate::engine::model::DimensionDef) -> i32 {
    if d.is_date_role {
        DBTYPE_DATE
    } else {
        DBTYPE_WSTR
    }
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
                <xsd:element sql:field="DATA_TYPE" name="DATA_TYPE" type="xsd:unsignedShort" minOccurs="0"/>"#;

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
          </row>"#,
    )
}

fn member_property_rows() -> String {
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
        let hier = &d.hierarchy_unique_name();
        for level in &[d.all_level_unique_name(), d.leaf_level_unique_name()] {
            let coords = RowCoords { dim, hier, level };
            for (name, content) in PROPS {
                out.push_str(&member_property_row(
                    catalog,
                    cube,
                    &coords,
                    name,
                    *content,
                    (*name == "MEMBER_VALUE").then(|| member_value_data_type(d)),
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
    for (name, content) in M_PROPS {
        out.push_str(&member_property_row(
            catalog,
            cube,
            &measures_coords,
            name,
            *content,
            (*name == "MEMBER_VALUE").then_some(DBTYPE_R8),
        ));
        out.push('\n');
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

fn member_value_rows() -> String {
    let project = proxy_project::project();
    let model = &project.model;
    let catalog = &project.config.catalog;
    let cube = &project.config.cube;
    let mut out = String::new();

    // [Measures] MEMBER_VALUE row (special case). Excel's cache build expects
    // this row to be present; removing it made the key-attribute marking in
    // the pivot cache definition disappear (plan 048).
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
          </row>
"#,
        data_type = DBTYPE_R8,
    ));

    for d in &model.dimensions {
        let dim = &d.dimension_unique_name();
        let hier = &d.hierarchy_unique_name();
        for level in &[d.all_level_unique_name(), d.leaf_level_unique_name()] {
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
          </row>
"#,
                data_type = member_value_data_type(d),
            ));
        }
    }

    out
}

pub fn get_mdschema_properties_response(property_type: Option<i32>) -> String {
    let rows = match property_type {
        Some(1) => member_property_rows(),
        Some(2) => system_property_rows(),
        Some(5) => member_value_rows(),
        _ => format!("{}\n{}", system_property_rows(), member_value_rows()),
    };
    discover_rowset_envelope("", PROPERTIES_ROW_FIELDS, &rows)
}

#[cfg(test)]
mod tests {
    use crate::project::project::ProxyProject;
    use crate::project::project::with_test_project;

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
            let resp = super::get_mdschema_properties_response(None);
            let date_row = member_value_row(&resp, "[Date].[Date].[Date]");
            assert!(date_row.contains("<DATA_TYPE>7</DATA_TYPE>"), "{date_row}");
            let cat_row = member_value_row(&resp, "[Category].[Category].[Category]");
            assert!(cat_row.contains("<DATA_TYPE>130</DATA_TYPE>"), "{cat_row}");
        });
    }
}
