//! Per-dimension member dictionaries (plan 031).
//!
//! MDSCHEMA_MEMBERS and drilldown rendering ask the same questions over and
//! over: the All member's child count, the distinct leaf values, the distinct
//! paths per hierarchy level, and the child count under a given path. On a wide
//! model a field-list open issues one to three queries per dimension and a
//! drilldown one query per axis member; this cache answers them from memory
//! after the first build.
//!
//! Scope and safety:
//!
//! - One cache per [`SemanticModel`], so tests that load different projects or
//!   use different backends can never share entries.
//! - Row-level-security users (filtered table access) bypass the cache and
//!   query directly: cached values are unfiltered, and a per-role dictionary is
//!   a follow-up.
//! - A data reload must call [`DimCache::clear`], since paths and counts depend
//!   on the data.

use crate::backend::QueryBackend;
use crate::engine::model::{DimensionDef, SemanticModel};
use crate::engine::plan::DimId;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Member dictionary for one dimension.
pub struct DimMembers {
    /// Distinct values at the first level — the All member's direct children.
    pub all_cardinality: u32,
    /// Distinct leaf values (flat dimensions; empty for leveled ones).
    pub leaf_values: Vec<String>,
    /// Leveled dimensions: per level, the distinct paths (one part per level,
    /// e.g. `["2024"]`, `["2024", "1"]`, `["2024", "1", "15"]`).
    pub level_paths: Vec<Vec<Vec<String>>>,
}

impl DimMembers {
    /// Distinct member count at `level_idx` (a `dim.levels` index).
    pub fn path_count(&self, level_idx: usize) -> Option<u32> {
        self.level_paths
            .get(level_idx)
            .map(|paths| paths.len() as u32)
    }

    /// Number of children of the member at `level_idx` identified by
    /// `key_path` (pipe-separated ancestor keys, e.g. `2024|1`). Zero for the
    /// deepest level.
    pub fn child_count(&self, level_idx: usize, key_path: &str) -> u32 {
        let Some(next) = self.level_paths.get(level_idx + 1) else {
            return 0;
        };
        let parts: Vec<&str> = key_path.split('|').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return 0;
        }
        let take = parts.len().min(level_idx + 1);
        next.iter()
            .filter(|path| {
                path.len() >= take
                    && path[..take]
                        .iter()
                        .map(|s| s.as_str())
                        .eq(parts[..take].iter().copied())
            })
            .count() as u32
    }
}

/// Lazily built member dictionaries, one per dimension.
#[derive(Default)]
pub struct DimCache {
    entries: RwLock<HashMap<DimId, Arc<DimMembers>>>,
}

impl DimCache {
    /// Dictionary for `dim`, built on first use.
    pub fn get<B: QueryBackend + ?Sized>(
        &self,
        model: &SemanticModel,
        dim: &DimensionDef,
        backend: &B,
    ) -> Arc<DimMembers> {
        if let Ok(entries) = self.entries.read()
            && let Some(hit) = entries.get(&dim.id)
        {
            return hit.clone();
        }
        let built = Arc::new(build(model, dim, backend));
        if let Ok(mut entries) = self.entries.write() {
            entries.insert(dim.id.clone(), built.clone());
        }
        built
    }

    /// Drop every dictionary — a data reload must not serve stale members.
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.write() {
            entries.clear();
        }
    }
}

fn build<B: QueryBackend + ?Sized>(
    model: &SemanticModel,
    dim: &DimensionDef,
    backend: &B,
) -> DimMembers {
    let table = model.dim_table_for_discovery(&dim.id);
    if dim.levels.is_empty() {
        let leaf_values = query_leaf_values(backend, dim, table, "");
        let all_cardinality = leaf_values.len() as u32;
        return DimMembers {
            all_cardinality,
            leaf_values,
            level_paths: Vec::new(),
        };
    }
    let level_paths = query_level_paths(backend, dim, table, "");
    let all_cardinality = level_paths.first().map(|p| p.len() as u32).unwrap_or(0);
    DimMembers {
        all_cardinality,
        leaf_values: Vec::new(),
        level_paths,
    }
}

/// Distinct leaf values for a flat dimension. `where_sql` is an optional
/// row-level-security predicate (empty for the cached, unfiltered dictionary).
pub fn query_leaf_values<B: QueryBackend + ?Sized>(
    backend: &B,
    dim: &DimensionDef,
    table: &str,
    where_sql: &str,
) -> Vec<String> {
    let filter = if where_sql.is_empty() {
        String::new()
    } else {
        format!(" WHERE {where_sql}")
    };
    backend.query_strings(&format!(
        "SELECT DISTINCT {} FROM {}{} ORDER BY {}",
        dim.physical_field, table, filter, dim.physical_field
    ))
}

