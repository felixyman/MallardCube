use axum::{
    Router,
    extract::State,
    http::{HeaderMap, HeaderName, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use clap::{Parser, Subcommand};
use std::io::Write;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tower_http::limit::RequestBodyLimitLayer;

use mallardcube::engine::model::{UserContext, resolve_user_context};
use mallardcube::parser::{XmlaRequest, parse_xmla};
use mallardcube::project::config::ProxyConfig;
use mallardcube::*;

/// A response that either exists in full or streams in chunks (plan 051-C).
enum XmlaBody {
    Full(String),
    /// Member rowset streamed chunk by chunk — the one response shape that can
    /// legitimately reach hundreds of megabytes.
    Members(mallardcube::xmla::discover::members::MemberResponse),
}

impl XmlaBody {
    /// Exact size for full bodies, the pre-computed estimate for streamed ones
    /// (the byte budget must trip before the first chunk is written).
    fn estimated_bytes(&self) -> usize {
        match self {
            XmlaBody::Full(text) => text.len(),
            XmlaBody::Members(response) => match response {
                mallardcube::xmla::discover::members::MemberResponse::Fault(message) => {
                    message.len()
                }
                mallardcube::xmla::discover::members::MemberResponse::Rowset(rowset) => {
                    rowset.estimated_bytes()
                }
            },
        }
    }

    fn into_body(self) -> axum::body::Body {
        use mallardcube::xmla::discover::members::MemberResponse;
        match self {
            XmlaBody::Full(text) => axum::body::Body::from(text),
            XmlaBody::Members(MemberResponse::Fault(message)) => {
                axum::body::Body::from(mallardcube::xmla::response::fault_response(&message))
            }
            XmlaBody::Members(MemberResponse::Rowset(rowset)) => {
                axum::body::Body::from_stream(rowset)
            }
        }
    }
}

struct AppState {
    /// Swappable so a reload can replace the pool without dropping in-flight
    /// requests (plan 041 phase C). Clone the `Arc` per request — the pool's
    /// round-robin counter lives inside it.
    backend_source: std::sync::RwLock<std::sync::Arc<backend::BackendSource>>,
    /// Catalog/cube/data-freshness facts for `GET /status`; the data stamp is
    /// updated on reload.
    status: std::sync::RwLock<mallardcube::status::StatusInfo>,
    /// Heavy-query slots (plan 051-B): every XMLA request holds one permit for
    /// its duration, which bounds engine concurrency — and with it, the memory
    /// the divided engine ceiling can actually add up to.
    query_slots: Arc<tokio::sync::Semaphore>,
}

/// Snapshot the current backend source (cheap `Arc` clone). A poisoned lock
/// must not take the server down, so recover the guard.
fn current_source(state: &AppState) -> std::sync::Arc<backend::BackendSource> {
    match state.backend_source.read() {
        Ok(slot) => slot.clone(),
        Err(e) => e.into_inner().clone(),
    }
}

fn current_status(state: &AppState) -> mallardcube::status::StatusInfo {
    let mut status = match state.status.read() {
        Ok(slot) => slot.clone(),
        Err(e) => e.into_inner().clone(),
    };
    // Cache accounting is live, not the value captured at startup.
    status.cache = mallardcube::execute::cache::RESULT_CACHE.stats();
    status
}

/// Reopen the data file and swap the pool: in-flight requests finish on the
/// old pool, new requests use the new one, and the result cache is cleared so
/// no request can serve pre-reload rows. Returns an error message on failure
/// (the old pool keeps serving).
fn reload_data(state: &AppState) -> Result<String, String> {
    let path = current_source(state).path().to_path_buf();

    // Rollups cannot be rebuilt while the live pool holds the sidecar
    // read-only; if the data changed, disable them (correct, slower) and
    // rebuild on the next restart.
    let aggregations_disabled = mallardcube::reload::disable_stale_aggregations(&path);

    let new_source = backend::BackendSource::file(&path).map_err(|e| e.to_string())?;
    let stamp = mallardcube::status::DataStamp::capture(new_source.path());
    let (size_bytes, mtime_unix) = (stamp.size_bytes, stamp.mtime_unix);
    match state.backend_source.write() {
        Ok(mut slot) => *slot = std::sync::Arc::new(new_source),
        Err(e) => *e.into_inner() = std::sync::Arc::new(new_source),
    }
    let mut status = current_status(state);
    status.data = stamp;
    status.data_epoch = mallardcube::execute::cache::bump_data_epoch();
    match state.status.write() {
        Ok(mut slot) => *slot = status,
        Err(e) => *e.into_inner() = status,
    }
    mallardcube::execute::cache::RESULT_CACHE.clear();
    // Member dictionaries depend on the data too (plan 031).
    mallardcube::proxy_project::project()
        .model
        .dim_cache
        .clear();

    let mut note = format!(
        "{} ({} bytes, modified {} unix)",
        path.display(),
        size_bytes,
        mtime_unix
    );
    if aggregations_disabled {
        note.push_str(" — aggregations disabled until restart (data changed)");
    }
    Ok(note)
}

// ---- CLI ----

#[derive(Parser)]
#[command(name = "mallard", about = "SSAS Tabular proxy for Excel + DuckDB")]
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
        /// Output directory (default: converted-project)
        #[arg(default_value = "converted-project")]
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
    /// Format a proxy-config canonically (JSON or YAML)
    Fmt {
        /// Config file to format
        config: String,
        /// Exit non-zero when the file is not canonical (for CI)
        #[arg(long)]
        check: bool,
        /// Print the effective config in this format instead of rewriting (yaml|json)
        #[arg(long, value_name = "FORMAT")]
        to: Option<String>,
    },
    /// Emit SQL to create demo fact tables
    SeedSql,
    /// Auto-detect a semantic model from a DuckDB database
    AutoModel {
        /// Path to the DuckDB database file
        db_path: String,
        /// Output directory for proxy-config.json (default: current dir)
        #[arg(long)]
        output: Option<String>,
        /// Override the auto-detected fact table
        #[arg(long)]
        fact: Option<String>,
    },
    /// Qualify a converted project for Excel readiness
    Qualify {
        /// Path to proxy-config.json
        #[arg(default_value = "projects/project3/proxy-config.json")]
        config: String,
        /// Optional path to xmla-trace.jsonl for replay validation
        trace: Option<String>,
        /// Fail when the proxy carries semantic-layer logic (fallback SQL, untranslated DAX)
        #[arg(long)]
        strict: bool,
    },
}

// ---- debug file logging ----

static DEBUG_LOG: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// Human-readable debug logging is opt-in (`MALLARDCUBE_DEBUG=1`). It writes
/// full request/response bodies to `debug-last-run.log` and flushes per call,
/// which serializes every request on the log file — a hard bottleneck under
/// concurrency, so it must stay off by default.
fn debug_log_enabled() -> bool {
    std::env::var("MALLARDCUBE_DEBUG").is_ok_and(|v| v == "1")
}

fn init_debug_log() {
    if !debug_log_enabled() {
        return;
    }
    let file =
        std::fs::File::create("debug-last-run.log").expect("failed to create debug-last-run.log");
    *DEBUG_LOG.lock().unwrap_or_else(|e| e.into_inner()) = Some(file);
}

fn debug_write(text: &str) {
    if !debug_log_enabled() {
        return;
    }
    if let Some(ref mut file) = *DEBUG_LOG.lock().unwrap_or_else(|e| e.into_inner()) {
        let _ = writeln!(file, "{}", text);
        let _ = file.flush();
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
            std::process::exit(mallardcube::tools::convert_tabular::run(vec![
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
            std::process::exit(mallardcube::tools::trace_replay::run(args));
        }
        Command::ExtractTrace { path } => {
            std::process::exit(mallardcube::tools::extract_trace_mdx::run(vec![
                "extract-trace".into(),
                path,
            ]));
        }
        Command::LoadReplay { args } => {
            let mut forwarded = vec!["load-replay".into()];
            forwarded.extend(args);
            std::process::exit(mallardcube::tools::load_replay::run(forwarded));
        }
        Command::Inventory { src_dir } => {
            std::process::exit(mallardcube::tools::inventory::run(vec![
                "inventory".into(),
                src_dir,
            ]));
        }
        Command::Fmt { config, check, to } => {
            let mut args = vec!["fmt".to_string()];
            if check {
                args.push("--check".into());
            }
            if let Some(to) = to {
                args.push("--to".into());
                args.push(to);
            }
            args.push(config);
            std::process::exit(mallardcube::tools::fmt::run(args));
        }
        Command::SeedSql => {
            std::process::exit(mallardcube::tools::seed_sql::run(vec!["seed-sql".into()]));
        }
        Command::AutoModel {
            db_path,
            output,
            fact,
        } => {
            let mut args = vec!["auto-model".into(), db_path];
            if let Some(o) = output {
                args.push("--output".into());
                args.push(o);
            }
            if let Some(f) = fact {
                args.push("--fact".into());
                args.push(f);
            }
            std::process::exit(mallardcube::tools::auto_model::run(args));
        }
        Command::Qualify {
            config,
            trace,
            strict,
        } => {
            let mut args = vec!["qualify".into(), config];
            if let Some(t) = trace {
                args.push(t);
            }
            if strict {
                args.push("--strict".into());
            }
            std::process::exit(mallardcube::tools::qualify::run(args));
        }
    }
}

/// Log every Rust panic — message, location, and a forced backtrace — to
/// stderr and to `mallard-crash.log`, so a crash leaves a forensic trail even
/// when the operator's terminal isn't capturing output.
fn install_panic_diagnostics() {
    std::panic::set_hook(Box::new(|info| {
        let bt = std::backtrace::Backtrace::force_capture();
        let thread = std::thread::current();
        let msg = format!(
            "PANIC on thread {:?}: {info}\nbacktrace:\n{bt}\n",
            thread.name().unwrap_or("<unnamed>")
        );
        eprintln!("!!! {msg}");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("mallard-crash.log")
        {
            use std::io::Write;
            let _ = f.write_all(msg.as_bytes());
        }
    }));
}

/// Exit with an actionable message when the DuckDB file cannot be opened.
///
/// The common case is a lock conflict: DuckDB allows one writer or many
/// readers, never both, so a load job (or another server) holding the file
/// makes startup fail. Say that plainly instead of a bare panic.
fn fatal_db_open_error(path: &str, err: &duckdb::Error) -> ! {
    let msg = err.to_string();
    eprintln!("❌ Could not open DuckDB: {path}");
    if msg.contains("Conflicting lock") || msg.contains("Could not set lock") {
        eprintln!(
            "   Another process holds the database file (a load job, or another\n   \
             MallardCube). DuckDB allows one writer or many readers — never both.\n   \
             Stop the writer and start again, or refresh via the staging-file +\n   \
             rename runbook (README → \"Refreshing data\")."
        );
    }
    eprintln!("   {msg}");
    std::process::exit(1);
}

/// `MALLARDCUBE_RELOAD_WATCH=<secs>`: poll the data file's size+mtime stamp and
/// reload when it changes. Covers platforms without SIGHUP and loaders that
/// cannot signal the process.
fn watch_interval() -> Option<u64> {
    std::env::var("MALLARDCUBE_RELOAD_WATCH")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|secs| *secs > 0)
}

async fn run_server() {
    install_panic_diagnostics();
    init_debug_log();
    debug_write("===== SSAS-PROXY DEBUG LOG =====");
    mallardcube::xmla_trace::init_trace();

    let config_path = std::env::var("PROXY_CONFIG").ok();
    let auto_db = std::env::var("MALLARDCUBE_DB").ok();

    match (config_path, auto_db) {
        (Some(path), _) => {
            proxy_project::init_project_for_serving(Some(&path)).expect("init project");
        }
        (None, Some(db)) => {
            // Zero-config AutoModel: detect a semantic model from the DuckDB
            // file, seeding date_dim tables in place.
            let abs_db = std::fs::canonicalize(&db).unwrap_or_else(|_| db.into());
            let fact_override = std::env::var("MALLARDCUBE_FACT").ok();
            let detected = mallardcube::tools::auto_model::detect_config(
                abs_db.to_string_lossy().as_ref(),
                fact_override.as_deref(),
                true,
            )
            .expect("AutoModel detection failed");
            let dir = abs_db.parent().unwrap_or(std::path::Path::new("."));
            proxy_project::init_project_with_config_for_serving(detected.config, dir)
                .expect("init project");
        }
        (None, None) => {
            proxy_project::init_project_for_serving(Some("projects/project3/proxy-config.json"))
                .expect("init project");
        }
    }

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

        let config_dir = std::env::var("PROXY_CONFIG")
            .ok()
            .or_else(|| std::env::var("MALLARDCUBE_DB").ok())
            .unwrap_or_else(|| "projects/project3/proxy-config.json".into());
        let db_path = proxy_project::resolve_db_path(&config_dir, p.config.db_path.as_deref());

        // Build the aggregation sidecar (rollups) before opening the read-only
        // pool, so each pooled connection can ATTACH it. The user's database is
        // only read during the build; the rollups land in the sidecar file.
        if let (Some(agg_cache), Some(db)) = (
            std::env::var("MALLARDCUBE_AGG_CACHE").ok(),
            db_path.as_deref(),
        ) {
            match mallardcube::engine::aggregate::ensure_aggregations(db, &agg_cache, &p.model) {
                Ok(()) => {
                    println!("📊 Aggregations: {agg_cache}");
                    debug_write(&format!("Aggregations: {agg_cache}"));
                }
                Err(e) => {
                    // Serve without aggregations (queries fall back to the fact).
                    eprintln!("⚠️  Aggregations disabled (build failed): {e}");
                    debug_write(&format!("Aggregations disabled (build failed): {e}"));
                }
            }
        }

        let backend_source = match db_path {
            Some(path) => {
                let source = match backend::init_backend_source(Some(&path)) {
                    Ok(source) => source,
                    Err(e) => fatal_db_open_error(&path, &e),
                };
                println!("🗄️  DuckDB: {path}");
                debug_write(&format!("DuckDB: {path}"));
                source
            }
            None => {
                let source = match backend::init_backend_source(None) {
                    Ok(source) => source,
                    Err(e) => fatal_db_open_error("<demo>", &e),
                };
                println!("🧪 DuckDB: demo ({})", source.path().display());
                debug_write(&format!("DuckDB: demo ({})", source.path().display()));
                source
            }
        };
        let status = mallardcube::status::StatusInfo {
            catalog: p.config.catalog.clone(),
            cube: p.config.cube.clone(),
            pool_size: backend::pool_size(),
            started_at_unix: mallardcube::status::now_unix(),
            data: mallardcube::status::DataStamp::capture(backend_source.path()),
            data_epoch: mallardcube::execute::cache::bump_data_epoch(),
            result_cache: mallardcube::execute::cache::enabled(),
            cache: mallardcube::execute::cache::RESULT_CACHE.stats(),
            auth: mallardcube::status::AuthStatus::from_config(&p.config),
            engine: mallardcube::engine::settings::effective(),
        };
        println!(
            "📅 Data stamp: {} ({} bytes, modified {} unix)",
            status.data.path, status.data.size_bytes, status.data.mtime_unix
        );
        if let Some(limit) = &status.engine.memory_limit {
            println!(
                "🧠 Engine memory limit: {} (source: {})",
                limit.value.display(),
                limit.source.as_str()
            );
        }
        let query_slots = mallardcube::engine::settings::query_slots();
        println!(
            "🚦 Query slots: {query_slots} concurrent request(s); timeout: {}",
            match mallardcube::engine::settings::timeout() {
                Some(t) => format!("{}s", t.as_secs()),
                None => "off".to_string(),
            }
        );
        std::sync::Arc::new(AppState {
            backend_source: std::sync::RwLock::new(std::sync::Arc::new(backend_source)),
            status: std::sync::RwLock::new(status),
            query_slots: Arc::new(tokio::sync::Semaphore::new(query_slots)),
        })
    };

    // Warn about declared restrictions we cannot honour: a DAX filter with no
    // SQL translation hides its table (fail closed), which is safe but worth
    // saying out loud at startup.
    {
        let p = proxy_project::project();
        let dax_only = p
            .config
            .roles
            .iter()
            .flat_map(|role| role.table_permissions.iter())
            .filter(|tp| tp.dax_filter.is_some() && tp.filter_expression.trim().is_empty())
            .count();
        if dax_only > 0 {
            println!(
                "⚠️  WARNING: {dax_only} table permission(s) declare a DAX filter this proxy \
                 cannot lower to SQL — those tables are hidden for the role (fail closed)"
            );
        }
    }

    // Warn if roles are defined but no auth config (roles are not enforced).
    {
        let p = proxy_project::project();
        if !p.config.roles.is_empty() && p.config.auth.is_none() {
            println!(
                "⚠️  WARNING: {} role(s) defined but no auth config — security is NOT enforced",
                p.config.roles.len()
            );
            debug_write(&format!(
                "WARNING: {} role(s) without auth config",
                p.config.roles.len()
            ));
        }
    }

    // ---- reload triggers (plan 041 phase C) ----

    // SIGHUP: `systemctl reload mallard` or `kill -HUP <pid>`.
    #[cfg(unix)]
    {
        let reload_state = state.clone();
        tokio::spawn(async move {
            let Ok(mut hangup) =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
            else {
                return;
            };
            while hangup.recv().await.is_some() {
                println!("🔄 SIGHUP received — reloading data");
                match reload_data(&reload_state) {
                    Ok(note) => println!("🔄 Reloaded {note}"),
                    Err(e) => eprintln!("❌ Reload failed: {e}"),
                }
            }
        });
    }

    // Stamp watcher: works where SIGHUP does not (Windows, or a loader that
    // cannot signal the process).
    if let Some(secs) = watch_interval() {
        let watch_state = state.clone();
        tokio::spawn(async move {
            let path = current_source(&watch_state).path().to_path_buf();
            let mut last = mallardcube::reload::file_stamp(&path);
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
                let current = mallardcube::reload::file_stamp(&path);
                if current != last {
                    println!("🔄 Data file changed — reloading");
                    match reload_data(&watch_state) {
                        Ok(note) => {
                            println!("🔄 Reloaded {note}");
                            last = current;
                        }
                        Err(e) => eprintln!("❌ Reload failed: {e}"),
                    }
                }
            }
        });
    }

    let bind_addr = std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let app = Router::new()
        .route("/xmla", post(handle_xmla))
        .route("/health", get(health))
        .route("/status", get(status))
        .with_state(state)
        .layer(RequestBodyLimitLayer::new(1_048_576)); // 1 MB
    let addr: SocketAddr = bind_addr
        .parse()
        .expect("invalid BIND_ADDRESS (e.g. 127.0.0.1:8080 or 0.0.0.0:8080)");
    // An unauthenticated non-loopback bind is an administrator for anyone who
    // can reach the port; refuse unless the operator opts in explicitly
    // (plan 058).
    let auth_status =
        mallardcube::status::AuthStatus::from_config(&proxy_project::project().config);
    let allow_anonymous =
        std::env::var("MALLARDCUBE_ALLOW_ANONYMOUS").is_ok_and(|value| value == "1");
    if let Err(message) =
        mallardcube::auth::bind_decision(addr, auth_status.configured, allow_anonymous)
    {
        eprintln!("❌ {message}");
        std::process::exit(1);
    }
    if !addr.ip().is_loopback() && !auth_status.configured {
        println!(
            "⚠️  Serving without authentication on {addr}: every client is an administrator \
             (MALLARDCUBE_ALLOW_ANONYMOUS=1)."
        );
    }
    println!("🚀 SSAS Proxy running on http://{}", addr);

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            eprintln!(
                "❌ Cannot bind {}: address already in use.\n   Another mallardcube instance is probably still running — stop it first, or set BIND_ADDRESS to a different port.",
                addr
            );
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("❌ Cannot bind {addr}: {e}");
            std::process::exit(1);
        }
    };
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
    // OIDC (JWT Bearer): validate the token and resolve roles from its claims.
    // Fails closed — no/missing/invalid token means deny-all.
    if let Some(oidc) = &auth.oidc {
        let token = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "));
        return match token {
            Some(t) => {
                match mallardcube::auth::validate_and_resolve(t, oidc, mallardcube::auth::cache()) {
                    Ok(identity) => {
                        resolve_user_context(config, &identity.user_id, &identity.groups)
                    }
                    Err(e) => {
                        eprintln!("⚠️  OIDC authentication failed: {e}");
                        UserContext::deny_all()
                    }
                }
            }
            None => {
                eprintln!("⚠️  OIDC configured but no Bearer token present — denying");
                UserContext::deny_all()
            }
        };
    }
    if auth.trusted_proxy {
        let header_name = HeaderName::from_bytes(auth.trusted_header.as_bytes())
            .unwrap_or_else(|_| HeaderName::from_static("x-user"));
        if let Some(user_id) = headers.get(&header_name).and_then(|v| v.to_str().ok()) {
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
    headers.insert(
        HeaderName::from_static("x-transport-caps-negotiation-flags"),
        "0,0,0,0,0".parse().unwrap(),
    );
    headers.insert(
        HeaderName::from_static("persistent-auth"),
        "true".parse().unwrap(),
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

// ---- operational endpoints (plan 041) ----

/// Liveness probe: 200 whenever the process is serving.
async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok\n")
}

/// Freshness/ops payload. Auth-gated when `auth` is configured: the trusted
/// header (or bearer token) must resolve to an identity, or the request is
/// denied — the payload exposes the data path.
async fn status(State(state): State<Arc<AppState>>, headers: HeaderMap) -> impl IntoResponse {
    let config = proxy_project::project().config.clone();
    let user = build_user_context(&headers, &config);
    if !mallardcube::status::authenticated(&user) {
        return (StatusCode::UNAUTHORIZED, "unauthorized\n".to_string());
    }
    (StatusCode::OK, current_status(&state).to_json())
}

// ---- XMLA request handler ----

async fn handle_xmla(
    State(state): State<Arc<AppState>>,
    http_headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    // Raw-request journal, written before anything can crash: if the process
    // dies natively (DuckDB FFI, stack overflow), the last entry here is the
    // request that killed it — the XMLA trace never sees those.
    if std::env::var_os("MALLARD_REQUEST_JOURNAL").is_some()
        && let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("mallard-last-requests.log")
    {
        use std::io::Write;
        let _ = writeln!(
            f,
            "===== REQUEST {:?} =====",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        );
        let _ = f.write_all(body.as_bytes());
        let _ = writeln!(f);
    }
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
        println!(
            "🔐 User '{}' authenticated as roles: {:?}",
            user_context.user_id, user_context.roles
        );
    }

    let headers = default_headers();
    let request = parse_xmla(&body);
    println!("📥 Request: {:?}", request);

    log_discover_context(&body);

    if body.contains("<Execute") {
        println!("🔍 Execute body:\n{}", body);
    }

    // Wait for a query slot before starting the clock: queueing is cheap and
    // bounded, running everything at once is what the slot count prevents.
    let waited = std::time::Instant::now();
    let permit = match state.query_slots.clone().acquire_owned().await {
        Ok(permit) => permit,
        Err(_) => {
            return (
                StatusCode::OK,
                headers,
                axum::body::Body::from(mallardcube::xmla::response::fault_response(
                    "Server is shutting down",
                )),
            );
        }
    };
    if waited.elapsed() > std::time::Duration::from_millis(50) {
        println!("⏳ waited {:?} for a query slot", waited.elapsed());
    }
    // One checkout for the whole request: the worker runs on it and the
    // timeout path interrupts the same connection.
    let request_backend = current_source(&state).checkout();
    let interrupt_target = request_backend.clone();

    let request_for_worker = request.clone();
    let body_for_worker = body.clone();
    let user_ctx = user_context.clone();
    let cfg = config.clone();
    let task = tokio::task::spawn_blocking(move || {
        // The permit is held for the whole request (released on return).
        let _permit = permit;
        mallardcube::xmla_trace::mark_request_start();
        // The session id comes from an XMLA `Session`/`EndSession` element
        // only — a foreign header entry named `Session` is ignored, exactly as
        // the reference does — and it is echoed as an escaped attribute rather
        // than filtered, so well-formed ids survive (plan 055 review). The
        // proxy is stateless: the id is not checked for existence.
        let session_id = mallardcube::xmla::parser::session_id(&body_for_worker);
        mallardcube::response::set_session_id(session_id);
        let backend = request_backend;
        // A panic in request handling must not take the whole server down:
        // log it (see install_panic_diagnostics) and answer with a SOAP fault
        // so the client sees an error instead of a dead connection.
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            route_request(
                &request_for_worker,
                &body_for_worker,
                backend.as_ref(),
                &user_ctx,
                &cfg,
            )
        })) {
            Ok(resp) => {
                // Last-resort size guard covering every response shape; the
                // member/cell caps do the work before this can trigger.
                match mallardcube::engine::settings::budget().bytes_exceeded(resp.estimated_bytes())
                {
                    Some(message) => {
                        eprintln!("!!! response size limit: {message}");
                        XmlaBody::Full(mallardcube::xmla::response::fault_response(&message))
                    }
                    None => resp,
                }
            }
            Err(payload) => {
                let msg = payload
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "unknown panic".to_string());
                eprintln!("!!! XMLA request handling panicked: {msg}");
                mallardcube::xmla_trace::trace_request(
                    "RequestPanic",
                    &body_for_worker,
                    &msg,
                    None,
                    None,
                );
                XmlaBody::Full(mallardcube::xmla::response::fault_response(&format!(
                    "Internal error: {msg}"
                )))
            }
        }
    });
    let response_body = match mallardcube::engine::settings::timeout() {
        None => task.await.expect("XMLA worker task panicked"),
        Some(timeout) => match tokio::time::timeout(timeout, task).await {
            Ok(Ok(resp)) => resp,
            Ok(Err(join_err)) => {
                eprintln!("!!! XMLA worker task failed: {join_err}");
                XmlaBody::Full(mallardcube::xmla::response::fault_response(&format!(
                    "Internal error: {join_err}"
                )))
            }
            Err(_elapsed) => {
                // Abort the engine query on the connection the worker holds;
                // DuckDB stops at its next interruption point and the worker
                // task unwinds into a fault of its own, which we drop.
                interrupt_target.interrupt();
                eprintln!(
                    "!!! query exceeded the {}s timeout; interrupted",
                    timeout.as_secs()
                );
                mallardcube::xmla_trace::trace_request(
                    "QueryTimeout",
                    &body,
                    "query cancelled",
                    None,
                    None,
                );
                XmlaBody::Full(mallardcube::xmla::response::fault_response(&format!(
                    "Query exceeded the {} second timeout and was cancelled",
                    timeout.as_secs()
                )))
            }
        },
    };

    if body.contains("MDSCHEMA_MEMBERS") {
        println!("📤 RESPONSE (MdschemaMembers): streamed in chunks");
    }

    (StatusCode::OK, headers, response_body.into_body())
}

