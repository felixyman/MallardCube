//! Recursive-descent parser for the Excel MDX subset (plan 047).
//!
//! `parse_select` turns a statement into an `ast::Select`. It is deliberately
//! permissive about *ordering* (Excel varies whitespace and clause order) and
//! strict about *structure*: unknown constructs become `ParseError`, which the
//! execute path turns into a named fault instead of a silently dropped axis.
//!
//! The derivations at the bottom replace the hand-rolled scanners in
//! `parser.rs` for axis extraction (`axis_dimension_ids`, `axis_level_members`,
//! axis ranges and the set-probe expression).

use super::ast::{Axis, CmpOp, Expr, MemberRef, Select};
use super::lexer::{ParseError, Token, lex};
use super::parser::SetExpr;

/// Parse a bare set expression (a `WITH SET` body, a `Filter` argument).
pub fn parse_set_expr(text: &str) -> Result<Expr, ParseError> {
    let toks = lex(text)?;
    let mut p = Parser {
        toks: &toks,
        pos: 0,
    };
    p.expr()
}

/// Parse an MDX statement into the subset AST.
pub fn parse_select(mdx: &str) -> Result<Select, ParseError> {
    let toks = lex(mdx)?;
    let mut p = Parser {
        toks: &toks,
        pos: 0,
    };
    p.select()
}

struct Parser<'a> {
    toks: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&'a Token> {
        self.toks.get(self.pos)
    }

    fn bump(&mut self) -> Option<&'a Token> {
        let t = self.toks.get(self.pos);
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn at_ident(&self, word: &str) -> bool {
        self.peek().is_some_and(|t| t.is_ident(word))
    }

    fn eat_ident(&mut self, word: &str) -> bool {
        if self.at_ident(word) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect_ident(&mut self, word: &str) -> Result<(), ParseError> {
        if self.eat_ident(word) {
            Ok(())
        } else {
            Err(ParseError::Malformed(format!(
                "expected `{word}`, found {:?}",
                self.peek()
            )))
        }
    }

    fn eat(&mut self, tok: &Token) -> bool {
        if self.peek() == Some(tok) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn select(&mut self) -> Result<Select, ParseError> {
        let mut sel = Select::default();

        // `WITH SET [name] AS <expr>` / `WITH MEMBER [Measures].[x] AS <expr>`.
        // Excel repeats the clause without another `WITH`:
        // `WITH MEMBER a AS '…' MEMBER b AS '…' SELECT …`.
        if self.eat_ident("WITH") {
            loop {
                if self.eat_ident("SET") {
                    let name = self.bracket_name()?;
                    self.expect_ident("AS")?;
                    let body = self.expr()?;
                    sel.with_sets.push((name, body));
                } else if self.eat_ident("MEMBER") {
                    let name = self.member_ref_name()?;
                    self.expect_ident("AS")?;
                    let body = self.expr()?;
                    sel.with_members.push((name, body));
                } else if self.at_ident("SELECT") {
                    break;
                } else {
                    return Err(ParseError::Unsupported(
                        "unsupported `WITH` clause (expected SET or MEMBER)".into(),
                    ));
                }
            }
        }

        self.expect_ident("SELECT")?;

        // Axes: `<set expr> [DIMENSION PROPERTIES …] ON (COLUMNS|ROWS|n)`.
        // An empty select clause (`SELECT  FROM [Model]`) has none.
        loop {
            if self.at_ident("FROM") {
                break;
            }
            let non_empty = self.eat_ident("NON") && {
                self.expect_ident("EMPTY")?;
                true
            };
            let mut exprs = Vec::new();
            exprs.push(self.expr()?);
            while self.eat(&Token::Comma) {
                exprs.push(self.expr()?);
            }
            if self.eat_ident("DIMENSION") {
                self.expect_ident("PROPERTIES")?;
                sel.dim_props.extend(self.prop_list());
            }
            if !self.eat_ident("ON") {
                // No axis clause: a bare set (CUBESET probes) or malformed.
                if exprs.len() == 1 && sel.axes.is_empty() {
                    sel.axes.push(Axis {
                        ordinal: 0,
                        non_empty,
                        exprs,
                    });
                    break;
                }
                return Err(ParseError::Malformed("expected `ON <axis>`".into()));
            }
            let ordinal = if self.eat_ident("COLUMNS") {
                0
            } else if self.eat_ident("ROWS") {
                1
            } else {
                match self.bump() {
                    Some(Token::Number(n)) => n.parse::<u32>().map_err(|_| {
                        ParseError::Malformed(format!("invalid axis ordinal '{n}'"))
                    })?,
                    other => {
                        return Err(ParseError::Malformed(format!(
                            "expected COLUMNS, ROWS or an axis number, found {other:?}"
                        )));
                    }
                }
            };
            sel.axes.push(Axis {
                ordinal,
                non_empty,
                exprs,
            });
            if !self.eat(&Token::Comma) {
                break;
            }
        }

        self.expect_ident("FROM")?;
        match self.peek() {
            Some(Token::Bracket(name)) => {
                sel.cube = Some(name.clone());
                self.pos += 1;
            }
            Some(Token::LParen) => {
                // Subselect: `FROM (SELECT … FROM [cube])`
                self.pos += 1;
                let inner = self.select()?;
                if !self.eat(&Token::RParen) {
                    return Err(ParseError::Malformed(
                        "unterminated subselect after FROM".into(),
                    ));
                }
                sel.cube = inner.cube.clone();
                sel.subquery = Some(Box::new(inner));
            }
            // Some metadata probes omit the cube name entirely
            // (`FROM  CELL PROPERTIES VALUE`); treat it as unspecified.
            _ => {}
        }

        if self.eat_ident("WHERE") {
            sel.where_clause = Some(self.expr()?);
        }
        if self.eat_ident("CELL") {
            self.expect_ident("PROPERTIES")?;
            sel.cell_props.extend(self.prop_list());
        }
        Ok(sel)
    }

    /// A set/member name: `[name]` or a bare identifier
    /// (`Set FilteredMembers As '…'`).
    fn bracket_name(&mut self) -> Result<String, ParseError> {
        match self.bump() {
            Some(Token::Bracket(n)) => Ok(n.clone()),
            Some(Token::Ident(n)) => Ok(n.clone()),
            other => Err(ParseError::Malformed(format!(
                "expected a name, found {other:?}"
            ))),
        }
    }

    /// `[D].[H].[L].&[k]` → its canonical unique name string.
    fn member_ref_name(&mut self) -> Result<String, ParseError> {
        let expr = self.member_ref()?;
        match expr {
            Expr::Member(m) => {
                let mut s = m
                    .parts
                    .iter()
                    .map(|p| format!("[{p}]"))
                    .collect::<Vec<_>>()
                    .join(".");
                if let Some(k) = &m.key {
                    for part in k.split('|') {
                        s.push_str(&format!(".&[{part}]"));
                    }
                }
                Ok(s)
            }
            Expr::Measure(name) => Ok(format!("[Measures].[{name}]")),
            _ => Err(ParseError::Malformed("expected a member reference".into())),
        }
    }

    /// Comma-separated property names. Excel mixes bare (`PARENT_UNIQUE_NAME`)
    /// and bracketed (`[Region].[Region].[Region]MEMBER_CAPTION`) forms; both
    /// are consumed here (the semantic layer keeps its own prop scanner).
    fn prop_list(&mut self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut current = String::new();
        loop {
            match self.peek() {
                Some(Token::Ident(p)) => {
                    let p = p.clone();
                    if p.eq_ignore_ascii_case("ON")
                        || p.eq_ignore_ascii_case("FROM")
                        || p.eq_ignore_ascii_case("WHERE")
                        || p.eq_ignore_ascii_case("CELL")
                    {
                        break;
                    }
                    current.push_str(&p);
                    self.pos += 1;
                }
                Some(Token::Bracket(b)) => {
                    let b = b.clone();
                    current.push_str(&format!("[{b}]"));
                    self.pos += 1;
                }
                Some(Token::Dot) => {
                    current.push('.');
                    self.pos += 1;
                }
                Some(Token::Comma) => {
                    if !current.is_empty() {
                        out.push(std::mem::take(&mut current));
                    }
                    self.pos += 1;
                }
                _ => break,
            }
        }
        if !current.is_empty() {
            out.push(current);
        }
        out
    }

    /// A set/tuple/member expression, including ranges and comparisons.
    fn expr(&mut self) -> Result<Expr, ParseError> {
        let primary = self.primary()?;
        if self.eat(&Token::Colon) {
            let rhs = self.primary()?;
            return Ok(Expr::Range(Box::new(primary), Box::new(rhs)));
        }
        if let Some(op) = self.peek_cmp() {
            self.pos += 1;
            let rhs = self.primary()?;
            return Ok(Expr::Binary {
                op,
                lhs: Box::new(primary),
                rhs: Box::new(rhs),
            });
        }
        Ok(primary)
    }

    fn peek_cmp(&self) -> Option<CmpOp> {
        match self.peek() {
            Some(Token::Gt) => Some(CmpOp::Gt),
            Some(Token::Ge) => Some(CmpOp::Ge),
            Some(Token::Lt) => Some(CmpOp::Lt),
            Some(Token::Le) => Some(CmpOp::Le),
            Some(Token::Eq) => Some(CmpOp::Eq),
            Some(Token::Ne) => Some(CmpOp::Ne),
            _ => None,
        }
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        // `-{ … }` — an excluded set (DrilldownMember collapse).
        if self.eat(&Token::Minus) {
            let inner = self.primary()?;
            return Ok(Expr::Exclude(Box::new(inner)));
        }
        let mut base = match self.peek() {
            Some(Token::LBrace) => {
                self.pos += 1;
                let mut items = Vec::new();
                if !self.eat(&Token::RBrace) {
                    items.push(self.expr()?);
                    while self.eat(&Token::Comma) {
                        items.push(self.expr()?);
                    }
                    if !self.eat(&Token::RBrace) {
                        return Err(ParseError::Malformed("unterminated `{ set }`".into()));
                    }
                }
                Expr::Set(items)
            }
            Some(Token::LParen) => {
                self.pos += 1;
                let mut items = Vec::new();
                if !self.eat(&Token::RParen) {
                    items.push(self.expr()?);
                    while self.eat(&Token::Comma) {
                        items.push(self.expr()?);
                    }
                    if !self.eat(&Token::RParen) {
                        return Err(ParseError::Malformed("unterminated `( tuple )`".into()));
                    }
                }
                Expr::Tuple(items)
            }
            Some(Token::Bracket(_)) => self.member_ref()?,
            Some(Token::Str(s)) => {
                let s = s.clone();
                self.pos += 1;
                Expr::Str(s)
            }
            Some(Token::Number(n)) => {
                let n = n.clone();
                self.pos += 1;
                Expr::Number(n)
            }
            Some(Token::Ident(name)) => {
                let name = name.clone();
                // `VBA![Date]()` lowers to `CURRENT_DATE`; other VBA functions
                // are not supported.
                if name.eq_ignore_ascii_case("VBA")
                    && self.toks.get(self.pos + 1) == Some(&Token::Bang)
                {
                    let is_date = matches!(
                        self.toks.get(self.pos + 2),
                        Some(Token::Bracket(b)) if b.eq_ignore_ascii_case("Date")
                    ) && self.toks.get(self.pos + 3) == Some(&Token::LParen)
                        && self.toks.get(self.pos + 4) == Some(&Token::RParen);
                    if is_date {
                        self.pos += 5;
                        return Ok(Expr::Call {
                            name: "VBA_DATE".into(),
                            args: Vec::new(),
                        });
                    }
                    return Err(ParseError::Unsupported(
                        "unsupported VBA function (only `VBA![Date]()` is lowered)".into(),
                    ));
                }
                self.pos += 1;
                if self.eat(&Token::LParen) {
                    let mut args = Vec::new();
                    loop {
                        if self.eat(&Token::RParen) {
                            break;
                        }
                        // Empty arguments are legal in Excel's calls
                        // (`DrilldownLevel(set, , 1, INCLUDE_CALC_MEMBERS)`).
                        if self.eat(&Token::Comma) {
                            continue;
                        }
                        args.push(self.expr()?);
                    }
                    Expr::Call { name, args }
                } else {
                    // Bare identifier (e.g. INCLUDE_CALC_MEMBERS) — keep as a
                    // string literal so callers can inspect it.
                    Expr::Str(name)
                }
            }
            other => {
                return Err(ParseError::Malformed(format!(
                    "expected a set/member expression, found {other:?}"
                )));
            }
        };

        // Postfix: `.Members` / `.AllMembers` / `.Children` / `.All`
        loop {
            if !self.eat(&Token::Dot) {
                break;
            }
            if self.eat_ident("MEMBERS") || self.eat_ident("ALLMEMBERS") {
                base = Expr::Members(Box::new(base));
            } else if self.eat_ident("CHILDREN") {
                base = Expr::Children(Box::new(base));
            } else if self.eat_ident("ALL") {
                base = Expr::Members(Box::new(base));
            } else {
                let prop = match self.peek() {
                    Some(Token::Ident(p)) => p.to_uppercase(),
                    _ => String::new(),
                };
                if matches!(
                    prop.as_str(),
                    "CURRENTMEMBER"
                        | "MEMBER_VALUE"
                        | "MEMBER_KEY"
                        | "MEMBER_UNIQUE_NAME"
                        | "MEMBER_CAPTION"
                        | "MEMBER_NAME"
                ) {
                    return Err(ParseError::Unsupported(
                        "member-property filters (`Filter` over `Member_Value`/`Member_Key`) are not supported yet"
                            .into(),
                    ));
                }
                return Err(ParseError::Unsupported(
                    "unsupported postfix after `.` (expected Members/Children)".into(),
                ));
            }
        }
        Ok(base)
    }

    fn member_ref(&mut self) -> Result<Expr, ParseError> {
        let mut parts: Vec<String> = Vec::new();
        let mut key_parts: Vec<String> = Vec::new();
        match self.bump() {
            Some(Token::Bracket(p)) => parts.push(p.clone()),
            other => {
                return Err(ParseError::Malformed(format!(
                    "expected [member], found {other:?}"
                )));
            }
        }
        loop {
            if !self.eat(&Token::Dot) {
                break;
            }
            match self.peek() {
                Some(Token::Bracket(_)) => {
                    let p = self.bracket_name()?;
                    parts.push(p);
                }
                Some(Token::Key(_)) => {
                    while let Some(Token::Key(k)) = self.peek() {
                        key_parts.push(k.clone());
                        self.pos += 1;
                    }
                }
                // `.Members` / `.Children` / … are postfix operators handled by
                // the caller: put the `.` back and stop. Any other bare
                // identifier is a member-name part (`[Measures].cChildren`,
                // `[D].[H].currentmember`).
                Some(Token::Ident(name)) => {
                    let n = name.clone();
                    if matches!(
                        n.to_uppercase().as_str(),
                        "MEMBERS" | "ALLMEMBERS" | "CHILDREN" | "ALL"
                    ) {
                        self.pos -= 1;
                        break;
                    }
                    parts.push(n);
                    self.pos += 1;
                }
                other => {
                    return Err(ParseError::Malformed(format!(
                        "expected [part] or &[key] after `.`, found {other:?}"
                    )));
                }
            }
        }
        let key = (!key_parts.is_empty()).then(|| key_parts.join("|"));
        // `[Measures].[X]` is a measure reference, not a dimension member.
        if parts
            .first()
            .is_some_and(|p| p.eq_ignore_ascii_case("Measures"))
            && let Some(name) = parts.get(1)
        {
            return Ok(Expr::Measure(name.clone()));
        }
        Ok(Expr::Member(MemberRef { parts, key }))
    }
}

// ---------------------------------------------------------------------------
// Derivations used by `ParsedMdx` (replacing the hand scanners)
// ---------------------------------------------------------------------------

/// Dimensions referenced on the axes, in clause order, deduplicated.
pub fn axis_dimension_ids(sel: &Select) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for axis in &sel.axes {
        for expr in &axis.exprs {
            collect_dims(expr, &mut out);
        }
    }
    out
}

