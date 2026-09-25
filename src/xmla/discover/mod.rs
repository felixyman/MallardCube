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
/// Do the request's catalog/cube names match the model we serve? Both the
/// restriction list (`CATALOG_NAME`/`CUBE_NAME`) and the `<Catalog>` property
/// count, names match case-insensitively, and an absent name means "the
/// session's scope" (measured 2026-09-25: the reference answers a mismatch
/// with an empty rowset for Discover and a fault for Execute).
pub(crate) fn in_scope(
    restrictions: &crate::xmla::parser::Restrictions,
    catalog: &str,
    cube: &str,
) -> bool {
    let matches = |value: &str, wanted: Option<&str>| {
        wanted.is_none_or(|wanted| value.trim().eq_ignore_ascii_case(wanted.trim()))
    };
    matches(catalog, restrictions.catalog_name.as_deref())
        && matches(cube, restrictions.cube_name.as_deref())
}

pub(crate) fn table_visible(
    config: &crate::project::config::ProxyConfig,
    user: &crate::engine::model::UserContext,
    table: &str,
) -> bool {
    crate::engine::model::effective_table_filter(config, user, table)
        != crate::engine::model::TableAccess::Hidden
}

/// Are any of the model's measures visible? The `[Measures]` hierarchy
/// (dimension, member, property and measure-group rowsets) is advertised only
/// when at least one measure's fact table is visible.
pub(crate) fn measures_visible(
    model: &crate::engine::model::SemanticModel,
    config: &crate::project::config::ProxyConfig,
    user: &crate::engine::model::UserContext,
) -> bool {
    model
        .fact_tables
        .iter()
        .any(|ft| table_visible(config, user, &ft.table_name))
}

/// The `*_VISIBILITY` restrictions: `0` answers the empty rowset, `1` (or
/// absent) answers everything (measured 2026-09-26).
pub(crate) fn hidden_by_visibility(value: Option<i32>) -> bool {
    value == Some(0)
}

/// An exact name restriction (`*_NAME`) matches case-insensitively; absent
/// means "no filter".
pub(crate) fn name_matches(wanted: Option<&str>, candidates: &[&str]) -> bool {
    let Some(wanted) = wanted else {
        return true;
    };
    candidates
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(wanted.trim()))
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

#[cfg(test)]
mod tests {
    use super::in_scope;
    use crate::xmla::parser::Restrictions;

    /// Scope names match case-insensitively, from the restriction list and the
    /// `<Catalog>` property, and an absent name means "the session's scope"
    /// (measured on the reference 2026-09-25).
    #[test]
    fn scope_names_match_case_insensitively() {
        let none = Restrictions::default();
        assert!(in_scope(&none, "Sales", "Model"));

        let exact = Restrictions {
            catalog_name: Some("Sales".into()),
            cube_name: Some("Model".into()),
            ..Default::default()
        };
        assert!(in_scope(&exact, "Sales", "Model"));

        let other_case = Restrictions {
            catalog_name: Some("sales".into()),
            cube_name: Some("MODEL".into()),
            ..Default::default()
        };
        assert!(in_scope(&other_case, "Sales", "Model"));

        let wrong_cube = Restrictions {
            cube_name: Some("Other".into()),
            ..Default::default()
        };
        assert!(!in_scope(&wrong_cube, "Sales", "Model"));

        let wrong_catalog = Restrictions {
            catalog_name: Some("Other".into()),
            ..Default::default()
        };
        assert!(!in_scope(&wrong_catalog, "Sales", "Model"));

        // The property catalog is a fault at dispatch, not an empty rowset:
        // `in_scope` only judges the restriction names (measured 2026-09-25).
        let property = Restrictions {
            property_catalog: Some("Other".into()),
            ..Default::default()
        };
        assert!(in_scope(&property, "Sales", "Model"));
    }
}
