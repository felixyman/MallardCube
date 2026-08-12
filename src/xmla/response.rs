use std::cell::RefCell;

thread_local! {
    static CURRENT_SESSION_ID: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Set the session id to echo back in the SOAP response header.
pub fn set_session_id(sid: Option<String>) {
    CURRENT_SESSION_ID.with(|c| *c.borrow_mut() = sid);
}

pub fn wrap_in_soap_envelope(inner_xml: &str) -> String {
    let session_id = CURRENT_SESSION_ID.with(|c| {
        c.borrow()
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string().to_uppercase())
    });
    format!(
        r#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Header>
    <Session xmlns="urn:schemas-microsoft-com:xml-analysis" SessionId="{session_id}" />
  </soap:Header>
  <soap:Body>
{}
  </soap:Body>
</soap:Envelope>"#,
        inner_xml
    )
}

/// Escape text content for safe XML insertion.
/// Handles `&`, `<`, `>`.
pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
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
