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
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

/// Excel's three variants arrive within ~50-100 ms of each other. A longer
/// TTL buys nothing and only widens the staleness window after data changes.
const TTL: Duration = Duration::from_secs(5);

/// A burst of distinct queries must not grow the cache without bound.
const CAPACITY: usize = 64;

struct Entry {
    result: QueryResult,
    inserted_at: Instant,
}

/// Cache of executed query results, keyed by plan + user scope.
pub struct ResultCache {
    ttl: Duration,
    capacity: usize,
    entries: Mutex<HashMap<String, Entry>>,
}

impl ResultCache {
    fn new() -> Self {
        Self {
            ttl: TTL,
            capacity: CAPACITY,
            entries: Mutex::new(HashMap::new()),
        }
    }

    #[cfg(test)]
    fn with_limits(ttl: Duration, capacity: usize) -> Self {
        Self {
            ttl,
            capacity,
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Return a fresh cached result; expired entries are dropped on access.
    /// A poisoned lock degrades to a miss (the request still executes).
    pub fn get(&self, key: &str) -> Option<QueryResult> {
        let mut entries = self.entries.lock().ok()?;
        match entries.get(key) {
            Some(entry) if entry.inserted_at.elapsed() < self.ttl => Some(entry.result.clone()),
            Some(_) => {
                entries.remove(key);
                None
            }
            None => None,
        }
    }

    /// Store a result. When full, expired entries are dropped first, then the
    /// oldest entry.
    pub fn insert(&self, key: String, result: QueryResult) {
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        if !entries.contains_key(&key) && entries.len() >= self.capacity {
            entries.retain(|_, entry| entry.inserted_at.elapsed() < self.ttl);
            if entries.len() >= self.capacity
                && let Some(oldest) = entries
                    .iter()
                    .min_by_key(|(_, entry)| entry.inserted_at)
                    .map(|(key, _)| key.clone())
            {
                entries.remove(&oldest);
            }
        }
        entries.insert(
            key,
            Entry {
                result,
                inserted_at: Instant::now(),
            },
        );
    }
    /// Drop every entry. Called on a data reload so no request can serve
    /// pre-reload rows.
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
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
pub fn cache_key(plan_key: &str, catalog: &str, cube: &str, user: &UserContext) -> String {
    let mut roles = user.roles.clone();
    roles.sort();
    let mut groups = user.groups.clone();
    groups.sort();
    format!(
        "{catalog}|{cube}|{plan_key}|user={}|admin={}|roles={}|groups={}",
        user.user_id,
        user.is_administrator,
        roles.join(","),
        groups.join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar(v: f64) -> QueryResult {
        QueryResult::Scalar(v)
    }

    #[test]
    fn hit_returns_the_stored_result() {
        let cache = ResultCache::with_limits(Duration::from_secs(60), 8);
        assert!(cache.get("k").is_none());
        cache.insert("k".into(), scalar(1.5));
        assert_eq!(cache.get("k"), Some(scalar(1.5)));
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
        assert_eq!(cache.get("b"), Some(scalar(2.0)));
        assert_eq!(cache.get("c"), Some(scalar(3.0)));
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
            cache_key("p", "cat", "cube", &alice),
            cache_key("p", "cat", "cube", &alice_again)
        );
        assert_ne!(
            cache_key("p", "cat", "cube", &alice),
            cache_key("p", "cat", "cube", &bob)
        );
        assert_ne!(
            cache_key("p", "cat", "cube", &alice),
            cache_key("p", "other", "cube", &alice),
            "different projects must not share entries"
        );
    }
}
