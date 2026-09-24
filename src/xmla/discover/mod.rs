pub mod catalogs;
pub mod cubes;
pub mod datasources;
pub mod dimensions;
pub mod enumerators;
pub mod functions;
pub mod hierarchies;
pub mod keywords;
pub mod kpis;
pub mod levels;
pub mod literals;
pub mod mdschema_properties;
pub mod measure_groups;
pub mod measuregroup_dimensions;
pub mod measures;
pub mod members;
pub mod sets;
pub mod tables;
pub mod tmschema;

/// Does a `(dimension, hierarchy, level)` coordinate satisfy a Discover
/// request's restriction list? Discover responses must honour these: Excel
/// asks for one hierarchy (or level) at a time while it builds a pivot cache,
/// and rows for other hierarchies corrupt the cache field (plan 048/049).
/// May this user see discover rows backed by `table`? An OLS-hidden table
/// disappears from the metadata rowsets, as it does in the reference (plan 051
/// review). Unlisted tables stay visible — measured: a role listing two tables
/// still sees all six dimensions.
pub(crate) fn table_visible(
    config: &crate::project::config::ProxyConfig,
    user: &crate::engine::model::UserContext,
    table: &str,
) -> bool {
    crate::engine::model::effective_table_filter(config, user, table)
        != crate::engine::model::TableAccess::Hidden
}

pub(crate) fn dimension_visible(
    model: &crate::engine::model::SemanticModel,
    config: &crate::project::config::ProxyConfig,
    user: &crate::engine::model::UserContext,
    dim_id: &str,
) -> bool {
    table_visible(config, user, model.dim_table_for_discovery(dim_id))
}

pub(crate) fn coordinates_match(
    restrictions: &crate::xmla::parser::Restrictions,
    dim: &str,
    hier: Option<&str>,
    level: Option<&str>,
) -> bool {
    let eq = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    if let Some(d) = &restrictions.dimension_unique_name
        && !eq(d, dim)
    {
        return false;
    }
    if let Some(h) = &restrictions.hierarchy_unique_name
        && !hier.is_some_and(|x| eq(h, x))
    {
        return false;
    }
    if let Some(l) = &restrictions.level_unique_name
        && !level.is_some_and(|x| eq(l, x))
    {
        return false;
    }
    true
}
