//! `ParsedMdx` — the flat view the semantic layer consumes.
//!
//! Plan 047 moved axis/filter extraction to the front-end (`lexer` + `ast` +
//! `frontend`): `parse_mdx` derives members, ranges, set probes, exclusions and
//! axis set ops from the AST and records `parse_error` when the statement is
//! outside the supported subset. The remaining scanners (classification flags,
//! drilldown targets, calculated counts, properties) are the next migration
//! step. Dimension names are dynamic — no hardcoded dimension vocabulary.
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, take_while},
    character::complete::{char, multispace0},
    multi::separated_list0,
    sequence::delimited,
};

// ---- unsupported constructs (plan 046) ----

/// Constructs MallardCube does not support yet, detected from the AST (plan
/// 047) so the execute path faults with a named reason instead of degrading to
/// a dropped axis or a wrong-hierarchy cellset.
pub fn unsupported_features(mdx: &str) -> Option<String> {
    let sel = match crate::mdx::frontend::parse_select(mdx) {
        Ok(sel) => sel,
        // A statement the front-end cannot parse faults with its reason.
        Err(e) => return Some(format!("unsupported MDX: {e}")),
    };
    use crate::mdx::frontend as fe;

    // Named sets (`WITH SET`): bodies we can parse are expanded by the
    // semantic layer; an unparseable body faults.
    let named = fe::named_sets(&sel);
    if named.len() < sel.with_sets.len() {
        return Some(
            "named sets (`WITH SET`) with an unparseable body are not supported yet".into(),
        );
    }
    // MDX time functions.
    // Most time-intelligence functions lower to date windows; the balance
    // functions (`ClosingPeriod`/`OpeningPeriod`) are not supported yet.
    const TIME_FNS: [&str; 2] = ["CLOSINGPERIOD", "OPENINGPERIOD"];
    if let Some(name) = fe::first_call(&sel, &TIME_FNS) {
        return Some(format!(
            "the MDX time function `{name}()` is not supported yet"
        ));
    }
    // `DateAdd`/`VBA!` are lowered inside member-value filters; elsewhere they
    // are not handled yet.
    let lowered_windows = fe::member_value_filters(&sel).len();
    if (fe::first_call(&sel, &["DATEADD"]).is_some() || fe::bodies_contain(&sel, "VBA!"))
        && lowered_windows == 0
    {
        return Some(
            "MDX date arithmetic (`DateAdd`/`VBA!`) outside a member-value filter is not supported yet"
                .into(),
        );
    }
    // Member-value filters we can lower (`Filter(set, Member_Value >=
    // DateAdd(…))`); anything else referencing member properties faults.
    if fe::member_property_filter_count(&sel) > lowered_windows {
        return Some(
            "member-property filters (`Filter` over `Member_Value`/`Member_Key`) are not supported yet"
                .into(),
        );
    }
    // Label filters (`Filter(set, InStr(caption, …) > 0)`) are not lowered;
    // faulting beats returning the unfiltered set while Excel shows the filter
    // as applied (plan 048).
    if fe::unsupported_filter_count(&sel) > 0 {
        return Some(
            "label filters (`Filter` over member captions/names) are not supported yet — \
             use Keep Only Selected Items or a value filter"
                .into(),
        );
    }
    // Member ranges inside a quoted calculated-member body (`COUNT({a : b})`)
    // are not handled yet; axis and slicer ranges are.
    if fe::bodies_contain_range(&sel) {
        return Some(
            "member ranges inside a calculated member (`COUNT({a : b})`) are not supported yet"
                .into(),
        );
    }
    // (A braced `{range}` beside a braced measure set used to fault here; the
    // structural axis flags fixed its classification — plan 047 increment 4.)
    None
}

