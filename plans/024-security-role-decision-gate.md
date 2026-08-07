# Plan 024: Security-role decision gate

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c89764f..HEAD -- src/tools/convert_tabular.rs src/tools/qualify.rs src/project/config.rs src/project/project.rs generated_project/conversion-report.md generated_project/proxy-config.json`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: `plans/022-genericize-converter-fallback-lowering.md`
- **Category**: direction
- **Planned at**: commit `c89764f`, 2026-06-17

## Why this matters

`generated_project` is the repo's most customer-shaped converted model. It
qualifies as `PARTIAL` only because of unsupported security roles. Every real
enterprise SSAS model will have security roles, so this is the biggest honest
blocker to claiming "3 real models end-to-end."

This plan does **not** attempt full role enforcement. Instead, it makes roles
machine-readable, gives operators an explicit decision gate, and graduates
`generated_project` from `PARTIAL` to `READY` with a documented caveat.

## Current state

The converter already parses roles from Tabular Editor exports:

```rust
// src/tools/convert_tabular.rs:50-53
struct RoleInfo {
    name: String,
    description: String,
}

// src/tools/convert_tabular.rs:323-338
fn parse_roles(dir: &str) -> Vec<RoleInfo> {
    // reads JSON files from roles/ directory
    // extracts name and description
}
```

But roles only appear in the conversion report as markdown text:

```md
// generated_project/conversion-report.md:116-122
## Roles

Security roles detected but NOT supported by the proxy:

- fys_läsbehörighet: Läsbehörighet för fys-data

Must be enforced outside the proxy if needed.
```

The `qualify` command detects roles by parsing that markdown:

```rust
// src/tools/qualify.rs:96-114
// --- check roles from sibling conversion-report.md ---
let report_path = Path::new(config_path).parent()
    .map(|d| d.join("conversion-report.md"));
if let Some(report_path) = report_path {
    if let Ok(text) = fs::read_to_string(&report_path) {
        if let Some(roles_section) = text.split("## Roles").nth(1) {
            let role_lines: Vec<&str> = roles_section.trim().lines()
                .filter(|l| l.starts_with("- ") && l.contains(":"))
                .collect();
            if !role_lines.is_empty() {
                partial.push(format!(
                    "{} unsupported security role(s) detected in conversion report",
                    role_lines.len()
                ));
            }
        }
    }
}
```

The `proxy-config.json` schema (`src/project/config.rs`) has no role-related
fields.

Repo conventions to match:

- Fail closed when semantics are unknown.
- Machine-readable config is better than markdown parsing.
- The `qualify` command is the readiness gate.
- "Must be enforced outside the proxy" is an acceptable outcome for roles —
  the proxy does not need to become a full security engine.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Build CLI | `cargo build --bin xmla_proxy` | exit 0 |
| Qualify generated_project | `cargo run --bin xmla_proxy -- qualify generated_project/proxy-config.json` | `READY` with role caveat |
| Qualify retail | `cargo run --bin xmla_proxy -- qualify generated_retail_analytics/proxy-config.json` | `READY` (no roles) |
| Full tests | `cargo test --lib` | all pass |

## Scope

**In scope**:
- `src/tools/convert_tabular.rs` — emit roles in `proxy-config.json`
- `src/project/config.rs` — add `roles` field to `ProxyConfig`
- `src/project/project.rs` — load roles into the model
- `src/tools/qualify.rs` — read roles from config, not markdown
- `generated_project/` — regenerate or patch config to include roles
- `generated_retail_analytics/` — regenerate (should have no roles)

**Out of scope**:
- runtime role enforcement (row-level security, filter-based access control)
- role-based measure hiding
- authentication / user identity
- the actual SSAS role permission model (table filters, DAX row filters)

## Steps

### Step 1: Add role metadata to the config schema

Add an optional `roles` field to `ProxyConfig` in `src/project/config.rs`:

```rust
#[derive(Deserialize, Debug, Clone, Default)]
pub struct RoleConfig {
    pub name: String,
    pub description: String,
    pub enforced: bool,  // false = documented but not enforced by proxy
}
```

Add `#[serde(default)] pub roles: Vec<RoleConfig>` to `ProxyConfig`.

The `enforced` field defaults to `false`. This is the honest contract: the
proxy knows about roles but does not enforce them.

**Verify**: `cargo build --lib` -> exit 0.

### Step 2: Emit roles in converter output

Update `render_proxy_config()` in `src/tools/convert_tabular.rs` to emit a
`"roles"` array in the JSON output:

