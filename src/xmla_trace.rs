/// XMLA trace capture — writes every request/response pair as NDJSON.
///
/// Enabled by setting `XMLA_TRACE=1` at startup.
/// Output: `xmla-trace.jsonl` in the working directory.
///
/// Each line is a self-contained JSON record suitable for replay testing.
use std::fs::File;
use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

static TRACE_FILE: Mutex<Option<File>> = Mutex::new(None);
static TRACE_SEQ: AtomicU64 = AtomicU64::new(0);

pub fn init_trace() {
    if std::env::var("XMLA_TRACE").is_ok_and(|v| v == "1") {
        let file = File::create("xmla-trace.jsonl").expect("failed to create xmla-trace.jsonl");
        *TRACE_FILE.lock().unwrap() = Some(file);
        eprintln!("[trace] XMLA trace enabled -> xmla-trace.jsonl");
    }
}

pub fn trace_enabled() -> bool {
    TRACE_FILE.lock().unwrap().is_some()
}

pub fn trace_request(
    request_kind: &str,
    request_xml: &str,
    response_xml: &str,
    mdx: Option<&str>,
    timings: Option<&crate::engine::timing::Timings>,
) {
    let mut guard = TRACE_FILE.lock().unwrap();
    let Some(ref mut file) = *guard else { return };

    let seq = TRACE_SEQ.fetch_add(1, Ordering::Relaxed);

    let mut rec = serde_json::json!({
        "seq": seq,
        "request_kind": request_kind,
        "request_xml": request_xml,
        "response_xml": response_xml,
    });

    if let Some(m) = mdx {
        rec["mdx"] = serde_json::Value::String(m.to_string());
    }

    if let Some(t) = timings {
        rec["timings"] = serde_json::json!({
            "runtime_path": t.runtime_path.as_str(),
            "plan_key": t.plan_key,
            "mdx_parse_us": t.mdx_parse_us,
            "semantic_us": t.semantic_us,
            "plan_us": t.plan_us,
            "sql_emit_us": t.sql_emit_us,
            "malloy_emit_us": t.malloy_emit_us,
            "malloy_compile_us": t.malloy_compile_us,
            "sql_execute_us": t.sql_execute_us,
            "xml_render_us": t.xml_render_us,
            "total_us": t.total_us,
            "js_compile_ms": t.js_compile_ms,
            "malloy_source_cache_hit": t.malloy_source_cache_hit,
            "compiled_sql_cache_hit": t.compiled_sql_cache_hit,
        });
    }

    let line = serde_json::to_string(&rec).unwrap();
    let _ = writeln!(file, "{line}");
    let _ = file.flush();
}
