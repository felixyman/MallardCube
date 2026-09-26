# Contributing to MallardCube

Thanks for helping! MallardCube is the Excel/XMLA compatibility layer for
DuckDB: Excel connects to it as a SSAS data source and gets PivotTable
behaviour against DuckDB data.

Two principles keep the project coherent:

1. **Excel is the specification.** Compatibility is judged by what real Excel
   (MSOLAP/ADOMD) accepts, not by what looks reasonable on paper. When in
   doubt, capture the request/response pair and compare with SSAS conventions.
2. **Direct SQL is the only runtime path.** `src/engine/sql.rs` stays
   DuckDB-dialect only; there is no second dialect.
3. **No measure logic in the proxy.** Definitions and materialisation live
   upstream (sqlmesh/dbt models, or a semantic layer); the proxy serves a thin
   projection. `cargo run --bin mallard -- qualify --strict <config>` enforces
   it, and `docs/DESIGN-INVARIANTS.md` has the recipe and the invariants.

## Development setup

```bash
cargo build
cargo run                       # demo project on http://localhost:8080/xmla
```

Requirements: Rust — the version is pinned by `rust-toolchain.toml` (rustup
installs it automatically on the first `cargo` invocation). Bump the pin
deliberately: update `rust-toolchain.toml`, the `dtolnay/rust-toolchain@…`
refs in `.github/workflows/`, and the Dockerfile's `rust:<version>-bookworm`
base together. DuckDB is compiled in — no external services. Excel on Windows
is only needed for end-to-end checks.

## Tests

Run the suite:

```bash
cargo test --lib
```

Tests that read the converted projects (`projects/generated_*`) build their
DuckDB fixtures on demand from tracked sources; `cargo run --bin
seed_projects_db` regenerates them explicitly. The suite is expected to be
green and **date-relative** —
tests that depend on "today" (time intelligence, demo dates) must derive
their expectations from the seeded data, never hardcode them.

Before opening a PR:

```bash
cargo fmt --check
cargo clippy --lib --release -- -D warnings
cargo test --lib
```

If the change touches query execution, run the server-side smoke test too:

```bash
bash scripts/proxy-smoke.sh serve   # starts the proxy with tracing
bash scripts/proxy-smoke.sh         # 8 assertions against projects/project3
```

## Excel end-to-end checks

For changes that affect what Excel sees (discover rowsets, cellsets, axes,
members, properties), verify in real Excel, not just with curl. The
`.agents/skills/proxy-excel-test/SKILL.md` procedure drives Excel through the
Excel MCP server and is the reference for that loop; it also explains how to
capture `xmla-trace.jsonl`.

Compatibility bugs must come with a trace. `XMLA_TRACE=1 cargo run` writes
NDJSON (`xmla-trace.jsonl`) containing every request and response — the MDX
Excel sent and the cellset returned. That is usually enough to reproduce
without the workbook.

## Reporting bugs

Use the issue template and include:

- Excel version/build and the exact pivot action (or the MDX),
- the relevant `xmla-trace.jsonl` request/response pair,
- expected vs actual behaviour,
- the raw SQL expectation if the correct result is in question.

## Plans and docs

Non-trivial work starts as a plan: add `plans/NNN-short-title.md`, register it
in the `plans/README.md` index, and link the plan from your PR. Behaviour
changes should update `README.md` (and `CONTEXT.md` for session-level notes);
architecture changes belong in `docs/DEVELOPER-GUIDE.md`.

## Commits and PRs

- Conventional-commit subjects (`feat:`, `fix:`, `docs:`, `packaging:`, ...),
  lowercase, no trailing period.
- Keep a PR focused; explain the user-visible behaviour change and how you
  verified it (tests, smoke, Excel).
