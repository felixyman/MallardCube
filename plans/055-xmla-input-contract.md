# Plan 055 — the XMLA input contract: every accepted input consumed or refused

Status: slice 1 landed; slices 2–4 open.

## Why

The last four review findings were one failure class: an input the proxy
accepted and ignored (`MEMBER_TYPE`, `CATALOG_NAME`/`CUBE_NAME`, nested
restrictions, a `Statement` element that vanished). This plan makes the surface
explicit and enforces it, so future findings are instances of a rule rather
than surprises.

## The contract

`DISCOVER_SCHEMA_ROWSETS` advertises, per rowset, the restriction names clients
may send. `schema_rowsets::advertised_restrictions()` / `advertises()` parse
that same `SCHEMA_ROWSET_DATA`, so the contract cannot drift from what clients
are told, and a test keeps every row's `RestrictionsMask` equal to its list's
bit mask.

## Reference semantics (SSAS 2025 tabular via the mirror, 2026-09-24)

| request | reference |
|---|---|
| unknown restriction name | SOAP fault: *"The restriction, BOGUS_NAME, is not recognized by the server."* |
| `CUBE_NAME` / `CATALOG_NAME` naming something else | **empty rowset** (not a fault) |
| `DIMENSION_UNIQUE_NAME` that matches | filters (1 row) |
| `DIMENSION_VISIBILITY=0` / `=1` | 0 rows / all six |
| unknown name with an **empty** value (self-closing or paired) | still a fault — the name is checked regardless of the value |
| advertised name with an empty value | **ignored** (no filter) |
| duplicate restriction names | **last wins** (`match-then-nomatch` → 0 rows) |
| nested `<restriction>` / `<column>`/`<value>` | **schema fault** — not valid XMLA |
| raw named child directly under `<Restrictions>` | **schema fault** |
| advertised lists | ours match the reference name-for-name (MEMBERS 14, DIMENSIONS 7, MEASURES 8, HIERARCHIES 9, LEVELS 10, PROPERTIES 13; identical masks) |

## What Excel actually sends (scan of 2,402 recorded requests)

- Almost everything is **flat**. `DISCOVER_PROPERTIES.PropertyName` arrives in
  the nested `<PropertyName><Value>x</Value>…</PropertyName>` form (348 nested,
  174 flat) — `<Value>` is a value, never a name. Two `MDSCHEMA_MEMBERS`
  requests use the `<restriction>` element form, which the reference rejects at
  the schema layer; the first version of this paragraph missed them because the
  scan only looked at `<RestrictionList>` (review, 2026-09-24).
- Names seen: `CATALOG_NAME`, `CUBE_NAME`, `HIERARCHY_UNIQUE_NAME`,
  `HIERARCHY_VISIBILITY`, `LEVEL_UNIQUE_NAME`, `MEASURE_VISIBILITY`,
  `DIMENSION_UNIQUE_NAME`, `MEMBER_TYPE`, `MEMBER_UNIQUE_NAME`, `TREE_OP`,
  `PROPERTY_NAME`, `PROPERTY_TYPE`, `PROPERTY_VISIBILITY`, `SchemaName`,
  `PropertyName` — each advertised for the rowset that sends it.

## Slice 1 — landed

- `Restrictions.seen` collects every restriction name; the parser returns
  `UnsupportedRestriction` when a name is not advertised for the requested
  rowset, and the dispatcher faults with the reference's message.
- `<Value>`/`<value>` are values, never names; `PropertyName` is recorded as a
  name (so misrouting is caught like the rest).
- Verification: an earlier 2,402-request corpus (probes plus a sweep) was lost
  to a traced restart — `XMLA_TRACE_FILE` now exists so gate runs cannot
  truncate a corpus again. The reproducible evidence is a fresh 1,156-request
  sweep corpus: replaying it produces **0 contract faults and 0 unexpected
  faults**, alongside 16/16 parity (3 known gaps), 13/13 fidelity, 8/8 smoke,
  and `sweep-diff -Source proxy` → `SWEEP OK`.
- Follow-up round 1: `<Value>` is a value only inside its naming element (a
  direct one is an unadvertised name and faults); control characters are
  rejected in text, CDATA and attributes.
- Follow-up round 2 (all measured on the reference): **namespace binding
  decides, not the prefix** — a prefixed element bound to the XMLA namespace is
  accepted, a foreign namespace or an undeclared prefix is not; control
  characters in comments, PIs, attribute names and element names are rejected
  ("Illegal xml character"); a nested `<PropertyName>` is a schema fault; two
  `<Restrictions>` are a schema fault even when one is self-closing. The parser
  now reads resolved namespaces (`NsReader`) and tracks open elements, so
  structural checks are about the schema, not the spelling.

