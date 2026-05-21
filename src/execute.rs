use crate::response::wrap_in_soap_envelope;

pub fn get_empty_execute_response() -> String {
    wrap_in_soap_envelope(
        r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:empty"/>
      </return>
    </ExecuteResponse>"#,
    )
}

/// Returns true when the statement looks like a DAX query (starts with EVALUATE,
/// optionally after DEFINE blocks/whitespace).
fn is_dax(statement: &str) -> bool {
    let trimmed = statement.trim_start();
    let upper = trimmed.to_uppercase();
    upper.starts_with("EVALUATE") || upper.starts_with("DEFINE")
}

/// Returns true when the MDX is a multidimensional cellset query
/// (has DIMENSION PROPERTIES or CELL PROPERTIES clauses).
fn is_cellset_query(mdx: &str) -> bool {
    mdx.contains("DIMENSION PROPERTIES") || mdx.contains("CELL PROPERTIES")
}

pub fn get_execute_statement_response(statement: &str) -> String {
    if is_dax(statement) {
        get_execute_dax_response(statement)
    } else if is_cellset_query(statement) {
        get_execute_cellset_response(statement)
    } else {
        get_execute_mdx_response(statement)
    }
}

fn get_execute_mdx_response(mdx: &str) -> String {
    let has_measures = mdx.contains("Measures") || mdx.contains("measures");
    let measure_name = "Total_Försäljning";
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

/// Returns a multidimensional cellset XML response for hierarchy enumeration
/// queries (DrilldownLevel from All). Currently hard-coded for the
/// Produktkategori hierarchy with one member (Kategori A).
fn get_execute_cellset_response(mdx: &str) -> String {
    let is_drilldown = mdx.contains("DrilldownLevel") && mdx.contains("[Produktkategori]");

    if is_drilldown {
        let inner = r#"    <ExecuteResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:mddataset"
              xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
              xmlns:xsd="http://www.w3.org/2001/XMLSchema">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:mddataset"
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
          <OlapInfo>
            <CubeInfo>
              <Cube>
                <CubeName>Model</CubeName>
              </Cube>
            </CubeInfo>
            <AxesInfo>
              <AxisInfo name="Axis0">
                <HierarchyInfo name="[Produktkategori].[Produktkategori]">
                  <UName name="[Produktkategori].[Produktkategori].[MEMBER_UNIQUE_NAME]" type="xsd:string"/>
                  <Caption name="[Produktkategori].[Produktkategori].[MEMBER_CAPTION]" type="xsd:string"/>
                  <LName name="[Produktkategori].[Produktkategori].[LEVEL_UNIQUE_NAME]" type="xsd:string"/>
                  <LNum name="[Produktkategori].[Produktkategori].[LEVEL_NUMBER]" type="xsd:int"/>
                  <DisplayInfo name="[Produktkategori].[Produktkategori].[DISPLAY_INFO]" type="xsd:unsignedInt"/>
                  <PARENT_UNIQUE_NAME name="[Produktkategori].[Produktkategori].[PARENT_UNIQUE_NAME]" type="xsd:string"/>
                  <HIERARCHY_UNIQUE_NAME name="[Produktkategori].[Produktkategori].[HIERARCHY_UNIQUE_NAME]" type="xsd:string"/>
                  <MEMBER_NAME name="[Produktkategori].[Produktkategori].[MEMBER_NAME]" type="xsd:string"/>
                  <MEMBER_KEY name="[Produktkategori].[Produktkategori].[MEMBER_KEY]" type="xsd:string"/>
                  <MEMBER_TYPE name="[Produktkategori].[Produktkategori].[MEMBER_TYPE]" type="xsd:int"/>
                  <MEMBER_VALUE name="[Produktkategori].[Produktkategori].[MEMBER_VALUE]" type="xsd:string"/>
                  <LEVEL_UNIQUE_NAME name="[Produktkategori].[Produktkategori].[LEVEL_UNIQUE_NAME]" type="xsd:string"/>
                  <PARENT_LEVEL name="[Produktkategori].[Produktkategori].[PARENT_LEVEL]" type="xsd:int"/>
                  <PARENT_COUNT name="[Produktkategori].[Produktkategori].[PARENT_COUNT]" type="xsd:int"/>
                  <CHILDREN_CARDINALITY name="[Produktkategori].[Produktkategori].[CHILDREN_CARDINALITY]" type="xsd:unsignedInt"/>
                </HierarchyInfo>
              </AxisInfo>
              <AxisInfo name="SlicerAxis">
                <HierarchyInfo name="[Measures]">
                  <UName name="[Measures].[MEMBER_UNIQUE_NAME]" type="xsd:string"/>
                  <Caption name="[Measures].[MEMBER_CAPTION]" type="xsd:string"/>
                  <LName name="[Measures].[LEVEL_UNIQUE_NAME]" type="xsd:string"/>
                  <LNum name="[Measures].[LEVEL_NUMBER]" type="xsd:int"/>
                  <DisplayInfo name="[Measures].[DISPLAY_INFO]" type="xsd:unsignedInt"/>
                </HierarchyInfo>
              </AxisInfo>
            </AxesInfo>
            <CellInfo>
              <Value name="VALUE"/>
              <FmtValue name="FORMATTED_VALUE" type="xsd:string"/>
              <FormatString name="FORMAT_STRING" type="xsd:string"/>
              <BackColor name="BACK_COLOR" type="xsd:string"/>
              <ForeColor name="FORE_COLOR" type="xsd:string"/>
            </CellInfo>
          </OlapInfo>
          <Axes>
            <Axis name="Axis0">
              <Tuples>
                <Tuple>
                  <Member Hierarchy="[Produktkategori].[Produktkategori]">
                    <UName>[Produktkategori].[Produktkategori].&amp;[Kategori A]</UName>
                    <Caption>Kategori A</Caption>
                    <LName>[Produktkategori].[Produktkategori].[Produktkategori]</LName>
                    <LNum>1</LNum>
                    <DisplayInfo>3</DisplayInfo>
                    <PARENT_UNIQUE_NAME>[Produktkategori].[Produktkategori].[All]</PARENT_UNIQUE_NAME>
                    <HIERARCHY_UNIQUE_NAME>[Produktkategori].[Produktkategori]</HIERARCHY_UNIQUE_NAME>
                    <MEMBER_NAME>Kategori A</MEMBER_NAME>
                    <MEMBER_KEY>Kategori A</MEMBER_KEY>
                    <MEMBER_TYPE>3</MEMBER_TYPE>
                    <MEMBER_VALUE>Kategori A</MEMBER_VALUE>
                    <PARENT_LEVEL>0</PARENT_LEVEL>
                    <PARENT_COUNT>1</PARENT_COUNT>
                    <CHILDREN_CARDINALITY>0</CHILDREN_CARDINALITY>
                  </Member>
                </Tuple>
              </Tuples>
            </Axis>
            <Axis name="SlicerAxis">
              <Tuples>
                <Tuple>
                  <Member Hierarchy="[Measures]">
                    <UName>[Measures].[Total Försäljning]</UName>
                    <Caption>Total Försäljning (SEK)</Caption>
                    <LName>[Measures].[MeasuresLevel]</LName>
                    <LNum>0</LNum>
                    <DisplayInfo>3</DisplayInfo>
                  </Member>
                </Tuple>
              </Tuples>
            </Axis>
          </Axes>
          <CellData>
            <Cell CellOrdinal="0">
              <Value xsi:type="xsd:double">1250000.5</Value>
              <FmtValue>1,250,000.50 SEK</FmtValue>
              <FormatString>#,##0.00 SEK</FormatString>
              <BackColor></BackColor>
              <ForeColor></ForeColor>
            </Cell>
          </CellData>
        </root>
      </return>
    </ExecuteResponse>"#;
        wrap_in_soap_envelope(inner)
    } else {
        // Unknown cellset query shape — fall back to the MDX path
        // This will likely fail, but it's better than crashing.
        get_execute_mdx_response(mdx)
    }
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
