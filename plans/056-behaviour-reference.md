# Plan 056 — Reference: how Excel / SSAS / MDX actually behave

Status: planned. Do after the currently planned work (plan 055 slices 2–4, then
the RLS and feature-parity buckets).

## Goal

A public, claim-based **Reference** section on MallardCube's site, written for
people building Excel-facing OLAP engines. Every claim carries its
environment, how it was observed, and a reproduction — and the same claim set
becomes the single source for the repo's agent skills, which today duplicate
the same facts (and have already aged).

## Why

Microsoft's spec describes protocol shapes. The expensive failures live one
layer above it — what **Excel sends and reads**, and how **SSAS answers**. The
questions that cost this project days are exactly the ones a reference should
answer up front:

- Excel reads `MDSCHEMA_PROPERTIES` **positionally**, and `memberValueDatatype`
  is stored per **attribute** hierarchy — the root cause of the date-filter
  saga (plan 048).
- `HIERARCHY_ORIGIN=6` for `[Measures]`; `2` for attribute hierarchies, and
  why the key bit (6) makes Excel write `memberValueDatatype="5"`.
- `PREFERRED_QUERY_PATTERNS=3` gates whether Excel asks for `MEMBER_VALUE` at
  all; `PROPERTY_TYPE=1/3/4` answers empty; `DISCOVER_SCHEMA_ROWSETS` must
  honour `SchemaName`.
