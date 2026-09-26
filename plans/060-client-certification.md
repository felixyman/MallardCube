# Plan 060 — Client certification: Excel and the XMLA clients

## Status

- **Priority**: P1 (the Excel edge is the product; today it lives as tribal knowledge)
- **Effort**: L
- **Risk**: MEDIUM (certifying a row means promising it; only certify what has a trace)
- **Depends on**: 057 (fault discipline), 056 (claims site)
- **Related**: 055 (input contract), 048 (date filters), 049 (nested layouts)
- **Category**: product / compatibility

## Why

The project's most valuable asset is the accumulated Excel/SSAS knowledge — the
1,156-request trace corpus, the VM sweeps, the parity catalog, the claims site.
It is also the least packaged: a new user cannot tell what is certified, what is
a known gap, and what is unsupported. The external review calls this the
"certified, versioned Excel/PivotTable support matrix". The machinery already
exists; this plan turns it into a maintained capability, and closes the three
client blockers that make any "XMLA client" claim dishonest.

## A. The matrix, generated not written

One machine-readable file (or a section of the claims data) where every row is:

- **capability** (e.g. "PivotTable: two row fields, drilldown"),
- **status**: `certified` / `known gap` / `unsupported`,
- **evidence**: probe id, trace sequence (if Excel-side), fixture path, the
  commit that certified it,
- **versions**: proxy version, Excel version(s) exercised.

The docs page is generated from this data; a CI check (like
`site/scripts/check-claims.mjs`) fails when a row lacks evidence or points at a
missing probe. Rows to cover: supported Excel versions; PivotTable layouts;
label/value/Top-N filters; date filters and time-intelligence gestures;
drilldown and collapse; slicers; drillthrough; CUBE functions; refresh
behaviour; number/date formatting; RLS/OLS behaviour; known unsupported
gestures.

## B. Close the client blockers

1. **ADODB `<Format>Tabular</Format>` — DONE 2026-09-26.** The flattened rowset
   is rendered from the same plan and result, and the probe's 5,000 field reads
   now cost the proxy 6 requests (they cost 2,418 until killed). Recorded
   differences: the reference also carries an `(All)` row and G9 scientific
   values.
2. **Subselect / Top-N — DONE 2026-09-26.** The idiom is recognised
   (`[XL_Filter_Set_0]` + `BottomSum`,  the dimension from the `Generate`
   helper, the limit and measure from the `BottomSum` arguments) and turned
   into a member filter before planning: the measure's values over the
   dimension's leaves, ascending, until the running total reaches the limit —
   the reference's semantics, which is why the mirror answers `{All, Toys}` for
   a Top-5 filter over revenue. An end-to-end test with the recorded statement
   asserts exactly that, and a parity case pins it (37/37).
3. **Sessions** — make one decision with plan 058-B7: documented statelessness
   (and a claim recording the divergence) or a bounded session registry. The
   reference faults an unknown session; today we echo it.
4. **Date member naming** — locale short dates and `T00:00:00` unique names
   where we emit ISO; probe the reference, then match or record.

## C. Non-Excel clients

Declare, explicitly: ADOMD and ADODB in (with fixtures), Power BI out until a
probe exists, arbitrary XMLA clients by conformance level. The matrix's front
matter states what "certified" means and what is out of scope.

## Scope

**In**: the matrix data + generator + CI check, docs page, the four blockers,
the client in/out declaration, and a trace-capture recipe for adding a row.

**Out**: new MDX features beyond what certification requires, Excel add-ins,
non-Windows clients without probes, Power BI certification.

## Done criteria

- The matrix is generated and machine-checked; every `certified` row links to a
  probe and (where applicable) a trace; the `known gap` and `unsupported` lists
  are published.
- The ADODB probe costs a bounded number of requests (the 5,000-read script
  finishes against the proxy the way it does against the mirror).
- Top-N either matches the mirror's grid or answers a fault naming the gap.
- The session and date-naming decisions are recorded as claims.

## STOP conditions

- Do not certify a row without a captured probe or trace — a claim without
  evidence is worse than no matrix.
- If the subselect idiom cannot be implemented safely this cycle, fault loudly
  and record the gap; never keep the silent answer.

### The date-key gap, measured (2026-09-26)

