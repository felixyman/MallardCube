/// XMLA trace capture — writes every request/response pair as NDJSON.
///
/// Enabled by setting `XMLA_TRACE=1` at startup.
/// Output: `xmla-trace.jsonl` in the working directory.
///
/// Each line is a self-contained JSON record suitable for replay testing.
use std::cell::Cell;
use std::fs::File;
use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

static TRACE_FILE: Mutex<Option<File>> = Mutex::new(None);
static TRACE_SEQ: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// Wall-clock start of the request, set on the blocking worker thread just
    /// before backend checkout, so `trace_request` can record total latency.
    static REQ_START: Cell<Option<Instant>> = const { Cell::new(None) };
}

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

/// Mark the start of a request on the blocking worker thread. Called before
/// backend checkout so the recorded `wall_us` includes connection-open time.
pub fn mark_request_start() {
    REQ_START.with(|c| c.set(Some(Instant::now())));
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

    let wall_us = REQ_START.with(|c| {
        let start = c.take();
        start.map(|s| s.elapsed().as_micros() as u64)
    });

    let mut rec = serde_json::json!({
        "seq": seq,
        "request_kind": request_kind,
        "request_xml": request_xml,
        "response_xml": response_xml,
    });

    if let Some(w) = wall_us {
        rec["wall_us"] = serde_json::Value::from(w);
    }

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
            "sql_execute_us": t.sql_execute_us,
            "xml_render_us": t.xml_render_us,
            "total_us": t.total_us,
        });
    }

    let line = serde_json::to_string(&rec).unwrap();
    let _ = writeln!(file, "{line}");
    let _ = file.flush();
}
