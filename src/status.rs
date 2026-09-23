//! Server status and data-freshness reporting (plan 041).
//!
//! The proxy holds the DuckDB file for its whole lifetime through a read-only
//! connection pool, so "which data am I serving, and since when?" is
//! operational information, not an implementation detail: a load job cannot
//! write the file while the server runs, and the server cannot see new data
//! until it reopens the file. `GET /status` exposes the stamp as JSON;
//! `GET /health` is a liveness probe. Both are auth-gated when `auth` is
//! configured.

use crate::engine::model::UserContext;
use crate::engine::settings::EngineSettings;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Stamp of the data file a server is serving.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataStamp {
    pub path: String,
    pub size_bytes: u64,
    pub mtime_unix: u64,
    /// When this process opened the file (startup, or reload once plan 041
    /// phase C lands).
    pub loaded_at_unix: u64,
}

impl DataStamp {
    /// Stamp a file. A missing file yields zeroed size/mtime — the demo
    /// database is created lazily, and `/status` must still answer.
    pub fn capture(path: &Path) -> Self {
        let (size_bytes, mtime_unix) = std::fs::metadata(path)
            .map(|m| {
                let mtime = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                (m.len(), mtime)
            })
            .unwrap_or((0, 0));
        Self {
            path: path.display().to_string(),
            size_bytes,
            mtime_unix,
            loaded_at_unix: now_unix(),
        }
    }
}

/// Seconds since the Unix epoch (0 if the clock is before it).
pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Payload served by `GET /status`.
#[derive(Debug, Clone)]
pub struct StatusInfo {
    pub catalog: String,
    pub cube: String,
    pub pool_size: usize,
    pub started_at_unix: u64,
    pub data: DataStamp,
    pub result_cache: bool,
    /// Engine settings in force and where each came from (plan 051-A/054-C).
    pub engine: EngineSettings,
}

impl StatusInfo {
    pub fn to_json(&self) -> String {
        let memory_limit = self.engine.memory_limit.as_ref();
        let temp_directory = self.engine.temp_directory.as_ref();
        let threads = self.engine.threads.as_ref();
        serde_json::json!({
            "catalog": self.catalog,
            "cube": self.cube,
            "pool_size": self.pool_size,
            "started_at_unix": self.started_at_unix,
            "result_cache": self.result_cache,
            "data": {
                "path": self.data.path,
                "size_bytes": self.data.size_bytes,
                "mtime_unix": self.data.mtime_unix,
                "loaded_at_unix": self.data.loaded_at_unix,
            },
            "engine": {
                "memory_limit": memory_limit.map(|s| s.value.display()),
                "memory_limit_source": memory_limit
                    .map(|s| s.source.as_str())
                    .unwrap_or("default"),
                "temp_directory": temp_directory.map(|s| s.value.display().to_string()),
                "temp_directory_source": temp_directory
                    .map(|s| s.source.as_str())
                    .unwrap_or("default"),
                "threads": threads.map(|s| s.value),
                "threads_source": threads.map(|s| s.source.as_str()).unwrap_or("default"),
            }
        })
        .to_string()
    }
}

/// Whether a request may read operational endpoints. Without an `auth` config
/// every request is the administrator (backward-compatible mode). With `auth`
/// configured, `build_user_context` returns deny-all when the identity is
/// missing or matches no role, and deny-all must not read `/status`.
pub fn authenticated(user: &UserContext) -> bool {
    user.is_administrator || !user.roles.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_captures_size_and_mtime() {
        let path =
            std::env::temp_dir().join(format!("mallardcube-stamp-{}.txt", std::process::id()));
        std::fs::write(&path, b"hello").unwrap();
        let stamp = DataStamp::capture(&path);
        assert_eq!(stamp.size_bytes, 5);
        assert!(stamp.mtime_unix > 0, "mtime must be captured");
        assert!(stamp.loaded_at_unix > 0, "loaded_at must be set");
        assert!(stamp.path.ends_with(".txt"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_yields_zeroed_stamp() {
        let stamp = DataStamp::capture(Path::new("/nonexistent/mallardcube.duckdb"));
        assert_eq!(stamp.size_bytes, 0);
        assert_eq!(stamp.mtime_unix, 0);
        assert!(stamp.loaded_at_unix > 0, "loaded_at is always set");
    }

    #[test]
    fn status_json_has_freshness_fields() {
        use crate::engine::settings::{MemoryLimit, Setting, SettingSource};
        let info = StatusInfo {
            catalog: "SALES_ANALYTICS".into(),
            cube: "Sales".into(),
            pool_size: 8,
            started_at_unix: 1,
            data: DataStamp {
                path: "x.duckdb".into(),
                size_bytes: 10,
                mtime_unix: 2,
                loaded_at_unix: 3,
            },
            result_cache: true,
            engine: EngineSettings {
                memory_limit: Some(Setting {
                    value: MemoryLimit {
                        text: "3006477107B".into(),
                        bytes: Some(3_006_477_107),
                    },
                    source: SettingSource::Cgroup,
                }),
                temp_directory: None,
                threads: None,
            },
        };
        let json = info.to_json();
        for needle in [
            "\"catalog\":\"SALES_ANALYTICS\"",
            "\"pool_size\":8",
            "\"mtime_unix\":2",
            "\"loaded_at_unix\":3",
            "\"result_cache\":true",
            "\"memory_limit\":\"2.8 GiB\"",
            "\"memory_limit_source\":\"cgroup\"",
            "\"temp_directory\":null",
            "\"threads\":null",
            "\"threads_source\":\"default\"",
        ] {
            assert!(json.contains(needle), "missing {needle} in {json}");
        }
    }

    #[test]
    fn authenticated_only_for_admins_or_role_holders() {
        assert!(authenticated(&UserContext::admin_default()));
        assert!(!authenticated(&UserContext::deny_all()));
        let mut user = UserContext::deny_all();
        user.roles = vec!["EU".into()];
        assert!(authenticated(&user), "a role holder is authenticated");
    }
}
