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

fn parse_category_filter(mdx: &str) -> Option<String> {
    let start = mdx.find("[Produktkategori].[Produktkategori].")?;
    let rest = &mdx[start..];
    if rest.contains("[Produktkategori].[Produktkategori].[All]") {
        return None;
    }
    if let Some(amp) = rest.find("&amp;[") {
        let begin = amp + 5;
        let end = rest[begin..].find(']')? + begin;
        return Some(rest[begin..end].to_string());
    }
    if let Some(amp) = rest.find("&[") {
        let begin = amp + 2;
        let end = rest[begin..].find(']')? + begin;
        return Some(rest[begin..end].to_string());
    }
    None
}

pub fn parse_mdx_filters(mdx: &str) -> Vec<String> {
    if let Some(where_filter) = parse_category_filter(mdx) {
        return vec![where_filter];
    }
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
    let mut result = Vec::new();
    for member in members_str.split(',') {
        let member = member.trim();
        if let Some(amp_start) = member.find("&[") {
            let begin = amp_start + 2;
            if let Some(end) = member[begin..].find(']') {
                result.push(member[begin..begin + end].to_string());
            }
        } else if let Some(amp_start) = member.find("&amp;[") {
            let begin = amp_start + 5;
            if let Some(end) = member[begin..].find(']') {
                result.push(member[begin..begin + end].to_string());
            }
        }
    }
    result
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticQuery {
    pub kind: SemanticQueryKind,
    pub dim_props: Vec<String>,
    pub cell_props: Vec<String>,
    pub category_filters: Vec<String>,
    pub cchildren_leaf_name: Option<String>,
}

pub fn semantic_query_from_mdx(mdx: &str) -> SemanticQuery {
    let upper = mdx.to_uppercase();
    let dim_props = parse_dimension_properties(mdx);
    let cell_props = parse_cell_properties(mdx);
    let category_filters = parse_mdx_filters(mdx);
    let has_axes = upper.contains("ON COLUMNS") || upper.contains("ON ROWS");
    let has_rows = upper.contains("ON ROWS");
    let has_cols = upper.contains("ON COLUMNS");
    let has_product = mdx.contains("[Produktkategori]");
    let has_measures = mdx.contains("[Measures]");
    let is_drilldown = has_product && (mdx.contains("DrilldownLevel") || mdx.contains(".Members"));

    let kind = if mdx.contains("WITH MEMBER [Measures].cChildren") {
        if cchildren_target_is_measures(mdx) {
            SemanticQueryKind::ChildrenCountMeasures
        } else if cchildren_target_is_product_leaf(mdx) {
            SemanticQueryKind::ChildrenCountLeafProduct
        } else {
            SemanticQueryKind::ChildrenCountForAll
        }
    } else if mdx.contains("WHERE ([Produktkategori].[Produktkategori].[All],[Measures].[Total Försäljning])") {
        SemanticQueryKind::SlicerAllAndMeasure
    } else if mdx.contains("AddCalculatedMembers({[Measures].[Total Försäljning].Children})") {
        SemanticQueryKind::MeasureChildrenEmpty
    } else if mdx.contains("AddCalculatedMembers({[Produktkategori].[Produktkategori].&[") && mdx.contains("].Children})") {
        SemanticQueryKind::LeafChildrenEmpty
    } else if mdx.contains("AddCalculatedMembers({[Produktkategori].[Produktkategori].[(All)].Members})") {
        SemanticQueryKind::AllLevelMembers
    } else if mdx.contains("AddCalculatedMembers({[Produktkategori].[Produktkategori].[All].Children})") {
        SemanticQueryKind::LeafLevelMembers
    } else if mdx.contains("AddCalculatedMembers({[Produktkategori].[Produktkategori].[Produktkategori].Members})") {
        SemanticQueryKind::LeafLevelMembers
    } else if has_rows && has_cols && has_product && has_measures {
        SemanticQueryKind::MeasureByCategory
    } else if is_drilldown {
        SemanticQueryKind::DrilldownCategories
    } else if !has_axes {
        SemanticQueryKind::SlicerOnly
    } else {
        SemanticQueryKind::SlicerOnly
    };

    SemanticQuery {
        kind,
        dim_props,
        cell_props,
        category_filters,
        cchildren_leaf_name: cchildren_filtered_member_name(mdx),
    }
}
