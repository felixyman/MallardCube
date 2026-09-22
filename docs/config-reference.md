# Config reference (`proxy-config.json` / `.yaml`)

A project is one config file plus the DuckDB data it points at. The config is
the *projection*: what Excel sees (catalog, cube, dimensions, measures,
captions, formats) and how it maps onto your tables. Metric definitions and
materialisation stay upstream — see `DESIGN-INVARIANTS.md`.

Formats: JSON or YAML (by extension, then by content). `mallard fmt` writes a
config canonically and converts between the two.

```bash
mallard fmt my-project/proxy-config.yaml            # rewrite canonically
mallard fmt --check my-project/proxy-config.yaml    # CI: non-zero when dirty
mallard fmt --to yaml my-project/proxy-config.json  # print as YAML
```

Only `catalog`, `cube` and the entries' `id` + `caption` are required.
Everything else has a default; `mallard fmt` omits defaults again.

## Top level

| Key | Type | Default | Meaning |
|-----|------|---------|---------|
| `catalog` | string | — | XMLA catalog name Excel sees in the connection |
| `cube` | string | — | Cube name (`FROM [<cube>]` in MDX) |
| `source_name` | string | `table_name` | DuckDB source/attachment name |
| `table_name` | string | `""` | Default fact table (single-fact projects) |
| `dialect` | string | `duckdb` | SQL dialect for measure expressions |
| `db_path` | string? | `null` | DuckDB file. Absent → demo mode (synthetic data) |
| `fact_tables` | array | `[]` | More than one fact table (see below) |
| `relationships` | array | `[]` | Fact→dimension join keys (see below) |
| `roles` | array | `[]` | Row/object-level security (`auth.md`) |
| `auth` | object? | `null` | Trusted-proxy / OIDC identity (`auth.md`) |
| `time_intelligence` | object? | `null` | Date dimension + flag columns |
| `dimensions` | array | `[]` | Excel dimensions/hierarchies |
| `measures` | array | `[]` | Excel measures |
| `dimensions_file` | string? | `null` | Section file (large models) |
| `measures_file` | string? | `null` | Section file |
| `relationships_file` | string? | `null` | Section file |
| `roles_file` | string? | `null` | Section file |

Section files hold the same list format as the inline array. Inline entries come
first, and paths resolve relative to the config file.

## `dimensions[]`

| Key | Type | Default | Meaning |
|-----|------|---------|---------|
| `id` | string | — | Internal id (query planning, filter routing) |
| `caption` | string | — | Excel-visible dimension label |
| `physical_field` | string | `id` | Column in the fact/dimension table |
| `description` | string | `""` | Optional description |
| `hierarchy_name` | string | `caption` | Hierarchy name under the dimension |
| `all_level_name` | string | `(All)` | Name of the `(All)` level |
| `leaf_level_name` | string | `caption` | Name of the leaf level |
| `ordinal` | int | list order | Field-list order |
| `visible` | bool | `true` | Hide the dimension from Excel when false |
| `has_all` | bool | `true` | Emit an `(All)` member |
| `cardinality_hint` | int | `0` | Metadata cardinality hint |
| `fact_table` | string? | first fact | Scope the dimension to one fact table |
| `shared` | bool | `false` | Applies to every fact table |
| `is_date_role` | bool | `false` | Treat as a date role (see below) |
| `hierarchy_levels` | array | `[]` | Multi-level hierarchy (see below) |
| `parent_child` | object? | `null` | Self-referencing hierarchy |

### `hierarchy_levels[]`

| Key | Type | Default | Meaning |
|-----|------|---------|---------|
| `name` | string | — | Level name, e.g. `Year`, `Quarter`, `Month` |
| `column` | string | — | SQL column providing the level's values |
| `level_number` | int | — | Distance from the root (`0` = top level) |
| `cardinality` | int | `0` | Cardinality hint |

Levels are what Excel's drill paths walk. Compound keys are built from the
ancestor path (`&[2024]&[1]`), and the deepest level is the grain.

### `parent_child`

| Key | Type | Meaning |
|-----|------|---------|
| `key_column` | string | Node key |
| `parent_column` | string | Parent key |

Synthetic levels (`Level 01..NN`) are materialised from the recursion at
project load; `hierarchy_levels` is ignored when `parent_child` is set.

### Date roles

A date role is a dimension with `is_date_role: true` and `hierarchy_levels`.
MallardCube exposes the full-date level twice, the way tabular SSAS does:

- the **user hierarchy** (`[Date].[Calendar]`) with the configured levels;
- a single-level **key attribute hierarchy** named after the leaf level
  (`[Date].[Full Date]`), typed as a date so Excel offers **Date Filters**.

The global `time_intelligence` block names the date dimension and the flag
columns used to lower time-intelligence MDX (`YTD`, `QTD`, `MTD`,
`PeriodsToDate`, and date-window filters):

