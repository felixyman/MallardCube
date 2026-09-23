//! Test-only backend wrapper that counts queries.
//!
//! Used to prove "no work was done" properties: a cache hit issues no queries
//! (plan 031), and a Discover restriction that matches no dimension must not
//! touch the engine at all (plan 051-D).

use crate::backend::{Backend, QueryBackend};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Wraps a backend and counts every query that reaches it.
pub struct Counting<'a> {
    inner: &'a Backend,
    calls: AtomicUsize,
}

impl<'a> Counting<'a> {
    pub fn new(inner: &'a Backend) -> Self {
        Self {
            inner,
            calls: AtomicUsize::new(0),
        }
    }

    /// Queries observed so far.
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }
}

impl QueryBackend for Counting<'_> {
    fn query_scalar(&self, sql: &str) -> f64 {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.inner.query_scalar(sql)
    }
    fn query_grouped_1d(&self, sql: &str) -> Vec<(String, f64)> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.inner.query_grouped_1d(sql)
    }
    fn query_pairs(&self, sql: &str) -> Vec<(String, String, f64)> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.inner.query_pairs(sql)
    }
    fn query_count(&self, sql: &str) -> u32 {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.inner.query_count(sql)
    }
    fn query_strings(&self, sql: &str) -> Vec<String> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.inner.query_strings(sql)
    }
    fn query_rows(&self, sql: &str) -> Vec<Vec<String>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.inner.query_rows(sql)
    }
    fn query_column_names(&self, sql: &str) -> Vec<String> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.inner.query_column_names(sql)
    }
}
