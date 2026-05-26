use crate::response::discover_rowset_envelope;
use crate::proxy_project;

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
                <xsd:element sql:field="PROPERTY_CONTENT_TYPE" name="PROPERTY_CONTENT_TYPE" type="xsd:short" minOccurs="0"/>"#;

fn member_property_row(
    catalog: &str,
    cube: &str,
    dim: &str,
    hier: &str,
    level: &str,
    prop_name: &str,
    content_type: u8,
) -> String {
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
            <PROPERTY_CONTENT_TYPE>{content_type}</PROPERTY_CONTENT_TYPE>
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
            for (name, content) in PROPS {
                out.push_str(&member_property_row(catalog, cube, dim, hier, level, name, *content));
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
    for (name, content) in M_PROPS {
        out.push_str(&member_property_row(
            catalog, cube,
            "[Measures]", "[Measures]", "[Measures].[MeasuresLevel]",
            name, *content,
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

    // [Measures] MEMBER_VALUE row (special case)
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
          </row>
"#,
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
          </row>
"#,
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
