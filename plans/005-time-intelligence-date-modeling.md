# Plan 005: Add time-intelligence support through date-dimension data modeling

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat a1b1bd4..HEAD -- src/engine/model.rs src/engine/plan.rs src/engine/sql.rs src/project/config.rs src/mdx/parser.rs src/mdx/semantic.rs src/execute/`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: L (three independent phases, each M effort)
- **Risk**: MED — touches config schema, model, plan IR, and SQL emission
- **Depends on**: plans/003-MDX-semantics-on-ParsedMdx.md (parser cube-agnostic)
- **Category**: direction
- **Planned at**: commit `a1b1bd4`, 2026-06-16

## Why this matters

Time intelligence (YTD, prior year comparisons, period-over-period) is the
#1 missing capability blocking this proxy from replacing a real SSAS instance.
Every real-world SSAS cube has date dimensions with time-calculated measures.
Without this, the proxy can serve demo cubes but cannot handle any converted
production model.

The recommended approach (from `docs/ssas-to-malloy-conversion.md` Section 7)
pushes time complexity into **data modeling** rather than into the proxy
runtime: pre-build a `date_dim` table with boolean flag columns (`ytd_flag`,
`prior_year_flag`, etc.), then express time measures as simple filtered
aggregates (`revenue { where: date_dim.ytd_flag = true }`). This keeps the
proxy runtime simple and testable while still supporting all standard Excel
time-intelligence use cases.

## Current state

### Config schema (the gap)

`src/project/config.rs:45-64` — `DimensionConfig` has NO date-specific fields.
Date-role dimensions from `generated_project` are indistinguishable from
regular dimensions; the only distinguishing feature is `fact_table` binding.

```rust
// src/project/config.rs:45-64 — current DimensionConfig
pub struct DimensionConfig {
    pub id: String,
    pub malloy_name: String,
    pub physical_field: String,
    pub caption: String,
    // … display metadata …
    pub fact_table: Option<String>,  // only scope-binding field
    pub shared: bool,
}
```

`ProxyConfig` has no `time_intelligence` block, no `date_dimension` field,
no way to declare which dimension is the calendar/date dimension.

### Semantic model (the gap)

`src/engine/model.rs:56-80` — `DimensionDef` is entirely flat. No notion of
time role, no date hierarchy levels (Year/Quarter/Month/Day), no flag column
references.

```rust
// src/engine/model.rs:56-80 — current DimensionDef
pub struct DimensionDef {
    pub id: DimId,                     // String
    pub semantic_name: String,
    pub physical_field: String,
    pub table_name: Option<String>,
    // … caption, hierarchy_name, levels …
}
```

###  was model (raw data shape)

The generated project has a concrete date-table schema to model after:

`generated_project/schema.sql` — each date-role table has columns:
```
DateKey, FullDate, Year, Quarter, Month, DayOfMonth, DayOfWeek,
WeekOfYear, MonthName, DayName, QuarterName
```

These tables already exist in the DuckDB schema for `generated_project`.
The plan adds flag columns to this shape rather than inventing a new one.

### Query plan (the gap)

`src/engine/plan.rs:45-62` — `QueryPlan` has no time-related variants:
```rust
pub enum QueryPlan {
    Total { measure: MeasId, filters: Vec<DimensionFilter> },
    GroupBy { measure: MeasId, group_by: Vec<DimId>, filters: Vec<DimensionFilter> },
    Count { dimension: DimId },
    Empty,
}
```

### Data-model approach (the target)

Following the conversion docs, time measures become plain filtered measures
once the date-dimension table carries the right flag columns. Excel sends:

```mdx
SELECT { [Measures].[Revenue YTD] } ON COLUMNS
FROM [Sales]
WHERE ([Date].[Calendar].[Year].&[2024], [Measures].[Revenue YTD])
```

The proxy processes this as: resolve `Revenue YTD` → measure with Malloy
`revenue { where: date_dim.ytd_flag = true }` → emit SQL with `WHERE
sales_fact.date_key IN (SELECT date_key FROM date_dim WHERE ytd_flag = true)`.
No runtime time-awareness needed.

### What the converter already does

`src/bin/convert_tabular.rs` already:
- Classifies Kalender-prefixed tables as date-role (line 107–108)
- Generates relationship entries for date-role joins (lines 392–421)
- Generates empty Malloy `extend {}` sources for date-role tables (lines 785–791)
- Generates `join_one:` clauses (lines 753–761)
- Auto-generates cumulative YTD SQL using DuckDB `SUM(...) OVER (PARTITION BY ... ORDER BY ...)` for ALLSELECTED/ISONORAFTER patterns (lines 843–905)
- Does NOT generate the flag columns or seed data

### Repo conventions

- Model types use owned `String` for IDs: `pub type DimId = String`, `pub type MeasId = String` (`src/engine/model.rs:23-24`).
- Config deserializes via `#[derive(Deserialize)]` with `#[serde(default)]` for optional fields (`src/project/config.rs`).
- Tests use the pattern `ProxyProject::load("project3/proxy-config.json")` to load fixtures; see `src/project/project.rs:305-402` for the project3 test block.
- SQL emission uses format strings with lowercase identifiers; see `src/engine/sql.rs` for the existing SELECT/GROUP BY/WHERE pattern.
- The trace-replay harness is at `src/bin/trace_replay.rs`; use it to verify Excel compatibility as new capabilities are added.
- Naming contract: `docs/naming-contract.md` — `id` (internal), `malloy_name` (code), `caption` (Excel-visible).

