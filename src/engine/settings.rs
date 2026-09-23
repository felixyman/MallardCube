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
//!    [`CGROUP_MEMORY_FRACTION`] of the pod limit.
//! 3. nothing — the engine's own default (a share of host RAM).
//!
//! `temp_directory` and `threads` are environment-only; their defaults stay
//! with the engine. Settings are resolved once per process (the pool calls
//! [`effective`]) and reported by `GET /status`, including where each value
//! came from.
//!
//! Note: until plan 051-A lands the shared-engine change, each pooled DuckDB
//! connection is its own instance, so the ceiling is per connection rather
//! than process-wide. The shared engine is what makes it a hard bound.

use std::path::PathBuf;
use std::sync::OnceLock;

/// Where an effective setting came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingSource {
    /// A `MALLARDCUBE_*` environment variable.
    Env,
    /// The container's cgroup memory limit.
    Cgroup,
    /// The engine's built-in default.
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

/// cgroup v1 reports "unlimited" as the page-aligned i64 maximum.
const CGROUP_V1_UNLIMITED: u64 = 0x7FFF_FFFF_FFFF_F000;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct EngineSettings {
    pub memory_limit: Option<Setting<MemoryLimit>>,
    pub temp_directory: Option<Setting<PathBuf>>,
    pub threads: Option<Setting<usize>>,
}

impl EngineSettings {
    /// Resolve from the process environment and the container's cgroup.
    pub fn resolve() -> Self {
        Self::resolve_from(&env_var, read_cgroup_limit())
    }

    /// Pure resolution, for tests.
    pub fn resolve_from(env: &dyn Fn(&str) -> Option<String>, cgroup_bytes: Option<u64>) -> Self {
        let memory_limit = resolve_memory_limit(env, cgroup_bytes);
        let temp_directory = resolve_temp_directory(env);
        let threads = resolve_threads(env);
        Self {
            memory_limit,
            temp_directory,
            threads,
        }
    }
}

fn resolve_memory_limit(
    env: &dyn Fn(&str) -> Option<String>,
    cgroup_bytes: Option<u64>,
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
        let capped = (bytes as f64 * CGROUP_MEMORY_FRACTION) as u64;
        return Some(Setting {
            value: MemoryLimit {
                text: format!("{capped}B"),
                bytes: Some(capped),
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
    fn container_limit_is_a_fraction_of_the_cgroup_value() {
        let settings = EngineSettings::resolve_from(&env_from(&[]), Some(4 * GIB));
        let limit = settings
            .memory_limit
            .expect("cgroup limit yields a setting");
        assert_eq!(limit.source, SettingSource::Cgroup);
        let bytes = limit.value.bytes.expect("derived limits carry bytes");
        assert_eq!(bytes, (4.0 * GIB as f64 * CGROUP_MEMORY_FRACTION) as u64);
        // The engine parses the byte-suffixed form (plain integers are rejected).
        assert!(limit.value.text.ends_with('B'));
        assert_eq!(parse_bytes(&limit.value.text), Some(bytes));
    }

    #[test]
    fn env_overrides_the_cgroup_and_keeps_the_user_text() {
        let env = env_from(&[("MALLARDCUBE_MEMORY_LIMIT", "8GiB")]);
        let settings = EngineSettings::resolve_from(&env, Some(2 * GIB));
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
        let settings = EngineSettings::resolve_from(&env, Some(4 * GIB));
        assert_eq!(
            settings.memory_limit.map(|s| s.source),
            Some(SettingSource::Cgroup),
            "a bad override does not discard the container limit"
        );
        assert!(settings.threads.is_none(), "a bad thread count is ignored");

        let bare = EngineSettings::resolve_from(&env_from(&[]), None);
        assert!(
            bare.memory_limit.is_none(),
            "no cgroup, no override: engine default"
        );
        assert_eq!(bare.memory_limit.as_ref().map(|s| s.source), None);
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
        let limit = EngineSettings::resolve_from(&env, Some(4 * GIB))
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
        let settings = EngineSettings::resolve_from(&env, None);
        let threads = settings.threads.expect("threads");
        assert_eq!(threads.value, 4);
        assert_eq!(threads.source, SettingSource::Env);
        let temp = settings.temp_directory.expect("temp dir");
        assert_eq!(temp.value, dir);
        let _ = std::fs::remove_dir_all(&temp.value);

        assert!(
            EngineSettings::resolve_from(&env_from(&[("MALLARDCUBE_THREADS", "0")]), None)
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
