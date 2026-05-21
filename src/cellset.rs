/// Generic cellset (mddataset) XML builder.
///
/// Builds the complete `<root xmlns="...mddataset">` response including
/// schema, OlapInfo, Axes, and CellData — driven entirely by the struct
/// fields below.  No hard‑coded hierarchy or member names.

use crate::response::wrap_in_soap_envelope;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// One member on an axis.
pub struct MemberConfig {
    pub u_name: String,
    pub caption: String,
    pub l_name: String,
    pub l_num: i32,
    pub display_info: u32,
    /// Extra dimension properties: (element_tag, value).  Do NOT include
    /// the standard five (UName / Caption / LName / LNum / DisplayInfo) here.
    pub dim_props: Vec<(String, String)>,
}

/// One cell in CellData.
pub struct CellConfig {
    pub ordinal: u32,
    pub value: f64,
    pub fmt_value: String,
    pub format_string: String,
    pub back_color: String,
    pub fore_color: String,
}

/// An axis description — name, hierarchy identity, and the member list.
pub struct AxisConfig {
    pub name: String,               // "Axis0" | "SlicerAxis"
    /// Full hierarchy unique name, e.g. "[Produktkategori].[Produktkategori]"
    pub hier_name: String,
    /// Members of this axis.
    pub members: Vec<MemberConfig>,
    /// Extra dimension-property *declarations* for the HierarchyInfo.
    /// (tag, qualified_name, type).  The standard five are added automatically.
    pub dim_prop_decls: Vec<(String, String, String)>,
}

/// Everything needed to produce the mddataset Execute response.
pub struct CellsetResponse {
    pub cube_name: String,
    pub axes: Vec<AxisConfig>,
    pub cells: Vec<CellConfig>,
}

// ---------------------------------------------------------------------------
// XML generation
// ---------------------------------------------------------------------------

fn hier_qualified(prefix: &str, suffix: &str) -> String {
    format!("{}.{}", prefix, suffix)
}

/// Produce the <HierarchyInfo> block for one axis.
fn render_hierarchy_info(axis: &AxisConfig) -> String {
    let mut out = String::new();
    let p = &axis.hier_name; // qualified-name prefix

    let standard: &[(&str, &str, &str)] = &[
        ("UName",      &hier_qualified(p, "[MEMBER_UNIQUE_NAME]"),  "xsd:string"),
        ("Caption",    &hier_qualified(p, "[MEMBER_CAPTION]"),      "xsd:string"),
        ("LName",      &hier_qualified(p, "[LEVEL_UNIQUE_NAME]"),   "xsd:string"),
        ("LNum",       &hier_qualified(p, "[LEVEL_NUMBER]"),        "xsd:int"),
        ("DisplayInfo",&hier_qualified(p, "[DISPLAY_INFO]"),        "xsd:unsignedInt"),
    ];
    for (tag, qname, typ) in standard {
        out.push_str(&format!(
            r#"                  <{tag} name="{qname}" type="{typ}"/>
"#,
        ));
    }
    for (tag, qname, typ) in &axis.dim_prop_decls {
        out.push_str(&format!(
            r#"                  <{tag} name="{qname}" type="{typ}"/>
"#,
        ));
    }
    out
}

/// Produce the <Member> XML for one member.
fn render_member(m: &MemberConfig, hier: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        r#"                  <Member Hierarchy="{hier}">
                    <UName>{u}</UName>
                    <Caption>{c}</Caption>
                    <LName>{l}</LName>
                    <LNum>{ln}</LNum>
                    <DisplayInfo>{di}</DisplayInfo>
"#,
        hier = hier,
        u = m.u_name,
        c = m.caption,
        l = m.l_name,
        ln = m.l_num,
        di = m.display_info,
    ));
    for (tag, val) in &m.dim_props {
        out.push_str(&format!(
            r#"                    <{tag}>{val}</{tag}>
"#,
        ));
    }
    out.push_str("                  </Member>\n");
    out
}