## Commands you will need

| Purpose   | Command                  | Expected on success |
|-----------|--------------------------|---------------------|
| Build     | `cargo build --lib`      | exit 0, no errors   |
| Tests     | `cargo test --lib`       | all pass            |
| Specific  | `cargo test --lib date_dim` | all pass (after step 3) |
| Specific  | `cargo test --lib time_intelligence` | all pass (after step 5) |

## Scope

**In scope** (the only files you should modify):
- `src/project/config.rs` — add `DateDimensionConfig`, `TimeIntelligenceConfig`, reference from `DimensionConfig`
- `src/engine/model.rs` — add `DateDimDef` to `SemanticModel`, date-role marker on `DimensionDef`
- `src/engine/plan.rs` — no new variants; time measures use existing `Total`/`GroupBy` with column-set filters
- `src/engine/sql.rs` — add date-dim flag-column join emission when a filter references a date-role dimension
- `src/test_support/fixtures.rs` — add time-related MDX fixtures
- `src/execute/dispatch.rs` — add time-intelligence replay/oracle tests
- `project3/proxy-config.json` — optional: add a minimal date-dimension declaration for testing
- `data/seed_date_dim.sql` — new file: DuckDB SQL to create and populate a `date_dim` calendar table

**Out of scope** (do NOT touch):
- `src/mdx/parser.rs` — no YTD/ParallelPeriod function parsing needed yet;
  Excel sends measure references, not inline time functions on the axis.
  Excel time functions (if they ever appear) are a separate follow-up plan.
- `src/bin/convert_tabular.rs` — converter enhancements are deferred; this
  plan focuses on runtime support.
- `src/engine/malloy.rs` — Malloy emitter changes deferred; the SQL path
  is the focus.
- `generated_project/` — real SSAS model time intelligence is a separate
  follow-up plan; this plan uses project3 + synthetic date data.
- Malloy runtime path (`MALLOY_RUNTIME=1`) — time intelligence on the
  Malloy path is deferred.

## Steps

### Step 1: Add `DateDimensionConfig` and `TimeIntelligenceConfig` to config schema

In `src/project/config.rs`, add two new structs and reference them:

Add `DateDimensionConfig`:
```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DateDimensionConfig {
    /// Which dimension serves as the calendar/date dimension.
    pub dimension_id: String,
    /// The date-key column that joins to fact table date columns.
    pub date_key_column: String,
    /// The full-date column (DATE type) for flag computation.
    pub full_date_column: String,
    /// DuckDB table name for the date dimension (defaults to "date_dim").
    pub table_name: String,
    /// Columns that already exist or should be generated:
    /// year, quarter, month, ytd_flag, prior_year_flag.
    pub flag_columns: DateFlagColumns,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DateFlagColumns {
    pub year_column: String,
    pub quarter_column: String,
    pub month_column: String,
    pub ytd_flag_column: String,
    pub prior_year_flag_column: String,
}
```

Add a `time_intelligence` optional field to `ProxyConfig`:
```rust
pub time_intelligence: Option<TimeIntelligenceConfig>,

// New struct:
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TimeIntelligenceConfig {
    pub date_dimension: DateDimensionConfig,
}
```