/// One requested axis: the dimensions in order plus the measures cross-joined
/// on it. Excel cross-joins the Values area with the field on the same edge
/// (`CrossJoin(<hierarchy>, {[Measures].[Revenue],[Measures].[Units]})`), so the
/// response has to keep them on that axis — splitting them onto separate axes
/// breaks every layout with a field in Columns or measures in Rows (plan 049).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AxisSpec {
    /// `ON COLUMNS` = 0, `ON ROWS` = 1.
    pub ordinal: u32,
    /// Dimension ids on this axis, in member order.
    pub dims: Vec<String>,
    /// Measures on this axis, in member order.
    pub measures: Vec<String>,
    /// The same members in the order they appear in the axis tuples, so a
    /// cross-joined measure lands on the side the statement put it.
    pub slots: Vec<AxisSlot>,
}

/// A member slot of an axis: a dimension or the measures set.
#[derive(Debug, Clone, PartialEq)]
pub enum AxisSlot {
    Dim(String),
    Measure(String),
}

impl AxisSpec {
    /// Does this axis carry both a dimension and the measures (a cross-join)?
    pub fn has_both(&self) -> bool {
        !self.dims.is_empty() && !self.measures.is_empty()
    }

    /// Are the measures the first member of the axis tuples?
    pub fn measures_first(&self) -> bool {
        matches!(self.slots.first(), Some(AxisSlot::Measure(_)))
    }
}

/// The requested axes, in ordinal order, with their dimensions and measures.
pub fn axis_specs(sel: &Select) -> Vec<AxisSpec> {
    let mut axes: Vec<&Axis> = sel.axes.iter().collect();
    axes.sort_by_key(|a| a.ordinal);
    let mut out: Vec<AxisSpec> = Vec::new();
    for axis in axes {
        let mut spec = AxisSpec {
            ordinal: axis.ordinal,
            ..AxisSpec::default()
        };
        for expr in &axis.exprs {
            collect_slots(expr, &mut spec);
        }
        out.push(spec);
    }
    out
}

/// Collect an axis expression's dimension and measure members in tuple order.
fn collect_slots(expr: &Expr, spec: &mut AxisSpec) {
    match expr {
        Expr::Measure(name) => {
            if !spec.measures.iter().any(|m| m == name) {
                spec.measures.push(name.clone());
                spec.slots.push(AxisSlot::Measure(name.clone()));
            }
        }
        Expr::Member(m) => {
            let dim = m.dim();
            if dim.eq_ignore_ascii_case("Measures") {
                // `[Measures].[X]` written as a plain member reference.
                if let Some(name) = m.parts.get(1)
                    && !spec.measures.iter().any(|n| n == name)
                {
                    spec.measures.push(name.clone());
                    spec.slots.push(AxisSlot::Measure(name.clone()));
                }
            } else if !dim.is_empty() && !spec.dims.iter().any(|d| d == dim) {
                spec.dims.push(dim.to_string());
                spec.slots.push(AxisSlot::Dim(dim.to_string()));
            }
        }
        Expr::Range(a, b) => {
            collect_slots(a, spec);
            collect_slots(b, spec);
        }
        Expr::Set(items) | Expr::Tuple(items) => {
            for item in items {
                collect_slots(item, spec);
            }
        }
        Expr::Call { args, .. } => {
            for arg in args {
                collect_slots(arg, spec);
            }
        }
        Expr::Members(inner) | Expr::Children(inner) | Expr::Exclude(inner) => {
            collect_slots(inner, spec)
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_slots(lhs, spec);
            collect_slots(rhs, spec);
        }
        _ => {}
    }
}

