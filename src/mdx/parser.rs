/// nom-based parser for the Excel MDX subset.
///
/// Parses member references, WHERE clauses, property clauses,
/// and axis expressions from Excel MDX probe/query strings.
///
/// Dimension names are dynamic — no hardcoded dimension vocabulary.
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, take_while},
    character::complete::{char, multispace0},
    multi::separated_list0,
    sequence::delimited,
};

// ---- whitespace ----

fn ws(input: &str) -> IResult<&str, &str> {
    multispace0(input)
}

fn sp(input: &str) -> IResult<&str, &str> {
    take_while(|c: char| c == ' ' || c == '\t')(input)
}

// ---- identifiers ----

/// A dimension reference — either the special `Measures` system dimension
/// or any user-configured cube dimension.
#[derive(Debug, Clone, PartialEq)]
pub enum DimRef {
    Measures,
    Cube(String),
}

fn dim_name(input: &str) -> IResult<&str, DimRef> {
    let (input, name) = take_while(|c: char| c != ']' && c != '.')(input)?;
    Ok((
        input,
        if name == "Measures" {
            DimRef::Measures
        } else {
            DimRef::Cube(name.to_string())
        },
    ))
}

fn bracket<'a, F, O>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O>
where
    F: FnMut(&'a str) -> IResult<&'a str, O>,
{
    delimited(char('['), inner, char(']'))
}

fn bracket_str(input: &str) -> IResult<&str, &str> {
    let (input, _) = char('[')(input)?;
    let (input, inner) = take_while(|c: char| c != ']')(input)?;
    let (input, _) = char(']')(input)?;
    Ok((input, inner))
}

fn dim_hierarchy(input: &str) -> IResult<&str, (DimRef, String)> {
    let (input, dim) = bracket(dim_name)(input)?;
    let (input, _) = char('.')(input)?;
    let (input, hname) = bracket_str(input)?;
    Ok((input, (dim, hname.to_string())))
}

// ---- member references ----

#[derive(Debug, Clone, PartialEq)]
pub enum MemberRef {
    All(DimRef),
    Leaf {
        dim: DimRef,
        key: String,
        /// Hierarchy level name for level-qualified references like
        /// `[Dim].[Hier].[Year].&[2024]`. None for plain leaf references.
        level: Option<String>,
    },
    Measure(String),
}

fn member_all(input: &str) -> IResult<&str, MemberRef> {
    let (input, (dim, _hier)) = dim_hierarchy(input)?;
    let (input, _) = alt((tag(".[All]"), tag(".[(All)]")))(input)?;
    Ok((input, MemberRef::All(dim)))
}

fn member_leaf(input: &str) -> IResult<&str, MemberRef> {
    let (input, (dim, _hier)) = dim_hierarchy(input)?;
    let (input, _) = tag(".&")(input)?;
    let (input, key) = bracket_str(input)?;
    Ok((
        input,
        MemberRef::Leaf {
            dim,
            key: key.to_string(),
            level: None,
        },
    ))
}

/// Name-based member reference `[Dim].[Hier].[Name]` (no `&` key qualifier).
/// Valid MDX — Excel usually emits `&`-qualified members, but hand-written MDX
/// and CUBEMEMBER with name references use this form.
fn member_named(input: &str) -> IResult<&str, MemberRef> {
    let (input, (dim, _hier)) = dim_hierarchy(input)?;
    let (input, _) = char('.')(input)?;
    let (input, key) = bracket_str(input)?;
    Ok((
        input,
        MemberRef::Leaf {
            dim,
            key: key.to_string(),
            level: None,
        },
    ))
}

/// Level-qualified key member `[Dim].[Hier].[Level].&[key]` — the level is
/// carried so the SQL emitter can filter on the level's column (e.g. the
/// `year` column for `[Date].[Date].[Year].&[2024]`). Excel emits this for
/// date-hierarchy filters.
fn member_level_leaf(input: &str) -> IResult<&str, MemberRef> {
    let (input, (dim, _hier)) = dim_hierarchy(input)?;
    let (input, _) = char('.')(input)?;
    let (input, level) = bracket_str(input)?;
    let (input, _) = tag(".&")(input)?;
    let (input, key) = bracket_str(input)?;
    Ok((
        input,
        MemberRef::Leaf {
            dim,
            key: key.to_string(),
            level: Some(level.to_string()),
        },
    ))
}

/// Level-qualified name member `[Dim].[Hier].[Level].[Name]` (no `&`).
fn member_level_named(input: &str) -> IResult<&str, MemberRef> {
    let (input, (dim, _hier)) = dim_hierarchy(input)?;
    let (input, _) = char('.')(input)?;
    let (input, level) = bracket_str(input)?;
    let (input, _) = char('.')(input)?;
    let (input, key) = bracket_str(input)?;
    Ok((
        input,
        MemberRef::Leaf {
            dim,
            key: key.to_string(),
            level: Some(level.to_string()),
        },
    ))
}

fn measure_member(input: &str) -> IResult<&str, MemberRef> {
    let (input, _) = bracket(dim_name)(input)?;
    let (input, _) = char('.')(input)?;
    let (input, name) = bracket_str(input)?;
    Ok((input, MemberRef::Measure(name.to_string())))
}

fn member_ref(input: &str) -> IResult<&str, MemberRef> {
    alt((
        member_all,
        member_level_leaf,
        member_level_named,
        member_leaf,
        member_named,
        measure_member,
    ))(input)
}

// ---- WHERE clause ----

fn where_clause(input: &str) -> IResult<&str, Vec<MemberRef>> {
    let (input, _) = tag("WHERE")(input)?;
    let (input, _) = sp(input)?;
    let (input, _) = char('(')(input)?;
    let (input, _) = ws(input)?;
    let (input, members) = separated_list0(delimited(ws, char(','), ws), member_ref)(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = char(')')(input)?;
    Ok((input, members))
}

// ---- subquery filter parsing ----

fn subquery_body(input: &str) -> IResult<&str, Vec<MemberRef>> {
    let (input, _) = tag("SELECT ")(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = tag("({")(input)?;
    let (input, _) = ws(input)?;
    let (input, members) = separated_list0(delimited(ws, char(','), ws), member_ref)(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = tag("})")(input)?;
    Ok((input, members))
}

fn find_all_subquery_members(input: &str) -> Vec<Vec<MemberRef>> {
    let mut results = Vec::new();
    let mut search_from = 0;
    while let Some(pos) = input[search_from..].find("SELECT ({") {
        let sub = &input[search_from + pos..];
        if let Ok((_, members)) = subquery_body(sub) {
            results.push(members);
        }
        search_from += pos + "SELECT (".len();
    }
    results
}

/// Extract every `[Measures].[name]` reference on the COLUMNS axis, in order.
/// Batched CUBEVALUE cells produce a multi-measure tuple set like
/// `SELECT {([Measures].[Revenue]),([Measures].[Units])} ON 0`. Set-function
/// sort/filter expressions (TopCount/Order/Filter on ROWS) are not axis
/// measures and are excluded.
fn find_all_select_measures(input: &str) -> Vec<String> {
    let upper = input.to_uppercase();
    let select_pos = upper.find("SELECT").unwrap_or(0);
    let from_pos = upper[select_pos..]
        .find("FROM")
        .map(|i| select_pos + i)
        .unwrap_or(input.len());
    let clause = &upper[select_pos..from_pos];

    // The COLUMNS axis is the expression directly before "ON COLUMNS"/"ON 0".
    let on_cols = clause
        .find("ON COLUMNS")
        .or_else(|| clause.find("ON 0"))
        .unwrap_or(clause.len());
    let before = &clause[..on_cols];
    let cols_start = before
        .rfind("ON ROWS")
        .map(|i| i + "ON ROWS".len())
        .or_else(|| before.rfind("ON 1").map(|i| i + "ON 1".len()))
        .unwrap_or(0);
    let cols_expr = &input[select_pos + cols_start..select_pos + on_cols];

    let mut result = Vec::new();
    let mut pos = 0;
    while let Some(i) = cols_expr[pos..].find("[Measures].[") {
        let start = pos + i + "[Measures].[".len();
        let Some(end) = cols_expr[start..].find(']') else {
            break;
        };
        result.push(cols_expr[start..start + end].to_string());
        pos = start + end + 1;
    }
    result
}

/// Parse a parenthesized, comma-separated member list (e.g. the tuple
/// `([Measures].[Revenue],[Category].[Category].&[Electronics])`).
fn paren_members(input: &str) -> IResult<&str, Vec<MemberRef>> {
    let (input, _) = ws(input)?;
    let (input, _) = char('(')(input)?;
    let (input, _) = ws(input)?;
    let (input, members) = separated_list0(delimited(ws, char(','), ws), member_ref)(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = char(')')(input)?;
    Ok((input, members))
}

/// Find dimension/measure members written as a tuple on the main SELECT axis,
/// e.g. `SELECT {([Measures].[Revenue],[Category].[Category].&[Electronics])} ON 0`.
fn find_select_tuple_members(input: &str) -> Vec<MemberRef> {
    let mut results = Vec::new();
    let mut search_from = 0;
    while let Some(pos) = input[search_from..].find("SELECT {(") {
        let after_brace = &input[search_from + pos + "SELECT {".len()..];
        if let Ok((_, members)) = paren_members(after_brace) {
            results.extend(members);
        }
        search_from += pos + "SELECT {(".len();
    }
    results
}

// ---- axis detection ----

/// Find the first non-Measures bracketed identifier in the MDX text.
fn detect_axis_dimension(input: &str) -> DimRef {
    let mut pos = 0;
    while let Some(open) = input[pos..].find('[') {
        let start = pos + open + 1;
        if let Some(close) = input[start..].find(']') {
            let name = &input[start..start + close];
            if name != "Measures" {
                return DimRef::Cube(name.to_string());
            }
            pos = start + close + 1;
        } else {
            break;
        }
    }
    DimRef::Measures
}

fn has_crossjoin(input: &str) -> bool {
    input.contains("CrossJoin(")
}

fn has_drilldown(input: &str) -> bool {
    input.contains("DrilldownLevel")
}

fn has_dot_members(input: &str) -> bool {
    input.contains(".Members")
}

fn has_dot_children(input: &str) -> bool {
    input.contains(".Children")
}

fn has_with_member_cchildren(input: &str) -> bool {
    input.contains("WITH MEMBER [Measures].cChildren")
}

// ---- property clause extraction ----

pub fn parse_dimension_properties(input: &str) -> Vec<String> {
    let up = input.to_uppercase();
    let Some(pos) = up.find("DIMENSION PROPERTIES ") else {
        return vec![];
    };
    let after = &input[pos + "DIMENSION PROPERTIES ".len()..];
    let end = after
        .find(" ON COLUMNS")
        .or_else(|| after.find(" ON ROWS"))
        .or_else(|| after.find(" FROM "))
        .or_else(|| after.find(" CELL PROPERTIES"))
        .unwrap_or(after.len());
    let raw = after[..end].trim();

    let known = &[
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
    let mut props = Vec::new();
    for token in raw.split(',') {
        let tu = token.trim().to_uppercase();
        for prop in known {
            if tu.ends_with(prop) && !props.iter().any(|p| p == prop) {
                props.push(prop.to_string());
                break;
            }
        }
    }
    props
}

pub fn parse_cell_properties(input: &str) -> Vec<String> {
    let up = input.to_uppercase();
    let Some(pos) = up.find("CELL PROPERTIES ") else {
        return vec![];
    };
    let after = &input[pos + "CELL PROPERTIES ".len()..];
    after
        .split(',')
        .map(|t| t.trim().to_uppercase())
        .filter(|t| !t.is_empty() && !t.contains(" "))
        .collect()
}

// ---- WHERE clause extraction ----

pub fn find_where_clause(input: &str) -> Option<Vec<MemberRef>> {
    let start = input.find("WHERE")?;
    let sub = &input[start..];
    where_clause(sub).ok().map(|(_, m)| m)
}

// ---- query-shape detection ----

#[derive(Debug, Clone, PartialEq)]
pub enum CChildrenTarget {
    None,
    All,
    Measures,
    ProductLeaf(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum CalculatedMembersPat {
    None,
    MeasureChildrenEmpty,
    LeafChildrenEmpty,
    AllLevelMembers,
    LeafLevelMembers,
}

fn detect_cchildren_target(input: &str) -> CChildrenTarget {
    let Some(start) = input.find("FilteredMembers As '") else {
        return CChildrenTarget::None;
    };
    let after_open = &input[start + "FilteredMembers As '".len()..];
    let Some(end) = after_open.find('\'') else {
        return CChildrenTarget::None;
    };
    let set = &after_open[..end];

    // Only measures mentioned, no cube dimension brackets at all
    if set.contains("[Measures]") && !set.contains("&[") && !set.contains("&amp;[") {
        // Check if the set references a specific leaf dimension member
        let has_leaf_dim = set.find('[').is_some_and(|i| {
            let rest = &set[i..];
            if let Some(close) = rest.find(']') {
                let name = &rest[1..close];
                name != "Measures"
            } else {
                false
            }
        });
        if !has_leaf_dim {
            return CChildrenTarget::Measures;
        }
    }

    if set.contains("&[") || set.contains("&amp;[") {
        if let Some(amp) = set.find("&[") {
            let begin = amp + 2;
            if let Some(closing) = set[begin..].find(']') {
                return CChildrenTarget::ProductLeaf(set[begin..begin + closing].to_string());
            }
        }
        if let Some(amp) = set.find("&amp;[") {
            let begin = amp + 5;
            if let Some(closing) = set[begin..].find(']') {
                return CChildrenTarget::ProductLeaf(set[begin..begin + closing].to_string());
            }
        }
    }

    CChildrenTarget::All
}

fn detect_calculated_members_pat(input: &str) -> CalculatedMembersPat {
    let Some(pos) = input.to_uppercase().find("ADDCALCULATEDMEMBERS({") else {
        return CalculatedMembersPat::None;
    };
    let rest = &input[pos..];

    if rest.contains("[Measures]") && rest.contains(".Children}") {
        return CalculatedMembersPat::MeasureChildrenEmpty;
    }

    if (rest.contains(".&[") || rest.contains(".&amp;[")) && rest.contains(".Children}") {
        return CalculatedMembersPat::LeafChildrenEmpty;
    }

    if rest.contains("[(All)]") && (rest.contains(".Members}") || rest.contains(".MEMBERS}")) {
        return CalculatedMembersPat::AllLevelMembers;
    }

    if rest.contains("[All]") && (rest.contains(".Children}") || rest.contains(".CHILDREN}")) {
        return CalculatedMembersPat::LeafLevelMembers;
    }

    if rest.contains(".Members}") || rest.contains(".MEMBERS}") {
        return CalculatedMembersPat::LeafLevelMembers;
    }

    CalculatedMembersPat::None
}

fn has_drilldown_member(input: &str) -> bool {
    input.contains("DrilldownMember(")
}

fn has_measures_in_where_or_cols(input: &str) -> bool {
    input.to_uppercase().contains("[MEASURES]")
}

/// An axis set function that transforms the row set (sort / limit / filter).
#[derive(Debug, Clone, PartialEq)]
pub enum AxisSetOp {
    /// `TopCount(set, n, expr)` (desc=true) / `BottomCount(...)` (desc=false).
    TopCount { n: usize, desc: bool },
    /// `TopPercent(set, p, expr)` — top p percent of members by expr.
    TopPercent { p: f64 },
    /// `Order(set, expr, DESC|ASC)`.
    Order { desc: bool },
    /// `Filter(set, [Measures].[X] OP value)` — value filter.
    Filter { op: CmpOp, value: f64 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum CmpOp {
    Gt,
    Ge,
    Lt,
    Le,
    Eq,
    Ne,
}

/// Detect an axis set function (TopCount/BottomCount/Order/Filter) in the outer
/// SELECT clause. Only measure-based sorts/filters are supported (label filters
/// and TopPercent/BottomPercent are not).
pub fn detect_axis_set_op(input: &str) -> Option<AxisSetOp> {
    let upper = input.to_uppercase();
    let select_pos = upper.find("SELECT").unwrap_or(0);
    let from_pos = upper[select_pos..]
        .find("FROM")
        .map(|i| select_pos + i)
        .unwrap_or(input.len());
    let clause = &input[select_pos..from_pos];
    let up = clause.to_uppercase();

    if let Some(pos) = up.find("TOPCOUNT(") {
        let after = &clause[pos + "TopCount(".len()..];
        let comma = after.find(',')?;
        let n: usize = after[comma + 1..]
            .trim()
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .ok()?;
        return Some(AxisSetOp::TopCount { n, desc: true });
    }
    if let Some(pos) = up.find("BOTTOMCOUNT(") {
        let after = &clause[pos + "BottomCount(".len()..];
        let comma = after.find(',')?;
        let n: usize = after[comma + 1..]
            .trim()
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .ok()?;
        return Some(AxisSetOp::TopCount { n, desc: false });
    }
    if let Some(pos) = up.find("TOPPERCENT(") {
        let after = &clause[pos + "TopPercent(".len()..];
        let comma = after.find(',')?;
        let p: f64 = after[comma + 1..]
            .trim()
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect::<String>()
            .parse()
            .ok()?;
        return Some(AxisSetOp::TopPercent { p });
    }
    if let Some(pos) = up.find("ORDER(") {
        let after = &clause[pos + "Order(".len()..];
        let desc = after.to_uppercase().contains("DESC");
        return Some(AxisSetOp::Order { desc });
    }
    if let Some(pos) = up.find("FILTER(") {
        let after = &clause[pos + "Filter(".len()..];
        if let Some((op, value)) = parse_filter_condition(after) {
            return Some(AxisSetOp::Filter { op, value });
        }
    }
    None
}

/// Parse `[Measures].[X] OP value` from a Filter() condition.
fn parse_filter_condition(s: &str) -> Option<(CmpOp, f64)> {
    let measure_pos = s.find("[Measures].[")?;
    let after_measure = &s[measure_pos + "[Measures].[".len()..];
    let name_end = after_measure.find(']')?;
    let after_name = after_measure[name_end + 1..].trim_start();
    let (op, rest) = if let Some(r) = after_name.strip_prefix(">=") {
        (CmpOp::Ge, r)
    } else if let Some(r) = after_name.strip_prefix("<=") {
        (CmpOp::Le, r)
    } else if let Some(r) = after_name.strip_prefix("<>") {
        (CmpOp::Ne, r)
    } else if let Some(r) = after_name.strip_prefix('>') {
        (CmpOp::Gt, r)
    } else if let Some(r) = after_name.strip_prefix('<') {
        (CmpOp::Lt, r)
    } else if let Some(r) = after_name.strip_prefix('=') {
        (CmpOp::Eq, r)
    } else {
        return None;
    };
    let value: f64 = rest
        .trim()
        .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .next()?
        .parse()
        .ok()?;
    Some((op, value))
}

/// True when the WHERE clause contains exactly one cube-dimension
/// member (All or Leaf) and one measure member.
fn is_slicer_all_measure(input: &str) -> bool {
    let members = match find_where_clause(input) {
        Some(m) => m,
        None => return false,
    };
    let cube_count = members
        .iter()
        .filter(|m| matches!(m, MemberRef::All(_) | MemberRef::Leaf { .. }))
        .count();
    let meas_count = members
        .iter()
        .filter(|m| matches!(m, MemberRef::Measure(_)))
        .count();
    cube_count == 1 && meas_count == 1 && members.len() == 2
}

// ---- complete mdx parse ----

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedMdx {
    pub dim_props: Vec<String>,
    pub cell_props: Vec<String>,
    pub has_rows: bool,
    pub has_cols: bool,
    pub has_crossjoin: bool,
    pub has_drilldown: bool,
    pub has_dot_members: bool,
    pub has_dot_children: bool,
    pub has_with_member_cchildren: bool,
    pub has_where_all_measure: bool,
    pub has_drilldown_member: bool,
    pub has_measures: bool,
    pub where_members: Vec<MemberRef>,
    pub subquery_members: Vec<MemberRef>,
    pub select_members: Vec<MemberRef>,
    pub main_dim: DimRef,
    pub cchildren_target: CChildrenTarget,
    pub calculated_members_pat: CalculatedMembersPat,
    /// The explicitly requested measure name, extracted from
    /// WHERE/columns (e.g. "Units" from [Measures].[Units]).
    pub selected_measure: Option<String>,
    /// All measure names referenced in the SELECT clause, in order.
    /// Multiple entries mean Excel batched several CUBEVALUE cells into one
    /// multi-measure query (e.g. `{[Measures].[Revenue],[Measures].[Units]}`).
    pub selected_measures: Vec<String>,
    /// The cube name extracted from `FROM [cubeName]` (e.g. "Sales").
    pub cube_name: Option<String>,
    /// Positionally-ordered dimension IDs from the select clause.
    /// Extracted from CrossJoin / DrilldownLevel expressions, unfiltered
    /// by the project model. e.g. ["Territory", "Category"].
    pub axis_dimension_ids: Vec<String>,
    /// Excluded members from a DrilldownMember collapse expression.
    /// Each tuple is (dimension_id, member_key).
    /// Empty when `has_drilldown_member` is false.
    pub excluded_members: Vec<(String, String)>,
    /// The dimension token following the DrilldownMember exclusion set.
    pub drilldown_member_hierarchy: Option<String>,
    /// Axis set function (TopCount/Order/Filter) wrapping the row set, if any.
    pub axis_set_op: Option<AxisSetOp>,
}

/// Extract dimension IDs from the select clause in positional order.
///
/// Mirrors the `parse_axis_dimensions()` logic from semantic.rs: finds the
/// axis expression (before `DIMENSION PROPERTIES` or `ON COLUMNS`), then
/// collects all non-Measures bracketed identifiers in left-to-right order,
/// deduplicated.
fn parse_axis_dimension_ids(before_from: &str) -> Vec<String> {
    // Axes live in the outer SELECT clause (between SELECT and the outer FROM);
    // subquery SELECTs sit inside FROM (...) and must not contribute.
    let upper = before_from.to_uppercase();
    let select_pos = upper.find("SELECT").unwrap_or(0);
    let from_pos = upper[select_pos..]
        .find("FROM")
        .map(|i| select_pos + i)
        .unwrap_or(before_from.len());
    let clause = &before_from[select_pos..from_pos];

    // Drop each "DIMENSION PROPERTIES <props> ON <axis>" segment (member-property
    // names would otherwise be mistaken for dimensions), then scan the remainder
    // — which includes both the COLUMNS and ROWS axis expressions.
    let mut scan = String::new();
    let mut rest = clause;
    loop {
        let upper = rest.to_uppercase();
        match upper.find("DIMENSION PROPERTIES") {
            Some(dp) => {
                scan.push_str(&rest[..dp]);
                let after = &upper[dp + "DIMENSION PROPERTIES".len()..];
                let end = after
                    .find("ON COLUMNS")
                    .or_else(|| after.find("ON ROWS"))
                    .or_else(|| after.find(" ON 0 "))
                    .or_else(|| after.find(" ON 1 "))
                    .unwrap_or(0);
                rest = &rest[dp + "DIMENSION PROPERTIES".len() + end..];
            }
            None => {
                scan.push_str(rest);
                break;
            }
        }
    }

    let mut ids = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut pos = 0;
    while let Some(open) = scan[pos..].find('[') {
        let abs = pos + open + 1;
        let close = scan[abs..].find(']').unwrap_or(scan.len() - abs);
        let id = &scan[abs..abs + close];
        if id != "Measures" && !id.is_empty() && seen.insert(id.to_string()) {
            ids.push(id.to_string());
        }
        pos = abs + close + 1;
    }
    ids
}

/// Parse excluded members from a DrilldownMember collapse expression.
/// Only scans within the `{-{ ... }}` exclusion set boundary — does NOT
/// pick up later WHERE slicer members.
fn parse_excluded_members_from_mdx(input: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let Some(excl_start) = input.find("{-{") else {
        return result;
    };

    // Bound to the closing }} of the exclusion set.
    let after_excl = &input[excl_start..];
    let Some(close) = after_excl[2..].find("}}") else {
        return result;
    };
    let excl_end = 2 + close + 2;
    let excl = &after_excl[..excl_end];

    let mut search_from = 0;
    while let Some(amp) = excl[search_from..].find("&[") {
        let begin = search_from + amp + 2;
        if let Some(end) = excl[begin..].find(']') {
            let key = excl[begin..begin + end].to_string();
            // Look backwards for the preceding [Dimension].
            let before = &excl[..search_from + amp];
            let dim = if let Some(last_dot) = before.rfind("].") {
                if let Some(open) = before[..last_dot].rfind('[') {
                    before[open + 1..last_dot].to_string()
                } else {
                    continue;
                }
            } else {
                continue;
            };
            result.push((dim, key));
            search_from = begin + end;
        } else {
            break;
        }
    }
    result
}

/// Parse the hierarchy target following a DrilldownMember exclusion set.
fn parse_drilldown_member_hierarchy_from_mdx(input: &str) -> Option<String> {
    let excl_start = input.find("{-{")?;
    let after_excl = &input[excl_start..];
    let close = after_excl[2..].find("}}")?;
    let rest = &after_excl[2 + close + 2..];
    let trimmed = rest.trim_start();
    let trimmed = trimmed.strip_prefix(',').unwrap_or(trimmed).trim_start();
    if !trimmed.starts_with('[') {
        return None;
    }
    let bracket_end = trimmed[1..].find(']')?;
    let hier = &trimmed[1..bracket_end + 1];
    let hier = hier.trim_matches(|c: char| c == '[' || c == ']');
    Some(hier.to_string())
}

pub fn parse_mdx(input: &str) -> ParsedMdx {
    let up = input.to_uppercase();

    // Find `FROM [` boundary generically (case-insensitive).
    let before_from = up.find("FROM [").map(|i| &input[..i]).unwrap_or(input);

    // Extract cube name from `FROM [cubeName]`.
    let cube_name: Option<String> = up.find("FROM [").and_then(|start| {
        let after_from = &input[start + "FROM [".len()..];
        after_from
            .find(']')
            .map(|end| after_from[..end].to_string())
    });

    // Parse axis dimension IDs from the select clause in positional order.
    // Strategy: split on CrossJoin( / DrilldownLevel( to find axis expressions,
    // then extract the first non-Measures bracketed identifier in each.
    let axis_dimension_ids = parse_axis_dimension_ids(before_from);

    // Parse excluded members from DrilldownMember if present.
    let excluded_members = if has_drilldown_member(input) {
        parse_excluded_members_from_mdx(input)
    } else {
        Vec::new()
    };

    let drilldown_member_hierarchy = if has_drilldown_member(input) {
        parse_drilldown_member_hierarchy_from_mdx(input)
    } else {
        None
    };

    let where_members = find_where_clause(input).unwrap_or_default();

    let all_subquery = find_all_subquery_members(input);
    let subquery_members: Vec<MemberRef> = all_subquery.into_iter().flatten().collect();

    let select_members = find_select_tuple_members(input);

    // All measures referenced in the SELECT clause, in order. A single cell
    // holds one measure; batched CUBEVALUE cells produce several.
    let select_measures = find_all_select_measures(input);
    let selected_measure = where_members
        .iter()
        .find_map(|m| match m {
            MemberRef::Measure(name) => Some(name.clone()),
            _ => None,
        })
        .or_else(|| select_measures.first().cloned());

    ParsedMdx {
        dim_props: parse_dimension_properties(input),
        cell_props: parse_cell_properties(input),
        has_rows: up.contains("ON ROWS") || up.contains(" ON 1 "),
        has_cols: up.contains("ON COLUMNS") || up.contains(" ON 0 "),
        has_crossjoin: has_crossjoin(input),
        has_drilldown: has_drilldown(input),
        has_dot_members: has_dot_members(input),
        has_dot_children: has_dot_children(input),
        has_with_member_cchildren: has_with_member_cchildren(input),
        has_where_all_measure: is_slicer_all_measure(input),
        has_drilldown_member: has_drilldown_member(input),
        has_measures: has_measures_in_where_or_cols(input),
        where_members,
        subquery_members,
        select_members,
        main_dim: detect_axis_dimension(before_from),
        cchildren_target: detect_cchildren_target(input),
        calculated_members_pat: detect_calculated_members_pat(input),
        selected_measure,
        selected_measures: select_measures,
        cube_name,
        axis_dimension_ids,
        excluded_members,
        drilldown_member_hierarchy,
        axis_set_op: detect_axis_set_op(input),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_member_all() {
        let (rest, m) = member_ref("[Produktkategori].[Produktkategori].[All]").unwrap();
        assert_eq!(m, MemberRef::All(DimRef::Cube("Produktkategori".into())));
        assert!(rest.is_empty());
    }

    #[test]
    fn parse_member_leaf() {
        let (rest, m) = member_ref("[Produktkategori].[Produktkategori].&[Kategori A]").unwrap();
        assert_eq!(
            m,
            MemberRef::Leaf {
                dim: DimRef::Cube("Produktkategori".into()),
                key: "Kategori A".into(),
                level: None,
            }
        );
        assert!(rest.is_empty());
    }

    #[test]
    fn parse_member_region() {
        let (rest, m) = member_ref("[Region].[Region].&[North]").unwrap();
        assert_eq!(
            m,
            MemberRef::Leaf {
                dim: DimRef::Cube("Region".into()),
                key: "North".into(),
                level: None,
            }
        );
        assert!(rest.is_empty());
    }

    #[test]
    fn parse_where_multiple() {
        let input = "WHERE ([Region].[Region].[All],[Measures].[Total Försäljning])";
        let (_rest, members) = where_clause(input).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0], MemberRef::All(DimRef::Cube("Region".into())));
        assert_eq!(members[1], MemberRef::Measure("Total Försäljning".into()));
    }

    #[test]
    fn parse_where_leaf() {
        let input = "WHERE ([Produktkategori].[Produktkategori].&[Kategori B],[Measures].[Total Försäljning])";
        let (_rest, members) = where_clause(input).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(
            members[0],
            MemberRef::Leaf {
                dim: DimRef::Cube("Produktkategori".into()),
                key: "Kategori B".into(),
                level: None,
            }
        );
    }

    #[test]
    fn parse_subquery() {
        let _input = "SELECT ({[Produktkategori].[Produktkategori].&[Kategori A],[Produktkategori].[Produktkategori].&[Kategori C]}) ON COLUMNS FROM [Model]";
        let (_rest, m) = subquery_body("SELECT ({[Produktkategori].[Produktkategori].&[Kategori A],[Produktkategori].[Produktkategori].&[Kategori C]})").unwrap();
        assert_eq!(m.len(), 2);
    }

    // ---- project3 tests (dynamic dimension names) ----

    #[test]
    fn parse_category_all() {
        let (rest, m) = member_ref("[Category].[Category].[All]").unwrap();
        assert_eq!(m, MemberRef::All(DimRef::Cube("Category".into())));
        assert!(rest.is_empty());
    }

    #[test]
    fn parse_territory_leaf() {
        let (rest, m) = member_ref("[Territory].[Territory].&[North]").unwrap();
        assert_eq!(
            m,
            MemberRef::Leaf {
                dim: DimRef::Cube("Territory".into()),
                key: "North".into(),
                level: None,
            }
        );
        assert!(rest.is_empty());
    }

    #[test]
    fn parse_channel_leaf() {
        let (rest, m) = member_ref("[Channel].[Channel].&[Online]").unwrap();
        assert_eq!(
            m,
            MemberRef::Leaf {
                dim: DimRef::Cube("Channel".into()),
                key: "Online".into(),
                level: None,
            }
        );
        assert!(rest.is_empty());
    }

    #[test]
    fn parse_measure_revenue() {
        let (rest, m) = member_ref("[Measures].[Revenue]").unwrap();
        assert_eq!(m, MemberRef::Measure("Revenue".into()));
        assert!(rest.is_empty());
    }

    #[test]
    fn parse_where_category_all_revenue() {
        let input = "WHERE ([Category].[Category].[All],[Measures].[Revenue])";
        let (_rest, members) = where_clause(input).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0], MemberRef::All(DimRef::Cube("Category".into())));
        assert_eq!(members[1], MemberRef::Measure("Revenue".into()));
    }

    #[test]
    fn parse_where_territory_leaf_revenue() {
        let input = "WHERE ([Territory].[Territory].&[North],[Measures].[Revenue])";
        let (_rest, members) = where_clause(input).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(
            members[0],
            MemberRef::Leaf {
                dim: DimRef::Cube("Territory".into()),
                key: "North".into(),
                level: None,
            }
        );
        assert_eq!(members[1], MemberRef::Measure("Revenue".into()));
    }

    #[test]
    fn parse_dim_props() {
        let input = "SELECT ... DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME,MEMBER_TYPE ON COLUMNS FROM [Model]";
        let props = parse_dimension_properties(input);
        assert!(props.contains(&"PARENT_UNIQUE_NAME".to_string()));
        assert!(props.contains(&"HIERARCHY_UNIQUE_NAME".to_string()));
        assert!(props.contains(&"MEMBER_TYPE".to_string()));
    }

    #[test]
    fn parse_cell_props() {
        let input = "SELECT ... CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";
        let props = parse_cell_properties(input);
        assert_eq!(
            props,
            vec!["VALUE", "FORMAT_STRING", "BACK_COLOR", "FORE_COLOR"]
        );
    }

    #[test]
    fn parse_selected_measure_from_columns() {
        let mdx = "Select {[Measures].[Units]} on columns from [Sales]";
        let parsed = parse_mdx(mdx);
        assert_eq!(
            parsed.selected_measure.as_deref(),
            Some("Units"),
            "expected Units from columns"
        );
    }

    #[test]
    fn parse_selected_measure_from_columns_bracketed_set() {
        let mdx = "SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales]";
        let parsed = parse_mdx(mdx);
        assert_eq!(
            parsed.selected_measure.as_deref(),
            Some("Revenue"),
            "expected Revenue from columns"
        );
    }
}