Use `#[serde(default)]` on all new fields. Implement `Default` manually for
the flag-column structs to provide sensible defaults:
- `table_name`: `"date_dim"`
- `date_key_column`: `"date_key"`
- `full_date_column`: `"full_date"`
- `year_column`: `"year"`
- `quarter_column`: `"quarter"`
- `month_column`: `"month"`
- `ytd_flag_column`: `"ytd_flag"`
- `prior_year_flag_column`: `"prior_year_flag"`

Match the existing serde patterns in the file — field attributes are
`#[serde(default)]` for `Option` and `#[serde(default = "bool::default")]`
for booleans (see `DimensionConfig` lines 45–64 for exemplar).

**Verify**: `cargo build --lib` → exit 0. Then add a test in
`src/project/config.rs` (following existing test pattern around line 140)
that deserializes a JSON fragment with `time_intelligence` and asserts
all field values, including defaults.

### Step 2: Add `DateDimDef` to the semantic model

In `src/engine/model.rs`:

Add to `SemanticModel` (around line 143–149, after `relationships`):
```rust
pub date_dim: Option<DateDimDef>,
```

Add the struct (before `SemanticModel`):
```rust
#[derive(Debug, Clone)]
pub struct DateDimDef {
    pub dimension_id: DimId,
    pub table_name: String,
    pub date_key_column: String,
    pub full_date_column: String,
    pub year_column: String,
    pub quarter_column: String,
    pub month_column: String,
    pub ytd_flag_column: String,
    pub prior_year_flag_column: String,
}
```

Add `is_date_role: bool` to `DimensionDef` (after `cardinality_hint`):
```rust
pub is_date_role: bool,
```

Update `build_semantic_model()` in `src/project/project.rs` to populate
`date_dim` and `is_date_role` from `DateDimensionConfig`. The dimension
whose `id` matches `date_dimension.dimension_id` gets `is_date_role: true`.

**Verify**: `cargo build --lib` → exit 0. Write a test in
`src/project/project.rs` that loads `project3/proxy-config.json` with a
`time_intelligence` block added to the config file (add the block to
`project3/proxy-config.json` in step 5), and asserts `model.date_dim.is_some()`.

### Step 3: Create the date-dimension seed table

Create `data/seed_date_dim.sql` with DuckDB SQL that generates a calendar
table with flag columns:

```sql
-- Date dimension: 2020-01-01 through 2030-12-31
CREATE TABLE IF NOT EXISTS date_dim AS
SELECT
    strftime(d, '%Y%m%d')::INTEGER AS date_key,
    d::DATE AS full_date,
    strftime(d, '%Y')::INTEGER AS year,
    CEIL(strftime(d, '%m')::INTEGER / 3.0)::INTEGER AS quarter,
    strftime(d, '%m')::INTEGER AS month,
    -- YTD flag: TRUE from Jan 1 through today
    d <= CURRENT_DATE AND strftime(d, '%Y') = strftime(CURRENT_DATE, '%Y')
        AS ytd_flag,
    -- Prior year flag: same day range, one year ago
    d >= (CURRENT_DATE - INTERVAL 1 YEAR - INTERVAL (DAYOFYEAR(CURRENT_DATE) - 1) DAY)
        AND d < (CURRENT_DATE - INTERVAL 1 YEAR + INTERVAL 1 DAY)
        AS prior_year_flag
FROM (
    SELECT UNNEST(GENERATE_SERIES(
        '2020-01-01'::DATE,
        '2030-12-31'::DATE,
        INTERVAL 1 DAY
    )) AS d
);
```

This file is SQL only — no binary data. It is committed to the repo so
tests can `Backend::execute_ddl()` to create the table in-memory.

**Verify**: Write a test in `src/backend/mod.rs` (or a new test block) that:
1. Creates an in-memory DuckDB `Backend`
2. Executes the DDL from `data/seed_date_dim.sql`
3. Queries `SELECT COUNT(*) FROM date_dim WHERE ytd_flag = true`
4. Asserts count > 0 and count <= 366

### Step 4: Emit date-flag column filters in SQL

In `src/engine/sql.rs`, add awareness of date-role dimensions when a
`DimensionFilter` targets the date dimension. When a filter dimension
has `is_date_role: true` and the measured measure uses a time flag:

Define a new auxiliary struct (keep it private to `sql.rs` or `plan.rs`):
```rust
/// Hint from the plan layer: this filter should use a date-flag column
/// join rather than a direct member filter.
pub struct TimeFlagFilter {
    pub flag_column: String,   // e.g., "ytd_flag"
}
```

Add this as an optional field to `DimensionFilter` in `src/engine/plan.rs`:
```rust
pub time_flag: Option<String>,
```

