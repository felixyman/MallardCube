/// MDX parsing, extraction, and semantic classification.
///
/// Converts raw Excel MDX probe/query strings into a `SemanticQuery`
/// so that response builders don't need to touch MDX strings directly.
///
/// Classification is now driven by `ParsedMdx` — structural flags
/// set by the nom parser — instead of bare `contains(...)` chains.

use crate::mdx_parser::{
    ParsedMdx, MemberRef, DimRef,
    CChildrenTarget, CalculatedMembersPat,
};

pub fn is_dax(statement: &str) -> bool {
    let trimmed = statement.trim_start();
    let upper = trimmed.to_uppercase();
    upper.starts_with("EVALUATE") || upper.starts_with("DEFINE")
}

pub fn is_mdx_select(mdx: &str) -> bool {
    let trimmed = mdx.trim_start();
    let upper = trimmed.to_uppercase();
    upper.starts_with("SELECT")
        || (upper.starts_with("WITH") && upper.contains("SELECT "))
}

// ---- reusable dimension/measure identifiers ----

pub const PRODUKTKATEGORI_HIER: &str = "[Produktkategori].[Produktkategori]";
pub const PRODUKTKATEGORI_ALL_U: &str = "[Produktkategori].[Produktkategori].[All]";
pub const PRODUKTKATEGORI_ALL_L: &str = "[Produktkategori].[Produktkategori].[(All)]";
pub const PRODUKTKATEGORI_LEAF_L: &str = "[Produktkategori].[Produktkategori].[Produktkategori]";
pub const MEASURES_HIER: &str = "[Measures]";
pub const MEASURES_LEVEL: &str = "[Measures].[MeasuresLevel]";

pub const REGION_HIER: &str = "[Region].[Region]";
pub const REGION_ALL_U: &str = "[Region].[Region].[All]";
pub const REGION_ALL_L: &str = "[Region].[Region].[(All)]";
pub const REGION_LEAF_L: &str = "[Region].[Region].[Region]";