fn collect_dims(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Member(m) => {
            let dim = m.dim();
            if !dim.is_empty()
                && !dim.eq_ignore_ascii_case("Measures")
                && !out.iter().any(|d| d == dim)
            {
                out.push(dim.to_string());
            }
        }
        Expr::Range(a, b) => {
            collect_dims(a, out);
            collect_dims(b, out);
        }
        Expr::Set(items) | Expr::Tuple(items) => {
            for item in items {
                collect_dims(item, out);
            }
        }
        Expr::Call { args, .. } => {
            for arg in args {
                collect_dims(arg, out);
            }
        }
        Expr::Members(inner) | Expr::Children(inner) => collect_dims(inner, out),
        _ => {}
    }
}

// ---- AST traversal helpers (plan 049, phase 2) -------------------------
//
// The classification flags and the semantic scanners used to read the MDX
// text. Every question they ask ("is there a CrossJoin?", "which dimension is
// on the axis?", "what is the DrilldownMember target set?") is answerable from
// the AST, which already handled the same syntax for the extractors above.
// These helpers replace the text scans.

/// Visit an expression and everything under it, depth-first.
pub fn walk_expr<'a>(e: &'a Expr, f: &mut impl FnMut(&'a Expr)) {
    f(e);
    match e {
        Expr::Range(a, b) => {
            walk_expr(a, f);
            walk_expr(b, f);
        }
        Expr::Tuple(items) | Expr::Set(items) => {
            for item in items {
                walk_expr(item, f);
            }
        }
        Expr::Call { args, .. } => {
            for arg in args {
                walk_expr(arg, f);
            }
        }
        Expr::Members(inner) | Expr::Children(inner) | Expr::Exclude(inner) => walk_expr(inner, f),
        Expr::Binary { lhs, rhs, .. } => {
            walk_expr(lhs, f);
            walk_expr(rhs, f);
        }
        _ => {}
    }
}

/// Visit every expression in the statement: the axes in ordinal order, the
/// slicer, a subselect (recursively) and the `WITH` bodies.
pub fn for_each_expr<'a>(sel: &'a Select, f: &mut impl FnMut(&'a Expr)) {
    let walk = |e: &'a Expr, f: &mut dyn FnMut(&'a Expr)| walk_expr(e, &mut |x| f(x));
    let mut axes: Vec<&Axis> = sel.axes.iter().collect();
    axes.sort_by_key(|a| a.ordinal);
    for axis in axes {
        for e in &axis.exprs {
            walk(e, f);
        }
    }
    if let Some(w) = &sel.where_clause {
        walk(w, f);
    }
    if let Some(sub) = &sel.subquery {
        for_each_expr(sub, f);
    }
    for (_, body) in sel.with_sets.iter().chain(sel.with_members.iter()) {
        walk(body, f);
    }
}

/// Does the statement mention a call to `name` (case-insensitive)?
pub fn mentions_call(sel: &Select, name: &str) -> bool {
    let mut found = false;
    for_each_expr(sel, &mut |e| {
        if let Expr::Call { name: n, .. } = e
            && n.eq_ignore_ascii_case(name)
        {
            found = true;
        }
    });
    found
}

/// The first call to `name` (case-insensitive) in statement order.
pub fn find_call<'a>(sel: &'a Select, name: &str) -> Option<&'a Expr> {
    let mut found: Option<&Expr> = None;
    for_each_expr(sel, &mut |e| {
        if found.is_none()
            && let Expr::Call { name: n, .. } = e
            && n.eq_ignore_ascii_case(name)
        {
            found = Some(e);
        }
    });
    found
}

/// Any `<expr>.Members` / `.AllMembers`?
pub fn mentions_members(sel: &Select) -> bool {
    let mut found = false;
    for_each_expr(sel, &mut |e| {
        if matches!(e, Expr::Members(_)) {
            found = true;
        }
    });
    found
}

/// Any `<expr>.Children`?
pub fn mentions_children(sel: &Select) -> bool {
    let mut found = false;
    for_each_expr(sel, &mut |e| {
        if matches!(e, Expr::Children(_)) {
            found = true;
        }
    });
    found
}

/// The first cube dimension on the axes (skipping `[Measures]`), in axis order.
pub fn first_axis_dimension(sel: &Select) -> Option<String> {
    let mut axes: Vec<&Axis> = sel.axes.iter().collect();
    axes.sort_by_key(|a| a.ordinal);
    for axis in axes {
        for e in &axis.exprs {
            let mut dim: Option<String> = None;
            walk_expr(e, &mut |x| {
                if dim.is_none()
                    && let Expr::Member(m) = x
                    && !m.dim().is_empty()
                    && !m.dim().eq_ignore_ascii_case("Measures")
                {
                    dim = Some(m.dim().to_string());
                }
            });
            if dim.is_some() {
                return dim;
            }
        }
    }
    None
}

/// The `WITH` body whose declared name matches `needle` (case-insensitive
/// substring), if any.
pub fn with_body<'a>(sel: &'a Select, needle: &str) -> Option<&'a Expr> {
    let needle = needle.to_lowercase();
    sel.with_members
        .iter()
        .chain(sel.with_sets.iter())
        .find(|(name, _)| name.to_lowercase().contains(&needle))
        .map(|(_, body)| body)
}

/// The body of a `WITH` member/set as text: quoted bodies are string literals
/// in the AST (`AS 'COUNT(…)'`), and only the inner text can say what they do.
pub fn with_body_text<'a>(sel: &'a Select, needle: &str) -> Option<&'a str> {
    match with_body(sel, needle)? {
        Expr::Str(text) => Some(text.as_str()),
        _ => None,
    }
}

/// Does a member reference name this dimension (case-insensitive)?
pub fn member_is_dim(e: &Expr, dim: &str) -> bool {
    matches!(e, Expr::Member(m) if m.dim().eq_ignore_ascii_case(dim))
}

/// Level sets on the axes: `<member with a level>.Members` → `(dim, level)`.
pub fn axis_level_members(sel: &Select) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for axis in &sel.axes {
        for expr in &axis.exprs {
            collect_level_members(expr, &mut out);
        }
    }
    out
}

fn collect_level_members(expr: &Expr, out: &mut Vec<(String, String)>) {
    match expr {
        Expr::Members(inner) => {
            if let Some(m) = inner.as_member()
                && let Some(level) = m.level()
            {
                out.push((m.dim().to_string(), level.to_string()));
            }
            collect_level_members(inner, out);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                collect_level_members(arg, out);
            }
        }
        Expr::Set(items) | Expr::Tuple(items) => {
            for item in items {
                collect_level_members(item, out);
            }
        }
        Expr::Range(a, b) => {
            collect_level_members(a, out);
            collect_level_members(b, out);
        }
        Expr::Children(inner) => collect_level_members(inner, out),
        _ => {}
    }
}

/// Member ranges on the axes: `{a : b}` → `(dim, level, from_key, to_key)`.
pub fn axis_member_ranges(sel: &Select) -> Vec<(String, String, String, String)> {
    let mut out = Vec::new();
    for axis in &sel.axes {
        for expr in &axis.exprs {
            collect_ranges(expr, &mut out);
        }
    }
    out
}

/// Member ranges in the slicer (`WHERE ({a : b})`).
pub fn where_member_ranges(sel: &Select) -> Vec<(String, String, String, String)> {
    let mut out = Vec::new();
    if let Some(w) = &sel.where_clause {
        collect_ranges(w, &mut out);
    }
    out
}

fn collect_ranges(expr: &Expr, out: &mut Vec<(String, String, String, String)>) {
    match expr {
        Expr::Range(a, b) => {
            if let (Some(from), Some(to)) = (a.as_member(), b.as_member())
                && let (Some(level), Some(fk), Some(tk)) =
                    (from.level(), from.key.as_deref(), to.key.as_deref())
            {
                out.push((
                    from.dim().to_string(),
                    level.to_string(),
                    fk.to_string(),
                    tk.to_string(),
                ));
            }
        }
        Expr::Call { args, .. } => {
            for arg in args {
                collect_ranges(arg, out);
            }
        }
        Expr::Set(items) | Expr::Tuple(items) => {
            for item in items {
                collect_ranges(item, out);
            }
        }
        Expr::Members(inner) | Expr::Children(inner) => collect_ranges(inner, out),
        _ => {}
    }
}

/// A set expression that stands alone on an axis (the CUBESET probe shapes),
/// translated into the existing `SetExpr` for the planner.
pub fn set_probe_expr(sel: &Select) -> Option<SetExpr> {
    if sel.axes.len() != 1 {
        return None;
    }
    let sets = named_sets(sel);
    let expanded = expand_named_sets(sel.axes[0].exprs.first()?, &sets);
    let expr = &expanded;
    // A measure set on the axis means the query is a normal pivot query, not a
    // set probe.
    if axis_has_measure(sel) {
        return None;
    }
    set_expr_from_ast(expr)
}

fn axis_has_measure(sel: &Select) -> bool {
    fn has_measure(expr: &Expr) -> bool {
        match expr {
            Expr::Measure(_) => true,
            Expr::Set(items) | Expr::Tuple(items) => items.iter().any(has_measure),
            Expr::Call { args, .. } => args.iter().any(has_measure),
            Expr::Members(inner) | Expr::Children(inner) => has_measure(inner),
            Expr::Range(a, b) => has_measure(a) || has_measure(b),
            _ => false,
        }
    }
    sel.axes.iter().any(|a| a.exprs.iter().any(has_measure))
}

