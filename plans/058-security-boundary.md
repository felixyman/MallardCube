# Plan 058 — Security boundary: isolation complete, defaults secure

## Status

- **Priority**: P1 (a staple cannot have an insecure default or a half-closed boundary)
- **Effort**: L
- **Risk**: MEDIUM (auth defaults change how deployments start; audit events are new surface)
- **Depends on**: 057 (fault taxonomy and probe discipline)
- **Related**: 026 (roles/UserContext), 051 (RLS/OLS rounds), 054 (settings surface)
- **Category**: security

## Why

The security work so far is real but unfinished: RLS predicates on the SQL, OLS
on the metadata rowsets, scope checks on catalogs/cubes, budgets that bound
resources. What remains is exactly what an on-prem operator notices first:

- binding `0.0.0.0` without auth serves every request as administrator;
- a recorded list of metadata/probe holes still leaks information (plan 051);
- restricted users are denied drillthrough instead of served filtered rows;
- there are no audit events, and generated artifacts can carry credentials;
- there is no published trust boundary, so every deployment invents one.

## A. Secure by default

- A loopback bind (`127.0.0.1`) stays frictionless — the 5-minute demo must not
  grow a setup step.
- A non-loopback bind without a configured auth mechanism **refuses to start**,
  unless `MALLARDCUBE_ALLOW_ANONYMOUS=1` is set explicitly (loud warning, and
  `/status` reports `anonymous: true`).
- CI proves both paths: 0.0.0.0 without auth exits non-zero; with the env var it
  starts and reports the posture; the demo on loopback is unchanged.

## B. Close the recorded holes

Each item gets a probe in `probe-fidelity.sh` or `parity/catalog.json` carrying
the reference's measured value (the VM probes from the 2026-09-25 review are
listed at the end of plan 051):

1. **`<Catalog>` property carried by every request.** Today only Execute and the
   variants with `Restrictions` carry it; `TMSCHEMA_*`, `DISCOVER_*` and
   `DBSCHEMA_TABLES` (`TABLE_CATALOG`) serve local rows for a foreign property.
   Fix shape: carry the property at request level, or attach `Restrictions` to
   every Discover variant.
2. **`MEASUREGROUP_NAME`** is advertised by `MDSCHEMA_MEASUREGROUPS` and
   `MDSCHEMA_MEASUREGROUP_DIMENSIONS` but never stored; store it and narrow the
   rowsets case-insensitively.
3. **Measure probe plans gated** on measure visibility for restricted users:
   `MeasuresList`, `MetaCountLiteral`, the set-probe member list, and the
   measure-target `CCHILDREN` probe.
4. **Filtered-dimension cardinalities**: axis `DISPLAY_INFO`/child counts come
   from static model hints; compute them with the role predicate for filtered
   dimensions.
5. **`STRTO_MEMBER`/member-only probes** must refuse or omit hidden dimensions
   and members.
6. **Foreign-namespace `<foo:Catalog>`** currently counts as the XMLA property;
   probe the mirror first, then match.
7. **Session catalog / session ids**: decide once — documented statelessness
   (the current divergence) or a bounded session registry. Probe the reference's
   `BeginSession`/unknown-session fault before choosing.

## C. Drillthrough for restricted users

Apply role predicates and OLS to drillthrough SQL, matching direct SQL for a
filtered role's rows, instead of the current refusal. Keep the refusal as the
fallback when a predicate cannot be lowered, and keep the fault text honest.

## D. Audit and redaction

- Structured audit events (JSONL, opt-in path): auth decision, role resolution,
  scope refusal, OLS/RLS refusal, drillthrough refusal, reload, config change —
  each with request id, user, roles, and the rule that decided.
- Redact credentials from generated artifacts, logs and qualify output
  (connection strings, sidecar paths if sensitive).

## E. Trust boundary

Publish the deployment contract: reverse-proxy/TLS examples (nginx/caddy),
service-account guidance, read-only database and sidecar credentials by
default, and a threat model for the XMLA parser, OIDC, converter inputs and
data refresh. One page, with the failure modes and what is out of scope.

## F. Column security stance

Not implemented, and said out loud: column-level security belongs upstream
(masked views/materialised columns). `qualify` warns when a role hides a table
but a measure reads its columns, so the boundary is visible rather than assumed.

## Scope

**In**: secure-default logic + CI, the seven recorded holes, drillthrough
predicates, audit events, redaction, trust-boundary docs, column-security
stance.

**Out**: column-level security implementation, a session registry until the
probe decides, TLS termination inside the proxy, OIDC provider building.

## Done criteria

