use axum::{
    Router,
    extract::State,
    http::{HeaderMap, HeaderName, StatusCode, header},
    response::IntoResponse,
    routing::post,
};
use clap::{Parser, Subcommand};
use std::io::Write;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tower_http::limit::RequestBodyLimitLayer;

use xmla_proxy::engine::model::{UserContext, resolve_user_context};
use xmla_proxy::parser::{XmlaRequest, parse_xmla};
use xmla_proxy::project::config::ProxyConfig;
use xmla_proxy::*;

#[derive(Clone)]
struct AppState {
    backend_source: backend::BackendSource,
}

// ---- CLI ----

#[derive(Parser)]
#[command(name = "mallardcube", about = "SSAS Tabular proxy for Excel + DuckDB")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the XMLA proxy server (default)
    Serve,
    /// Convert a Tabular Editor export to proxy project
    ConvertTabular {
        /// Path to Tabular Editor source (directory for folder/TMDL format, or .bim file)
        src_dir: String,
        /// Output directory (default: generated_project)
        #[arg(default_value = "generated_project")]
        out_dir: String,
        /// Number of dummy rows for fact tables (default: 10000)
        #[arg(long, default_value = "10000")]
        dummy_rows: usize,
    },
    /// Replay an XMLA trace against the current project
    TraceReplay {
        /// Path to xmla-trace.jsonl
        #[arg(default_value = "xmla-trace.jsonl")]
        trace_path: String,
        /// Path to proxy-config.json
        project: Option<String>,
    },
    /// Extract unique MDX from a trace into Rust constants
    ExtractTrace {
        /// Path to xmla-trace.jsonl
        #[arg(default_value = "xmla-trace.jsonl")]
        path: String,
    },
    /// Concurrently replay captured XMLA requests against a live /xmla endpoint
    LoadReplay {
        /// Arguments forwarded to the load-replay tool
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Build inventory report from a Tabular Editor export
    Inventory {
        /// Path to Tabular Editor source (directory for folder/TMDL format, or .bim file)
        src_dir: String,
    },
    /// Seed DuckDB with generated_project test data
    SeedGeneratedDb,
    /// Emit SQL to create demo fact tables
    SeedSql,
    /// Qualify a converted project for Excel readiness
    Qualify {
        /// Path to proxy-config.json
        #[arg(default_value = "project3/proxy-config.json")]
        config: String,
        /// Optional path to xmla-trace.jsonl for replay validation
        trace: Option<String>,
    },
}

// ---- debug file logging ----

static DEBUG_LOG: Mutex<Option<std::fs::File>> = Mutex::new(None);

fn init_debug_log() {
    let file =
        std::fs::File::create("debug-last-run.log").expect("failed to create debug-last-run.log");
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

// ---- main ----

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => run_server().await,
        Command::ConvertTabular {
            src_dir,
            out_dir,
            dummy_rows,
        } => {
            std::process::exit(xmla_proxy::tools::convert_tabular::run(vec![
                "convert-tabular".into(),
                src_dir,
                out_dir,
                format!("--dummy-rows={}", dummy_rows),
            ]));
        }
        Command::TraceReplay {
            trace_path,
            project,
        } => {
            let mut args = vec!["trace-replay".into(), trace_path];
            if let Some(p) = project {
                args.push(p);
            }
            std::process::exit(xmla_proxy::tools::trace_replay::run(args));
        }
        Command::ExtractTrace { path } => {
            std::process::exit(xmla_proxy::tools::extract_trace_mdx::run(vec![
                "extract-trace".into(),
                path,
            ]));
        }
        Command::LoadReplay { args } => {
            let mut forwarded = vec!["load-replay".into()];
            forwarded.extend(args);
            std::process::exit(xmla_proxy::tools::load_replay::run(forwarded));
        }
        Command::Inventory { src_dir } => {
            std::process::exit(xmla_proxy::tools::inventory::run(vec![
                "inventory".into(),
                src_dir,
            ]));
        }
        Command::SeedGeneratedDb => {
            std::process::exit(xmla_proxy::tools::seed_generated_db::run(vec![
                "seed-generated-db".into(),
            ]));
        }
        Command::SeedSql => {
            std::process::exit(xmla_proxy::tools::seed_sql::run(vec!["seed-sql".into()]));
        }
        Command::Qualify { config, trace } => {
            let mut args = vec!["qualify".into(), config];
            if let Some(t) = trace {
                args.push(t);
            }
            std::process::exit(xmla_proxy::tools::qualify::run(args));
        }
    }
}

