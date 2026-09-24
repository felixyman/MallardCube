use quick_xml::Reader;
use quick_xml::events::Event;

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

/// XML 1.0 forbids these characters anywhere in a document. The reference's
/// parser rejects such requests before the protocol layer sees them, and a
/// restriction name carrying one would make our own fault unparsable (plan 055
/// review).
fn has_xml_invalid_control(text: &str) -> bool {
    text.chars()
        .any(|c| matches!(c, '\u{0}'..='\u{8}' | '\u{b}' | '\u{c}' | '\u{e}'..='\u{1f}'))
}

fn attributes_have_control(element: &quick_xml::events::BytesStart<'_>) -> bool {
    element.attributes().flatten().any(|attribute| {
        attribute
            .unescape_value()
            .map(|value| has_xml_invalid_control(&value))
            .unwrap_or(false)
    })
}

/// Structural rules the reference's schema enforces (plan 055 review): a
/// `<Restrictions>` is an unprefixed direct child of `<Discover>`, holds at
/// most one `<RestrictionList>`, and nothing else lives directly under it.
fn structural_error(
    raw: &[u8],
    local: &[u8],
    parent: Option<&[u8]>,
    in_discover: bool,
    in_restrictions: bool,
    in_restriction_list: bool,
    restriction_list_seen: bool,
) -> Option<String> {
    // A prefixed element cannot be in the XMLA default namespace, so a
    // structural name with a prefix is a schema error.
    if matches!(local, b"Restrictions" | b"RestrictionList") && raw != local {
        return Some(format!(
            "<{}> must be in the default namespace",
            String::from_utf8_lossy(local)
        ));
    }
    match raw {
        b"Restrictions"
            if !in_discover || in_restrictions || parent != Some(b"Discover".as_slice()) =>
        {
            Some("<Restrictions> must be a direct child of <Discover>".to_string())
        }
        b"RestrictionList" if !in_restrictions || in_restriction_list || restriction_list_seen => {
            Some("<RestrictionList> must be the only child of <Restrictions>".to_string())
        }
        b"RestrictionList" => None,
        _ if in_restrictions && !in_restriction_list => Some(format!(
            "unexpected <{}> under <Restrictions>",
            String::from_utf8_lossy(raw)
        )),
        _ => None,
    }
}

pub fn parse_xmla(xml: &str) -> XmlaRequest {
    let mut reader = Reader::from_str(xml);

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
    let mut in_discover = false;
    let mut restriction_list_seen = false;
    // Raw names of the open elements, so structural checks can require the
    // right parent (e.g. `<Restrictions>` directly under `<Discover>`, not
    // inside a `<Command>`).
    let mut open_elements: Vec<Vec<u8>> = Vec::new();
    let mut malformed: Option<String> = None;

    let mut parsed_request_type = String::new();
    let mut requested_properties: Vec<String> = Vec::new();
    let mut statement_text = String::new();
    let mut property_type: Option<i32> = None;
    let mut member_unique_name: Option<String> = None;
    let mut tree_op: Option<i32> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let raw = e.name();
                let name = e.local_name();
                if attributes_have_control(e) {
                    malformed.get_or_insert_with(|| {
                        "request text contains an XML-invalid control character".to_string()
                    });
                }
                if malformed.is_none()
                    && let Some(reason) = structural_error(
                        raw.as_ref(),
                        name.as_ref(),
                        open_elements.last().map(Vec::as_slice),
                        in_discover,
                        in_restrictions,
                        in_restriction_list,
                        restriction_list_seen,
                    )
                {
                    malformed = Some(reason);
                }
                open_elements.push(raw.as_ref().to_vec());
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
                    b"Discover" => in_discover = true,
                    b"Restrictions" => in_restrictions = true,
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
            Ok(Event::Empty(ref e)) => {
                let raw = e.name();
                let name = e.local_name();
                if name.as_ref() == b"Execute" {
                    is_execute = true;
                }
                if name.as_ref() == b"Discover" {
                    in_discover = true;
                }
                if attributes_have_control(e) {
                    malformed.get_or_insert_with(|| {
                        "request text contains an XML-invalid control character".to_string()
                    });
                }
                if malformed.is_none()
                    && let Some(reason) = structural_error(
                        raw.as_ref(),
                        name.as_ref(),
                        open_elements.last().map(Vec::as_slice),
                        in_discover,
                        in_restrictions,
                        in_restriction_list,
                        restriction_list_seen,
                    )
                {
                    malformed = Some(reason);
                }
                if raw.as_ref() == b"RestrictionList" {
                    restriction_list_seen = true;
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
                        .push(String::from_utf8_lossy(raw.as_ref()).to_string());
                }
            }
            Ok(Event::Text(e)) => match e.unescape() {
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
            Ok(Event::CData(e)) => {
                let decoded = String::from_utf8_lossy(e.as_ref());
                if has_xml_invalid_control(&decoded) {
                    malformed.get_or_insert_with(|| {
                        "request text contains an XML-invalid control character".to_string()
                    });
                }
                pending_text.push_str(&decoded);
            }
            Ok(Event::End(ref e)) => {
                let name = e.local_name();
                open_elements.pop();
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
                    b"Discover" => in_discover = false,
                    b"Restrictions" => in_restrictions = false,
                    b"RestrictionList" => {
                        in_restriction_list = false;
                        restriction_name = None;
                    }
                    _ if in_restriction_list => restriction_name = None,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => {
                malformed.get_or_insert_with(|| "request XML could not be read".to_string());
                break;
            }
            _ => (),
        }
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