/// Translate an AST expression into the planner's `SetExpr` where possible.
pub fn set_expr_from_ast(expr: &Expr) -> Option<SetExpr> {
    match expr {
        Expr::Members(inner) => {
            let m = inner.as_member()?;
            Some(match m.level() {
                Some(level) => SetExpr::LevelMembers {
                    dim: m.dim().to_string(),
                    level: Some(level.to_string()),
                },
                None => SetExpr::LevelMembers {
                    dim: m.dim().to_string(),
                    level: None,
                },
            })
        }
        Expr::Range(a, b) => {
            let (from, to) = (a.as_member()?, b.as_member()?);
            Some(SetExpr::MemberRange {
                from: uname(from),
                to: uname(to),
            })
        }
        Expr::Call { name, args } => {
            let upper = name.to_uppercase();
            if upper == "HEAD" || upper == "TAIL" {
                let src = set_expr_from_ast(args.first()?)?;
                let n: usize = match args.get(1)? {
                    Expr::Number(n) => n.parse().ok()?,
                    _ => return None,
                };
                Some(if upper == "HEAD" {
                    SetExpr::Head(Box::new(src), n)
                } else {
                    SetExpr::Tail(Box::new(src), n)
                })
            } else if upper == "FILTER" {
                // `Filter(<level set>, <predicate>)` lists the level; the
                // predicate becomes a window filter in the semantic layer.
                args.first().and_then(set_expr_from_ast)
            } else if matches!(
                upper.as_str(),
                "YTD" | "QTD" | "MTD" | "PERIODSTODATE" | "PARALLELPERIOD" | "LASTPERIODS"
            ) {
                // Time-intelligence sets list the anchor's level; the date
                // window itself comes from the semantic layer's filter.
                let anchor = match upper.as_str() {
                    "PERIODSTODATE" | "LASTPERIODS" => args.get(1),
                    "PARALLELPERIOD" => args.get(2),
                    _ => args.first(),
                }?
                .as_member()?;
                Some(SetExpr::LevelMembers {
                    dim: anchor.dim().to_string(),
                    level: anchor.level().map(str::to_string),
                })
            } else {
                None
            }
        }
        Expr::Set(items) => {
            if items.len() == 1
                && let Some(inner) = set_expr_from_ast(&items[0])
            {
                return Some(inner);
            }
            let unames: Option<Vec<String>> = items.iter().map(member_uname).collect();
            Some(SetExpr::MemberList { unames: unames? })
        }
        _ => None,
    }
}

fn member_uname(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Member(m) => Some(uname(m)),
        Expr::Measure(name) => Some(format!("[Measures].[{name}]")),
        _ => None,
    }
}

fn uname(m: &MemberRef) -> String {
    let mut s = m
        .parts
        .iter()
        .map(|p| format!("[{p}]"))
        .collect::<Vec<_>>()
        .join(".");
    if let Some(k) = &m.key {
        for part in k.split('|') {
            s.push_str(&format!(".&[{part}]"));
        }
    }
    s
}

// ---------------------------------------------------------------------------
// Filter/set-op derivations (increment 2)
// ---------------------------------------------------------------------------

use super::parser::{AxisSetOp, CmpOp as PCmpOp, DimRef, MemberRef as PMemberRef};

fn ast_to_member(e: &Expr) -> Option<PMemberRef> {
    match e {
        Expr::Member(m) => {
            let dim = if m.dim().eq_ignore_ascii_case("Measures") {
                DimRef::Measures
            } else {
                DimRef::Cube(m.dim().to_string())
            };
            match &m.key {
                // `[D].[H].[L].&[k]` — level-qualified key.
                Some(k) => Some(PMemberRef::Leaf {
                    dim,
                    key: k.clone(),
                    level: m.level().map(str::to_string),
                }),
                None => {
                    let last = m.parts.last().map(String::as_str).unwrap_or("");
                    if last.eq_ignore_ascii_case("All") || last.eq_ignore_ascii_case("(All)") {
                        return Some(PMemberRef::All(dim));
                    }
                    // Name-form member references (no `&` key qualifier):
                    // `[D].[H].[Name]` and `[D].[H].[Level].[Name]`. These are
                    // member *names*, not level references — the level-drag
                    // path handles `.Members` separately.
                    match m.parts.len() {
                        3 => Some(PMemberRef::Leaf {
                            dim,
                            key: m.parts[2].clone(),
                            level: None,
                        }),
                        4.. => Some(PMemberRef::Leaf {
                            dim,
                            key: m.parts[3].clone(),
                            level: Some(m.parts[2].clone()),
                        }),
                        _ => None,
                    }
                }
            }
        }
        Expr::Measure(name) => Some(PMemberRef::Measure(name.clone())),
        _ => None,
    }
}

fn flatten_members(e: &Expr, out: &mut Vec<PMemberRef>) {
    match e {
        Expr::Set(items) | Expr::Tuple(items) => {
            for item in items {
                flatten_members(item, out);
            }
        }
        // A range is not a member list: its endpoints must not become plain
        // member filters (that would AND an OR-of-endpoints filter with the
        // range and drop the members between them).
        Expr::Range(..) => {}
        Expr::Exclude(inner) => flatten_members(inner, out),
        // Level sets (`.Members`) are handled by the level-drag path, not as
        // member filters.
        Expr::Members(_) | Expr::Children(_) => {}
        // A comparison predicate is not a member.
        Expr::Binary { .. } => {}
        other => {
            if let Some(m) = ast_to_member(other) {
                out.push(m);
            }
        }
    }
}

/// Slicer members (`WHERE (…)` / `WHERE {…}`).
pub fn where_members(sel: &Select) -> Vec<PMemberRef> {
    let mut out = Vec::new();
    if let Some(w) = &sel.where_clause {
        flatten_members(w, &mut out);
    }
    out
}

/// Members of `FROM (SELECT …)` subselects (nested subselects included).
pub fn subquery_members(sel: &Select) -> Vec<PMemberRef> {
    fn collect(sub: &Select, out: &mut Vec<PMemberRef>) {
        for axis in &sub.axes {
            for e in &axis.exprs {
                flatten_members(e, out);
            }
        }
        if let Some(inner) = &sub.subquery {
            collect(inner, out);
        }
    }
    let mut out = Vec::new();
    if let Some(sub) = &sel.subquery {
        collect(sub, &mut out);
    }
    out
}

/// Members of parenthesized tuples on the outer axes (`SELECT {(a, b), …}`).
pub fn select_members(sel: &Select) -> Vec<PMemberRef> {
    let mut out = Vec::new();
    for tuple in select_tuples(sel) {
        out.extend(tuple);
    }
    out
}

/// Batched CUBEVALUE tuples on the outer axes.
pub fn select_tuples(sel: &Select) -> Vec<Vec<PMemberRef>> {
    let mut out = Vec::new();
    for axis in &sel.axes {
        for e in &axis.exprs {
            if let Expr::Set(items) = e {
                for item in items {
                    if let Expr::Tuple(tuple) = item {
                        let mut members = Vec::new();
                        for part in tuple {
                            flatten_members(part, &mut members);
                        }
                        out.push(members);
                    }
                }
            }
        }
    }
    out
}

