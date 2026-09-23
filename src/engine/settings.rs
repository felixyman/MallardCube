//! Engine settings (plan 051-A / 054-C).
//!
//! The limits the proxy hands to the SQL engine. The important one is the
//! memory ceiling: inside a container the pod's cgroup limit is the truth, so
//! the default is derived from it instead of from the host's RAM (DuckDB's own
//! default is a share of the host, which is wrong on Kubernetes and can get the
//! process OOM-killed).
//!
//! Resolution order:
//!
//! 1. `MALLARDCUBE_MEMORY_LIMIT` — passed to the engine verbatim (`4GiB`,
//!    `4GB`, `4294967296B`, `80%`); an unparsable value is ignored with a
//!    warning rather than failing startup.
//! 2. the container's cgroup limit (`memory.max` on v2,
//!    `memory/memory.limit_in_bytes` on v1), capped at
//!    [`CGROUP_MEMORY_FRACTION`] of the pod limit **and divided between the
//!    query slots** — the semaphore is what makes that division a bound, so
//!    `slots × limit ≤ fraction × pod limit`.
//! 3. nothing — the engine's own default (a share of host RAM).
//!
//! `temp_directory`, `threads`, `max_concurrent_queries`, and `query_timeout`
//! are environment-only. Settings are resolved once per process (the pool
//! calls [`effective`]) and reported by `GET /status`, including where each
//! value came from.
//!
//! Note: DuckDB's Rust binding does not expose a shared `Database`, so each
//! pooled connection is still its own engine instance. The query-slot
//! semaphore plus the divided ceiling is the bounded substitute (plan 051-A).

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

/// Where an effective setting came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingSource {
    /// A `MALLARDCUBE_*` environment variable.
    Env,
    /// The container's cgroup memory limit.
    Cgroup,
    /// The built-in default.
    Default,
}

impl SettingSource {
    pub fn as_str(self) -> &'static str {
        match self {
            SettingSource::Env => "env",
            SettingSource::Cgroup => "cgroup",
            SettingSource::Default => "default",
        }
    }
}

/// A resolved setting and its origin.
#[derive(Debug, Clone, PartialEq)]
pub struct Setting<T> {
    pub value: T,
    pub source: SettingSource,
}

/// Engine-facing memory ceiling: `text` is what the engine parses, `bytes` is
/// for display when we derived it ourselves (an `80%` value has no byte count).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryLimit {
    pub text: String,
    pub bytes: Option<u64>,
}

impl MemoryLimit {
    /// Human-readable form for `/status` (`2.8 GiB`, or the raw text).
    pub fn display(&self) -> String {
        match self.bytes {
            Some(bytes) => human_bytes(bytes),
            None => self.text.clone(),
        }
    }
}

/// Fraction of the cgroup limit handed to the engine. The remainder covers the
/// proxy's own allocations — dimension dictionaries, response buffers, the
/// result cache — which live outside DuckDB's budget.
const CGROUP_MEMORY_FRACTION: f64 = 0.7;

/// Default number of heavy-query slots per core.
const QUERY_SLOTS_PER_CORE: usize = 4;

/// Default request timeout. No request should be able to hang forever; `0`
/// disables the limit for anyone running deliberately long batches.
const DEFAULT_QUERY_TIMEOUT_S: u64 = 300;

/// cgroup v1 reports "unlimited" as the page-aligned i64 maximum.
const CGROUP_V1_UNLIMITED: u64 = 0x7FFF_FFFF_FFFF_F000;

#[derive(Debug, Clone, PartialEq)]
pub struct EngineSettings {
    pub memory_limit: Option<Setting<MemoryLimit>>,
    pub temp_directory: Option<Setting<PathBuf>>,
    pub threads: Option<Setting<usize>>,
    /// How many requests may run engine queries at once. Bounds concurrency
    /// *and* the memory each one may use.
    pub max_concurrent_queries: Setting<usize>,
    /// Per-request engine timeout; `None` = no limit.
    pub query_timeout: Setting<Option<u64>>,
}

