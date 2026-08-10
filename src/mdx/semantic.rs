/// MDX parsing, extraction, and semantic classification.
///
/// Converts raw Excel MDX probe/query strings into a `SemanticQuery`
/// so that response builders don't need to touch MDX strings directly.
///
/// Classification is now driven by `ParsedMdx` — structural flags
/// set by the nom parser — instead of bare `contains(...)` chains.
use crate::mdx_parser::{CChildrenTarget, CalculatedMembersPat, DimRef, MemberRef, ParsedMdx};

pub fn is_dax(statement: &str) -> bool {
    let trimmed = statement.trim_start();
    let upper = trimmed.to_uppercase();
    upper.starts_with("EVALUATE") || upper.starts_with("DEFINE")
}

pub fn is_drillthrough(statement: &str) -> bool {
    let upper = statement.trim_start().to_uppercase();
    upper.starts_with("DRILLTHROUGH")
}

pub fn is_mdx_select(mdx: &str) -> bool {
    let trimmed = mdx.trim_start();
    let upper = trimmed.to_uppercase();
    upper.starts_with("SELECT") || (upper.starts_with("WITH") && upper.contains("SELECT "))
}

pub fn is_measure_metadata_probe(mdx: &str) -> bool {
    mdx.contains("strtomember(\"")
}

// ---- property parsing (delegates to nom parser) ----

pub fn parse_dimension_properties(mdx: &str) -> Vec<String> {
    crate::mdx_parser::parse_dimension_properties(mdx)
}

pub fn parse_cell_properties(mdx: &str) -> Vec<String> {
    crate::mdx_parser::parse_cell_properties(mdx)
}

// ---- filter extraction (delegates to nom parser) ----

