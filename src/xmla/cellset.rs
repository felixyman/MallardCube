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
    pub hierarchy: String,
    pub u_name: String,
    pub caption: String,
    pub l_name: String,
    pub l_num: i32,
    pub display_info: u32,
    pub children_cardinality: u32,
    /// Extra dimension properties: (element_tag, value).  Do NOT include
    /// the standard five (UName / Caption / LName / LNum / DisplayInfo) here.
    pub dim_props: Vec<(String, String)>,
}

pub struct TupleConfig {
    pub members: Vec<MemberConfig>,
}

pub struct HierarchyConfig {
    pub name: String,
    pub dim_prop_decls: Vec<(String, String, String)>,
}

/// One cell in CellData.
pub struct CellConfig {
    pub ordinal: u32,
    pub value: f64,
    pub fmt_value: String,
    pub format_string: String,
    pub back_color: String,
    pub fore_color: String,
    /// When set, emits `<Value xsi:type="xsd:string">` instead of numeric Value.
    pub string_value: Option<String>,
}

/// An axis description — name, hierarchy identity, and the member list.
pub struct AxisConfig {
    pub name: String, // "Axis0" | "SlicerAxis"
    pub hierarchies: Vec<HierarchyConfig>,
    pub tuples: Vec<TupleConfig>,
}

/// Everything needed to produce the mddataset Execute response.
pub struct CellsetResponse {
    pub cube_name: String,
    pub axes: Vec<AxisConfig>,
    pub cells: Vec<CellConfig>,
    pub include_value: bool,
    pub include_fmt_value: bool,
    pub include_format_string: bool,
    pub include_back_color: bool,
    pub include_fore_color: bool,
}

// ---------------------------------------------------------------------------
// XML generation
// ---------------------------------------------------------------------------

fn hier_qualified(prefix: &str, suffix: &str) -> String {
    format!("{}.{}", prefix, suffix)
}

fn render_hierarchy_info(hier: &HierarchyConfig) -> String {
    let mut out = String::new();
    let p = &hier.name; // qualified-name prefix

    let standard: &[(&str, &str, &str)] = &[
        (
            "UName",
            &hier_qualified(p, "[MEMBER_UNIQUE_NAME]"),
            "xsd:string",
        ),
        (
            "Caption",
            &hier_qualified(p, "[MEMBER_CAPTION]"),
            "xsd:string",
        ),
        (
            "LName",
            &hier_qualified(p, "[LEVEL_UNIQUE_NAME]"),
            "xsd:string",
        ),
        ("LNum", &hier_qualified(p, "[LEVEL_NUMBER]"), "xsd:int"),
        (
            "DisplayInfo",
            &hier_qualified(p, "[DISPLAY_INFO]"),
            "xsd:unsignedInt",
        ),
        (
            "CHILDREN_CARDINALITY",
            &hier_qualified(p, "[CHILDREN_CARDINALITY]"),
            "xsd:unsignedInt",
        ),
    ];
    for (tag, qname, typ) in standard {
        out.push_str(&format!(
            r#"                  <{tag} name="{qname}" type="{typ}"/>
"#,
        ));
    }
    for (tag, qname, typ) in &hier.dim_prop_decls {
        out.push_str(&format!(
            r#"                  <{tag} name="{qname}" type="{typ}"/>
"#,
        ));
    }
    out
}

/// Produce the <Member> XML for one member.
fn render_member(m: &MemberConfig) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        r#"                  <Member Hierarchy="{hier}">
                    <UName>{u}</UName>
                    <Caption>{c}</Caption>
                    <LName>{l}</LName>
                    <LNum>{ln}</LNum>
                    <DisplayInfo>{di}</DisplayInfo>
                    <CHILDREN_CARDINALITY>{cc}</CHILDREN_CARDINALITY>
"#,
        hier = m.hierarchy,
        u = m.u_name,
        c = m.caption,
        l = m.l_name,
        ln = m.l_num,
        di = m.display_info,
        cc = m.children_cardinality,
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
        for tuple in &axis.tuples {
            out.push_str("                <Tuple>\n");
            for member in &tuple.members {
                out.push_str(&render_member(member));
            }
            out.push_str("                </Tuple>\n");
        }
        out.push_str("              </Tuples>\n");
        out.push_str("            </Axis>\n");
    }
    out.push_str("          </Axes>\n");
    out
}

/// Produce the <CellData> block.
fn render_cells(cells: &[CellConfig], resp: &CellsetResponse) -> String {
    let mut out = String::new();
    out.push_str("          <CellData>\n");
    for cell in cells {
        out.push_str(&format!(
            r#"            <Cell CellOrdinal="{ord}">
"#,
            ord = cell.ordinal
        ));
        if resp.include_value {
            if let Some(ref sv) = cell.string_value {
                out.push_str(&format!(
                    "              <Value xsi:type=\"xsd:string\">{}</Value>\n",
                    sv
                ));
            } else {
                out.push_str(&format!(
                    r#"              <Value xsi:type="xsd:double">{val}</Value>
"#,
                    val = cell.value,
                ));
            }
        }
        if resp.include_fmt_value {
            out.push_str(&format!(
                "              <FmtValue>{}</FmtValue>\n",
                cell.fmt_value
            ));
        }
        if resp.include_format_string {
            out.push_str(&format!(
                "              <FormatString>{}</FormatString>\n",
                cell.format_string
            ));
        }
        if resp.include_back_color {
            out.push_str(&format!(
                "              <BackColor>{}</BackColor>\n",
                cell.back_color
            ));
        }
        if resp.include_fore_color {
            out.push_str(&format!(
                "              <ForeColor>{}</ForeColor>\n",
                cell.fore_color
            ));
        }
        out.push_str("            </Cell>\n");
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
        if axis.hierarchies.is_empty() {
            olap_info.push_str(&format!(
                "              <AxisInfo name=\"{name}\">\n              </AxisInfo>\n",
                name = axis.name,
            ));
        } else {
            olap_info.push_str(&format!(
                "              <AxisInfo name=\"{}\">\n",
                axis.name
            ));
            for hier in &axis.hierarchies {
                olap_info.push_str(&format!(
                    "                <HierarchyInfo name=\"{}\">\n",
                    hier.name,
                ));
                olap_info.push_str(&render_hierarchy_info(hier));
                olap_info.push_str("                </HierarchyInfo>\n");
            }
            olap_info.push_str("              </AxisInfo>\n");
        }
    }
    olap_info.push_str("            </AxesInfo>\n");
    // CellInfo
    olap_info.push_str("            <CellInfo>\n");
    if r.include_value {
        olap_info.push_str("              <Value name=\"VALUE\"/>\n");
    }
    if r.include_fmt_value {
        olap_info
            .push_str("              <FmtValue name=\"FORMATTED_VALUE\" type=\"xsd:string\"/>\n");
    }
    if r.include_format_string {
        olap_info
            .push_str("              <FormatString name=\"FORMAT_STRING\" type=\"xsd:string\"/>\n");
    }
    if r.include_back_color {
        olap_info.push_str("              <BackColor name=\"BACK_COLOR\" type=\"xsd:string\"/>\n");
    }
    if r.include_fore_color {
        olap_info.push_str("              <ForeColor name=\"FORE_COLOR\" type=\"xsd:string\"/>\n");
    }
    olap_info.push_str("            </CellInfo>\n          </OlapInfo>\n");

    // --- Assemble ---
    let axes_xml = render_axes(&r.axes);
    let cells_xml = render_cells(&r.cells, r);

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