/// Route a parsed XMLA request to the appropriate handler.
/// Route a parsed XMLA request. `MDSCHEMA_MEMBERS` returns a streamed body;
/// everything else is built in full by [`route_full`].
fn route_request<B: backend::QueryBackend + ?Sized>(
    request: &XmlaRequest,
    body: &str,
    backend: &B,
    user: &UserContext,
    config: &ProxyConfig,
) -> XmlaBody {
    if let XmlaRequest::MdschemaMembers {
        member_unique_name,
        tree_op,
        restrictions,
    } = request
    {
        println!(
            "📥 MDSCHEMA_MEMBERS (filter_member={:?}, tree_op={:?}, hier={:?}, level={:?})",
            member_unique_name,
            tree_op,
            restrictions.hierarchy_unique_name,
            restrictions.level_unique_name
        );
        debug_write("===== MDSCHEMA_MEMBERS REQUEST =====");
        debug_write(&format!(
            "filter_member: {:?}, tree_op: {:?}, restrictions: {:?}",
            member_unique_name, tree_op, restrictions
        ));
        let response = members::get_members_response_body(
            member_unique_name.as_deref(),
            *tree_op,
            restrictions,
            backend,
            user,
            config,
        );
        // Log and trace a bounded preview: a wide hierarchy's rowset is
        // hundreds of megabytes and must not be materialised for a log line.
        let preview = match &response {
            members::MemberResponse::Fault(message) => message.clone(),
            members::MemberResponse::Rowset(rowset) => rowset.preview(8 * 1024),
        };
        debug_write("RESPONSE XML (preview):");
        debug_write(&preview);
        mallardcube::xmla_trace::trace_request("MdschemaMembers", body, &preview, None, None);
        return XmlaBody::Members(response);
    }
    XmlaBody::Full(route_full(request, body, backend, user, config))
}