/// Produce the <Axes> block.
fn render_axes(axes: &[AxisConfig]) -> String {
    let mut out = String::new();
    out.push_str("          <Axes>\n");
    for axis in axes {
        out.push_str(&format!("            <Axis name=\"{}\">\n", axis.name));
        out.push_str("              <Tuples>\n");
        for m in &axis.members {
            out.push_str("                <Tuple>\n");
            out.push_str(&render_member(m, &axis.hier_name));
            out.push_str("                </Tuple>\n");
        }
        out.push_str("              </Tuples>\n");
        out.push_str("            </Axis>\n");
    }
    out.push_str("          </Axes>\n");
    out
}

/// Produce the <CellData> block.
fn render_cells(cells: &[CellConfig]) -> String {
    let mut out = String::new();
    out.push_str("          <CellData>\n");
    for cell in cells {
        out.push_str(&format!(
            r#"            <Cell CellOrdinal="{ord}">
              <Value xsi:type="xsd:double">{val}</Value>
              <FmtValue>{fmt}</FmtValue>
              <FormatString>{fs}</FormatString>
              <BackColor>{bc}</BackColor>
              <ForeColor>{fc}</ForeColor>
            </Cell>
"#,
            ord = cell.ordinal,
            val = cell.value,
            fmt = cell.fmt_value,
            fs = cell.format_string,
            bc = cell.back_color,
            fc = cell.fore_color,
        ));
    }
    out.push_str("          </CellData>\n");
    out
}

/// Produce the complete mddataset inner XML (inside <ExecuteResponse>).
pub fn render_cellset(r: &CellsetResponse) -> String {
    // --- constant schema ---
    let schema = r#"          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:mddataset"
                       elementFormDefault="qualified"
                       xmlns="urn:schemas-microsoft-com:xml-analysis:mddataset">
            <xsd:element name="root">
              <xsd:complexType>
                <xsd:sequence>
                  <xsd:any namespace="http://www.w3.org/2001/XMLSchema"
                           processContents="strict" minOccurs="0"/>
                  <xsd:element name="OlapInfo" minOccurs="0"/>
                  <xsd:element name="Axes" minOccurs="0"/>
                  <xsd:element name="CellData" minOccurs="0"/>
                </xsd:sequence>
              </xsd:complexType>
            </xsd:element>
          </xsd:schema>
"#;

    // --- OlapInfo ---
    let mut olap_info = String::new();
    olap_info.push_str("          <OlapInfo>\n");
    // CubeInfo
    olap_info.push_str(&format!(
        r#"            <CubeInfo>
              <Cube>
                <CubeName>{cube}</CubeName>
              </Cube>
            </CubeInfo>
"#,
        cube = r.cube_name,
    ));
    // AxesInfo
    olap_info.push_str("            <AxesInfo>\n");
    for axis in &r.axes {
        olap_info.push_str(&format!(
            "              <AxisInfo name=\"{name}\">\n                <HierarchyInfo name=\"{hier}\">\n",
            name = axis.name,
            hier = axis.hier_name,
        ));
        olap_info.push_str(&render_hierarchy_info(axis));
        olap_info.push_str("                </HierarchyInfo>\n              </AxisInfo>\n");
    }
    olap_info.push_str("            </AxesInfo>\n");
    // CellInfo
    olap_info.push_str(
        r#"            <CellInfo>
              <Value name="VALUE"/>
              <FmtValue name="FORMATTED_VALUE" type="xsd:string"/>
              <FormatString name="FORMAT_STRING" type="xsd:string"/>
              <BackColor name="BACK_COLOR" type="xsd:string"/>
              <ForeColor name="FORE_COLOR" type="xsd:string"/>
            </CellInfo>
          </OlapInfo>
"#,
    );

    // --- Assemble ---
    let axes_xml = render_axes(&r.axes);
    let cells_xml = render_cells(&r.cells);

    let inner = format!(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:mddataset"
              xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
              xmlns:xsd="http://www.w3.org/2001/XMLSchema">
{schema}{olap_info}{axes_xml}{cells_xml}        </root>
      </return>
    </ExecuteResponse>"#,
    );

    wrap_in_soap_envelope(&inner)
}
