/// Long-lived Node Malloy compiler.
///
/// Spawns `js/malloy-worker.js` once and keeps it alive for
/// repeated compile requests. Uses NDJSON over stdin/stdout.
///
/// Implements `MalloyCompiler` so it can replace the one-shot
/// `NodeMalloyCompiler` in benchmarks and tests.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Mutex, atomic::{AtomicU64, Ordering}};
use std::time::{Duration, Instant};
use crate::engine::malloy_compiler::{CompileResult, MalloyCompiler, MalloyCompileError};

const COMPILE_TIMEOUT: Duration = Duration::from_secs(60);

pub struct LongLivedNodeMalloyCompiler {
    stdin: Mutex<ChildStdin>,
    reader: Mutex<BufReader<std::process::ChildStdout>>,
    child: Mutex<Option<Child>>,
    next_id: AtomicU64,
    request_lock: Mutex<()>,
}

impl Drop for LongLivedNodeMalloyCompiler {
    fn drop(&mut self) {
        // Best-effort shutdown
        let mut stdin = self.stdin.lock().unwrap();
        let _ = writeln!(stdin, r#"{{"type":"shutdown"}}"#);
        let _ = stdin.flush();
        drop(stdin);
        if let Ok(mut child) = self.child.lock() {
            if let Some(ref mut c) = *child {
                let _ = c.kill();
            }
        }
    }
}

impl LongLivedNodeMalloyCompiler {
    pub fn new() -> Result<Self, MalloyCompileError> {
        let config_path = std::env::var("PROXY_CONFIG")
            .unwrap_or_else(|_| "project3/proxy-config.json".into());
        let mut cmd = &mut Command::new("node");
        cmd = cmd.arg("js/malloy-worker.js")
            .env("PROXY_CONFIG", &config_path);

        // Worker compiles against an in-memory derived schema — it does not
        // open the real DuckDB file.  The Rust backend is the only process
        // that opens the real database, avoiding DuckDB file-lock conflicts.

        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| MalloyCompileError::RuntimeError(format!("spawn worker: {e}")))?;

        let stdin = child.stdin.take()
            .ok_or_else(|| MalloyCompileError::RuntimeError("no stdin".into()))?;
        let stdout = child.stdout.take()
            .ok_or_else(|| MalloyCompileError::RuntimeError("no stdout".into()))?;
        let mut reader = BufReader::new(stdout);

        // Wait for ready signal
        let mut line = String::new();
        reader.read_line(&mut line)
            .map_err(|e| MalloyCompileError::RuntimeError(format!("read ready: {e}")))?;

        let v: serde_json::Value = serde_json::from_str(&line)
            .map_err(|e| MalloyCompileError::RuntimeError(format!("parse ready: {e}")))?;
        if v["type"] != "ready" {
            return Err(MalloyCompileError::RuntimeError(format!(
                "expected ready, got: {line}"
            )));
        }

        Ok(Self {
            stdin: Mutex::new(stdin),
            reader: Mutex::new(reader),
            child: Mutex::new(Some(child)),
            next_id: AtomicU64::new(1),
            request_lock: Mutex::new(()),
        })
    }

    fn send_request(&self, source: &str) -> Result<(String, f64), MalloyCompileError> {
        let _lock = self.request_lock.lock().unwrap();

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let req = serde_json::json!({"id": id, "type": "compile", "source": source});
        let req_line = serde_json::to_string(&req).unwrap();

        {
            let mut stdin = self.stdin.lock().unwrap();
            writeln!(stdin, "{req_line}")
                .map_err(|e| MalloyCompileError::RuntimeError(format!("write: {e}")))?;
            stdin.flush()
                .map_err(|e| MalloyCompileError::RuntimeError(format!("flush: {e}")))?;
        }

        let start = Instant::now();
        loop {
            if start.elapsed() > COMPILE_TIMEOUT {
                return Err(MalloyCompileError::RuntimeError("compile timeout".into()));
            }

            let mut line = String::new();
            {
                let mut reader = self.reader.lock().unwrap();
                reader.read_line(&mut line)
                    .map_err(|e| MalloyCompileError::RuntimeError(format!("read response: {e}")))?;
            }

            let v: serde_json::Value = serde_json::from_str(&line)
                .map_err(|e| MalloyCompileError::RuntimeError(format!("parse response: {e}")))?;

            if v["id"].as_u64() != Some(id) {
                continue; // skip stale responses
            }

            if v["ok"].as_bool() == Some(true) {
                let sql = v["sql"].as_str().unwrap_or("").to_string();
                let ms = v["compile_ms"].as_f64().unwrap_or(0.0);
                return Ok((sql, ms));
            } else {
                let err = v["error"].as_str().unwrap_or("unknown error");
                return Err(MalloyCompileError::RuntimeError(err.into()));
            }
        }
    }
}

