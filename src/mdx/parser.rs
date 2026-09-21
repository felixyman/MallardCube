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
    combinator::map,
    multi::separated_list0,
    sequence::delimited,
};

// ---- unsupported constructs (plan 046) ----

/// Constructs MallardCube does not support yet, detected up front so the
/// execute path can return a clear fault instead of a dropped axis or a
/// wrong-hierarchy cellset.
///
/// Deliberately conservative: only constructs that are verified broken today.
pub fn unsupported_features(mdx: &str) -> Option<String> {
    let upper = mdx.to_uppercase();
    if upper.contains("WITH SET") {
        return Some("named sets (`WITH SET`) are not supported yet".into());
    }
    for (needle, name) in [
        ("PERIODSTODATE(", "PeriodsToDate()"),
        ("PARALLELPERIOD(", "ParallelPeriod()"),
        ("LASTPERIODS(", "LastPeriods()"),
        ("CLOSINGPERIOD(", "ClosingPeriod()"),
        ("OPENINGPERIOD(", "OpeningPeriod()"),
        ("YTD(", "YTD()"),
        ("QTD(", "QTD()"),
        ("MTD(", "MTD()"),
    ] {
        if upper.contains(needle) {
            return Some(format!(
                "the MDX time function `{name}` is not supported yet"
            ));
        }
    }
    if upper.contains("VBA!") || upper.contains("DATEADD(") {
        return Some("MDX date arithmetic (`DateAdd`/`VBA!`) is not supported yet".into());
    }
    if upper.contains("FILTER(")
        && (upper.contains("MEMBER_VALUE")
            || upper.contains("MEMBER_KEY")
            || upper.contains("CURRENTMEMBER"))
    {
        return Some(
            "member-property filters (`Filter` over `Member_Value`/`Member_Key`) are not supported yet"
                .into(),
        );
    }
    // A braced `{range}` beside a braced measure set (`{[Measures].[X]} ON 0,
    // {a : b} ON 1`) is not classified as an axis yet — fault instead of
    // returning a slicer-only cellset.
    if let Ok(sel) = crate::mdx::frontend::parse_select(mdx) {
        let axis_set_has = |pred: &dyn Fn(&crate::mdx::ast::Expr) -> bool| {
            sel.axes.iter().any(|a| {
                a.exprs.iter().any(
                    |e| matches!(e, crate::mdx::ast::Expr::Set(items) if items.iter().any(pred)),
                )
            })
        };
        if axis_set_has(&|e| matches!(e, crate::mdx::ast::Expr::Measure(_)))
            && axis_set_has(&|e| matches!(e, crate::mdx::ast::Expr::Range(..)))
        {
            return Some(
                "member ranges on a pivot axis beside a measure set are not supported yet".into(),
            );
        }
    }

    // Member ranges are supported on the axis (a pivot axis, a set probe, or a
    // bare set). In a slicer, or inside a quoted calculated-member body
    // (`COUNT({a : b})`), they are not handled yet.
    if let Some(pos) = member_range_pos(mdx) {
        let from_pos = upper.find("FROM").unwrap_or(mdx.len());
        if pos > from_pos || inside_quotes(mdx, pos) {
            return Some(
                "member ranges outside the axis (`{a : b}` in a slicer or a calculated member) are not supported yet"
                    .into(),
            );
        }
    }
    None
}

/// Is a byte position inside a quoted string (`'…'` / `"…"`)?
fn inside_quotes(mdx: &str, pos: usize) -> bool {
    let mut quote: Option<char> = None;
    for (i, c) in mdx.char_indices() {
        if i >= pos {
            break;
        }
        match (quote, c) {
            (None, '\'' | '"') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            _ => {}
        }
    }
    quote.is_some()
}

/// Byte position of a `:` between two bracketed members, outside brackets.
fn member_range_pos(mdx: &str) -> Option<usize> {
    let bytes = mdx.as_bytes();
    let mut depth = 0usize;
    let mut last_non_space: Option<u8> = None;
    for (i, &c) in bytes.iter().enumerate() {
        match c {
            b'[' => depth += 1,
            b']' => depth = depth.saturating_sub(1),
            b':' if depth == 0 && last_non_space == Some(b']') => {
                let mut j = i + 1;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b'[' {
                    return Some(i);
                }
            }
            _ => {}
        }
        if !c.is_ascii_whitespace() {
            last_non_space = Some(c);
        }
    }
    None
}

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

