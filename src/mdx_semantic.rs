/// MDX parsing, extraction, and semantic classification.
///
/// Converts raw Excel MDX probe/query strings into a `SemanticQuery`
/// so that response builders don't need to touch MDX strings directly.

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

fn dim_key_str(dim: &crate::mdx_parser::DimKey) -> String {
    match dim {
        crate::mdx_parser::DimKey::Region => "Region".into(),
        crate::mdx_parser::DimKey::Produktkategori => "Produktkategori".into(),
        crate::mdx_parser::DimKey::Measures => "Measures".into(),
    }
}

pub fn parse_mdx_filters(mdx: &str) -> Vec<DimensionFilter> {
    let parsed = crate::mdx_parser::parse_mdx(mdx);
    let mut result: Vec<DimensionFilter> = Vec::new();

    let mut add_leaf = |result: &mut Vec<DimensionFilter>, dim_str: String, key: &str| {
        if let Some(df) = result.iter_mut().find(|f| f.dimension == dim_str) {
            if !df.members.contains(&key.to_string()) { df.members.push(key.to_string()); }
        } else {
            result.push(DimensionFilter { dimension: dim_str, members: vec![key.to_string()] });
        }
    };

    // Collect from WHERE clause
    for m in &parsed.where_members {
        if let crate::mdx_parser::MemberRef::Leaf { dim, key } = m {
            add_leaf(&mut result, dim_key_str(dim), key);
        }
    }

    // Also collect from ALL subquery nests (don't short-circuit)
    for m in &parsed.subquery_members {
        if let crate::mdx_parser::MemberRef::Leaf { dim, key } = m {
            add_leaf(&mut result, dim_key_str(dim), key);
        }
    }

    result
}

pub fn parse_slicer_dimensions(mdx: &str) -> Vec<SlicerSelection> {
    let parsed = crate::mdx_parser::parse_mdx(mdx);
    let mut result = Vec::new();
    for mref in &parsed.where_members {
        match mref {
            crate::mdx_parser::MemberRef::All(dim) => {
                result.push(SlicerSelection { dimension: dim_key_str(dim), is_all: true });
            }
            crate::mdx_parser::MemberRef::Leaf { dim, .. } => {
                let dim_str = dim_key_str(dim);
                if !result.iter().any(|s: &SlicerSelection| s.dimension == dim_str) {
                    result.push(SlicerSelection { dimension: dim_str, is_all: false });
                }
            }
            _ => {}
        }
    }
    result
}

// ---- utilities ----

pub fn includes_prop(props: &[String], name: &str) -> bool {
    props.iter().any(|prop| prop == name)
}

// ---- cChildren probe helpers (string-based, used by classification) ----

pub fn cchildren_target_is_measures(mdx: &str) -> bool {
    if let Some(start) = mdx.find("FilteredMembers As '") {
        let after_open = &mdx[start + "FilteredMembers As '".len()..];
        if let Some(end) = after_open.find('\'') {
            let set = &after_open[..end];
            return set.contains("[Measures]") && !set.contains("[Produktkategori]");
        }
    }
    false
}

pub fn cchildren_target_is_product_leaf(mdx: &str) -> bool {
    if let Some(start) = mdx.find("FilteredMembers As '") {
        let after_open = &mdx[start + "FilteredMembers As '".len()..];
        if let Some(end) = after_open.find('\'') {
            let set = &after_open[..end];
            return set.contains("[Produktkategori]") && (set.contains("&[") || set.contains("&amp;["));
        }
    }
    false
}