async fn run_server() {
    init_debug_log();
    debug_write("===== SSAS-PROXY DEBUG LOG =====");
    xmla_proxy::xmla_trace::init_trace();

    let config_path = std::env::var("PROXY_CONFIG")
        .ok()
        .unwrap_or_else(|| "project3/proxy-config.json".into());
    proxy_project::init_project(Some(&config_path)).expect("init project");

    let state = {
        let p = proxy_project::project();
        println!("📁 Project loaded: {}", p.config.catalog);
        debug_write(&format!(
            "Project loaded: {} | cube={} | {} dims, {} measures",
            p.config.catalog,
            p.config.cube,
            p.model.dimensions.len(),
            p.model.measures.len(),
        ));

        let db_path = proxy_project::resolve_db_path(&config_path, p.config.db_path.as_deref());
        let backend_source = match db_path {
            Some(path) => {
                let source = backend::init_backend_source(Some(&path))
                    .expect(&format!("failed to configure DuckDB: {path}"));
                println!("🗄️  DuckDB: {path}");
                debug_write(&format!("DuckDB: {path}"));
                source
            }
            None => {
                let source =
                    backend::init_backend_source(None).expect("failed to init demo DuckDB");
                println!("🧪 DuckDB: demo ({})", source.path().display());
                debug_write(&format!("DuckDB: demo ({})", source.path().display()));
                source
            }
        };
        std::sync::Arc::new(AppState { backend_source })
    };

    // Warn if roles are defined but no auth config (roles are not enforced).
    {
        let p = proxy_project::project();
        if !p.config.roles.is_empty() && p.config.auth.is_none() {
            println!("⚠️  WARNING: {} role(s) defined but no auth config — security is NOT enforced", p.config.roles.len());
            debug_write(&format!("WARNING: {} role(s) without auth config", p.config.roles.len()));
        }
    }

    if std::env::var("MALLOY_RUNTIME").map_or(false, |v| v == "1") {
        execute_builders::enable_malloy_runtime();
        println!("🧪 Malloy runtime ENABLED (MALLOY_RUNTIME=1)");
        debug_write("Malloy runtime: ENABLED");
        execute_builders::warm_malloy_worker();
    } else {
        println!("📊 Malloy runtime disabled (set MALLOY_RUNTIME=1 to enable)");
        debug_write("Malloy runtime: disabled");
    }

    let bind_addr = std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let app = Router::new()
        .route("/xmla", post(handle_xmla))
        .with_state(state)
        .layer(RequestBodyLimitLayer::new(1_048_576)); // 1 MB
    let addr: SocketAddr = bind_addr
        .parse()
        .expect("invalid BIND_ADDRESS (e.g. 127.0.0.1:8080 or 0.0.0.0:8080)");
    println!("🚀 SSAS Proxy running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// Build a `UserContext` from request headers and proxy config.
///
/// - If `config.auth` is `None` → returns admin default (no auth = full access).
/// - If `auth.trusted_proxy` is true → reads trusted header (default `X-User`).
///   If present, resolves roles; if missing, returns deny-all (fail closed).
/// - If `auth.trusted_proxy` is false → returns admin default.
fn build_user_context(headers: &HeaderMap, config: &ProxyConfig) -> UserContext {
    let Some(auth) = &config.auth else {
        return UserContext::admin_default();
    };
    if auth.trusted_proxy {
        let header_name = HeaderName::from_bytes(auth.trusted_header.as_bytes())
            .unwrap_or_else(|_| HeaderName::from_static("x-user"));
        if let Some(user_id) = headers
            .get(&header_name)
            .and_then(|v| v.to_str().ok())
        {
            resolve_user_context(config, user_id, &[])
        } else {
            // Missing trusted header: deny all (fail closed).
            UserContext::deny_all()
        }
    } else {
        UserContext::admin_default()
    }
}

// ---- HTTP helpers ----

fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "text/xml; charset=utf-8".parse().unwrap(),
    );
    headers.insert(header::SERVER, "SSAS-Proxy/2.0".parse().unwrap());
    headers.insert(header::CONNECTION, "close".parse().unwrap());
    headers.insert(
        HeaderName::from_static("x-transport-caps-negotiation-flags"),
        "0,0,0,0,0".parse().unwrap(),
    );
    headers
}

