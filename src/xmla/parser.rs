use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use quick_xml::reader::NsReader;

/// The XMLA namespace every protocol element lives in. The reference accepts
/// prefixed elements bound to it and rejects foreign namespaces or undeclared
/// prefixes (measured 2026-09-24) — the prefix is irrelevant, the resolved
/// namespace is not.
const XMLA_NAMESPACE: &[u8] = b"urn:schemas-microsoft-com:xml-analysis";

/// The reference accepts only SOAP 1.1 (measured 2026-09-24: SOAP 1.2, a
/// foreign envelope and an XMLA-default envelope all fault).
const SOAP_NAMESPACE: &[u8] = b"http://schemas.xmlsoap.org/soap/envelope/";

/// Values from a Discover request's `RestrictionList` (MS-SSAS). Discover
/// responses must honour these: Excel asks for one hierarchy's member
/// properties at a time while it builds pivot cache fields, and returning rows
/// for other hierarchies corrupts the cache field (plan 048).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Restrictions {
    pub catalog_name: Option<String>,
    pub cube_name: Option<String>,
    pub dimension_unique_name: Option<String>,
    pub hierarchy_unique_name: Option<String>,
    pub level_unique_name: Option<String>,
    pub property_name: Option<String>,
    /// `MDSCHEMA_FUNCTIONS` restriction: 1 = built-in MDX functions,
    /// 2 = user-defined (plan 049).
    pub origin: Option<i32>,
    /// `MDSCHEMA_MEMBERS` restriction: member type (1 = regular, 2 = `(All)`,
    /// 4 = measure). Dropping it widened the rowset — the mirror excludes
    /// `(All)` for `MEMBER_TYPE=1` (plan 051 round 3).
    pub member_type: Option<i32>,
    /// Every restriction name seen in this request, in order. Validated
    /// against the rowset's advertised contract (`DISCOVER_SCHEMA_ROWSETS`)
    /// before dispatch: an unadvertised name faults like the reference instead
    /// of being ignored (plan 055).
    pub seen: Vec<String>,
    /// `DISCOVER_SCHEMA_ROWSETS` restriction (`SchemaName`, no underscore).
    /// Excel asks for one rowset's entry to learn its restrictions; answering
    /// with the whole list makes it miss `HIERARCHY_VISIBILITY` and skip the
    /// visibility-filtered queries that lead to the key attribute's
    /// `MEMBER_VALUE` type (plan 048).
    pub schema_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum XmlaRequest {
    DiscoverProperties {
        property_names: Vec<String>,
    },
    DiscoverSchemaRowsets {
        /// `SchemaName` restriction: return only this rowset's entry.
        schema_name: Option<String>,
    },
    DiscoverLiterals,
    DbSchemaCatalogs,
    MdschemaCubes,
    DbschemaTables,
    MdschemaDimensions,
    MdschemaMeasures,
    MdschemaHierarchies {
        restrictions: Restrictions,
    },
    MdschemaLevels {
        restrictions: Restrictions,
    },
    MdschemaFunctions {
        restrictions: Restrictions,
    },
    MdschemaProperties {
        property_type: Option<i32>,
        restrictions: Restrictions,
    },
    MdschemaMembers {
        member_unique_name: Option<String>,
        tree_op: Option<i32>,
        restrictions: Restrictions,
    },
    MdschemaSets,
    MdschemaKpis,
    MdschemaMeasureGroups,
    MdschemaMeasureGroupDimensions,
    TmschemaModel,
    TmschemaTables,
    TmschemaColumns,
    TmschemaMeasures,
    TmschemaHierarchies,
    TmschemaLevels,
    TmschemaRelationships,
    TmschemaPartitions,
    DiscoverXmlMetadata,
    DiscoverCalcDependency,
    DiscoverEnumerators,
    DiscoverKeywords,
    DiscoverDatasources,
    BeginSession,
    ExecuteEmpty,
    /// The request XML could not be read faithfully — an unparsable entity, or
    /// text this parser dropped. Answering those with an empty cellset is a
    /// silent wrong answer: CDATA-wrapped statements (what .NET/PowerShell/Java
    /// SOAP clients emit) used to vanish here, and a restriction we could not
    /// decode used to be dropped, returning the *unrestricted* rowset (plan
    /// 051). An empty `<Statement>` is not malformed — see `ExecuteEmpty`.
    Malformed(String),
    /// A restriction name the rowset does not advertise (plan 055). The
    /// reference faults with "The restriction, X, is not recognized by the
    /// server" rather than ignoring it.
    UnsupportedRestriction(String),
    ExecuteStatement(String),
    Unknown,
}

/// Apply one restriction value by element name. Only the flat
/// `<RestrictionList>` form reaches this: the reference rejects every other
/// child of `<Restrictions>` (verified 2026-09-24). Returns whether the name
/// was recognised.
fn apply_restriction(restrictions: &mut Restrictions, name: &[u8], text: &str) -> bool {
    match name {
        b"CATALOG_NAME" => restrictions.catalog_name = Some(text.to_string()),
        b"CUBE_NAME" => restrictions.cube_name = Some(text.to_string()),
        b"DIMENSION_UNIQUE_NAME" => restrictions.dimension_unique_name = Some(text.to_string()),
        b"HIERARCHY_UNIQUE_NAME" => restrictions.hierarchy_unique_name = Some(text.to_string()),
        b"LEVEL_UNIQUE_NAME" => restrictions.level_unique_name = Some(text.to_string()),
        b"PROPERTY_NAME" => restrictions.property_name = Some(text.to_string()),
        b"ORIGIN" => restrictions.origin = text.parse().ok(),
        b"MEMBER_TYPE" => restrictions.member_type = text.parse().ok(),
        b"SchemaName" => restrictions.schema_name = Some(text.to_string()),
        _ => return false,
    }
    true
}

fn is_name_start_char(c: char) -> bool {
    matches!(c, '_' | 'A'..='Z' | 'a'..='z')
        || matches!(
            c as u32,
            0xC0..=0xD6
                | 0xD8..=0xF6
                | 0xF8..=0x2FF
                | 0x370..=0x37D
                | 0x37F..=0x1FFF
                | 0x200C..=0x200D
                | 0x2070..=0x218F
                | 0x2C00..=0x2FEF
                | 0x3001..=0xD7FF
                | 0xF900..=0xFDCF
                | 0xFDF0..=0xFFFD
                | 0x10000..=0xEFFFF
        )
}

fn is_name_char(c: char) -> bool {
    is_name_start_char(c)
        || matches!(c, '-' | '.' | '0'..='9' | '\u{b7}')
        || matches!(c as u32, 0x300..=0x36F | 0x203F..=0x2040)
}

/// XML 1.0 QName validation: quick-xml hands us raw names (references and all),
/// and the reference rejects anything outside the production — "Illegal
/// qualified name character" for `Request&amp;Type` or `<1Bogus/>`, "A
/// qualified name cannot contain multiple colons" for `<a:b:c/>` (measured
/// 2026-09-24).
fn is_qname(name: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(name) else {
        return false;
    };
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if first == ':' || !is_name_start_char(first) {
        return false;
    }
    let mut colon = false;
    let mut last = first;
    for c in chars {
        if c == ':' {
            if colon {
                return false;
            }
            colon = true;
        } else if !is_name_char(c) {
            return false;
        }
        last = c;
    }
    last != ':'
}