impl EngineSettings {
    /// Resolve from the process environment and the container's cgroup.
    pub fn resolve() -> Self {
        let parallelism = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        Self::resolve_from(&env_var, read_cgroup_limit(), parallelism)
    }

    /// Pure resolution, for tests (`parallelism` stands in for the CPU count).
    pub fn resolve_from(
        env: &dyn Fn(&str) -> Option<String>,
        cgroup_bytes: Option<u64>,
        parallelism: usize,
    ) -> Self {
        let max_concurrent_queries = resolve_query_slots(env, parallelism);
        let memory_limit = resolve_memory_limit(env, cgroup_bytes, max_concurrent_queries.value);
        let temp_directory = resolve_temp_directory(env);
        let threads = resolve_threads(env);
        let query_timeout = resolve_query_timeout(env);
        Self {
            memory_limit,
            temp_directory,
            threads,
            max_concurrent_queries,
            query_timeout,
        }
    }

    /// Requests allowed to run engine queries at once.
    pub fn query_slots(&self) -> usize {
        self.max_concurrent_queries.value
    }

    /// Per-request engine timeout, if one is configured.
    pub fn timeout(&self) -> Option<Duration> {
        self.query_timeout.value.map(Duration::from_secs)
    }
}

fn resolve_query_slots(env: &dyn Fn(&str) -> Option<String>, parallelism: usize) -> Setting<usize> {
    if let Some(raw) = env("MALLARDCUBE_MAX_CONCURRENT_QUERIES") {
        match raw.parse::<usize>() {
            Ok(n) if n >= 1 => {
                return Setting {
                    value: n,
                    source: SettingSource::Env,
                };
            }
            _ => eprintln!(
                "⚠️  MALLARDCUBE_MAX_CONCURRENT_QUERIES={raw:?} is not a positive integer — ignoring it"
            ),
        }
    }
    Setting {
        value: (parallelism / QUERY_SLOTS_PER_CORE).max(1),
        source: SettingSource::Default,
    }
}

fn resolve_query_timeout(env: &dyn Fn(&str) -> Option<String>) -> Setting<Option<u64>> {
    if let Some(raw) = env("MALLARDCUBE_QUERY_TIMEOUT_S") {
        match raw.parse::<u64>() {
            Ok(0) => {
                return Setting {
                    value: None,
                    source: SettingSource::Env,
                };
            }
            Ok(secs) => {
                return Setting {
                    value: Some(secs),
                    source: SettingSource::Env,
                };
            }
            Err(_) => eprintln!(
                "⚠️  MALLARDCUBE_QUERY_TIMEOUT_S={raw:?} is not a number of seconds — ignoring it"
            ),
        }
    }
    Setting {
        value: Some(DEFAULT_QUERY_TIMEOUT_S),
        source: SettingSource::Default,
    }
}

fn resolve_memory_limit(
    env: &dyn Fn(&str) -> Option<String>,
    cgroup_bytes: Option<u64>,
    slots: usize,
) -> Option<Setting<MemoryLimit>> {
    if let Some(text) = env("MALLARDCUBE_MEMORY_LIMIT") {
        if engine_accepts_memory(&text) {
            return Some(Setting {
                value: MemoryLimit {
                    bytes: parse_bytes(&text),
                    text,
                },
                source: SettingSource::Env,
            });
        }
        eprintln!(
            "⚠️  MALLARDCUBE_MEMORY_LIMIT={text:?} is not a DuckDB memory value \
             (expected e.g. 4GiB, 4GB, 4294967296B, or 80%) — ignoring it"
        );
    }
    if let Some(bytes) = cgroup_bytes.filter(|b| *b > 0) {
        let per_slot = (bytes as f64 * CGROUP_MEMORY_FRACTION / slots.max(1) as f64) as u64;
        return Some(Setting {
            value: MemoryLimit {
                text: format!("{per_slot}B"),
                bytes: Some(per_slot),
            },
            source: SettingSource::Cgroup,
        });
    }
    None
}

