/// MDX parsing, extraction, and semantic classification.
///
/// Converts raw Excel MDX probe/query strings into a `SemanticQuery`
/// so that response builders don't need to touch MDX strings directly.
///
/// Classification is now driven by `ParsedMdx` — structural flags
/// set by the nom parser — instead of bare `contains(...)` chains.
use crate::mdx_parser::{
    AxisSetOp, CChildrenTarget, CalculatedCount, CalculatedMembersPat, DimRef, MemberRef,
    ParsedMdx, SetExpr,
};

pub fn is_dax(statement: &str) -> bool {
    let trimmed = statement.trim_start();
    let upper = trimmed.to_uppercase();
    upper.starts_with("EVALUATE") || upper.starts_with("DEFINE")
}

pub fn is_drillthrough(statement: &str) -> bool {
    let upper = statement.trim_start().to_uppercase();
    upper.starts_with("DRILLTHROUGH")
}

/// SSAS statement Excel sends when the user hits Refresh on an OLAP pivot
/// (`REFRESH CUBE [<cube>]`). The proxy's data is live, so refreshing is a
/// no-op that must succeed: a fault here makes Excel report "The query did
/// not run" and abort the refresh (plan 048).
pub fn is_refresh_cube(statement: &str) -> bool {
    let mut words = statement.split_whitespace();
    matches!(words.next(), Some(w) if w.eq_ignore_ascii_case("REFRESH"))
        && matches!(words.next(), Some(w) if w.eq_ignore_ascii_case("CUBE"))
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
    /// Hierarchy level name for level-qualified filters (e.g. "Year" in
    /// `[Date].[Calendar].[Year].&[2024]`), so the SQL can filter the level column.
    pub level: Option<String>,
    /// Inclusive member range on `level` (`{[D].[H].[L].&[a] : [D].[H].[L].&[b]}`):
    /// `(from_key, to_key)` in the engine's `|`-joined key format.
    pub range: Option<(String, String)>,
    /// Period-to-date window on a date role's full-date column (`YTD(m)`).
    pub date_window: Option<DateWindow>,
}

pub use crate::mdx::ast::DateWindow;

/// One tuple on the SELECT axis (measure + member slicers), e.g. batched
/// CUBEVALUE cells with different slicers.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisTuple {
    pub measure: Option<String>,
    pub filters: Vec<DimensionFilter>,
}