/// The element's own `xmlns` attribute: `Some(true)` declares a default
/// namespace, `Some(false)` explicitly un-declares one (`xmlns=""`), `None`
/// says nothing. quick-xml resolves both empty and absent to `Unbound`, but the
/// reference faults an explicit undeclaration on a semantic element (measured
/// 2026-09-24).
fn default_namespace_attribute(element: &quick_xml::events::BytesStart<'_>) -> Option<bool> {
    element.attributes().flatten().find_map(|attribute| {
        (attribute.key.as_ref() == b"xmlns").then(|| {
            attribute
                .unescape_value()
                .map(|value| !value.is_empty())
                .unwrap_or(false)
        })
    })
}

/// XML 1.0 forbids these characters anywhere in a document. The reference's
/// parser rejects such requests before the protocol layer sees them, and a
/// restriction name carrying one would make our own fault unparsable (plan 055
/// review).
fn has_xml_invalid_control(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(
            c,
            '\u{0}'..='\u{8}' | '\u{b}' | '\u{c}' | '\u{e}'..='\u{1f}' | '\u{fffe}' | '\u{ffff}'
        )
    })
}

/// Attribute names and values are part of the document's lexical surface: a
/// control character anywhere, or an attribute we cannot decode, is a malformed
/// request (the reference's parser rejects both — measured 2026-09-24).
fn attribute_error(element: &quick_xml::events::BytesStart<'_>) -> Option<String> {
    for attribute in element.attributes() {
        let Ok(attribute) = attribute else {
            return Some("request contains an unreadable attribute".to_string());
        };
        if !is_qname(attribute.key.as_ref()) {
            return Some("request contains an invalid attribute name".to_string());
        }
        match attribute.unescape_value() {
            Ok(value) => {
                if has_xml_invalid_control(&value) {
                    return Some(
                        "request text contains an XML-invalid control character".to_string(),
                    );
                }
            }
            Err(_) => return Some("request contains an unreadable attribute".to_string()),
        }
    }
    None
}

fn lexical_error(malformed: &mut Option<String>, bytes: &[u8]) {
    if has_xml_invalid_control(&String::from_utf8_lossy(bytes)) {
        malformed.get_or_insert_with(|| {
            "request text contains an XML-invalid control character".to_string()
        });
    }
}

fn parent_is_xmla_discover(open_elements: &[(Vec<u8>, bool, bool)]) -> bool {
    open_elements
        .last()
        .map(|(local, namespace_ok, _)| *namespace_ok && local.as_slice() == b"Discover")
        .unwrap_or(false)
}

fn parent_is_soap_envelope(open_elements: &[(Vec<u8>, bool, bool)]) -> bool {
    open_elements
        .last()
        .map(|(local, _, soap_ok)| *soap_ok && local.as_slice() == b"Envelope")
        .unwrap_or(false)
}

/// Structural rules the reference's schema enforces (plan 055 review): a
/// `<Restrictions>` is an unprefixed direct child of `<Discover>`, holds at
/// most one `<RestrictionList>`, and nothing else lives directly under it.
/// The parser state the structural rules need, grouped so the signature stays
/// readable.
#[derive(Clone, Copy)]
struct Structure {
    namespace_ok: bool,
    soap_ok: bool,
    parent_is_xmla_discover: bool,
    parent_is_soap_envelope: bool,
    is_root: bool,
    restrictions_seen: bool,
    in_restrictions: bool,
    in_restriction_list: bool,
    restriction_list_seen: bool,
    body_seen: bool,
    header_seen: bool,
}

fn structural_error(local: &[u8], state: Structure) -> Option<String> {
    // The SOAP skeleton is positional: `Envelope` is the root, `Body`/`Header`
    // are its direct children and SOAP-bound. A foreign element that merely
    // shares one of those names (a SOAP header entry, say) is not the skeleton
    // — the reference accepted one while faulting a body without an envelope
    // and a nested body (measured 2026-09-24).
    if local == b"Envelope" && (!state.is_root || !state.soap_ok) {
        return Some("<Envelope> must be the SOAP root element".to_string());
    }
    if matches!(local, b"Body" | b"Header") && state.soap_ok {
        // Only a SOAP-bound element can be the skeleton container: a foreign
        // element that merely shares the name is an extension (SOAP header
        // entries are explicitly allowed) and is ignored.
        if !state.parent_is_soap_envelope {
            return Some(format!(
                "<{}> must be a direct child of the SOAP envelope",
                String::from_utf8_lossy(local)
            ));
        }
        if (local == b"Body" && state.body_seen) || (local == b"Header" && state.header_seen) {
            return Some(format!(
                "<{}> appears more than once",
                String::from_utf8_lossy(local)
            ));
        }
    }
    // Every element the protocol interprets must be in the XMLA namespace: the
    // reference faults a foreign or undeclared-prefixed RequestType, Execute,
    // Command and Statement alike (measured 2026-09-24). Elements we merely
    // pass over — the engine's own `<Version>` header, for one — are not
    // checked.
    let semantic = matches!(
        local,
        b"Discover"
            | b"Execute"
            | b"Command"
            | b"Statement"
            | b"RequestType"
            | b"Restrictions"
            | b"RestrictionList"
            | b"PropertyName"
            | b"Properties"
            | b"PropertyList"
            | b"BeginSession"
            | b"BeginGetSessionToken"
            | b"PROPERTY_TYPE"
            | b"MEMBER_UNIQUE_NAME"
            | b"TREE_OP"
    );
    if !state.namespace_ok && (semantic || state.in_restrictions || state.in_restriction_list) {
        return Some(format!(
            "<{}> is not in the XMLA namespace",
            String::from_utf8_lossy(local)
        ));
    }
    match local {
        b"Restrictions" if !state.parent_is_xmla_discover || state.restrictions_seen => {
            Some("<Restrictions> must be a direct child of <Discover>".to_string())
        }
        b"RestrictionList"
            if !state.in_restrictions
                || state.in_restriction_list
                || state.restriction_list_seen =>
        {
            Some("<RestrictionList> must be the only child of <Restrictions>".to_string())
        }
        b"RestrictionList" => None,
        _ if state.in_restrictions && !state.in_restriction_list => Some(format!(
            "unexpected <{}> under <Restrictions>",
            String::from_utf8_lossy(local)
        )),
        _ => None,
    }
}

/// The session id carried by an XMLA `Session`/`EndSession` element, XML
/// unescaped. Only the XMLA namespace counts: a foreign header entry named
/// `Session` is ignored, exactly as the reference does (measured 2026-09-24).
/// The proxy is stateless and only echoes the id, so no existence check is
/// possible (the reference faults an unknown session; recorded as a
/// divergence).
pub fn session_id(xml: &str) -> Option<String> {
    let mut reader = NsReader::from_str(xml);
    loop {
        match reader.read_resolved_event() {
            Ok((namespace, Event::Start(ref e))) | Ok((namespace, Event::Empty(ref e))) => {
                let in_xmla = matches!(
                    &namespace,
                    ResolveResult::Bound(ns) if ns.as_ref() == XMLA_NAMESPACE
                );
                if in_xmla && matches!(e.local_name().as_ref(), b"Session" | b"EndSession") {
                    for attribute in e.attributes().flatten() {
                        if attribute.key.as_ref() == b"SessionId" {
                            return attribute
                                .unescape_value()
                                .ok()
                                .map(|value| value.to_string());
                        }
                    }
                }
            }
            Ok((_, Event::Eof)) | Err(_) => return None,
            _ => {}
        }
    }
}

