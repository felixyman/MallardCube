# Plan 019: Make fallback capability detection conservative for scalar outer queries

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c89764f..HEAD -- src/engine/model.rs src/engine/plan.rs src/project/project.rs generated_project/proxy-config.json generated_project/sql_fallback/ src/execute/dispatch.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: `plans/015-converted-measure-execution-tests.md`
- **Category**: correctness
- **Planned at**: commit `c89764f`, 2026-06-17

## Why this matters

The fallback capability gate is only trustworthy if it reflects the outer result
shape the runtime will actually see. Right now, any fallback SQL containing a
`GROUP BY` anywhere is auto-classified as `Universal`, even when the outer query
still returns one scalar.

That overclaims grouped support and can make Excel queries look supported when
they are not.

## Current state

Relevant excerpts:

```rust
// src/engine/model.rs:336-344
if !upper.contains("GROUP BY") {
    return Some(FallbackCapability::ScalarOnly);
}
Some(FallbackCapability::Universal)
```

```sql
-- generated_project/sql_fallback/medeltid_undersökningsslut_till_signering_(ej_akut).sql
SELECT AVG(avg_per_remiss) AS value
FROM (
    SELECT remissnummer, AVG(...) AS avg_per_remiss
    FROM dw_fys_f_undersökning
    GROUP BY remissnummer
)
```

```rust
// src/engine/plan.rs
Some(FallbackCapability::Universal) => execute fallback for grouped plans as supported
```

Repo conventions to match:

- Fail closed when grouped support is uncertain.
- Prefer explicit capability metadata over heuristic overclaiming.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Build CLI | `cargo build --bin xmla_proxy` | exit 0 |
| Generated-project tests | `cargo test --lib generated_project` | all pass |
| Full tests | `cargo test --lib` | all pass |

## Scope

**In scope**:
- `src/engine/model.rs`
- `src/engine/plan.rs`
- explicit fallback capability metadata for checked-in converted projects if needed
- generated-project regression tests

**Out of scope**:
- unrelated converter translation work
- retail stub retirement itself

## Steps

### Step 1: Characterize the outer result shape contract

Use the new execution tests from plan 015 to pin down which generated-project
fallback measures are honestly scalar-only vs grouped-capable.

Do not start by making the heuristic more clever without a failing test.

**Verify**: `cargo build --bin xmla_proxy` -> exit 0.

### Step 2: Replace the broad `GROUP BY` heuristic with a conservative rule

Choose the narrowest safe contract:

- inspect the outer SQL result shape instead of substring-matching any nested
  `GROUP BY`, or
- require explicit `fallback_capability` metadata for grouped-capable converted
  fallbacks and make auto-classification default to `ScalarOnly` unless proven
  otherwise.

The end state should never classify a scalar outer query as `Universal` only
because an inner subquery groups rows.

**Verify**: add a regression proving `Medeltid Undersökningsslut till signering (ej akut)` is not treated as universally grouped-capable.

### Step 3: Update checked-in converted artifacts if needed

If the safest rule is explicit metadata, regenerate or patch the checked-in
converted config to carry that capability clearly.

**Verify**: `cargo test --lib generated_project` -> all pass.

## Test plan

- Add at least one grouped-shape regression for a scalar outer fallback.
- Preserve fail-closed behavior when grouped support is not explicit.

## Done criteria

- [ ] `cargo build --bin xmla_proxy` exits 0
- [ ] `cargo test --lib` exits 0
- [ ] Nested `GROUP BY` no longer implies universal grouped support by default
- [ ] Generated-project grouped fallback behavior is covered by regression tests
- [ ] `plans/README.md` status row updated

## STOP conditions

- There is no stable way to infer outer result shape without a real SQL parser.
- Tightening the heuristic would incorrectly downgrade already-proved grouped fallbacks.

## Maintenance notes

- Conservative under-support is acceptable here; overclaiming grouped support is not.
- If explicit metadata wins, document that pattern in the converted artifact contract.
