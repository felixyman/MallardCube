thread_local! {
    static CURRENT_SESSION_ID: std::cell::RefCell<Option<String>> =
        const { std::cell::RefCell::new(None) };
}

/// Set the session id to carry in this response's SOAP Header: the reference
/// returns one only for a `BeginSession` request (a plain session-bound
/// response has no header, measured 2026-09-27).
pub fn set_session_id(sid: Option<String>) {
    CURRENT_SESSION_ID.with(|current| *current.borrow_mut() = sid);
}

/// The SOAP envelope split into its opening and closing halves, so a large
/// inner payload can be written incrementally (plan 051-C).
///
/// The Session header appears only when [`set_session_id`] set one — the
/// reference issues no id for a sessionless request (measured 2026-09-27).
pub fn soap_envelope_parts() -> (String, String) {
    let header = CURRENT_SESSION_ID
        .with(|current| current.borrow().clone())
        .map(|session_id| {
            format!(
                "  <soap:Header>\n    <Session xmlns=\"urn:schemas-microsoft-com:xml-analysis\" SessionId=\"{}\" />\n  </soap:Header>\n",
                xml_escape_attr(&session_id)
            )
        })
        .unwrap_or_default();
    let open = format!(
        "<soap:Envelope xmlns:soap=\"http://schemas.xmlsoap.org/soap/envelope/\">\n{header}  <soap:Body>\n"
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

/// Escape text for a double-quoted XML attribute: `xml_escape` plus quotes.
pub fn xml_escape_attr(s: &str) -> String {
    xml_escape(s).replace('"', "&quot;").replace('\'', "&apos;")
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
/// The reference's rowset-schema preamble: the root element, then the `uuid`
/// and `xmlDocument` helper types every reference rowset schema carries — as
/// *siblings* of the row type, before it (measured 2026-09-28).
///
/// Nesting them inside the row type's sequence is invalid XSD; MSOLAP then
/// rejects the whole response ("xsd:restriction … cannot appear under
/// …/complexType/sequence/(any)", measured 2026-09-30 while validating the
/// TPC-H model from Excel/ADODB).
pub(crate) const ROWSET_SCHEMA_PREAMBLE: &str = r#"              <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
                <xsd:element name="root">
                  <xsd:complexType><xsd:sequence minOccurs="0" maxOccurs="unbounded"><xsd:element name="row" type="row"/></xsd:sequence></xsd:complexType>
                </xsd:element>
                <xsd:simpleType name="uuid"><xsd:restriction base="xsd:string"><xsd:pattern value="[0-9a-zA-Z]{8}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{4}-[0-9a-zA-Z]{12}"/></xsd:restriction></xsd:simpleType>
                <xsd:complexType name="xmlDocument"><xsd:sequence><xsd:any/></xsd:sequence></xsd:complexType>
"#;

/// The row complexType opening, written after [`ROWSET_SCHEMA_PREAMBLE`] and
/// before the column elements.
pub(crate) const ROWSET_SCHEMA_ROW_OPEN: &str = r#"                <xsd:complexType name="row">
                  <xsd:sequence>
"#;

/// The row complexType and schema closing.
pub(crate) const ROWSET_SCHEMA_CLOSE: &str = r#"                  </xsd:sequence>
                </xsd:complexType>
              </xsd:schema>
"#;

/// Encode a column name as an XML element name the way the reference does:
/// `[`/`]` and every other non-name character become `_xHHHH_`.
pub(crate) fn encoded_element_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for character in name.chars() {
        match character {
            c if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '?' | '_') => {
                out.push(c)
            }
            c => out.push_str(&format!("_x{:04X}_", c as u32)),
        }
    }
    out
}

pub fn discover_rowset_parts(extra_schema: &str, row_fields: &str) -> (String, String) {
    // Every reference rowset schema carries the `uuid` and `xmlDocument`
    // helper types (measured 2026-09-28). A caller-supplied `uuid` — the
    // `UUID_TYPE` constant some rowsets pass — is kept rather than duplicated.
    let uuid = if extra_schema.contains("name=\"uuid\"") {
        String::new()
    } else {
        format!("{UUID_TYPE}\n")
    };
    let open = format!(
        r#"    <DiscoverResponse xmlns="urn:schemas-microsoft-com:xml-analysis">
      <return>
        <root xmlns="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:rowset" xmlns:sql="urn:schemas-microsoft-com:xml-sql" elementFormDefault="qualified">
            <xsd:element name="root">
              <xsd:complexType><xsd:sequence minOccurs="0" maxOccurs="unbounded"><xsd:element name="row" type="row"/></xsd:sequence></xsd:complexType>
            </xsd:element>
{uuid}{extra_schema}
            <xsd:complexType name="xmlDocument"><xsd:sequence><xsd:any/></xsd:sequence></xsd:complexType>
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
    /// An attribute value must survive quotes too — the session id is echoed
    /// from client input.
    /// Every rowset schema carries the reference's `uuid` and `xmlDocument`
    /// helper types; a caller-supplied `uuid` is not duplicated (measured
    /// 2026-09-28).
    #[test]
    fn rowset_schema_carries_the_helper_types() {
        let (open, _) = discover_rowset_parts("", "<xsd:element name=\"A\"/>");
        assert_eq!(open.matches("name=\"uuid\"").count(), 1, "{open}");
        assert_eq!(open.matches("name=\"xmlDocument\"").count(), 1, "{open}");
        let (open, _) = discover_rowset_parts(UUID_TYPE, "<xsd:element name=\"A\"/>");
        assert_eq!(
            open.matches("name=\"uuid\"").count(),
            1,
            "no duplicate uuid"
        );
        assert_eq!(open.matches("name=\"xmlDocument\"").count(), 1, "{open}");
    }

    /// The reference returns a Session header only for a `BeginSession`
    /// request; a sessionless response has no SOAP Header at all (measured
    /// 2026-09-27).
    #[test]
    fn envelope_carries_the_session_header_only_when_set() {
        set_session_id(None);
        let (open, close) = soap_envelope_parts();
        assert!(!open.contains("<Session"), "{open}");
        assert!(!open.contains("soap:Header"), "{open}");
        assert!(open.contains("<soap:Body>"), "{open}");
        assert!(close.contains("</soap:Envelope>"), "{close}");

        set_session_id(Some("abc-123".to_string()));
        let (open, _) = soap_envelope_parts();
        assert!(open.contains("SessionId=\"abc-123\""), "{open}");
        set_session_id(None);
    }

    #[test]
    fn xml_escape_attr_escapes_quotes() {
        assert_eq!(
            xml_escape_attr("a\"b'c&d<e>"),
            "a&quot;b&apos;c&amp;d&lt;e&gt;"
        );
    }

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