fn resolve_temp_directory(env: &dyn Fn(&str) -> Option<String>) -> Option<Setting<PathBuf>> {
    let value = PathBuf::from(env("MALLARDCUBE_TEMP_DIR")?);
    if !value.exists() {
        // The engine only needs this when a query spills; creating it now
        // keeps a mid-query failure out of the picture.
        if let Err(e) = std::fs::create_dir_all(&value) {
            eprintln!(
                "⚠️  MALLARDCUBE_TEMP_DIR={} could not be created ({e}) — ignoring it",
                value.display()
            );
            return None;
        }
    }
    Some(Setting {
        value,
        source: SettingSource::Env,
    })
}

fn resolve_threads(env: &dyn Fn(&str) -> Option<String>) -> Option<Setting<usize>> {
    let raw = env("MALLARDCUBE_THREADS")?;
    match raw.parse::<usize>() {
        Ok(n) if n >= 1 => Some(Setting {
            value: n,
            source: SettingSource::Env,
        }),
        _ => {
            eprintln!("⚠️  MALLARDCUBE_THREADS={raw:?} is not a positive integer — ignoring it");
            None
        }
    }
}

/// Does DuckDB accept this as a `memory_limit` value? Byte quantities in any
/// of its unit spellings, or a percentage of the detected limit.
fn engine_accepts_memory(text: &str) -> bool {
    if let Some(pct) = text.strip_suffix('%') {
        return pct.trim().parse::<f64>().is_ok();
    }
    parse_bytes(text).is_some()
}

/// Parse `4GiB`, `4GB`, `4.5MiB`, `4294967296B`, or plain bytes. Decimal (KB)
/// and binary (KiB) units follow DuckDB's own parsing.
pub fn parse_bytes(text: &str) -> Option<u64> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    let (digits, multiplier) = [
        ("kib", 1u64 << 10),
        ("mib", 1 << 20),
        ("gib", 1 << 30),
        ("tib", 1 << 40),
        ("kb", 1_000),
        ("mb", 1_000_000),
        ("gb", 1_000_000_000),
        ("tb", 1_000_000_000_000),
        ("b", 1),
    ]
    .into_iter()
    .find_map(|(unit, mult)| lower.strip_suffix(unit).map(|d| (d, mult)))
    .unwrap_or((lower.as_str(), 1));
    let value: f64 = digits.trim().parse().ok()?;
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    Some((value * multiplier as f64) as u64)
}

/// `2.8 GiB`-style formatting for `/status`.
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// The container's memory limit, if one is set and finite.
pub fn read_cgroup_limit() -> Option<u64> {
    read_cgroup_v2("/sys/fs/cgroup/memory.max")
        .or_else(|| read_cgroup_v1("/sys/fs/cgroup/memory/memory.limit_in_bytes"))
}

fn read_cgroup_v2(path: &str) -> Option<u64> {
    parse_cgroup_v2(&std::fs::read_to_string(path).ok()?)
}

fn read_cgroup_v1(path: &str) -> Option<u64> {
    parse_cgroup_v1(&std::fs::read_to_string(path).ok()?)
}

fn parse_cgroup_v2(text: &str) -> Option<u64> {
    let t = text.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("max") {
        return None;
    }
    t.parse::<u64>().ok().filter(|v| *v > 0)
}

fn parse_cgroup_v1(text: &str) -> Option<u64> {
    let v = text.trim().parse::<u64>().ok()?;
    if v == 0 || v >= CGROUP_V1_UNLIMITED {
        return None;
    }
    Some(v)
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

static CURRENT: OnceLock<EngineSettings> = OnceLock::new();

/// Resolve once per process and remember the result. The pool calls this
/// before opening connections; everything else reads [`current`].
pub fn effective() -> EngineSettings {
    CURRENT.get_or_init(EngineSettings::resolve).clone()
}

/// The settings this process resolved, if the pool has opened yet.
pub fn current() -> Option<&'static EngineSettings> {
    CURRENT.get()
}

/// Configured per-request engine timeout (used by the request handler).
pub fn timeout() -> Option<Duration> {
    current().and_then(|s| s.timeout())
}

