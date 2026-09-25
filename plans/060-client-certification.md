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

1. **ADODB `<Format>Tabular</Format>`** — diagnosed 2026-09-25: ADODB asks for
   a flattened rowset, the reference answers one, we answer a cellset, and
   MSOLAP re-sends the query once per field read (5,000 reads: mirror 6
   requests, proxy 2,418). Implement the rowset shape from the same SQL plan
   output and add the ADODB probe as a regression with a bounded request count.
2. **Subselect / Top-N** — Excel sends the Top-5 filter as a server-side
   subselect (`Generate` / `BottomSum` / `Except` / `DrilldownLevel`). Either
   implement the idiom (or the general subselect semantics) or fault loudly;
   what is not acceptable is today's silent full-set answer while the mirror
   filters.
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