fn extract_block<'a>(body: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = body.find(open)? + open.len();
    let end = body[start..].find(close)? + start;
    Some(body[start..end].trim())
}

fn log_discover_context(body: &str) {
    if let Some(restrictions) = extract_block(body, "<RestrictionList", "</RestrictionList>") {
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

// ---- XMLA request handler ----

async fn handle_xmla(
    State(state): State<Arc<AppState>>,
    http_headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    let request_type = body.find("<RequestType>").and_then(|start| {
        let after = start + 13;
        body[after..]
            .find("</RequestType>")
            .map(|end| &body[after..after + end])
    });
    if let Some(rt) = request_type {
        println!("🔍 RequestType: {}", rt);
    }

    let config = proxy_project::project().config.clone();
    let user_context = build_user_context(&http_headers, &config);
    if !user_context.is_administrator && !user_context.roles.is_empty() {
        println!("🔐 User '{}' authenticated as roles: {:?}", user_context.user_id, user_context.roles);
    }

    let headers = default_headers();
    let request = parse_xmla(&body);
    println!("📥 Request: {:?}", request);

    log_discover_context(&body);

    if body.contains("<Execute") {
        println!("🔍 Execute body:\n{}", body);
    }

    let request_for_worker = request.clone();
    let body_for_worker = body.clone();
    let user_ctx = user_context.clone();
    let cfg = config.clone();
    let backend_source = state.backend_source.clone();
    let response_body = tokio::task::spawn_blocking(move || {
        let backend = backend_source
            .checkout()
            .expect("failed to checkout DuckDB backend");
        route_request(&request_for_worker, &body_for_worker, &backend, &user_ctx, &cfg)
    })
    .await
    .expect("XMLA worker task panicked");

    if body.contains("MDSCHEMA_MEMBERS") {
        println!(
            "📤 RESPONSE (MdschemaMembers):\n{}",
            &response_body[..response_body.len().min(2000)]
        );
    }

    (StatusCode::OK, headers, response_body)
}

/// Route a parsed XMLA request to the appropriate handler.
fn route_request<B: backend::QueryBackend + ?Sized>(
    request: &XmlaRequest,
    body: &str,
    backend: &B,
    user: &UserContext,
    config: &ProxyConfig,
) -> String {
    match request {
        XmlaRequest::BeginSession | XmlaRequest::ExecuteEmpty => {
            let resp = execute::dispatch::get_empty_execute_response();
            xmla_proxy::xmla_trace::trace_request(
                &format!("{:?}", request),
                body,
                &resp,
                None,
                None,
            );
            resp
        }

        XmlaRequest::DiscoverProperties { property_names } => {
            let resp = if property_names.len() == 1 && property_names[0] == "Catalog" {
                println!("Excel asking for Catalog");
                properties::get_single_property_response(
                    "Catalog",
                    &proxy_project::project().config.catalog,
                )
            } else {
                println!("Excel asking for properties: {:?}", property_names);
                properties::get_properties_response(property_names)
            };
            xmla_proxy::xmla_trace::trace_request(
                &format!("{:?}", request),
                body,
                &resp,
                None,
                None,
            );
            resp
        }

        XmlaRequest::DiscoverSchemaRowsets => {
            let resp = schema_rowsets::get_schemas_response();
            xmla_proxy::xmla_trace::trace_request("DiscoverSchemaRowsets", body, &resp, None, None);
            resp
        }
        XmlaRequest::DbSchemaCatalogs => {
            let resp = catalogs::get_catalogs_response();
            xmla_proxy::xmla_trace::trace_request("DbSchemaCatalogs", body, &resp, None, None);
            resp
        }
        XmlaRequest::MdschemaCubes => {
            let resp = cubes::get_cubes_response();
            xmla_proxy::xmla_trace::trace_request("MdschemaCubes", body, &resp, None, None);
            resp
        }
        XmlaRequest::DbschemaTables => {
            let resp = tables::get_tables_response();
            xmla_proxy::xmla_trace::trace_request("DbschemaTables", body, &resp, None, None);
            resp
        }

        XmlaRequest::MdschemaDimensions => {
            println!("📥 Sending Dimensions to Excel");
            let resp = dimensions::get_dimensions_response();
            xmla_proxy::xmla_trace::trace_request("MdschemaDimensions", body, &resp, None, None);
            resp
        }
        XmlaRequest::MdschemaMeasures => {
            println!("📥 Sending Measures to Excel");
            let resp = measures::get_measures_response();
            xmla_proxy::xmla_trace::trace_request("MdschemaMeasures", body, &resp, None, None);
            resp
        }
        XmlaRequest::MdschemaHierarchies => {
            println!("📥 Hierarchies");
            let resp = hierarchies::get_hierarchies_response();
            xmla_proxy::xmla_trace::trace_request("MdschemaHierarchies", body, &resp, None, None);
            resp
        }
        XmlaRequest::MdschemaLevels => {
            println!("📥 Levels");
            let resp = levels::get_levels_response();
            xmla_proxy::xmla_trace::trace_request("MdschemaLevels", body, &resp, None, None);
            resp
        }

        XmlaRequest::ExecuteStatement(mdx) => {
            println!("📥 MDX: {}", mdx);
            debug_write("===== EXECUTE REQUEST =====");
            debug_write(&format!("MDX: {}", mdx));
            debug_write("REQUEST XML:");
            debug_write(body);
            let (resp, timings) =
                execute_builders::get_execute_cellset_response_timed_malloy_with_backend(
                    mdx, backend, user, config,
                );
            debug_write("RESPONSE XML:");
            debug_write(&resp);
            debug_write(&timings.to_log_line());
            xmla_proxy::xmla_trace::trace_request(
                "ExecuteStatement",
                body,
                &resp,
                Some(mdx),
                Some(&timings),
            );
            resp
        }

        XmlaRequest::MdschemaProperties { property_type } => {
            println!("📥 MDSCHEMA_PROPERTIES (PROPERTY_TYPE={:?})", property_type);
            let resp = mdschema_properties::get_mdschema_properties_response(*property_type);
            xmla_proxy::xmla_trace::trace_request("MdschemaProperties", body, &resp, None, None);
            resp
        }
        XmlaRequest::MdschemaMembers {
            member_unique_name,
            tree_op,
        } => {
            println!(
                "📥 MDSCHEMA_MEMBERS (filter_member={:?}, tree_op={:?})",
                member_unique_name, tree_op
            );
            debug_write("===== MDSCHEMA_MEMBERS REQUEST =====");
            debug_write(&format!(
                "filter_member: {:?}, tree_op: {:?}",
                member_unique_name, tree_op
            ));
            let resp = members::get_members_response_with_backend(
                member_unique_name.as_deref(),
                *tree_op,
                backend,
            );
            debug_write("RESPONSE XML:");
            debug_write(&resp);
            xmla_proxy::xmla_trace::trace_request("MdschemaMembers", body, &resp, None, None);
            resp
        }

        XmlaRequest::DiscoverLiterals => {
            println!("📥 DISCOVER_LITERALS");
            let resp = literals::get_literals_response();
            xmla_proxy::xmla_trace::trace_request("DiscoverLiterals", body, &resp, None, None);
            resp
        }
        XmlaRequest::MdschemaSets => {
            println!("📥 MDSCHEMA_SETS");
            let resp = sets::get_sets_response();
            xmla_proxy::xmla_trace::trace_request("MdschemaSets", body, &resp, None, None);
            resp
        }
        XmlaRequest::MdschemaKpis => {
            println!("📥 MDSCHEMA_KPIS");
            let resp = kpis::get_kpis_response();
            xmla_proxy::xmla_trace::trace_request("MdschemaKpis", body, &resp, None, None);
            resp
        }
        XmlaRequest::MdschemaMeasureGroups => {
            println!("📥 MDSCHEMA_MEASUREGROUPS");
            let resp = measure_groups::get_measure_groups_response();
            xmla_proxy::xmla_trace::trace_request("MdschemaMeasureGroups", body, &resp, None, None);
            resp
        }
        XmlaRequest::MdschemaMeasureGroupDimensions => {
            println!("📥 MDSCHEMA_MEASUREGROUP_DIMENSIONS");
            let resp = measuregroup_dimensions::get_measuregroup_dimensions_response();
            xmla_proxy::xmla_trace::trace_request(
                "MdschemaMeasureGroupDimensions",
                body,
                &resp,
                None,
                None,
            );
            resp
        }

        XmlaRequest::TmschemaModel => {
            println!("📥 TMSCHEMA_MODEL");
            let resp = tmschema::get_tmschema_model_response();
            xmla_proxy::xmla_trace::trace_request("TmschemaModel", body, &resp, None, None);
            resp
        }
        XmlaRequest::TmschemaTables => {
            println!("📥 TMSCHEMA_TABLES");
            let resp = tmschema::get_tmschema_tables_response();
            xmla_proxy::xmla_trace::trace_request("TmschemaTables", body, &resp, None, None);
            resp
        }
        XmlaRequest::TmschemaColumns => {
            println!("📥 TMSCHEMA_COLUMNS");
            let resp = tmschema::get_tmschema_columns_response();
            xmla_proxy::xmla_trace::trace_request("TmschemaColumns", body, &resp, None, None);
            resp
        }
        XmlaRequest::TmschemaMeasures => {
            println!("📥 TMSCHEMA_MEASURES");
            let resp = tmschema::get_tmschema_measures_response();
            xmla_proxy::xmla_trace::trace_request("TmschemaMeasures", body, &resp, None, None);
            resp
        }
        XmlaRequest::TmschemaHierarchies => {
            println!("📥 TMSCHEMA_HIERARCHIES");
            let resp = tmschema::get_tmschema_hierarchies_response();
            xmla_proxy::xmla_trace::trace_request("TmschemaHierarchies", body, &resp, None, None);
            resp
        }
        XmlaRequest::TmschemaLevels => {
            println!("📥 TMSCHEMA_LEVELS");
            let resp = tmschema::get_tmschema_levels_response();
            xmla_proxy::xmla_trace::trace_request("TmschemaLevels", body, &resp, None, None);
            resp
        }
        XmlaRequest::TmschemaRelationships => {
            println!("📥 TMSCHEMA_RELATIONSHIPS");
            let resp = tmschema::get_tmschema_relationships_response();
            xmla_proxy::xmla_trace::trace_request("TmschemaRelationships", body, &resp, None, None);
            resp
        }
        XmlaRequest::TmschemaPartitions => {
            println!("📥 TMSCHEMA_PARTITIONS");
            let resp = tmschema::get_tmschema_partitions_response();
            xmla_proxy::xmla_trace::trace_request("TmschemaPartitions", body, &resp, None, None);
            resp
        }
        XmlaRequest::DiscoverXmlMetadata => {
            println!("📥 DISCOVER_XML_METADATA");
            let resp = tmschema::get_discover_xml_metadata_response();
            xmla_proxy::xmla_trace::trace_request("DiscoverXmlMetadata", body, &resp, None, None);
            resp
        }
        XmlaRequest::DiscoverCalcDependency => {
            println!("📥 DISCOVER_CALC_DEPENDENCY");
            let resp = tmschema::get_discover_calc_dependency_response();
            xmla_proxy::xmla_trace::trace_request(
                "DiscoverCalcDependency",
                body,
                &resp,
                None,
                None,
            );
            resp
        }

        XmlaRequest::Unknown => {
            eprintln!("Unknown request: {}", body);
            xmla_proxy::xmla_trace::trace_request("Unknown", body, "", None, None);
            return String::new();
        }
    }
}