/// Configured heavy-query slots (used to size the request semaphore).
pub fn query_slots() -> usize {
    current().map(|s| s.query_slots()).unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env_from(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name: &str| map.get(name).cloned()
    }

    const GIB: u64 = 1 << 30;

    #[test]
    fn container_limit_is_divided_between_the_query_slots() {
        // 16 cores → 4 slots; 4 GiB pod → 70% of it, quartered.
        let settings = EngineSettings::resolve_from(&env_from(&[]), Some(4 * GIB), 16);
        assert_eq!(settings.query_slots(), 4);
        let limit = settings
            .memory_limit
            .expect("cgroup limit yields a setting");
        assert_eq!(limit.source, SettingSource::Cgroup);
        let bytes = limit.value.bytes.expect("derived limits carry bytes");
        assert_eq!(
            bytes,
            (4.0 * GIB as f64 * CGROUP_MEMORY_FRACTION / 4.0) as u64
        );
        // The engine parses the byte-suffixed form (plain integers are rejected).
        assert!(limit.value.text.ends_with('B'));
        assert_eq!(parse_bytes(&limit.value.text), Some(bytes));
    }

    #[test]
    fn env_overrides_the_cgroup_and_keeps_the_user_text() {
        let env = env_from(&[("MALLARDCUBE_MEMORY_LIMIT", "8GiB")]);
        let settings = EngineSettings::resolve_from(&env, Some(2 * GIB), 16);
        let limit = settings.memory_limit.expect("env limit");
        assert_eq!(limit.source, SettingSource::Env);
        assert_eq!(limit.value.text, "8GiB", "engine-ready text passes through");
        assert_eq!(limit.value.bytes, Some(8 * GIB), "bytes are for display");
        assert_eq!(limit.value.display(), "8.0 GiB");
    }

    #[test]
    fn invalid_env_values_fall_back_rather_than_failing() {
        let env = env_from(&[
            ("MALLARDCUBE_MEMORY_LIMIT", "lots"),
            ("MALLARDCUBE_THREADS", "many"),
        ]);
        let settings = EngineSettings::resolve_from(&env, Some(4 * GIB), 16);
        assert_eq!(
            settings.memory_limit.map(|s| s.source),
            Some(SettingSource::Cgroup),
            "a bad override does not discard the container limit"
        );
        assert!(settings.threads.is_none(), "a bad thread count is ignored");

        let bare = EngineSettings::resolve_from(&env_from(&[]), None, 16);
        assert!(
            bare.memory_limit.is_none(),
            "no cgroup, no override: engine default"
        );
    }

    #[test]
    fn query_slots_default_to_a_quarter_of_the_cores() {
        let slots =
            |cores: usize| EngineSettings::resolve_from(&env_from(&[]), None, cores).query_slots();
        assert_eq!(slots(16), 4);
        assert_eq!(slots(8), 2);
        assert_eq!(slots(4), 1);
        assert_eq!(slots(1), 1, "at least one slot");

        let env = env_from(&[("MALLARDCUBE_MAX_CONCURRENT_QUERIES", "2")]);
        let settings = EngineSettings::resolve_from(&env, Some(4 * GIB), 16);
        assert_eq!(settings.query_slots(), 2);
        assert_eq!(settings.max_concurrent_queries.source, SettingSource::Env);
        // The memory division follows the override, not the core count.
        assert_eq!(
            settings.memory_limit.unwrap().value.bytes,
            Some((4.0 * GIB as f64 * CGROUP_MEMORY_FRACTION / 2.0) as u64)
        );

        let bad = env_from(&[("MALLARDCUBE_MAX_CONCURRENT_QUERIES", "0")]);
        assert_eq!(
            EngineSettings::resolve_from(&bad, None, 16).query_slots(),
            4,
            "zero is not a valid slot count"
        );
    }

    #[test]
    fn query_timeout_defaults_to_five_minutes_and_zero_disables_it() {
        let default = EngineSettings::resolve_from(&env_from(&[]), None, 16);
        assert_eq!(default.timeout(), Some(Duration::from_secs(300)));
        assert_eq!(default.query_timeout.source, SettingSource::Default);

        let disabled = EngineSettings::resolve_from(
            &env_from(&[("MALLARDCUBE_QUERY_TIMEOUT_S", "0")]),
            None,
            16,
        );
        assert_eq!(disabled.timeout(), None);
        assert_eq!(disabled.query_timeout.source, SettingSource::Env);

        let custom = EngineSettings::resolve_from(
            &env_from(&[("MALLARDCUBE_QUERY_TIMEOUT_S", "45")]),
            None,
            16,
        );
        assert_eq!(custom.timeout(), Some(Duration::from_secs(45)));
    }

    #[test]
    fn percent_and_bytes_forms_are_accepted() {
        assert!(engine_accepts_memory("80%"));
        assert!(engine_accepts_memory("4GiB"));
        assert!(engine_accepts_memory("4GB"));
        assert!(engine_accepts_memory("4 GB"), "DuckDB accepts spaced units");
        assert!(engine_accepts_memory("4294967296B"));
        assert!(engine_accepts_memory("4294967296"));
        assert!(!engine_accepts_memory("4 gigabytes"));
        assert!(!engine_accepts_memory("lots"));
        assert!(!engine_accepts_memory(""));
    }

    #[test]
    fn percent_values_keep_the_raw_text() {
        let env = env_from(&[("MALLARDCUBE_MEMORY_LIMIT", "80%")]);
        let limit = EngineSettings::resolve_from(&env, Some(4 * GIB), 16)
            .memory_limit
            .expect("percent is a valid engine value");
        assert_eq!(limit.source, SettingSource::Env);
        assert_eq!(limit.value.bytes, None);
        assert_eq!(limit.value.display(), "80%");
    }

    #[test]
    fn cgroup_parsing_matches_both_layouts() {
        assert_eq!(parse_cgroup_v2("2147483648\n"), Some(2 * GIB));
        assert_eq!(parse_cgroup_v2("max"), None, "v2 unlimited");
        assert_eq!(parse_cgroup_v2(""), None);
        assert_eq!(parse_cgroup_v2("0"), None);
        assert_eq!(parse_cgroup_v1("2147483648"), Some(2 * GIB));
        assert_eq!(
            parse_cgroup_v1("9223372036854771712"),
            None,
            "v1 unlimited sentinel"
        );
        assert_eq!(parse_cgroup_v1("0"), None);
    }

    #[test]
    fn threads_and_temp_dir_come_from_the_environment() {
        let dir = std::env::temp_dir().join(format!("mallardcube-set-{}", std::process::id()));
        let env = env_from(&[
            ("MALLARDCUBE_THREADS", "4"),
            ("MALLARDCUBE_TEMP_DIR", dir.to_str().unwrap()),
        ]);
        let settings = EngineSettings::resolve_from(&env, None, 16);
        let threads = settings.threads.expect("threads");
        assert_eq!(threads.value, 4);
        assert_eq!(threads.source, SettingSource::Env);
        let temp = settings.temp_directory.expect("temp dir");
        assert_eq!(temp.value, dir);
        let _ = std::fs::remove_dir_all(&temp.value);

        assert!(
            EngineSettings::resolve_from(&env_from(&[("MALLARDCUBE_THREADS", "0")]), None, 16)
                .threads
                .is_none(),
            "zero threads is not a valid override"
        );
    }

    #[test]
    fn human_bytes_uses_binary_units() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(1024), "1.0 KiB");
        assert_eq!(human_bytes(3 * GIB / 2), "1.5 GiB");
    }

    #[test]
    fn parse_bytes_handles_both_unit_systems() {
        assert_eq!(parse_bytes("1KiB"), Some(1024));
        assert_eq!(parse_bytes("1.5GiB"), Some(GIB + GIB / 2));
        assert_eq!(parse_bytes("1GB"), Some(1_000_000_000));
        assert_eq!(parse_bytes("4096B"), Some(4096));
        assert_eq!(parse_bytes("4096"), Some(4096));
        assert_eq!(parse_bytes("not-a-size"), None);
    }
}