In `src/engine/sql.rs`, in the WHERE-clause builder, when a
`DimensionFilter` has `time_flag = Some(flag)` AND the model has a
`date_dim`, emit a subquery join:

```sql
-- Instead of: WHERE territory = 'Northwest'
-- Emit: WHERE territory = 'Northwest'
--   AND date_key IN (SELECT date_key FROM date_dim WHERE ytd_flag = true)
```

The exact pattern should follow the existing `joins_and_where()` helper
in `src/engine/sql.rs:88-116`. Do NOT modify the main group-by SELECT
generation — only extend the WHERE clause when `time_flag.is_some()`.

The generated SQL for "Revenue YTD by Territory" should look like:
```sql
SELECT territory, SUM(revenue)
FROM sales_fact
WHERE date_key IN (SELECT date_key FROM date_dim WHERE ytd_flag = true)
GROUP BY territory
ORDER BY territory
```

**Verify**: `cargo test --lib` → all pass. Write a dedicated test in
`src/engine/sql.rs` that:
1. Builds a `QueryPlan::Total` with a `DimensionFilter` carrying
   `time_flag: Some("ytd_flag".into())`
2. Calls the SQL emitter with a model that has `date_dim` set
3. Asserts the output SQL contains `IN (SELECT date_key FROM date_dim WHERE ytd_flag = true)`

### Step 5: Add minimal time-intelligence config to project3

In `project3/proxy-config.json`, add a `time_intelligence` block:

```jsonc
"time_intelligence": {
    "date_dimension": {
        "dimension_id": "Date",
        "date_key_column": "date_key",
        "full_date_column": "full_date"
    }
}
```

Add a `Date` dimension entry:
```jsonc
{
    "id": "Date",
    "malloy_name": "date_dim",
    "physical_field": "date_dim.full_date",
    "caption": "Date",
    "hierarchy_name": "Calendar",
    "all_level_name": "(All)",
    "leaf_level_name": "Date",
    "ordinal": 5,
    "visible": true,
    "has_all": true,
    "fact_table": "default",
    "is_date_role": true
}
```

Add a `Revenue YTD` measure:
```jsonc
{
    "id": "Revenue YTD",
    "malloy_name": "revenue_ytd",
    "physical_expr": "revenue.sum()",
    "sql_expr": "SUM(revenue)",
    "caption": "Revenue YTD",
    "format_string": "#,##0.00",
    "ordinal": 3,
    "visible": true,
    "measure_group_name": "Sales",
    "time_intelligence": {
        "flag_column": "ytd_flag"
    }
}
```

Update `MeasureConfig` in `src/project/config.rs` to accept an optional
`time_intelligence` block with a `flag_column` field.

**Verify**: `cargo test --lib` → all pass. Then run `cargo run` and curl
the metadata rowsets to confirm the `Date` dimension and `Revenue YTD`
measure appear in `MDSCHEMA_DIMENSIONS` and `MDSCHEMA_MEASURES`.

### Step 6: End-to-end replay test with a synthetic time MDX query

Add a new constant in `src/test_support/fixtures.rs`:
```rust
pub const TIME_YTD_REVENUE_BY_TERRITORY: &str = r#"SELECT
  NON EMPTY { [Measures].[Revenue YTD] } ON COLUMNS,
  NON EMPTY { [Territory].[Territory].[Territory].Members } ON ROWS
FROM [Sales]
WHERE ([Measures].[Revenue YTD]) CELL PROPERTIES VALUE, FORMAT_STRING"#;
```

Add a test in `src/execute/dispatch.rs` following the pattern of the
existing Excel trace tests (see `excel_trace_total_revenue_matches_raw_sql`
at ~line 890):

```rust
#[test]
fn time_ytd_revenue_by_territory_matches_raw_sql() {
    let _project3 = with_project3();
    ensure_seeded_for_time_test();       // create date_dim in backend
    let response = get_execute_cellset_response(TIME_YTD_REVENUE_BY_TERRITORY);
    let captions = axis_captions(&response, "Axis0");
    let values = cell_values(&response);
    // Oracle: SUM(revenue) from sales_fact joined with date_dim
    // where ytd_flag = true, grouped by territory
    let (expected_captions, expected_values) = query_grouped_1d(
        "SELECT territory, SUM(revenue) FROM sales_fact \
         WHERE date_key IN (SELECT date_key FROM date_dim WHERE ytd_flag = true) \
         GROUP BY territory ORDER BY territory"
    );
    assert_eq!(captions, expected_captions);
    assert_float_slices_eq(&values, &expected_values);
}
```