/// Parse one or more compound MDX key components: `&[2026]&[4]` becomes the
/// internal path `2026|4`.
fn composite_key(input: &str) -> IResult<&str, String> {
    let (mut rest, first) = bracket_str(input)?;
    let mut parts = vec![first.to_string()];
    while rest.starts_with('&') {
        let (next, part) = bracket_str(&rest[1..])?;
        parts.push(part.to_string());
        rest = next;
    }
    Ok((rest, parts.join("|")))
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
    let (input, key) = composite_key(input)?;
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
    let (input, key) = composite_key(input)?;
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

/// Parse a `{ member | (tuple), ... }` axis set into its member refs.
fn member_set(input: &str) -> IResult<&str, Vec<MemberRef>> {
    let (input, _) = ws(input)?;
    let (input, _) = char('{')(input)?;
    let (input, _) = ws(input)?;
    let (input, items) = separated_list0(
        delimited(ws, char(','), ws),
        alt((map(paren_members, |ms| ms), map(member_ref, |m| vec![m]))),
    )(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = char('}')(input)?;
    Ok((input, items.into_iter().flatten().collect()))
}

/// Members restricted by a FROM-clause subselect:
/// `FROM (SELECT {[Dim].[Hier].&[k]} ON COLUMNS FROM [Cube])`. SSAS applies
/// these as slicer-axis restrictions.
fn find_subselect_members(input: &str) -> Vec<MemberRef> {
    let mut results = Vec::new();
    let upper = input.to_uppercase();
    let mut search_from = 0;
    while let Some(pos) = upper[search_from..].find("(SELECT ") {
        let after = &input[search_from + pos + "(SELECT ".len()..];
        if let Ok((_, members)) = member_set(after) {
            results.extend(members);
        }
        search_from += pos + "(SELECT ".len();
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

/// Parse a `{ (tuple), (tuple), ... }` set of parenthesized tuples.
fn select_set(input: &str) -> IResult<&str, Vec<Vec<MemberRef>>> {
    let (input, _) = ws(input)?;
    let (input, _) = char('{')(input)?;
    let (input, _) = ws(input)?;
    let (input, tuples) = separated_list0(delimited(ws, char(','), ws), paren_members)(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = char('}')(input)?;
    Ok((input, tuples))
}

/// Find a set of parenthesized tuples on the SELECT axis, one `Vec<MemberRef>`
/// per tuple. Batched CUBEVALUE cells with different slicers produce
/// `SELECT {([M],[D].[L].&[k1]),([M],[D].[L2].&[k2])} ON 0`.
fn find_select_tuples(input: &str) -> Vec<Vec<MemberRef>> {
    let mut results = Vec::new();
    let mut search_from = 0;
    while let Some(pos) = input[search_from..].find("SELECT {") {
        let after_brace = &input[search_from + pos + "SELECT ".len()..];
        if let Ok((_, tuples)) = select_set(after_brace) {
            results.extend(tuples);
        }
        search_from += pos + "SELECT ".len();
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
        "MEMBER_CAPTION",
        "MEMBER_UNIQUE_NAME",
        "MEMBER_KEY",
        "MEMBER_TYPE",
        "MEMBER_VALUE",
        "LEVEL_NUMBER",
        "LEVEL_UNIQUE_NAME",
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
        // A level-qualified source (`{AddCalculatedMembers({[D].[H].[Level].Members})}`)
        // is a normal level set: the plan and renderer honor the level and emit
        // level-qualified unique names. Only the unqualified
        // `[D].[H].Members` form means the leaf grain.
        return if parse_axis_level_members(input).is_empty() {
            CalculatedMembersPat::LeafLevelMembers
        } else {
            CalculatedMembersPat::None
        };
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

// ---- set expressions (Excel CUBESET probes) ----

/// A set expression on the SELECT axis, as Excel builds it for CUBESET
/// validation: a member source optionally wrapped in Head/Tail/Subset.
#[derive(Debug, Clone, PartialEq)]
pub enum SetExpr {
    /// `[Measures].Members` — every defined measure.
    Measures,
    /// `[Dim].[Hier].[Level].Members` — level None means the leaf/physical
    /// grain (`[Dim].[Hier].Members`).
    LevelMembers { dim: String, level: Option<String> },
    /// `[Dim].[Hier].[(All)].Members` / `[Dim].[Hier].[All].Children` — the
    /// first-level members under All.
    AllMembers { dim: String },
    /// An explicit member list like `{[D].[H].&[a],[D].[H].&[b]}` (outer
    /// braces stripped by the caller). Unames may be XML-escaped.
    MemberList { unames: Vec<String> },
    /// A member range `{[D].[H].[L].&[a] : [D].[H].[L].&[b]}` — the members of
    /// the endpoints' level between the two keys, in hierarchy order.
    MemberRange { from: String, to: String },
    /// `Head(set, n)` — first n members.
    Head(Box<SetExpr>, usize),
    /// `Tail(set, n)` — last n members.
    Tail(Box<SetExpr>, usize),
}

/// A calculated member whose body is `COUNT(<set>)`, e.g. Excel's
/// `WITH MEMBER [Measures].[XL_SD] AS 'COUNT([Date].[Date].[Year].Members)'`.
#[derive(Debug, Clone, PartialEq)]
pub struct CalculatedCount {
    pub member_name: String,
    pub set: SetExpr,
}

/// Parse a set source like `[Date].[Date].[Year].Members`,
/// `[Sales].[Sales].Children`, or `[Dim].[Dim].[All].Members` into a SetExpr.
/// The closing `}` of an enclosing set may be present; it is ignored.
fn parse_set_source(text: &str) -> Option<SetExpr> {
    let t = text.trim().trim_end_matches('}').trim_end();
    let dot = t.rfind('.')?;
    let func = &t[dot + 1..];
    let src = t[..dot].trim_end();
    let members = func.eq_ignore_ascii_case("Members")
        || func.eq_ignore_ascii_case("AllMembers")
        || func.eq_ignore_ascii_case("Children");
    if !members {
        return None;
    }
    // Collect bracketed segments from the source reference. More than three
    // segments means a member-qualified source (e.g. `[X].&[2024]&[2]`),
    // which is not a supported set source.
    let mut segs: Vec<String> = Vec::new();
    let mut rest = src;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let close = after.find(']')?;
        segs.push(after[..close].to_string());
        rest = &after[close + 1..];
    }
    if segs.len() == 1 {
        // `[Measures].Members`
        if segs[0] == "Measures" && func.eq_ignore_ascii_case("Members") {
            return Some(SetExpr::Measures);
        }
        return None;
    }
    if segs.len() > 3 {
        return None;
    }
    let dim = segs[0].clone();
    if segs.len() == 2 {
        Some(SetExpr::LevelMembers { dim, level: None })
    } else {
        let level = &segs[segs.len() - 1];
        if level == "(All)" || level == "All" {
            Some(SetExpr::AllMembers { dim })
        } else {
            Some(SetExpr::LevelMembers {
                dim,
                level: Some(level.clone()),
            })
        }
    }
}

/// Parse the SELECT-axis set expression: `{ HEAD(src, n) }`, `{ TAIL(src, n) }`,
/// or a bare `{ src }`. Returns None when the axis isn't a simple set probe.
pub fn parse_axis_set_expr(input: &str) -> Option<SetExpr> {
    let upper = input.to_uppercase();
    let select_pos = upper.find("SELECT")?;
    let from_rel = upper[select_pos..].find("FROM")?;
    let clause = &input[select_pos..select_pos + from_rel];

    // Outermost braces around the axis set.
    let open = clause.find('{')?;
    let close = clause.rfind('}')?;
    if close <= open {
        return None;
    }
    let body = clause[open + 1..close].trim();

    let up = body.to_uppercase();
    // A member range is a set in its own right: `{a : b}`.
    if let Some((from, to)) = parse_member_range(body) {
        return Some(SetExpr::MemberRange { from, to });
    }
    // Explicit member list (bare): `[D].[H].&[a],[D].[H].&[b]` — braces are
    // already stripped by the caller.
    if let Some(unames) = parse_member_list(body) {
        return Some(SetExpr::MemberList { unames });
    }
    for (fn_name, is_head) in [("HEAD(", true), ("TAIL(", false)] {
        if let Some(p) = up.find(fn_name) {
            let after = &body[p + fn_name.len()..];
            // Split the count from the source: a brace-wrapped set ends at its
            // closing brace; otherwise at the first top-level comma.
            let trimmed = after.trim_start();
            let (src_text, n_text) = if trimmed.starts_with('{') {
                let close = trimmed.find('}')?;
                (&trimmed[..=close], &trimmed[close + 1..])
            } else {
                let mut depth = 0i32;
                let mut comma = None;
                for (j, ch) in after.char_indices() {
                    match ch {
                        '[' | '(' => depth += 1,
                        ']' | ')' => {
                            depth -= 1;
                            if ch == ')' && depth == 0 {
                                break;
                            }
                        }
                        ',' if depth == 0 => {
                            comma = Some(j);
                            break;
                        }
                        _ => {}
                    }
                }
                let c = comma?;
                (&after[..c], &after[c + 1..])
            };
            let n: usize = n_text
                .trim()
                .trim_start_matches(',')
                .trim()
                .trim_end_matches(')')
                .trim()
                .parse()
                .ok()?;
            let src_core = src_text.trim().strip_prefix('{').unwrap_or(src_text.trim());
            let src_core = src_core.strip_suffix('}').unwrap_or(src_core);
            let src = if let Some((from, to)) = parse_member_range(src_core) {
                SetExpr::MemberRange { from, to }
            } else if src_text.contains("&[") || src_text.contains("&amp;[") {
                let unames = parse_member_list(src_core)?;
                SetExpr::MemberList { unames }
            } else {
                parse_set_source(src_core)?
            };
            return Some(if is_head {
                SetExpr::Head(Box::new(src), n)
            } else {
                SetExpr::Tail(Box::new(src), n)
            });
        }
    }
    parse_set_source(body)
}

/// Parse a member range `[D].[H].[L].&[a] : [D].[H].[L].&[b]`. Both endpoints
/// must be bracketed member unames; the `:` sits outside brackets.
fn parse_member_range(text: &str) -> Option<(String, String)> {
    let t = text
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();
    if !t.starts_with('[') {
        return None;
    }
    let mut depth = 0usize;
    let mut split = None;
    for (i, &c) in t.as_bytes().iter().enumerate() {
        match c {
            b'[' => depth += 1,
            b']' => depth = depth.saturating_sub(1),
            b':' if depth == 0 => {
                split = Some(i);
                break;
            }
            _ => {}
        }
    }
    let i = split?;
    let from = t[..i].trim();
    let to = t[i + 1..].trim();
    let is_uname = |s: &str| s.starts_with('[') && (s.contains("&[") || s.contains("&amp;["));
    (is_uname(from) && is_uname(to)).then(|| (from.to_string(), to.to_string()))
}

/// `[D].[H].[L].&[k1]&[k2]` → `(dim, level, "k1|k2")` (compound keys join
/// with `|`, the engine's path separator).
pub(crate) fn parse_level_member(uname: &str) -> Option<(String, String, String)> {
    let toks = bracket_tokens(uname, 8);
    if toks.len() < 4 {
        return None;
    }
    Some((toks[0].clone(), toks[2].clone(), toks[3..].join("|")))
}

/// Split an explicit member list (`[D].[H].&[a],[D].[H].&[b]`) on commas that
/// sit outside brackets. Returns None when the text isn't a member list.
fn parse_member_list(text: &str) -> Option<Vec<String>> {
    let t = text
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();
    if !t.starts_with('[') || !t.contains("&[") {
        return None;
    }
    let mut pieces: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_bracket = false;
    for ch in t.chars() {
        match ch {
            '[' => {
                in_bracket = true;
                cur.push(ch);
            }
            ']' => {
                in_bracket = false;
                cur.push(ch);
            }
            ',' if !in_bracket => {
                pieces.push(cur.trim().to_string());
                cur = String::new();
            }
            _ => cur.push(ch),
        }
    }
    pieces.push(cur.trim().to_string());
    if !pieces.is_empty() && pieces.iter().all(|p| p.starts_with('[')) {
        Some(pieces)
    } else {
        None
    }
}

/// Find `WITH MEMBER [Measures].[name] AS '<expr>'` where expr is exactly
/// `COUNT(<set>)`. Tolerates XML-escaped ampersands in member unames.
pub fn parse_calculated_count(input: &str) -> Option<CalculatedCount> {
    let up = input.to_uppercase();
    let wm = up.find("WITH MEMBER ")?;
    let rest = &input[wm + "WITH MEMBER ".len()..];
    // Member reference: [Measures].[Name]
    let open = rest.find("[Measures].")?;
    let after = &rest[open + "[Measures].".len()..];
    let b_open = after.find('[')?;
    let b_close = after[b_open..].find(']')?;
    let name = after[b_open + 1..b_open + b_close].to_string();

    let as_pos = up[wm..].find(" AS '")?;
    let expr_start = wm + as_pos + " AS '".len();
    let expr_rest = &input[expr_start..];
    let quote_end = expr_rest.find('\'')?;
    let expr = &expr_rest[..quote_end];
    let eup = expr.trim_start().to_uppercase();
    // The body must be exactly COUNT(<set>) — other calculated-member
    // expressions (cchildren etc.) are not handled here.
    if !eup.starts_with("COUNT(") {
        return None;
    }
    let cp = eup.find("COUNT(")?;
    let set_text = expr[cp + "COUNT(".len()..]
        .trim_end()
        .strip_suffix(')')
        .unwrap_or("")
        .replace("&amp;", "&");
    let set = if let Some(unames) = parse_member_list(&set_text) {
        SetExpr::MemberList { unames }
    } else {
        parse_set_source(&set_text)?
    };
    Some(CalculatedCount {
        member_name: name,
        set,
    })
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
    } else {
        let r = after_name.strip_prefix('=')?;
        (CmpOp::Eq, r)
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
    /// Tuples on the SELECT axis, one `Vec<MemberRef>` per tuple (e.g. batched
    /// CUBEVALUE with different slicers).
    pub select_tuples: Vec<Vec<MemberRef>>,
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
    /// A CUBESET-style set expression on the SELECT axis
    /// (`{ HEAD(...,n) }`, bare `{ [...].Members }`), if any.
    pub axis_set_expr: Option<SetExpr>,
    /// Calculated members whose body is `COUNT(<set>)`, in declaration order.
    pub calculated_counts: Vec<CalculatedCount>,
    /// Explicit level-set sources on the axes: `(dim, level)` pairs from
    /// `[Dim].[Hier].[Level].Members` (Excel's field-list level drag).
    pub axis_level_members: Vec<(String, String)>,
    /// Member ranges on the axes: `(dim, level, from_key, to_key)`.
    pub axis_member_ranges: Vec<(String, String, String, String)>,
    /// `DrilldownLevel(...)` targets on the axes (dimension + optional level
    /// expression/index). Without a level they drill to the top level below
    /// `(All)` — the whole-hierarchy drag.
    pub drilldown_targets: Vec<DrilldownTarget>,
}

/// The outer SELECT clause (between the first SELECT and FROM), used by the
/// axis scanners. Subquery SELECTs live inside FROM (...) and are excluded.
fn outer_select_clause(input: &str) -> &str {
    let upper = input.to_uppercase();
    let select_pos = upper.find("SELECT").unwrap_or(0);
    let from_pos = upper[select_pos..]
        .find("FROM")
        .map(|i| select_pos + i)
        .unwrap_or(input.len());
    &input[select_pos..from_pos]
}

/// Bracket contents (`[X]` -> `X`) in order, up to `max`.
pub(crate) fn bracket_tokens(text: &str, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while out.len() < max {
        let Some(open) = rest.find('[') else { break };
        let after = &rest[open + 1..];
        let Some(close) = after.find(']') else { break };
        out.push(after[..close].to_string());
        rest = &after[close + 1..];
    }
    out
}

/// Explicit level-set sources on the axes: `[Dim].[Hier].[Level].Members`.
/// Returns `(dim, level)` pairs in clause order, deduplicated. Bare
/// `[X].Members` and `[X].[Y].[(All)].Members` are not level sets.
/// `.AllMembers` (the Excel 2016 shape) is treated as the same level set.
fn parse_axis_level_members(input: &str) -> Vec<(String, String)> {
    let clause = outer_select_clause(input).replace(".AllMembers", ".Members");
    let clause = clause.as_str();
    let mut out: Vec<(String, String)> = Vec::new();
    let mut pos = 0;
    while let Some(i) = clause[pos..].find(".Members") {
        let abs = pos + i;
        // Walk backwards over the `.`-separated bracket chain.
        let mut rest = &clause[..abs];
        let mut segs: Vec<String> = Vec::new();
        while segs.len() < 4 && rest.ends_with(']') {
            let close = rest.len() - 1;
            let Some(open) = rest[..close].rfind('[') else {
                break;
            };
            segs.push(rest[open + 1..close].to_string());
            let before = &rest[..open];
            if let Some(stripped) = before.strip_suffix('.') {
                rest = stripped;
            } else {
                break;
            }
        }
        segs.reverse();
        if segs.len() == 3
            && segs[0] != "Measures"
            && !segs[2].eq_ignore_ascii_case("all")
            && !segs[2].eq_ignore_ascii_case("(all)")
        {
            let pair = (segs[0].clone(), segs[2].clone());
            if !out.contains(&pair) {
                out.push(pair);
            }
        }
        pos = abs + 1;
    }
    out
}

/// A `DrilldownLevel(...)` call on an axis.
#[derive(Debug, Clone, PartialEq)]
pub struct DrilldownTarget {
    pub dim: String,
    /// Level-expression argument (`DrilldownLevel(set, [D].[H].[Level])`).
    pub level: Option<String>,
    /// Numeric index argument (`DrilldownLevel(set, , N)`).
    pub index: Option<usize>,
}

/// Split top-level comma-separated arguments, respecting `()`/`{}`/`[]` nesting.
pub(crate) fn split_top_level_args(text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for ch in text.chars() {
        match ch {
            '(' | '{' | '[' => {
                depth += 1;
                cur.push(ch);
            }
            ')' | '}' | ']' => {
                depth -= 1;
                cur.push(ch);
            }
            ',' if depth == 0 => {
                args.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    args.push(cur.trim().to_string());
    args
}

/// Byte index of the `)` matching the `(` at `open`, if balanced.
pub(crate) fn matching_paren(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (i, ch) in text[open..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + i);
                }
            }
            _ => {}
        }
    }
    None
}

/// `DrilldownLevel(...)` targets on the axes: the dimension, plus an explicit
/// level expression or numeric index when present. Without either, the call
/// drills to the top level below `(All)` (Excel's whole-hierarchy drag).
fn parse_drilldown_targets(input: &str) -> Vec<DrilldownTarget> {
    let clause = outer_select_clause(input);
    let upper = clause.to_uppercase();
    let mut out: Vec<DrilldownTarget> = Vec::new();
    let mut pos = 0;
    while let Some(i) = upper[pos..].find("DRILLDOWNLEVEL") {
        let after_name = pos + i + "DRILLDOWNLEVEL".len();
        let Some(rel) = clause[after_name..].find('(') else {
            break;
        };
        let open = after_name + rel;
        let Some(close) = matching_paren(clause, open) else {
            break;
        };
        let args = split_top_level_args(&clause[open + 1..close]);
        let toks = args
            .first()
            .map(|a| bracket_tokens(a, 3))
            .unwrap_or_default();
        let from_all = toks.len() == 3
            && (toks[2].eq_ignore_ascii_case("all") || toks[2].eq_ignore_ascii_case("(all)"));
        if toks.len() >= 2 && toks[0] != "Measures" {
            let dim = toks[0].clone();
            let level = args.get(1).filter(|a| !a.is_empty()).and_then(|a| {
                let lv = bracket_tokens(a, 3);
                (lv.len() == 3).then(|| lv[2].clone())
            });
            let index = args.get(2).and_then(|a| a.trim().parse::<usize>().ok());
            // Only a drill from `(All)` (hierarchy drag) or an explicitly
            // named level/index is a level target.
            if (from_all || level.is_some() || index.is_some())
                && !out.iter().any(|t: &DrilldownTarget| t.dim == dim)
            {
                out.push(DrilldownTarget { dim, level, index });
            }
        }
        pos = close;
    }
    out
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

    // Plan 047: the front-end (lexer + AST) owns axis/filter extraction when it
    // can parse the statement; the legacy scanners remain as a transitional
    // fallback for shapes the AST does not model yet.
    let frontend = crate::mdx::frontend::parse_select(input);
    let (axis_dimension_ids, axis_level_members, axis_member_ranges, axis_set_expr) =
        match &frontend {
            Ok(sel) => (
                crate::mdx::frontend::axis_dimension_ids(sel),
                crate::mdx::frontend::axis_level_members(sel),
                crate::mdx::frontend::axis_member_ranges(sel),
                crate::mdx::frontend::set_probe_expr(sel),
            ),
            Err(_) => (
                parse_axis_dimension_ids(before_from),
                parse_axis_level_members(before_from),
                Vec::new(),
                parse_axis_set_expr(input),
            ),
        };

    // Parse excluded members from DrilldownMember if present.
    // Filter/set-op derivations from the same AST (see above).
    let (
        where_members,
        subquery_members,
        select_members,
        select_tuples,
        excluded_members,
        drilldown_member_hierarchy,
        axis_set_op,
    ) = match &frontend {
        Ok(sel) => (
            crate::mdx::frontend::where_members(sel),
            crate::mdx::frontend::subquery_members(sel),
            crate::mdx::frontend::select_members(sel),
            crate::mdx::frontend::select_tuples(sel),
            crate::mdx::frontend::excluded_members(sel),
            crate::mdx::frontend::drilldown_member_hierarchy(sel),
            crate::mdx::frontend::axis_set_op(sel),
        ),
        Err(_) => {
            let excluded = if has_drilldown_member(input) {
                parse_excluded_members_from_mdx(input)
            } else {
                Vec::new()
            };
            let hierarchy = if has_drilldown_member(input) {
                parse_drilldown_member_hierarchy_from_mdx(input)
            } else {
                None
            };
            let all_subquery = find_all_subquery_members(input);
            let mut sq: Vec<MemberRef> = all_subquery.into_iter().flatten().collect();
            sq.extend(find_subselect_members(input));
            (
                find_where_clause(input).unwrap_or_default(),
                sq,
                find_select_tuple_members(input),
                find_select_tuples(input),
                excluded,
                hierarchy,
                detect_axis_set_op(input),
            )
        }
    };

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
        select_tuples,
        main_dim: detect_axis_dimension(before_from),
        cchildren_target: detect_cchildren_target(input),
        calculated_members_pat: detect_calculated_members_pat(input),
        selected_measure,
        selected_measures: select_measures,
        cube_name,
        axis_dimension_ids,
        excluded_members,
        drilldown_member_hierarchy,
        axis_set_op,
        axis_set_expr,
        calculated_counts: parse_calculated_count(input).into_iter().collect(),
        axis_level_members,
        axis_member_ranges,
        drilldown_targets: parse_drilldown_targets(before_from),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_features_are_detected() {
        for (mdx, needle) in [
            (
                "SELECT {HEAD(YTD([Date].[Date].[Year].&[2024]),1)} ON 0 FROM [Sales]",
                "YTD()",
            ),
            (
                "SELECT {PeriodsToDate([Date].[Date].[Year], [Date].[Date].[Month].&[2024]&[6])} ON 1 FROM [Sales]",
                "PeriodsToDate()",
            ),
            (
                "WITH SET [Last30] AS 'x' SELECT {[Measures].[Revenue]} ON 0 FROM [Sales]",
                "named sets",
            ),
            (
                "SELECT {[Measures].[Revenue]} ON 0 FROM [Sales] WHERE ({[Date].[Date].[Year].&[2022] : [Date].[Date].[Year].&[2024]})",
                "member ranges",
            ),
            (
                "WITH MEMBER [Measures].[XL_SD] AS 'COUNT({[Date].[Date].[Year].&[2022] : [Date].[Date].[Year].&[2024]})' SELECT {[Measures].[XL_SD]} ON 0 FROM [Sales]",
                "member ranges",
            ),
            (
                "SELECT {[Measures].[Revenue]} ON 0, {[Date].[Date].[Year].&[2022] : [Date].[Date].[Year].&[2024]} ON 1 FROM [Sales]",
                "member ranges",
            ),
            (
                "SELECT {[Measures].[Revenue]} ON 0 FROM [Sales] WHERE FILTER([Date].[Date].[Date].Members, [Date].[Date].CurrentMember.Member_Value >= 1)",
                "member-property filters",
            ),
            (
                "SELECT {[Measures].[Revenue]} ON 0 FROM [Sales] WHERE FILTER([Date].[Date].[Date].Members, DateAdd(\"d\", -30, VBA![Date]()) <= 1)",
                "date arithmetic",
            ),
        ] {
            let reason = unsupported_features(mdx).unwrap_or_else(|| panic!("not detected: {mdx}"));
            assert!(reason.contains(needle), "{mdx} → {reason}");
        }
    }

    #[test]
    fn supported_mdx_is_not_flagged() {
        for mdx in [
            "SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales]",
            "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Date].[Date].[All]},,,INCLUDE_CALC_MEMBERS)}) ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue])",
            "SELECT {([Measures].[Revenue],[Category].[Category].&[Electronics])} ON 0 FROM [Sales]",
            "SELECT [Category].[Category].Members ON ROWS, {[Measures].[Revenue]} ON COLUMNS FROM [Sales]",
            "SELECT {HEAD([Date].[Date].[Year].Members,1)} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            "WITH MEMBER [Measures].[XL_SD] AS 'COUNT([Date].[Date].[Year].Members)' SELECT {[Measures].[XL_SD]} ON 0 FROM [Sales]",
            "SELECT {[Measures].[Revenue]} ON 0 FROM [Sales] WHERE FILTER([Category].[Category].Members, [Measures].[Revenue] > 100)",
            "SELECT {HEAD({[Date].[Date].[Year].&[2022] : [Date].[Date].[Year].&[2024]},1)} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            "SELECT {[Date].[Date].[Year].&[2022] : [Date].[Date].[Year].&[2024]} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
        ] {
            assert!(unsupported_features(mdx).is_none(), "false positive: {mdx}");
        }
    }

    #[test]
    fn parse_member_all() {
        let (rest, m) = member_ref("[ProductCategory].[ProductCategory].[All]").unwrap();
        assert_eq!(m, MemberRef::All(DimRef::Cube("ProductCategory".into())));
        assert!(rest.is_empty());
    }

    #[test]
    fn parse_member_leaf() {
        let (rest, m) = member_ref("[ProductCategory].[ProductCategory].&[Category A]").unwrap();
        assert_eq!(
            m,
            MemberRef::Leaf {
                dim: DimRef::Cube("ProductCategory".into()),
                key: "Category A".into(),
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
        let input = "WHERE ([Region].[Region].[All],[Measures].[Total Sales])";
        let (_rest, members) = where_clause(input).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0], MemberRef::All(DimRef::Cube("Region".into())));
        assert_eq!(members[1], MemberRef::Measure("Total Sales".into()));
    }

    #[test]
    fn parse_where_leaf() {
        let input = "WHERE ([ProductCategory].[ProductCategory].&[Category B],[Measures].[Total Sales])";
        let (_rest, members) = where_clause(input).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(
            members[0],
            MemberRef::Leaf {
                dim: DimRef::Cube("ProductCategory".into()),
                key: "Category B".into(),
                level: None,
            }
        );
    }

    #[test]
    fn parse_subquery() {
        let _input = "SELECT ({[ProductCategory].[ProductCategory].&[Category A],[ProductCategory].[ProductCategory].&[Category C]}) ON COLUMNS FROM [Model]";
        let (_rest, m) = subquery_body("SELECT ({[ProductCategory].[ProductCategory].&[Category A],[ProductCategory].[ProductCategory].&[Category C]})").unwrap();
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

#[cfg(test)]
mod set_expr_tests {

    use super::*;

    #[test]
    fn parses_head_over_level_members() {
        let mdx = "SELECT {HEAD([Date].[Date].[Year].Members,1)} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL";
        assert_eq!(
            parse_axis_set_expr(mdx),
            Some(SetExpr::Head(
                Box::new(SetExpr::LevelMembers {
                    dim: "Date".into(),
                    level: Some("Year".into())
                }),
                1
            ))
        );
    }

    #[test]
    fn parses_bare_level_members() {
        let mdx = "SELECT {[Date].[Date].[Year].Members} ON 0 FROM [Sales]";
        assert_eq!(
            parse_axis_set_expr(mdx),
            Some(SetExpr::LevelMembers {
                dim: "Date".into(),
                level: Some("Year".into())
            })
        );
    }

    #[test]
    fn parses_tail_and_all_members() {
        let mdx = "SELECT {TAIL([Date].[Date].[(All)].Members,2)} ON 0 FROM [Sales]";
        assert_eq!(
            parse_axis_set_expr(mdx),
            Some(SetExpr::Tail(
                Box::new(SetExpr::AllMembers { dim: "Date".into() }),
                2
            ))
        );
    }

    #[test]
    fn ignores_non_set_axes() {
        assert_eq!(
            parse_axis_set_expr("SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales]"),
            None
        );
        assert_eq!(
            parse_axis_set_expr(
                "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Date].[Date].[All]},,,INCLUDE_CALC_MEMBERS)}) ON COLUMNS FROM [Sales]"
            ),
            None
        );
    }

    #[test]
    fn parses_all_members_level() {
        let mdx = "SELECT {[Measures].[Revenue]} ON COLUMNS, {[Date].[Date].[Quarter].AllMembers} ON ROWS FROM [Sales]";
        assert_eq!(
            parse_axis_level_members(mdx),
            vec![("Date".to_string(), "Quarter".to_string())]
        );
    }

    #[test]
    fn parses_drilldown_level_arguments() {
        let level_expr = "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Date].[Date].[All]}, [Date].[Date].[Quarter])}) ON COLUMNS FROM [Sales]";
        let t = parse_drilldown_targets(level_expr);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].dim, "Date");
        assert_eq!(t[0].level.as_deref(), Some("Quarter"));
        assert_eq!(t[0].index, None);

        let index = "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Date].[Date].[All]},,1)}) ON COLUMNS FROM [Sales]";
        let t = parse_drilldown_targets(index);
        assert_eq!(t[0].index, Some(1));
        assert_eq!(t[0].level, None);

        // Plain hierarchy drag: no level or index argument.
        let plain = "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Date].[Date].[All]},,,INCLUDE_CALC_MEMBERS)}) ON COLUMNS FROM [Sales]";
        let t = parse_drilldown_targets(plain);
        assert_eq!((t[0].level.clone(), t[0].index), (None, None));
    }

    #[test]
    fn parses_subselect_members() {
        let mdx = "SELECT {[Measures].[Revenue]} ON COLUMNS FROM (SELECT {[Date].[Date].[Year].&[2024]} ON COLUMNS FROM [Sales])";
        let members = find_subselect_members(mdx);
        assert_eq!(members.len(), 1, "{members:?}");
        match &members[0] {
            MemberRef::Leaf {
                dim: DimRef::Cube(name),
                key,
                ..
            } => {
                assert_eq!(name, "Date");
                assert_eq!(key, "2024");
            }
            other => panic!("expected leaf member, got {other:?}"),
        }
    }

    #[test]
    fn parses_calculated_count() {
        let mdx = "WITH MEMBER [Measures].[XL_SD] AS 'COUNT([Date].[Date].[Year].Members)' SELECT {[Measures].[XL_SD]} ON 0 FROM [Sales] CELL PROPERTIES VALUE";
        let cc = parse_calculated_count(mdx).expect("calculated count");
        assert_eq!(cc.member_name, "XL_SD");
        assert_eq!(
            cc.set,
            SetExpr::LevelMembers {
                dim: "Date".into(),
                level: Some("Year".into())
            }
        );
    }

    #[test]
    fn parses_calculated_count_over_explicit_list() {
        let mdx = "WITH MEMBER [Measures].[XL_SD] AS 'COUNT({[Date].[Date].[Year].&[2020],[Date].[Date].[Year].&[2021]})' SELECT {[Measures].[XL_SD]} ON 0 FROM [Sales] CELL PROPERTIES VALUE";
        let cc = parse_calculated_count(mdx).expect("calculated count");
        assert_eq!(
            cc.set,
            SetExpr::MemberList {
                unames: vec![
                    "[Date].[Date].[Year].&[2020]".into(),
                    "[Date].[Date].[Year].&[2021]".into()
                ]
            }
        );
    }

    #[test]
    fn calculated_count_ignores_non_count_bodies() {
        let mdx = "WITH MEMBER [Measures].cChildren As 'AddCalculatedMembers([Channel].[Channel].currentmember.children).count' Set FilteredMembers As '{[Channel].[Channel].&[Wholesale]}' Select {[Measures].cChildren} on ROWS, Hierarchize(Generate(FilteredMembers, Ascendants([Channel].[Channel].currentmember))) DIMENSION PROPERTIES PARENT_UNIQUE_NAME, MEMBER_TYPE ON COLUMNS FROM [Sales]";
        assert_eq!(parse_calculated_count(mdx), None);
    }
}
