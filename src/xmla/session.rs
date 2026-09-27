//! Process-local session registry (plan 058-B7).
//!
//! The reference issues a session id for a `BeginSession` header and faults
//! any id it did not issue ("The '<id>' session ID cannot be found. Either the
//! session does not exist or it has already expired.", measured 2026-09-27);
//! `EndSession` invalidates the id. The proxy keeps a bounded, expiring set of
//! the ids it issued — no cross-instance sharing, matching the one-engine-per-
//! instance design. A restarted proxy forgets its ids, exactly like a
//! restarted server.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The most live sessions kept; the oldest is dropped beyond this.
const CAPACITY: usize = 1024;
/// Sessions older than this expire (the reference's default is minutes).
const TTL: Duration = Duration::from_secs(30 * 60);

pub struct SessionRegistry {
    live: Mutex<VecDeque<(String, Instant)>>,
}

impl SessionRegistry {
    pub const fn new() -> Self {
        Self {
            live: Mutex::new(VecDeque::new()),
        }
    }

    /// Issue a new session id and register it.
    pub fn begin(&self) -> String {
        let id = uuid::Uuid::new_v4().to_string().to_uppercase();
        let mut live = self.live.lock().expect("session registry poisoned");
        Self::expire(&mut live);
        while live.len() >= CAPACITY {
            live.pop_front();
        }
        live.push_back((id.clone(), Instant::now()));
        id
    }

    /// Is this id one the proxy issued and still live?
    pub fn contains(&self, id: &str) -> bool {
        let mut live = self.live.lock().expect("session registry poisoned");
        Self::expire(&mut live);
        live.iter().any(|(live_id, _)| live_id == id)
    }

    /// End a session; returns whether it was live.
    pub fn end(&self, id: &str) -> bool {
        let mut live = self.live.lock().expect("session registry poisoned");
        Self::expire(&mut live);
        let before = live.len();
        live.retain(|(live_id, _)| live_id != id);
        live.len() != before
    }

    fn expire(live: &mut VecDeque<(String, Instant)>) {
        let now = Instant::now();
        while let Some((_, issued)) = live.front() {
            if now.duration_since(*issued) > TTL {
                live.pop_front();
            } else {
                break;
            }
        }
    }
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// The process-wide registry.
pub fn global() -> &'static SessionRegistry {
    static REGISTRY: SessionRegistry = SessionRegistry::new();
    &REGISTRY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issued_ids_are_live_and_end_invalidates_them() {
        let registry = SessionRegistry::new();
        let id = registry.begin();
        assert!(registry.contains(&id));
        assert!(registry.end(&id));
        assert!(!registry.contains(&id));
        // Ending an unknown id is not a live session.
        assert!(!registry.end("nope"));
        assert!(!registry.contains("nope"));
    }

    #[test]
    fn the_oldest_sessions_fall_out_at_capacity() {
        let registry = SessionRegistry::new();
        let first = registry.begin();
        for _ in 1..CAPACITY {
            let _ = registry.begin();
        }
        assert!(registry.contains(&first), "still within capacity");
        let _ = registry.begin();
        assert!(!registry.contains(&first), "the oldest is evicted");
    }
}