pub const PRODUKTKATEGORI_PROP_NAMES: &[&str] = &[
    "PARENT_UNIQUE_NAME",
    "HIERARCHY_UNIQUE_NAME",
    "MEMBER_NAME",
    "MEMBER_KEY",
    "MEMBER_TYPE",
    "MEMBER_VALUE",
    "PARENT_LEVEL",
    "PARENT_COUNT",
    "CHILDREN_CARDINALITY",
];

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

    let mut add_leaf = |result: &mut Vec<DimensionFilter>, dim_str: String, key: &str| {
        if let Some(df) = result.iter_mut().find(|f| f.dimension == dim_str) {
            if !df.members.contains(&key.to_string()) { df.members.push(key.to_string()); }
        } else {
            result.push(DimensionFilter { dimension: dim_str, members: vec![key.to_string()] });
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
                result.push(SlicerSelection { dimension: dim_ref_str(dim), is_all: true });
            }
            MemberRef::Leaf { dim, .. } => {
                let dim_str = dim_ref_str(dim);
                if !result.iter().any(|s: &SlicerSelection| s.dimension == dim_str) {
                    result.push(SlicerSelection { dimension: dim_str, is_all: false });
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
}

fn row_dimension_from_mdx(mdx: &str) -> Option<String> {
    let project = crate::proxy_project::project();
    let select_end = mdx.find(&format!("FROM [{}]", project.config.cube))
        .or_else(|| mdx.find("FROM [Model]"))
        .unwrap_or(mdx.len());
    let select_part = &mdx[..select_end];
    for dim in &project.model.dimensions {
        if select_part.contains(&format!("[{}]", dim.id)) {
            return Some(dim.id.clone());
        }
    }
    None
}

fn parse_axis_dimensions(mdx: &str) -> Vec<String> {
    let mut result = Vec::new();
    let project = crate::proxy_project::project();
    let from_pos = mdx.find(&format!("FROM [{}]", project.config.cube))
        .or_else(|| mdx.find("FROM [Model]"))
        .unwrap_or(mdx.len());
    let select_part = &mdx[..from_pos];
    let axis_expr_end = select_part.find("DIMENSION PROPERTIES").unwrap_or(select_part.len());
    let axis_expr = &select_part[..axis_expr_end];

    // Collect all configured dimension IDs and scan for them in the axis
    // expression, preserving positional order.
    let project = crate::proxy_project::project();
    let mut positions: Vec<(usize, String)> = project.model.dimensions.iter()
        .filter_map(|d| axis_expr.find(&format!("[{0}]", d.id)).map(|p| (p, d.id.clone())))
        .collect();
    positions.sort_by_key(|(p, _)| *p);

    for (_, dim) in positions {
        result.push(dim);
    }
    result
}

// ---- main classification entry point ----

pub fn semantic_query_from_mdx(mdx: &str) -> SemanticQuery {
    let parsed = crate::mdx_parser::parse_mdx(mdx);

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
                } else if parsed.has_rows && parsed.has_cols && parsed.main_dim != DimRef::Measures && parsed.has_measures {
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

    SemanticQuery {
        kind,
        dim_props: parsed.dim_props.clone(),
        cell_props: parsed.cell_props.clone(),
        filters: filters_from_parsed(&parsed),
        cchildren_leaf_name,
        row_dimension: row_dimension_from_mdx(mdx),
        axis_dimensions: parse_axis_dimensions(mdx),
        slicers: slicers_from_parsed(&parsed),
        excluded_members: parse_excluded_members(mdx),
        drilldown_member_hierarchy: parse_drilldown_member_hierarchy(mdx),
        measure: parsed.selected_measure.clone(),
    }
}

fn parse_excluded_members(mdx: &str) -> Vec<ExcludedMember> {
    let model = &crate::proxy_project::project().model;
    let default_dim = model.default_dimension_id()
        .unwrap_or_else(|| "Produktkategori".into());
    let mut result = Vec::new();
    let Some(excl_start) = mdx.find("{-{") else { return result; };
    let excl = &mdx[excl_start..];
    let mut search_from = 0;
    while let Some(amp) = excl[search_from..].find("&[") {
        let begin = search_from + amp + 2;
        if let Some(end) = excl[begin..].find(']') {
            let key = excl[begin..begin + end].to_string();
            // Look backwards from the &[ to find the preceding [Dimension]
            let before = &excl[..search_from + amp];
            let dim = if let Some(last_dot) = before.rfind("].") {
                if let Some(open) = before[..last_dot].rfind('[') {
                    let raw = &before[open + 1..last_dot];
                    model.lookup_dimension(raw)
                        .map(|d| d.id.clone())
                        .unwrap_or_else(|| raw.to_string())
                } else { default_dim.clone() }
            } else { default_dim.clone() };
            result.push(ExcludedMember { dimension: dim, key });
            search_from = begin + end;
        } else {
            break;
        }
    }
    result
}

fn parse_drilldown_member_hierarchy(mdx: &str) -> Option<String> {
    let Some(excl_start) = mdx.find("{-{") else { return None; };
    let after_excl = &mdx[excl_start..];
    let Some(close) = after_excl[2..].find("}}") else { return None; };
    let rest = &after_excl[2 + close + 2..];
    let trimmed = rest.trim_start();
    let trimmed = trimmed.strip_prefix(',').unwrap_or(trimmed).trim_start();
    if !trimmed.starts_with('[') { return None; }
    let bracket_end = trimmed[1..].find(']')?;
    let hier = &trimmed[1..bracket_end + 1];
    let hier = hier.trim_matches(|c: char| c == '[' || c == ']');
    let model = &crate::proxy_project::project().model;
    model.lookup_dimension(hier).map(|d| d.id.clone())
}
