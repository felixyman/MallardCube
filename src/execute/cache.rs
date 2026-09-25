//! Short-lived query-result cache for the Excel hot path.
//!
//! Excel sends each PivotTable query three times in a row — one per
//! `CELL PROPERTIES` variant (`VALUE`, `FORMAT_STRING/BACK_COLOR/FORE_COLOR`,
//! `CELL_ORDINAL`). All three produce the same plan and the same SQL, so
//! without a cache DuckDB executes identical work three times per user action.
//! Caching the executed [`QueryResult`] for a few seconds collapses the triple
//! into one execution; the cellset is still rendered per request, so each
//! variant keeps its own cell properties.
//!
//! The cache key includes the user scope: row-level-security predicates are
//! injected at SQL-emission time from the user's roles, so results must never
//! cross users.

use crate::engine::model::UserContext;
use crate::engine::plan::QueryResult;
use crate::project::config::RoleConfig;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

/// Excel's three variants arrive within ~50-100 ms of each other. A longer
/// TTL buys nothing and only widens the staleness window after data changes.
const TTL: Duration = Duration::from_secs(5);

/// A burst of distinct queries must not grow the cache without bound.
const CAPACITY: usize = 64;

/// Default byte budget for the cached results (overridable with
/// `MALLARDCUBE_CACHE_MAX_BYTES`; `0` keeps only the entry cap). The entry cap
/// alone is not a memory bound: one wide pivot can be hundreds of megabytes.
const DEFAULT_MAX_BYTES: usize = 64 * 1024 * 1024;

/// What `/status` reports: entries, bytes and hit accounting.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CacheStats {
    pub enabled: bool,
    pub entries: usize,
    pub bytes: usize,
    pub max_bytes: usize,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let lookups = self.hits + self.misses;
        if lookups == 0 {
            0.0
        } else {
            self.hits as f64 / lookups as f64
        }
    }
}

/// Approximate heap footprint of a cached result. Group keys are Strings and
/// dominate the wide shapes, so count their bytes plus a small per-element
/// overhead; the values themselves are 8 bytes each.
pub fn estimated_bytes(result: &QueryResult) -> usize {
    const ELEMENT: usize = 24;
    let bytes = match result {
        QueryResult::Scalar(_) | QueryResult::Count(_) => 16,
        QueryResult::Empty => 0,
        QueryResult::Multi(values) => values.len() * 8 + ELEMENT,
        QueryResult::Grouped(rows) => rows.iter().map(|(key, _)| key.len() + 16 + ELEMENT).sum(),
        QueryResult::Pairs(rows) => rows
            .iter()
            .map(|(first, second, _)| first.len() + second.len() + 24 + ELEMENT)
            .sum(),
        QueryResult::MultiGrouped(rows) => rows
            .iter()
            .map(|(key, values)| key.len() + values.len() * 8 + ELEMENT)
            .sum(),
        QueryResult::MultiGrouped2(rows) => rows
            .iter()
            .map(|(first, second, values)| first.len() + second.len() + values.len() * 8 + ELEMENT)
            .sum(),
        QueryResult::MultiGroupedN(rows) => rows
            .iter()
            .map(|(keys, values)| {
                keys.iter().map(|key| key.len() + ELEMENT).sum::<usize>()
                    + values.len() * 8
                    + ELEMENT
            })
            .sum(),
    };
    bytes.max(16)
}

struct Entry {
    /// Shared, not cloned: a hit used to deep-clone the whole result — on a
    /// 422k-group shape that is ~1.3M String allocations on two of every three
    /// Excel requests (plan 051 review).
    result: Arc<QueryResult>,
    inserted_at: Instant,
    /// Approximate footprint, for the byte budget and `/status`.
    bytes: usize,
}