```json
"roles": [
    { "name": "fys_läsbehörighet", "description": "Läsbehörighet för fys-data", "enforced": false }
]
```

Also update the conversion report to reference the config field instead of
being the sole source of role information.

**Verify**: `cargo run --bin xmla_proxy -- convert-tabular data/retailanalytics_tabular /tmp/opencode/roles-024` -> exit 0. Grep the output config for `"roles"`.

### Step 3: Update `qualify` to read roles from config

Replace the markdown-parsing role detection in `src/tools/qualify.rs` with
config-based detection:

- If `p.config.roles` is non-empty and all have `enforced: false`:
  - Report as `PARTIAL` with message: `"N documented role(s) not enforced by proxy"`
- If any role has `enforced: true`:
  - Report as `BLOCKED` with message: `"role enforcement is not implemented"`
  - (This should not happen today since the converter never sets `enforced: true`,
    but it future-proofs the gate.)
- If `p.config.roles` is empty:
  - No role-related issues.

**Verify**: `cargo run --bin xmla_proxy -- qualify generated_retail_analytics/proxy-config.json` -> `READY` (no roles).

### Step 4: Update generated_project to include roles in config

Either regenerate `generated_project/` from source (if available) or manually
patch `generated_project/proxy-config.json` to include the roles array based
on the conversion report.

**Verify**: `cargo run --bin xmla_proxy -- qualify generated_project/proxy-config.json` -> `PARTIAL` with message about unenforced roles (not `BLOCKED`).

### Step 5: Decide the final qualification verdict for role-bearing projects

This is the key design decision: should a project with documented-but-unenforced
roles qualify as `READY` or `PARTIAL`?

Recommended: **`READY` with an informational note**, not `PARTIAL`.

Rationale:
- The proxy honestly does not enforce roles.
- Most SSAS proxy use cases are internal/trusted networks where role
  enforcement is handled at the data layer (DuckDB views) or by the SSAS
  server being replaced.
- Marking every role-bearing model as perpetually `PARTIAL` would make the
  `READY` verdict unreachable for any real enterprise model.

If the user disagrees, keep it as `PARTIAL` and document the rationale.

**Verify**: `cargo run --bin xmla_proxy -- qualify generated_project/proxy-config.json` -> `READY` (or `PARTIAL` if the user chose that) with a clear message about roles.

### Step 6: Add tests and run full suite

Add tests in `src/tools/qualify.rs`:
- A project with no roles qualifies `READY`.
- A project with unenforced roles qualifies `READY` (or `PARTIAL`) with a note.
- A project with `enforced: true` roles qualifies `BLOCKED`.

Add a test in `src/project/project.rs` that loads a config with roles and
verifies they are parsed correctly.

**Verify**: `cargo test --lib` -> all pass.

## Test plan

- Config deserialization test for `roles` field with and without roles.
- Qualify tests for no-roles, unenforced-roles, and enforced-roles cases.
- Verify `generated_project` qualifies with the expected verdict.
- Verify `generated_retail_analytics` still qualifies `READY`.

## Done criteria

- [ ] `cargo build --bin xmla_proxy` exits 0
- [ ] `cargo test --lib` exits 0
- [ ] `ProxyConfig` has a `roles` field with `RoleConfig` struct
- [ ] The converter emits roles in `proxy-config.json`
- [ ] `qualify` reads roles from config, not from markdown
- [ ] `generated_project/proxy-config.json` includes roles
- [ ] `generated_project` qualification verdict is `READY` (or `PARTIAL` with
      a clear role message, per the design decision in Step 5)
- [ ] `generated_retail_analytics` still qualifies `READY`
- [ ] `plans/README.md` status row updated

## STOP conditions

- Adding the `roles` field to `ProxyConfig` breaks deserialization of existing
  configs in a way that cannot be fixed with `#[serde(default)]`.
- The generated_project source export is not available and the config cannot
  be manually patched (unlikely — it's a JSON file).
- The user decides that unenforced roles must always be `BLOCKED`, which would
  make this plan a no-op (the proxy would never be `READY` for enterprise
  models).

## Maintenance notes

- This plan makes roles machine-readable but does not enforce them.
- If real runtime role enforcement is needed later, the `enforced: true` path
  is already wired as a `BLOCKED` gate.
- The conversion report should no longer be the sole source of role
  information — config is authoritative.
- Reviewers should verify that the `enforced: false` contract is documented
  clearly enough that operators understand the proxy does not do row-level
  security.