/// Distinct paths per hierarchy level. `where_sql` is an optional
/// row-level-security predicate (empty for the cached, unfiltered dictionary).
pub fn query_level_paths<B: QueryBackend + ?Sized>(
    backend: &B,
    dim: &DimensionDef,
    table: &str,
    where_sql: &str,
) -> Vec<Vec<Vec<String>>> {
    let filter = if where_sql.is_empty() {
        String::new()
    } else {
        format!(" WHERE {where_sql}")
    };
    (0..dim.levels.len())
        .map(|i| {
            let exprs: Vec<String> = dim.levels[..=i]
                .iter()
                .map(|l| format!("CAST({} AS VARCHAR)", l.column))
                .collect();
            let concat = if exprs.len() == 1 {
                exprs[0].clone()
            } else {
                format!("({})", exprs.join(" || '|' || "))
            };
            backend
                .query_strings(&format!(
                    "SELECT DISTINCT {concat} FROM {table}{filter} ORDER BY 1"
                ))
                .into_iter()
                .map(|s| s.split('|').map(|p| p.to_string()).collect())
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::backend::Backend;
    use crate::test_support::counting::Counting;

    fn project3() -> crate::project::project::ProxyProject {
        crate::project::project::ProxyProject::load("projects/project3/proxy-config.json")
            .expect("load project3")
    }

    #[test]
    fn flat_dimension_matches_direct_queries() {
        let project = project3();
        let backend = Backend::test_fixture();
        crate::project::project::with_test_project(project, || {
            let model = &crate::proxy_project::project().model;
            let dim = model.dim_def_opt("Territory").expect("Territory dim");
            let members = model.dim_cache.get(model, dim, backend);
            let direct = backend
                .query_strings("SELECT DISTINCT territory FROM sales_fact ORDER BY territory");
            assert_eq!(members.leaf_values, direct);
            assert_eq!(members.all_cardinality as usize, direct.len());
            assert!(members.level_paths.is_empty(), "flat dim has no levels");
        });
    }

    #[test]
    fn leveled_dimension_matches_direct_queries() {
        let project = project3();
        let backend = Backend::test_fixture();
        crate::project::project::with_test_project(project, || {
            let model = &crate::proxy_project::project().model;
            let dim = model.dim_def_opt("Date").expect("Date dim");
            let members = model.dim_cache.get(model, dim, backend);
            assert_eq!(members.level_paths.len(), dim.levels.len());
            // Every level's paths are non-empty and each deeper level refines
            // the previous one (children share their parent's prefix).
            for (i, paths) in members.level_paths.iter().enumerate() {
                assert!(!paths.is_empty(), "level {i} has members");
                assert!(paths.iter().all(|p| p.len() == i + 1));
            }
            // Child counts agree with a direct COUNT(DISTINCT).
            let quarters_in_2024 = members.child_count(0, "2024");
            let direct = backend.query_count(
                "SELECT COUNT(DISTINCT quarter) FROM date_dim WHERE CAST(year AS VARCHAR) = '2024'",
            );
            assert_eq!(quarters_in_2024, direct, "quarter count under 2024");
            assert_eq!(
                members.child_count(3, "2024|1|15"),
                0,
                "leaf has no children"
            );
            assert_eq!(members.path_count(0), Some(11), "demo years");
        });
    }

    #[test]
    fn second_lookup_is_served_from_memory() {
        let project = project3();
        crate::project::project::with_test_project(project, || {
            let model = &crate::proxy_project::project().model;
            let inner = Backend::test_fixture();
            let backend = Counting::new(&inner);
            let dim = model.dim_def_opt("Territory").expect("Territory dim");
            let first = model.dim_cache.get(model, dim, &backend);
            let after_build = backend.calls();
            assert!(after_build > 0, "the first lookup queries the backend");
            let second = model.dim_cache.get(model, dim, &backend);
            assert_eq!(
                backend.calls(),
                after_build,
                "the second lookup must not query"
            );
            assert_eq!(first.leaf_values, second.leaf_values);
        });
    }

    /// Wiring proof: a full MDSCHEMA_MEMBERS response is query-free once the
    /// dictionaries are warm (plan 031).
    #[test]
    fn members_response_is_query_free_after_warmup() {
        let project = project3();
        crate::project::project::with_test_project(project, || {
            let project = crate::proxy_project::project();
            let inner = Backend::test_fixture();
            let backend = Counting::new(&inner);
            let user = crate::engine::model::UserContext::admin_default();
            let config = &project.config;

            let first = crate::xmla::discover::members::get_members_response_with_backend(
                None,
                None,
                &crate::xmla::parser::Restrictions::default(),
                &backend,
                &user,
                config,
            );
            let after_first = backend.calls();
            assert!(
                after_first > 0,
                "the first response builds the dictionaries"
            );

            let second = crate::xmla::discover::members::get_members_response_with_backend(
                None,
                None,
                &crate::xmla::parser::Restrictions::default(),
                &backend,
                &user,
                config,
            );
            assert_eq!(
                backend.calls(),
                after_first,
                "the second response must issue no metadata queries"
            );
            // Compare the SOAP bodies: the envelope carries a per-request
            // session id, the rowset itself must be identical.
            let body = |xml: &str| xml.split("<soap:Body>").nth(1).unwrap_or("").to_string();
            assert_eq!(body(&first), body(&second), "and it must be identical");
        });
    }

    #[test]
    fn clear_forces_a_rebuild() {
        let project = project3();
        crate::project::project::with_test_project(project, || {
            let model = &crate::proxy_project::project().model;
            let inner = Backend::test_fixture();
            let backend = Counting::new(&inner);
            let dim = model.dim_def_opt("Territory").expect("Territory dim");
            model.dim_cache.get(model, dim, &backend);
            let after_build = backend.calls();
            model.dim_cache.clear();
            model.dim_cache.get(model, dim, &backend);
            assert!(
                backend.calls() > after_build,
                "a cleared cache must rebuild (data reload correctness)"
            );
        });
    }
}