- MSOLAP sends `<Statement/>` with `BeginSession` — faulting it breaks every
  connection (plan 055 slice 1's near-miss); MSOLAP refuses plain-HTTP
  `msmdpump` ("must use secure channels"); a stale WinINET proxy with
  `<-loopback>` breaks MSOLAP while `curl` keeps working.
- SSAS semantics that take one probe to settle but are undocumented together:
  an unknown restriction **faults**, a wrong `CUBE_NAME` answers an **empty**
  rowset, `DIMENSION_VISIBILITY=0` returns nothing.
- Cellset shape rules: member element order, `CHILDREN_CARDINALITY` only when
  requested, axis placement by hierarchy/level numbers, the
  `DrilldownMember(CrossJoin(…))` shape Excel sends for two row fields.

None of this is customer-specific or MallardCube-specific; it is reference
behaviour of the ecosystem.

## Audience and voice

- **Engine implementers**, protocol-level and repro-driven. A claim must be
  actionable: "send X, and Excel does Y; if your engine does Z, the field
  comes up empty".
- No marketing, no product pitch inside a claim. Product relevance goes in a
  short "what this means for an engine" note at most.
- Open questions are published **as open**, not omitted: Top-N's missing
  `TopCount`, RLS propagation, `NON EMPTY` semantics. A reference that
  overstates is worse than none.

## Content model — one source of truth

```
reference/
  claims.jsonl          # one JSON object per claim (machine-checkable)
  pages/*.md            # prose pages, reference claims by id
```

A claim:

```json
{
  "id": "membervalue-per-attribute-hierarchy",
  "title": "Excel stores memberValueDatatype per attribute hierarchy",
  "status": "verified",
  "environment": {
    "engine": "SSAS 2025 tabular",
    "compat": "1700",
    "edition": "StandardDeveloper64",
    "client": "Excel 365 16.0.x",
    "date": "2026-09-22"
  },
  "method": "relay capture of pivotCacheDefinition + MDSCHEMA_PROPERTIES probes",
  "repro": "https://github.com/<repo>/tree/master/reference/repro/member-value.md",
  "catalog_case": "levels-date-key",
  "supersedes": "optional-earlier-claim-id",
  "notes": "user hierarchy gets time=\"1\" but no memberValueDatatype; attribute hierarchy gets \"7\" from its level's MEMBER_VALUE DATA_TYPE"
}
```

- `status`: `verified` · `open` · `superseded`
- `supersedes` / `superseded_by`: when a newer direct measurement disproves an
  older claim, the old one keeps its id with `status: superseded` and a link, so
  a page shows the history instead of quietly changing its mind. First case: the
  nested `<restriction>` forms were recorded as honoured in plan 051;
  measurement (plan 055) shows the reference rejects them as schema errors.
- `catalog_case`: optional link to `parity/catalog.json`; when present the claim
  is re-checked by `probe-parity.sh` (the catalog case gains an optional
  `reference_claim` field and the checker asserts the link both ways).
- `notes` is markdown; long prose lives in pages.

Pages are MDX under `site/src/content/docs/reference/` and pull claims in via a
small `<Claim id="…"/>` component that reads `claims.jsonl` at build time, so
the claim text can never drift between the registry and a page.

## Site integration

Add a **Reference** section to the Starlight sidebar, seeded with:

| page | content |
|---|---|
| `reference/index` | what this is, the claim model, how to verify a claim yourself |
| `reference/excel-metadata` | rowsets Excel reads, positional reads, `PROPERTY_TYPE` semantics, visibility, `SchemaName`, `MEMBER_VALUE`, `PREFERRED_QUERY_PATTERNS` |
| `reference/date-filters` | the full root cause, the exact subquery Excel sends, what an engine must implement (plan 048) |
| `reference/cellset-shape` | member element order and fields, level numbers, cardinality rules, axis placement, dimension properties |
| `reference/pivot-gestures` | per-gesture request shapes: drilldown, nested fields, value/label/Top-N filters, subtotals, page fields, multi-select, Show Values As, drillthrough — with trace excerpts |
| `reference/engine-semantics` | restriction contracts, scope semantics, measure/level types, RLS (open items marked) |
| `reference/probing-cookbook` | ADOMD vs DMV vs relay vs UIA; environment traps (WinINET, secure channels, empty statements, session begin) |

`connect-excel.mdx` and `mdx-support.mdx` link into Reference for the "why".

Deployment: `.github/workflows/deploy-docs.yml` triggers on `site/**` only, so
`reference/**` must be added to its paths (or the registry moves under `site/`)
— a claims-only change has to rebuild the site.

## Consolidation of the skills

When the first pages land, move the facts out of the agent skills:

- `ssas-reference-oracle` shrinks to **procedures** (VM setup, deploy/process a
  model, ADOMD/DMV probing, relay capture) plus pointers into `reference/`.
  Its "verified metadata rules" tables become claims.
- `windows-mcp-desktop` keeps UI procedures; Excel dialog behaviour becomes a
  claim under `pivot-gestures` (or a small `excel-ui` page).
- `proxy-excel-test` keeps the test methodology, pointing at Reference for the
  behaviour it asserts.

No fact may exist in two places: skills link, pages render claims.

## Verification and CI

- `site/scripts/check-claims.mjs` (wired into `npm run build`, next to
  `check-links.mjs`): required fields, unique ids, status enum, ISO dates,
  non-empty `method`/`repro`, `catalog_case` resolves to a real
  `parity/catalog.json` id, and every `<Claim id>` used by a page resolves.
- Claims with a `catalog_case` are re-checked whenever `probe-parity.sh` runs;
  the catalog case's `source` field names the claim id.
- Staleness: a page renders each claim with its environment and date; claims
  older than twelve months are flagged "stale — re-verify" (computed at build
  time, not hand-maintained).

## Content hygiene and licensing

- Our own observations only; link to Microsoft docs, never copy their text.
- Generic demo data only; no customer content (repo rule).
- Prefer text transcripts of requests/responses over screenshots; when a UI
  behaviour is the claim (the date-filter dialog), record element names and
  actions rather than pixels.

## Seeding inventory (what already exists, and where)

| source | becomes |
|---|---|
| `.agents/skills/ssas-reference-oracle/SKILL.md` tables (dimension types, hierarchy origins, level types/dbtypes, member-value rows, positional reads, `PROPERTY_TYPE`, `SchemaName`, `PREFERRED_QUERY_PATTERNS`, attribute-hierarchy axis members, compound unames, member order, `CHILDREN_CARDINALITY`, date-filter rule + captured MDX + dialog behaviour) | claims under `excel-metadata`, `date-filters`, `cellset-shape`, `pivot-gestures` |
| plan 048 (date-filter root cause), plan 051 (parser/RLS/hygiene findings), plan 055 (restriction contract, scope semantics, reference probes) | claims under `date-filters`, `engine-semantics` |
| `xmla-trace.jsonl` + the sweeps (`scripts/vm/sweep*.ps1`, `parity/sweep*-baseline.txt`) | `pivot-gestures` excerpts |
| VM procedures (VM readiness, relay, UIA helpers, `Roles=` RLS probing, TMSL deploys) | `probing-cookbook` |
| this session: empty-Statement session begin, secure-channel requirement, WinINET trap, `RPC_E_CALL_REJECTED` | `probing-cookbook`, `engine-semantics` |

## Deliverables, in order

1. Scaffolding: `reference/claims.jsonl`, the `<Claim>` component,
   `check-claims.mjs` in the build, the sidebar section and `reference/index`.
2. The three highest-value pages: `excel-metadata`, `date-filters`,
   `cellset-shape` — migrating the skill's tables with no new prose.
3. `pivot-gestures` (excerpts rendered from traces; add a small script to emit
   the request/response snippets).
4. `engine-semantics` and `probing-cookbook`.
5. Skill consolidation (trim the three skills to procedures + pointers).
6. Contributor note: how to add a claim (run a probe, stamp the environment,
   link a catalog case when one exists).

## Risks

- **Drift** if claims are not re-verified: mitigated by the catalog link and
  staleness flags — but only for claims the parity runner covers; the rest
  rely on the date stamp.
- **Over-formalising**: if the claim schema slows writing, loosen it (keep
  `id`, `status`, `environment`, `method`, `date`) rather than skipping
  provenance.
- **Scope creep** into a general MDX tutorial: stay on observed behaviour.
- **Published open questions** may read as shortcomings: frame them as the
  point of the exercise (a reference that says what is *not* settled).

## Acceptance

- The three seed pages published, every claim with environment + method + date.
- `npm run build` fails on a malformed claim, a dangling `<Claim id>`, or a
  dangling `catalog_case`.
- The skills contain no duplicated facts — only procedures and pointers.
