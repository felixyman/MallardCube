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

### Implemented 2026-09-27

`drillthrough_row_predicate(config, user)` decides the lowering: `Ok(None)` for
an unrestricted user, `Ok(Some(sql))` when the drilled (primary) table itself
is filtered, and `Err` for the restrictions a raw `SELECT *` cannot express —

- a filter on another table (the cube-space narrowing needs a join),
- a hidden primary table,
- an OLS-hidden dimension reachable from the drilled table (flat on it, or
  relationship-backed on it): `SELECT *` would return that dimension's columns
  and key values. The column projection that would let these through is the
  follow-up (review 2026-09-27).

The decisions are *effective* ones: another matched role that grants full
access removes a restriction (union semantics, same as the aggregate path).
The request path faults on `Err` with the refusal text and otherwise passes the
predicate into `get_execute_drillthrough_response_with_predicate`, which
appends it to the statement's own slicer filters. The original entry point
stays for tests and the trace replay tool (admin context, noted there).
Callers must run `unhonourable_filter_fault` first: a DAX-only filter surfaces
as `Hidden` and faults.

Unit coverage: `drillthrough_lowers_primary_filters_and_refuses_the_rest`
(primary filter lowered; a `date_dim` filter refused; the effective-union
direction; hidden table refused; administrator unrestricted),
`drillthrough_applies_the_role_predicate` (project3, role
`territory = 'North' AND category = 'Jewelry'` — under the LIMIT, unlike the
unfiltered case — row count equal to direct SQL and no other territory in the
rowset) and `drillthrough_refuses_when_a_reachable_dimension_is_hidden`
(Contoso, a hidden relationship-backed dimension). 622 tests in debug and
release, fidelity 13/13, parity 38/38, smoke 8/8.

Still open: lowering a filtered *dimension* table through its relationship
(and projecting columns around hidden objects) instead of refusing, and the
reference measurement for a row-filtered role's drillthrough (presumed filtered
rows; not re-measured on the mirror). The parity catalogue has no restricted
drillthrough case: no shipped project configures `auth`, so it needs one with
roles first.

## D. Audit and redaction

- Structured audit events (JSONL, opt-in path): auth decision, role resolution,
  scope refusal, OLS/RLS refusal, drillthrough refusal, reload, config change —
  each with request id, user, roles, and the rule that decided.
- Redact credentials from generated artifacts, logs and qualify output
  (connection strings, sidecar paths if sensitive).

### Implemented 2026-09-27

- `audit.rs`: opt-in JSONL stream (`MALLARDCUBE_AUDIT_FILE`), one event per
  line with `ts_unix_ms`, `request_id`, `pid`, `event`, `user`, `admin`,
  `roles`, `rule` and `detail` — identities and rules, never bodies or
  credentials, and the detail is truncated so a caller-controlled value cannot
  grow a record without bound. The request id is generated per request and set
  on both the async handler and the blocking worker, so every decision in one
  request correlates (verified live: a request's `auth` and `refusal` events
  share an id). A record is one `write_all` under `O_APPEND`, so concurrent
  workers cannot interleave halves of two lines; the file is created `0600`.
- Emitted decisions: `auth` (auth mode and role resolution); refusals for the
  model permission (both the streaming `MDSCHEMA_MEMBERS` route and Execute),
  catalog scope, cube scope, a DAX filter that cannot be lowered, a
  drillthrough, a hidden dimension on the plan path, a hidden probe, a
  hidden-dimension probe, authored SQL for a restricted role; plus `error` for
  a failed member dictionary; plus `reload`.
- Fail-closed visibility of the control itself: when the audit file cannot be
  written, the proxy warns once (rather than silently manufacturing false
  assurance) and `/status` reports `audit.enabled`.
- Redaction: the converter's generated load script never embeds the source
  password — the ATTACH carries `Password=<REDACTED>` and a comment to restore
  it from the operator's secret store.
- Tests: the event schema is pinned (exact key set, no body field), long
  details truncate, a detail containing a quote and a newline still lands as
  one parsable line, and both the translation and the full rendered script are
  asserted to contain neither the secret nor the raw connection string.
  626 tests in debug and release, fidelity 13/13, parity 38/38, smoke 8/8.

