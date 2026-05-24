use axum::{
    http::{header, HeaderMap, HeaderName, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use std::net::SocketAddr;
use std::sync::Mutex;
use std::io::Write;

use xmla_proxy::parser::{parse_xmla, XmlaRequest};
use xmla_proxy::*;

// --- debug file logging ---

static DEBUG_LOG: Mutex<Option<std::fs::File>> = Mutex::new(None);

fn init_debug_log() {
    let file = std::fs::File::create("debug-last-run.log")
        .expect("failed to create debug-last-run.log");
    *DEBUG_LOG.lock().unwrap() = Some(file);
}

fn debug_write(text: &str) {
    if let Ok(mut guard) = DEBUG_LOG.lock() {
        if let Some(ref mut file) = *guard {
            let _ = writeln!(file, "{}", text);
            let _ = file.flush();
        }
    }
}

// --- main ---

#[tokio::main]
async fn main() {
    init_debug_log();
    debug_write("===== SSAS-PROXY DEBUG LOG =====");

    if std::env::var("MALLOY_RUNTIME").map_or(false, |v| v == "1") {
        execute_builders::enable_malloy_runtime();
        println!("🧪 Malloy runtime ENABLED (MALLOY_RUNTIME=1)");
        debug_write("Malloy runtime: ENABLED");
        execute_builders::warm_malloy_worker();
    } else {
        println!("📊 Malloy runtime disabled (set MALLOY_RUNTIME=1 to enable)");
        debug_write("Malloy runtime: disabled");
    }

    let app = Router::new().route("/xmla", post(handle_xmla));
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("🚀 Rust-XMLA Proxy (v3 - ModuleRefactor) snurrar på http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "text/xml; charset=utf-8".parse().unwrap());
    headers.insert(header::SERVER, "Rust-Malloy-Proxy/2.0".parse().unwrap());
    headers.insert(header::CONNECTION, "close".parse().unwrap());
    headers.insert(
        HeaderName::from_static("x-transport-caps-negotiation-flags"),
        "0,0,0,0,0".parse().unwrap(),
    );
    headers
}

/// Extracts `<open>...</close>` (first occurrence) verbatim from `body`.
/// Returns the trimmed inner contents, or None if either tag is missing.
fn extract_block<'a>(body: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = body.find(open)? + open.len();
    let end = body[start..].find(close)? + start;
    Some(body[start..end].trim())
}

/// Print the `<RestrictionList>` and `<PropertyList>` blocks from a Discover
/// request body, when present. Helps us see what Excel is actually asking for.
fn log_discover_context(body: &str) {
    if let Some(restrictions) = extract_block(body, "<RestrictionList", "</RestrictionList>") {
        // <RestrictionList ...> — strip leading attrs up to the first '>' so we
        // print just the inner XML.
        let inner = match restrictions.find('>') {
            Some(idx) => restrictions[idx + 1..].trim(),
            None => restrictions,
        };
        if !inner.is_empty() {
            println!("🎯 RestrictionList:\n{}", inner);
        } else {
            println!("🎯 RestrictionList: (empty)");
        }
    }
    if let Some(properties) = extract_block(body, "<PropertyList", "</PropertyList>") {
        let inner = match properties.find('>') {
            Some(idx) => properties[idx + 1..].trim(),
            None => properties,
        };
        if !inner.is_empty() {
            println!("⚙️  PropertyList:\n{}", inner);
        }
    }
}

async fn handle_xmla(body: String) -> impl IntoResponse {
    if body.contains("<RequestType>") {
        let req_start = body.find("<RequestType>").unwrap() + 13;
        let req_end = body.find("</RequestType>").unwrap();
        println!("🔍 Rå RequestType från Excel: {}", &body[req_start..req_end]);
    }

    let headers = default_headers();
    let request = parse_xmla(&body);
    println!("📥 Fick anrop, tolkade som: {:?}", request);

    log_discover_context(&body);

    if body.contains("<Execute") {
        println!("🔍 Rå Execute från Excel:\n{}", body);
    }

    let response_body = match request {
        XmlaRequest::BeginSession | XmlaRequest::ExecuteEmpty => {
            execute::get_empty_execute_response()
        }

        XmlaRequest::DiscoverProperties { property_names } => {
            if property_names.len() == 1 && property_names[0] == "Catalog" {
                println!("Excel frågar efter Catalog");
                properties::get_single_property_response("Catalog", "KTH_KEX_MALLOY_CUBE")
            } else {
                println!("Excel frågar efter egenskaper: {:?}", property_names);
                properties::get_properties_response(&property_names)
            }
        }

        XmlaRequest::DiscoverSchemaRowsets => schema_rowsets::get_schemas_response(),
        XmlaRequest::DbSchemaCatalogs => catalogs::get_catalogs_response(),
        XmlaRequest::MdschemaCubes => cubes::get_cubes_response(),
        XmlaRequest::DbschemaTables => tables::get_tables_response(),
        XmlaRequest::MdschemaDimensions => {
            println!("📥 Skickar Dimensioner till Excel!");
            dimensions::get_dimensions_response()
        }
        XmlaRequest::MdschemaMeasures => {
            println!("📥 Skickar Measures till Excel!");
            measures::get_measures_response()
        }
        XmlaRequest::MdschemaHierarchies => {
            println!("📥 Hierarchies");
            hierarchies::get_hierarchies_response()
        }
        XmlaRequest::MdschemaLevels => {
            println!("📥 Levels");
            levels::get_levels_response()
        }
        XmlaRequest::ExecuteStatement(mdx) => {
            println!("📥 MDX Statement: {}", mdx);
            debug_write(&format!("===== EXECUTE REQUEST ====="));
            debug_write(&format!("MDX: {}", mdx));
            debug_write("REQUEST XML:");
            debug_write(&body);
            // Use instrumented path for timing collection
            let (resp, timings) = execute_builders::get_execute_cellset_response_timed_malloy(&mdx);
            debug_write("RESPONSE XML:");
            debug_write(&resp);
            debug_write(&timings.to_log_line());
            resp
        }
        XmlaRequest::MdschemaProperties { property_type } => {
            println!("📥 MDSCHEMA_PROPERTIES (PROPERTY_TYPE={:?})", property_type);
            mdschema_properties::get_mdschema_properties_response(property_type)
        }
        XmlaRequest::MdschemaMembers { member_unique_name, tree_op } => {
            println!("📥 MDSCHEMA_MEMBERS (filter_member={:?}, tree_op={:?})", member_unique_name, tree_op);
            debug_write(&format!("===== MDSCHEMA_MEMBERS REQUEST ====="));
            debug_write(&format!("filter_member: {:?}, tree_op: {:?}", member_unique_name, tree_op));
            let resp = members::get_members_response(member_unique_name.as_deref(), tree_op);
            debug_write("RESPONSE XML:");
            debug_write(&resp);
            resp
        }
        XmlaRequest::DiscoverLiterals => {
            println!("📥 DISCOVER_LITERALS");
            literals::get_literals_response()
        }
        XmlaRequest::MdschemaSets => {
            println!("📥 MDSCHEMA_SETS");
            sets::get_sets_response()
        }
        XmlaRequest::MdschemaKpis => {
            println!("📥 MDSCHEMA_KPIS");
            kpis::get_kpis_response()
        }
        XmlaRequest::MdschemaMeasureGroups => {
            println!("📥 MDSCHEMA_MEASUREGROUPS");
            measure_groups::get_measure_groups_response()
        }
        XmlaRequest::MdschemaMeasureGroupDimensions => {
            println!("📥 MDSCHEMA_MEASUREGROUP_DIMENSIONS");
            measuregroup_dimensions::get_measuregroup_dimensions_response()
        }

        XmlaRequest::TmschemaModel => {
            println!("📥 TMSCHEMA_MODEL");
            tmschema::get_tmschema_model_response()
        }
        XmlaRequest::TmschemaTables => {
            println!("📥 TMSCHEMA_TABLES");
            tmschema::get_tmschema_tables_response()
        }
        XmlaRequest::TmschemaColumns => {
            println!("📥 TMSCHEMA_COLUMNS");
            tmschema::get_tmschema_columns_response()
        }
        XmlaRequest::TmschemaMeasures => {
            println!("📥 TMSCHEMA_MEASURES");
            tmschema::get_tmschema_measures_response()
        }
        XmlaRequest::TmschemaHierarchies => {
            println!("📥 TMSCHEMA_HIERARCHIES");
            tmschema::get_tmschema_hierarchies_response()
        }
        XmlaRequest::TmschemaLevels => {
            println!("📥 TMSCHEMA_LEVELS");
            tmschema::get_tmschema_levels_response()
        }
        XmlaRequest::TmschemaRelationships => {
            println!("📥 TMSCHEMA_RELATIONSHIPS");
            tmschema::get_tmschema_relationships_response()
        }
        XmlaRequest::TmschemaPartitions => {
            println!("📥 TMSCHEMA_PARTITIONS");
            tmschema::get_tmschema_partitions_response()
        }
        XmlaRequest::DiscoverXmlMetadata => {
            println!("📥 DISCOVER_XML_METADATA");
            tmschema::get_discover_xml_metadata_response()
        }
        XmlaRequest::DiscoverCalcDependency => {
            println!("📥 DISCOVER_CALC_DEPENDENCY");
            tmschema::get_discover_calc_dependency_response()
        }

        XmlaRequest::Unknown => {
            println!("❌ Okänt anrop.");
            return (StatusCode::BAD_REQUEST, headers, "Okänt anrop".to_string());
        }
    };

    if body.contains("MDSCHEMA_MEMBERS") {
        println!("📤 RESPONSE (MdschemaMembers):\n{}", &response_body[..response_body.len().min(2000)]);
    }

    (StatusCode::OK, headers, response_body)
}
