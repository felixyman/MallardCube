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

// ---- clause extraction ----

fn clause_contents<'a>(mdx: &'a str, keyword: &str, terminators: &[&str]) -> Option<&'a str> {
    let upper = mdx.to_uppercase();
    let keyword_upper = keyword.to_uppercase();
    let start = upper.find(&keyword_upper)? + keyword_upper.len();

    let mut end = mdx.len();
    for term in terminators {
        let term_upper = term.to_uppercase();
        if let Some(idx) = upper[start..].find(&term_upper) {
            end = end.min(start + idx);
        }
    }

    Some(mdx[start..end].trim())
}

// ---- property parsing ----

pub fn parse_dimension_properties(mdx: &str) -> Vec<String> {
    let Some(raw) = clause_contents(
        mdx,
        "DIMENSION PROPERTIES",
        &[" ON COLUMNS", " ON ROWS", " FROM ", " CELL PROPERTIES"],
    ) else {
        return vec![];
    };

    let mut props = Vec::new();
    for token in raw.split(',') {
        let token_upper = token.trim().to_uppercase();
        for prop in PRODUKTKATEGORI_PROP_NAMES {
            if token_upper.ends_with(prop) {
                if !props.iter().any(|p| p == prop) {
                    props.push((*prop).to_string());
                }
                break;
            }
        }
    }
    props
}

pub fn parse_cell_properties(mdx: &str) -> Vec<String> {
    let Some(raw) = clause_contents(mdx, "CELL PROPERTIES", &[]) else {
        return vec![];
    };

    raw.split(',')
        .map(|token| token.trim().to_uppercase())
        .filter(|token| !token.is_empty())
        .collect()
}

// ---- filter extraction ----

/// Extract the content of `WHERE (...)` using balanced-paren scanning.
fn where_clause_payload(mdx: &str) -> Option<&str> {
    let start = mdx.find("WHERE (")?;
    let after_where = &mdx[start + "WHERE (".len()..];
    let mut depth: u32 = 1;
    let mut end = 0;
    for (i, ch) in after_where.char_indices() {
        if ch == '(' { depth += 1; }
        if ch == ')' {
            depth -= 1;
            if depth == 0 {
                end = i;
                break;
            }
        }
    }
    if end == 0 { return None; }
    Some(after_where[..end].trim())
}

/// Extract member names for a given dimension prefix from a member-set slice.
fn extract_dimension_member_names(slice: &str, dim_pattern: &str) -> Vec<String> {
    let mut names = Vec::new();
    let pattern = &format!("{}.", dim_pattern);

    let mut search_from = 0;
    while let Some(start) = slice[search_from..].find(pattern) {
        let abs_start = search_from + start;
        let after_pattern = &slice[abs_start + pattern.len()..];

        if after_pattern.starts_with("[All]") {
            search_from = abs_start + pattern.len() + "[All]".len();
            continue;
        }

        if let Some(amp_start) = after_pattern.find("&[") {
            let begin = abs_start + pattern.len() + amp_start + 2;
            if let Some(end) = slice[begin..].find(']') {
                names.push(slice[begin..begin + end].to_string());
                search_from = begin + end;
                continue;
            }
        }
        if let Some(amp_start) = after_pattern.find("&amp;[") {
            let begin = abs_start + pattern.len() + amp_start + 5;
            if let Some(end) = slice[begin..].find(']') {
                names.push(slice[begin..begin + end].to_string());
                search_from = begin + end;
                continue;
            }
        }

        search_from = abs_start + pattern.len();
    }
    names
}

/// Dimension-tagged filter members.
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionFilter {
    pub dimension: String,
    pub members: Vec<String>,
}

/// Off-axis dimension appearing in the WHERE clause.
/// Carries the dimension and whether `[All]` was selected.
#[derive(Debug, Clone, PartialEq)]
pub struct SlicerSelection {
    pub dimension: String,
    pub is_all: bool,
}

fn dimension_key_for_hier(hier: &str) -> &str {
    match hier {
        "[Region].[Region]" => "Region",
        _ => "Produktkategori",
    }
}

pub fn parse_mdx_filters(mdx: &str) -> Vec<DimensionFilter> {
    let visible_dims: &[&str] = &[PRODUKTKATEGORI_HIER, REGION_HIER];

    // 1. Check WHERE clause for dimension-tagged filters
    if let Some(where_payload) = where_clause_payload(mdx) {
        for dim in visible_dims {
            let names = extract_dimension_member_names(where_payload, dim);
            if !names.is_empty() {
                return vec![DimensionFilter {
                    dimension: dimension_key_for_hier(dim).to_string(),
                    members: names,
                }];
            }
        }
    }

    // 2. Check subquery SELECT ({...}) for filters
    let sub_start = match mdx.find("SELECT ({") {
        Some(p) => p,
        None => return vec![],
    };
    let sub_rest = &mdx[sub_start..];
    let sub_end = match sub_rest.find("})") {
        Some(p) => p,
        None => return vec![],
    };
    let members_str = &sub_rest["SELECT ({".len()..sub_end];
    for dim in visible_dims {
        let names = extract_dimension_member_names(members_str, dim);
        if !names.is_empty() {
            return vec![DimensionFilter {
                dimension: dimension_key_for_hier(dim).to_string(),
                members: names,
            }];
        }
    }
    vec![]
}