impl MalloyCompiler for LongLivedNodeMalloyCompiler {
    fn compile_malloy(&self, source: &str) -> Result<CompileResult, MalloyCompileError> {
        let (sql, compile_ms) = self.send_request(source)?;
        Ok(CompileResult { sql, compile_ms })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::model::{default_model, SemanticModel};
    use crate::engine::plan::{QueryPlan, TypedDimensionFilter};
    use crate::engine::malloy::malloy_source_for_query_plan;
    use std::sync::OnceLock;

    fn compiler() -> &'static LongLivedNodeMalloyCompiler {
        static C: OnceLock<LongLivedNodeMalloyCompiler> = OnceLock::new();
        C.get_or_init(|| LongLivedNodeMalloyCompiler::new().expect("start worker"))
    }

    fn model() -> SemanticModel {
        default_model()
    }

    #[test]
    fn compile_total() {
        let plan = QueryPlan::Total { measure: "TotalSales".to_string(), filters: vec![] };
        let r = compiler().compile_query(&model(), &plan).expect("compile total");
        assert!(!r.sql.is_empty());
        assert!(r.sql.to_uppercase().contains("SUM"));
    }

    #[test]
    fn compile_group_by_one() {
        let plan = QueryPlan::GroupBy {
            measure: "TotalSales".to_string(),
            group_by: vec!["Produktkategori".to_string()],
            filters: vec![],
        };
        let r = compiler().compile_query(&model(), &plan).expect("compile groupby 1");
        assert!(r.sql.contains("GROUP BY"));
    }

    #[test]
    fn compile_group_by_two() {
        let plan = QueryPlan::GroupBy {
            measure: "TotalSales".to_string(),
            group_by: vec!["Produktkategori".to_string(), "Region".to_string()],
            filters: vec![],
        };
        let r = compiler().compile_query(&model(), &plan).expect("compile groupby 2");
        assert!(r.sql.contains("GROUP BY"));
    }

    #[test]
    fn compile_filtered() {
        let plan = QueryPlan::GroupBy {
            measure: "TotalSales".to_string(),
            group_by: vec!["Produktkategori".to_string()],
            filters: vec![TypedDimensionFilter {
                dimension: "Region".to_string(),
                time_flag: None,
                members: vec!["North".into()],
            }],
        };
        let r = compiler().compile_query(&model(), &plan).expect("compile filtered");
        assert!(r.sql.to_uppercase().contains("WHERE"));
    }

    #[test]
    fn compile_rejects_count() {
        let plan = QueryPlan::Count { dimension: "Produktkategori".to_string() };
        assert!(compiler().compile_query(&model(), &plan).is_err());
    }

    #[test]
    fn js_compile_ms_is_populated() {
        let plan = QueryPlan::Total { measure: "TotalSales".to_string(), filters: vec![] };
        let src = malloy_source_for_query_plan(&model(), &plan);
        // Use a unique source so Malloy cannot reuse an internal cache.
        let unique = format!("{src}\n-- u1");
        let r = compiler().compile_malloy(&unique).expect("compile");
        assert!(r.compile_ms > 0.0, "JS compile_ms should be populated by worker, got 0");
    }

    #[test]
    fn cold_compile_reports_js_time() {
        let m = model();
        let plan = QueryPlan::Total { measure: "TotalSales".to_string(), filters: vec![] };
        let base = malloy_source_for_query_plan(&m, &plan);
        let compiler = compiler();

        // Warm-up once so worker and connection are ready.
        let _ = compiler.compile_malloy(&base).unwrap();

        // Cold: unique source every iteration; the worker MUST report
        // a non-zero compile_ms even if Malloy caches the model internally.
        for i in 0..5 {
            let unique = format!("{base}\n-- u{i}");
            let r = compiler.compile_malloy(&unique).unwrap();
            assert!(r.compile_ms > 0.0, "iteration {i}: JS compile_ms was 0");
            assert!(!r.sql.is_empty());
        }

        // Also verify the warm (same-source) path is sub-500ms.
        let start = std::time::Instant::now();
        let _ = compiler.compile_malloy(&base).unwrap();
        assert!(start.elapsed().as_millis() < 500, "warm compile too slow");
    }

    #[test]
    fn warm_compile_is_fast() {
        let m = model();
        let plan = QueryPlan::Total { measure: "TotalSales".to_string(), filters: vec![] };
        let compiler = compiler();
        let _first = compiler.compile_query(&m, &plan).unwrap();
        let start = std::time::Instant::now();
        let _second = compiler.compile_query(&m, &plan).unwrap();
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 500, "warm compile too slow: {}ms", elapsed.as_millis());
    }
}
