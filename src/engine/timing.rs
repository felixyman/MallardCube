/// Request timing spans for analytic query performance measurement.
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimePath {
    DirectSql,
}

impl RuntimePath {
    pub fn as_str(&self) -> &str {
        match self {
            RuntimePath::DirectSql => "direct_sql",
        }
    }
}

pub struct Timings {
    pub runtime_path: RuntimePath,
    pub plan_key: String,
    /// True when the executed result came from the short-lived result cache
    /// (plan 032) instead of DuckDB.
    pub cache_hit: bool,
    pub mdx_parse_us: u64,
    pub semantic_us: u64,
    pub plan_us: u64,
    pub sql_emit_us: u64,
    pub sql_execute_us: u64,
    pub xml_render_us: u64,
    pub total_us: u64,
    total_start: Instant,
}

impl Timings {
    pub fn new(path: RuntimePath, plan_key: String, mdx_parse_us: u64, semantic_us: u64) -> Self {
        Timings {
            runtime_path: path,
            plan_key,
            cache_hit: false,
            mdx_parse_us,
            semantic_us,
            plan_us: 0,
            sql_emit_us: 0,
            sql_execute_us: 0,
            xml_render_us: 0,
            total_us: 0,
            total_start: Instant::now(),
        }
    }

    pub fn finish(&mut self) {
        self.total_us = self.total_start.elapsed().as_micros() as u64;
    }

    pub fn to_log_line(&self) -> String {
        format!(
            "TIMINGS path={} plan_key={} cache_hit={} mdx_parse={}us semantic={}us plan={}us \
             sql_emit={}us sql_execute={}us xml_render={}us total={}us",
            self.runtime_path.as_str(),
            self.plan_key,
            self.cache_hit,
            self.mdx_parse_us,
            self.semantic_us,
            self.plan_us,
            self.sql_emit_us,
            self.sql_execute_us,
            self.xml_render_us,
            self.total_us,
        )
    }
}