/// Detect dimensions referenced in the WHERE clause with their `[All]` member.
/// These are off-axis slicer dimensions that must appear on SlicerAxis.
pub fn parse_slicer_dimensions(mdx: &str) -> Vec<SlicerSelection> {
    let mut result = Vec::new();
    let where_payload = match where_clause_payload(mdx) {
        Some(p) => p,
        None => return result,
    };

    for (hier, dim_key) in [
        (PRODUKTKATEGORI_HIER, "Produktkategori"),
        (REGION_HIER, "Region"),
    ] {
        let pattern = format!("{}.", hier);
        if where_payload.contains(&pattern) {
            let is_all = where_payload.contains(&format!("{}.[All]", hier));
            result.push(SlicerSelection {
                dimension: dim_key.to_string(),
                is_all,
            });
        }
    }
    result
}

/// Detect which visible dimension is on the query axis (ON COLUMNS/ON ROWS).
fn row_dimension_from_mdx(mdx: &str) -> Option<&'static str> {
    // Search for FROM [Model] to skip any subquery FROM clauses.
    let select_end = mdx.find("FROM [Model]").unwrap_or_else(|| {
        mdx.find("FROM [model]").unwrap_or(mdx.len())
    });

    if mdx[..select_end].contains("[Region]") {
        Some("Region")
    } else if mdx[..select_end].contains("[Produktkategori]") {
        Some("Produktkategori")
    } else {
        None
    }
}

// ---- cChildren probe helpers ----

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
    CrossJoinProbe,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticQuery {
    pub kind: SemanticQueryKind,
    pub dim_props: Vec<String>,
    pub cell_props: Vec<String>,
    /// Dimension-tagged filter members.
    pub filters: Vec<DimensionFilter>,
    pub cchildren_leaf_name: Option<String>,
    /// Visible dimension on the query axis (Rows/Columns), or None for slicer-only.
    pub row_dimension: Option<String>,
    /// Off-axis dimensions from the WHERE clause (including All selections).
    pub slicers: Vec<SlicerSelection>,
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

    let kind = if mdx.contains("CrossJoin(") {
        SemanticQueryKind::CrossJoinProbe
    } else if mdx.contains("WITH MEMBER [Measures].cChildren") {
        if cchildren_target_is_measures(mdx) {
            SemanticQueryKind::ChildrenCountMeasures
        } else if cchildren_target_is_product_leaf(mdx) {
            SemanticQueryKind::ChildrenCountLeafProduct
        } else {
            SemanticQueryKind::ChildrenCountForAll
        }
    } else if mdx.contains("AddCalculatedMembers({[Measures].[Total Försäljning].Children})") {
        SemanticQueryKind::MeasureChildrenEmpty
    } else if (mdx.contains("AddCalculatedMembers({[Produktkategori].[Produktkategori].&[") || mdx.contains("AddCalculatedMembers({[Region].[Region].&["))
        && mdx.contains("].Children})") {
        SemanticQueryKind::LeafChildrenEmpty
    } else if mdx.contains("AddCalculatedMembers({[Produktkategori].[Produktkategori].[(All)].Members})")
        || mdx.contains("AddCalculatedMembers({[Region].[Region].[(All)].Members})") {
        SemanticQueryKind::AllLevelMembers
    } else if mdx.contains("AddCalculatedMembers({[Produktkategori].[Produktkategori].[All].Children})")
        || mdx.contains("AddCalculatedMembers({[Region].[Region].[All].Children})") {
        SemanticQueryKind::LeafLevelMembers
    } else if mdx.contains("AddCalculatedMembers({[Produktkategori].[Produktkategori].[Produktkategori].Members})")
        || mdx.contains("AddCalculatedMembers({[Region].[Region].[Region].Members})") {
        SemanticQueryKind::LeafLevelMembers
    } else if has_rows && has_cols && has_visible_dim && has_measures {
        SemanticQueryKind::MeasureByCategory
    } else if is_drilldown {
        SemanticQueryKind::DrilldownCategories
    } else if !has_axes
        && (mdx.contains("WHERE ([Produktkategori].[Produktkategori].[All],[Measures].[Total Försäljning])")
            || mdx.contains("WHERE ([Region].[Region].[All],[Measures].[Total Försäljning])")) {
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
        slicers: parse_slicer_dimensions(mdx),
    }
}
