//! Data reload support (plan 041 phase C).
//!
//! The server keeps its read-only pool for the life of the process, so a data
//! refresh is a controlled reopen: SIGHUP (or the optional stamp watcher)
//! builds a new [`BackendSource`](crate::backend::BackendSource), swaps it in
//! for new requests, and lets in-flight requests finish on the old pool.
//!
//! Aggregation sidecars are the wrinkle: rebuilding one needs write access
//! that the live pool holds (it attaches the sidecar read-only), so a stale
//! sidecar is *disabled* for the reload — queries fall back to the fact table,
//! which is correct but slower — and rebuilt on the next restart.

use std::path::Path;
use std::time::UNIX_EPOCH;

/// Size + mtime stamp used to detect that a data file changed.
pub fn file_stamp(path: &Path) -> (u64, u64) {
    std::fs::metadata(path)
        .map(|m| {
            let mtime = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            (m.len(), mtime)
        })
        .unwrap_or((0, 0))
}

/// Disable aggregation routing when the configured sidecar no longer matches
/// the data file. Returns true when routing was disabled (the caller should
/// log it); false when there is no sidecar or it is current.
pub fn disable_stale_aggregations(db: &Path) -> bool {
    let Some(sidecar) = crate::engine::aggregate::cache_path() else {
        return false;
    };
    if crate::engine::aggregate::sidecar_is_current(db, &sidecar) {
        return false;
    }
    crate::engine::aggregate::disable();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("mallardcube-reload-{}-{name}", std::process::id()))
    }

    #[test]
    fn file_stamp_tracks_content_changes() {
        let path = temp_path("stamp.txt");
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, b"one").unwrap();
        let first = file_stamp(&path);
        std::fs::write(&path, b"one-two").unwrap();
        let second = file_stamp(&path);
        assert_ne!(first, second, "a size change must be visible in the stamp");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_stamps_as_zero() {
        assert_eq!(file_stamp(Path::new("/nonexistent/x.duckdb")), (0, 0));
    }

    /// The reload contract: after the runbook's rename, the old pool keeps
    /// serving the old inode while a reopened source reads the new file.
    #[test]
    fn reopened_source_sees_the_replaced_file() {
        let path = temp_path("swap.duckdb");
        let staging = temp_path("swap-staging.duckdb");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&staging);

        let conn = duckdb::Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE t (i INT); INSERT INTO t VALUES (1);")
            .unwrap();
        drop(conn);
        let old = crate::backend::BackendSource::file(&path).unwrap();
        assert_eq!(old.checkout().query_count("SELECT COUNT(*) FROM t"), 1);

        let conn = duckdb::Connection::open(&staging).unwrap();
        conn.execute_batch("CREATE TABLE t (i INT); INSERT INTO t VALUES (1),(2),(3);")
            .unwrap();
        drop(conn);
        std::fs::rename(&staging, &path).unwrap();

        assert_eq!(
            old.checkout().query_count("SELECT COUNT(*) FROM t"),
            1,
            "in-flight requests keep the old snapshot"
        );
        let new = crate::backend::BackendSource::file(&path).unwrap();
        assert_eq!(
            new.checkout().query_count("SELECT COUNT(*) FROM t"),
            3,
            "a reopened source reads the new file"
        );

        let _ = std::fs::remove_file(&path);
    }
}