pub fn cchildren_filtered_member_name(mdx: &str) -> Option<String> {
    let key_start = mdx.find("FilteredMembers As '")?;
    let after_open = &mdx[key_start + "FilteredMembers As '".len()..];
    let set_end = after_open.find('\'')?;
    let set = &after_open[..set_end];
    if let Some(amp_start) = set.find("&[") {
        let begin = amp_start + 2;
        let end = set[begin..].find(']')? + begin;
        return Some(set[begin..end].to_string());
    }
    if let Some(amp_start) = set.find("&amp;[") {
        let begin = amp_start + 5;
        let end = set[begin..].find(']')? + begin;
        return Some(set[begin..end].to_string());
    }
    None
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
pub struct SemanticQuery {
    pub kind: SemanticQueryKind,
    pub dim_props: Vec<String>,
    pub cell_props: Vec<String>,
    /// Dimension-tagged filter members.
    pub filters: Vec<DimensionFilter>,
    pub cchildren_leaf_name: Option<String>,
    /// Visible dimension on the query axis, or None for slicer-only.
    pub row_dimension: Option<String>,
    /// All visible dimensions on the query axis in order (1 for simple, 2+ for CrossJoin).
    pub axis_dimensions: Vec<String>,
    /// Off-axis dimensions from the WHERE clause (including All selections).
    pub slicers: Vec<SlicerSelection>,
    /// Excluded member keys for DrilldownMember collapse.
    pub excluded_members: Vec<String>,
    /// Target hierarchy for DrilldownMember (e.g. "Region" or "Produktkategori").
    pub drilldown_member_hierarchy: Option<String>,
}

fn row_dimension_from_mdx(mdx: &str) -> Option<&'static str> {
    let select_end = mdx.find("FROM [Model]").unwrap_or(mdx.len());
    if mdx[..select_end].contains("[Region]") {
        Some("Region")
    } else if mdx[..select_end].contains("[Produktkategori]") {
        Some("Produktkategori")
    } else {
        None
    }
}

fn parse_axis_dimensions(mdx: &str) -> Vec<String> {
    let mut result = Vec::new();
    // Only consider the axis expression, not DIMENSION PROPERTIES.
    // Extract between the last "})" before FROM or the first "DIMENSION PROPERTIES".
    let from_pos = mdx.find("FROM [Model]").unwrap_or(mdx.len());
    let select_part = &mdx[..from_pos];
    // Remove the DIMENSION PROPERTIES section to only look at the axis expr
    let axis_expr_end = select_part.find("DIMENSION PROPERTIES").unwrap_or(select_part.len());
    let axis_expr = &select_part[..axis_expr_end];

    for dim in &["Produktkategori", "Region"] {
        // Only count if the dimension appears in the axis expression itself
        // (DrilldownLevel, CrossJoin, Hierarchize)
        if axis_expr.contains(dim) || axis_expr.contains(&format!("[{dim}]")) {
            result.push(dim.to_string());
        }
    }
    result
}