/// Is there a `] : [` member range in this text? (Used for quoted bodies the
/// AST cannot see into.)
pub(crate) fn text_has_member_range(text: &str) -> bool {
    member_range_pos(text).is_some()
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
/// `year` column for `[Date].[Calendar].[Year].&[2024]`). Excel emits this for
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
/// `WITH MEMBER [Measures].[XL_SD] AS 'COUNT([Date].[Calendar].[Year].Members)'`.
#[derive(Debug, Clone, PartialEq)]
pub struct CalculatedCount {
    pub member_name: String,
    pub set: SetExpr,
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
    // Plan 047: the set body goes through the front-end (it understands
    // `Filter(...)` windows, ranges, HEAD/Tail and time-intelligence calls).
    let set = crate::mdx::frontend::parse_set_expr(&set_text)
        .ok()
        .and_then(|e| crate::mdx::frontend::set_expr_from_ast(&e))?;
    Some(CalculatedCount {
        member_name: name,
        set,
    })
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
    /// Member ranges in the slicer (`WHERE ({a : b})`).
    pub where_member_ranges: Vec<(String, String, String, String)>,
    /// When the front-end cannot parse the statement, the reason. The execute
    /// path faults on this instead of degrading to a dropped axis (plan 047).
    pub parse_error: Option<String>,
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
    /// Hierarchy name from the set argument (`[D].[H].[All]`). A hierarchy that
    /// names a level — the key attribute hierarchy `[Date].[Full Date]` — drills
    /// into that level rather than the top one (plan 048).
    pub hierarchy: Option<String>,
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

/// `[D].[H].[L].&[k1]&[k2]` → `(dim, level, "k1|k2")` (compound keys join
/// with `|`, the engine's path separator). Used by the range planner.
pub(crate) fn parse_level_member(uname: &str) -> Option<(String, String, String)> {
    let toks = bracket_tokens(uname, 8);
    if toks.len() < 4 {
        return None;
    }
    Some((toks[0].clone(), toks[2].clone(), toks[3..].join("|")))
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
            let hierarchy = toks.get(1).cloned();
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
                out.push(DrilldownTarget {
                    dim,
                    hierarchy,
                    level,
                    index,
                });
            }
        }
        pos = close;
    }
    out
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

    // Plan 047: the front-end (lexer + AST) is the source of truth for axis and
    // filter extraction. A statement it cannot parse carries `parse_error`, and
    // the execute path faults on it instead of degrading.
    let frontend = crate::mdx::frontend::parse_select(input);
    let parse_error = frontend.as_ref().err().map(|e| e.to_string());
    let (
        axis_dimension_ids,
        axis_level_members,
        axis_member_ranges,
        where_member_ranges,
        axis_set_expr,
    ) = match &frontend {
        Ok(sel) => (
            crate::mdx::frontend::axis_dimension_ids(sel),
            crate::mdx::frontend::axis_level_members(sel),
            crate::mdx::frontend::axis_member_ranges(sel),
            crate::mdx::frontend::where_member_ranges(sel),
            crate::mdx::frontend::set_probe_expr(sel),
        ),
        Err(_) => (Vec::new(), Vec::new(), Vec::new(), Vec::new(), None),
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
        Err(_) => (
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            None,
        ),
    };

    // All measures referenced in the SELECT clause, in order. A single cell
    // holds one measure; batched CUBEVALUE cells produce several.
    let select_measures = match &frontend {
        Ok(sel) => crate::mdx::frontend::selected_measures(sel),
        Err(_) => Vec::new(),
    };
    let axis_presence = match &frontend {
        Ok(sel) => crate::mdx::frontend::axis_presence(sel),
        Err(_) => (false, false),
    };
    let mentions_measure_derived = match &frontend {
        Ok(sel) => crate::mdx::frontend::mentions_measure(sel),
        Err(_) => false,
    };
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
        has_rows: axis_presence.1,
        has_cols: axis_presence.0,
        has_crossjoin: has_crossjoin(input),
        has_drilldown: has_drilldown(input),
        has_dot_members: has_dot_members(input),
        has_dot_children: has_dot_children(input),
        has_with_member_cchildren: has_with_member_cchildren(input),
        has_where_all_measure: is_slicer_all_measure(input),
        has_drilldown_member: has_drilldown_member(input),
        has_measures: mentions_measure_derived,
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
        where_member_ranges,
        parse_error,
        drilldown_targets: parse_drilldown_targets(before_from),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Plan 047: the captured Excel workload is the parser regression corpus.
    // Every statement must parse (no parse_error) and keep the derivations the
    // semantic layer relies on.
    #[test]
    fn workload_corpus_parses_and_derives() {
        let text = std::fs::read_to_string("scripts/bench-workload.jsonl")
            .expect("read the captured workload corpus");
        let mut statements: Vec<String> = Vec::new();
        for line in text.lines() {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let xml = v["request_xml"].as_str().unwrap_or("");
            let Some(start) = xml.find("<Statement>") else {
                continue;
            };
            let Some(end) = xml[start..].find("</Statement>") else {
                continue;
            };
            statements.push(xml[start + "<Statement>".len()..start + end].to_string());
        }
        assert!(!statements.is_empty(), "the corpus should not be empty");
        for mdx in &statements {
            // The corpus is captured traffic, not only MDX: DRILLTHROUGH and
            // DAX statements have their own paths and their own tests.
            if !crate::mdx_semantic::is_mdx_select(mdx) {
                continue;
            }
            let parsed = parse_mdx(mdx);
            assert!(
                parsed.parse_error.is_none(),
                "corpus statement must parse: {mdx}\n  error: {:?}",
                parsed.parse_error
            );
            assert!(parsed.cube_name.is_some(), "cube name missing for {mdx}");
            if mdx.contains("Drilldown") {
                assert!(
                    !parsed.axis_dimension_ids.is_empty(),
                    "drilldown axis dimensions missing for {mdx}"
                );
            }
        }
    }

    #[test]
    fn unsupported_features_are_detected() {
        for (mdx, needle) in [
            (
                "SELECT {ClosingPeriod([Date].[Calendar].[Year], [Date].[Calendar].[Month].&[2024]&[2]&[6])} ON 1 FROM [Sales]",
                "ClosingPeriod()",
            ),
            (
                "WITH SET [Last30] AS 'HEAD(Filter([Date].[Calendar].[Date].Members, [Date].[Calendar].CurrentMember.Member_Value >= 1), 1)' SELECT {[Measures].[Revenue]} ON 0 FROM [Sales]",
                "member-property filters",
            ),
            (
                "WITH MEMBER [Measures].[XL_SD] AS 'COUNT({[Date].[Calendar].[Year].&[2022] : [Date].[Calendar].[Year].&[2024]})' SELECT {[Measures].[XL_SD]} ON 0 FROM [Sales]",
                "member ranges",
            ),
            (
                "WITH MEMBER [Measures].[XL_SD] AS 'COUNT({[Date].[Calendar].[Year].&[2022] : [Date].[Calendar].[Year].&[2024]})' SELECT {[Measures].[XL_SD]} ON 0 FROM [Sales]",
                "member ranges",
            ),
            (
                "SELECT {[Measures].[Revenue]} ON 0 FROM [Sales] WHERE FILTER([Date].[Calendar].[Date].Members, [Date].[Calendar].CurrentMember.Member_Value >= 1)",
                "member-property filters",
            ),
            (
                "SELECT {[Measures].[Revenue]} ON 0 FROM [Sales] WHERE FILTER([Date].[Calendar].[Date].Members, DateAdd(\"d\", -30, VBA![Date]()) <= 1)",
                "date arithmetic",
            ),
            (
                // A label filter: previously dropped silently, returning the
                // unfiltered set while Excel showed the filter as applied.
                "SELECT NON EMPTY Hierarchize({Filter({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)}, InStr([Category].[Category].CurrentMember.MEMBER_CAPTION, \"Bo\") > 0)}) ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue])",
                "label filters",
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
            "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}) ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue])",
            "SELECT {([Measures].[Revenue],[Category].[Category].&[Electronics])} ON 0 FROM [Sales]",
            "SELECT [Category].[Category].Members ON ROWS, {[Measures].[Revenue]} ON COLUMNS FROM [Sales]",
            "SELECT {HEAD([Date].[Calendar].[Year].Members,1)} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            "WITH MEMBER [Measures].[XL_SD] AS 'COUNT([Date].[Calendar].[Year].Members)' SELECT {[Measures].[XL_SD]} ON 0 FROM [Sales]",
            "SELECT {[Measures].[Revenue]} ON 0 FROM [Sales] WHERE FILTER([Category].[Category].Members, [Measures].[Revenue] > 100)",
            "SELECT {HEAD({[Date].[Calendar].[Year].&[2022] : [Date].[Calendar].[Year].&[2024]},1)} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            "SELECT {HEAD(YTD([Date].[Calendar].[Year].&[2024]),1)} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            "SELECT {YTD([Date].[Calendar].[Month].&[2024]&[6])} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            "SELECT {PeriodsToDate([Date].[Calendar].[Year], [Date].[Calendar].[Month].&[2024]&[2]&[6])} ON 1 FROM [Sales]",
            "WITH SET [Last30] AS 'Filter([Date].[Calendar].[Date].Members, [Date].[Calendar].CurrentMember.Member_Value >= DateAdd(\"d\", -30, VBA![Date]()))' SELECT {[Measures].[Revenue]} ON 0 FROM [Sales]",
            "SELECT {ParallelPeriod([Date].[Calendar].[Year], -1, [Date].[Calendar].[Year].&[2024])} ON 1 FROM [Sales]",
            "SELECT {LastPeriods(3, [Date].[Calendar].[Year].&[2024])} ON 1 FROM [Sales]",
            "SELECT {[Date].[Calendar].[Year].&[2022] : [Date].[Calendar].[Year].&[2024]} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
            "SELECT {[Measures].[Revenue]} ON 0, {[Date].[Calendar].[Year].&[2022] : [Date].[Calendar].[Year].&[2024]} ON 1 FROM [Sales]",
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
        let input =
            "WHERE ([ProductCategory].[ProductCategory].&[Category B],[Measures].[Total Sales])";
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
    fn parses_all_members_level() {
        let mdx = "SELECT {[Measures].[Revenue]} ON COLUMNS, {[Date].[Calendar].[Quarter].AllMembers} ON ROWS FROM [Sales]";
        assert_eq!(
            parse_axis_level_members(mdx),
            vec![("Date".to_string(), "Quarter".to_string())]
        );
    }

    #[test]
    fn parses_drilldown_level_arguments() {
        let level_expr = "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Date].[Calendar].[All]}, [Date].[Calendar].[Quarter])}) ON COLUMNS FROM [Sales]";
        let t = parse_drilldown_targets(level_expr);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].dim, "Date");
        assert_eq!(t[0].level.as_deref(), Some("Quarter"));
        assert_eq!(t[0].index, None);

        let index = "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,1)}) ON COLUMNS FROM [Sales]";
        let t = parse_drilldown_targets(index);
        assert_eq!(t[0].index, Some(1));
        assert_eq!(t[0].level, None);

        // Plain hierarchy drag: no level or index argument.
        let plain = "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}) ON COLUMNS FROM [Sales]";
        let t = parse_drilldown_targets(plain);
        assert_eq!((t[0].level.clone(), t[0].index), (None, None));
    }

    #[test]
    fn parses_calculated_count() {
        let mdx = "WITH MEMBER [Measures].[XL_SD] AS 'COUNT([Date].[Calendar].[Year].Members)' SELECT {[Measures].[XL_SD]} ON 0 FROM [Sales] CELL PROPERTIES VALUE";
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
        let mdx = "WITH MEMBER [Measures].[XL_SD] AS 'COUNT({[Date].[Calendar].[Year].&[2020],[Date].[Calendar].[Year].&[2021]})' SELECT {[Measures].[XL_SD]} ON 0 FROM [Sales] CELL PROPERTIES VALUE";
        let cc = parse_calculated_count(mdx).expect("calculated count");
        assert_eq!(
            cc.set,
            SetExpr::MemberList {
                unames: vec![
                    "[Date].[Calendar].[Year].&[2020]".into(),
                    "[Date].[Calendar].[Year].&[2021]".into()
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