/// Byte budget for the cache. `MALLARDCUBE_CACHE_MAX_BYTES=0` disables the
/// byte bound (the entry cap still applies).
fn max_bytes_from_env() -> usize {
    std::env::var("MALLARDCUBE_CACHE_MAX_BYTES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_MAX_BYTES)
}

/// Cache of executed query results, keyed by plan + user scope.
pub struct ResultCache {
    ttl: Duration,
    capacity: usize,
    max_bytes: usize,
    entries: Mutex<HashMap<String, Entry>>,
    bytes_used: AtomicUsize,
    hits: AtomicU64,
    misses: AtomicU64,
    evictions: AtomicU64,
}

impl ResultCache {
    fn new() -> Self {
        Self::with_byte_limit(TTL, CAPACITY, max_bytes_from_env())
    }

    #[cfg(test)]
    fn with_limits(ttl: Duration, capacity: usize) -> Self {
        Self::with_byte_limit(ttl, capacity, DEFAULT_MAX_BYTES)
    }

    fn with_byte_limit(ttl: Duration, capacity: usize, max_bytes: usize) -> Self {
        Self {
            ttl,
            capacity,
            max_bytes,
            entries: Mutex::new(HashMap::new()),
            bytes_used: AtomicUsize::new(0),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
        }
    }

    /// Return a fresh cached result; expired entries are dropped on access.
    /// A poisoned lock degrades to a miss (the request still executes).
    pub fn get(&self, key: &str) -> Option<Arc<QueryResult>> {
        let mut entries = self.entries.lock().ok()?;
        match entries.get(key) {
            Some(entry) if entry.inserted_at.elapsed() < self.ttl => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                Some(Arc::clone(&entry.result))
            }
            Some(_) => {
                if let Some(entry) = entries.remove(key) {
                    self.bytes_used.fetch_sub(entry.bytes, Ordering::Relaxed);
                }
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    /// Store a result. Evicts expired entries first, then the oldest, until
    /// both the entry cap and the byte budget have room.
    pub fn insert(&self, key: String, result: Arc<QueryResult>) {
        let bytes = estimated_bytes(&result);
        if self.max_bytes > 0 && bytes > self.max_bytes {
            // A single result larger than the whole budget would evict
            // everything else and still not fit: do not cache it at all.
            return;
        }
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        if let Some(previous) = entries.remove(&key) {
            self.bytes_used.fetch_sub(previous.bytes, Ordering::Relaxed);
        }
        loop {
            let used = self.bytes_used.load(Ordering::Relaxed);
            let has_room = entries.len() < self.capacity
                && (self.max_bytes == 0 || used + bytes <= self.max_bytes);
            if has_room {
                break;
            }
            let expired: Vec<String> = entries
                .iter()
                .filter(|(_, entry)| entry.inserted_at.elapsed() >= self.ttl)
                .map(|(key, _)| key.clone())
                .collect();
            if !expired.is_empty() {
                for key in expired {
                    if let Some(entry) = entries.remove(&key) {
                        self.bytes_used.fetch_sub(entry.bytes, Ordering::Relaxed);
                        self.evictions.fetch_add(1, Ordering::Relaxed);
                    }
                }
                continue;
            }
            let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, entry)| entry.inserted_at)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some(entry) = entries.remove(&oldest) {
                self.bytes_used.fetch_sub(entry.bytes, Ordering::Relaxed);
                self.evictions.fetch_add(1, Ordering::Relaxed);
            }
        }
        self.bytes_used.fetch_add(bytes, Ordering::Relaxed);
        entries.insert(
            key,
            Entry {
                result,
                inserted_at: Instant::now(),
                bytes,
            },
        );
    }

    /// Entries, bytes and hit accounting for `/status`.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            enabled: enabled(),
            entries: self
                .entries
                .lock()
                .map(|entries| entries.len())
                .unwrap_or(0),
            bytes: self.bytes_used.load(Ordering::Relaxed),
            max_bytes: self.max_bytes,
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
        }
    }

    /// Drop every entry. Called on a data reload so no request can serve
    /// pre-reload rows.
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
            self.bytes_used.store(0, Ordering::Relaxed);
        }
    }
}

