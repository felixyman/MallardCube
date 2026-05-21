use crate::response::wrap_in_soap_envelope;
use crate::cellset;

pub fn get_empty_execute_response() -> String {
    wrap_in_soap_envelope(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:empty"/>
      </return>
    </ExecuteResponse>"#,
    )
}

fn is_dax(statement: &str) -> bool {
    let trimmed = statement.trim_start();
    let upper = trimmed.to_uppercase();
    upper.starts_with("EVALUATE") || upper.starts_with("DEFINE")
}

fn is_mdx_select(mdx: &str) -> bool {
    mdx.trim_start().to_uppercase().starts_with("SELECT")
}

pub fn get_execute_statement_response(statement: &str) -> String {
    if is_dax(statement) {
        get_execute_dax_response(statement)
    } else if is_mdx_select(statement) {
        get_execute_cellset_response(statement)
    } else {
        get_execute_mdx_response(statement)
    }
}

// ---- helpers for building cellset data ----

fn produktkategori_dim_props(name: &str) -> Vec<(String, String)> {
    vec![
        ("PARENT_UNIQUE_NAME".into(), "[Produktkategori].[Produktkategori].[All]".into()),
        ("HIERARCHY_UNIQUE_NAME".into(), "[Produktkategori].[Produktkategori]".into()),
        ("MEMBER_NAME".into(), name.to_string()),
        ("MEMBER_KEY".into(), name.to_string()),
        ("MEMBER_TYPE".into(), "3".into()),
        ("MEMBER_VALUE".into(), name.to_string()),
        ("PARENT_LEVEL".into(), "0".into()),
        ("PARENT_COUNT".into(), "1".into()),
        ("CHILDREN_CARDINALITY".into(), "0".into()),
    ]
}

fn produktkategori_dim_decls() -> Vec<(String, String, String)> {
    let p = "[Produktkategori].[Produktkategori]";
    vec![
        ("PARENT_UNIQUE_NAME".into(),   format!("{p}.[PARENT_UNIQUE_NAME]"),   "xsd:string".into()),
        ("HIERARCHY_UNIQUE_NAME".into(),format!("{p}.[HIERARCHY_UNIQUE_NAME]"),"xsd:string".into()),
        ("MEMBER_NAME".into(),          format!("{p}.[MEMBER_NAME]"),          "xsd:string".into()),
        ("MEMBER_KEY".into(),           format!("{p}.[MEMBER_KEY]"),           "xsd:string".into()),
        ("MEMBER_TYPE".into(),          format!("{p}.[MEMBER_TYPE]"),          "xsd:int".into()),
        ("MEMBER_VALUE".into(),         format!("{p}.[MEMBER_VALUE]"),         "xsd:string".into()),
        ("PARENT_LEVEL".into(),         format!("{p}.[PARENT_LEVEL]"),         "xsd:int".into()),
        ("PARENT_COUNT".into(),         format!("{p}.[PARENT_COUNT]"),         "xsd:int".into()),
        ("CHILDREN_CARDINALITY".into(), format!("{p}.[CHILDREN_CARDINALITY]"), "xsd:unsignedInt".into()),
    ]
}

fn measurement_cell(ordinal: u32) -> cellset::CellConfig {
    cellset::CellConfig {
        ordinal,
        value: 1250000.5,
        fmt_value: "1,250,000.50 SEK".into(),
        format_string: "#,##0.00 SEK".into(),
        back_color: String::new(),
        fore_color: String::new(),
    }
}

fn measures_slicer_member() -> cellset::MemberConfig {
    cellset::MemberConfig {
        u_name: "[Measures].[Total Försäljning]".into(),
        caption: "Total Försäljning (SEK)".into(),
        l_name: "[Measures].[MeasuresLevel]".into(),
        l_num: 0,
        display_info: 3,
        dim_props: vec![],
    }
}

fn slicer_axis() -> cellset::AxisConfig {
    cellset::AxisConfig {
        name: "SlicerAxis".into(),
        hier_name: "[Measures]".into(),
        members: vec![measures_slicer_member()],
        dim_prop_decls: vec![],
    }
}

// ---- cellset response builders ----