- The secure-default CI test exists and passes; the demo is unaffected.
- Every one of the seven holes has a probe whose recorded value is the
  reference's, and the proxy matches it.
- Drillthrough with a role returns the same rows as direct SQL with the role
  predicate, proven by a test.
- Audit events are emitted and covered by a test; docs publish the trust
  boundary and the column-security stance.

## STOP conditions

- Do not invent session semantics before the mirror probe; keep the documented
  statelessness until then.
- Do not implement column-level security in the proxy — enforce the upstream
  view boundary instead.

### Scope measurements and the two divergences they found (2026-09-25, VM)

The review's probe list, answered by the mirror (MallardDemo/Model) and
compared with the proxy:

| request | reference | proxy |
|---|---|---|
| `MDSCHEMA_MEMBERS`, foreign `CUBE_NAME` | 0 rows | 0 rows (matches) |
| `MDSCHEMA_MEMBERS`, foreign `<Catalog>` | access fault | fault (matches) |
| `DBSCHEMA_CATALOGS`, foreign `CATALOG_NAME` | 0 rows | 0 rows (matches) |
| `MDSCHEMA_PROPERTIES`, catalog-only mismatch | 0 rows | 0 rows (matches) |
| `DRILLTHROUGH … FROM [NoSuchCube]` | `The NoSuchCube cube does not exist.` | fault (matches) |
| `MDSCHEMA_DIMENSIONS`, `<CUBE_NAME></CUBE_NAME>` | **0 rows** | 0 rows after the fix |
| `TMSCHEMA_TABLES`, foreign `<Catalog>` | **fault** | fault after the fix |
| `DISCOVER_SCHEMA_ROWSETS`, foreign `<Catalog>` | ignored (132 rows) | ignored (63 rows — our catalogue is smaller, unrelated) |
| `MDSCHEMA_DIMENSIONS` with no/empty `<Catalog>` | 3 rows (the server's default catalog) | 6 rows (the configured catalog) — the documented stateless divergence |

Two fixes came out of it: an **empty scope value matches nothing** (ignoring it
served the configured catalog), and **`TMSCHEMA_*` carries the `<Catalog>`
property** and faults a foreign one — while `DISCOVER_SCHEMA_ROWSETS`
legitimately ignores it, so the treatment is per-rowset, not global.

Five parity cases were added (`members-wrong-cube-is-empty`,
`catalogs-wrong-catalog-is-empty`, `properties-wrong-catalog-is-empty`,
`dimensions-empty-cube-restriction-is-empty`, `drillthrough-unknown-cube-faults`)
→ 24/24 matched with the two known gaps.

Still open in this section: the empty/absent `<Catalog>` default-catalog
divergence above, and the same probe treatment for `DISCOVER_PROPERTIES`,
`DISCOVER_LITERALS` and `DBSCHEMA_TABLES` (`TABLE_CATALOG`).

### Advertised restrictions are now applied (2026-09-26)

The mirror's `DISCOVER_SCHEMA_ROWSETS` answered which restrictions each rowset
advertises — our lists already matched it exactly. What did not match was the
*behaviour*: `DIMENSION_VISIBILITY`, `HIERARCHY_VISIBILITY`, `LEVEL_VISIBILITY`,
`MEASURE_VISIBILITY`, `PROPERTY_VISIBILITY` and the name restrictions were
accepted and ignored. Measured on the reference: every `*_VISIBILITY=0` returns
**0 rows** and `=1` returns everything (6 dimensions, 7 hierarchies, 16 levels,
6 measures, 39 properties, 11 measure-group dimensions); `MEASURE_NAME=Revenue`
returns exactly that measure; an unknown `MEASUREGROUP_NAME` returns 0 rows.

Implemented as two rules in `discover`: `hidden_by_visibility` (0 ⇒ the empty
rowset, since we only serve visible objects) and `name_matches` (case-insensitive
exact names), applied in dimensions, hierarchies, levels, measures, properties,
measure groups and measure-group dimensions. Verified live and pinned by twelve
new parity cases — `dimensions-visibility-zero` lost its `known_gap`, so the
catalogue is **36/36 with one known gap left**.

Recorded: `LEVEL_NAME` is advertised and parsed but applied at none of the five
level-row sites (a half-applied filter would silently drop rows, so it was
reverted rather than half-done — review F4). The earlier note here misdiagnosed
`HIERARCHY_NAME=Category` returning two rows: the extra row is the special-cased
`Measures` hierarchy, which the name filter never saw (the reference returns 0
rows for an unknown `HIERARCHY_NAME`), not a key-attribute hierarchy. Fixed.