Still open, recorded: the load script also copies an unparsed M expression and
a source URL verbatim, which can themselves carry credentials (`Odbc.DataSource`
with an inline password, a basic-auth URL) — a `redact_secrets` pass over every
string copied from the model is the follow-up. OIDC claim values are not
audited (only the resolved user and roles); there is no live config reload, so
"config change" is not an event; and `XMLA_TRACE` still records raw bodies by
explicit opt-in and is a debugging trace rather than an audit stream.

## E. Trust boundary

Publish the deployment contract: reverse-proxy/TLS examples (nginx/caddy),
service-account guidance, read-only database and sidecar credentials by
default, and a threat model for the XMLA parser, OIDC, converter inputs and
data refresh. One page, with the failure modes and what is out of scope.

*Implemented 2026-09-27: the Trust boundary page on the site states what the
boundary covers (identity modes, the secure default, roles, drillthrough, the
audit stream), what is deliberately out of scope (column security, TLS
termination inside the proxy, the identity provider, multi-tenancy), gives
nginx and Caddy examples that overwrite the identity header, the service
account and data handling, and a threat-model table over the XMLA parser,
identity headers, OIDC, converter inputs and refresh. It also records the
`XMLA_TRACE` raw-body caveat and the loader-script redaction gap. Site build:
18 pages, 525 internal links OK.*

## F. Column security stance

Not implemented, and said out loud: column-level security belongs upstream
(masked views/materialised columns). `qualify` warns when a role hides a table
but a measure reads its columns, so the boundary is visible rather than assumed.

*Implemented 2026-09-27: `qualify` prints a non-blocking `[NOTE]` per role and
table — "role 'X' hides table 'T' from metadata, but N measure(s) read its
columns (…); column-level hiding is not enforced by the proxy — mask the column
upstream" — without changing the verdict, and the trust-boundary page states
the stance. Test: `column_security_stance_is_reported` (project4, a role hiding
`inventory_fact` names Stock and Cost; no hidden table, no note).*

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

**LEVEL_NAME closed 2026-09-27**: measured on the mirror (MDSCHEMA_LEVELS:
`LEVEL_NAME=Category` returns exactly one row, the match is case-insensitive,
an unknown name returns 0 rows, `LEVEL_VISIBILITY=0` returns 0 rows), then
applied at all six level-row sites (MeasuresLevel, the user-hierarchy `(All)`
and levels, the flat leaf, and both key-hierarchy rows). Two parity cases carry
the mirror's values — the catalogue is **40/40** — and the unit test covers the
name, case, unknown and the leveled and MeasuresLevel sites.

### Measure and member probes gated on the access view, per measure (2026-09-27)

The recorded probe holes are closed: a restricted user no longer counts, names
or resolves objects whose table their roles deny.

- `AccessView.visible_measures: Option<Vec<String>>` replaces the coarse
  `measures_visible` bool. The runtime fills it per measure from the effective
  table filters; the renderer's `[Measures]` slicer hierarchy asks the view
  whether any measure is visible at all.
- `MetaCountLiteral` counts only visible measures: `COUNT([Measures].Members)`
  answers 4 for the administrator and 2 for a role denied one of project4's two
  fact tables, where it used to answer the full model count.
  `COUNT(<member list>)` drops hidden measures and members of OLS-hidden
  dimensions before counting.