`axis-date-key-drilldown` was the last known gap. Measured on the mirror: the
date-key drilldown returns **4,019 members with and without `NON EMPTY`** — the
reference does not prune date members when the axis does not ask it to. The
proxy prunes to the fact-covered dates (2,459) either way, i.e. it behaves as
if `NON EMPTY` were always present.

Impact: nil for Excel (its queries always carry `NON EMPTY`), real for clients
that do not — ADODB/ADOMD members would be missing. Fix shape: render a
non-`NON EMPTY` drilldown axis from the member dictionary (the same one
`MDSCHEMA_MEMBERS` enumerates, 4,019 here) with empty cells for the members that
have no facts, instead of from the group rows. Left as the one known gap rather
than half-landed in a renderer that every query depends on.

### Review round over the sprint (2026-09-26)

Fixed from the review: `MDSCHEMA_HIERARCHIES` now applies `HIERARCHY_NAME` to the
`Measures` row too (the extra row that was misdiagnosed as a key-attribute
hierarchy — an unknown name answers 0 rows); the subselect recognizer **fails
closed** (presence of `XL_Filter_Set_` with any parse miss faults instead of
answering the unfiltered set — one extra space used to fail open) and refuses a
statement carrying more than one filter set rather than binding the wrong
`BottomSum`; `MEASUREGROUP_NAME` filters `MDSCHEMA_MEASURES`; `TMSCHEMA_TABLES`
honours the `Name` it advertises; and the secure default no longer breaks the
repo's own scripts or docs (review-proxy, proxy-smoke, bench, rls-rollup-ab,
two site examples and the Excel-test skill carry `MALLARDCUBE_ALLOW_ANONYMOUS=1`).

Recorded, not fixed:

- **Tabular + filters**: every filtered tabular request now faults (the Top-N
  filter injects a member filter). The claim that the reference refuses those
  shapes is unverified and probably wrong — its flattened rowset looks generic.
  Probe first: the reference's column order, `sql:type` attributes, and whether
  it answers a filtered / multi-measure / two-dimension Execute.
- **`LEVEL_NAME`** is advertised and ignored (five level-row sites).
- **The ranking SQL omits the measure's time-intelligence filter** (latent: the
  `BottomSum` ranking uses unfiltered numbers for a YTD measure while the cells
  use YTD), and the subselect path runs before the fallback and hidden-dimension
  refusals.
- **A hand-written `BottomSum` on the axis** answers a measure-as-member (no
  fault, wrong set) — the mirror of the recognizer gap, pre-existing.
- **`TMSCHEMA_TABLES` + `ID`** is advertised and ignored (`Name` is applied).
- Reference probes outstanding: tabular shapes/order/types, `HIERARCHY_NAME`
  unknown, `LEVEL_NAME`/`MEASUREGROUP_NAME` negatives, `TMSCHEMA_TABLES`
  `Name=nonsense`, `*_VISIBILITY` values other than 0/1, `BottomSum` boundary /
  negatives / ties, and a two-filter Excel capture.

### Probe results: the tabular rowset is generic, and the boundary is right (2026-09-26)

Measured on the mirror, answering the review's outstanding probes:

- **The flattened rowset is generic**, not a narrow shape: it answers a
  multi-measure Execute (1 row, one column per measure), a two-dimension
  crossjoin (105 rows; columns = each dimension's `MEMBER_CAPTION` then the
  measure) and a measure×member crossjoin (one column per tuple,
  `[Measures].[Revenue].[Channel].[Channel].&[Direct]`). Our blanket refusal for
  everything but the lone measure and the single grouped dimension is therefore a
  **limitation, not fidelity**. The multi-measure shape is implemented now; the
  two-dimension and tuple shapes are recorded, with their measured shapes.
- **`BottomSum` boundary**: with a limit of 30,000,000 over categories the mirror
  returns `All | Baby | Toys` with values 49,141,416 (the filtered total) /
  24,701,616 / 24,440,800 — the member that *crosses* the limit is included, so
  our `running >= limit` is right.
- **Negatives**: `HIERARCHY_NAME=nonsense` → 0 rows and `MEASUREGROUP_NAME`
  nonsense on measures → 0 rows and `TMSCHEMA_TABLES Name=nonsense` → 0 rows —
  all matching the fixes; `LEVEL_NAME=Category` → 1 row (still unimplemented).
