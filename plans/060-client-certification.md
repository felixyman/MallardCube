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
