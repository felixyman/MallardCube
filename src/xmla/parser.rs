use quick_xml::Reader;
use quick_xml::events::Event;

#[derive(Debug, Clone, PartialEq)]
pub enum XmlaRequest {
    DiscoverProperties {
        property_names: Vec<String>,
    },
    DiscoverSchemaRowsets,
    DiscoverLiterals,
    DbSchemaCatalogs,
    MdschemaCubes,
    DbschemaTables,
    MdschemaDimensions,
    MdschemaMeasures,
    MdschemaHierarchies,
    MdschemaLevels,
    MdschemaProperties {
        property_type: Option<i32>,
    },
    MdschemaMembers {
        member_unique_name: Option<String>,
        tree_op: Option<i32>,
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
    BeginSession,
    ExecuteEmpty,
    ExecuteStatement(String),
    Unknown,
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
                _ => (),
            },
            Ok(Event::Empty(ref e)) if e.local_name().as_ref() == b"Execute" => {
                is_execute = true;
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().trim().to_string();

                if !text.is_empty() {
                    if in_request_type {
                        parsed_request_type = text;
                    } else if in_property_name {
                        requested_properties.push(text);
                    } else if in_statement {
                        statement_text = text;
                    } else if in_property_type {
                        if let Ok(v) = text.parse::<i32>() {
                            property_type = Some(v);
                        }
                    } else if in_member_unique_name {
                        member_unique_name = Some(text);
                    } else if in_tree_op && let Ok(v) = text.parse::<i32>() {
                        tree_op = Some(v);
                    }
                }
            }
            Ok(Event::End(ref e)) => match e.local_name().as_ref() {
                b"RequestType" => in_request_type = false,
                b"PropertyName" => in_property_name = false,
                b"Statement" => in_statement = false,
                b"PROPERTY_TYPE" => in_property_type = false,
                b"MEMBER_UNIQUE_NAME" => in_member_unique_name = false,
                b"TREE_OP" => in_tree_op = false,
                _ => (),
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => (),
        }
    }

    match parsed_request_type.as_str() {
        "DISCOVER_PROPERTIES" => {
            return XmlaRequest::DiscoverProperties {
                property_names: requested_properties,
            };
        }
        "DISCOVER_SCHEMA_ROWSETS" => return XmlaRequest::DiscoverSchemaRowsets,
        "DISCOVER_LITERALS" => return XmlaRequest::DiscoverLiterals,
        "DBSCHEMA_CATALOGS" => return XmlaRequest::DbSchemaCatalogs,
        "MDSCHEMA_CUBES" => return XmlaRequest::MdschemaCubes,
        "DBSCHEMA_TABLES" => return XmlaRequest::DbschemaTables,
        "MDSCHEMA_DIMENSIONS" => return XmlaRequest::MdschemaDimensions,
        "MDSCHEMA_MEASURES" => return XmlaRequest::MdschemaMeasures,
        "MDSCHEMA_HIERARCHIES" => return XmlaRequest::MdschemaHierarchies,
        "MDSCHEMA_LEVELS" => return XmlaRequest::MdschemaLevels,
        "MDSCHEMA_PROPERTIES" => return XmlaRequest::MdschemaProperties { property_type },
        "MDSCHEMA_MEMBERS" => {
            return XmlaRequest::MdschemaMembers {
                member_unique_name,
                tree_op,
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
        _ => (),
    };

    if is_execute {
        if !statement_text.is_empty() {
            return XmlaRequest::ExecuteStatement(statement_text);
        } else if is_begin_session {
            return XmlaRequest::BeginSession;
        } else {
            return XmlaRequest::ExecuteEmpty;
        }
    }

    XmlaRequest::Unknown
}
