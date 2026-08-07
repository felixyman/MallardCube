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

pub fn is_mdx_select(mdx: &str) -> bool {
    let trimmed = mdx.trim_start();
    let upper = trimmed.to_uppercase();
    upper.starts_with("SELECT") || (upper.starts_with("WITH") && upper.contains("SELECT "))
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

    SemanticQuery {
        kind,
        dim_props: parsed.dim_props.clone(),
        cell_props: parsed.cell_props.clone(),
        filters: filters_from_parsed(&parsed),
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
    }
}