/// Shape 1: slicer-only (e.g. dimension removed, measure stays).
/// `SELECT FROM [Model] WHERE ([Measures]...) CELL PROPERTIES ...`
fn build_slicer_only() -> String {
    let resp = cellset::CellsetResponse {
        cube_name: "Model".into(),
        axes: vec![slicer_axis()],
        cells: vec![measurement_cell(0)],
    };
    cellset::render_cellset(&resp)
}

/// Shape 2: hierarchy drilldown (e.g. first drag of Produktkategori to Rows).
/// `SELECT ... DrilldownLevel({[All]}) ... ON COLUMNS ...`
fn build_drilldown() -> String {
    let names = ["Kategori A", "Kategori B", "Kategori C", "Kategori D"];
    let mut members = Vec::new();
    for (_i, &name) in names.iter().enumerate() {
        let u_name = format!("[Produktkategori].[Produktkategori].&amp;[{}]", name);
        members.push(cellset::MemberConfig {
            u_name,
            caption: name.to_string(),
            l_name: "[Produktkategori].[Produktkategori].[Produktkategori]".into(),
            l_num: 1,
            display_info: 3,
            dim_props: produktkategori_dim_props(name),
        });
    }

    let mut cells = Vec::new();
    for i in 0..members.len() {
        cells.push(measurement_cell(i as u32));
    }

    let axis0 = cellset::AxisConfig {
        name: "Axis0".into(),
        hier_name: "[Produktkategori].[Produktkategori]".into(),
        members,
        dim_prop_decls: produktkategori_dim_decls(),
    };

    let resp = cellset::CellsetResponse {
        cube_name: "Model".into(),
        axes: vec![axis0, slicer_axis()],
        cells,
    };
    cellset::render_cellset(&resp)
}

fn get_execute_cellset_response(mdx: &str) -> String {
    let has_axes = mdx.contains("ON COLUMNS") || mdx.contains("ON ROWS");
    let is_drilldown = mdx.contains("[Produktkategori]")
        && (mdx.contains("DrilldownLevel") || mdx.contains(".Members"));

    if is_drilldown {
        build_drilldown()
    } else if !has_axes {
        build_slicer_only()
    } else {
        // multi-axis query we don't yet pattern-match — minimal fallback
        build_slicer_only()
    }
}

fn get_execute_mdx_response(mdx: &str) -> String {
    let has_measures = mdx.contains("Measures") || mdx.contains("measures");
    let measure_name = "Total_Forsaljning";
    let measure_value = if has_measures { "1250000.5" } else { "" };

    let inner = format!(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence minOccurs="0" maxOccurs="unbounded"><xsd:element name="row" type="row"/></xsd:sequence></xsd:complexType>
            </xsd:element>
            <xsd:complexType name="row">
              <xsd:sequence>
                <xsd:element sql:field="{name}" name="{name}" type="xsd:double" minOccurs="0"/>
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
          <row>
            <{name}>{val}</{name}>
          </row>
        </root>
      </return>
    </ExecuteResponse>"#,
        name = measure_name,
        val = measure_value,
    );
    wrap_in_soap_envelope(&inner)
}

/// Minimal DAX EVALUATE response: returns a single-row rowset with the
/// `Faktatabell[Total Försäljning (SEK)]` measure column.
fn get_execute_dax_response(_dax: &str) -> String {
    // DAX result columns are normally named `'Table'[Column]` — Excel will
    // accept the bracketed form. We use a column name aligned with the
    // measure caption so a drag-to-Values renders the expected number.
    let col_xml_name = "Faktatabell_x005B_Total_x0020_Försäljning_x0020__x0028_SEK_x0029__x005D_";
    let col_sql_field = "[Faktatabell].[Total Försäljning (SEK)]";

    let inner = format!(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence minOccurs="0" maxOccurs="unbounded"><xsd:element name="row" type="row"/></xsd:sequence></xsd:complexType>
            </xsd:element>
            <xsd:complexType name="row">
              <xsd:sequence>
                <xsd:element sql:field="{sqlf}" name="{xname}" type="xsd:double" minOccurs="0"/>
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
          <row>
            <{xname}>1250000.5</{xname}>
          </row>
        </root>
      </return>
    </ExecuteResponse>"#,
        sqlf = col_sql_field,
        xname = col_xml_name,
    );
    wrap_in_soap_envelope(&inner)
}