/// Members excluded from a DrilldownMember collapse (`-{ … }`).
pub fn excluded_members(sel: &Select) -> Vec<(String, String)> {
    fn walk(e: &Expr, out: &mut Vec<(String, String)>) {
        match e {
            Expr::Exclude(inner) => {
                let mut members = Vec::new();
                flatten_members(inner, &mut members);
                for m in members {
                    if let PMemberRef::Leaf { dim, key, .. } = m {
                        let dim = match dim {
                            DimRef::Cube(d) => d,
                            DimRef::Measures => "Measures".to_string(),
                        };
                        out.push((dim, key));
                    }
                }
            }
            Expr::Call { args, .. } => {
                for a in args {
                    walk(a, out);
                }
            }
            Expr::Set(items) | Expr::Tuple(items) => {
                for i in items {
                    walk(i, out);
                }
            }
            Expr::Range(a, b) => {
                walk(a, out);
                walk(b, out);
            }
            Expr::Members(inner) | Expr::Children(inner) => walk(inner, out),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for axis in &sel.axes {
        for e in &axis.exprs {
            walk(e, &mut out);
        }
    }
    out
}

/// The dimension token following a DrilldownMember exclusion set.
pub fn drilldown_member_hierarchy(sel: &Select) -> Option<String> {
    fn walk(e: &Expr) -> Option<String> {
        match e {
            Expr::Call { name, args } if name.eq_ignore_ascii_case("DrilldownMember") => {
                args.iter().rev().find_map(|a| {
                    let m = a.as_member()?;
                    (m.key.is_none() && !m.parts.is_empty()).then(|| m.dim().to_string())
                })
            }
            Expr::Call { args, .. } => args.iter().find_map(walk),
            Expr::Set(items) | Expr::Tuple(items) => items.iter().find_map(walk),
            Expr::Range(a, b) => walk(a).or_else(|| walk(b)),
            Expr::Members(inner) | Expr::Children(inner) => walk(inner),
            _ => None,
        }
    }
    sel.axes.iter().flat_map(|a| a.exprs.iter()).find_map(walk)
}

/// A TopCount/BottomCount/TopPercent/Order/Filter wrapper around the axis set.
pub fn axis_set_op(sel: &Select) -> Option<AxisSetOp> {
    // Excel wraps its current Top/Bottom N in a subselect:
    // `FROM (SELECT Generate(<set> AS [XL_Filter_Set_0],
    //        TopCount(Filter(Except(DrilldownLevel(<set>.Current …), …),
    //                  Not IsEmpty(<measure>)), n, <measure>)) ON COLUMNS …)`.
    // The outer axis is the plain drilldown, so the limit only shows up there.
    fn num(e: &Expr) -> Option<f64> {
        match e {
            Expr::Number(n) => n.parse().ok(),
            _ => None,
        }
    }
    fn walk(e: &Expr) -> Option<AxisSetOp> {
        match e {
            Expr::Call { name, args } => match name.to_uppercase().as_str() {
                "TOPCOUNT" | "BOTTOMCOUNT" => Some(AxisSetOp::TopCount {
                    n: num(args.get(1)?)? as usize,
                    desc: name.eq_ignore_ascii_case("TopCount"),
                }),
                "TOPPERCENT" | "BOTTOMPERCENT" => Some(AxisSetOp::TopPercent {
                    p: num(args.get(1)?)?,
                }),
                "ORDER" => Some(AxisSetOp::Order {
                    desc: args
                        .iter()
                        .any(|a| matches!(a, Expr::Str(s) if s.eq_ignore_ascii_case("DESC"))),
                }),
                "FILTER" => {
                    // A value filter compares a *measure* against a number;
                    // anything else (caption/name tests, function calls) is a
                    // label filter, which the semantic layer does not lower and
                    // `unsupported_filter_count` faults on.
                    //
                    // Excel wraps the condition in parentheses
                    // (`Filter(set, ([Measures].[Revenue]>26000000))`), which
                    // the parser models as a one-item tuple.
                    let condition = args.iter().find_map(|a| match a {
                        Expr::Tuple(items) => {
                            items.first().filter(|c| matches!(c, Expr::Binary { .. }))
                        }
                        Expr::Binary { .. } => Some(a),
                        _ => None,
                    });
                    match condition {
                        Some(Expr::Binary { lhs, op, rhs }) if matches!(**lhs, Expr::Measure(_)) => {
                            Some(AxisSetOp::Filter {
                                op: to_pcmp(*op),
                                value: num(rhs)?,
                            })
                        }
                        _ => None,
                    }
                }
                _ => args.iter().find_map(walk),
            },
            Expr::Set(items) | Expr::Tuple(items) => items.iter().find_map(walk),
            Expr::Range(a, b) => walk(a).or_else(|| walk(b)),
            Expr::Members(inner) | Expr::Children(inner) => walk(inner),
            _ => None,
        }
    }
    if let Some(op) = sel.axes.iter().flat_map(|a| a.exprs.iter()).find_map(walk) {
        return Some(op);
    }
    // A subselect Top/Bottom N filters the outer axis (`TopCountFilter`): the
    // reference keeps the surviving members in the outer axis's order.
    sel.subquery
        .as_ref()
        .and_then(|sub| sub.axes.iter().flat_map(|a| a.exprs.iter()).find_map(walk))
        .map(|op| match op {
            AxisSetOp::TopCount { n, desc } => AxisSetOp::TopCountFilter { n, desc },
            other => other,
        })
}

/// Does this expression contain an `IsEmpty(...)` call (case-insensitive)?
fn contains_is_empty_call(e: &Expr) -> bool {
    let mut found = false;
    walk_expr(e, &mut |x| {
        if let Expr::Call { name, .. } = x
            && name.eq_ignore_ascii_case("IsEmpty")
        {
            found = true;
        }
    });
    found
}

/// `(has_cols, has_rows)` from the axes' ordinals — structural, so
/// comma-separated axes (`… ON 0, … ON 1`) are not missed by a text scan.
pub fn axis_presence(sel: &Select) -> (bool, bool) {
    (
        sel.axes.iter().any(|a| a.ordinal == 0),
        sel.axes.iter().any(|a| a.ordinal == 1),
    )
}

/// Measures referenced on the axes, axis order (0 first), deduplicated.
pub fn selected_measures(sel: &Select) -> Vec<String> {
    let mut axes: Vec<&Axis> = sel.axes.iter().collect();
    axes.sort_by_key(|a| a.ordinal);
    let mut out: Vec<String> = Vec::new();
    for axis in axes {
        for e in &axis.exprs {
            collect_measures(e, &mut out);
        }
    }
    out
}

fn collect_measures(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Measure(name) => {
            if !out.contains(name) {
                out.push(name.clone());
            }
        }
        Expr::Set(items) | Expr::Tuple(items) => {
            for i in items {
                collect_measures(i, out);
            }
        }
        Expr::Call { args, .. } => {
            for a in args {
                collect_measures(a, out);
            }
        }
        Expr::Range(a, b) => {
            collect_measures(a, out);
            collect_measures(b, out);
        }
        Expr::Members(inner) | Expr::Children(inner) | Expr::Exclude(inner) => {
            collect_measures(inner, out)
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_measures(lhs, out);
            collect_measures(rhs, out);
        }
        _ => {}
    }
}

/// Is a measure referenced anywhere (axes, slicer, subselect) or defined by a
/// `WITH MEMBER [Measures].…` clause?
pub fn mentions_measure(sel: &Select) -> bool {
    fn walk(e: &Expr) -> bool {
        match e {
            Expr::Measure(_) => true,
            Expr::Set(items) | Expr::Tuple(items) => items.iter().any(walk),
            Expr::Call { args, .. } => args.iter().any(walk),
            Expr::Range(a, b) => walk(a) || walk(b),
            Expr::Members(inner) | Expr::Children(inner) | Expr::Exclude(inner) => walk(inner),
            Expr::Binary { lhs, rhs, .. } => walk(lhs) || walk(rhs),
            _ => false,
        }
    }
    sel.with_members
        .iter()
        .any(|(name, _)| name.to_uppercase().starts_with("[MEASURES]"))
        || sel.axes.iter().flat_map(|a| a.exprs.iter()).any(walk)
        || sel.where_clause.as_ref().is_some_and(walk)
        || sel
            .subquery
            .as_ref()
            .is_some_and(|s| s.axes.iter().flat_map(|a| a.exprs.iter()).any(walk))
}

/// The first call to one of `names` anywhere in the statement (axes, WHERE,
/// subselect), case-insensitively. Returns the name as written.
pub fn first_call(sel: &Select, names: &[&str]) -> Option<String> {
    fn walk(e: &Expr, names: &[&str]) -> Option<String> {
        match e {
            Expr::Call { name, args } => {
                if names.iter().any(|n| name.eq_ignore_ascii_case(n)) {
                    return Some(name.clone());
                }
                args.iter().find_map(|a| walk(a, names))
            }
            Expr::Set(items) | Expr::Tuple(items) => items.iter().find_map(|i| walk(i, names)),
            Expr::Range(a, b) => walk(a, names).or_else(|| walk(b, names)),
            Expr::Members(inner) | Expr::Children(inner) | Expr::Exclude(inner) => {
                walk(inner, names)
            }
            Expr::Binary { lhs, rhs, .. } => walk(lhs, names).or_else(|| walk(rhs, names)),
            _ => None,
        }
    }
    let in_axes = sel
        .axes
        .iter()
        .flat_map(|a| a.exprs.iter())
        .find_map(|e| walk(e, names));
    in_axes
        .or_else(|| sel.where_clause.as_ref().and_then(|w| walk(w, names)))
        .or_else(|| {
            sel.subquery.as_ref().and_then(|sub| {
                sub.axes
                    .iter()
                    .flat_map(|a| a.exprs.iter())
                    .find_map(|e| walk(e, names))
            })
        })
}

/// Do any quoted `WITH MEMBER` / `WITH SET` bodies contain `needle`
/// (case-insensitive)? The bodies are opaque MDX text to the AST.
pub fn bodies_contain(sel: &Select, needle: &str) -> bool {
    let needle = needle.to_uppercase();
    sel.with_members
        .iter()
        .chain(sel.with_sets.iter())
        .any(|(_, body)| match body {
            Expr::Str(s) => s.to_uppercase().contains(&needle),
            _ => false,
        })
}

/// Named sets from `WITH SET [name] AS '<set expr>'`, with their bodies parsed.
pub fn named_sets(sel: &Select) -> Vec<(String, Expr)> {
    fn set_like(e: &Expr) -> bool {
        matches!(
            e,
            Expr::Set(_)
                | Expr::Tuple(_)
                | Expr::Member(_)
                | Expr::Measure(_)
                | Expr::Members(_)
                | Expr::Children(_)
                | Expr::Range(..)
                | Expr::Call { .. }
        )
    }
    sel.with_sets
        .iter()
        .filter_map(|(name, body)| match body {
            Expr::Str(s) => parse_set_expr(s)
                .ok()
                .filter(set_like)
                .map(|e| (name.clone(), e)),
            other if set_like(other) => Some((name.clone(), other.clone())),
            _ => None,
        })
        .collect()
}

/// Replace single-part named-set references (`[Last30]`) with their bodies.
pub fn expand_named_sets(expr: &Expr, sets: &[(String, Expr)]) -> Expr {
    match expr {
        Expr::Member(m) if m.parts.len() == 1 => {
            if let Some((_, body)) = sets
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(&m.parts[0]))
            {
                return expand_named_sets(body, sets);
            }
            expr.clone()
        }
        Expr::Set(items) => Expr::Set(items.iter().map(|i| expand_named_sets(i, sets)).collect()),
        Expr::Tuple(items) => {
            Expr::Tuple(items.iter().map(|i| expand_named_sets(i, sets)).collect())
        }
        Expr::Call { name, args } => Expr::Call {
            name: name.clone(),
            args: args.iter().map(|a| expand_named_sets(a, sets)).collect(),
        },
        Expr::Range(a, b) => Expr::Range(
            Box::new(expand_named_sets(a, sets)),
            Box::new(expand_named_sets(b, sets)),
        ),
        Expr::Members(inner) => Expr::Members(Box::new(expand_named_sets(inner, sets))),
        Expr::Children(inner) => Expr::Children(Box::new(expand_named_sets(inner, sets))),
        Expr::Exclude(inner) => Expr::Exclude(Box::new(expand_named_sets(inner, sets))),
        Expr::Binary { op, lhs, rhs } => Expr::Binary {
            op: *op,
            lhs: Box::new(expand_named_sets(lhs, sets)),
            rhs: Box::new(expand_named_sets(rhs, sets)),
        },
        other => other.clone(),
    }
}