pub fn parse_xmla(xml: &str) -> XmlaRequest {
    let mut reader = NsReader::from_str(xml);

    let mut in_request_type = false;
    let mut is_execute = false;
    let mut in_property_name = false;
    let mut in_statement = false;
    let mut is_begin_session = false;
    let mut in_property_type = false;
    let mut in_member_unique_name = false;
    let mut in_tree_op = false;
    let mut in_restriction_list = false;
    let mut restriction_name: Option<Vec<u8>> = None;
    let mut restrictions = Restrictions::default();
    // Text accumulates for the element currently open (Text and CDATA alike)
    // and is consumed when that element ends. Processing per text event lost
    // mixed content and ignored CDATA entirely (plan 051).
    let mut pending_text = String::new();
    // `<Restrictions>` may only contain `<RestrictionList>`: its direct
    // children are checked when they start, because the reference rejects
    // anything else at the schema layer and ignoring it answered with a wider
    // rowset (verified 2026-09-24).
    let mut in_restrictions = false;
    let mut restriction_list_seen = false;
    // Open elements as (local name, namespace ok), so structural checks can
    // require the right parent (e.g. `<Restrictions>` directly under
    // `<Discover>`, not inside a `<Command>`).
    let mut open_elements: Vec<(Vec<u8>, bool, bool)> = Vec::new();
    let mut restrictions_seen = false;
    // Explicit `xmlns=""` (as opposed to no declaration at all) makes an
    // element invalid for the reference; the flag is inherited until a new
    // default namespace is declared.
    let mut default_undeclared_stack: Vec<bool> = Vec::new();
    let mut envelope_seen = false;
    let mut body_seen = false;
    let mut header_seen = false;
    let mut malformed: Option<String> = None;

    let mut parsed_request_type = String::new();
    let mut requested_properties: Vec<String> = Vec::new();
    let mut statement_text = String::new();
    let mut property_type: Option<i32> = None;
    let mut member_unique_name: Option<String> = None;
    let mut tree_op: Option<i32> = None;

    loop {
        let (namespace, event) = match reader.read_resolved_event() {
            Ok(resolved) => resolved,
            Err(_) => {
                malformed.get_or_insert_with(|| "request XML could not be read".to_string());
                break;
            }
        };
        let mut namespace_ok = match &namespace {
            ResolveResult::Bound(ns) => ns.as_ref() == XMLA_NAMESPACE,
            ResolveResult::Unbound => true,
            ResolveResult::Unknown(_) => false,
        };
        let mut soap_ok = match &namespace {
            ResolveResult::Bound(ns) => ns.as_ref() == SOAP_NAMESPACE,
            ResolveResult::Unbound => true,
            ResolveResult::Unknown(_) => false,
        };
        match event {
            Event::Start(ref e) => {
                let name = e.local_name();
                let inherited = default_undeclared_stack.last().copied().unwrap_or(false);
                let default_undeclared = match default_namespace_attribute(e) {
                    Some(declares) => !declares,
                    None => inherited,
                };
                if default_undeclared && matches!(&namespace, ResolveResult::Unbound) {
                    namespace_ok = false;
                    soap_ok = false;
                }
                if !is_qname(e.name().as_ref()) {
                    malformed.get_or_insert_with(|| {
                        "request contains an invalid element name".to_string()
                    });
                }
                if let Some(reason) = attribute_error(e) {
                    malformed.get_or_insert(reason);
                }
                if malformed.is_none()
                    && let Some(reason) = structural_error(
                        name.as_ref(),
                        Structure {
                            namespace_ok,
                            soap_ok,
                            parent_is_xmla_discover: parent_is_xmla_discover(&open_elements),
                            parent_is_soap_envelope: parent_is_soap_envelope(&open_elements),
                            is_root: open_elements.is_empty(),
                            restrictions_seen,
                            in_restrictions,
                            in_restriction_list,
                            restriction_list_seen,
                            body_seen,
                            header_seen,
                        },
                    )
                {
                    malformed = Some(reason);
                }
                if in_property_name && !matches!(name.as_ref(), b"Value" | b"value") {
                    malformed
                        .get_or_insert_with(|| "unexpected child of <PropertyName>".to_string());
                }
                default_undeclared_stack.push(default_undeclared);
                if name.as_ref() == b"Envelope" && soap_ok {
                    envelope_seen = true;
                }
                if parent_is_soap_envelope(&open_elements) {
                    if name.as_ref() == b"Body" {
                        body_seen = true;
                    }
                    if name.as_ref() == b"Header" {
                        header_seen = true;
                    }
                }
                open_elements.push((name.as_ref().to_vec(), namespace_ok, soap_ok));
                match name.as_ref() {
                    b"RequestType" => in_request_type = true,
                    b"PropertyName" => {
                        in_property_name = true;
                        if in_restriction_list {
                            restrictions.seen.push("PropertyName".into());
                        }
                    }
                    b"Statement" => in_statement = true,
                    b"BeginSession" | b"BeginGetSessionToken" => is_begin_session = true,
                    b"Execute" => is_execute = true,
                    b"Restrictions" => {
                        in_restrictions = true;
                        restrictions_seen = true;
                    }
                    b"PROPERTY_TYPE" => {
                        in_property_type = true;
                        if in_restriction_list {
                            restrictions.seen.push("PROPERTY_TYPE".into());
                        }
                    }
                    b"MEMBER_UNIQUE_NAME" => {
                        in_member_unique_name = true;
                        if in_restriction_list {
                            restrictions.seen.push("MEMBER_UNIQUE_NAME".into());
                        }
                    }
                    b"TREE_OP" => {
                        in_tree_op = true;
                        if in_restriction_list {
                            restrictions.seen.push("TREE_OP".into());
                        }
                    }
                    b"RestrictionList" => {
                        in_restriction_list = true;
                        restriction_list_seen = true;
                    }
                    name => {
                        // `<Value>`/`<value>` carry a restriction's value only
                        // inside the element that names the restriction
                        // (`<PropertyName><Value>x</Value>`); on their own they
                        // are an unadvertised restriction name and must fault
                        // (plan 055 review).
                        let nested_value = matches!(name, b"Value" | b"value") && in_property_name;
                        if in_restriction_list && !nested_value {
                            restriction_name = Some(name.to_vec());
                        }
                    }
                }
            }
            Event::Empty(ref e) => {
                let name = e.local_name();
                let inherited = default_undeclared_stack.last().copied().unwrap_or(false);
                let default_undeclared = match default_namespace_attribute(e) {
                    Some(declares) => !declares,
                    None => inherited,
                };
                if default_undeclared && matches!(&namespace, ResolveResult::Unbound) {
                    namespace_ok = false;
                    soap_ok = false;
                }
                if name.as_ref() == b"Execute" {
                    is_execute = true;
                }
                if !is_qname(e.name().as_ref()) {
                    malformed.get_or_insert_with(|| {
                        "request contains an invalid element name".to_string()
                    });
                }
                if let Some(reason) = attribute_error(e) {
                    malformed.get_or_insert(reason);
                }
                if malformed.is_none()
                    && let Some(reason) = structural_error(
                        name.as_ref(),
                        Structure {
                            namespace_ok,
                            soap_ok,
                            parent_is_xmla_discover: parent_is_xmla_discover(&open_elements),
                            parent_is_soap_envelope: parent_is_soap_envelope(&open_elements),
                            is_root: open_elements.is_empty(),
                            restrictions_seen,
                            in_restrictions,
                            in_restriction_list,
                            restriction_list_seen,
                            body_seen,
                            header_seen,
                        },
                    )
                {
                    malformed = Some(reason);
                }
                if name.as_ref() == b"Restrictions" {
                    restrictions_seen = true;
                }
                if name.as_ref() == b"RestrictionList" {
                    restriction_list_seen = true;
                }
                if name.as_ref() == b"Envelope" && soap_ok {
                    envelope_seen = true;
                }
                if parent_is_soap_envelope(&open_elements) {
                    if name.as_ref() == b"Body" {
                        body_seen = true;
                    }
                    if name.as_ref() == b"Header" {
                        header_seen = true;
                    }
                }
                // A self-closing restriction name still names a restriction:
                // the reference faults an unadvertised one even when its value
                // is empty, and ignores an advertised one (verified
                // 2026-09-24). `<Value>` is only a value inside its naming
                // element.
                let nested_value = matches!(name.as_ref(), b"Value" | b"value") && in_property_name;
                if in_restriction_list && !nested_value && name.as_ref() != b"RestrictionList" {
                    restrictions
                        .seen
                        .push(String::from_utf8_lossy(name.as_ref()).to_string());
                }
            }
            Event::Text(e) => match e.unescape() {
                Ok(decoded) => {
                    if has_xml_invalid_control(&decoded) {
                        malformed.get_or_insert_with(|| {
                            "request text contains an XML-invalid control character".to_string()
                        });
                    }
                    pending_text.push_str(&decoded);
                }
                Err(_) => {
                    malformed.get_or_insert_with(|| {
                        "request text contains an unparsable XML entity".to_string()
                    });
                }
            },
            // CDATA is literal text: .NET/PowerShell/Java SOAP clients wrap
            // statements in it, and ignoring it answered them with an empty
            // cellset (plan 051).
            Event::CData(e) => {
                let decoded = String::from_utf8_lossy(e.as_ref());
                if has_xml_invalid_control(&decoded) {
                    malformed.get_or_insert_with(|| {
                        "request text contains an XML-invalid control character".to_string()
                    });
                }
                pending_text.push_str(&decoded);
            }
            Event::End(ref e) => {
                let name = e.local_name();
                open_elements.pop();
                default_undeclared_stack.pop();
                let text = pending_text.trim().to_string();
                if text.is_empty() {
                    // An empty value does not apply a restriction, but the name
                    // still has to be advertised: the reference faults an
                    // unadvertised name even when empty (verified 2026-09-24).
                    if in_restriction_list && let Some(restriction) = restriction_name.as_deref() {
                        restrictions
                            .seen
                            .push(String::from_utf8_lossy(restriction).to_string());
                    }
                } else {
                    if in_restriction_list && let Some(restriction) = restriction_name.as_deref() {
                        restrictions
                            .seen
                            .push(String::from_utf8_lossy(restriction).to_string());
                        apply_restriction(&mut restrictions, restriction, &text);
                    }
                    if in_request_type {
                        parsed_request_type = text.clone();
                    } else if in_property_name {
                        requested_properties.push(text.clone());
                    } else if in_statement {
                        statement_text.push_str(&text);
                    } else if in_property_type {
                        if let Ok(v) = text.parse::<i32>() {
                            property_type = Some(v);
                        }
                    } else if in_member_unique_name {
                        member_unique_name = Some(text.clone());
                    } else if in_tree_op && let Ok(v) = text.parse::<i32>() {
                        tree_op = Some(v);
                    }
                }
                pending_text.clear();
                match name.as_ref() {
                    b"RequestType" => in_request_type = false,
                    b"PropertyName" => in_property_name = false,
                    b"Statement" => in_statement = false,
                    b"PROPERTY_TYPE" => in_property_type = false,
                    b"MEMBER_UNIQUE_NAME" => in_member_unique_name = false,
                    b"TREE_OP" => in_tree_op = false,
                    b"Restrictions" => in_restrictions = false,
                    b"RestrictionList" => {
                        in_restriction_list = false;
                        restriction_name = None;
                    }
                    _ if in_restriction_list => restriction_name = None,
                    _ => {}
                }
            }
            Event::Eof => break,
            // Comments, processing instructions, declarations and doctypes are
            // lexical surface too: the reference rejects control characters in
            // them (measured 2026-09-24).
            Event::Comment(e) => lexical_error(&mut malformed, e.as_ref()),
            Event::PI(e) => lexical_error(&mut malformed, e.as_ref()),
            Event::Decl(e) => lexical_error(&mut malformed, e.as_ref()),
            // The reference prohibits DTDs outright ("DTD is prohibited",
            // measured 2026-09-24).
            Event::DocType(_) => {
                malformed.get_or_insert_with(|| "DTD is prohibited".to_string());
            }
        }
    }

    if envelope_seen && !body_seen {
        return XmlaRequest::Malformed("<Body> is required under the SOAP envelope".to_string());
    }

    if let Some(reason) = malformed {
        return XmlaRequest::Malformed(reason);
    }

    // The rowset's advertised contract is the set of restriction names it
    // accepts (what DISCOVER_SCHEMA_ROWSETS tells clients). The reference
    // faults on anything else — "The restriction, X, is not recognized by the
    // server" — rather than answering with a wider rowset (verified against
    // SSAS 2025, 2026-09-24).
    if !parsed_request_type.is_empty()
        && let Some(name) = restrictions
            .seen
            .iter()
            .find(|name| !crate::xmla::schema_rowsets::advertises(&parsed_request_type, name))
    {
        return XmlaRequest::UnsupportedRestriction(name.clone());
    }

    match parsed_request_type.as_str() {
        "DISCOVER_PROPERTIES" => {
            return XmlaRequest::DiscoverProperties {
                property_names: requested_properties,
            };
        }
        "DISCOVER_SCHEMA_ROWSETS" => {
            return XmlaRequest::DiscoverSchemaRowsets {
                schema_name: restrictions.schema_name.clone(),
            };
        }
        "DISCOVER_LITERALS" => return XmlaRequest::DiscoverLiterals,
        "DBSCHEMA_CATALOGS" => return XmlaRequest::DbSchemaCatalogs,
        "MDSCHEMA_CUBES" => return XmlaRequest::MdschemaCubes,
        "DBSCHEMA_TABLES" => return XmlaRequest::DbschemaTables,
        "MDSCHEMA_DIMENSIONS" => return XmlaRequest::MdschemaDimensions,
        "MDSCHEMA_MEASURES" => return XmlaRequest::MdschemaMeasures,
        "MDSCHEMA_HIERARCHIES" => {
            return XmlaRequest::MdschemaHierarchies {
                restrictions: restrictions.clone(),
            };
        }
        "MDSCHEMA_LEVELS" => {
            return XmlaRequest::MdschemaLevels {
                restrictions: restrictions.clone(),
            };
        }
        "MDSCHEMA_FUNCTIONS" => {
            return XmlaRequest::MdschemaFunctions {
                restrictions: restrictions.clone(),
            };
        }
        "MDSCHEMA_PROPERTIES" => {
            return XmlaRequest::MdschemaProperties {
                property_type,
                restrictions,
            };
        }
        "MDSCHEMA_MEMBERS" => {
            return XmlaRequest::MdschemaMembers {
                member_unique_name,
                tree_op,
                restrictions,
            };
        }
        "MDSCHEMA_SETS" => return XmlaRequest::MdschemaSets,
        "MDSCHEMA_KPIS" => return XmlaRequest::MdschemaKpis,
        "MDSCHEMA_MEASUREGROUPS" => return XmlaRequest::MdschemaMeasureGroups,
        "MDSCHEMA_MEASUREGROUP_DIMENSIONS" => return XmlaRequest::MdschemaMeasureGroupDimensions,
        "TMSCHEMA_MODEL" => return XmlaRequest::TmschemaModel,
        "TMSCHEMA_TABLES" => return XmlaRequest::TmschemaTables,
        "TMSCHEMA_COLUMNS" => return XmlaRequest::TmschemaColumns,
        "TMSCHEMA_MEASURES" => return XmlaRequest::TmschemaMeasures,
        "TMSCHEMA_HIERARCHIES" => return XmlaRequest::TmschemaHierarchies,
        "TMSCHEMA_LEVELS" => return XmlaRequest::TmschemaLevels,
        "TMSCHEMA_RELATIONSHIPS" => return XmlaRequest::TmschemaRelationships,
        "TMSCHEMA_PARTITIONS" => return XmlaRequest::TmschemaPartitions,
        "DISCOVER_XML_METADATA" => return XmlaRequest::DiscoverXmlMetadata,
        "DISCOVER_CALC_DEPENDENCY" => return XmlaRequest::DiscoverCalcDependency,
        "DISCOVER_ENUMERATORS" => return XmlaRequest::DiscoverEnumerators,
        "DISCOVER_KEYWORDS" => return XmlaRequest::DiscoverKeywords,
        "DISCOVER_DATASOURCES" => return XmlaRequest::DiscoverDatasources,
        _ => (),
    };

    if is_execute {
        if !statement_text.trim().is_empty() {
            return XmlaRequest::ExecuteStatement(statement_text);
        } else if is_begin_session {
            return XmlaRequest::BeginSession;
        } else {
            return XmlaRequest::ExecuteEmpty;
        }
    }

    XmlaRequest::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CDATA is what .NET, PowerShell and Java SOAP clients emit; ignoring it
    /// answered them with an empty cellset instead of running the query.
    #[test]
    fn cdata_statement_is_read() {
        let xml = r#"<Envelope><Body><Execute><Command>
            <Statement><![CDATA[SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales]]]></Statement>
        </Command></Execute></Body></Envelope>"#;
        match parse_xmla(xml) {
            XmlaRequest::ExecuteStatement(statement) => {
                assert!(
                    statement.contains("SELECT {[Measures].[Revenue]}"),
                    "{statement}"
                );
            }
            other => panic!("expected the statement, got {other:?}"),
        }
    }

    /// Mixed content used to overwrite: the last text event won.
    #[test]
    fn statement_text_accumulates_across_events() {
        let xml = r#"<Envelope><Body><Execute><Command>
            <Statement>SELECT {[Measures].[Revenue]} <![CDATA[ON COLUMNS]]> FROM [Sales]</Statement>
        </Command></Execute></Body></Envelope>"#;
        match parse_xmla(xml) {
            XmlaRequest::ExecuteStatement(statement) => {
                assert_eq!(
                    statement.split_whitespace().collect::<Vec<_>>().join(" "),
                    "SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales]"
                );
            }
            other => panic!("expected the statement, got {other:?}"),
        }
    }

    /// Unreadable text must fault: a blanked statement hid the problem, and a
    /// dropped restriction silently returned the unrestricted rowset.
    #[test]
    fn unparsable_entity_is_malformed() {
        let statement = r#"<Envelope><Body><Execute><Command>
            <Statement>SELECT FROM [Sales] &foo; WHERE</Statement>
        </Command></Execute></Body></Envelope>"#;
        assert!(
            matches!(parse_xmla(statement), XmlaRequest::Malformed(_)),
            "an unparsable entity in a statement must fault"
        );

        let restriction = r#"<Envelope><Body><Discover>
            <RequestType>MDSCHEMA_MEMBERS</RequestType>
            <Restrictions><RestrictionList>
              <CUBE_NAME>Sales</CUBE_NAME>
              <DIMENSION_UNIQUE_NAME>[Category]&foo;</DIMENSION_UNIQUE_NAME>
            </RestrictionList></Restrictions>
        </Discover></Body></Envelope>"#;
        assert!(
            matches!(parse_xmla(restriction), XmlaRequest::Malformed(_)),
            "an unparsable entity in a restriction must fault, not widen"
        );
    }

    /// An empty or whitespace-only Statement — paired or self-closing — is the
    /// reference's empty success, not a malformed request. MSOLAP's session
    /// begin carries exactly `<Statement/>`, so faulting it broke every real
    /// connection (verified against SSAS 2025, 2026-09-24).
    #[test]
    fn empty_statement_is_the_empty_success() {
        for body in [
            r#"<Envelope><Body><Execute><Command><Statement></Statement></Command></Execute></Body></Envelope>"#,
            r#"<Envelope><Body><Execute><Command><Statement/></Command></Execute></Body></Envelope>"#,
            r#"<Envelope><Body><Execute><Command><Statement>   </Statement></Command></Execute></Body></Envelope>"#,
        ] {
            assert!(
                matches!(parse_xmla(body), XmlaRequest::ExecuteEmpty),
                "empty statements answer like the reference: {body}"
            );
        }

        let absent = r#"<Envelope><Body><Execute><Command/></Execute></Body></Envelope>"#;
        assert!(matches!(parse_xmla(absent), XmlaRequest::ExecuteEmpty));
    }

    /// Both nested restriction forms must land in the same fields as the flat
    /// one; they used to be ignored, widening the rowset with no diagnostic.
    /// A restriction name the rowset does not advertise is a client error: the
    /// reference faults rather than answering with a wider rowset (verified
    /// against SSAS 2025, 2026-09-24).
    #[test]
    fn unadvertised_restriction_names_fault() {
        let bogus = r#"<Envelope><Body><Discover><RequestType>MDSCHEMA_DIMENSIONS</RequestType>
            <Restrictions><RestrictionList><CUBE_NAME>Sales</CUBE_NAME><BOGUS_NAME>x</BOGUS_NAME></RestrictionList></Restrictions>
        </Discover></Body></Envelope>"#;
        match parse_xmla(bogus) {
            XmlaRequest::UnsupportedRestriction(name) => assert_eq!(name, "BOGUS_NAME"),
            other => panic!("expected UnsupportedRestriction, got {other:?}"),
        }

        // A name advertised for a different rowset is misrouted, not valid.
        let misrouted = r#"<Envelope><Body><Discover><RequestType>MDSCHEMA_DIMENSIONS</RequestType>
            <Restrictions><RestrictionList><MEMBER_TYPE>1</MEMBER_TYPE></RestrictionList></Restrictions>
        </Discover></Body></Envelope>"#;
        assert!(matches!(
            parse_xmla(misrouted),
            XmlaRequest::UnsupportedRestriction(_)
        ));

        // Advertised for this rowset: parses as before, no fault.
        let advertised = r#"<Envelope><Body><Discover><RequestType>MDSCHEMA_DIMENSIONS</RequestType>
            <Restrictions><RestrictionList><DIMENSION_VISIBILITY>1</DIMENSION_VISIBILITY></RestrictionList></Restrictions>
        </Discover></Body></Envelope>"#;
        assert!(matches!(
            parse_xmla(advertised),
            XmlaRequest::MdschemaDimensions
        ));
    }

    /// Every child of `<Restrictions>` other than `<RestrictionList>` is a
    /// schema error for the reference — it answers a fault, not a rowset
    /// (verified 2026-09-24). Ignoring those forms widened the rowset; the
    /// nested `<restriction>` handling this proxy once had was an invention
    /// (plan 055 review).
    #[test]
    fn invalid_restriction_children_are_rejected() {
        for body in [
            r#"<Envelope><Body><Discover>
                <RequestType>MDSCHEMA_MEMBERS</RequestType>
                <Restrictions><restriction>
                  <CUBE_NAME>Sales</CUBE_NAME>
                </restriction></Restrictions>
            </Discover></Body></Envelope>"#,
            r#"<Envelope><Body><Discover>
                <RequestType>MDSCHEMA_MEMBERS</RequestType>
                <Restrictions>
                  <restriction><column>CUBE_NAME</column><value>Sales</value></restriction>
                </Restrictions>
            </Discover></Body></Envelope>"#,
            r#"<Envelope><Body><Discover>
                <RequestType>MDSCHEMA_HIERARCHIES</RequestType>
                <Restrictions><DIMENSION_UNIQUE_NAME>[Category]</DIMENSION_UNIQUE_NAME></Restrictions>
            </Discover></Body></Envelope>"#,
            r#"<Envelope><Body><Discover>
                <RequestType>MDSCHEMA_HIERARCHIES</RequestType>
                <Restrictions><BOGUS_NAME>x</BOGUS_NAME></Restrictions>
            </Discover></Body></Envelope>"#,
        ] {
            assert!(
                matches!(parse_xmla(body), XmlaRequest::Malformed(_)),
                "{body}"
            );
        }
    }

    /// A restriction name is validated even when its value is empty: the
    /// reference faults an unadvertised name regardless of the value, and
    /// ignores an advertised name with an empty value (verified 2026-09-24).
    #[test]
    fn empty_restriction_values_still_validate_the_name() {
        for body in [
            r#"<Envelope><Body><Discover><RequestType>MDSCHEMA_HIERARCHIES</RequestType>
                <Restrictions><RestrictionList><BOGUS_NAME/></RestrictionList></Restrictions>
            </Discover></Body></Envelope>"#,
            r#"<Envelope><Body><Discover><RequestType>MDSCHEMA_HIERARCHIES</RequestType>
                <Restrictions><RestrictionList><BOGUS_NAME></BOGUS_NAME></RestrictionList></Restrictions>
            </Discover></Body></Envelope>"#,
        ] {
            assert!(matches!(
                parse_xmla(body),
                XmlaRequest::UnsupportedRestriction(_)
            ));
        }

        // Advertised and empty: parsed, and the empty value does not filter.
        let known_empty = r#"<Envelope><Body><Discover><RequestType>MDSCHEMA_HIERARCHIES</RequestType>
            <Restrictions><RestrictionList><DIMENSION_UNIQUE_NAME/></RestrictionList></Restrictions>
        </Discover></Body></Envelope>"#;
        assert!(matches!(
            parse_xmla(known_empty),
            XmlaRequest::MdschemaHierarchies { .. }
        ));
    }

    /// A `<Value>` element is a value only inside its naming element. On its
    /// own it is an unadvertised restriction name and must fault, not vanish
    /// (plan 055 review).
    #[test]
    fn direct_value_restrictions_are_not_exempt() {
        let direct = r#"<Envelope><Body><Discover><RequestType>DISCOVER_PROPERTIES</RequestType>
            <Restrictions><RestrictionList><Value>ServerName</Value></RestrictionList></Restrictions>
        </Discover></Body></Envelope>"#;
        assert!(matches!(
            parse_xmla(direct),
            XmlaRequest::UnsupportedRestriction(_)
        ));

        let self_closing = r#"<Envelope><Body><Discover><RequestType>DISCOVER_PROPERTIES</RequestType>
            <Restrictions><RestrictionList><Value/></RestrictionList></Restrictions>
        </Discover></Body></Envelope>"#;
        assert!(matches!(
            parse_xmla(self_closing),
            XmlaRequest::UnsupportedRestriction(_)
        ));

        // Excel's real form: the value belongs to PropertyName and is collected.
        let nested = r#"<Envelope><Body><Discover><RequestType>DISCOVER_PROPERTIES</RequestType>
            <Restrictions><RestrictionList><PropertyName><Value>Catalog</Value></PropertyName></RestrictionList></Restrictions>
        </Discover></Body></Envelope>"#;
        match parse_xmla(nested) {
            XmlaRequest::DiscoverProperties { property_names } => {
                assert_eq!(property_names, vec!["Catalog".to_string()]);
            }
            other => panic!("expected properties, got {other:?}"),
        }
    }

    /// The structural rules the reference's schema enforces: `<Restrictions>`
    /// is a direct child of `<Discover>`, unprefixed, with one
    /// `<RestrictionList>` (plan 055 review).
    #[test]
    fn structural_violations_are_malformed() {
        for body in [
            // Nested inside a foreign element.
            r#"<Envelope><Body><Discover><RequestType>MDSCHEMA_DIMENSIONS</RequestType>
                <Command><Restrictions><RestrictionList><CUBE_NAME>Sales</CUBE_NAME></RestrictionList></Restrictions></Command>
            </Discover></Body></Envelope>"#,
            // Two lists.
            r#"<Envelope><Body><Discover><RequestType>MDSCHEMA_DIMENSIONS</RequestType>
                <Restrictions><RestrictionList><CUBE_NAME>Sales</CUBE_NAME></RestrictionList>
                <RestrictionList><CUBE_NAME>Sales</CUBE_NAME></RestrictionList></Restrictions>
            </Discover></Body></Envelope>"#,
            // Prefixed structural element.
            r#"<Envelope><Body><Discover><RequestType>MDSCHEMA_DIMENSIONS</RequestType>
                <ms:Restrictions><RestrictionList><CUBE_NAME>Sales</CUBE_NAME></RestrictionList></ms:Restrictions>
            </Discover></Body></Envelope>"#,
        ] {
            assert!(
                matches!(parse_xmla(body), XmlaRequest::Malformed(_)),
                "{body}"
            );
        }
    }

    /// The reference requires the XMLA namespace on every element it
    /// interprets, SOAP 1.1 for the envelope, and rejects malformed names and
    /// DTDs (measured 2026-09-24). All of these used to be accepted.
    #[test]
    fn semantic_namespaces_soap_and_lexical_names_are_enforced() {
        // A foreign RequestType under an XMLA Discover.
        let foreign_request_type = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body>
            <Discover xmlns="urn:schemas-microsoft-com:xml-analysis"><f:RequestType xmlns:f="urn:example:foreign">MDSCHEMA_DIMENSIONS</f:RequestType></Discover>
        </s:Body></s:Envelope>"#;
        assert!(matches!(
            parse_xmla(foreign_request_type),
            XmlaRequest::Malformed(_)
        ));

        // An undeclared prefix on Execute, and a foreign Execute.
        let undeclared = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><x:Execute><x:Command/></x:Execute></s:Body></s:Envelope>"#;
        assert!(matches!(parse_xmla(undeclared), XmlaRequest::Malformed(_)));

        let foreign_execute = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body>
            <f:Execute xmlns:f="urn:example:foreign"><f:Command><f:Statement>SELECT FROM [Sales]</f:Statement></f:Command></f:Execute>
        </s:Body></s:Envelope>"#;
        assert!(matches!(
            parse_xmla(foreign_execute),
            XmlaRequest::Malformed(_)
        ));

        // SOAP 1.2 and a foreign envelope are not SOAP 1.1; a non-soap prefix
        // bound to 1.1 is fine.
        let soap12 = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"><s:Body><Discover xmlns="urn:schemas-microsoft-com:xml-analysis"><RequestType>DISCOVER_DATASOURCES</RequestType></Discover></s:Body></s:Envelope>"#;
        assert!(matches!(parse_xmla(soap12), XmlaRequest::Malformed(_)));

        let non_soap_prefix = r#"<soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/"><soapenv:Body><Discover xmlns="urn:schemas-microsoft-com:xml-analysis"><RequestType>DISCOVER_DATASOURCES</RequestType></Discover></soapenv:Body></soapenv:Envelope>"#;
        assert!(matches!(
            parse_xmla(non_soap_prefix),
            XmlaRequest::DiscoverDatasources
        ));

        // Names: references are never part of a name.
        let bad_name = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><Discover xmlns="urn:schemas-microsoft-com:xml-analysis"><Request&amp;Type/><RequestType>DISCOVER_DATASOURCES</RequestType></Discover></s:Body></s:Envelope>"#;
        assert!(matches!(parse_xmla(bad_name), XmlaRequest::Malformed(_)));

        // DTDs are prohibited.
        let doctype = r#"<!DOCTYPE s:Envelope [<!ENTITY x "y">]><s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><Discover xmlns="urn:schemas-microsoft-com:xml-analysis"><RequestType>DISCOVER_DATASOURCES</RequestType></Discover></s:Body></s:Envelope>"#;
        assert!(matches!(parse_xmla(doctype), XmlaRequest::Malformed(_)));
    }

    /// The SOAP skeleton is positional, and the reference's other round-6
    /// answers: a foreign header entry is fine, a body without an envelope /
    /// nested body / missing body fault, explicit `xmlns=""` faults, and
    /// invalid QNames fault (measured 2026-09-24).
    #[test]
    fn soap_skeleton_qnames_and_undeclaration_match_the_reference() {
        // A foreign header entry named Header is not the SOAP Header.
        let foreign_header = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Header><f:Header xmlns:f="urn:probe:foreign"/></s:Header><s:Body><Discover xmlns="urn:schemas-microsoft-com:xml-analysis"><RequestType>DISCOVER_DATASOURCES</RequestType></Discover></s:Body></s:Envelope>"#;
        assert!(matches!(
            parse_xmla(foreign_header),
            XmlaRequest::DiscoverDatasources
        ));

        // A body without an envelope, a nested body, and no body at all.
        let stray_body = r#"<s:Body xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"/>"#;
        assert!(matches!(parse_xmla(stray_body), XmlaRequest::Malformed(_)));
        let nested_body = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><s:Body/></s:Body></s:Envelope>"#;
        assert!(matches!(parse_xmla(nested_body), XmlaRequest::Malformed(_)));
        let no_body =
            r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"></s:Envelope>"#;
        assert!(matches!(parse_xmla(no_body), XmlaRequest::Malformed(_)));

        // An explicit undeclaration of the XMLA namespace (as opposed to no
        // declaration at all, which stays the documented divergence).
        let undeclared = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body>
            <Discover xmlns="urn:schemas-microsoft-com:xml-analysis"><RequestType xmlns="">MDSCHEMA_DIMENSIONS</RequestType></Discover>
        </s:Body></s:Envelope>"#;
        assert!(matches!(parse_xmla(undeclared), XmlaRequest::Malformed(_)));

        // QName violations, element and attribute alike.
        for bad in ["<1Bogus/>", "<Bad~Name/>", "<a:b:c/>"] {
            let body = format!(
                "<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\"><s:Body><Discover xmlns=\"urn:schemas-microsoft-com:xml-analysis\">{bad}<RequestType>DISCOVER_DATASOURCES</RequestType></Discover></s:Body></s:Envelope>"
            );
            assert!(
                matches!(parse_xmla(&body), XmlaRequest::Malformed(_)),
                "{bad}"
            );
        }
        let bad_attr = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><Discover xmlns="urn:schemas-microsoft-com:xml-analysis" 1foo="x"><RequestType>DISCOVER_DATASOURCES</RequestType></Discover></s:Body></s:Envelope>"#;
        assert!(matches!(parse_xmla(bad_attr), XmlaRequest::Malformed(_)));
    }

    /// The session id is read from an XMLA `Session`/`EndSession` only and
    /// returned XML-unescaped (lossless); a foreign header entry named
    /// `Session` is ignored like the reference (measured 2026-09-24).
    #[test]
    fn session_id_is_namespace_aware_and_lossless() {
        let xmla_session = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Header><Session xmlns="urn:schemas-microsoft-com:xml-analysis" SessionId="id with spaces"/></s:Header><s:Body/></s:Envelope>"#;
        assert_eq!(session_id(xmla_session), Some("id with spaces".to_string()));

        let escaped = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Header><Session xmlns="urn:schemas-microsoft-com:xml-analysis" SessionId="a&amp;b"/></s:Header><s:Body/></s:Envelope>"#;
        assert_eq!(session_id(escaped), Some("a&b".to_string()));

        let foreign = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Header><f:Session xmlns:f="urn:probe:foreign" SessionId="FOREIGN-ID"/></s:Header><s:Body/></s:Envelope>"#;
        assert_eq!(session_id(foreign), None);

        let end_session = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Header><EndSession xmlns="urn:schemas-microsoft-com:xml-analysis" SessionId="abc"/></s:Header><s:Body/></s:Envelope>"#;
        assert_eq!(session_id(end_session), Some("abc".to_string()));
    }

    /// Namespace binding decides, not the prefix: the reference accepts a
    /// prefixed element bound to the XMLA namespace and rejects a foreign one
    /// (measured 2026-09-24).
    #[test]
    fn namespace_binding_decides_not_the_prefix() {
        let prefixed = r#"<Envelope><Body>
            <x:Discover xmlns:x="urn:schemas-microsoft-com:xml-analysis">
              <x:RequestType>MDSCHEMA_DIMENSIONS</x:RequestType>
              <x:Restrictions><x:RestrictionList><x:CUBE_NAME>Sales</x:CUBE_NAME></x:RestrictionList></x:Restrictions>
            </x:Discover>
        </Body></Envelope>"#;
        assert!(matches!(
            parse_xmla(prefixed),
            XmlaRequest::MdschemaDimensions
        ));

        let foreign = r#"<Envelope><Body>
            <Discover xmlns="urn:schemas-microsoft-com:xml-analysis">
              <RequestType>MDSCHEMA_DIMENSIONS</RequestType>
              <other:Restrictions xmlns:other="http://example.com/other"><RestrictionList><CUBE_NAME>Sales</CUBE_NAME></RestrictionList></other:Restrictions>
            </Discover>
        </Body></Envelope>"#;
        assert!(matches!(parse_xmla(foreign), XmlaRequest::Malformed(_)));
    }

    /// The reference faults a nested `<PropertyName>` as a schema error
    /// (measured 2026-09-24); it used to be reinterpreted as a value.
    #[test]
    fn nested_property_name_is_malformed() {
        let body = r#"<Envelope><Body><Discover><RequestType>DISCOVER_PROPERTIES</RequestType>
            <Restrictions><RestrictionList><PropertyName><PropertyName>Catalog</PropertyName></PropertyName></RestrictionList></Restrictions>
        </Discover></Body></Envelope>"#;
        assert!(matches!(parse_xmla(body), XmlaRequest::Malformed(_)));
    }

    /// The reference rejects control characters in comment and PI content
    /// ("Illegal xml character", measured 2026-09-24).
    #[test]
    fn control_characters_in_comments_and_pis_are_malformed() {
        let comment = format!(
            "<Envelope><Body><Discover><RequestType>MDSCHEMA_HIERARCHIES</RequestType><!-- a{}b --></Discover></Body></Envelope>",
            '\u{1}'
        );
        assert!(matches!(parse_xmla(&comment), XmlaRequest::Malformed(_)));

        let instruction = format!(
            "<Envelope><Body><Discover><RequestType>MDSCHEMA_HIERARCHIES</RequestType><?pi a{}b?></Discover></Body></Envelope>",
            '\u{1}'
        );
        assert!(matches!(
            parse_xmla(&instruction),
            XmlaRequest::Malformed(_)
        ));
    }

    /// A control character in an attribute *name*, and a self-closing
    /// duplicate `<Restrictions>`, are both faults for the reference (measured
    /// 2026-09-24).
    #[test]
    fn attribute_names_and_duplicate_restrictions_are_checked() {
        let attribute = format!(
            "<Envelope><Body><Discover><RequestType>MDSCHEMA_HIERARCHIES</RequestType><Restrictions><RestrictionList><CUBE_NAME foo{}bar=\"x\">Sales</CUBE_NAME></RestrictionList></Restrictions></Discover></Body></Envelope>",
            '\u{1}'
        );
        assert!(matches!(parse_xmla(&attribute), XmlaRequest::Malformed(_)));

        let duplicate = r#"<Envelope><Body><Discover><RequestType>MDSCHEMA_DIMENSIONS</RequestType>
            <Restrictions><RestrictionList><CUBE_NAME>Sales</CUBE_NAME></RestrictionList></Restrictions>
            <Restrictions/>
        </Discover></Body></Envelope>"#;
        assert!(matches!(parse_xmla(duplicate), XmlaRequest::Malformed(_)));
    }

    /// XML 1.0 forbids control characters anywhere in a document; the
    /// reference's parser rejects the request before the protocol layer sees
    /// it. CDATA and attributes are paths the text check alone misses (plan
    /// 055 review).
    #[test]
    fn control_characters_in_cdata_and_attributes_are_malformed() {
        let cdata = format!(
            "<Envelope><Body><Discover><RequestType>MDSCHEMA_HIERARCHIES</RequestType>\
             <Restrictions><RestrictionList><DIMENSION_UNIQUE_NAME><![CDATA[{}]]></DIMENSION_UNIQUE_NAME></RestrictionList></Restrictions>\
             </Discover></Body></Envelope>",
            '\u{1}'
        );
        assert!(matches!(parse_xmla(&cdata), XmlaRequest::Malformed(_)));

        let attribute = format!(
            "<Envelope xmlns:soap=\"http://schemas.xmlsoap.org/soap/envelope/\" data-x=\"{}\"\
             ><Body><Discover><RequestType>MDSCHEMA_HIERARCHIES</RequestType></Discover></Body></Envelope>",
            '\u{1}'
        );
        assert!(matches!(parse_xmla(&attribute), XmlaRequest::Malformed(_)));
    }

    /// XML 1.0 forbids control characters; the reference's parser rejects the
    /// request, and a name carrying one must not reach our fault text (plan
    /// 055 review).
    #[test]
    fn control_characters_in_text_are_malformed() {
        let body = r#"<Envelope><Body><Discover><RequestType>MDSCHEMA_HIERARCHIES</RequestType>
            <Restrictions><RestrictionList><BOGUS_NAME>&#x1;</BOGUS_NAME></RestrictionList></Restrictions>
        </Discover></Body></Envelope>"#;
        assert!(matches!(parse_xmla(body), XmlaRequest::Malformed(_)));
    }

    #[test]
    fn restriction_list_is_parsed() {
        let xml = r#"<?xml version="1.0"?>
        <Envelope xmlns="http://schemas.xmlsoap.org/soap/envelope/">
          <Body>
            <Discover xmlns="urn:schemas-microsoft-com:xml-analysis">
              <RequestType>MDSCHEMA_PROPERTIES</RequestType>
              <Restrictions>
                <RestrictionList>
                  <CATALOG_NAME>SALES_ANALYTICS</CATALOG_NAME>
                  <CUBE_NAME>Sales</CUBE_NAME>
                  <HIERARCHY_UNIQUE_NAME>[Date].[Full Date]</HIERARCHY_UNIQUE_NAME>
                  <PROPERTY_NAME>MEMBER_VALUE</PROPERTY_NAME>
                  <PROPERTY_TYPE>1</PROPERTY_TYPE>
                </RestrictionList>
              </Restrictions>
              <Properties/>
            </Discover>
          </Body>
        </Envelope>"#;
        match parse_xmla(xml) {
            XmlaRequest::MdschemaProperties {
                property_type,
                restrictions,
            } => {
                assert_eq!(property_type, Some(1));
                assert_eq!(
                    restrictions.catalog_name.as_deref(),
                    Some("SALES_ANALYTICS")
                );
                assert_eq!(restrictions.cube_name.as_deref(), Some("Sales"));
                assert_eq!(
                    restrictions.hierarchy_unique_name.as_deref(),
                    Some("[Date].[Full Date]")
                );
                assert_eq!(restrictions.property_name.as_deref(), Some("MEMBER_VALUE"));
                assert_eq!(restrictions.dimension_unique_name, None);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn discover_without_restrictions_parses() {
        let xml = r#"<Envelope xmlns="http://schemas.xmlsoap.org/soap/envelope/"><Body><Discover xmlns="urn:schemas-microsoft-com:xml-analysis"><RequestType>MDSCHEMA_LEVELS</RequestType><Restrictions/><Properties/></Discover></Body></Envelope>"#;
        assert_eq!(
            parse_xmla(xml),
            XmlaRequest::MdschemaLevels {
                restrictions: Restrictions::default()
            }
        );
    }
}