/// Route a request whose response is built in full. `route_request` handles
/// the streaming member rowset first and delegates everything else here.
fn route_full<B: backend::QueryBackend + ?Sized>(
    request: &XmlaRequest,
    body: &str,
    backend: &B,
    user: &UserContext,
    config: &ProxyConfig,
) -> String {
    // A user whose roles grant no model permission (no matching role, or a
    // role set to `none`) has no access at all: the reference answers a
    // permission fault rather than advertising the model. Measured: an
    // unknown session/role combination answers "the user ... does not have
    // permission", and a role-less user's metadata is not disclosed.
    if crate::engine::model::effective_model_permission(config, user)
        == crate::project::config::ModelPermission::None
    {
        let resp = mallardcube::response::fault_response(
            "the user has no access to this model: no role grants read permission",
        );
        mallardcube::xmla_trace::trace_request("NoModelPermission", body, &resp, None, None);
        return resp;
    }

    // A `<Catalog>` property naming another database is refused the way the
    // reference refuses it, for Discover and Execute alike (measured
    // 2026-09-25). A mismatched *restriction* gets the empty rowset instead.
    if let Some(fault) =
        execute::runtime::catalog_scope_fault(request.property_catalog(), config, user)
    {
        mallardcube::xmla_trace::trace_request("ScopeFault", body, &fault, None, None);
        return fault;
    }

    match request {
        XmlaRequest::BeginSession | XmlaRequest::ExecuteEmpty => {
            let resp = execute::dispatch::get_empty_execute_response();
            mallardcube::xmla_trace::trace_request(
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
            mallardcube::xmla_trace::trace_request(
                &format!("{:?}", request),
                body,
                &resp,
                None,
                None,
            );
            resp
        }

        XmlaRequest::DiscoverSchemaRowsets { schema_name } => {
            let resp = schema_rowsets::get_schemas_response(schema_name.as_deref());
            mallardcube::xmla_trace::trace_request(
                "DiscoverSchemaRowsets",
                body,
                &resp,
                None,
                None,
            );
            resp
        }
        XmlaRequest::DbSchemaCatalogs { restrictions } => {
            let resp = catalogs::get_catalogs_response(restrictions);
            mallardcube::xmla_trace::trace_request("DbSchemaCatalogs", body, &resp, None, None);
            resp
        }
        XmlaRequest::MdschemaCubes { restrictions } => {
            let resp = cubes::get_cubes_response(restrictions);
            mallardcube::xmla_trace::trace_request("MdschemaCubes", body, &resp, None, None);
            resp
        }
        XmlaRequest::DbschemaTables { restrictions } => {
            let resp = tables::get_tables_response(restrictions, user, config);
            mallardcube::xmla_trace::trace_request("DbschemaTables", body, &resp, None, None);
            resp
        }

        XmlaRequest::MdschemaDimensions { restrictions } => {
            println!("📥 Sending Dimensions to Excel");
            let resp = dimensions::get_dimensions_response(restrictions, user, config);
            mallardcube::xmla_trace::trace_request("MdschemaDimensions", body, &resp, None, None);
            resp
        }
        XmlaRequest::MdschemaMeasures { restrictions } => {
            println!("📥 Sending Measures to Excel");
            let resp = measures::get_measures_response(restrictions, user, config);
            mallardcube::xmla_trace::trace_request("MdschemaMeasures", body, &resp, None, None);
            resp
        }
        XmlaRequest::MdschemaHierarchies { restrictions } => {
            println!("📥 Hierarchies");
            let resp = hierarchies::get_hierarchies_response(restrictions, user, config);
            mallardcube::xmla_trace::trace_request("MdschemaHierarchies", body, &resp, None, None);
            resp
        }
        XmlaRequest::MdschemaLevels { restrictions } => {
            println!("📥 Levels");
            let resp = levels::get_levels_response(restrictions, user, config);
            mallardcube::xmla_trace::trace_request("MdschemaLevels", body, &resp, None, None);
            resp
        }

        XmlaRequest::ExecuteStatement { mdx, catalog } => {
            println!("📥 MDX: {}", mdx);
            debug_write("===== EXECUTE REQUEST =====");
            debug_write(&format!("MDX: {}", mdx));
            debug_write("REQUEST XML:");
            debug_write(body);

            let (resp, timings) = if let Some(fault) =
                execute::runtime::catalog_scope_fault(catalog.as_deref(), config, user)
            {
                (fault, None)
            } else if let Some(fault) = execute::runtime::unhonourable_filter_fault(config, user) {
                (fault, None)
            } else if mdx_semantic::is_drillthrough(mdx)
                && let Some(fault) = execute::runtime::mdx_cube_scope_fault(mdx, config)
            {
                (fault, None)
            } else if mdx_semantic::is_drillthrough(mdx) {
                match execute::runtime::drillthrough_fault(config, user) {
                    Some(fault) => (fault, None),
                    None => (
                        execute::dispatch::get_execute_drillthrough_response(mdx, backend),
                        None,
                    ),
                }
            } else {
                let (r, t) =
                    execute_builders::get_execute_cellset_response_with_backend_and_context(
                        mdx, backend, user, config,
                    );
                (r, Some(t))
            };

            debug_write("RESPONSE XML:");
            debug_write(&resp);
            mallardcube::xmla_trace::trace_request(
                "ExecuteStatement",
                body,
                &resp,
                Some(mdx),
                timings.as_ref(),
            );
            resp
        }

        XmlaRequest::MdschemaProperties {
            property_type,
            restrictions,
        } => {
            println!("📥 MDSCHEMA_PROPERTIES (PROPERTY_TYPE={:?})", property_type);
            let resp = mdschema_properties::get_mdschema_properties_response(
                *property_type,
                restrictions,
                user,
                config,
            );
            mallardcube::xmla_trace::trace_request("MdschemaProperties", body, &resp, None, None);
            resp
        }
        XmlaRequest::MdschemaMembers { .. } => {
            // Handled by `route_request`, which streams the rowset.
            unreachable!("MDSCHEMA_MEMBERS is routed by route_request")
        }

        XmlaRequest::DiscoverLiterals => {
            println!("📥 DISCOVER_LITERALS");
            let resp = literals::get_literals_response();
            mallardcube::xmla_trace::trace_request("DiscoverLiterals", body, &resp, None, None);
            resp
        }
        XmlaRequest::MdschemaSets => {
            println!("📥 MDSCHEMA_SETS");
            let resp = sets::get_sets_response();
            mallardcube::xmla_trace::trace_request("MdschemaSets", body, &resp, None, None);
            resp
        }
        XmlaRequest::MdschemaKpis => {
            println!("📥 MDSCHEMA_KPIS");
            let resp = kpis::get_kpis_response();
            mallardcube::xmla_trace::trace_request("MdschemaKpis", body, &resp, None, None);
            resp
        }
        XmlaRequest::MdschemaMeasureGroups { restrictions } => {
            println!("📥 MDSCHEMA_MEASUREGROUPS");
            let resp = measure_groups::get_measure_groups_response(restrictions, user, config);
            mallardcube::xmla_trace::trace_request(
                "MdschemaMeasureGroups",
                body,
                &resp,
                None,
                None,
            );
            resp
        }
        XmlaRequest::MdschemaMeasureGroupDimensions { restrictions } => {
            println!("📥 MDSCHEMA_MEASUREGROUP_DIMENSIONS");
            let resp = measuregroup_dimensions::get_measuregroup_dimensions_response(
                restrictions,
                user,
                config,
            );
            mallardcube::xmla_trace::trace_request(
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
            mallardcube::xmla_trace::trace_request("TmschemaModel", body, &resp, None, None);
            resp
        }
        XmlaRequest::TmschemaTables => {
            println!("📥 TMSCHEMA_TABLES");
            let resp = tmschema::get_tmschema_tables_response(user, config);
            mallardcube::xmla_trace::trace_request("TmschemaTables", body, &resp, None, None);
            resp
        }
        XmlaRequest::TmschemaColumns => {
            println!("📥 TMSCHEMA_COLUMNS");
            let resp = tmschema::get_tmschema_columns_response();
            mallardcube::xmla_trace::trace_request("TmschemaColumns", body, &resp, None, None);
            resp
        }
        XmlaRequest::TmschemaMeasures => {
            println!("📥 TMSCHEMA_MEASURES");
            let resp = tmschema::get_tmschema_measures_response();
            mallardcube::xmla_trace::trace_request("TmschemaMeasures", body, &resp, None, None);
            resp
        }
        XmlaRequest::TmschemaHierarchies => {
            println!("📥 TMSCHEMA_HIERARCHIES");
            let resp = tmschema::get_tmschema_hierarchies_response();
            mallardcube::xmla_trace::trace_request("TmschemaHierarchies", body, &resp, None, None);
            resp
        }
        XmlaRequest::TmschemaLevels => {
            println!("📥 TMSCHEMA_LEVELS");
            let resp = tmschema::get_tmschema_levels_response();
            mallardcube::xmla_trace::trace_request("TmschemaLevels", body, &resp, None, None);
            resp
        }
        XmlaRequest::TmschemaRelationships => {
            println!("📥 TMSCHEMA_RELATIONSHIPS");
            let resp = tmschema::get_tmschema_relationships_response(user, config);
            mallardcube::xmla_trace::trace_request(
                "TmschemaRelationships",
                body,
                &resp,
                None,
                None,
            );
            resp
        }
        XmlaRequest::TmschemaPartitions => {
            println!("📥 TMSCHEMA_PARTITIONS");
            let resp = tmschema::get_tmschema_partitions_response();
            mallardcube::xmla_trace::trace_request("TmschemaPartitions", body, &resp, None, None);
            resp
        }
        XmlaRequest::DiscoverXmlMetadata => {
            println!("📥 DISCOVER_XML_METADATA");
            let resp = tmschema::get_discover_xml_metadata_response();
            mallardcube::xmla_trace::trace_request("DiscoverXmlMetadata", body, &resp, None, None);
            resp
        }
        XmlaRequest::DiscoverCalcDependency => {
            println!("📥 DISCOVER_CALC_DEPENDENCY");
            let resp = tmschema::get_discover_calc_dependency_response();
            mallardcube::xmla_trace::trace_request(
                "DiscoverCalcDependency",
                body,
                &resp,
                None,
                None,
            );
            resp
        }
        XmlaRequest::DiscoverEnumerators => {
            println!("📥 DISCOVER_ENUMERATORS");
            let resp = enumerators::get_enumerators_response();
            mallardcube::xmla_trace::trace_request("DiscoverEnumerators", body, &resp, None, None);
            resp
        }
        XmlaRequest::DiscoverKeywords => {
            println!("📥 DISCOVER_KEYWORDS");
            let resp = keywords::get_keywords_response();
            mallardcube::xmla_trace::trace_request("DiscoverKeywords", body, &resp, None, None);
            resp
        }
        XmlaRequest::DiscoverDatasources => {
            println!("📥 DISCOVER_DATASOURCES");
            let resp = datasources::get_datasources_response();
            mallardcube::xmla_trace::trace_request("DiscoverDatasources", body, &resp, None, None);
            resp
        }

        XmlaRequest::MdschemaFunctions { restrictions } => {
            println!("📥 MDSCHEMA_FUNCTIONS");
            let resp = functions::get_functions_response(restrictions);
            mallardcube::xmla_trace::trace_request("MdschemaFunctions", body, &resp, None, None);
            resp
        }
        XmlaRequest::UnsupportedRestriction(name) => {
            // The rowset does not advertise this restriction name; the
            // reference faults instead of ignoring it (plan 055).
            eprintln!("!!! Unsupported restriction: {name}");
            let resp = mallardcube::response::fault_response(&format!(
                "The restriction, {name}, is not recognized by the server."
            ));
            mallardcube::xmla_trace::trace_request(
                "UnsupportedRestriction",
                body,
                &resp,
                None,
                None,
            );
            resp
        }
        XmlaRequest::Malformed(reason) => {
            // The request could not be read faithfully: an unparsable entity,
            // or a Statement we could not decode (CDATA used to land here as an
            // empty success). Fault — a silent empty cellset is a wrong answer.
            eprintln!("!!! Malformed request: {reason}");
            let resp =
                mallardcube::response::fault_response(&format!("malformed request: {reason}"));
            mallardcube::xmla_trace::trace_request("Malformed", body, &resp, None, None);
            resp
        }
        XmlaRequest::Unknown => {
            // Answer with a fault: an empty body makes Excel report "XML
            // parsing failed … a document must contain exactly one root
            // element", which hides what actually went wrong (plan 049).
            eprintln!("Unknown request: {}", body);
            let resp = mallardcube::response::fault_response(
                "unsupported request: this proxy does not handle this XMLA request type",
            );
            mallardcube::xmla_trace::trace_request("Unknown", body, &resp, None, None);
            resp
        }
    }
}