/// How many `Filter(...)` calls reference a member property? Compared against
/// `member_value_filters` to fault on predicates we cannot lower.
pub fn member_property_filter_count(sel: &Select) -> usize {
    fn is_member_value(e: &Expr) -> bool {
        e.as_member().is_some_and(|m| {
            m.parts
                .iter()
                .any(|p| p.replace('_', "").eq_ignore_ascii_case("membervalue"))
        })
    }
    fn walk(e: &Expr, out: &mut usize) {
        match e {
            Expr::Call { name, args } => {
                if name.eq_ignore_ascii_case("Filter")
                    && args.get(1).is_some_and(|p| match p {
                        Expr::Binary { lhs, .. } => is_member_value(lhs),
                        _ => false,
                    })
                {
                    *out += 1;
                }
                for a in args {
                    walk(a, out);
                }
            }
            Expr::Set(items) | Expr::Tuple(items) => {
                for i in items {
                    walk(i, out);
                }
            }
            Expr::Range(a, b) => {
                walk(a, out);
                walk(b, out);
            }
            Expr::Members(inner) | Expr::Children(inner) | Expr::Exclude(inner) => walk(inner, out),
            Expr::Binary { lhs, rhs, .. } => {
                walk(lhs, out);
                walk(rhs, out);
            }
            _ => {}
        }
    }
    let sets = named_sets(sel);
    let mut out = 0;
    for axis in &sel.axes {
        for e in &axis.exprs {
            walk(&expand_named_sets(e, &sets), &mut out);
        }
    }
    if let Some(w) = &sel.where_clause {
        walk(&expand_named_sets(w, &sets), &mut out);
    }
    for (_, body) in &sets {
        walk(body, &mut out);
    }
    for (_, body) in &sel.with_members {
        if let Expr::Str(s) = body
            && let Ok(e) = parse_set_expr(s)
        {
            walk(&e, &mut out);
        }
    }
    out
}

/// Excel's Date Filters predicate:
/// `Filter(<hierarchy>.Levels(n).AllMembers, CurrentMember.MemberValue <op>
/// CDate("YYYY-MM-DD"))`, typically inside a `FROM (SELECT …)` subquery.
/// Returns `(dimension, op, ISO date)` for each predicate found.
///
/// The front-end parses `.Levels(n)` as a member whose last part is `Levels`
/// plus a stray argument, so the call is matched structurally: one argument
/// names the hierarchy, another carries the level number, and one is the
/// comparison against a `CDate` literal.
pub fn date_value_filters(sel: &Select) -> Vec<(String, CmpOp, String)> {
    fn iso_date(e: &Expr) -> Option<String> {
        match e {
            Expr::Str(s) => Some(s.clone()),
            Expr::Call { name, args } if name.eq_ignore_ascii_case("CDate") => match args.first() {
                Some(Expr::Str(s)) => Some(s.clone()),
                _ => None,
            },
            _ => None,
        }
    }
    fn is_member_value(e: &Expr) -> bool {
        e.as_member().is_some_and(|m| {
            m.parts.iter().any(|p| {
                let p = p.replace('_', "").to_lowercase();
                p == "membervalue" || p == "memberkey"
            })
        })
    }
    fn walk(e: &Expr, out: &mut Vec<(String, CmpOp, String)>) {
        match e {
            Expr::Call { name, args } if name.eq_ignore_ascii_case("Filter") => {
                let dim = args.iter().find_map(|a| match a {
                    Expr::Member(m) if m.parts.len() >= 2 => Some(m.parts[0].clone()),
                    _ => None,
                });
                let cmp = args
                    .iter()
                    .find_map(|a| match a {
                        // Only comparisons: the set argument and the level
                        // number are not the condition.
                        Expr::Tuple(items) => {
                            items.first().filter(|c| matches!(c, Expr::Binary { .. }))
                        }
                        Expr::Binary { .. } => Some(a),
                        _ => None,
                    })
                    .and_then(|c| match c {
                        Expr::Binary { op, lhs, rhs } if is_member_value(lhs) => {
                            iso_date(rhs).map(|d| (*op, d))
                        }
                        _ => None,
                    });
                if let (Some(dim), Some((op, date))) = (dim, cmp) {
                    out.push((dim, op, date));
                }
                for a in args {
                    walk(a, out);
                }
            }
            Expr::Call { args, .. } => {
                for a in args {
                    walk(a, out);
                }
            }
            Expr::Set(items) | Expr::Tuple(items) => {
                for i in items {
                    walk(i, out);
                }
            }
            Expr::Range(a, b) => {
                walk(a, out);
                walk(b, out);
            }
            Expr::Members(inner) | Expr::Children(inner) | Expr::Exclude(inner) => walk(inner, out),
            Expr::Binary { lhs, rhs, .. } => {
                walk(lhs, out);
                walk(rhs, out);
            }
            _ => {}
        }
    }
    let sets = named_sets(sel);
    let mut out = Vec::new();
    for axis in &sel.axes {
        for e in &axis.exprs {
            walk(&expand_named_sets(e, &sets), &mut out);
        }
    }
    if let Some(sub) = &sel.subquery {
        for axis in &sub.axes {
            for e in &axis.exprs {
                walk(&expand_named_sets(e, &sets), &mut out);
            }
        }
    }
    out
}

/// The dimension a caption reference belongs to
/// (`[Category].[Category].CurrentMember.member_caption`).
pub(crate) fn caption_dim(e: &Expr) -> Option<String> {
    e.as_member().and_then(|m| {
        m.parts
            .iter()
            .any(|p| {
                p.eq_ignore_ascii_case("member_caption") || p.eq_ignore_ascii_case("member_name")
            })
            .then(|| m.parts.first().cloned().unwrap_or_default())
    })
}