#[derive(Debug, Clone, PartialEq)]
pub struct DimensionFilter {
    pub dimension: String,
    pub members: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlicerSelection {
    pub dimension: String,
    pub is_all: bool,
}

fn dim_ref_str(dim: &DimRef) -> String {
    match dim {
        DimRef::Measures => "Measures".into(),
        DimRef::Cube(name) => name.clone(),
    }
}

fn filters_from_parsed(parsed: &ParsedMdx) -> Vec<DimensionFilter> {
    let mut result: Vec<DimensionFilter> = Vec::new();

    let add_leaf = |result: &mut Vec<DimensionFilter>, dim_str: String, key: &str| {
        if let Some(df) = result.iter_mut().find(|f| f.dimension == dim_str) {
            if !df.members.contains(&key.to_string()) {
                df.members.push(key.to_string());
            }
        } else {
            result.push(DimensionFilter {
                dimension: dim_str,
                members: vec![key.to_string()],
            });
        }
    };

    for m in &parsed.where_members {
        if let MemberRef::Leaf { dim, key } = m {
            add_leaf(&mut result, dim_ref_str(dim), key);
        }
    }

    for m in &parsed.subquery_members {
        if let MemberRef::Leaf { dim, key } = m {
            add_leaf(&mut result, dim_ref_str(dim), key);
        }
    }

    result
}

fn slicers_from_parsed(parsed: &ParsedMdx) -> Vec<SlicerSelection> {
    let mut result = Vec::new();
    for mref in &parsed.where_members {
        match mref {
            MemberRef::All(dim) => {
                result.push(SlicerSelection {
                    dimension: dim_ref_str(dim),
                    is_all: true,
                });
            }
            MemberRef::Leaf { dim, .. } => {
                let dim_str = dim_ref_str(dim);
                if !result
                    .iter()
                    .any(|s: &SlicerSelection| s.dimension == dim_str)
                {
                    result.push(SlicerSelection {
                        dimension: dim_str,
                        is_all: false,
                    });
                }
            }
            _ => {}
        }
    }
    result
}

// ---- public wrappers (for tests and standalone calls) ----

pub fn parse_mdx_filters(mdx: &str) -> Vec<DimensionFilter> {
    filters_from_parsed(&crate::mdx_parser::parse_mdx(mdx))
}

pub fn parse_slicer_dimensions(mdx: &str) -> Vec<SlicerSelection> {
    slicers_from_parsed(&crate::mdx_parser::parse_mdx(mdx))
}

// ---- cChildren probe helpers (wrappers over parser for test compat) ----

pub fn cchildren_target_is_measures(mdx: &str) -> bool {
    matches!(
        crate::mdx_parser::parse_mdx(mdx).cchildren_target,
        CChildrenTarget::Measures,
    )
}

pub fn cchildren_target_is_product_leaf(mdx: &str) -> bool {
    matches!(
        crate::mdx_parser::parse_mdx(mdx).cchildren_target,
        CChildrenTarget::ProductLeaf(_),
    )
}

pub fn cchildren_filtered_member_name(mdx: &str) -> Option<String> {
    match crate::mdx_parser::parse_mdx(mdx).cchildren_target {
        CChildrenTarget::ProductLeaf(name) => Some(name),
        _ => None,
    }
}

// ---- utilities ----

pub fn includes_prop(props: &[String], name: &str) -> bool {
    props.iter().any(|prop| prop == name)
}

// ---- semantic query model ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticQueryKind {
    ChildrenCountForAll,
    ChildrenCountLeafProduct,
    ChildrenCountMeasures,
    SlicerAllAndMeasure,
    MeasureChildrenEmpty,
    LeafChildrenEmpty,
    AllLevelMembers,
    LeafLevelMembers,
    MeasureByCategory,
    DrilldownCategories,
    SlicerOnly,
    DrilldownMemberProbe,
    MeasureMetadataProbe,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExcludedMember {
    pub dimension: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticQuery {
    pub kind: SemanticQueryKind,
    pub dim_props: Vec<String>,
    pub cell_props: Vec<String>,
    pub filters: Vec<DimensionFilter>,
    pub cchildren_leaf_name: Option<String>,
    pub row_dimension: Option<String>,
    pub axis_dimensions: Vec<String>,
    pub slicers: Vec<SlicerSelection>,
    pub excluded_members: Vec<ExcludedMember>,
    pub drilldown_member_hierarchy: Option<String>,
    /// Explicitly requested measure from MDX (WHERE or columns).
    pub measure: Option<String>,
    /// When drilling a multi-level hierarchy, which level index to group by.
    pub drilldown_level: Option<usize>,
    /// Measure name parsed from strtomember() probe (CUBEVALUE metadata query).
    pub metadata_probe_measure: Option<String>,
    /// Requested properties: e.g. "UniqueName", "caption", "level.UniqueName".
    pub metadata_probe_properties: Vec<String>,
}

// ---- main classification entry point ----

fn extract_drill_member(mdx: &str) -> Option<(String, String, String)> {
    let pos = mdx.rfind(".&[")?;
    let prefix = &mdx[..pos];
    let bracket_open = prefix.rfind("{[")?;
    let member_ref = &mdx[bracket_open + 1..];
    let close = member_ref.find('}')?;
    let member = &member_ref[..close];
    let dim = first_bracket(member)?;
    let level = third_bracket(member).unwrap_or_default();
    let key = parse_amp_key(member)?;
    if key.is_empty() || dim.is_empty() {
        return None;
    }
    Some((dim, level, key))
}

pub(crate) fn first_bracket(s: &str) -> Option<String> {
    let rest = s.strip_prefix('[')?;
    let close = rest.find(']')?;
    Some(rest[..close].to_string())
}

fn third_bracket(s: &str) -> Option<String> {
    let mut rest = s.strip_prefix('[')?;
    // skip [Dim]
    let close = rest.find(']')?;
    rest = &rest[close + 1..];
    // skip .[Hier]
    rest = rest.strip_prefix(".[")?;
    let close = rest.find(']')?;
    rest = &rest[close + 1..];
    // extract [Level]
    rest = rest.strip_prefix(".[")?;
    let close = rest.find(']')?;
    Some(rest[..close].to_string())
}

pub(crate) fn parse_amp_key(s: &str) -> Option<String> {
    let idx = s.find(".&[")?;
    Some(s[idx + 3..].split(']').next()?.to_string())
}

fn extract_strtomember_target(mdx: &str) -> Option<String> {
    let prefix = "strtomember(\"";
    let pos = mdx.find(prefix)?;
    let rest = &mdx[pos + prefix.len()..];
    let close = rest.find('"')?;
    Some(rest[..close].to_string())
}

fn extract_strtomember_properties(mdx: &str) -> Vec<String> {
    let mut props = Vec::new();
    for part in mdx.split("MEMBER [Measures].[").skip(1) {
        if part.contains("UniqueName") && !part.contains(".level.UniqueName") {
            props.push("UniqueName".to_string());
        } else if part.contains(".level.UniqueName") {
            props.push("level.UniqueName".to_string());
        } else if part.contains("properties(\"caption\")") {
            props.push("caption".to_string());
        }
    }
    props
}

pub fn semantic_query_from_mdx(mdx: &str) -> SemanticQuery {
    let parsed = crate::mdx_parser::parse_mdx(mdx);

    if mdx.contains("strtomember(\"") {
        let measure = extract_strtomember_target(mdx);
        let props = extract_strtomember_properties(mdx);
        return SemanticQuery {
            kind: SemanticQueryKind::MeasureMetadataProbe,
            dim_props: vec![],
            cell_props: parsed.cell_props.clone(),
            filters: vec![],
            cchildren_leaf_name: None,
            row_dimension: None,
            axis_dimensions: vec![],
            slicers: vec![],
            excluded_members: vec![],
            drilldown_member_hierarchy: None,
            measure: None,
            drilldown_level: None,
            metadata_probe_measure: measure,
            metadata_probe_properties: props,
        };
    }

    let kind = if parsed.has_with_member_cchildren {
        match &parsed.cchildren_target {
            CChildrenTarget::Measures => SemanticQueryKind::ChildrenCountMeasures,
            CChildrenTarget::ProductLeaf(_) => SemanticQueryKind::ChildrenCountLeafProduct,
            _ => SemanticQueryKind::ChildrenCountForAll,
        }
    } else {
        match &parsed.calculated_members_pat {
            CalculatedMembersPat::MeasureChildrenEmpty => SemanticQueryKind::MeasureChildrenEmpty,
            CalculatedMembersPat::LeafChildrenEmpty => SemanticQueryKind::LeafChildrenEmpty,
            CalculatedMembersPat::AllLevelMembers => SemanticQueryKind::AllLevelMembers,
            CalculatedMembersPat::LeafLevelMembers => SemanticQueryKind::LeafLevelMembers,
            CalculatedMembersPat::None => {
                if parsed.has_drilldown_member {
                    SemanticQueryKind::DrilldownMemberProbe
                } else if parsed.has_drilldown || parsed.has_dot_members {
                    SemanticQueryKind::DrilldownCategories
                } else if parsed.has_rows
                    && parsed.has_cols
                    && parsed.main_dim != DimRef::Measures
                    && parsed.has_measures
                {
                    SemanticQueryKind::MeasureByCategory
                } else if !parsed.has_rows && !parsed.has_cols {
                    if parsed.has_where_all_measure {
                        SemanticQueryKind::SlicerAllAndMeasure
                    } else {
                        SemanticQueryKind::SlicerOnly
                    }
                } else {
                    SemanticQueryKind::SlicerOnly
                }
            }
        }
    };

    let cchildren_leaf_name = match &parsed.cchildren_target {
        CChildrenTarget::ProductLeaf(name) => Some(name.clone()),
        _ => None,
    };

    let project = crate::proxy_project::project();

    let mut kind = kind;
    let mut drilldown_level: Option<usize> = parsed
        .axis_dimension_ids
        .first()
        .and_then(|id| project.model.dim_def_opt(id))
        .filter(|d| !d.levels.is_empty())
        .map(|_| 0);

    let mut extra_filters: Vec<DimensionFilter> = Vec::new();

    if (parsed.has_drilldown || parsed.has_drilldown_member)
        && let Some((dim_name, level_name, key)) = extract_drill_member(mdx)
        && let Some(dim) = project.model.dim_def_opt(&dim_name)
        && let Some(level_idx) = dim.levels.iter().position(|l| l.name == level_name)
    {
        drilldown_level = Some(level_idx + 1);
        extra_filters.push(DimensionFilter {
            dimension: dim_name,
            members: vec![key],
        });
        // Route to the single-dimension drilldown renderer,
        // not the DrilldownMemberProbe 2-dimension path.
        if kind == SemanticQueryKind::DrilldownMemberProbe {
            kind = SemanticQueryKind::DrilldownCategories;
        }
    }

    let mut filters = filters_from_parsed(&parsed);
    filters.extend(extra_filters);

    SemanticQuery {
        kind,
        dim_props: parsed.dim_props.clone(),
        cell_props: parsed.cell_props.clone(),
        filters,
        cchildren_leaf_name,
        row_dimension: parsed
            .axis_dimension_ids
            .iter()
            .find(|id| project.model.dim_def_opt(id).is_some())
            .cloned(),
        axis_dimensions: parsed
            .axis_dimension_ids
            .iter()
            .filter(|id| project.model.dim_def_opt(id).is_some())
            .cloned()
            .collect(),
        slicers: slicers_from_parsed(&parsed),
        excluded_members: parsed
            .excluded_members
            .iter()
            .map(|(dim, key)| ExcludedMember {
                dimension: dim.clone(),
                key: key.clone(),
            })
            .collect(),
        drilldown_member_hierarchy: parsed.drilldown_member_hierarchy.clone(),
        measure: parsed.selected_measure.clone(),
        drilldown_level,
        metadata_probe_measure: None,
        metadata_probe_properties: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_drillmember_year() {
        let mdx = r##"SELECT NON EMPTY Hierarchize(DrilldownMember({{DrilldownLevel({[Date].[Date].[All]},,,INCLUDE_CALC_MEMBERS)}}, {[Date].[Date].[Year].&[2024]},,,INCLUDE_CALC_MEMBERS)) ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE"##;
        let r = extract_drill_member(mdx);
        assert_eq!(r, Some(("Date".into(), "Year".into(), "2024".into())));
    }

    #[test]
    fn extract_drillmember_quarter() {
        let mdx = "{[Dim].[Hier].[Quarter].&[2]}";
        let r = extract_drill_member(mdx);
        assert_eq!(r, Some(("Dim".into(), "Quarter".into(), "2".into())));
    }

    #[test]
    fn extract_drillmember_all_skips() {
        let mdx = "DrilldownLevel({[Date].[Date].[All]},,,INCLUDE_CALC_MEMBERS)";
        let r = extract_drill_member(mdx);
        assert_eq!(r, None);
    }

    #[test]
    fn extract_drillmember_no_amp() {
        let mdx = "{[Date].[Date].[Year]}";
        let r = extract_drill_member(mdx);
        assert_eq!(r, None);
    }

    #[test]
    fn first_bracket_valid() {
        assert_eq!(
            first_bracket("[Date].[Date].[Year].&[2024]"),
            Some("Date".into())
        );
    }

    #[test]
    fn first_bracket_empty() {
        assert_eq!(first_bracket("no brackets"), None);
    }

    #[test]
    fn third_bracket_level() {
        assert_eq!(
            third_bracket("[Dim].[Hier].[Level].&[key]"),
            Some("Level".into())
        );
    }

    #[test]
    fn third_bracket_no_level() {
        assert_eq!(third_bracket("[Dim].[Hier].&[key]"), None);
    }

    #[test]
    fn amp_key_normal() {
        assert_eq!(
            parse_amp_key("[Date].[Date].[Year].&[2024]"),
            Some("2024".into())
        );
    }

    #[test]
    fn amp_key_no_amp() {
        assert_eq!(parse_amp_key("[Date].[Date].[Year]"), None);
    }

    #[test]
    fn is_measure_metadata_probe_detects_strtomember() {
        assert!(is_measure_metadata_probe(
            r##"WITH MEMBER [Measures].[XL_SD0] AS 'strtomember("[Measures].[Total Försäljning]").UniqueName'"##
        ));
    }

    #[test]
    fn extract_strtomember_target_parses_brackets() {
        assert_eq!(
            extract_strtomember_target(
                r##"WITH MEMBER [Measures].[XL_SD0] AS 'strtomember("[Measures].[Total Försäljning]").UniqueName'"##
            ),
            Some("[Measures].[Total Försäljning]".into())
        );
    }

    #[test]
    fn extract_strtomember_target_parses_dim_member() {
        assert_eq!(
            extract_strtomember_target(
                r##"WITH MEMBER [Measures].[XL_SD0] AS 'strtomember("[Category].[Category].&[Kategori A]").UniqueName'"##
            ),
            Some("[Category].[Category].&[Kategori A]".into())
        );
    }

    #[test]
    fn extract_strtomember_properties_parses_all_three() {
        let mdx = r##"WITH MEMBER [Measures].[XL_SD0] AS 'strtomember("[Measures].[Total Försäljning]").UniqueName' MEMBER [Measures].[XL_SD1] AS 'strtomember("[Measures].[Total Försäljning]").properties("caption")' MEMBER [Measures].[XL_SD2] AS '{strtomember("[Measures].[Total Försäljning]")}.item(0).item(0).level.UniqueName' SELECT {[Measures].[XL_SD0],[Measures].[XL_SD1],[Measures].[XL_SD2]} ON 0 FROM"##;
        let props = extract_strtomember_properties(mdx);
        assert_eq!(props, vec!["UniqueName", "caption", "level.UniqueName"]);
    }

    #[test]
    fn semantic_query_classifies_cubevalue_metadata_probe() {
        let mdx = r##"WITH MEMBER [Measures].[XL_SD0] AS 'strtomember("[Measures].[Total Försäljning]").UniqueName' MEMBER [Measures].[XL_SD1] AS 'strtomember("[Measures].[Total Försäljning]").properties("caption")' MEMBER [Measures].[XL_SD2] AS '{strtomember("[Measures].[Total Försäljning]")}.item(0).item(0).level.UniqueName' SELECT {[Measures].[XL_SD0],[Measures].[XL_SD1],[Measures].[XL_SD2]} ON 0 FROM  CELL PROPERTIES VALUE"##;
        let q = semantic_query_from_mdx(mdx);
        assert_eq!(q.kind, SemanticQueryKind::MeasureMetadataProbe);
        assert_eq!(
            q.metadata_probe_measure,
            Some("[Measures].[Total Försäljning]".into())
        );
        assert_eq!(q.metadata_probe_properties.len(), 3);
    }
}
