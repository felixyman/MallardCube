/// Public modules — grouped by architectural layer.
///
/// New code should import from these grouped paths:
///   use crate::project::config as proxy_config;
///   use crate::project::project as proxy_project;
///   use crate::mdx::semantic as mdx_semantic;
///   etc.
pub mod auth;
pub mod backend;
pub mod engine;
pub mod execute;
pub mod mdx;
pub mod project;
pub mod test_support;
pub mod tools;
pub mod xmla;
pub mod xmla_trace;

pub use execute::axis_members;
pub use execute::builders as execute_builders;
pub use execute::dispatch as execute_dispatch;
pub use mdx::parser as mdx_parser;
pub use mdx::semantic as mdx_semantic;
/// Backward-compatible flat re-exports.
///
/// These exist so existing internal callers don't break.  New code
/// should prefer the grouped paths listed above.
/// When all internal callers migrate, remove this block.
pub use project::config as proxy_config;
pub use project::project as proxy_project;
pub use test_support::fixtures as test_fixtures;
pub use xmla::cellset;
pub use xmla::discover::catalogs;
pub use xmla::discover::cubes;
pub use xmla::discover::datasources;
pub use xmla::discover::dimensions;
pub use xmla::discover::enumerators;
pub use xmla::discover::hierarchies;
pub use xmla::discover::keywords;
pub use xmla::discover::kpis;
pub use xmla::discover::levels;
pub use xmla::discover::literals;
pub use xmla::discover::mdschema_properties;
pub use xmla::discover::measure_groups;
pub use xmla::discover::measuregroup_dimensions;
pub use xmla::discover::measures;
pub use xmla::discover::members;
pub use xmla::discover::sets;
pub use xmla::discover::tables;
pub use xmla::discover::tmschema;
pub use xmla::parser;
pub use xmla::properties;
pub use xmla::response;
pub use xmla::rowset;
pub use xmla::schema_rowsets;
