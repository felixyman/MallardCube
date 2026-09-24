# parity — recorded reference behaviour, and the VM scripts that capture it

Two halves of one idea: **capture on the VM, assert everywhere**.

## Assert (runs anywhere, no VM needed)

| gate | what it covers |
|---|---|
| `cargo test --lib` | unit level: parser, plans, renderers, member rows |
| `bash scripts/probe-fidelity.sh [url]` | 11 behaviour probes for the silent-wrong-answer class (CDATA, entities, nested restrictions, empty statements) |
| `bash scripts/probe-parity.sh [url]` | replays `parity/catalog.json` against a proxy and compares against observations recorded from the real SSAS 2025 tabular mirror |
| `bash scripts/proxy-smoke.sh [url]` | 8 end-to-end smoke assertions |

**Adding a case.** Whenever a review or a VM session finds a proxy/reference
difference, add it to `parity/catalog.json`: the request, and the **mirror's**
value (not the proxy's). Prefer stable observations — see the drift notes.
`expect` is **always the reference's value**. A case may carry a `known_gap`
string: the mismatch is then reported as `KNOWN` (with both values and the
note) instead of failing the run. When the gap is fixed the case prints
`PASS … no longer reproduces`, which is the signal to drop `known_gap`.

## Capture (VM, `C:\Users\Public\Documents\parity\`)

- `vm-ready.ps1` — idempotent environment setup and health check: starts the
  8090 listener in front of the HTTPS pump, clears a stale WinINET proxy
  (which makes MSOLAP and Excel fail while `curl` keeps working), kills stale
  Excel, verifies SSAS/IIS/mirror/ADOMD, prints `VM READY` or `VM NOT READY`.
  Run it first in every VM session.
- `uia-probe.ps1` — dump a window's UI Automation tree (or list windows)
  instead of screenshotting. Shows each control's name, automation id, class,
  rect and enabled/offscreen flags.
- `uia-click.ps1` — click an element by name or automation id, foregrounding
  the window first (Office dialogs ignore clicks when inactive). `-DryRun`
  prints the target and clicks nothing; always dry-run a new sequence.
- `scripts/vm/sweep3.ps1`, `scripts/vm/sweep2.ps1` — the Excel COM sweeps, now
  versioned (with `vm-ready.ps1`, `uia-probe.ps1`, `uia-click.ps1` and the
  mirror-chain scripts; see `scripts/vm/README.md`).
- `scripts/vm/sweep-diff.ps1` — runs a sweep and diffs it against
  `parity/sweep*-baseline.txt`, failing on anything except the one known flaky
  `two_hier_same_dim` COM line.

## Drift notes

- Observations that depend on **fact rows** drift between the mirror snapshot
  and the proxy's regenerated demo data — the demo is anchored to "today", so
  the set of fact-bearing dates differs (the mirror's snapshot was taken days
  earlier). Prefer dimension-level counts, which are stable.
- Date member names/captions: the mirror emits locale short dates
  (`1/1/2020`) and unique names with a `T00:00:00` suffix; the proxy emits the
  raw ISO value (`2020-01-01`). Recorded as a deliberate open parity nit.
- The mirror writes `<Value>` as a double in scientific notation
  (`5.21586767E8`); the proxy writes `521586767`. The runner compares numbers
  numerically, so both match.

- Per-date revenue differs between the mirror snapshot and the proxy's demo:
  the same total is spread over a differently-anchored window, so only totals
  and member *sets* compare, not per-date values.
- The Excel date-filter subquery (recorded verbatim in the
  `ssas-reference-oracle` skill) is answered by the proxy with `(All)` plus the
  filtered date, value 135408. It is **not** in the catalog: under ADOMD the
  mirror's standalone filter matches the date (142336 — the drift above), but
  the full subquery returned an empty axis there, so a mirror-verified
  expectation still needs a live Excel session.
