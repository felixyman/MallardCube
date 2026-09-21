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

        // `WITH SET [name] AS <expr>` / `WITH MEMBER [Measures].[x] AS <expr>`
        while self.eat_ident("WITH") {
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
            } else {
                return Err(ParseError::Unsupported(
                    "unsupported `WITH` clause (expected SET or MEMBER)".into(),
                ));
            }
        }

        self.expect_ident("SELECT")?;

        // Axes: `<set expr> [DIMENSION PROPERTIES …] ON (COLUMNS|ROWS|n)`
        loop {
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
            other => {
                return Err(ParseError::Malformed(format!(
                    "expected a cube name after FROM, found {other:?}"
                )));
            }
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

    /// `[name]` → `name` (for `WITH SET` names).
    fn bracket_name(&mut self) -> Result<String, ParseError> {
        match self.bump() {
            Some(Token::Bracket(n)) => Ok(n.clone()),
            other => Err(ParseError::Malformed(format!(
                "expected [name], found {other:?}"
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

    /// Comma-separated bare identifiers (properties).
    fn prop_list(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        loop {
            match self.peek() {
                Some(Token::Ident(p))
                    if !p.eq_ignore_ascii_case("ON")
                        && !p.eq_ignore_ascii_case("FROM")
                        && !p.eq_ignore_ascii_case("WHERE")
                        && !p.eq_ignore_ascii_case("CELL") =>
                {
                    out.push(p.clone());
                    self.pos += 1;
                }
                _ => break,
            }
            if !self.eat(&Token::Comma) {
                break;
            }
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
                // the caller: put the `.` back and stop.
                Some(Token::Ident(_)) => {
                    self.pos -= 1;
                    break;
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
    let expr = sel.axes[0].exprs.first()?;
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
        Expr::Range(a, b) => {
            flatten_members(a, out);
            flatten_members(b, out);
        }
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

/// Members of a `FROM (SELECT …)` subselect.
pub fn subquery_members(sel: &Select) -> Vec<PMemberRef> {
    let mut out = Vec::new();
    if let Some(sub) = &sel.subquery {
        for axis in &sub.axes {
            for e in &axis.exprs {
                flatten_members(e, &mut out);
            }
        }
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
                "FILTER" => match args.get(1) {
                    Some(Expr::Binary { op, rhs, .. }) => Some(AxisSetOp::Filter {
                        op: to_pcmp(*op),
                        value: num(rhs)?,
                    }),
                    _ => None,
                },
                _ => args.iter().find_map(walk),
            },
            Expr::Set(items) | Expr::Tuple(items) => items.iter().find_map(walk),
            Expr::Range(a, b) => walk(a).or_else(|| walk(b)),
            Expr::Members(inner) | Expr::Children(inner) => walk(inner),
            _ => None,
        }
    }
    sel.axes.iter().flat_map(|a| a.exprs.iter()).find_map(walk)
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
            "SELECT NON EMPTY Hierarchize(DrilldownMember({{DrilldownLevel({[Date].[Date].[All]},,,INCLUDE_CALC_MEMBERS)}}, {[Date].[Date].[Year].&[2022]},,,INCLUDE_CALC_MEMBERS)) DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME ON COLUMNS FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE",
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
            "SELECT {HEAD({[Date].[Date].[Year].&[2022] : [Date].[Date].[Year].&[2024]},1)} ON 0 FROM [Sales] CELL PROPERTIES CELL_ORDINAL",
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
            "SELECT {[Measures].[Revenue]} ON 0, {[Date].[Date].[Year].&[2022] : [Date].[Date].[Year].&[2024]} ON 1 FROM [Sales]",
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
            "SELECT [Date].[Date].[Quarter].Members ON ROWS, {[Measures].[Revenue]} ON COLUMNS FROM [Sales]",
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
            "WITH MEMBER [Measures].[XL_SD] AS 'COUNT([Date].[Date].[Year].Members)' SELECT {[Measures].[XL_SD]} ON 0 FROM [Sales] CELL PROPERTIES VALUE",
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
            "SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales] WHERE ([Date].[Date].[Year].&[2024])",
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
    fn unknown_postfix_is_unsupported_not_silent() {
        let err =
            parse_select("SELECT [Date].[Date].[Year].Unknown ON 0 FROM [Sales]").unwrap_err();
        assert!(matches!(err, ParseError::Unsupported(_)), "{err}");
    }
}