pub fn semantic_query_from_mdx(mdx: &str) -> SemanticQuery {
    let upper = mdx.to_uppercase();
    let dim_props = parse_dimension_properties(mdx);
    let cell_props = parse_cell_properties(mdx);
    let filters = parse_mdx_filters(mdx);
    let row_dimension = row_dimension_from_mdx(mdx).map(|s| s.to_string());
    let has_axes = upper.contains("ON COLUMNS") || upper.contains("ON ROWS");
    let has_rows = upper.contains("ON ROWS");
    let has_cols = upper.contains("ON COLUMNS");
    let has_visible_dim = mdx.contains("[Produktkategori]") || mdx.contains("[Region]");
    let has_measures = mdx.contains("[Measures]");
    let is_drilldown = has_visible_dim && (mdx.contains("DrilldownLevel") || mdx.contains(".Members"));

    let kind = if mdx.contains("WITH MEMBER [Measures].cChildren") {
        if cchildren_target_is_measures(mdx) {
            SemanticQueryKind::ChildrenCountMeasures
        } else if cchildren_target_is_product_leaf(mdx) {
            SemanticQueryKind::ChildrenCountLeafProduct
        } else {
            SemanticQueryKind::ChildrenCountForAll
        }
    } else if mdx.contains("AddCalculatedMembers({[Measures].[Total Försäljning].Children})") {
        SemanticQueryKind::MeasureChildrenEmpty
    } else if (mdx.contains("AddCalculatedMembers({[Produktkategori].[Produktkategori].&[")
        || mdx.contains("AddCalculatedMembers({[Region].[Region].&["))
        && mdx.contains("].Children})")
    {
        SemanticQueryKind::LeafChildrenEmpty
    } else if mdx.contains("AddCalculatedMembers({[Produktkategori].[Produktkategori].[(All)].Members})")
        || mdx.contains("AddCalculatedMembers({[Region].[Region].[(All)].Members})")
    {
        SemanticQueryKind::AllLevelMembers
    } else if mdx.contains("AddCalculatedMembers({[Produktkategori].[Produktkategori].[All].Children})")
        || mdx.contains("AddCalculatedMembers({[Region].[Region].[All].Children})")
    {
        SemanticQueryKind::LeafLevelMembers
    } else if mdx.contains("AddCalculatedMembers({[Produktkategori].[Produktkategori].[Produktkategori].Members})")
        || mdx.contains("AddCalculatedMembers({[Region].[Region].[Region].Members})")
    {
        SemanticQueryKind::LeafLevelMembers
    } else if has_rows && has_cols && has_visible_dim && has_measures {
        SemanticQueryKind::MeasureByCategory
    } else if mdx.contains("DrilldownMember(") {
        SemanticQueryKind::DrilldownMemberProbe
    } else if is_drilldown {
        SemanticQueryKind::DrilldownCategories
    } else if !has_axes && (
        mdx.contains("WHERE ([Produktkategori].[Produktkategori].[All],[Measures].[Total Försäljning])")
            || mdx.contains("WHERE ([Region].[Region].[All],[Measures].[Total Försäljning])"))
    {
        SemanticQueryKind::SlicerAllAndMeasure
    } else if !has_axes {
        SemanticQueryKind::SlicerOnly
    } else {
        SemanticQueryKind::SlicerOnly
    };

    SemanticQuery {
        kind,
        dim_props,
        cell_props,
        filters,
        cchildren_leaf_name: cchildren_filtered_member_name(mdx),
        row_dimension,
        axis_dimensions: parse_axis_dimensions(mdx),
        slicers: parse_slicer_dimensions(mdx),
        excluded_members: parse_excluded_members(mdx),
        drilldown_member_hierarchy: parse_drilldown_member_hierarchy(mdx),
    }
}

fn parse_excluded_members(mdx: &str) -> Vec<String> {
    let mut result = Vec::new();
    let Some(excl_start) = mdx.find("{-{") else { return result; };
    let excl = &mdx[excl_start..];
    let mut search_from = 0;
    while let Some(amp) = excl[search_from..].find("&[") {
        let begin = search_from + amp + 2;
        if let Some(end) = excl[begin..].find(']') {
            result.push(excl[begin..begin + end].to_string());
            search_from = begin + end;
        } else {
            break;
        }
    }
    result
}

fn parse_drilldown_member_hierarchy(mdx: &str) -> Option<String> {
    // DrilldownMember(CrossJoin(...), {-{...}}, [Region].[Region])
    let Some(excl_start) = mdx.find("{-{") else { return None; };
    let after_excl = &mdx[excl_start..];
    // Find the closing }} of the excluded set
    let Some(close) = after_excl[2..].find("}}") else { return None; };
    let rest = &after_excl[2 + close + 2..];
    // Next bracketed hierarchy: , [Region].[Region]
    let trimmed = rest.trim_start();
    let trimmed = trimmed.strip_prefix(',').unwrap_or(trimmed).trim_start();
    if !trimmed.starts_with('[') { return None; }
    let bracket_end = trimmed[1..].find(']')?;
    let hier = &trimmed[1..bracket_end + 1];
    let hier = hier.trim_matches(|c: char| c == '[' || c == ']');
    if hier == "Region" {
        Some("Region".into())
    } else if hier == "Produktkategori" {
        Some("Produktkategori".into())
    } else {
        None
    }
}