/// Build per-tuple filters from a tuple's members (the Leaf members only).
fn filters_from_tuple_members(members: &[MemberRef]) -> Vec<DimensionFilter> {
    let mut result: Vec<DimensionFilter> = Vec::new();
    for m in members {
        if let MemberRef::Leaf { dim, key, level } = m {
            let dim_str = dim_ref_str(dim);
            if let Some(df) = result.iter_mut().find(|f| f.dimension == dim_str) {
                if !df.members.contains(key) {
                    df.members.push(key.clone());
                }
            } else {
                result.push(DimensionFilter {
                    dimension: dim_str,
                    members: vec![key.clone()],
                    level: level.clone(),
                    range: None,
                    date_window: None,
                });
            }
        }
    }
    result
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

/// Lower MDX date windows into filters: `(dim, level, window)`.
///
/// Two forms are supported:
/// - period-to-date calls (`YTD(m)`, `QTD(m)`, `MTD(m)`, `PeriodsToDate(level, m)`):
///   the anchor member's key parts map to the model's levels by index, and the
///   SQL bounds the window with `date_trunc(<period>, anchor) .. anchor`;
/// - member-value windows (`Filter(<level set>, CurrentMember.Member_Value >=
///   DateAdd("d", -30, VBA![Date]()))`): the date column is compared to
///   `CURRENT_DATE + INTERVAL '<amount> <unit>'`.
///
/// Named-set references (`WITH SET [x] AS '…'`) are expanded first.
fn date_windows(
    mdx: &str,
    model: &crate::engine::model::SemanticModel,
) -> Vec<(String, String, DateWindow)> {
    use crate::mdx::ast::{CmpOp, Expr};

    /// Align a member key path to the model's levels from the anchor's level (a
    /// short key anchors at that level), returning `(level name, value)` pins.
    fn align_pins(
        model: &crate::engine::model::SemanticModel,
        dim: &str,
        level: &str,
        parts: &[String],
    ) -> Vec<(String, String)> {
        model
            .dim_def_opt(dim)
            .and_then(|d| {
                let level_idx = d.levels.iter().position(|l| l.name == level)?;
                let start = (level_idx + 1).saturating_sub(parts.len());
                Some(
                    d.levels[start..=level_idx]
                        .iter()
                        .map(|l| l.name.clone())
                        .zip(parts.iter().cloned())
                        .collect(),
                )
            })
            .unwrap_or_default()
    }

    fn period_for(name: &str) -> Option<&'static str> {
        match name.to_uppercase().as_str() {
            "YTD" => Some("year"),
            "QTD" => Some("quarter"),
            "MTD" => Some("month"),
            _ => None,
        }
    }
    fn level_period(level: &str) -> Option<&'static str> {
        let l = level.to_lowercase();
        if l.contains("year") {
            Some("year")
        } else if l.contains("quarter") {
            Some("quarter")
        } else if l.contains("month") {
            Some("month")
        } else if l.contains("day") || l.contains("date")
        {
            Some("day")
        } else {
            None
        }
    }
    fn anchor_of(m: &crate::mdx::ast::MemberRef) -> Option<(String, String, Vec<String>)> {
        let level = m.level()?.to_string();
        let key = m.key.clone()?;
        let parts: Vec<String> = key.split('|').map(str::to_string).collect();
        Some((m.dim().to_string(), level, parts))
    }
    fn unit_name(s: &str) -> Option<String> {
        Some(match s.to_lowercase().as_str() {
            "d" | "dd" | "day" => "day".into(),
            "ww" | "wk" | "week" => "week".into(),
            "m" | "mm" | "month" => "month".into(),
            "yyyy" | "yy" | "year" => "year".into(),
            _ => return None,
        })
    }
    fn date_shift(e: &Expr) -> Option<(i64, String)> {
        match e {
            Expr::Call { name, args } if name.eq_ignore_ascii_case("DateAdd") => {
                let unit = match args.first() {
                    Some(Expr::Str(s)) => unit_name(s)?,
                    _ => return None,
                };
                let amount = match args.get(1) {
                    Some(Expr::Number(n)) => n.parse().ok()?,
                    _ => return None,
                };
                match args.get(2) {
                    Some(Expr::Call { name, .. }) if name == "VBA_DATE" => Some((amount, unit)),
                    _ => None,
                }
            }
            Expr::Call { name, .. } if name == "VBA_DATE" => Some((0, "day".into())),
            _ => None,
        }
    }
    fn is_member_value(e: &Expr) -> bool {
        e.as_member().is_some_and(|m| {
            m.parts
                .iter()
                .any(|p| p.eq_ignore_ascii_case("Member_Value"))
        })
    }

    let Ok(sel) = crate::mdx::frontend::parse_select(mdx) else {
        return Vec::new();
    };
    let named = crate::mdx::frontend::named_sets(&sel);
    let mut out: Vec<(String, String, DateWindow)> = Vec::new();

    fn walk(
        e: &Expr,
        model: &crate::engine::model::SemanticModel,
        out: &mut Vec<(String, String, DateWindow)>,
    ) {
        match e {
            Expr::Call { name, args } => {
                let period = period_for(name).or_else(|| {
                    if name.eq_ignore_ascii_case("PeriodsToDate") {
                        args.first()
                            .and_then(|a| a.as_member())
                            .and_then(|m| m.level())
                            .and_then(level_period)
                    } else {
                        None
                    }
                });
                if let Some(period) = period {
                    let anchor_member = if name.eq_ignore_ascii_case("PeriodsToDate") {
                        args.get(1).and_then(|a| a.as_member())
                    } else {
                        args.first().and_then(|a| a.as_member())
                    };
                    if let Some(m) = anchor_member
                        && let Some((dim, level, parts)) = anchor_of(m)
                    {
                        // Align the key path to the level chain from the
                        // anchor's level (a short key anchors at that level).
                        let pins = align_pins(model, &dim, &level, &parts);
                        out.push((
                            dim,
                            level,
                            DateWindow::ToDate {
                                anchor: pins,
                                period: period.to_string(),
                            },
                        ));
                    }
                } else if name.eq_ignore_ascii_case("ParallelPeriod") {
                    if let (Some(level_member), Some(offset), Some(anchor)) = (
                        args.first().and_then(|a| a.as_member()),
                        args.get(1).and_then(|a| match a {
                            Expr::Number(n) => n.parse::<i64>().ok(),
                            _ => None,
                        }),
                        args.get(2).and_then(|a| a.as_member()),
                    ) && let Some(unit) = level_member.level().and_then(level_period)
                        && let (Some(level), Some(key)) = (anchor.level(), anchor.key.as_deref())
                    {
                        let parts: Vec<String> = key.split('|').map(str::to_string).collect();
                        let dim = anchor.dim().to_string();
                        let pins = align_pins(model, &dim, level, &parts);
                        out.push((
                            dim,
                            level.to_string(),
                            DateWindow::Parallel {
                                anchor: pins,
                                level: unit.to_string(),
                                offset,
                            },
                        ));
                    }
                } else if name.eq_ignore_ascii_case("LastPeriods") {
                    if let (Some(count), Some(anchor)) = (
                        args.first().and_then(|a| match a {
                            Expr::Number(n) => n.parse::<i64>().ok(),
                            _ => None,
                        }),
                        args.get(1).and_then(|a| a.as_member()),
                    ) && let (Some(level), Some(key)) = (anchor.level(), anchor.key.as_deref())
                        && let Some(unit) = level_period(level)
                    {
                        let parts: Vec<String> = key.split('|').map(str::to_string).collect();
                        let dim = anchor.dim().to_string();
                        let pins = align_pins(model, &dim, level, &parts);
                        out.push((
                            dim,
                            level.to_string(),
                            DateWindow::LastPeriods {
                                anchor: pins,
                                level: unit.to_string(),
                                count,
                            },
                        ));
                    }
                } else if name.eq_ignore_ascii_case("Filter") {
                    // `Filter(<level set>, <member-value comparison>)`.
                    if let (Some(set), Some(Expr::Binary { op, lhs, rhs })) =
                        (args.first(), args.get(1))
                        && is_member_value(lhs)
                        && let Some((amount, unit)) = date_shift(rhs)
                        && let Some(m) = set.as_member()
                        && let Some(level) = m.level()
                    {
                        out.push((
                            m.dim().to_string(),
                            level.to_string(),
                            DateWindow::Relative {
                                op: *op,
                                amount,
                                unit,
                            },
                        ));
                    }
                }
                for a in args {
                    walk(a, model, out);
                }
            }
            Expr::Set(items) | Expr::Tuple(items) => {
                for i in items {
                    walk(i, model, out);
                }
            }
            Expr::Range(a, b) => {
                walk(a, model, out);
                walk(b, model, out);
            }
            Expr::Members(inner) | Expr::Children(inner) | Expr::Exclude(inner) => {
                walk(inner, model, out)
            }
            _ => {}
        }
    }

    for axis in &sel.axes {
        for e in &axis.exprs {
            let expanded = crate::mdx::frontend::expand_named_sets(e, &named);
            walk(&expanded, model, &mut out);
        }
    }
    for (_, body) in &named {
        walk(body, model, &mut out);
    }
    for (_, body) in &sel.with_members {
        if let crate::mdx::ast::Expr::Str(s) = body
            && let Ok(e) = crate::mdx::frontend::parse_set_expr(s)
        {
            walk(&e, model, &mut out);
        }
    }
    // Silence an unused-import warning for `CmpOp` in builds without filters.
    let _ = std::marker::PhantomData::<CmpOp>;
    out
}

