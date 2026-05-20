use axum::{
    http::{header, HeaderMap, HeaderName, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use std::net::SocketAddr;

mod parser;
mod response;
mod properties;
mod schema_rowsets;
mod catalogs;
mod cubes;
mod tables;
mod dimensions;
mod measures;
mod hierarchies;
mod levels;
mod mdschema_properties;
mod members;
mod literals;
mod sets;
mod kpis;
mod measure_groups;
mod measuregroup_dimensions;
mod execute;

use parser::{parse_xmla, XmlaRequest};

#[tokio::main]
async fn main() {
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

async fn handle_xmla(body: String) -> impl IntoResponse {
    if body.contains("<RequestType>") {
        let req_start = body.find("<RequestType>").unwrap() + 13;
        let req_end = body.find("</RequestType>").unwrap();
        println!("🔍 Rå RequestType från Excel: {}", &body[req_start..req_end]);
    }

    let headers = default_headers();
    let request = parse_xmla(&body);
    println!("📥 Fick anrop, tolkade som: {:?}", request);

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
            execute::get_execute_statement_response(&mdx)
        }
        XmlaRequest::MdschemaProperties { property_type } => {
            println!("📥 MDSCHEMA_PROPERTIES (PROPERTY_TYPE={:?})", property_type);
            mdschema_properties::get_mdschema_properties_response(property_type)
        }
        XmlaRequest::MdschemaMembers => {
            println!("📥 MDSCHEMA_MEMBERS");
            members::get_members_response()
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

        XmlaRequest::Unknown => {
            println!("❌ Okänt anrop.");
            return (StatusCode::BAD_REQUEST, headers, "Okänt anrop".to_string());
        }
    };

    (StatusCode::OK, headers, response_body)
}