- The CUBESET `[Measures].Members` probe lists only the visible measures.
- The runtime refuses probes that name a hidden object: `strtomember` and
  member-only targets, plus set-probe member lists (`MemberList`, Head/Tail
  wrapped, with `&amp;` decoding). The fault names the class, not the hidden
  object. Value queries were already covered by plan-time Gate 2 (asserted now:
  a denied measure's query returns no values while a visible one does).
- All three cChildren builders suppress their synthetic `[Measures]` axes when
  no measure is visible, and the all-level-members probe refuses a
  relationship-backed dimension whose table OLS hides — the one probe that
  builds its axis dimension outside the plan filters (found in review).
- Probe shapes (`SetProbe`/`MemberOnlyProbe`/`MeasureMetadataProbe`) skip the
  data-axis shape validation, which now runs after the set-probe
  classification: `[Measures].Members` on an axis no longer panics in debug
  builds while rendering fine in release.

Unit coverage (618 tests, debug and release): four new tests —
`hidden_fact_tables_disappear_from_measure_probes` (project4),
`no_visible_measures_are_not_advertised_by_probes`,
`cchildren_with_no_visible_measures_hides_the_measures_hierarchy`, and
`all_level_members_probe_refuses_a_hidden_dimension` (Contoso, relationship
backed) — plus 13/13 fidelity, 38/38 parity, 8/8 smoke.

Still unmeasured against the reference: the exact answer for an inaccessible
member in these probes (fault vs omission) and the value shape of the measure
`cChildren` probe. The mirror's current model no longer carries the trace's
measure name (`[Total Sales]`), and deciding fault-vs-omit from the reference
needs role deployment there; the current choice is fail-closed and recorded
here rather than assumed.

### Filtered dimensions count under the role predicate (2026-09-27)

The axis member builders served the static `cardinality_hint` to every role, so
a row-filtered user could read the unfiltered cardinality (and saw counts that
ignored their filter).

- `DimCache` entries are now keyed by `(epoch, dimension, predicate)`.
  `get_filtered` builds a dictionary under the role's RLS predicate, so one
  role's members can never serve another's, and the steady state costs no
  scans: a second request re-queries nothing (asserted with a counting
  backend).
- `AccessView.filtered_dims` carries, per filtered dimension, the predicate and
  the per-level counts from the per-role dictionary. The member builders
  (`all_member_for_*`, `leaf_member_for*`, the key-hierarchy All member) use
  those counts instead of the hint, for both flat and leveled dimensions.
- The drill builders read the *effective* dictionary
  (`axis_members::effective_dictionary`): the no-`NON EMPTY` member list, the
  per-member child counts and `drill_children_cardinalities` all come from the
  filtered dictionary. That helper groups the dictionary's level paths instead
  of issuing its own SQL, so a drill no longer queries at all once the
  dictionary is warm (the first drill per role and epoch pays the build).
- A failed dictionary build faults ("a dimension dictionary query failed: …")
  and consumes the latch, so one transient error neither renders
  `CHILDREN_CARDINALITY 0` nor poisons the pooled connection.
- Administrator and unfiltered paths are unchanged: the access view is only
  attached for restricted users, and the hint remains the fallback.

Unit coverage: `filtered_dimension_counts_use_the_role_predicate` (project3, a
role filtered to `territory = 'North'`) asserts the All member's children
cardinality equals the filtered distinct count, that a second identical request
adds no queries, that a *different* predicate gets its own entry (its count, and
a rebuild), and that the no-`NON EMPTY` drilldown dictionary lists no member
from outside the role's rows;
`a_failed_dictionary_build_faults_and_clears_the_latch` pins the failure
behaviour. 620 tests in debug and release, fidelity 13/13, parity 38/38,
smoke 8/8.

Still open from the review of this section (recorded, not hidden):

- The per-member child counts a drill overwrites come from the filtered
  dictionary now, but the *level-wide* value the builders place there first
  (`per_level[level+1]`) is still a whole-level number used as one member's
  children — the same shape as the old static hint, pre-existing.
- The rewritten `drill_children_cardinalities` narrows to the requested
  ancestor prefix where the old SQL (for a key shorter than the drilled level)
  counted globally; that is a fix, but no test or catalogue case pins it.
- `parity/catalog.json` has no `CHILDREN_CARDINALITY`/`DISPLAY_INFO` case, so
  the emitted per-member counts are unguarded by the corpus; a `DrilldownLevel`
  case with `DIMENSION PROPERTIES CHILDREN_CARDINALITY` needs a
  mirror-recorded value.
- A flat dimension's leaf members now report 0 children for a filtered role
  where an unfiltered role keeps the hint, so the expand affordance differs by
  role. Less capability revealed, but unmeasured against the reference.
- Counts are computed for the dimensions whose *discovery table* is filtered;
  a relationship-backed dimension reached through a filtered fact table keeps
  the hint (the role may read all of the dimension table, so this is a fidelity
  gap, not a leak).
- The per-role entries are bounded by distinct predicates × dimensions and are
  kept until the next reload; a deployment that filters per user, rather than
  per role, should watch that map (`DimCache` has no eviction of its own).
- Unmeasured against the reference: whether SSAS reports the filtered
  cardinality for a row-filtered role, and the exact `DISPLAY_INFO` bit
  pattern there. These counts follow the filtered cube space, which is the
  fail-closed reading.
