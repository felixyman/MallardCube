# Plan 047: MDX front-end — lexer + AST for the Excel subset

## Status

- **Priority**: P1 (foundation for plans 046 slices 4–5 and every future Excel shape)
- **Effort**: M (a few focused days across three increments)
- **Risk**: LOW–MEDIUM (regression harness = existing 467 tests + captured workload)
- **Depends on**: 046 slices 1–3 (fault infrastructure, ranges)
- **Category**: parser / architecture
- **Status**: **IN PROGRESS 2026-09-21** — increments 1–2 landed (lexer + AST +
  axis/filter extraction); increment 3 (delete the scanners, retire the lexical
  pre-scan, trace corpus) TODO.

## Why

The MDX front-end is a hybrid: `nom` for local fragments plus ~20 hand-rolled
string scanners for structure (`parse_axis_set_expr`, `parse_axis_dimension_ids`,
`parse_member_list`, `outer_select_clause`, `bracket_tokens`, …), and
`ParsedMdx` is a flat bag of 10 `has_*` flags plus ~20 vectors rather than a
tree. There is no parse-error path: unparseable MDX degrades into a dropped
axis or a default query. This session alone produced four silent misparses:

- `{[Measures].[X]} ON 0, {range} ON 1` — `parse_axis_set_expr` spans the first
  `{` to the last `}` and invents a `Measures` dimension range.
- `WITH SET [x] AS …` — answered with a `[Category]` axis.
- `Filter(…, Member_Value >= DateAdd(…))` — treated as a measure-value filter.
- A member range parsed as one bogus uname; `parse_axis_dimension_ids` collects
  every bracketed token (measures, levels, keys) and depends on later filtering,
  which silently emptied `axis_dims` in the two-axis case.

The remaining plan-046 work (named sets, member-value filters, `DateAdd`, MDX
time functions) all needs real set-expression parsing; without it we add a
third and fourth scanner hack for the same concept.

## Goal

A lexer + recursive-descent parser for the **Excel MDX subset** producing a
small typed AST, with `Unsupported(reason)` as a first-class outcome that flows
into the SOAP fault path. Not a full MDX implementation: everything outside the
subset faults with a named reason.

## Increments

| # | Increment | Effort | Status |
|---|---|---|---|
| 1 | **Lexer + AST + axis extraction**: tokenize MDX; parse `SELECT <axes> FROM <cube> [WHERE …]`; sets, tuples, members, ranges, calls, `.Members`/`.Children`; derive `axis_dimension_ids`, `axis_level_members`, axis ranges and the set-probe expression from the AST (replacing those scanners) | M | **DONE** |
| 2 | **Migrate WHERE/slicers, set ops, drilldown exclusions** to the AST | M | **DONE** |
| 3 | **Delete the hand scanners**; `ParsedMdx` becomes a thin view over the AST (or is replaced); add the captured workload/trace corpus as a parser regression suite | S/M | TODO |

## Increment 1 evidence (2026-09-21)

- `lexer.rs` (tokens + malformed-input errors), `ast.rs` (`Select`, `Axis`,
  `Expr`, `MemberRef`), `frontend.rs` (recursive descent + derivations).
- `ParsedMdx` now takes `axis_dimension_ids`, `axis_level_members`,
  `axis_member_ranges` and `axis_set_expr` from the AST when it parses; the
  legacy scanners remain as a documented transitional fallback.
- The captured Excel workload replays **7/7 without faults** (drilldown,
  DrilldownMember, nested DrilldownMember, slicer-only), smoke 8/8.
- Verified live: `HEAD({2022 : 2024}, 1)` → 2022, a bare range → 2022–2024,
  `HEAD([Date].[Date].[Year].Members, 1)` → 2020, the `XL_SD` COUNT probe → 11.
- **Bug found and fixed**: `filter_suffix` ignored the new `range` field, so
  the result cache served a range probe's response for a plain probe (a range
  HEAD returned 2022 for a plain level HEAD). The plan key now fingerprints
  ranges (`2022..2024@Year`) and a regression test covers it.
- Still faulting (increment 2): a braced `{range}` beside a braced measure set
  (that shape is not classified as an axis yet), slicer ranges, and ranges
  inside quoted calculated-member bodies.

## Increment 2 evidence (2026-09-21)

- Lexer/AST extended with comparison operators, a minus token and
  `Expr::{Binary, Exclude}`; the parser accepts `-{ … }` collapse exclusions
  and predicates inside `Filter(...)`.
- `ParsedMdx` now derives `where_members`, `subquery_members`,
  `select_members`, `select_tuples`, `excluded_members`,
  `drilldown_member_hierarchy` and `axis_set_op` from the AST (the legacy
  scanners remain as the fallback for unparseable statements).
- Name-form member references (`[Category].[Category].[Electronics]`) map to a
  leaf key like the old parser, while `.Members` level sets stay on the
  level-drag path (found by the suite: the name-form WHERE filter regressed
  until the conversion handled it).
- Verified: 482 tests green, smoke 8/8, the captured workload replays 7/7 with
  no faults, name-form WHERE → 24,719,896, TopCount/Order/Filter behave.
- Deferred to increment 3: retiring the lexical `unsupported_features` pre-scan
  (it still works and is covered by tests; the AST rewrite lands with the
  scanner deletion and the trace corpus).

## Increment 1 design

- `src/mdx/lexer.rs` — `Token::{Bracket, Key, Ident, Number, Str, Dot, Comma,
  Colon, LParen, RParen, LBrace, RBrace, Bang}`; handles `[ident]`, `&[key]`
  (and XML-escaped `&amp;[key]`), quoted strings, numbers, bare identifiers.
- `src/mdx/ast.rs` — `Select { axes, cube, where_clause, cell_props, dim_props,
  with_sets, with_members }`, `Axis { ordinal, non_empty, exprs }`,
  `Expr::{Member, Measure, Range, Tuple, Set, Call, Members, Children, Number,
  Str}`, `MemberRef { parts, key }`, `ParseError::{Malformed, Unsupported}`.
- `src/mdx/frontend.rs` — `parse_select(&str) -> Result<Select, ParseError>`
  plus derivations used by `ParsedMdx`: `axis_dimension_ids`,
  `axis_level_members`, `axis_member_ranges`, `set_probe_expr`, `has_*` flags.
- `parser.rs` uses the front-end for those fields when it parses; the remaining
  scanners stay until increment 2 (transitional, documented).

## Non-goals

- MDX evaluation, calculated-member semantics, the full function library.
- Accepting every valid MDX statement: the subset is explicit and everything
  else faults loudly.
