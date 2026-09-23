use std::cell::RefCell;

thread_local! {
    static CURRENT_SESSION_ID: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Set the session id to echo back in the SOAP response header.
pub fn set_session_id(sid: Option<String>) {
    CURRENT_SESSION_ID.with(|c| *c.borrow_mut() = sid);
}

/// The SOAP envelope split into its opening and closing halves, so a large
/// inner payload can be written incrementally (plan 051-C).
pub fn soap_envelope_parts() -> (String, String) {
    let session_id = CURRENT_SESSION_ID.with(|c| {
        c.borrow()
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string().to_uppercase())
    });
    let open = format!(
        r#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Header>
    <Session xmlns="urn:schemas-microsoft-com:xml-analysis" SessionId="{session_id}" />
  </soap:Header>
  <soap:Body>
"#
    );
    let close = r#"  </soap:Body>
</soap:Envelope>"#
        .to_string();
    (open, close)
}

pub fn wrap_in_soap_envelope(inner_xml: &str) -> String {
    let (open, close) = soap_envelope_parts();
    format!("{open}{inner_xml}\n{close}")
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

/// SOAP fault envelope for requests the proxy cannot honour. Used for
/// unsupported MDX constructs (plan 046) and internal errors: a clear fault
/// beats a silently dropped axis or a wrong-hierarchy cellset.
pub fn fault_response(message: &str) -> String {
    format!(
        "<soap:Envelope xmlns:soap=\"http://schemas.xmlsoap.org/soap/envelope/\">\
         <soap:Body><soap:Fault><faultcode>XMLAnalysisError</faultcode>\
         <faultstring>{}</faultstring></soap:Fault></soap:Body>\
         </soap:Envelope>",
        xml_escape(message)
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

/// The rowset envelope split into `<rows>`-less halves: everything up to where
/// the row elements start, and the closing tags. Callers that write rows
/// incrementally (plan 051-C) avoid building a second full copy of the payload.
pub fn discover_rowset_parts(extra_schema: &str, row_fields: &str) -> (String, String) {
    let open = format!(
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
"#
    );
    let close = r#"        </root>
      </return>
    </DiscoverResponse>"#
        .to_string();
    (open, close)
}

pub fn discover_rowset_envelope(extra_schema: &str, row_fields: &str, rows: &str) -> String {
    let (open, close) = discover_rowset_parts(extra_schema, row_fields);
    let inner = format!("{open}{rows}\n{close}");
    wrap_in_soap_envelope(&inner)
}
