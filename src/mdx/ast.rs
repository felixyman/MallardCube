//! AST for the Excel MDX subset (plan 047).
//!
//! Small and explicit on purpose: it models the shapes Excel/MSOLAP sends
//! (axes, sets, tuples, members, ranges, calls) so the semantic layer can stop
//! guessing from a flat bag of flags. Anything outside the subset is a parse
//! error, which the execute path turns into a named fault.

/// A `SELECT … FROM … [WHERE …]` statement.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Select {
    pub axes: Vec<Axis>,
    pub cube: Option<String>,
    /// `FROM (SELECT …)`: a subselect restricts the outer query.
    pub subquery: Option<Box<Select>>,
    pub where_clause: Option<Expr>,
    pub cell_props: Vec<String>,
    pub dim_props: Vec<String>,
    pub with_sets: Vec<(String, Expr)>,
    pub with_members: Vec<(String, Expr)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Axis {
    /// `ON COLUMNS` = 0, `ON ROWS` = 1, `ON <n>` = n.
    pub ordinal: u32,
    pub non_empty: bool,
    pub exprs: Vec<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Member(MemberRef),
    /// `[Measures].[Revenue]`
    Measure(String),
    /// `a : b` — an inclusive member range.
    Range(Box<Expr>, Box<Expr>),
    /// `(a, b)` — a tuple.
    Tuple(Vec<Expr>),
    /// `{a, b}` — a set.
    Set(Vec<Expr>),
    /// `HEAD(set, 3)`, `DrilldownLevel(…)`, `Filter(…)`, …
    Call {
        name: String,
        args: Vec<Expr>,
    },
    /// `<expr>.Members` / `.AllMembers`
    Members(Box<Expr>),
    /// `<expr>.Children`
    Children(Box<Expr>),
    /// `-{ … }` — an excluded set (DrilldownMember collapse).
    Exclude(Box<Expr>),
    /// A comparison predicate (`[Measures].[X] > 100`) inside `Filter`.
    Binary {
        op: CmpOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Number(String),
    Str(String),
}

/// A date window on a date-role dimension's full-date column.
#[derive(Debug, Clone, PartialEq)]
pub enum DateWindow {
    /// Period-to-date (`YTD(m)`): `date_trunc(<period>, anchor) .. anchor`.
    ToDate {
        /// `(level name, value)` equalities identifying the anchor member.
        anchor: Vec<(String, String)>,
        /// `year` | `quarter` | `month` — the period-to-date grain.
        period: String,
    },
    /// Relative window
    /// (`Filter(…, Member_Value >= DateAdd('d', -30, VBA![Date]()))`): the date
    /// column compared to `CURRENT_DATE + INTERVAL '<amount> <unit>'`.
    Relative {
        op: CmpOp,
        amount: i64,
        unit: String,
    },
    /// `ParallelPeriod(level, n, anchor)`: the period at `level` shifted by `n`.
    Parallel {
        anchor: Vec<(String, String)>,
        level: String,
        offset: i64,
    },
    /// `LastPeriods(n, anchor)`: the `count` periods at the anchor's level
    /// ending at the anchor.
    LastPeriods {
        anchor: Vec<(String, String)>,
        level: String,
        count: i64,
    },
}

/// Comparison operators in filter predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Gt,
    Ge,
    Lt,
    Le,
    Eq,
    Ne,
}

/// A bracketed member reference: `[D]`, `[D].[H]`, `[D].[H].[L]`,
/// `[D].[H].[L].&[key]`, compound keys joined with `|`.
#[derive(Debug, Clone, PartialEq)]
pub struct MemberRef {
    pub parts: Vec<String>,
    pub key: Option<String>,
}

impl MemberRef {
    pub fn dim(&self) -> &str {
        self.parts.first().map(String::as_str).unwrap_or("")
    }

    pub fn hierarchy(&self) -> Option<&str> {
        self.parts.get(1).map(String::as_str)
    }

    /// The level part of a level-qualified reference (`[D].[H].[L].&[k]`).
    pub fn level(&self) -> Option<&str> {
        (self.parts.len() >= 3).then(|| self.parts[2].as_str())
    }
}

impl Expr {
    /// The member reference at the root of this expression, if any.
    pub fn as_member(&self) -> Option<&MemberRef> {
        match self {
            Expr::Member(m) => Some(m),
            Expr::Members(inner) | Expr::Children(inner) => inner.as_member(),
            _ => None,
        }
    }

    /// The member reference at the root of an excluded/wrapped expression.
    pub fn root_member(&self) -> Option<&MemberRef> {
        match self {
            Expr::Member(m) => Some(m),
            Expr::Exclude(inner) | Expr::Members(inner) | Expr::Children(inner) => {
                inner.root_member()
            }
            _ => None,
        }
    }

    /// The call name at the root of this expression, if any.
    pub fn as_call(&self) -> Option<&str> {
        match self {
            Expr::Call { name, .. } => Some(name),
            _ => None,
        }
    }
}
