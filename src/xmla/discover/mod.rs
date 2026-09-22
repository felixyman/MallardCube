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
