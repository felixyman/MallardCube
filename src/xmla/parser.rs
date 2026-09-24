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
    ExecuteStatement(String),
    Unknown,
}

/// Apply one restriction value by element name. Shared by the flat
/// `<RestrictionList>` form and both nested `<restriction>` forms so the paths
/// cannot diverge. Returns whether the name was recognised.
fn apply_restriction(restrictions: &mut Restrictions, name: &[u8], text: &str) -> bool {
    match name {
        b"CATALOG_NAME" => restrictions.catalog_name = Some(text.to_string()),
        b"CUBE_NAME" => restrictions.cube_name = Some(text.to_string()),
        b"DIMENSION_UNIQUE_NAME" => restrictions.dimension_unique_name = Some(text.to_string()),
        b"HIERARCHY_UNIQUE_NAME" => restrictions.hierarchy_unique_name = Some(text.to_string()),
        b"LEVEL_UNIQUE_NAME" => restrictions.level_unique_name = Some(text.to_string()),
        b"PROPERTY_NAME" => restrictions.property_name = Some(text.to_string()),
        b"ORIGIN" => restrictions.origin = text.parse().ok(),
        b"SchemaName" => restrictions.schema_name = Some(text.to_string()),
        _ => return false,
    }
    true
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
    // The nested restriction forms — `<restriction><NAME>…</NAME></restriction>`
    // and the standard `<restriction><column>…</column><value>…</value>` —
    // were ignored, so those clients silently received the unrestricted rowset.
    let mut in_nested_restriction = false;
    let mut pending_column: Option<String> = None;
    let mut malformed: Option<String> = None;

    let mut parsed_request_type = String::new();
    let mut requested_properties: Vec<String> = Vec::new();
    let mut statement_text = String::new();
    let mut property_type: Option<i32> = None;
    let mut member_unique_name: Option<String> = None;
    let mut tree_op: Option<i32> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => match e.local_name().as_ref() {
                b"RequestType" => in_request_type = true,
                b"PropertyName" => in_property_name = true,
                b"Statement" => in_statement = true,
                b"BeginSession" | b"BeginGetSessionToken" => is_begin_session = true,
                b"Execute" => is_execute = true,
                b"PROPERTY_TYPE" => in_property_type = true,
                b"MEMBER_UNIQUE_NAME" => in_member_unique_name = true,
                b"TREE_OP" => in_tree_op = true,
                b"RestrictionList" => in_restriction_list = true,
                b"restriction" => in_nested_restriction = true,
                b"column" | b"value" if in_nested_restriction => {}
                name => {
                    if in_restriction_list || in_nested_restriction {
                        restriction_name = Some(name.to_vec());
                    }
                }
            },
            Ok(Event::Empty(ref e)) if e.local_name().as_ref() == b"Execute" => {
                is_execute = true;
            }
            Ok(Event::Text(e)) => match e.unescape() {
                Ok(decoded) => pending_text.push_str(&decoded),
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
                pending_text.push_str(&String::from_utf8_lossy(e.as_ref()));
            }
            Ok(Event::End(ref e)) => {
                let name = e.local_name();
                let text = pending_text.trim().to_string();
                if !text.is_empty() {
                    match name.as_ref() {
                        // Standard nested form: `<column>` names the
                        // restriction, the following `<value>` carries it.
                        b"column" if in_nested_restriction => {
                            pending_column = Some(text.clone());
                        }
                        b"value" if in_nested_restriction => {
                            if let Some(column) = pending_column.take() {
                                apply_restriction(&mut restrictions, column.as_bytes(), &text);
                            }
                        }
                        _ => {
                            // Flat form and `<restriction><NAME>…</NAME>`: the
                            // element name is the restriction name.
                            if (in_restriction_list || in_nested_restriction)
                                && let Some(restriction) = restriction_name.as_deref()
                            {
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
                    b"restriction" => in_nested_restriction = false,
                    b"RestrictionList" => {
                        in_restriction_list = false;
                        restriction_name = None;
                    }
                    _ if in_restriction_list || in_nested_restriction => restriction_name = None,
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
    #[test]
    fn nested_restriction_forms_are_parsed() {
        let element_form = r#"<Envelope><Body><Discover>
            <RequestType>MDSCHEMA_MEMBERS</RequestType>
            <Restrictions><restriction>
              <CUBE_NAME>Sales</CUBE_NAME>
              <DIMENSION_UNIQUE_NAME>[Category]</DIMENSION_UNIQUE_NAME>
            </restriction></Restrictions>
        </Discover></Body></Envelope>"#;
        match parse_xmla(element_form) {
            XmlaRequest::MdschemaMembers { restrictions, .. } => {
                assert_eq!(restrictions.cube_name.as_deref(), Some("Sales"));
                assert_eq!(
                    restrictions.dimension_unique_name.as_deref(),
                    Some("[Category]")
                );
            }
            other => panic!("expected members, got {other:?}"),
        }

        let column_value_form = r#"<Envelope><Body><Discover>
            <RequestType>MDSCHEMA_MEMBERS</RequestType>
            <Restrictions>
              <restriction><column>CUBE_NAME</column><value>Sales</value></restriction>
              <restriction><column>DIMENSION_UNIQUE_NAME</column><value>[Category]</value></restriction>
            </Restrictions>
        </Discover></Body></Envelope>"#;
        match parse_xmla(column_value_form) {
            XmlaRequest::MdschemaMembers { restrictions, .. } => {
                assert_eq!(restrictions.cube_name.as_deref(), Some("Sales"));
                assert_eq!(
                    restrictions.dimension_unique_name.as_deref(),
                    Some("[Category]")
                );
            }
            other => panic!("expected members, got {other:?}"),
        }
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