## Slice 2 — open: advertised but unapplied

| rowset | consumed today | advertised but unapplied |
|---|---|---|
| DBSCHEMA_CATALOGS | — | `CATALOG_NAME` |
| DBSCHEMA_TABLES | — | `TABLE_*` |
| MDSCHEMA_CUBES | — | scope, `CUBE_SOURCE`, `BASE_CUBE_NAME` |
| MDSCHEMA_DIMENSIONS | — | scope, `DIMENSION_NAME`, `DIMENSION_UNIQUE_NAME`, `CUBE_SOURCE`, `DIMENSION_VISIBILITY` |
| MDSCHEMA_HIERARCHIES | dimension, hierarchy | scope, `HIERARCHY_NAME`, `HIERARCHY_ORIGIN`, `CUBE_SOURCE`, `HIERARCHY_VISIBILITY` |
| MDSCHEMA_LEVELS | dimension, hierarchy, level | scope, `LEVEL_NAME`, `LEVEL_ORIGIN`, `CUBE_SOURCE`, `LEVEL_VISIBILITY` |
| MDSCHEMA_MEASURES | — | scope, `MEASURE_NAME`, `MEASURE_UNIQUE_NAME`, `MEASUREGROUP_NAME`, `CUBE_SOURCE`, `MEASURE_VISIBILITY` |
| MDSCHEMA_PROPERTIES | cube scope flag, dimension, hierarchy, level, `PROPERTY_TYPE`, `PROPERTY_NAME` | member, `PROPERTY_CONTENT_TYPE`, `PROPERTY_ORIGIN`, `CUBE_SOURCE`, `PROPERTY_VISIBILITY` |
| MDSCHEMA_MEMBERS | dimension, hierarchy, level, `MEMBER_TYPE`, `MEMBER_UNIQUE_NAME`, `TREE_OP` | scope, `LEVEL_NUMBER`, `MEMBER_NAME`, `MEMBER_CAPTION`, `CUBE_SOURCE`, `SCOPE` |
| MDSCHEMA_FUNCTIONS | `ORIGIN` | `LIBRARY_NAME`, `INTERFACE_NAME`, `FUNCTION_NAME`, `CATALOG_NAME` |
| MDSCHEMA_SETS / KPIS / MEASUREGROUPS / MEASUREGROUP_DIMENSIONS | — | all |
| TMSCHEMA_* | — | `ID`, `Name`, `TableID`, … |
| DISCOVER_* | `PropertyName`, `SchemaName` | the rest |

Semantics to implement: scope names that mismatch → empty rowset; selection
filters → applied; visibility `0` → excluded. Two gaps are already pinned by
catalog cases (`dimensions-visibility-zero`, `dimensions-wrong-cube-is-empty`).

## Slice 3 — open: Execute `<Properties>`

Unparsed today: `Catalog` (validate), `Format` (Tabular vs Multidimensional),
`Content` (`SchemaData` vs `Data`), `AxisFormat`, `LocaleIdentifier` (the
"232 966,0" number-format item), `Timeout` (we have our own). The reference's
behaviour for each needs recording before implementing.

## Slice 4 — open: catalog cases for every slice-2/3 behaviour

Add one `parity/catalog.json` case per implemented behaviour, with the mirror's
value — and a `known_gap` on any that remain unimplemented.

Follow-up round 3 (all measured on the reference): every element the protocol
interprets must be in the XMLA namespace (a foreign `RequestType`, `Execute`,
`Command` or `Statement` faults); the envelope must be SOAP **1.1** (SOAP 1.2,
a foreign envelope and an XMLA-default envelope fault); element and attribute
names cannot carry reference syntax (`Request&amp;Type`); and all DTDs are
prohibited. The session id is only echoed when it is a plain token (a raw
capture from a rejected request made the response unparsable), and `xml_escape`
escapes CR as `&#xD;` so values round-trip.

Deliberate divergence from the reference: a request whose elements declare
*no* namespace (`ResolveResult::Unbound`) is accepted, where the reference's
schema requires the XMLA namespace. Kept because unit tests and hand-written
curl probes are easier to read un-namespaced, and a client that omits the
namespace is malformed rather than dangerous — revisit if one ever relies on
it.
