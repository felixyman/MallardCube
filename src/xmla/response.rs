pub fn wrap_in_soap_envelope(inner_xml: &str) -> String {
    format!(
        r#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Header>
    <Session xmlns="urn:schemas-microsoft-com:xml-analysis" SessionId="RUST-SESSION-456" />
  </soap:Header>
  <soap:Body>
{}
  </soap:Body>
</soap:Envelope>"#,
        inner_xml
    )
}

pub const UUID_TYPE: &str = r#"<xsd:simpleType name="uuid">
              <xsd:restriction base="xsd:string">
                <xsd:pattern value="[0-9a-zA-Z]{8}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{12}"/>
              </xsd:restriction>
            </xsd:simpleType>"#;

pub fn empty_discover_response() -> String {
    let inner = r#"    <DiscoverResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" />
        </root>
      </return>
    </DiscoverResponse>"#;
    wrap_in_soap_envelope(inner)
}

pub fn discover_rowset_envelope(extra_schema: &str, row_fields: &str, rows: &str) -> String {
    let inner = format!(
        r#"    <DiscoverResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence minOccurs="0" maxOccurs="unbounded"><xsd:element name="row" type="row"/></xsd:sequence></xsd:complexType>
            </xsd:element>
{extra_schema}
            <xsd:complexType name="row">
              <xsd:sequence>
{row_fields}
              </xsd:sequence>
            </xsd:complexType>
          </xsd:schema>
{rows}
        </root>
      </return>
    </DiscoverResponse>"#,
    );
    wrap_in_soap_envelope(&inner)
}