The `ensure_seeded_for_time_test()` helper should call the backend to
execute the DDL from `data/seed_date_dim.sql`. Use an in-memory appender
or `Backend::get().execute_ddl(&sql)` — add the execute_ddl method to
the Backend trait if it doesn't already exist (check `src/backend/mod.rs`
for the existing `seed_in_memory()` pattern at ~line 320).

**Verify**: `cargo test --lib time_ytd` → 1 test passes. YTD revenue values
must be strictly ≤ total revenue (no time filter).

### Step 7: Run full suite

`cargo test --lib` → all tests pass (current baseline 205 + new tests).

If the date-dimension seed table is large or expensive to create repeatedly,
use `OnceLock` or a module-level static to create it once and reuse across
the test session. The `ensure_seeded_for_time_test()` helper should be
idempotent.

## Test plan

New tests to write:

1. **`src/project/config.rs`** — `time_intelligence_config_deserializes`
   Loads a JSON config fragment with `time_intelligence` block, asserts
   all field values including defaults for flag column names.

2. **`src/engine/sql.rs`** — `time_flag_filter_emits_date_dim_subquery`
   Builds a plan with `time_flag: Some("ytd_flag")`, asserts SQL contains
   the date_dim subquery.

3. **`src/backend/mod.rs`** — `date_dim_seed_creates_ytd_rows`
   Executes `data/seed_date_dim.sql`, queries ytd_flag rows, asserts count
   between 1 and 366.

4. **`src/execute/dispatch.rs`** — `time_ytd_revenue_by_territory_matches_raw_sql`
   End-to-end: MDX → plan → SQL → DuckDB → cellset. Oracle: raw SQL with
   date_dim join. Assert YTD ≤ unfiltered total.

Model after the existing Excel trace tests in `src/execute/dispatch.rs`;
follow the `with_project3()` + `axis_captions` + raw SQL oracle pattern.

## Done criteria

- [ ] `cargo build --lib` exits 0
- [ ] `cargo test --lib` exits 0; at least 4 new tests pass (steps 1, 2, 3, 4, 6)
- [ ] `project3/proxy-config.json` contains a `time_intelligence` block
- [ ] `data/seed_date_dim.sql` exists and is committed
- [ ] `MDSCHEMA_DIMENSIONS` response includes the `Date` dimension
- [ ] `MDSCHEMA_MEASURES` response includes `Revenue YTD`
- [ ] End-to-end test: YTD revenue ≤ total revenue (date filter works)
- [ ] No files outside the in-scope list are modified (`git status`)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- The config schema change causes deserialization failures on
  `project2/`, `project3/`, or `project4/` proxy-config files that lack
  the new `time_intelligence` field (the field must be `Option` with
  `#[serde(default)]`).
- The `dim_table_for_discovery()` method from plan 004 conflicts with
  the new `is_date_role` field — they must coexist without breaking
  metadata discovery.
- DuckDB does not support `GENERATE_SERIES` in the version used (1.05);
  fall back to a recursive CTE for date generation if needed.
- The date_dim table creation in step 6 test is too slow (>5 seconds for
  ~4000 rows on the developer's machine); use a smaller date range
  (current year only) for test-only seeding.
- Any existing test (the 205 baseline) fails after any step — fix before
  proceeding.

## Maintenance notes

- The `time_flag` field on `DimensionFilter` is deliberately lightweight:
  it carries only the flag column name, not the full date-dimension join
  logic. The SQL emitter resolves the rest from `SemanticModel.date_dim`.
  If more complex time windows are needed later (rolling 30-day, fiscal
  year offsets), add them as new columns in `date_dim` and new flag names
  — no plan-level changes needed.
- The converter (`src/bin/convert_tabular.rs`) should eventually
  auto-generate the `time_intelligence` block for converted projects, but
  that is explicitly deferred from this plan.
- Excel does not typically send `YTD()` or `ParallelPeriod()` in request
  MDX — it references pre-defined time measures. If a real Excel session
  ever produces time-function MDX, that is a separate parser plan.
- The `date_dim` table approach means time-intelligence correctness depends
  on the seed data being accurate. Test coverage must include boundary
  dates (year start, year end, leap years).
- All `#[serde(default)]` annotations on new config fields must be tested
  with a minimal JSON fixture that omits them, to prove backward compat
  with existing config files.
