# Plan 026: Security roles and UserContext for the direct-SQL runtime

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 46eeb39..HEAD -- src/project/config.rs src/engine/plan.rs src/engine/sql.rs src/engine/model.rs src/main.rs src/xmla/discover/members.rs src/tools/qualify.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: none (Plan 024 was the decision gate; this is the implementation)
- **Category**: security
- **Planned at**: commit `46eeb39`, 2026-06-22

## Why this matters

The proxy currently has no concept of a user. Every request sees every row of
every table, and `RoleConfig` is deliberately informational-only. For an
enterprise SSAS replacement this is a hard blocker: most real Tabular models
ship with row-level security (RLS) on dimension tables (e.g. "Region managers
see only their region") and Excel users expect Windows Authentication to drive
that filtering.

This plan introduces a `UserContext` that flows from the auth boundary through
`route_request` into plan generation and SQL emission, plus a conservative
subset of SSAS Tabular role semantics — RLS via SQL predicates, model-level
read/deny, and measure visibility — that is enough to enforce real customer
roles without attempting the full SSAS security surface. Native Kerberos is
explicitly out of scope; the plan defines a trusted-identity boundary that a
reverse proxy (IIS/nginx) can feed, and documents that boundary.

## Current state

Relevant files and their roles:

- `src/project/config.rs` — `ProxyConfig` already has `roles: Vec<RoleConfig>`
  (serde default empty), but `RoleConfig` is intentionally minimal:
  `{ name, description }` only. Comment at lines 142-157 says roles are
  informational, not enforced, and surface as PARTIAL in `qualify`.
- `src/engine/plan.rs` — `QueryPlan` enum (Total/GroupBy/Count/Empty),
  `TypedDimensionFilter { dimension, members, time_flag }`,
  `plan_from_semantic_with_model()` builds plans, `compatible_filters()` drops
  unrelated dim filters (SSAS behavior). Filters flow into SQL via
  `filters_with_time_flag()`.
- `src/engine/sql.rs` — `sql_for_query_plan()` emits DuckDB SQL.
  `joins_and_where()` (line 140) and `sql_where_with_cols()` (line 97) build
  JOIN + WHERE from `TypedDimensionFilter[]`. Filters emit as
  `col IN ('v1', 'v2')`. This is the injection point for role predicates —
  role filters are just additional WHERE predicates added to every fact-table
  scan.
- `src/engine/model.rs` — `SemanticModel` has fact_tables, dimensions,
  measures, relationships, date_dim/date_dims. `dim_table_for_discovery()`
  resolves physical table for distinct queries. `rel_for_dimension()` finds
  relationship. No role/filter state on model. `DimensionDef`/`MeasureDef`
  have `visible: bool` but no per-role visibility.
- `src/main.rs:280-479` — `handle_xmla` is async, parses request, clones into
  `spawn_blocking` closure with `backend_source.checkout()`, calls
  `route_request(&request, &body, &backend)`. `route_request` dispatches by
  `XmlaRequest` variant. Execute calls
  `get_execute_cellset_response_timed_malloy_with_backend(mdx, backend)`.
  MDSCHEMA_MEMBERS calls `get_members_response_with_backend(member_filter,
  tree_op, backend)`. No `UserContext` exists today. `AppState` only has
  `backend_source`.
- `src/xmla/discover/members.rs` — `build_leaf_member_rows` runs
  `SELECT DISTINCT {physical_field} FROM {dim_table} ORDER BY {physical_field}`
  — this is where member-level RLS must filter (role filter on dimension table
  applied to this query). `get_members_response_with_backend` is the public
  entry.
- `src/tools/qualify.rs` — `qualify()` flags any non-empty `p.config.roles`
  as PARTIAL with reason "N unsupported security role(s) detected in config"
  (lines 100-105). Tests assert `generated_project` is PARTIAL due to roles.

Current `RoleConfig`:

```rust
// src/project/config.rs:142-157
/// Security role detected during Tabular model conversion.
///
/// Deliberately minimal: captures role name and description only.
/// Does NOT capture full SSAS role semantics (table permissions, row filters,
/// member security). The proxy does not enforce roles at runtime — they are
/// informational, surfacing in `qualify` as PARTIAL to remind operators that
/// security must be handled outside the proxy.
///
/// An `enforced` field is intentionally omitted (YAGNI): no role will be
/// `enforced: true` until runtime enforcement is implemented.
#[derive(Debug, Clone, Deserialize)]
pub struct RoleConfig {
    pub name: String,
    #[serde(default)]
    pub description: String,
}
```

Current SQL WHERE construction (the injection point):

```rust
// src/engine/sql.rs:97-137
fn sql_where_with_cols(
    model: &SemanticModel,
    filters: &[TypedDimensionFilter],
    col_map: &HashMap<String, String>,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    for f in filters {
        if f.time_flag.is_some() { /* date_dim subquery */ continue; }
        if f.members.is_empty() { continue; }
        if let Some(d) = model.dim_def_opt(&f.dimension) {
            let col = col_map.get(f.dimension.as_str())
                .cloned().unwrap_or_else(|| d.physical_field.clone());
            let vals: Vec<String> = f.members.iter()
                .map(|m| format!("'{}'", m.replace('\'', "''"))).collect();
            parts.push(format!("{} IN ({})", col, vals.join(", ")));
        }
    }
    if parts.is_empty() { String::new() }
    else { format!(" WHERE {}", parts.join(" AND ")) }
}
```

Repo conventions to match:

- Tests live inline in `#[cfg(test)]` modules; see
  `docs/DEVELOPER-GUIDE.md:224-240`.
- New server-path code should prefer explicit dependency passing over adding
  new globals; the `BackendSource` / `AppState` pattern from Plan 025 is the
  exemplar. `UserContext` should be threaded explicitly, not stored in a
  global.
- `README.md:8-9` — "Direct SQL is the default runtime path." Roles must
  enforce on the direct-SQL path. Malloy runtime enforcement is a follow-up.
- `CONTEXT.md:73-77` — prefer correctness over guessing, fail closed on
  unsupported semantics. Unknown role shapes must fail closed, not silently
  grant access.

## SSAS Tabular security semantics this plan honors

Inlined from research so the executor does not need to look it up:

1. **Role shape**: role = name + `modelPermission` (none/read/readRefresh/
   refresh/administrator) + members (Windows users/groups) + `tablePermissions[]`
   (per-table `filterExpression` DAX + optional `metadataPermission`/
   `columnPermissions`). Row filters only for Read or Read-and-Process roles.
2. **RLS**: DAX TRUE/FALSE per row of target table. `=FALSE()` denies all rows.
   No filter on a table in a Read role = full access to that table. Filters
   cascade through active relationships in many-direction (dim filtered ->
   fact auto-filtered). Measures evaluated under filtered row set automatically.
3. **Multiple roles = UNION (OR) semantics**, not intersection. Permission
   levels unioned (most permissive wins; None+Read=Read). Row filters across
   different tables unioned. No deny permission in Tabular.
4. **Dimension/member security**: Tabular has NO AllowedSet/DeniedSet. Member
   restriction = row filter on dimension table. MDSCHEMA_MEMBERS Discover should
   respect row filters under secured role.
5. **OLS (Object-Level Security, 1400+ SL)**: `tablePermissions[].metadataPermission:
   none` hides table entirely (data + metadata). `columnPermissions[].metadataPermission:
   none` hides column; measures referencing secured column become invisible.
   Hidden objects return error "column cannot be found", not NULL.
6. **No role = deny all** (empty result/error, no cubes in Discover). No
   implicit default role. Server admins bypass RLS/OLS entirely.
7. **HTTP XMLA auth**: Excel/MSOLAP over HTTP uses IIS + msmdpump.dll. IIS
   authenticates (Windows auth/Basic), impersonates, forwards to SSAS via TCP.
   Auth methods: Negotiate (SPNEGO: Kerberos preferred, NTLM fallback), Kerberos
   (constrained delegation, SPN required), NTLM (single-hop only), Basic
   (cleartext, needs SSL), Anonymous. SPN: `MSOLAPSvc.3/<hostname>:<instancename>`.

## Scope of SSAS semantics implemented in this plan

**In scope (implemented)**:
- `UserContext` struct threaded from auth boundary through plan/SQL/discover.
- `RoleConfig` extended with `model_permission`, `members`, and
  `table_permissions[]` carrying a SQL-based `filter_expression` (not DAX).
- RLS via SQL predicates: role filter on a table becomes an extra WHERE
  predicate on every scan of that table and cascades to fact tables via active
  relationships.
- Model-level permission: `none` = deny all (empty results, no cubes in
  Discover), `read` = subject to RLS, `administrator` = bypass RLS.
- Measure visibility per role (hide measures whose fact table or referenced
  dimension is OLS-secured).
- MDSCHEMA_MEMBERS Discover respects RLS on dimension tables.
- Multiple roles = union (OR) of row filters across tables; most permissive
  model_permission wins.
- No role = deny all.
- Qualify: enforced roles (with table_permissions + members) no longer
  automatically PARTIAL; only unsupported role shapes are.
- Trusted-identity auth boundary: `X-User` header from a configured trusted
  proxy, with a config gate that refuses to enable it unless
  `auth.trusted_proxy = true`.

**Out of scope (deferred)**:
- Native Kerberos/SPNEGO/NTLM in Rust. The plan defines the boundary; a
  reverse proxy (IIS/nginx) handles actual Windows Auth and forwards
  `X-User`. Native Kerberos is a follow-up plan.
- DAX `filterExpression` parsing. This plan accepts SQL fragments in
  `filter_expression`, not DAX. The converter may emit SQL fragments directly
  or leave `filter_expression` empty for manual fill-in. A future plan can add
  a DAX-to-SQL lowering pass.
- `USERELATIONSHIP()` blocking under RLS (hard block in SSAS). The proxy does
  not implement inactive-relationship activation today, so this is moot.
- Cell security (Multidimensional only, not in Tabular).
- `readRefresh`/`refresh` permission distinction (proxy is read-only runtime).
- Dynamic RLS functions (`USERNAME()`, `USERPRINCIPALNAME()`, `CUSTOMDATA()`).
  A follow-up can wire `USERNAME()` to the authenticated user id.
- OLS `columnPermissions` (column-level hide). This plan implements
  table-level OLS only. Column-level is a follow-up.
- Malloy runtime role enforcement. Roles enforce on direct-SQL path only.
  `MALLOY_RUNTIME=1` will not enforce roles in this plan; document that gap.
- Many-to-many security bridge tables / bi-directional cross-filter. Single
  active relationship direction only.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Build binary | `cargo build --bin xmla_proxy` | exit 0 |
| Targeted role tests | `cargo test --lib role_` | all new role tests pass |
| Full library tests | `cargo test --lib` | all tests pass |
| Qualify enforced-role project | `cargo run --bin xmla_proxy -- qualify <config with enforced roles>` | READY or PARTIAL, not BLOCKED for role-only reasons |

## Scope

**In scope** (the only files you should modify):

- `src/project/config.rs` — extend `RoleConfig`, add `TablePermissionConfig`,
  `ModelPermission`, `AuthConfig`.
- `src/engine/model.rs` — add `UserContext`, role resolution, effective
  permission/filter computation.
- `src/engine/plan.rs` — thread `UserContext` into plan generation; inject role
  filters.
- `src/engine/sql.rs` — emit role predicates in WHERE clauses.
- `src/main.rs` — parse trusted identity header, build `UserContext`, thread
  into `route_request`.
- `src/xmla/discover/members.rs` — apply role filters to member enumeration.
- `src/tools/qualify.rs` — update role verdict logic.
- `src/tools/convert_tabular.rs` — emit extended `RoleConfig` fields from
  Tabular `.bim`/TMDL `tablePermissions` (SQL fragments, not DAX lowering).
- New test file or inline tests in the above modules.

**Out of scope** (do NOT touch, even though they look related):

- `src/engine/malloy.rs`, `src/engine/malloy_node*.rs` — Malloy runtime
  enforcement is a follow-up.
- `js/malloy-worker.js` — no change.
- `src/backend/mod.rs` — no change to backend/connection logic.
- `src/xmla/parser.rs`, `src/xmla/response.rs` — no XMLA protocol changes
  beyond what is needed to thread `UserContext` (the hardcoded `SessionId`
  remains a separate follow-up).
- Any converter DAX-to-SQL lowering. The converter emits SQL fragments or
  empty `filter_expression`; it does not translate DAX.

## Git workflow

- Branch: `advisor/026-security-roles`
- Commit message style in recent history is short plain phrases (examples:
  `progress correctness`, `single-binary refactor`). Use a concise imperative
  phrase that matches that simplicity.
- Do NOT push or open a PR unless the operator explicitly asks for it.

## Steps

### Step 1: Extend `RoleConfig` and add `AuthConfig`

In `src/project/config.rs`:

- Add `enum ModelPermission { None, Read, Administrator }` with serde
  rename to lowercase strings (`none`, `read`, `administrator`). Default `Read`
  if missing for backward compat.
- Add `struct TablePermissionConfig { table: String, filter_expression: String,
  #[serde(default)] metadata_permission: ModelPermission }` where
  `metadata_permission: None` means OLS-hide the table.
- Extend `RoleConfig` to:
  ```rust
  pub struct RoleConfig {
      pub name: String,
      #[serde(default)]
      pub description: String,
      #[serde(default = "default_read")]
      pub model_permission: ModelPermission,
      #[serde(default)]
      pub members: Vec<RoleMemberConfig>,
      #[serde(default)]
      pub table_permissions: Vec<TablePermissionConfig>,
  }
  ```
- Add `struct RoleMemberConfig { member_name: String, #[serde(default)]
  member_type: String }` (values: `user`, `group`).
- Add `struct AuthConfig { #[serde(default)] trusted_proxy: bool,
  #[serde(default)] trusted_header: String }` with default
  `trusted_header = "X-User"`. Add `auth: Option<AuthConfig>` to `ProxyConfig`
  (default `None` = no auth, deny-all behavior).
- Keep backward compat: existing configs with only `{name, description}` still
  parse (model_permission defaults to Read, members/table_permissions empty).

**Verify**: `cargo build --lib` -> exit 0. Existing configs still parse.

### Step 2: Add `UserContext` and role resolution in `src/engine/model.rs`

Add:

```rust
pub struct UserContext {
    pub user_id: String,        // e.g. "DOMAIN\\user" from trusted header
    pub groups: Vec<String>,    // resolved group memberships (from header or config)
    pub roles: Vec<String>,     // resolved role names this user belongs to
    pub is_administrator: bool, // bypass RLS/OLS
}
```

Add a function `resolve_user_context(config: &ProxyConfig, user_id: &str,
groups: &[String]) -> UserContext` that:
- Matches `user_id` and each group against `RoleConfig.members[].member_name`.
- Collects all matching role names.
- Sets `is_administrator = true` if any matched role has
  `model_permission: Administrator`.
- If no roles match and no `auth` config is present, return a context with
  `is_administrator = true` (backward compat: no auth = full access, matching
  current behavior). If `auth` is present but no roles match, return a context
  with empty roles and `is_administrator = false` (deny all).

Add `effective_table_filter(model, user, table_name) -> Option<String>` that:
- Returns `None` if `user.is_administrator` (no filter).
- Collects `filter_expression` from all matched roles for that table.
- If any matched role has no `table_permission` for that table, return `None`
  (full access for that table per SSAS "no filter = full access").
- If all matched roles have `filter_expression` for that table, OR them
  together with `OR` (union semantics across roles).
- If any matched role has `metadata_permission: None` for that table, return
  `Some("__OLS_HIDDEN__")` sentinel — caller hides the table entirely.

Add `effective_model_permission(model, user) -> ModelPermission` that returns
the most permissive permission across matched roles (Administrator > Read >
None).

**Verify**: `cargo build --lib` -> exit 0. Add unit tests for
`resolve_user_context` and `effective_table_filter` covering: no auth config
(admin), admin role bypass, single role single table, multiple roles union,
OLS hide, no matching role deny-all.

### Step 3: Thread `UserContext` through `route_request` and plan generation

In `src/main.rs`:
- Add `user_context: UserContext` to `AppState` (built once at startup for the
  no-auth backward-compat case; for auth, built per-request from the trusted
  header).
- In `handle_xmla`, after parsing, if `auth.trusted_proxy` is true, read the
  trusted header (`X-User` by default) and build a per-request `UserContext`.
  If the header is missing, return a 401-style empty response (deny closed).
- Pass `&UserContext` into `route_request` and through to execute/discover
  paths.

In `src/engine/plan.rs`:
- Add `user: &UserContext` parameter to `plan_from_semantic_with_model()` and
  `execute_plan_with_backend()`.
- In `plan_from_semantic_with_model()`, after building `TypedDimensionFilter[]`,
  inject role filters: for each fact table in the plan, call
  `effective_table_filter(model, user, fact_table_name)`. If it returns
  `Some("__OLS_HIDDEN__")`, return `QueryPlan::Empty`. If it returns
  `Some(sql)`, add a synthetic `TypedDimensionFilter` variant or append the
  raw SQL to the WHERE clause in `sql.rs` (see Step 4).
- If `effective_model_permission(model, user) == None`, return `QueryPlan::Empty`
  for all queries.

Keep legacy wrappers (`execute_plan()`, `execute_plan_with_sql()`) working by
passing a default admin `UserContext` (backward compat for tests/benchmarks
that do not pass auth).

**Verify**: `cargo build --bin xmla_proxy` -> exit 0. `cargo test --lib` ->
all existing tests pass (they use the admin-default legacy path).

### Step 4: Emit role predicates in SQL

In `src/engine/sql.rs`:
- Add a `role_predicates: Vec<String>` parameter (or a `UserContext` param) to
  `sql_for_query_plan()`, `joins_and_where()`, and `sql_where_with_cols()`.
- For each fact table in the plan, call `effective_table_filter(model, user,
  fact_table)` and collect non-empty, non-sentinel results as raw SQL
  predicates. Append them to the WHERE clause with `AND`.
- For dimension-table scans (Count plan, member discovery), apply the
  dimension table's role filter the same way.
- The sentinel `__OLS_HIDDEN__` is handled in plan generation (returns Empty),
  so SQL emission never sees it.

Example target SQL shape:

```sql
-- Without roles:
SELECT SUM(revenue) FROM sales_fact f
-- With role filter on dim_territory cascading to sales_fact:
SELECT SUM(revenue) FROM sales_fact f
 JOIN dim_territory _territory ON f.territory_key = _territory.territory_key
 WHERE _territory.region = 'EU'
-- With direct role filter on sales_fact:
SELECT SUM(revenue) FROM sales_fact f
 WHERE f.region = 'EU'
```

Role filter SQL fragments are emitted verbatim from `filter_expression`. The
converter/operator is responsible for producing valid DuckDB SQL fragments
referencing the correct table alias (`f` for fact table, `_<dim_id>` for
joined dimension tables). Document this contract in `README.md`.

**Verify**: `cargo test --lib role_` -> new SQL emission tests pass. Add
tests covering: no roles (no extra WHERE), admin bypass, single role filter on
fact table, single role filter on dimension table (cascades via JOIN), OLS
hide returns Empty, multiple roles union (OR).

### Step 5: Apply role filters to MDSCHEMA_MEMBERS Discover

In `src/xmla/discover/members.rs`:
- Thread `UserContext` into `get_members_response_with_backend()` and
  `build_leaf_member_rows()` / `build_all_member_rows()`.
- In `build_leaf_member_rows()`, if `effective_table_filter(model, user,
  dim_table)` returns `Some(sql)`, append `WHERE {sql}` to the
  `SELECT DISTINCT ... FROM {dim_table}` query. If it returns the OLS sentinel,
  return an empty member list.
- In `build_all_member_rows()`, if the dimension's table is OLS-hidden, return
  empty.

**Verify**: `cargo test --lib role_` -> discover member tests pass. Add a
test proving a user with a role filter on `dim_segment` sees only filtered
members in MDSCHEMA_MEMBERS.

### Step 6: Update `qualify` to distinguish enforced vs informational roles

In `src/tools/qualify.rs`:
- Replace the blanket "N unsupported security role(s)" PARTIAL with logic:
  - If a role has non-empty `table_permissions` with non-empty
    `filter_expression`, it is **enforced** — do not flag as PARTIAL.
  - If a role has `model_permission: Administrator`, it is enforced — do not
    flag.
  - If a role has `members` but no `table_permissions`, flag PARTIAL with
    "role 'X' has members but no table permissions (no RLS enforced)".
  - If a role has `table_permissions` with empty `filter_expression` and
    `metadata_permission: Read`, flag PARTIAL with "role 'X' table 'Y' has no
    filter (full access)".
  - If `auth` is absent but `roles` are present, flag PARTIAL with "roles
    defined but no auth config — roles will not be enforced at runtime".
- Update tests: `generated_project` should move from PARTIAL (roles) to either
  READY (if roles are enforced) or PARTIAL with a more specific reason.

**Verify**: `cargo run --bin xmla_proxy -- qualify generated_project/proxy-config.json`
-> verdict is READY or PARTIAL with a role-specific reason, not the blanket
"unsupported security role(s)". `cargo test --lib` -> all pass.

### Step 7: Converter emits extended role metadata

In `src/tools/convert_tabular.rs`:
- When reading Tabular `.bim`/TMDL roles, emit the extended `RoleConfig`
  fields: `model_permission`, `members` (from `memberName`/`memberType`),
  `table_permissions` (from `filterExpression` and `metadataPermission`).
- For `filterExpression`: emit the raw DAX expression in a `dax_filter`
  field (new, optional) and leave `filter_expression` (SQL) empty with a
  conversion-report note "role 'X' table 'Y' filter needs manual SQL
  translation from DAX". Do NOT attempt DAX-to-SQL lowering.
- For `metadataPermission: none`, emit `metadata_permission: "none"` in the
  table permission.
- Update `conversion-report.md` to list roles with their enforcement status
  (enforced if SQL filter present, needs-manual if DAX-only, OLS if
  metadata_permission none).

**Verify**: `cargo build --bin xmla_proxy` -> exit 0. Run converter on a
fixture `.bim` with roles (or add a small test fixture) and verify
`proxy-config.json` contains the extended role fields. `cargo test --lib` ->
all pass.

### Step 8: Document the auth boundary and role contract

In `README.md`, add a "Security and roles" section documenting:
- The trusted-proxy auth boundary: set `auth.trusted_proxy = true` and
  `auth.trusted_header = "X-User"` in config; put IIS/nginx in front with
  Windows Auth; the proxy trusts the header only when `trusted_proxy = true`.
- The role contract: `filter_expression` is a DuckDB SQL fragment; fact table
  alias is `f`; dimension table alias is `_<dim_id>`; fragments are OR'd across
  roles (union); `metadata_permission: none` hides the table.
- What is enforced (RLS via SQL, model-level read/deny, measure visibility,
  MDSCHEMA_MEMBERS filtering) and what is not (Malloy runtime, DAX lowering,
  column-level OLS, dynamic USERNAME(), native Kerberos).

**Verify**: `cargo build --bin xmla_proxy` -> exit 0. Visual review of the
new README section.

## Test plan

New tests, all named with `role_` prefix so `cargo test --lib role_` is a
stable targeted command:

- `src/engine/model.rs`:
  - `role_resolve_no_auth_is_admin` — no auth config -> admin context (backward
    compat).
  - `role_resolve_admin_bypass` — admin role -> is_administrator true.
  - `role_resolve_single_role_single_table` — one role, one table filter.
  - `role_resolve_multiple_roles_union` — two roles, OR'd filters.
  - `role_resolve_ols_hide` — metadata_permission none -> sentinel.
  - `role_resolve_no_matching_role_deny_all` — auth present, no role match ->
    empty roles, not admin.
- `src/engine/sql.rs`:
  - `role_sql_no_filter` — admin user, no extra WHERE.
  - `role_sql_fact_table_filter` — filter on fact table appended to WHERE.
  - `role_sql_dim_table_filter_cascades` — filter on dim table joins and
    filters.
  - `role_sql_ols_returns_empty` — OLS sentinel -> QueryPlan::Empty -> empty
    SQL.
- `src/xmla/discover/members.rs`:
  - `role_discover_filters_members` — user with dim filter sees only allowed
    members.
  - `role_discover_ols_hides_members` — OLS-hidden dim returns empty member
    list.
- `src/tools/qualify.rs`:
  - `role_qualify_enforced_role_ready` — role with SQL filter -> not PARTIAL
    for role reason.
  - `role_qualify_role_without_auth_partial` — roles present, no auth config
    -> PARTIAL with specific reason.

Use existing inline `#[cfg(test)]` module pattern. Model new role tests on
existing `src/engine/sql.rs` tests (line 175+) for SQL emission shape and
`src/engine/model.rs` tests for model resolution.

Verification: `cargo test --lib role_` -> all new tests pass.
`cargo test --lib` -> all tests pass (existing tests use admin-default
backward-compat path).

## Done criteria

- [ ] `cargo build --bin xmla_proxy` exits 0
- [ ] `cargo test --lib role_` exits 0 with all new role tests
- [ ] `cargo test --lib` exits 0 (existing tests pass via admin-default path)
- [ ] `RoleConfig` carries `model_permission`, `members`, `table_permissions`
- [ ] `UserContext` is threaded from `handle_xmla` through `route_request` to
      plan/SQL/discover
- [ ] RLS filters emit as WHERE predicates on direct-SQL path
- [ ] OLS `metadata_permission: none` hides tables (Empty plan, empty members)
- [ ] No matching role under auth config = deny all (Empty plan)
- [ ] `qualify` no longer blanket-flags enforced roles as PARTIAL
- [ ] Converter emits extended role metadata with DAX-in-`dax_filter`,
      SQL-`filter_expression` empty, conversion-report note
- [ ] `README.md` documents the trusted-proxy auth boundary and role SQL
      contract
- [ ] `plans/README.md` status row updated

## STOP conditions

- The current in-scope code no longer matches the excerpts above (drift since
  `46eeb39`).
- Backward compat cannot be preserved without breaking existing configs (e.g.
  serde default for `model_permission` does not work).
- Thread-safe `UserContext` threading requires changing `XmlaRequest` or
  `route_request` signatures in a way that breaks Plan 025's concurrency
  path.
- The converter's `.bim`/TMDL role reader does not expose
  `tablePermissions`/`metadataPermission` in a way that maps to the new
  config fields (would require converter parser changes beyond this plan's
  scope).
- A step's verification fails twice after a reasonable fix attempt.
- The executor concludes that role enforcement requires Malloy runtime
  changes (it should not — direct SQL only).

## Maintenance notes

- **Malloy runtime gap**: this plan enforces roles on direct SQL only. If
  `MALLOY_RUNTIME=1` is enabled, role filters will NOT be applied to Malloy-
  compiled queries. This is a known gap; document it in README and flag in
  `qualify` when both `MALLOY_RUNTIME` and roles are configured. A follow-up
  plan should either inject role predicates into Malloy query sources or
  disable Malloy runtime when roles are active.
- **DAX lowering gap**: `filter_expression` is SQL, not DAX. Real Tabular
  models ship DAX `filterExpression`. The converter emits DAX in `dax_filter`
  and leaves SQL empty. Operators must manually translate. A future plan can
  add a DAX-to-SQL lowering pass for simple patterns (`[Col] = "value"`,
  `[Col] IN (...)`, `FALSE()`).
- **Native Kerberos**: this plan defines the trusted-header boundary but does
  not implement SPNEGO/Kerberos in Rust. A reverse proxy (IIS with Windows
  Auth, or nginx with a Kerberos module) must terminate auth and set the
  trusted header. A follow-up plan can evaluate native Kerberos via a Rust
  crate if single-binary Linux deployment becomes a hard requirement.
- **Reviewer scrutiny**: focus on (1) the deny-all path when auth is
  configured but no role matches — this must not accidentally grant admin,
  (2) the union/OR semantics across roles — must not intersect, (3) the OLS
  sentinel propagation — must reach Empty plan and empty members, not leak
  through, (4) backward compat — existing configs without `auth` must keep
  working as admin.
- **Future: dynamic RLS** (`USERNAME()`): a follow-up can substitute
  `${USER}` or `${USERNAME}` in `filter_expression` with the authenticated
  user id at plan time. Not in this plan.
- **Future: column-level OLS**: `columnPermissions` is deferred. Table-level
  OLS only in this plan.