fn filters_from_parsed(parsed: &ParsedMdx) -> Vec<DimensionFilter> {
    let mut result: Vec<DimensionFilter> = Vec::new();

    let add_leaf =
        |result: &mut Vec<DimensionFilter>, dim_str: String, key: &str, level: Option<&str>| {
            if let Some(df) = result.iter_mut().find(|f| f.dimension == dim_str) {
                if !df.members.contains(&key.to_string()) {
                    df.members.push(key.to_string());
                }
            } else {
                result.push(DimensionFilter {
                    dimension: dim_str,
                    members: vec![key.to_string()],
                    level: level.map(|s| s.to_string()),
                    range: None,
                    date_window: None,
                });
            }
        };

    for m in &parsed.where_members {
        if let MemberRef::Leaf { dim, key, level } = m {
            add_leaf(&mut result, dim_ref_str(dim), key, level.as_deref());
        }
    }

    for m in &parsed.subquery_members {
        if let MemberRef::Leaf { dim, key, level } = m {
            add_leaf(&mut result, dim_ref_str(dim), key, level.as_deref());
        }
    }

    for m in &parsed.select_members {
        if let MemberRef::Leaf { dim, key, level } = m {
            add_leaf(&mut result, dim_ref_str(dim), key, level.as_deref());
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
    for mref in &parsed.select_members {
        if let MemberRef::Leaf { dim, .. } = mref {
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
    MemberOnlyProbe,
    /// Excel CUBESET validation / CUBECOUNT probes. See `set_probe` /
    /// `set_count` on the query for which one fired.
    SetProbe,
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
    /// All measures requested on the SELECT axis, in order. Multiple entries
    /// mean a multi-measure query (batched CUBEVALUE cells).
    pub measures: Vec<String>,
    /// When drilling a multi-level hierarchy, which level index to group by —
    /// one entry per `axis_dimensions` entry (`None` = leaf/physical grain).
    pub drilldown_levels: Vec<Option<usize>>,
    /// True when the axis is an explicit level set
    /// (`[Dim].[Hier].[Level].Members`) rather than a drilldown.
    pub level_drag: bool,
    /// Measure/member names parsed from strtomember() probe (CUBEVALUE metadata query).
    pub metadata_probe_targets: Vec<String>,
    /// Requested properties: e.g. "UniqueName", "caption", "level.UniqueName".
    pub metadata_probe_properties: Vec<String>,
    /// Dimension member UName strings for MemberOnlyProbe (e.g. "[Category].[Category].&[Category A]").
    pub member_only_unames: Vec<String>,
    /// Axis set function (TopCount/Order/Filter) to apply to the row set.
    pub axis_set_op: Option<AxisSetOp>,
    /// Tuples on the SELECT axis (measure + member slicers), for multi-tuple
    /// CUBEVALUE batches. Empty unless the axis is a set of parenthesized tuples.
    pub axis_tuples: Vec<AxisTuple>,
    /// A CUBESET-style set expression on the SELECT axis (`SetProbe`).
    pub set_probe: Option<SetExpr>,
    /// A calculated `COUNT(<set>)` member referenced by the axis (`SetProbe`).
    pub set_count: Option<CalculatedCount>,
    /// Members named by a `DrilldownMember(...)` expansion, per dimension.
    /// Distinguishes the drill filter from real slicers when re-querying the
    /// hierarchy's input set.
    pub drill_members: Vec<(String, Vec<String>)>,
}

impl SemanticQuery {
    /// Level of the primary (first) axis dimension, if any.
    pub fn drilldown_level(&self) -> Option<usize> {
        self.drilldown_levels.first().copied().flatten()
    }
}

// ---- main classification entry point ----

/// All members of a `DrilldownMember(<set>, {members}, ...)` expansion:
/// `(dimension, level, keys)` in set order. Excel sends several members when
/// the user runs "Expand Entire Field" / "Expand to <Level>".
pub(crate) fn extract_drill_members(mdx: &str) -> Option<(String, String, Vec<String>)> {
    let upper = mdx.to_uppercase();
    let pos = upper.find("DRILLDOWNMEMBER(")?;
    let open = pos + "DRILLDOWNMEMBER".len();
    let close = crate::mdx_parser::matching_paren(mdx, open)?;
    let args = crate::mdx_parser::split_top_level_args(&mdx[open + 1..close]);
    let set = args.get(1)?.trim();
    let inner = set
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();
    let mut dim: Option<String> = None;
    let mut level = String::new();
    let mut keys: Vec<String> = Vec::new();
    for item in crate::mdx_parser::split_top_level_args(inner) {
        let toks = crate::mdx_parser::bracket_tokens(&item, 3);
        if toks.len() < 2 {
            continue;
        }
        let Some(key) = parse_amp_key(&item) else {
            continue;
        };
        if dim.is_none() {
            dim = Some(toks[0].clone());
            level = toks.get(2).cloned().unwrap_or_default();
        }
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    let dim = dim?;
    if keys.is_empty() {
        return None;
    }
    Some((dim, level, keys))
}

pub(crate) fn parse_amp_key(s: &str) -> Option<String> {
    let idx = s.find(".&[")?;
    let mut rest = &s[idx + 3..];
    let mut parts = Vec::new();
    loop {
        let end = rest.find(']')?;
        parts.push(rest[..end].to_string());
        rest = &rest[end + 1..];
        if let Some(next) = rest.strip_prefix("&[") {
            rest = next;
        } else {
            break;
        }
    }
    Some(parts.join("|"))
}

fn extract_strtomember_targets(mdx: &str) -> Vec<String> {
    let prefix = "strtomember(\"";
    let mut targets = Vec::new();
    let mut pos = 0;
    while let Some(found) = mdx[pos..].find(prefix) {
        let abs = pos + found + prefix.len();
        if let Some(close) = mdx[abs..].find('"') {
            let target = mdx[abs..abs + close].to_string();
            if !targets.contains(&target) {
                targets.push(target);
            }
            pos = abs + close + 1;
        } else {
            break;
        }
    }
    targets
}

fn extract_strtomember_properties(mdx: &str) -> Vec<String> {
    let mut props = Vec::new();
    for part in mdx.split("MEMBER [Measures].[").skip(1) {
        let prop = if part.contains("UniqueName") && !part.contains(".level.UniqueName") {
            "UniqueName"
        } else if part.contains(".level.UniqueName") {
            "level.UniqueName"
        } else if part.contains("properties(\"caption\")") {
            "caption"
        } else {
            continue;
        };
        if !props.contains(&prop.to_string()) {
            props.push(prop.to_string());
        }
    }
    props
}

pub fn semantic_query_from_mdx(mdx: &str) -> SemanticQuery {
    let parsed = crate::mdx_parser::parse_mdx(mdx);

    if mdx.contains("strtomember(\"") {
        let targets = extract_strtomember_targets(mdx);
        let props = extract_strtomember_properties(mdx);
        return SemanticQuery {
            kind: SemanticQueryKind::MeasureMetadataProbe,
            set_probe: None,
            set_count: None,
            drill_members: vec![],
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
            measures: vec![],
            drilldown_levels: vec![],
            level_drag: false,
            metadata_probe_targets: targets,
            metadata_probe_properties: props,
            member_only_unames: vec![],
            axis_set_op: None,
            axis_tuples: vec![],
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
                } else if parsed.has_cols && !parsed.has_rows && !parsed.has_measures {
                    SemanticQueryKind::MemberOnlyProbe
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
    let mut level_drag = false;

    // A slicer/measure+member-tuple query has no real axis dimension: any
    // dimension referenced in the select tuple must land in the SlicerAxis
    // (as a filter), not be skipped as an axis dimension. (Kept identical to
    // the construction below.)
    let axis_dims: Vec<String> = if matches!(
        kind,
        SemanticQueryKind::SlicerOnly | SemanticQueryKind::SlicerAllAndMeasure
    ) {
        vec![]
    } else {
        parsed
            .axis_dimension_ids
            .iter()
            .filter(|id| project.model.dim_def_opt(id).is_some())
            .cloned()
            .collect()
    };

    // Per-dimension hierarchy level: one entry per axis dimension.
    let mut drilldown_levels: Vec<Option<usize>> = vec![None; axis_dims.len()];

    // Explicit level sets (`[Dim].[Hier].[Level].Members`) — a field-list
    // level drag. Excel sends the level's members directly, so serve exactly
    // that level (compound keys where the level key isn't unique).
    for (dim_name, level_name) in &parsed.axis_level_members {
        if let Some(i) = axis_dims.iter().position(|d| d == dim_name)
            && let Some(level_idx) = project
                .model
                .dim_def_opt(dim_name)
                .and_then(|d| d.levels.iter().position(|l| l.name == *level_name))
        {
            drilldown_levels[i] = Some(level_idx);
            level_drag = true;
        }
    }

    // A member range on an axis (`{a : b}`) lists the level's members between
    // the keys: level-drag rendering plus a range filter (plan 047: the axis
    // dimensions come from the AST, so this is correct for multi-axis clauses).
    for (dim_name, level_name, _, _) in &parsed.axis_member_ranges {
        if let Some(i) = axis_dims.iter().position(|d| d == dim_name)
            && let Some(level_idx) = project
                .model
                .dim_def_opt(dim_name)
                .and_then(|d| d.levels.iter().position(|l| l.name == *level_name))
        {
            drilldown_levels[i] = Some(level_idx);
            level_drag = true;
        }
    }

    // Period-to-date time functions (`YTD(m)`, `QTD(m)`, `MTD(m)`,
    // `PeriodsToDate(level, m)`) lower to a date window on the anchor's date
    // role; the axis lists the anchor's level, restricted by the window.
    let time_windows = date_windows(mdx, &project.model);
    for (dim_name, level_name, _) in &time_windows {
        if let Some(i) = axis_dims.iter().position(|d| d == dim_name)
            && let Some(level_idx) = project
                .model
                .dim_def_opt(dim_name)
                .and_then(|d| d.levels.iter().position(|l| l.name == *level_name))
        {
            drilldown_levels[i] = Some(level_idx);
            level_drag = true;
        }
    }

    // Whole-hierarchy drags (`DrilldownLevel({...All})`) start at the top
    // level; Excel may instead name the level explicitly
    // (`DrilldownLevel({...All}, [Date].[Calendar].[Quarter])` or `, , N`).
    for target in &parsed.drilldown_targets {
        let Some(i) = axis_dims.iter().position(|d| *d == target.dim) else {
            continue;
        };
        let Some(def) = project.model.dim_def_opt(&target.dim) else {
            continue;
        };
        if def.levels.is_empty() {
            continue;
        }
        let level = target
            .level
            .as_ref()
            .and_then(|name| def.levels.iter().position(|l| l.name == *name))
            .or(target.index)
            .unwrap_or(0)
            .min(def.levels.len() - 1);
        if drilldown_levels[i].is_none() {
            drilldown_levels[i] = Some(level);
        }
    }

    let mut extra_filters: Vec<DimensionFilter> = Vec::new();

    // Drilling into specific member(s) shows one level below them. Excel sends
    // several members for "Expand Entire Field" / "Expand to <Level>", and the
    // set may mix levels (a year plus its quarters). The drill target is one
    // level below the *deepest* member: its key path length is that level's
    // index + 1.
    let mut drill_members: Vec<(String, Vec<String>)> = Vec::new();
    if (parsed.has_drilldown || parsed.has_drilldown_member)
        && let Some((dim_name, level_name, keys)) = extract_drill_members(mdx)
        && let Some(dim) = project.model.dim_def_opt(&dim_name)
        && !dim.levels.is_empty()
    {
        let max_parts = keys.iter().map(|k| k.split('|').count()).max().unwrap_or(1);
        let target = max_parts.min(dim.levels.len() - 1);
        match axis_dims.iter().position(|d| *d == dim_name) {
            Some(i) => drilldown_levels[i] = Some(target),
            // No axis dimension captured (unusual shape): keep the level so
            // the single-dimension path still drills.
            None if drilldown_levels.is_empty() => drilldown_levels.push(Some(target)),
            None => {}
        }
        drill_members.push((dim_name.clone(), keys.clone()));
        extra_filters.push(DimensionFilter {
            dimension: dim_name,
            members: keys,
            level: Some(level_name),
            range: None,
            date_window: None,
        });
        // Route to the single-dimension drilldown renderer,
        // not the DrilldownMemberProbe 2-dimension path.
        if kind == SemanticQueryKind::DrilldownMemberProbe {
            kind = SemanticQueryKind::DrilldownCategories;
        }
    }

    let mut filters = filters_from_parsed(&parsed);
    for (dim_name, level_name, from_key, to_key) in &parsed.axis_member_ranges {
        filters.push(DimensionFilter {
            dimension: dim_name.clone(),
            members: vec![],
            level: Some(level_name.clone()),
            range: Some((from_key.clone(), to_key.clone())),
            date_window: None,
        });
    }
    for (dim_name, level_name, from_key, to_key) in &parsed.where_member_ranges {
        filters.push(DimensionFilter {
            dimension: dim_name.clone(),
            members: vec![],
            level: Some(level_name.clone()),
            range: Some((from_key.clone(), to_key.clone())),
            date_window: None,
        });
    }
    for (dim_name, level_name, window) in time_windows {
        filters.push(DimensionFilter {
            dimension: dim_name,
            members: vec![],
            level: Some(level_name),
            range: None,
            date_window: Some(window),
        });
    }
    filters.extend(extra_filters);

    // A drill scoped by a compound member (a slicer or subselect like
    // `[Date].[Calendar].[Quarter].&[2026]&[4]`) starts one level below that
    // member: expanding Q4-2026 shows its months, scoped to that year.
    if parsed.has_drilldown || parsed.has_drilldown_member {
        for f in &filters {
            let Some(level_name) = &f.level else { continue };
            if f.members.len() != 1 {
                continue;
            }
            let Some(i) = axis_dims.iter().position(|d| *d == f.dimension) else {
                continue;
            };
            let Some(def) = project.model.dim_def_opt(&f.dimension) else {
                continue;
            };
            let Some(li) = def.levels.iter().position(|l| l.name == *level_name) else {
                continue;
            };
            if drilldown_levels[i].is_none_or(|cur| li + 1 > cur) {
                drilldown_levels[i] = Some(li + 1);
            }
        }
    }

    // Excel CUBESET / CUBECOUNT probes: a calculated COUNT member referenced
    // by the axis, or a set expression (HEAD/TAIL/bare Members) on the axis.
    let set_count = parsed
        .calculated_counts
        .iter()
        .find(|cc| {
            let clause_end = mdx.to_uppercase().find(" FROM ").unwrap_or(mdx.len());
            mdx[..clause_end]
                .to_uppercase()
                .contains(&format!("[MEASURES].[{}]", cc.member_name.to_uppercase()))
        })
        .cloned();
    let set_probe = if set_count.is_none()
        && parsed.axis_set_expr.is_some()
        && matches!(parsed.calculated_members_pat, CalculatedMembersPat::None)
        && !parsed.has_drilldown
        && !parsed.has_drilldown_member
        && !parsed.has_crossjoin
        && parsed.axis_set_op.is_none()
        && parsed.select_tuples.is_empty()
    {
        parsed.axis_set_expr.clone()
    } else {
        None
    };
    if set_count.is_some() || set_probe.is_some() {
        kind = SemanticQueryKind::SetProbe;
    }

    // A member range restricts the set (and the aggregate) to the level's
    // members between the two keys, in hierarchy order.
    if let Some(se) = &set_probe {
        let mut cursor = se;
        loop {
            match cursor {
                crate::mdx_parser::SetExpr::Head(inner, _)
                | crate::mdx_parser::SetExpr::Tail(inner, _) => cursor = inner,
                crate::mdx_parser::SetExpr::MemberRange { from, to } => {
                    if let (Some((dim, level, from_key)), Some((_, _, to_key))) = (
                        crate::mdx_parser::parse_level_member(from),
                        crate::mdx_parser::parse_level_member(to),
                    ) {
                        filters.push(DimensionFilter {
                            dimension: dim,
                            members: vec![],
                            level: Some(level),
                            range: Some((from_key, to_key)),
                            date_window: None,
                        });
                    }
                    break;
                }
                _ => break,
            }
        }
    }

    let member_only_unames: Vec<String> = if kind == SemanticQueryKind::MemberOnlyProbe {
        parse_member_only_unames(mdx)
    } else {
        vec![]
    };

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
        axis_dimensions: axis_dims,
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
        measures: parsed.selected_measures.clone(),
        drilldown_levels,
        level_drag,
        metadata_probe_targets: vec![],
        metadata_probe_properties: vec![],
        member_only_unames,
        axis_set_op: parsed.axis_set_op.clone(),
        set_probe,
        set_count,
        drill_members,
        axis_tuples: parsed
            .select_tuples
            .iter()
            .map(|tuple| AxisTuple {
                measure: tuple.iter().find_map(|m| match m {
                    MemberRef::Measure(name) => Some(name.clone()),
                    _ => None,
                }),
                filters: filters_from_tuple_members(tuple),
            })
            .collect(),
    }
}

fn parse_member_only_unames(mdx: &str) -> Vec<String> {
    let mut unames = Vec::new();
    if let Some(start) = mdx.find('{') {
        let rest = &mdx[start + 1..];
        if let Some(end) = rest.find('}') {
            unames.push(rest[..end].to_string());
        }
    }
    unames
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubecount_probe_classifies_as_set_count() {
        let mdx = "WITH MEMBER [Measures].[XL_SD] AS 'COUNT([Date].[Calendar].[Year].Members)' SELECT {[Measures].[XL_SD]} ON 0 FROM [Sales] CELL PROPERTIES VALUE";
        let q = semantic_query_from_mdx(mdx);
        assert_eq!(q.kind, SemanticQueryKind::SetProbe);
        let cc = q.set_count.expect("set_count set");
        assert_eq!(cc.member_name, "XL_SD");
        assert_eq!(
            cc.set,
            crate::mdx_parser::SetExpr::LevelMembers {
                dim: "Date".into(),
                level: Some("Year".into())
            }
        );
    }

    #[test]
    fn cubeset_probe_classifies_as_set_member_probe() {
        let mdx = "SELECT {HEAD([Date].[Calendar].[Year].Members,1)} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL";
        let q = semantic_query_from_mdx(mdx);
        assert_eq!(q.kind, SemanticQueryKind::SetProbe);
        assert!(q.set_count.is_none());
        assert!(matches!(
            q.set_probe,
            Some(crate::mdx_parser::SetExpr::Head(_, 1))
        ));
    }

    #[test]
    fn regular_queries_do_not_classify_as_set_probe() {
        // DrilldownLevel pivot render
        let q = semantic_query_from_mdx(
            "SELECT NON EMPTY Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}) ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue])",
        );
        assert_ne!(q.kind, SemanticQueryKind::SetProbe);
        assert!(q.set_probe.is_none() && q.set_count.is_none());
        // cchildren query (WITH MEMBER with non-COUNT body)
        let q = semantic_query_from_mdx(
            "WITH MEMBER [Measures].cChildren As 'AddCalculatedMembers([Channel].[Channel].currentmember.children).count' Set FilteredMembers As '{[Channel].[Channel].&[Wholesale]}' Select {[Measures].cChildren} on ROWS, Hierarchize(Generate(FilteredMembers, Ascendants([Channel].[Channel].currentmember))) DIMENSION PROPERTIES PARENT_UNIQUE_NAME, MEMBER_TYPE ON COLUMNS FROM [Sales]",
        );
        assert_ne!(q.kind, SemanticQueryKind::SetProbe);
    }

    #[test]
    fn level_members_set_records_explicit_level() {
        // Dragging the Quarter level: the level qualifier must survive and be
        // marked as a level set (not a drilldown).
        let p = crate::proxy_project::ProxyProject::load("projects/project3/proxy-config.json")
            .expect("load project3");
        crate::project::project::with_test_project(p, || {
            let mdx = "SELECT {[Measures].[Revenue]} ON COLUMNS, [Date].[Calendar].[Quarter].Members ON ROWS FROM [Sales]";
            let q = semantic_query_from_mdx(mdx);
            assert_eq!(q.axis_dimensions, vec!["Date"]);
            assert_eq!(q.drilldown_levels, vec![Some(1)], "Quarter is level 1");
            assert!(q.level_drag);
        });
    }

    #[test]
    fn crossjoin_records_top_level_per_hierarchy_regardless_of_order() {
        // The measure is listed first; the leveled dimension second. Both
        // hierarchy drags must get level 0 (not the leaf grain).
        let p = crate::proxy_project::ProxyProject::load("projects/project3/proxy-config.json")
            .expect("load project3");
        crate::project::project::with_test_project(p, || {
            let mdx = "SELECT {[Measures].[Revenue]} ON COLUMNS, CrossJoin(Hierarchize({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)}), Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)})) ON ROWS FROM [Sales]";
            let q = semantic_query_from_mdx(mdx);
            assert_eq!(q.axis_dimensions, vec!["Category", "Date"]);
            assert_eq!(q.drilldown_levels, vec![None, Some(0)]);
            assert!(!q.level_drag);
        });
    }

    #[test]
    fn bare_hierarchy_members_stay_at_leaf_grain() {
        // `[Date].[Full Date].Members` is the leaf level (individual dates), not the
        // top level — only an explicit level or DrilldownLevel promotes it.
        let p = crate::proxy_project::ProxyProject::load("projects/project3/proxy-config.json")
            .expect("load project3");
        crate::project::project::with_test_project(p, || {
            let mdx = "SELECT {[Measures].[Revenue]} ON COLUMNS, [Date].[Full Date].Members ON ROWS FROM [Sales]";
            let q = semantic_query_from_mdx(mdx);
            assert_eq!(q.axis_dimensions, vec!["Date"]);
            assert_eq!(q.drilldown_levels, vec![None]);
            assert!(!q.level_drag);
        });
    }

    #[test]
    fn extract_drillmembers_single_year() {
        let mdx = r##"SELECT NON EMPTY Hierarchize(DrilldownMember({{DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}}, {[Date].[Calendar].[Year].&[2024]},,,INCLUDE_CALC_MEMBERS)) ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE"##;
        let r = extract_drill_members(mdx);
        assert_eq!(r, Some(("Date".into(), "Year".into(), vec!["2024".into()])));
    }

    #[test]
    fn extract_drillmembers_multiple_years() {
        // "Expand Entire Field" sends every parent member in one set.
        let mdx = r##"SELECT NON EMPTY Hierarchize(DrilldownMember({{DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}}, {[Date].[Calendar].[Year].&[2026],[Date].[Calendar].[Year].&[2027]},,,INCLUDE_CALC_MEMBERS)) ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE"##;
        let r = extract_drill_members(mdx);
        assert_eq!(
            r,
            Some((
                "Date".into(),
                "Year".into(),
                vec!["2026".into(), "2027".into()]
            ))
        );
    }

    #[test]
    fn extract_drillmembers_none_without_member_set() {
        let mdx = "DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)";
        assert_eq!(extract_drill_members(mdx), None);
    }

    #[test]
    fn amp_key_normal() {
        assert_eq!(
            parse_amp_key("[Date].[Calendar].[Year].&[2024]"),
            Some("2024".into())
        );
    }

    #[test]
    fn amp_key_no_amp() {
        assert_eq!(parse_amp_key("[Date].[Calendar].[Year]"), None);
    }

    #[test]
    fn is_measure_metadata_probe_detects_strtomember() {
        assert!(is_measure_metadata_probe(
            r##"WITH MEMBER [Measures].[XL_SD0] AS 'strtomember("[Measures].[Total Sales]").UniqueName'"##
        ));
    }

    #[test]
    fn extract_strtomember_target_parses_brackets() {
        assert_eq!(
            extract_strtomember_targets(
                r##"WITH MEMBER [Measures].[XL_SD0] AS 'strtomember("[Measures].[Total Sales]").UniqueName'"##
            ),
            vec!["[Measures].[Total Sales]"]
        );
    }

    #[test]
    fn extract_strtomember_target_parses_dim_member() {
        assert_eq!(
            extract_strtomember_targets(
                r##"WITH MEMBER [Measures].[XL_SD0] AS 'strtomember("[Category].[Category].&[Category A]").UniqueName'"##
            ),
            vec!["[Category].[Category].&[Category A]"]
        );
    }

    #[test]
    fn extract_strtomember_targets_parses_multiple() {
        let mdx = r##"WITH MEMBER [Measures].[XL_SD0] AS 'strtomember("[Category].[Category].&[A]").UniqueName' MEMBER [Measures].[XL_SD3] AS 'strtomember("[Measures].[Revenue]").UniqueName' SELECT"##;
        assert_eq!(
            extract_strtomember_targets(mdx),
            vec!["[Category].[Category].&[A]", "[Measures].[Revenue]"]
        );
    }

    #[test]
    fn extract_strtomember_properties_parses_all_three() {
        let mdx = r##"WITH MEMBER [Measures].[XL_SD0] AS 'strtomember("[Measures].[Total Sales]").UniqueName' MEMBER [Measures].[XL_SD1] AS 'strtomember("[Measures].[Total Sales]").properties("caption")' MEMBER [Measures].[XL_SD2] AS '{strtomember("[Measures].[Total Sales]")}.item(0).item(0).level.UniqueName' SELECT {[Measures].[XL_SD0],[Measures].[XL_SD1],[Measures].[XL_SD2]} ON 0 FROM"##;
        let props = extract_strtomember_properties(mdx);
        assert_eq!(props, vec!["UniqueName", "caption", "level.UniqueName"]);
    }

    #[test]
    fn semantic_query_classifies_cubevalue_metadata_probe() {
        let mdx = r##"WITH MEMBER [Measures].[XL_SD0] AS 'strtomember("[Measures].[Total Sales]").UniqueName' MEMBER [Measures].[XL_SD1] AS 'strtomember("[Measures].[Total Sales]").properties("caption")' MEMBER [Measures].[XL_SD2] AS '{strtomember("[Measures].[Total Sales]")}.item(0).item(0).level.UniqueName' SELECT {[Measures].[XL_SD0],[Measures].[XL_SD1],[Measures].[XL_SD2]} ON 0 FROM  CELL PROPERTIES VALUE"##;
        let q = semantic_query_from_mdx(mdx);
        assert_eq!(q.kind, SemanticQueryKind::MeasureMetadataProbe);
        assert_eq!(
            q.metadata_probe_targets,
            vec!["[Measures].[Total Sales]"]
        );
        assert_eq!(q.metadata_probe_properties.len(), 3);
    }
}
