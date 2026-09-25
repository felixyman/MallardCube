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

/// Authentication and role posture, so an operator can see whether row-level
/// security is actually in force. Without a mechanism every request is the
/// administrator — correct for a trusted single-user deployment, a silent hole
/// in any other, and previously invisible from `/status`.
///
/// "Configured" means a mechanism that can actually authenticate (the
/// trusted-proxy header, or OIDC) — not merely that an `auth` block exists. An
/// `auth` block with neither leaves requests anonymous, and
/// `build_user_context` makes those administrators; reporting `deny` there was
/// a contradiction an operator could act on (plan 051 review).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthStatus {
    /// Is an authentication mechanism configured?
    pub configured: bool,
    /// Roles the config declares.
    pub roles: usize,
    /// Do the declared roles narrow access **and** is something enforcing
    /// them? Roles without a mechanism are informational only.
    pub rls_active: bool,
    /// What an unidentified request gets: `admin` without auth, `deny` with it.
    pub anonymous: &'static str,
}

impl AuthStatus {
    pub fn from_config(config: &crate::project::config::ProxyConfig) -> Self {
        let mechanism = config
            .auth
            .as_ref()
            .is_some_and(|auth| auth.trusted_proxy || auth.oidc.is_some());
        Self {
            configured: mechanism,
            roles: config.roles.len(),
            rls_active: mechanism && config.any_role_narrows_access(),
            anonymous: if mechanism { "deny" } else { "admin" },
        }
    }
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
    /// Live cache accounting: entries, bytes and hits (plan 051-B).
    pub cache: crate::execute::cache::CacheStats,
    /// Engine settings in force and where each came from (plan 051-A/054-C).
    pub engine: EngineSettings,
    /// Authentication and role posture (plan 051 security review).
    pub auth: AuthStatus,
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
            "cache": {
                "enabled": self.cache.enabled,
                "entries": self.cache.entries,
                "bytes": self.cache.bytes,
                "max_bytes": self.cache.max_bytes,
                "hits": self.cache.hits,
                "misses": self.cache.misses,
                "hit_rate": self.cache.hit_rate(),
                "evictions": self.cache.evictions,
            },
            "data": {
                "path": self.data.path,
                "size_bytes": self.data.size_bytes,
                "mtime_unix": self.data.mtime_unix,
                "loaded_at_unix": self.data.loaded_at_unix,
            },
            "auth": {
                "configured": self.auth.configured,
                "roles": self.auth.roles,
                "rls_active": self.auth.rls_active,
                "anonymous": self.auth.anonymous,
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
                "max_concurrent_queries": self.engine.query_slots(),
                "max_concurrent_queries_source": self.engine.max_concurrent_queries.source.as_str(),
                "query_timeout_s": self.engine.query_timeout.value,
                "query_timeout_s_source": self.engine.query_timeout.source.as_str(),
                "budget": {
                    "max_members": self.engine.budget.max_members.value,
                    "max_cells": self.engine.budget.max_cells.value,
                    "max_response_bytes": self.engine.budget.max_response_bytes.value,
                },
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
            cache: Default::default(),
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
            auth: AuthStatus {
                configured: false,
                roles: 0,
                rls_active: false,
                anonymous: "admin",
            },
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
                max_concurrent_queries: Setting {
                    value: 4,
                    source: SettingSource::Default,
                },
                query_timeout: Setting {
                    value: Some(300),
                    source: SettingSource::Default,
                },
                budget: crate::engine::settings::ResponseBudget::default(),
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
            "\"max_concurrent_queries\":4",
            "\"query_timeout_s\":300",
            "\"query_timeout_s_source\":\"default\"",
            // serde_json orders keys alphabetically.
            "\"auth\":{\"anonymous\":\"admin\",\"configured\":false,\"rls_active\":false,\"roles\":0}",
            "\"max_members\":1000000",
            "\"max_cells\":2000000",
        ] {
            assert!(json.contains(needle), "missing {needle} in {json}");
        }
    }

    /// The posture must describe what is enforced, not what is declared.
    #[test]
    fn auth_status_reports_the_enforced_mode() {
        use crate::project::config::{
            AuthConfig, ModelPermission, RoleConfig, TablePermissionConfig,
        };

        let mut config = crate::proxy_project::project().config.clone();
        let filtered_role = RoleConfig {
            name: "EU".into(),
            description: String::new(),
            model_permission: ModelPermission::Read,
            members: vec![],
            table_permissions: vec![TablePermissionConfig {
                table: "sales_fact".into(),
                filter_expression: "territory = 'North'".into(),
                dax_filter: None,
                metadata_permission: ModelPermission::Read,
            }],
        };

        // No auth at all: every request is the administrator.
        let bare = AuthStatus::from_config(&config);
        assert!(!bare.configured);
        assert_eq!(bare.anonymous, "admin");
        assert!(!bare.rls_active);

        // Roles declared, nothing enforcing them: informational only.
        config.roles = vec![filtered_role.clone()];
        let declared_only = AuthStatus::from_config(&config);
        assert!(!declared_only.configured, "no mechanism");
        assert_eq!(declared_only.roles, 1);
        assert!(
            !declared_only.rls_active,
            "roles nobody enforces are not active RLS"
        );
        assert_eq!(declared_only.anonymous, "admin");

        // An auth block with no mechanism is still anonymous — say so.
        config.auth = Some(AuthConfig {
            trusted_proxy: false,
            trusted_header: "X-User".into(),
            oidc: None,
        });
        let empty_auth = AuthStatus::from_config(&config);
        assert!(
            !empty_auth.configured,
            "an auth block alone authenticates nobody"
        );
        assert_eq!(empty_auth.anonymous, "admin");

        // A mechanism plus a narrowing role: this is real RLS.
        config.auth = Some(AuthConfig {
            trusted_proxy: true,
            trusted_header: "X-User".into(),
            oidc: None,
        });
        let enforced = AuthStatus::from_config(&config);
        assert!(enforced.configured);
        assert!(enforced.rls_active);
        assert_eq!(enforced.anonymous, "deny");
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
