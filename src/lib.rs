// Public modules — grouped by architectural layer
pub mod backend;
pub mod project;
pub mod mdx;
pub mod execute;
pub mod xmla;
pub mod engine;
pub mod test_support;

// Backward-compatible re-exports — external code still uses the old flat names.
// These can be removed once all internal callers are migrated.
pub use project::config as proxy_config;
pub use project::project as proxy_project;
pub use mdx::parser as mdx_parser;
pub use mdx::semantic as mdx_semantic;
pub use test_support::fixtures as test_fixtures;
pub use execute::dispatch as execute_dispatch;
pub use execute::builders as execute_builders;
pub use execute::axis_members as axis_members;
pub use xmla::parser as parser;
pub use xmla::response as response;
pub use xmla::rowset as rowset;
pub use xmla::cellset as cellset;
pub use xmla::properties as properties;
pub use xmla::schema_rowsets as schema_rowsets;
pub use xmla::discover::catalogs as catalogs;
pub use xmla::discover::cubes as cubes;
pub use xmla::discover::tables as tables;
pub use xmla::discover::dimensions as dimensions;
pub use xmla::discover::measures as measures;
pub use xmla::discover::hierarchies as hierarchies;
pub use xmla::discover::levels as levels;
pub use xmla::discover::members as members;
pub use xmla::discover::literals as literals;
pub use xmla::discover::sets as sets;
pub use xmla::discover::kpis as kpis;
pub use xmla::discover::measure_groups as measure_groups;
pub use xmla::discover::measuregroup_dimensions as measuregroup_dimensions;
pub use xmla::discover::mdschema_properties as mdschema_properties;
pub use xmla::discover::tmschema as tmschema;
