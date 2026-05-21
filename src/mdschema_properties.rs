use crate::response::discover_rowset_envelope;

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

/// Helper: emit a property row anchored at a (dim, hier, level) for PROPERTY_TYPE=1
/// (intrinsic MEMBER properties). Filters in MDSCHEMA_PROPERTIES use
/// HIERARCHY_UNIQUE_NAME, so it MUST be present on every row.
fn member_property_row(
    dim: &str,
    hier: &str,
    level: &str,
    prop_name: &str,
    content_type: u8,
) -> String {
    format!(
        r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
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

/// Intrinsic MEMBER properties (PROPERTY_TYPE=1) for the Produktkategori
/// hierarchy. Emitted at both levels of the hierarchy so Excel can construct
/// axis queries against either the (All) level or the leaf level.
fn member_property_rows() -> String {
    const HIER: &str = "[Produktkategori].[Produktkategori]";
    const DIM: &str = "[Produktkategori]";
    const LEVEL_ALL: &str = "[Produktkategori].[Produktkategori].[(All)]";
    const LEVEL_LEAF: &str = "[Produktkategori].[Produktkategori].[Produktkategori]";

    // Standard intrinsic member property names + their PROPERTY_CONTENT_TYPE.
    // Content types: 0 = Regular, 1 = Id, 2 = Relation_to_parent, 4 = Property_formatting.
    // We use 0 throughout; Excel doesn't appear to enforce subtypes here.
    let props: [(&str, u8); 12] = [
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

    let mut out = String::new();
    for level in [LEVEL_ALL, LEVEL_LEAF] {
        for (name, content) in props.iter() {
            out.push_str(&member_property_row(DIM, HIER, level, name, *content));
            out.push('\n');
        }
    }
    // Also emit member properties for the hidden [Measures] hierarchy so
    // Excel can construct references to [Measures].[Total Försäljning].
    const M_DIM: &str = "[Measures]";
    const M_HIER: &str = "[Measures]";
    const M_LEVEL: &str = "[Measures].[MeasuresLevel]";
    let meas_props: [(&str, u8); 4] = [
        ("MEMBER_CAPTION", 0),
        ("MEMBER_NAME", 0),
        ("MEMBER_UNIQUE_NAME", 1),
        ("MEMBER_VALUE", 0),
    ];
    for (name, content) in meas_props.iter() {
        out.push_str(&member_property_row(M_DIM, M_HIER, M_LEVEL, name, *content));
        out.push('\n');
    }
    out
}

/// CELL properties (PROPERTY_TYPE=2). These are cube-scoped, not level-scoped,
/// so we omit `LEVEL_UNIQUE_NAME` entirely (the schema declares it minOccurs=0).
fn system_property_rows() -> String {
    let props: [(&str, u8); 8] = [
        ("VALUE", 0),
        ("FORMATTED_VALUE", 1),
        ("FORMAT_STRING", 2),
        ("FORE_COLOR", 2),
        ("BACK_COLOR", 2),
        ("FONT_NAME", 2),
        ("FONT_SIZE", 2),
        ("CELL_ORDINAL", 0),
    ];

    let mut out = String::new();
    for (name, content) in props.iter() {
        out.push_str(&format!(
            r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>[Measures]</DIMENSION_UNIQUE_NAME>
            <PROPERTY_NAME>{name}</PROPERTY_NAME>
            <PROPERTY_CAPTION>{name}</PROPERTY_CAPTION>
            <PROPERTY_TYPE>2</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>{content}</PROPERTY_CONTENT_TYPE>
          </row>
"#,
        ));
    }
    out
}

/// MEMBER_VALUE-style rows (PROPERTY_TYPE=5). Includes the hidden Measures
/// dimension and the Produktkategori hierarchy at the (All) and leaf levels.
fn member_value_rows() -> String {
    const DIM: &str = "[Produktkategori]";
    const HIER: &str = "[Produktkategori].[Produktkategori]";
    const LEVEL_ALL: &str = "[Produktkategori].[Produktkategori].[(All)]";
    const LEVEL_LEAF: &str = "[Produktkategori].[Produktkategori].[Produktkategori]";

    let mut out = String::new();
    // Hidden Measures hierarchy row (restored for metadata consistency)
    out.push_str(&format!(
        r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
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
    for level in [LEVEL_ALL, LEVEL_LEAF] {
        out.push_str(&format!(
            r#"          <row>
            <CATALOG_NAME>KTH_KEX_MALLOY_CUBE</CATALOG_NAME>
            <CUBE_NAME>Model</CUBE_NAME>
            <DIMENSION_UNIQUE_NAME>{DIM}</DIMENSION_UNIQUE_NAME>
            <HIERARCHY_UNIQUE_NAME>{HIER}</HIERARCHY_UNIQUE_NAME>
            <LEVEL_UNIQUE_NAME>{level}</LEVEL_UNIQUE_NAME>
            <PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>
            <PROPERTY_CAPTION>MEMBER_VALUE</PROPERTY_CAPTION>
            <PROPERTY_TYPE>5</PROPERTY_TYPE>
            <PROPERTY_CONTENT_TYPE>0</PROPERTY_CONTENT_TYPE>
          </row>
"#,
        ));
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
