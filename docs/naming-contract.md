# Naming Contract

Several distinct name types flow through the proxy. Every module must use the
correct type intentionally — never conflate them.

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
  - Does not need to match the DuckDB column name.
  - Appears in generated `plan_key` strings.
  - This is the value in `QueryPlan.group_by` / `QueryPlan::Total.measure`.

## 2. `caption` (Excel-visible label)

- **Where defined**: `proxy-config.json` – `dimensions[].caption` /
  `measures[].caption`
- **What it is**: the human-readable label Excel shows in the PivotTable
  field list.
- **Used by**: XMLA metadata rowsets, member rendering, `DISCOVER_*`
  responses.
- **Examples**: `"Produktkategori"`, `"Category"`, `"Revenue"`.
- **Rules**:
  - Can be any user-friendly string, including spaces and Unicode.
  - There is no requirement that `caption` matches `id` or `physical_field`.
  - Appears in `DimensionDef.caption`, `MeasureDef.caption`,
    `MeasureDef.display_name`.

## 3. `physical_field` (DuckDB column name)

- **Where defined**: `proxy-config.json` – `dimensions[].physical_field`
- **What it is**: the DuckDB column backing a dimension.
- **Used by**: SQL queries and DuckDB introspection.
- **Rules**:
  - Must be a real column in the dimension's table.
  - May use `table.column` syntax.
  - Often equals `id`, but they are conceptually distinct.

## 4. `sql_expr` (measure SQL expression)

- **Where defined**: `proxy-config.json` – `measures[].sql_expr`
- **What it is**: the DuckDB SQL expression that computes the measure.
- **Examples**: `"SUM(revenue)"`, `"COUNT(*)"`, `"AVG(price)"`.
- **Rules**:
  - Direct SQL is the only runtime path — this is the expression that runs.

## Additional conventions

- **`hierarchy_name` / `all_level_name` / `leaf_level_name`**: XMLA-specific
  presentation details that SSAS-style clients expect. Defined per dimension
  in the config. Values are always SSAS-friendly strings (e.g. `"(All)"`).

## Lookup rules

- To find a dimension by its MDX bracketed name:
  1. Try `lookup_dimension()` which searches by `caption`, `id`, and
     dimension-unique-name fragments.
  2. Fall back to `default_dimension_id()`.
- To find a measure: use `lookup_measure()` (matches `id`, `caption`, or
  `display_name`) or `default_measure_id()`.