/// Process-wide cache for the execution path.
pub static RESULT_CACHE: LazyLock<ResultCache> = LazyLock::new(ResultCache::new);

/// Whether the cache is active. `MALLARDCUBE_RESULT_CACHE=0` disables it
/// (A/B benchmarks, or a deployment that wants zero staleness).
pub fn enabled() -> bool {
    std::env::var("MALLARDCUBE_RESULT_CACHE")
        .map(|v| v != "0")
        .unwrap_or(true)
}

/// Cache key for a plan under a user context. The catalog/cube scope keeps
/// projects apart (tests run several models in one process; a server serves
/// one), and roles/groups/the administrator flag are part of the key because
/// they determine the RLS predicates in the emitted SQL — two users must never
/// share an entry.
///
/// The matched roles' whole *definitions* are in the key too, not just their
/// names: the predicates follow the permissions, and two configs can reuse a
/// role name with different permissions (the tests do; a reload swaps the
/// config in-process until the cache is cleared).
pub fn cache_key(
    plan_key: &str,
    catalog: &str,
    cube: &str,
    user: &UserContext,
    roles_config: &[RoleConfig],
) -> String {
    let mut roles = user.roles.clone();
    roles.sort();
    let mut groups = user.groups.clone();
    groups.sort();
    let mut matched: Vec<&RoleConfig> = roles_config
        .iter()
        .filter(|role| user.roles.iter().any(|name| name == &role.name))
        .collect();
    matched.sort_by(|a, b| a.name.cmp(&b.name));
    format!(
        "{catalog}|{cube}|{plan_key}|user={}|admin={}|roles={}|groups={}|cfg={matched:?}",
        user.user_id,
        user.is_administrator,
        roles.join(","),
        groups.join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar(v: f64) -> Arc<QueryResult> {
        Arc::new(QueryResult::Scalar(v))
    }

    /// Two configs can reuse a role name with different permissions (the test
    /// suite does); the emitted predicates follow the definitions, so the keys
    /// must differ.
    #[test]
    fn role_definitions_are_part_of_the_key() {
        use crate::project::config::{ModelPermission, TablePermissionConfig};

        let mut user = UserContext::deny_all();
        user.roles = vec!["OLS".into()];
        let role = |permission: ModelPermission| RoleConfig {
            name: "OLS".into(),
            description: String::new(),
            model_permission: ModelPermission::Read,
            members: vec![],
            table_permissions: vec![TablePermissionConfig {
                table: "sales".into(),
                filter_expression: String::new(),
                dax_filter: None,
                metadata_permission: permission,
            }],
        };
        let hidden = [role(ModelPermission::None)];
        let visible = [role(ModelPermission::Read)];
        assert_ne!(
            cache_key("p", "cat", "cube", &user, &hidden),
            cache_key("p", "cat", "cube", &user, &visible)
        );
    }

    #[test]
    fn hit_returns_the_stored_result() {
        let cache = ResultCache::with_limits(Duration::from_secs(60), 8);
        assert!(cache.get("k").is_none());
        cache.insert("k".into(), scalar(1.5));
        assert_eq!(cache.get("k").as_deref(), Some(&QueryResult::Scalar(1.5)));
    }

    #[test]
    fn expired_entry_is_a_miss() {
        let cache = ResultCache::with_limits(Duration::ZERO, 8);
        cache.insert("k".into(), scalar(1.0));
        assert!(
            cache.get("k").is_none(),
            "a zero TTL means the entry is stale immediately"
        );
    }

    #[test]
    fn capacity_evicts_the_oldest_entry() {
        let cache = ResultCache::with_limits(Duration::from_secs(60), 2);
        cache.insert("a".into(), scalar(1.0));
        std::thread::sleep(Duration::from_millis(2));
        cache.insert("b".into(), scalar(2.0));
        std::thread::sleep(Duration::from_millis(2));
        cache.insert("c".into(), scalar(3.0));
        assert!(cache.get("a").is_none(), "oldest entry evicted");
        assert_eq!(cache.get("b").as_deref(), Some(&QueryResult::Scalar(2.0)));
        assert_eq!(cache.get("c").as_deref(), Some(&QueryResult::Scalar(3.0)));
    }

    /// A hit must hand back the *same* allocation: the old code deep-cloned the
    /// whole result, which on a wide pivot is millions of allocations per
    /// repeated request (plan 051 review).
    #[test]
    fn a_hit_shares_the_result_instead_of_cloning_it() {
        let cache = ResultCache::with_limits(Duration::from_secs(60), 8);
        let stored = scalar(2.5);
        cache.insert("k".into(), Arc::clone(&stored));
        let hit = cache.get("k").expect("hit");
        let hit_again = cache.get("k").expect("hit");
        assert!(Arc::ptr_eq(&stored, &hit), "same allocation, not a copy");
        assert!(Arc::ptr_eq(&hit, &hit_again));
    }

    /// One wide result must not blow through the budget: an entry larger than
    /// the whole budget is not cached, and smaller ones evict to fit.
    #[test]
    fn the_byte_budget_bounds_the_cache() {
        let cache = ResultCache::with_byte_limit(Duration::from_secs(60), 64, 600);
        let wide = Arc::new(QueryResult::Grouped(
            (0..1_000)
                .map(|i| (format!("group-{i:04}"), i as f64))
                .collect(),
        ));
        assert!(estimated_bytes(&wide) > 600);
        cache.insert("wide".into(), Arc::clone(&wide));
        assert!(
            cache.get("wide").is_none(),
            "oversized results are not cached"
        );

        let small = |i: usize| {
            Arc::new(QueryResult::Grouped(vec![(
                format!("group-{i:04}"),
                i as f64,
            )]))
        };
        for i in 0..20 {
            cache.insert(format!("k{i}"), small(i));
        }
        let stats = cache.stats();
        assert!(stats.bytes <= 600, "budget holds: {} bytes", stats.bytes);
        assert!(stats.entries < 20, "evictions made room: {stats:?}");
        assert!(stats.evictions > 0, "{stats:?}");
    }

    #[test]
    fn stats_count_hits_and_misses() {
        let cache = ResultCache::with_limits(Duration::from_secs(60), 8);
        assert!(cache.get("k").is_none());
        cache.insert("k".into(), scalar(1.0));
        assert!(cache.get("k").is_some());
        assert!(cache.get("k").is_some());
        let stats = cache.stats();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.entries, 1);
        assert!((stats.hit_rate() - 2.0 / 3.0).abs() < 1e-9);
        cache.clear();
        assert_eq!(cache.stats().bytes, 0);
    }

    #[test]
    fn clear_drops_every_entry() {
        let cache = ResultCache::with_limits(Duration::from_secs(60), 8);
        cache.insert("a".into(), scalar(1.0));
        assert!(cache.get("a").is_some());
        cache.clear();
        assert!(cache.get("a").is_none(), "a reload must drop stale rows");
    }

    #[test]
    fn cache_key_separates_users_and_is_group_order_stable() {
        let mut alice = UserContext::admin_default();
        alice.user_id = "alice".into();
        alice.roles = vec!["EU".into(), "Finance".into()];
        alice.groups = vec!["g1".into(), "g2".into()];

        let mut alice_again = alice.clone();
        alice_again.roles.reverse();
        alice_again.groups.reverse();

        let mut bob = UserContext::admin_default();
        bob.user_id = "bob".into();
        bob.roles = alice.roles.clone();

        assert_eq!(
            cache_key("p", "cat", "cube", &alice, &[]),
            cache_key("p", "cat", "cube", &alice_again, &[])
        );
        assert_ne!(
            cache_key("p", "cat", "cube", &alice, &[]),
            cache_key("p", "cat", "cube", &bob, &[])
        );
        assert_ne!(
            cache_key("p", "cat", "cube", &alice, &[]),
            cache_key("p", "other", "cube", &alice, &[]),
            "different projects must not share entries"
        );
    }
}
