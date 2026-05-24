# Naming Contract

Three distinct name types flow through the proxy.  Every module must use
the correct type intentionally — never conflate them.

## 1. `id` (internal / XMLA identifier)

- **Where defined**: `proxy-config.json` – `dimensions[].id` / `measures[].id`
- **What it is**: the stable primary key the proxy uses to identify a
  dimension or measure internally.
- **Used by**: `SemanticModel`, `QueryPlan`, `plan_key`, filter routing,
  MDX semantic parsing.
- **Examples**: `"Produktkategori"`, `"Region"`, `"Category"`,
  `"Territory"`, `"Revenue"`.
- **Rules**:
  - Must be unique within the project.
  - Does not need to match the Malloy field name or DuckDB column name.
  - Appears in generated `plan_key` strings.
  - This is the value in `QueryPlan.group_by` / `QueryPlan::Total.measure`.

## 2. `malloy_name` (semantic / runtime field name)

- **Where defined**: `proxy-config.json` – `dimensions[].malloy_name` /
  `measures[].malloy_name`
- **What it is**: the name the Malloy compiler and DuckDB know for a field
  or measure.
- **Used by**: Malloy emitter (query fragments), DuckDB schema operations.
- **Examples**: `"produktkategori"`, `"region"`, `"revenue"`.
- **Rules**:
  - Must match the corresponding field name in the `.malloy` source.
  - Stored in `DimensionDef.semantic_name` / `MeasureDef.semantic_name`.
  - Never appears directly in XMLA output or Excel-visible metadata.

## 3. `caption` (Excel-visible label)

- **Where defined**: `proxy-config.json` – `dimensions[].caption` /
  `measures[].caption`
- **What it is**: the human-readable label Excel shows in the PivotTable
  field list.
- **Used by**: XMLA metadata rowsets, member rendering, `DISCOVER_*`
  responses.
- **Examples**: `"Produktkategori"`, `"Category"`, `"Revenue"`.
- **Rules**:
  - Can be any user-friendly string, including spaces and Unicode.
  - There is no requirement that `caption` matches `id` or `malloy_name`.
  - Appears in `DimensionDef.caption`, `MeasureDef.caption`,
    `MeasureDef.display_name`.

## Additional conventions

- **`physical_field`**: the DuckDB column name (`DimensionsDef.physical_field`).
  Used by SQL queries and DuckDB introspection.  Often equals `malloy_name`,
  but they are conceptually distinct.
- **`hierarchy_name` / `all_level_name` / `leaf_level_name`**: XMLA-specific
  presentation details that SSAS-style clients expect.  Defined per dimension
  in the config.  Values are always SSAS-friendly strings (e.g. `"(All)"`).
- **`physical_expr`** (measures only): the Malloy expression (e.g.
  `"sales.sum()"`).  Used by the Malloy emitter.
- **`sql_expr`** (measures only): the SQL expression (e.g. `"SUM(sales)"`).
  Used only by the direct-SQL fallback path.  Optional in the future.

## Lookup rules

- To find a dimension by its MDX bracketed name:
  1. Try `lookup_dimension()` which searches by `caption`, `id`, and
     dimension-unique-name fragments.
  2. Fall back to `default_dimension_id()`.
- To find a measure: use `meas_def(id)` or `default_measure_id()`.