/// Reduce Excel's label-filter condition to a `LabelFilter`. The forms Excel
/// sends: a caption comparison (`caption = "x"`), `Left(caption, n) = "x"` for
/// "begins with", `Right(caption, n) = "x"` for "ends with", and
/// `InStr(caption, "x") > 0` / `= 0` for "contains" / "does not contain".
pub(crate) fn label_filter_condition(
    lhs: &Expr,
    op: CmpOp,
    rhs: &Expr,
) -> Option<crate::mdx::ast::LabelFilter> {
    use crate::mdx::ast::LabelFilter;
    fn text(e: &Expr) -> Option<String> {
        match e {
            Expr::Str(s) => Some(s.clone()),
            _ => None,
        }
    }
    if caption_dim(lhs).is_some() {
        let s = text(rhs)?;
        return Some(match op {
            CmpOp::Eq => LabelFilter::Eq(s),
            CmpOp::Ne => LabelFilter::Ne(s),
            CmpOp::Gt => LabelFilter::Gt(s),
            CmpOp::Ge => LabelFilter::Ge(s),
            CmpOp::Lt => LabelFilter::Lt(s),
            CmpOp::Le => LabelFilter::Le(s),
        });
    }
    let Expr::Call { name, args } = lhs else {
        return None;
    };
    match name.to_uppercase().as_str() {
        // `Left(caption, n) = "B"` / `Right(caption, n) = "B"` compare to text.
        "LEFT" | "RIGHT" => {
            let s = text(rhs)?;
            match (name.to_uppercase().as_str(), op) {
                ("LEFT", CmpOp::Eq) => Some(LabelFilter::BeginsWith(s)),
                ("LEFT", CmpOp::Ne) => Some(LabelFilter::DoesNotBeginWith(s)),
                ("RIGHT", CmpOp::Eq) => Some(LabelFilter::EndsWith(s)),
                ("RIGHT", CmpOp::Ne) => Some(LabelFilter::DoesNotEndWith(s)),
                _ => None,
            }
        }
        // `InStr([start,] caption, "B") > 0` — Excel sends the start position
        // (`InStr(1, caption, "oo")`), so take the string argument.
        "INSTR" => {
            let s = args.iter().find_map(|a| match a {
                Expr::Str(s) => Some(s.clone()),
                _ => None,
            })?;
            let n = match rhs {
                Expr::Number(n) => n.as_str(),
                _ => return None,
            };
            match (op, n) {
                (CmpOp::Gt, "0") => Some(LabelFilter::Contains(s)),
                (CmpOp::Eq, "0") => Some(LabelFilter::DoesNotContain(s)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Excel's Label Filters:
/// `Filter(<level>.AllMembers, (<hierarchy>.CurrentMember.member_caption <cmp>
/// "text"))`, or the `Left`/`Right`/`InStr` forms it builds for
/// "begins with" / "ends with" / "contains". Returns `(dimension, filter)`.
pub fn label_filters(sel: &Select) -> Vec<(String, crate::mdx::ast::LabelFilter)> {
    use crate::mdx::ast::LabelFilter;

    fn walk(e: &Expr, out: &mut Vec<(String, LabelFilter)>) {
        match e {
            Expr::Call { name, args } if name.eq_ignore_ascii_case("Filter") => {
                let cond = args.iter().find_map(|a| match a {
                    Expr::Tuple(items) => {
                        items.first().filter(|c| matches!(c, Expr::Binary { .. }))
                    }
                    Expr::Binary { .. } => Some(a),
                    _ => None,
                });
                if let Some(Expr::Binary { op, lhs, rhs }) = cond
                    && let Some(filter) = label_filter_condition(lhs, *op, rhs)
                    // The caption reference sits in the call's arguments
                    // (`InStr(1, caption, "oo")` puts it second), or is the
                    // left-hand side for a direct caption comparison.
                    && let Some(dim) = match lhs.as_ref() {
                        Expr::Call { args, .. } => args.iter().find_map(caption_dim),
                        other => caption_dim(other),
                    }
                {
                    out.push((dim, filter));
                }
                for a in args {
                    walk(a, out);
                }
            }
            Expr::Call { args, .. } => {
                for a in args {
                    walk(a, out);
                }
            }
            Expr::Set(items) | Expr::Tuple(items) => {
                for i in items {
                    walk(i, out);
                }
            }
            Expr::Range(a, b) => {
                walk(a, out);
                walk(b, out);
            }
            Expr::Members(inner) | Expr::Children(inner) | Expr::Exclude(inner) => walk(inner, out),
            Expr::Binary { lhs, rhs, .. } => {
                walk(lhs, out);
                walk(rhs, out);
            }
            _ => {}
        }
    }
    let sets = named_sets(sel);
    let mut out = Vec::new();
    for axis in &sel.axes {
        for e in &axis.exprs {
            walk(&expand_named_sets(e, &sets), &mut out);
        }
    }
    if let Some(sub) = &sel.subquery {
        for axis in &sub.axes {
            for e in &axis.exprs {
                walk(&expand_named_sets(e, &sets), &mut out);
            }
        }
    }
    out
}

/// `Filter(set, <condition>)` calls the proxy does not lower. Label filters
/// (`Filter(set, InStr(<caption>, …) > 0)`, caption/name comparisons) would
/// otherwise be dropped silently — the axis came back unfiltered while Excel
/// showed the filter as applied (plan 048). `Member_Value`/`Member_Key`
/// comparisons lower to date windows; measure-vs-number comparisons lower to
/// SQL value filters.
pub fn unsupported_filter_count(sel: &Select) -> usize {
    fn is_member_property(e: &Expr) -> bool {
        e.as_member().is_some_and(|m| {
            m.parts.iter().any(|p| {
                let p = p.replace('_', "").to_lowercase();
                p == "membervalue" || p == "memberkey"
            })
        })
    }
    fn walk(e: &Expr, out: &mut usize) {
        match e {
            Expr::Call { name, args } => {
                if name.eq_ignore_ascii_case("Filter") {
                    // The condition is usually the second argument, but
                    // Excel's `.Levels(n).AllMembers` set parses into extra
                    // arguments, so find the comparison wherever it landed.
                    let condition = args.iter().find_map(|a| match a {
                        // The condition may be wrapped in a tuple; ignore the
                        // set argument and the level number that Excel's
                        // `.Levels(n).AllMembers` parses into.
                        Expr::Tuple(items) => {
                            items.first().filter(|c| matches!(c, Expr::Binary { .. }))
                        }
                        Expr::Binary { .. } => Some(a),
                        _ => None,
                    });
                    let lowered = match condition {
                        // `IsEmpty(<measure>)` (Excel's Top/Bottom N wraps it
                        // in `Not`): the axes are `NON EMPTY` anyway, so there
                        // is nothing to lower and nothing to fault on.
                        _ if args.iter().any(contains_is_empty_call) => true,
                        // Excel's Label Filters (`Left(caption,1)="B"`, …)
                        // lower to caption predicates on the dimension column.
                        Some(Expr::Binary { op, lhs, rhs })
                            if label_filter_condition(lhs, *op, rhs).is_some() =>
                        {
                            true
                        }
                        // `Member_Value`/`MemberValue` comparisons lower to
                        // date windows: relative (`DateAdd`) or absolute
                        // (`CDate`, Excel's Date Filters).
                        Some(Expr::Binary { lhs, rhs, .. }) if is_member_property(lhs) => {
                            matches!(**rhs, Expr::Number(_) | Expr::Str(_))
                                || matches!(
                                    rhs.as_ref(),
                                    Expr::Call { name, .. }
                                        if name.eq_ignore_ascii_case("DateAdd")
                                            || name.eq_ignore_ascii_case("CDate")
                                            || name == "VBA_DATE"
                                )
                        }
                        Some(Expr::Binary { lhs, rhs, .. }) => {
                            matches!(**lhs, Expr::Measure(_)) && matches!(**rhs, Expr::Number(_))
                        }
                        _ => false,
                    };
                    if !lowered {
                        *out += 1;
                    }
                }
                for a in args {
                    walk(a, out);
                }
            }
            Expr::Set(items) | Expr::Tuple(items) => {
                for i in items {
                    walk(i, out);
                }
            }
            Expr::Range(a, b) => {
                walk(a, out);
                walk(b, out);
            }
            Expr::Members(inner) | Expr::Children(inner) | Expr::Exclude(inner) => walk(inner, out),
            Expr::Binary { lhs, rhs, .. } => {
                walk(lhs, out);
                walk(rhs, out);
            }
            _ => {}
        }
    }
    let sets = named_sets(sel);
    let mut out = 0;
    for axis in &sel.axes {
        for e in &axis.exprs {
            walk(&expand_named_sets(e, &sets), &mut out);
        }
    }
    if let Some(w) = &sel.where_clause {
        walk(&expand_named_sets(w, &sets), &mut out);
    }
    if let Some(sub) = &sel.subquery {
        out += unsupported_filter_count(sub);
    }
    out
}

/// `Filter(<level set>, <member-value comparison against a date expression>)`
/// predicates we can lower: `(set, op, amount, unit)`.
pub fn member_value_filters(sel: &Select) -> Vec<(Expr, CmpOp, i64, String)> {
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
    fn walk(e: &Expr, out: &mut Vec<(Expr, CmpOp, i64, String)>) {
        match e {
            Expr::Call { name, args } if name.eq_ignore_ascii_case("Filter") => {
                if let (Some(set), Some(Expr::Binary { op, lhs, rhs })) =
                    (args.first(), args.get(1))
                    && is_member_value(lhs)
                    && let Some((amount, unit)) = date_shift(rhs)
                {
                    out.push((set.clone(), *op, amount, unit));
                }
                for a in args {
                    walk(a, out);
                }
            }
            Expr::Call { args, .. } => {
                for a in args {
                    walk(a, out);
                }
            }
            Expr::Set(items) | Expr::Tuple(items) => {
                for i in items {
                    walk(i, out);
                }
            }
            Expr::Range(a, b) => {
                walk(a, out);
                walk(b, out);
            }
            Expr::Members(inner) | Expr::Children(inner) | Expr::Exclude(inner) => walk(inner, out),
            Expr::Binary { lhs, rhs, .. } => {
                walk(lhs, out);
                walk(rhs, out);
            }
            _ => {}
        }
    }
    let sets = named_sets(sel);
    let mut out = Vec::new();
    for axis in &sel.axes {
        for e in &axis.exprs {
            walk(&expand_named_sets(e, &sets), &mut out);
        }
    }
    if let Some(w) = &sel.where_clause {
        walk(&expand_named_sets(w, &sets), &mut out);
    }
    for (_, body) in &sets {
        walk(body, &mut out);
    }
    // `WITH MEMBER … AS 'COUNT(<set>)'` bodies.
    for (_, body) in &sel.with_members {
        if let Expr::Str(s) = body
            && let Ok(e) = parse_set_expr(s)
        {
            walk(&e, &mut out);
        }
    }
    out
}

/// Is a member range present in the slicer (`WHERE {a : b}`)?
pub fn has_range_in_slicer(sel: &Select) -> bool {
    fn walk(e: &Expr) -> bool {
        match e {
            Expr::Range(..) => true,
            Expr::Set(items) | Expr::Tuple(items) => items.iter().any(walk),
            Expr::Call { args, .. } => args.iter().any(walk),
            Expr::Members(inner) | Expr::Children(inner) | Expr::Exclude(inner) => walk(inner),
            Expr::Binary { lhs, rhs, .. } => walk(lhs) || walk(rhs),
            _ => false,
        }
    }
    sel.where_clause.as_ref().is_some_and(walk)
}

/// Does a quoted calculated-member body contain a member range
/// (`COUNT({a : b})`)?
pub fn bodies_contain_range(sel: &Select) -> bool {
    sel.with_members.iter().any(|(_, body)| match body {
        Expr::Str(s) => crate::mdx::parser::text_has_member_range(s),
        _ => false,
    })
}

/// Does a `Filter(...)` predicate reference a member property
/// (`[D].[H].CurrentMember.Member_Value`, …)? Those filters are not supported
/// yet. Other `.currentmember` uses (e.g. `Ascendants(...)`) are fine.
pub fn mentions_member_property(sel: &Select) -> bool {
    const PROPS: [&str; 6] = [
        "CURRENTMEMBER",
        "MEMBER_VALUE",
        "MEMBER_KEY",
        "MEMBER_UNIQUE_NAME",
        "MEMBER_CAPTION",
        "MEMBER_NAME",
    ];
    fn walk(e: &Expr) -> bool {
        match e {
            Expr::Member(m) => m
                .parts
                .iter()
                .any(|p| PROPS.iter().any(|prop| p.eq_ignore_ascii_case(prop))),
            Expr::Measure(_) => false,
            Expr::Set(items) | Expr::Tuple(items) => items.iter().any(walk),
            Expr::Call { args, .. } => args.iter().any(walk),
            Expr::Range(a, b) => walk(a) || walk(b),
            Expr::Members(inner) | Expr::Children(inner) | Expr::Exclude(inner) => walk(inner),
            Expr::Binary { lhs, rhs, .. } => walk(lhs) || walk(rhs),
            _ => false,
        }
    }
    fn in_filter(e: &Expr) -> bool {
        match e {
            Expr::Call { name, args } => {
                (name.eq_ignore_ascii_case("Filter") && args.iter().any(walk))
                    || args.iter().any(in_filter)
            }
            Expr::Set(items) | Expr::Tuple(items) => items.iter().any(in_filter),
            Expr::Members(inner) | Expr::Children(inner) | Expr::Exclude(inner) => in_filter(inner),
            Expr::Range(a, b) => in_filter(a) || in_filter(b),
            _ => false,
        }
    }
    let in_axes = sel.axes.iter().flat_map(|a| a.exprs.iter()).any(in_filter);
    in_axes
        || sel.where_clause.as_ref().is_some_and(in_filter)
        || sel
            .subquery
            .as_ref()
            .is_some_and(|s| s.axes.iter().flat_map(|a| a.exprs.iter()).any(in_filter))
}

fn to_pcmp(op: CmpOp) -> PCmpOp {
    match op {
        CmpOp::Gt => PCmpOp::Gt,
        CmpOp::Ge => PCmpOp::Ge,
        CmpOp::Lt => PCmpOp::Lt,
        CmpOp::Le => PCmpOp::Le,
        CmpOp::Eq => PCmpOp::Eq,
        CmpOp::Ne => PCmpOp::Ne,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_drilldown_member_statement() {
        let sel = parse_select(
            "SELECT NON EMPTY Hierarchize(DrilldownMember({{DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}}, {[Date].[Calendar].[Year].&[2022]},,,INCLUDE_CALC_MEMBERS)) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE",
        )
        .expect("parse");
        assert_eq!(sel.cube.as_deref(), Some("Sales"));
        assert_eq!(sel.axes.len(), 1);
        assert_eq!(sel.axes[0].ordinal, 0);
        assert!(sel.axes[0].non_empty);
        assert_eq!(axis_dimension_ids(&sel), vec!["Date".to_string()]);
        assert!(sel.dim_props.contains(&"PARENT_UNIQUE_NAME".to_string()));
        assert!(matches!(sel.where_clause, Some(Expr::Tuple(_))));
    }

    #[test]
    fn parses_a_member_range() {
        let sel = parse_select(
            "SELECT {HEAD({[Date].[Calendar].[Year].&[2022] : [Date].[Calendar].[Year].&[2024]},1)} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
        )
        .expect("parse");
        assert_eq!(
            axis_member_ranges(&sel),
            vec![(
                "Date".to_string(),
                "Year".to_string(),
                "2022".to_string(),
                "2024".to_string()
            )]
        );
        assert!(matches!(set_probe_expr(&sel), Some(SetExpr::Head(_, 1))));
    }

    #[test]
    fn parses_two_axes_without_confusing_the_sets() {
        let sel = parse_select(
            "SELECT {[Measures].[Revenue]} ON 0, {[Date].[Calendar].[Year].&[2022] : [Date].[Calendar].[Year].&[2024]} ON 1 FROM [Sales]",
        )
        .expect("parse");
        assert_eq!(sel.axes.len(), 2);
        assert_eq!(axis_dimension_ids(&sel), vec!["Date".to_string()]);
        assert_eq!(
            axis_member_ranges(&sel),
            vec![(
                "Date".to_string(),
                "Year".to_string(),
                "2022".to_string(),
                "2024".to_string()
            )]
        );
        // A measure on the axis means this is a pivot query, not a set probe.
        assert!(set_probe_expr(&sel).is_none());
    }

    #[test]
    fn parses_level_members_and_calls() {
        let sel = parse_select(
            "SELECT [Date].[Calendar].[Quarter].Members ON ROWS, {[Measures].[Revenue]} ON COLUMNS FROM [Sales]",
        )
        .expect("parse");
        assert_eq!(
            axis_level_members(&sel),
            vec![("Date".to_string(), "Quarter".to_string())]
        );
    }

    #[test]
    fn parses_with_member_and_subselect() {
        let sel = parse_select(
            "WITH MEMBER [Measures].[XL_SD] AS 'COUNT([Date].[Calendar].[Year].Members)' SELECT {[Measures].[XL_SD]} ON 0 FROM [Sales] CELL PROPERTIES VALUE",
        )
        .expect("parse");
        assert_eq!(sel.with_members.len(), 1);
        assert_eq!(sel.with_members[0].0, "[Measures].[XL_SD]");

        let sel = parse_select(
            "SELECT {[Measures].[Revenue]} ON COLUMNS FROM (SELECT ({[Category].[Category].&[Electronics]}) ON COLUMNS FROM [Sales])",
        )
        .expect("parse");
        assert!(sel.subquery.is_some());
    }

    // Trace shapes from real Excel sessions: they must parse and derive the
    // fields the semantic layer consumes.
    #[test]
    fn parses_trace_shapes() {
        // CCHILDREN probe: bare member names, a bare `Set X As` clause, an
        // empty `FROM` is not present here but `Ascendants(…currentmember)` is.
        let sel = parse_select(
            "WITH MEMBER [Measures].cChildren As 'AddCalculatedMembers([ProductCategory].[ProductCategory].currentmember.children).count' Set FilteredMembers As '{[ProductCategory].[ProductCategory].&[Category B]}' Select {[Measures].cChildren} on ROWS, Hierarchize(Generate(FilteredMembers, Ascendants([ProductCategory].[ProductCategory].currentmember))) DIMENSION PROPERTIES PARENT_UNIQUE_NAME, MEMBER_TYPE ON COLUMNS FROM [Model]",
        )
        .expect("CCHILDREN probe parses");
        assert_eq!(sel.with_members.len(), 1);
        assert_eq!(
            axis_dimension_ids(&sel),
            vec!["ProductCategory".to_string()]
        );

        // Empty select clause with a slicer (report-filter probes).
        let sel = parse_select(
            "SELECT  FROM [Model] WHERE ([ProductCategory].[ProductCategory].&[Category A],[Measures].[Total Sales]) CELL PROPERTIES VALUE",
        )
        .expect("empty select parses");
        assert!(sel.axes.is_empty());
        assert_eq!(where_members(&sel).len(), 2);

        // Nested subselects.
        let sel = parse_select(
            "SELECT {[Measures].[Revenue]} ON COLUMNS FROM (SELECT ({[Region].[Region].&[North]}) ON COLUMNS FROM (SELECT ({[ProductCategory].[ProductCategory].&[Category A]}) ON COLUMNS FROM [Model])) WHERE ([Measures].[Total Sales]) CELL PROPERTIES VALUE",
        )
        .expect("nested subselect parses");
        assert_eq!(subquery_members(&sel).len(), 2);

        // Metadata probe: `FROM` without a cube name.
        let sel = parse_select(
            "WITH MEMBER [Measures].[XL_SD0] AS 'strtomember(\"[Measures].[Revenue]\").UniqueName' SELECT {[Measures].[XL_SD0]} ON 0 FROM  CELL PROPERTIES VALUE",
        )
        .expect("empty FROM parses");
        assert!(sel.cube.is_none());
    }

    #[test]
    fn derives_where_members_both_forms() {
        let sel = parse_select(
            "SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales] WHERE ([Category].[Category].[Electronics])",
        )
        .expect("parse");
        let members = where_members(&sel);
        assert!(
            matches!(&members[..], [PMemberRef::Leaf { key, level: None, .. }] if key == "Electronics"),
            "{members:?}"
        );

        let sel = parse_select(
            "SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales] WHERE ([Date].[Calendar].[Year].&[2024])",
        )
        .expect("parse");
        let members = where_members(&sel);
        assert!(
            matches!(&members[..], [PMemberRef::Leaf { key, level: Some(l), .. }] if key == "2024" && l == "Year"),
            "{members:?}"
        );
    }

    #[test]
    fn derives_batched_cubevalue_tuples() {
        let sel = parse_select(
            "SELECT {([Measures].[Revenue],[Category].[Category].&[Electronics]),([Measures].[Revenue],[Category].[Category].&[Books])} ON 0 FROM [Sales]",
        )
        .expect("parse");
        assert_eq!(select_tuples(&sel).len(), 2);
        assert_eq!(select_members(&sel).len(), 4);
    }

    #[test]
    fn derives_collapse_exclusions_and_hierarchy() {
        let sel = parse_select(
            "SELECT NON EMPTY Hierarchize(DrilldownMember(CrossJoin({[ProductCategory].[ProductCategory].[All],[ProductCategory].[ProductCategory].[ProductCategory].AllMembers}, {([Region].[Region].[All])}), {-{[ProductCategory].[ProductCategory].&[Category A]}}, [Region].[Region])) ON COLUMNS FROM [Model]",
        )
        .expect("parse");
        assert_eq!(
            excluded_members(&sel),
            vec![("ProductCategory".to_string(), "Category A".to_string())]
        );
        assert_eq!(drilldown_member_hierarchy(&sel).as_deref(), Some("Region"));
    }

    #[test]
    fn derives_axis_set_ops() {
        let sel = parse_select(
            "SELECT TopCount([Category].[Category].Members, 5, [Measures].[Revenue]) ON 0 FROM [Sales]",
        )
        .expect("parse");
        assert_eq!(
            axis_set_op(&sel),
            Some(AxisSetOp::TopCount { n: 5, desc: true })
        );

        let sel = parse_select(
            "SELECT Order([Category].[Category].Members, [Measures].[Revenue], DESC) ON 0 FROM [Sales]",
        )
        .expect("parse");
        assert_eq!(axis_set_op(&sel), Some(AxisSetOp::Order { desc: true }));

        let sel = parse_select(
            "SELECT Filter([Category].[Category].Members, [Measures].[Revenue] > 100) ON 0 FROM [Sales]",
        )
        .expect("parse");
        assert_eq!(
            axis_set_op(&sel),
            Some(AxisSetOp::Filter {
                op: PCmpOp::Gt,
                value: 100.0
            })
        );
    }

    #[test]
    fn member_property_filter_is_detected_not_silent() {
        let sel = parse_select(
            "SELECT {[Measures].[Revenue]} ON 0 FROM [Sales] WHERE FILTER([Date].[Calendar].[Date].Members, [Date].[Calendar].CurrentMember.Member_Value >= 1)",
        )
        .expect("parse");
        assert!(mentions_member_property(&sel));
        // A bare `.currentmember` elsewhere (Ascendants) is fine.
        let sel = parse_select(
            "SELECT Hierarchize(Generate({[D].[H].&[x]}, Ascendants([D].[H].currentmember))) ON 0 FROM [Sales]",
        )
        .expect("parse");
        assert!(!mentions_member_property(&sel));
    }
}