```yaml
time_intelligence:
  date_dimension:
    dimension_id: Date
    date_key_column: date_key
    full_date_column: full_date
    table_name: date_dim
    flag_columns:
      year_column: year
      quarter_column: quarter
      month_column: month
      ytd_flag_column: ytd_flag
      prior_year_ytd_flag_column: prior_year_ytd_flag
      current_year_flag_column: current_year_flag
      qtd_flag_column: qtd_flag
      mtd_flag_column: mtd_flag
```

Each flag column is optional: omitting one means the column does not exist
upstream, and the proxy treats the corresponding function as unavailable
instead of guessing a name. Per-measure overrides live in
`measures[].time_intelligence` (`flag_column`, `dimension_id`) for
role-playing calendars.

## `measures[]`

| Key | Type | Default | Meaning |
|-----|------|---------|---------|
| `id` | string | — | Internal id |
| `caption` | string | — | Excel-visible name |
| `sql_expr` | string | `""` | DuckDB SQL aggregate, e.g. `SUM(revenue)` |
| `display_name` | string | `caption` | `MEASURE_DISPLAY_FOLDER`-style label |
| `description` | string | `""` | Optional description |
| `format_string` | string | `#,##0.00` | Excel number format |
| `units` | string | `""` | Unit suffix |
| `ordinal` | int | list order | Field-list order |
| `visible` | bool | `true` | Hide from Excel when false |
| `fact_table` | string? | first fact | Scope to a fact table |
| `aggregator` | int | `0` | OLE DB aggregator hint |
| `measure_group_name` | string | fact group | Measure group (folder) |
| `numeric_precision` / `numeric_scale` | int | — | Metadata precision/scale |
| `expression` | string | `""` | Original expression (documentation) |
| `sql_fallback_file` | string? | `null` | SQL file for the CUBEVALUE fallback path |
| `time_intelligence` | object? | `null` | Per-measure date-role override |
| `fallback_capability` | string? | `null` | Declared fallback capability |

The proxy serves `SUM`-style aggregates, ratio-of-sums, and identity measures.
Anything more complex belongs upstream in the marts.

## `fact_tables[]` and `relationships[]`

```yaml
fact_tables:
  - id: sales
    source_name: sales_data
    table_name: sales_fact
    measure_group_name: Sales
  - id: inventory
    source_name: inventory_data
    table_name: inventory_fact
    measure_group_name: Inventory

relationships:
  - fact_table: sales
    fact_column: date_key
    dimension_id: Date
    dim_table: date_dim
    dim_column: date_key
```

| Key | Meaning |
|-----|---------|
| `fact_tables[].id` | Fact id referenced by `fact_table` on dims/measures |
| `fact_tables[].source_name` | DuckDB source name (defaults to `table_name`) |
| `fact_tables[].table_name` | Physical table |
| `fact_tables[].measure_group_name` | Measure group shown in Excel |
| `relationships[].fact_table` | Owning fact id |
| `relationships[].fact_column` | Join column on the fact |
| `relationships[].dimension_id` | Dimension id |
| `relationships[].dim_table` / `dim_column` | Physical dim table and key |

`shared: true` dimensions apply to every fact table; `fact_table` on a
dimension or measure scopes it to one. Unrelated dimension filters are silently
ignored, as SSAS does.

## Examples

Minimal single-fact project:

```yaml
catalog: SALES_ANALYTICS
cube: Sales
source_name: sales_data
table_name: sales_fact
db_path: ./sales.duckdb

dimensions:
  - id: Category
    caption: Category
    physical_field: category
  - id: Channel
    caption: Channel
    physical_field: channel

measures:
  - id: Revenue
    caption: Revenue
    sql_expr: SUM(revenue)
```

Date role with a calendar hierarchy:

```yaml
dimensions:
  - id: Date
    caption: Date
    physical_field: date_key
    is_date_role: true
    hierarchy_name: Calendar
    leaf_level_name: Full Date
    hierarchy_levels:
      - { name: Year,      column: year,    level_number: 0, cardinality: 6 }
      - { name: Quarter,   column: quarter, level_number: 1, cardinality: 24 }
      - { name: Month,     column: month,   level_number: 2, cardinality: 72 }
      - { name: Full Date, column: full_date, level_number: 3, cardinality: 2200 }

time_intelligence:
  date_dimension:
    dimension_id: Date
    date_key_column: date_key
    full_date_column: full_date
    table_name: date_dim
```

The leaf level's name (`Full Date`) becomes the key attribute hierarchy
(`[Date].[Full Date]`) — that is the field Excel offers Date Filters on.

## See also

- `README.md` — quick start, connecting Excel, performance.
- `docs/converting-models.md` — generating a config from a Tabular export.
- `docs/auth.md` — `roles` and `auth` in detail.
- `docs/DESIGN-INVARIANTS.md` — what the proxy deliberately does not do.
