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

/// Whether a value is a date-only ISO string (`2020-01-01`).
pub(crate) fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
}

/// The unique-name key the reference gives a date member: an ISO date gains a
/// `T00:00:00` suffix (`[Date].[Full Date].&[2020-01-01T00:00:00]`, measured
/// 2026-09-27).
pub(crate) fn date_member_key(value: &str) -> String {
    if is_iso_date(value) {
        format!("{value}T00:00:00")
    } else {
        value.to_string()
    }
}

/// The short-date caption the reference gives a date member in an en-US
/// session (`1/1/2020`, measured 2026-09-27).
pub(crate) fn date_member_caption(value: &str) -> String {
    if !is_iso_date(value) {
        return value.to_string();
    }
    let mut parts = value.split('-');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(year), Some(month), Some(day)) => {
            format!(
                "{}/{}/{}",
                month.trim_start_matches('0'),
                day.trim_start_matches('0'),
                year
            )
        }
        _ => value.to_string(),
    }
}

/// The value behind a date member's `T00:00:00` key form; anything else is
/// unchanged. Filters arrive with the key the metadata advertised.
pub(crate) fn date_member_value(key: &str) -> &str {
    crate::engine::model::strip_date_member_time(key)
}

/// The `*_VISIBILITY` restriction is a bitmask, not a boolean: bit 0 selects
/// visible objects, bit 1 hidden ones, and higher bits select nothing. Absent
/// means visible; negative values fault like the reference (measured
/// 2026-09-27: `1` visible, `2` hidden, `3` both, `4` none, `5` visible, `-1`
/// "Out of present range").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Visibility {
    Visible,
    Hidden,
    Both,
    None,
    Invalid,
}

pub(crate) fn visibility(value: Option<i32>) -> Visibility {
    match value {
        None => Visibility::Visible,
        Some(value) if value < 0 => Visibility::Invalid,
        Some(value) => match (value & 1 != 0, value & 2 != 0) {
            (true, true) => Visibility::Both,
            (true, false) => Visibility::Visible,
            (false, true) => Visibility::Hidden,
            (false, false) => Visibility::None,
        },
    }
}

/// Does an object with this visibility belong in the response?
pub(crate) fn visibility_selects(mode: Visibility, is_visible: bool) -> bool {
    match mode {
        Visibility::Visible => is_visible,
        Visibility::Hidden => !is_visible,
        Visibility::Both => true,
        Visibility::None | Visibility::Invalid => false,
    }
}

/// The reference's fault for a negative `*_VISIBILITY` value.
pub(crate) fn visibility_fault() -> String {
    crate::xmla::response::fault_response(
        "The following system error occurred:  Out of present range.",
    )
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
    use super::*;

    #[test]
    fn date_member_naming_matches_the_reference() {
        assert!(is_iso_date("2020-01-01"));
        assert!(!is_iso_date("2020-1-1"));
        assert!(!is_iso_date("North"));
        assert_eq!(date_member_key("2020-01-01"), "2020-01-01T00:00:00");
        assert_eq!(date_member_key("North"), "North");
        assert_eq!(date_member_caption("2020-01-01"), "1/1/2020");
        assert_eq!(date_member_caption("2020-11-30"), "11/30/2020");
        assert_eq!(date_member_caption("North"), "North");
        assert_eq!(date_member_value("2020-01-01T00:00:00"), "2020-01-01");
        assert_eq!(date_member_value("2020-01-01"), "2020-01-01");
        assert_eq!(date_member_value("North"), "North");
    }
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
