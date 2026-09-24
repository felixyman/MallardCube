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
///
/// Handles `&`, `<`, `>`; XML 1.0-forbidden control characters (which can only
/// come from engine data, since the parser rejects them in requests) become
/// U+FFFD rather than making the whole response unparsable — a visible marker
/// beats a document no client can read.
pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\u{0}'..='\u{8}'
            | '\u{b}'
            | '\u{c}'
            | '\u{e}'..='\u{1f}'
            | '\u{fffe}'
            | '\u{ffff}' => {
                out.push('\u{fffd}');
            }
            // A literal CR would be normalised to LF by every XML parser, so a
            // value carrying one would silently change (plan 055 review).
            '\r' => out.push_str("&#xD;"),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// XML parsers normalise a literal CR to LF, so a value carrying one must
    /// be escaped; tab, LF, emoji and U+FFFD survive unchanged, and
    /// XML-invalid controls become U+FFFD (plan 055 review).
    #[test]
    fn xml_escape_round_trips_control_characters() {
        let escaped = xml_escape("a\r\tb\n\u{1}\u{fffd}\u{1f389}<&>");
        assert!(escaped.contains("&#xD;"), "{escaped}");
        assert!(escaped.contains('\t'), "{escaped}");
        assert!(escaped.contains('\n'), "{escaped}");
        assert!(escaped.contains('\u{fffd}'), "{escaped}");
        assert!(escaped.contains('\u{1f389}'), "{escaped}");
        assert!(escaped.contains("&lt;&amp;&gt;"), "{escaped}");
        assert!(!escaped.contains('\u{1}'), "{escaped}");
    }
}