- **Visibility values other than 0/1 are not a simple no-op**: 
  `DIMENSION_VISIBILITY=2` answers 1 row and `HIERARCHY_VISIBILITY=-1` faults
  ("system error ... range"). Ours treats anything but 0 as visible — recorded.
- A filtered tabular request still needs a *valid* probe (both attempts carried
  invalid MDX: an unescaped `&` and a dimension on two axes); the reference's
  rule for that shape is unmeasured.

### Excel-level confirmation (2026-09-26)

`bash sweep-diff.ps1 -Source proxy` through real Excel, diffed against the
versioned baseline: the only line that moved is `top5`, and it moved to the
mirror's own grid — `grid 3x2 | Toys | 24 440 800,00 | Grand Total | 24 440 800,00`.
The fresh full run is now **byte-identical to the mirror baseline**, so
`parity/sweep3-proxy-baseline.txt` is updated to it: every gesture in the sweep
set produces the same Excel grid against both engines (the two specs that fail
to build fail identically on both, a COM limitation of the harness).

### Level sets list the dictionary; the key hierarchy still needs its view (2026-09-26)

Measured on the mirror: without `NON EMPTY` a level set lists **every** member of
the level (44 quarters, 132 months, 11 years), and only the tuples with data
carry cells; `NON EMPTY` prunes the axis it is written on (my first probe put it
on the measures axis and nothing changed — the keyword is per axis).

The proxy's level listings were data-driven; the tests said so in a comment. Now
level sets and non-key-hierarchy drilldowns without `NON EMPTY` list the
dictionary in its order and emit sparse cells — the ordinals index the full
member list, exactly the reference's shape. Three tests moved from the old
data-driven counts to the reference's (11 / 44 / 132).

The key-attribute hierarchy (`[Date].[Full Date]`) is deliberately excluded: its
members need the view's namespace (`[Date].[Full Date].&[<date>]`), which the
members rowset builds but the axis path does not, so the cells would not match
the group keys (measured: 4,018 members, zero cells). That keeps
`axis-date-key-drilldown` as the one known gap — now with its whole shape known:
All + 4,018 dates, 758 sparse cells on a filtered axis, unique names carrying
`T00:00:00` and locale captions (`1/1/2020`).

### The sweep's top-5 line is harness-flaky (2026-09-26)

After the level-set change, `sweep-diff` repeatedly reported `top5` as the
unfiltered 22x2 grid, while the same spec run directly (`top5-probe.ps1`) showed
the reference's `Toys | 24 440 800,00`. Capturing the sweep's own traffic through
the relay settled it: the swept statement is byte-identical to the recorded
idiom, the proxy answered `Axis0 = {All, Toys}` with two cells — the reference's
shape — and *Excel* displayed the pre-filter grid because its OLAP filter
refresh is asynchronous and the sweep reads `TableRange2` before it settles.

So the product answer for the swept request is verified correct; the sweep's
top-5 line needs a settle (or the relay capture) before it is trusted. The sweep
now sets `BackgroundQuery = $false` as a mitigation.

### The subselect ranking uses the measure's time window (2026-09-26)

Fixed from the review: `filter_members_for_subselect` hand-built a `GroupBy` with
no filters, so a `BottomSum(…, N, [Measures].[Revenue YTD])` ranked its members on
unfiltered revenue while the outer cellset used the YTD window. It now builds its
filters through `filters_with_time_flag`, the same helper the plan builder uses.
The finding was latent (no fixture measure combined a filter idiom with a window),
so it is a latent-wrong-answer fix rather than a behaviour change on the demos.

### The last known gap is closed: zero known gaps (2026-09-26)

`axis-date-key-drilldown` passes. The non-`NON EMPTY` axis is rendered from the
member dictionary, and for the key-attribute hierarchy the members go through
`apply_key_hierarchy_view`, so they carry the view's namespace
(`[Date].[Full Date].&[2020-01-01]`) and its own `(All)`. The member keys then
match the group keys, which is what makes the sparse cells line up — the piece
the earlier half-landing missed (4,018 members with zero cells before; 4,019
members with the reference's cells now).

`parity/catalog.json`: **38/38 matched, no known gaps.** The remaining recorded
differences are the tabular `(All)` row and G9 formatting (ADODB-only), the
date-member naming and the visibility outliers, none of which the catalogue
gates yet.
