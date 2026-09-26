//! Structured audit events (plan 058-D).
//!
//! Opt-in via `MALLARDCUBE_AUDIT_FILE`: one JSON object per line, appended as
//! the proxy makes decisions. Events carry the request id, the user, their
//! roles, the rule that decided and a short detail — never request bodies or
//! credentials — so the stream is safe to ship to a log collector.
//!
//! The request id is thread-local and set around the synchronous request
//! handling in `main`; when none is set (tests, tools) it reads `"-"`.

use crate::engine::model::UserContext;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

thread_local! {
    static REQUEST_ID: std::cell::RefCell<Option<String>> =
        const { std::cell::RefCell::new(None) };
}

fn audit_path() -> Option<&'static PathBuf> {
    static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    PATH.get_or_init(|| {
        std::env::var("MALLARDCUBE_AUDIT_FILE")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    })
    .as_ref()
}

/// Set the current request's correlation id.
pub fn set_request_id(id: &str) {
    REQUEST_ID.with(|cell| *cell.borrow_mut() = Some(id.to_string()));
}

pub fn new_request_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn current_request_id() -> String {
    REQUEST_ID
        .with(|cell| cell.borrow().clone())
        .unwrap_or_else(|| "-".to_string())
}

/// Whether the audit stream is configured (used by `/status`, so an operator
/// can confirm the control is live rather than assume it).
pub fn enabled() -> bool {
    audit_path().is_some()
}

/// Append one decision event. With no audit file configured (the default),
/// this is a no-op.
pub fn emit(event: &str, user: &UserContext, rule: &str, detail: &str) {
    let Some(path) = audit_path() else {
        return;
    };
    let line = event_json(
        event,
        user,
        rule,
        detail,
        &current_request_id(),
        now_unix_millis(),
    );
    append_event(path, &line);
}

/// One `write_all` per event: the line is a single append under `O_APPEND`, so
/// concurrent workers cannot interleave halves of two records (review
/// 2026-09-27). A failure warns once — an audit stream that silently writes
/// nothing is worse than none.
fn append_event(path: &std::path::Path, line: &str) {
    let mut record = String::with_capacity(line.len() + 1);
    record.push_str(line);
    record.push('\n');
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let opened = options.open(path);
    let written = opened.and_then(|mut file| file.write_all(record.as_bytes()));
    if let Err(error) = written {
        static WARNED: OnceLock<()> = OnceLock::new();
        WARNED.get_or_init(|| {
            eprintln!(
                "⚠️  cannot write the audit stream at {}: {error}",
                path.display()
            );
        });
    }
}

fn now_unix_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

/// Events keep only a bounded, JSON-escaped detail: a caller-controlled value
/// (a request property, a user header) must not become an unbounded record.
const MAX_DETAIL_CHARS: usize = 256;

/// The event line. Pure so tests can pin the shape: structured, correlated,
/// and never carrying request bodies.
fn event_json(
    event: &str,
    user: &UserContext,
    rule: &str,
    detail: &str,
    request_id: &str,
    ts_unix_ms: u128,
) -> String {
    let detail = truncate_detail(detail);
    serde_json::json!({
        "ts_unix_ms": ts_unix_ms,
        "request_id": request_id,
        "pid": std::process::id(),
        "event": event,
        "user": user.user_id,
        "admin": user.is_administrator,
        "roles": user.roles,
        "rule": rule,
        "detail": detail,
    })
    .to_string()
}

fn truncate_detail(detail: &str) -> String {
    if detail.chars().count() <= MAX_DETAIL_CHARS {
        return detail.to_string();
    }
    let mut out: String = detail.chars().take(MAX_DETAIL_CHARS).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(event: &str, rule: &str, detail: &str) -> String {
        event_json(event, &UserContext::deny_all(), rule, detail, "req-1", 42)
    }

    #[test]
    fn events_are_structured_and_carry_no_bodies() {
        let mut user = UserContext::deny_all();
        user.roles = vec!["EU".into()];
        let line = event_json(
            "refusal",
            &user,
            "restricted-fallback",
            "measure 'X' uses authored SQL that cannot be filtered",
            "req-1",
            42,
        );
        let value: serde_json::Value = serde_json::from_str(&line).expect("json");
        let mut keys: Vec<&str> = value
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "admin",
                "detail",
                "event",
                "pid",
                "request_id",
                "roles",
                "rule",
                "ts_unix_ms",
                "user"
            ],
            "the event schema is pinned: no body field may creep in"
        );
        assert_eq!(value["request_id"], "req-1");
        assert_eq!(value["event"], "refusal");
        assert_eq!(value["user"], user.user_id);
        assert_eq!(value["roles"][0], "EU");
        assert_eq!(value["admin"], false);
        assert_eq!(value["rule"], "restricted-fallback");
        assert_eq!(value["pid"], std::process::id());
        assert!(!line.contains("SELECT"), "{line}");
    }

    #[test]
    fn caller_controlled_details_are_bounded() {
        let line = event("refusal", "catalog-scope", &"x".repeat(1000));
        let value: serde_json::Value = serde_json::from_str(&line).expect("json");
        let detail = value["detail"].as_str().expect("detail");
        assert_eq!(detail.chars().count(), MAX_DETAIL_CHARS + 1);
        assert!(detail.ends_with('…'));
    }

    #[test]
    fn an_event_is_one_parsable_line() {
        let dir =
            std::env::temp_dir().join(format!("mallardcube-audit-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("audit.jsonl");
        let _ = std::fs::remove_file(&path);

        // A detail with a quote and a newline must land as one JSONL line.
        append_event(
            &path,
            &event("auth", "role-resolution", "quote \" and \n inside"),
        );
        append_event(&path, &event("refusal", "catalog-scope", "second"));

        let content = std::fs::read_to_string(&path).expect("read");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2, "{content}");
        for line in lines {
            let _: serde_json::Value = serde_json::from_str(line).expect("each line parses");
        }
        let _ = std::fs::remove_file(&path);
    }
}
