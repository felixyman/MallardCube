# SSAS Tabular → Malloy + DuckDB Conversion Reference

This document is a specification for converting Microsoft SSAS Tabular Model
(`Model.bim`) projects to Malloy semantic models backed by DuckDB, served
through the the `mallard` Excel/XMLA proxy.

It is designed to be used as a system prompt for an LLM. The LLM should:

1. Parse a `.bim` JSON file
2. Classify each measure (simple / sql_complex / untranslatable)
3. Emit `model.malloy`, `proxy-config.json`, and complex measure SQL files

---

## 1. Input: `.bim` file structure

A `.bim` file is a JSON document with a root `model` object.
The fields that matter for conversion:

```jsonc
{
  "model": {
    "name": "SalesModel",           // → catalog name
    "tables": [
      {
        "name": "Sales",            // → Malloy source name
        "columns": [
          {
            "name": "Revenue",      // column name
            "dataType": "double",   // Int64 | double | string | dateTime | boolean
            "sourceColumn": "Amount", // actual DB column (may differ from name)
            "isHidden": false       // true = exclude from dimension list
          },
          {
            "name": "TotalRevenue",
            "type": "calculated",   // measure marker
            "expression": "SUM(Sales[Revenue])",
            "formatString": "#,##0.00",
            "displayFolder": "Sales"
          }
        ],
        "partitions": [
          {
            "source": {
              "type": "m",          // "m" = M query, "query" = SQL, etc.
              "expression": "let Source = Sql.Database(...)"
            }
          }
        ]
      }
    ],
    "relationships": [
      {
        "fromTable": "Sales",
        "fromColumn": "ProductKey",
        "toTable": "Product",
        "toColumn": "ProductKey",
        "isActive": true            // false = alternate, not primary
      }
    ]
  }
}
```

### What to ignore

- `perspectives` — Excel-level view filters, handled by proxy config visibility
- `roles` / `rowLevelSecurity` — not supported
- `annotations` — discard
- `translations` — not supported
- `cultures` / `linguisticMetadata` — not supported
- Partitions with `source.type = "m"` — M queries can't be imported to DuckDB; mark for manual data migration

---

## 2. Output files

The converter produces three outputs for a project named `MyProject`:

| File | Purpose |
|------|---------|
| `MyProject.bim-model/model.malloy` | Malloy semantic model (sources, joins, dimensions, measures) |
| `MyProject.bim-model/proxy-config.json` | Excel/XMLA presentation (captions, formatting, ordinals) |
| `MyProject.bim-model/complex_measures/` | DuckDB SQL templates for measures Malloy can't express |

### Naming rules

- **`id`** (in proxy-config): Use the original `.bim` column/measure name. This is the internal identifier.
- **`malloy_name`** : Lowercase the name, replace spaces with underscores. Must be a valid Malloy identifier.
- **`caption`** : Use the original `.bim` name — Excel sees this.

Example: `.bim` column `"Product Name"` →
```
id: "Product Name"
malloy_name: "product_name"
caption: "Product Name"
```

---

## 3. Table and column mapping

### Table → Malloy source

Each `.bim` table becomes a Malloy source. The source name is the lowercased
table name with spaces replaced by underscores.

```
.bim table "Sales" → source: sales is duckdb.table('sales') extend { ... }
```

The DuckDB table name should match what the user creates — assume it matches
the lowercased `.bim` table name unless the user specifies otherwise.

### Column → dimension

Every non-hidden, non-calculated column becomes a Malloy dimension.
If the column name matches the column physically (no rename),
no dimension declaration is needed — Malloy discovers it automatically.

```
Sales[Category] (string, not hidden)
  → proxy-config dimension with id: "Category", malloy_name: "category"
  → Malloy: column "category" auto-discovered, no explicit dimension needed
```

If the column name differs from the source:

```
Sales[Category] with sourceColumn: "cat" and name: "Category"
  → Malloy: dimension: category is cat
```

### Data type mapping

| `.bim` dataType | DuckDB type |
|----------------|-------------|
| `int64` | `BIGINT` |
| `double` | `DOUBLE` |
| `string` | `VARCHAR` |
| `boolean` | `BOOLEAN` |
| `dateTime` | `TIMESTAMP` |
| `decimal` | `DECIMAL` |

### Relationship → Malloy join

Active relationships become Malloy joins:

```malloy
source: sales is duckdb.table('sales') extend {
  join_one: product with product_key
  join_many: orders with order_id
}
```

Rules:
- If `toTable` is the one-side (dimension table): `join_one: to_table with from_column`
- If `toTable` is the many-side (fact table): `join_many: to_table with from_column`
- Inactive relationships (`isActive: false`): skip, document as not supported
- Cross-filter direction is ignored — Malloy handles bi-directional naturally
- Self-joins (role-playing dimensions): create separate join names

Example of role-playing date dimensions:

```malloy
source: sales is duckdb.table('sales') extend {
  join_one: order_date_dim is date_dim with order_date_key
  join_one: ship_date_dim is date_dim with ship_date_key
}
```

---

## 4. DAX → Malloy measure map

### Classification: Simple

These DAX patterns map directly to Malloy measures.
Tag them `"type": "simple"` in proxy-config (or omit — simple is default).

| DAX pattern | Malloy equivalent |
|---|---|
| `SUM(Table[Col])` | `measure: name is col.sum()` |
| `SUMX(Table, Table[Col])` | `measure: name is col.sum()` |
| `COUNT(Table[Col])` | `measure: name is col.count()` |
| `COUNTROWS(Table)` | `measure: name is count()` |
| `DISTINCTCOUNT(Table[Col])` | `measure: name is col.count(distinct true)` |
| `MIN(Table[Col])` | `measure: name is col.min()` |
| `MAX(Table[Col])` | `measure: name is col.max()` |
| `AVERAGE(Table[Col])` | `measure: name is col.avg()` |
| `CALCULATE(SUM(Table[Col]), Table[FilterCol]="value")` | Prefer DuckDB generated column (Section 6) + Malloy `{ where: }`. Direct alternative: `measure: name is col.sum() { where: filter_col = 'value' }` |
| `CALCULATE(SUM(Col), T1[C]="A" \|\| T1[C]="B")` | Prefer generated column. Direct: `measure: name is col.sum() { where: c = 'A' or c = 'B' }` |
| `CALCULATE(SUM(Col), T1[C]="A" && T2[D]="B")` | (AND across tables not supported in Malloy measures — classify as sql_complex) |
| `DIVIDE(a, b, 0)` | Handled in proxy rendering; emit `a/b` with null guard fallback |

### Classification: SQL complex

These DAX patterns cannot be expressed in Malloy. Tag them `"type": "sql_complex"`.
They will be served by the DuckDB SQL fallback path.

| DAX pattern | Why Malloy can't | DuckDB SQL approach |
|---|---|---|
| `CALCULATE(SUM(Col), ALL(T))` | Context removal has no relational equivalent | Window: `SUM(col) OVER ()` |
| `CALCULATE(SUM(Col), ALLEXCEPT(T, Dim))` | Selective context removal | Window + PARTITION BY |
| `CALCULATE(SUM(Col), FILTER(T, ComplexExpr))` | Iterative filter context | WHERE EXISTS / CTE |
| `SUMX(T, T[Col1] * T[Col2])` | Row-level iterator | Subquery with computed column |
| `AVERAGEX(T, Expr)` | Row-level iterator | Subquery with computed column + AVG |
| `RANKX(ALL(T), SUM(Col))` | Ranking over groups | Window: `RANK() OVER (ORDER BY SUM(col))` |
| `IF(Cond, A, B)` / `SWITCH` at measure level | Malloy has no conditional measure logic | CASE WHEN in SQL |
| `CALCULATE(..., KEEPFILTERS(...))` | Filter context intersection | Explicit combined WHERE |
| `CALCULATE(..., USERELATIONSHIP(...))` | Inactive relationship activation | Separate join + measure in Malloy |

### Classification: Untranslatable

Tag these `"type": "untranslatable"`. They produce no output — only a human-readable
note in the log. The user must rewrite them manually.

| DAX pattern | Why untranslatable |
|---|---|
| Nested `CALCULATE` with stacked filter contexts | No relational mapping exists |
| `CALCULATE(..., ALL(Table[Col]))` inside another `CALCULATE` | Context stacking is DAX-specific |
| `CROSSFILTER(..., Both)` direction changes | Malloy doesn't model filter direction |
| `TREATAS` table expression | No equivalent in Malloy or SQL |
| Arbitrary DAX expression as a measure | Needs human decomposition |
| Measures referencing other measures that are themselves untranslatable | Cascade — mark all as untranslatable |

### Determining the table alias for Malloy

When a DAX measure references a table, use the lowercased table name:

```
SUM(Sales[Revenue]) → sales.revenue.sum()
SUM('Product Sales'[Amount]) → product_sales.amount.sum()
```

When a CALCULATE filter references a column from a different table,
the converter must trace the relationship to determine which join to use:

```
CALCULATE(SUM(Sales[Revenue]), Product[Category]="Electronics")
  // Product is joined to Sales via the active relationship
  // Malloy measure references the source directly:
  → measure: electronics_revenue is revenue.sum() { where: category = 'Electronics' }
```

---

## 5. DAX → Complex (DuckDB SQL fallback)

Complex measures produce SQL template files. Each file contains a DuckDB query
that the proxy substitutes table/column placeholders into at runtime.

### Template format

```sql
-- complex_measures/electronics_share.sql
-- Type: CALCULATE + ALL
-- Original DAX: CALCULATE(SUM(Sales[Revenue]), ALL(Product))
-- Returns: electronics share of total revenue

SELECT
  SUM(revenue) AS total_revenue,
  SUM(CASE WHEN category = 'Electronics' THEN revenue ELSE 0 END) / NULLIF(SUM(revenue), 0) AS electronics_share
FROM {sales_fact}
```

The proxy substitutes `{table_placeholders}` with the actual DuckDB table name
from `proxy_config.json`.

### Example conversions

#### RANKX / window function

```dax
RANKX(ALL(Product), SUM(Sales[Revenue]))
```

```sql
-- Complex measure: product_revenue_rank
SELECT
  product_key,
  SUM(revenue) AS revenue,
  RANK() OVER (ORDER BY SUM(revenue) DESC) AS rank
FROM {sales}
GROUP BY product_key
```

#### SUMX with expression

```dax
SUMX(Sales, Sales[Quantity] * Sales[UnitPrice])
```

```sql
-- Complex measure: total_line_amount
SELECT SUM(quantity * unit_price) AS total_line_amount
FROM {sales}
```

(A measure this simple should actually be a Malloy measure: `measure: total_line_amount is (quantity * unit_price).sum()`. The converter should prefer Malloy when the expression is a simple arithmetic combination of columns.)

#### CALCULATE + ALLEXCEPT

```dax
CALCULATE(SUM(Sales[Revenue]), ALLEXCEPT(Sales, Sales[Country]))
```

```sql
-- Complex measure: country_total
SELECT
  country,
  SUM(revenue) AS country_total,
  SUM(SUM(revenue)) OVER (PARTITION BY country) AS all_within_country
FROM {sales}
GROUP BY country, -- other dimensions
```

#### IF / conditional

```dax
IF(SUM(Sales[Revenue]) > 10000, SUM(Sales[Revenue]) * 0.9, SUM(Sales[Revenue]))
```

```sql
-- Complex measure: discounted_revenue
SELECT
  SUM(revenue) AS raw_revenue,
  CASE WHEN SUM(revenue) > 10000 THEN SUM(revenue) * 0.9 ELSE SUM(revenue) END AS discounted_revenue
FROM {sales}
```

---

## 6. DuckDB generated columns (pre-computed filters)

The single best way to maximize Malloy coverage is to pre-compute static
CALCULATE filters as DuckDB generated columns on the fact table.
A generated column becomes a boolean dimension that Malloy can filter on
as a simple `{ where: }` clause — no SQL fallback needed.

### What can be pre-computed

Any CALCULATE filter that depends only on columns **within the same table**
and does not involve context manipulation (ALL, ALLEXCEPT, KEEPFILTERS):

| DAX pattern | Pre-computed column | Malloy measure |
|---|---|---|
| `CALCULATE(SUM(Rev), Category="Electronics")` | `is_electronics BOOLEAN` | `revenue.sum() { where: is_electronics = true }` |
| `CALCULATE(SUM(Rev), Amount > 1000)` | `is_high_value BOOLEAN` | `revenue.sum() { where: is_high_value = true }` |
| `CALCULATE(SUM(Rev), Channel="Online" AND Segment="Consumer")` | `is_online_consumer BOOLEAN` | `revenue.sum() { where: is_online_consumer = true }` |
| `CALCULATE(SUM(Rev), Category="A" \|\| Category="B")` | `is_target_category BOOLEAN` | `revenue.sum() { where: is_target_category = true }` |

### DuckDB syntax

```sql
ALTER TABLE sales ADD COLUMN is_electronics BOOLEAN
  GENERATED ALWAYS AS (category = 'Electronics');

ALTER TABLE sales ADD COLUMN is_high_value BOOLEAN
  GENERATED ALWAYS AS (amount > 1000);

ALTER TABLE sales ADD COLUMN is_online_consumer BOOLEAN
  GENERATED ALWAYS AS (channel = 'Online' AND segment = 'Consumer');
```

Generated columns are automatically maintained — no ETL, no triggers.
They recalculate on every INSERT/UPDATE and are readable by Malloy.

### When the filter involves a joined dimension

If the CALCULATE filters on a column in a joined table, and the condition
is static, use a Malloy `where:` clause referencing the join — no generated
column needed because Malloy propagates the filter through the join at query time:

```dax
CALCULATE(SUM(Sales[Revenue]), Product[Category]="Electronics")
```

```malloy
measure: electronics_revenue is revenue.sum() { where: product.category = 'Electronics' }
```

### What CANNOT be pre-computed

These must stay as `sql_complex` because the filter depends on the
user's dynamic selection context, not a fixed condition:

| DAX pattern | Why dynamic |
|---|---|
| `CALCULATE(SUM(Rev), ALL(Product))` | "ALL" means "ignore whatever Product the user filtered on" — it changes per query |
| `CALCULATE(SUM(Rev), ALLEXCEPT(T, Dim))` | Selective context removal — depends on which dims are on-axes |
| `CALCULATE(SUM(Rev), KEEPFILTERS(...))` | Context intersection — depends on the current Excel filter state |
| `RANKX(ALL(T), SUM(Col))` | Rank order changes based on what columns are visible |

### How the converter emits this

For each measure with a static CALCULATE filter:

1. Emit `ALTER TABLE ADD COLUMN ... GENERATED ALWAYS AS (...)` in `schema.sql`
2. The column becomes a dimension in the Malloy model
3. Emit a simple Malloy measure using `{ where: generated_col = true }`
4. Tag as `"type": "simple"` in proxy-config — no `sql_complex` needed

### Effect on coverage

Pre-computation shifts the measure classification split from roughly 60/40
to 85/15 in favor of Malloy. Most SSAS Tabular models use CALCULATE primarily
for static partition filters (category, channel, segment, date ranges).
Every one of those becomes a generated column + simple Malloy measure.

---

## 7. Time intelligence

**Rule: Do NOT translate time-intelligence DAX to SQL.**

DAX time-intelligence functions (`TOTALYTD`, `SAMEPERIODLASTYEAR`, `DATESYTD`, etc.)
do not map to relational queries. Instead, **model a date dimension table** with
pre-built columns, then filter on those columns in Malloy measures.

### Date dimension schema

Create a DuckDB table `date_dim` with a row for every relevant date:

```sql
CREATE TABLE date_dim (
    date_key       INTEGER NOT NULL,  -- YYYYMMDD
    full_date      DATE NOT NULL,
    year           INTEGER NOT NULL,
    quarter        INTEGER NOT NULL,  -- 1-4
    month          INTEGER NOT NULL,  -- 1-12
    month_name     VARCHAR NOT NULL,  -- January, February, ...
    day_of_month   INTEGER NOT NULL,
    day_of_week    INTEGER NOT NULL,  -- 1=Monday, 7=Sunday
    week_of_year   INTEGER NOT NULL,
    ytd_flag       BOOLEAN NOT NULL,  -- TRUE for all dates <= today in current year
    is_current_year  BOOLEAN NOT NULL,
    prior_year_flag  BOOLEAN NOT NULL, -- TRUE for dates in the prior year, same month/day
    fiscal_year    INTEGER            -- if different from calendar year
);
```

### Malloy source for date dimension

```malloy
source: date_dim is duckdb.table('date_dim') extend {
  dimension: date_key is date_key
  dimension: year is year
  dimension: quarter is quarter
  dimension: ytd_flag is ytd_flag
  measure: date_count is count()
}
```

### Converting time-intelligence DAX

#### TOTALYTD → ytd_flag filter

```dax
TOTALYTD(SUM(Sales[Revenue]), 'Date'[Date])
```

```malloy
measure: ytd_revenue is revenue.sum() { where: date_dim.ytd_flag = true }
```

#### SAMEPERIODLASTYEAR → prior_year_flag

```dax
CALCULATE(SUM(Sales[Revenue]), SAMEPERIODLASTYEAR('Date'[Date]))
```

```malloy
measure: revenue_prior_year is revenue.sum() { where: date_dim.prior_year_flag = true }
```

#### Year-over-year growth

```dax
DIVIDE(
  SUM(Sales[Revenue]) - CALCULATE(SUM(Sales[Revenue]), SAMEPERIODLASTYEAR('Date'[Date])),
  CALCULATE(SUM(Sales[Revenue]), SAMEPERIODLASTYEAR('Date'[Date]))
)
```

This is handled in the BI layer. Emit two Malloy measures and let the BI tool
compute the ratio:

```malloy
measure: revenue_current is revenue.sum() { where: date_dim.is_current_year = true }
measure: revenue_prior is revenue.sum() { where: date_dim.prior_year_flag = true }
```

### Generating the date dimension

The user must create the `date_dim` table in DuckDB before using the model.
A helper script or SQL generator is assumed. The converter should:
1. Detect date/time columns referenced in DAX measures
2. Emit a `CREATE TABLE date_dim` SQL file with instructions
3. Add `join_one: date_dim with date_key` to the fact source in Malloy
4. Warn if no date dimension table exists in the `.bim`

### Time intelligence that can't be pre-modelled

If the `.bim` uses `DATESBETWEEN`, `DATESINPERIOD`, or other dynamic date ranges
as filters inside CALCULATE, these become `sql_complex` measures with DuckDB SQL:

```dax
CALCULATE(SUM(Sales[Revenue]), DATESBETWEEN('Date'[Date], [StartDate], [EndDate]))
```

```sql
-- Complex measure: revenue_between
SELECT SUM(revenue) FROM {sales} s
JOIN {date_dim} d ON s.date_key = d.date_key
WHERE d.full_date BETWEEN {start} AND {end}
```

---

## 8. Star schema / multi-table

### Malloy join syntax

| Relationship direction | Malloy syntax |
|---|---|
| Fact → Dimension (many-to-one) | `join_one: dim with fk_column` |
| Fact → Fact detail (one-to-many) | `join_many: detail with fk_column` |
| Dimension → Dimension (snowflake) | `join_one: sub_dim with fk_column` |

### Tracing relationships from `.bim`

Given:

```json
{
  "relationships": [
    { "fromTable": "Sales", "fromColumn": "ProductKey",
      "toTable": "Product", "toColumn": "ProductKey" }
  ]
}
```

Emit:

```malloy
source: sales is duckdb.table('sales') extend {
  join_one: product with product_key
```

If the `.bim` has no relationships (single table model), no joins are emitted.

### Naming joined columns

When a column comes from a joined dimension, prefix it with the join name in
Malloy filters:

```malloy
measure: electronics_revenue is revenue.sum() { where: product.category = 'Electronics' }
```

The proxy-config dimension `physical_field` should include the join prefix:

```jsonc
{ "id": "Product Category", "malloy_name": "category",
  "physical_field": "product.category" }
```

### Snowflake / multi-level dimensions

Chained relationships are flattened in Malloy. If Product → Subcategory → Category
is chained, only the direct join (Sales → Product) is emitted, and the
Product source definition includes its own join to Subcategory:

```malloy
source: product is duckdb.table('product') extend {
  join_one: subcategory with subcategory_key
}
source: sales is duckdb.table('sales') extend {
  join_one: product with product_key
}
```

Filters reference the full path:

```malloy
{ where: product.subcategory.category.name = 'Electronics' }
```

---

## 9. Untranslatable DAX patterns

### Iterator functions with complex expressions

```dax
SUMX(
  FILTER(Sales, Sales[Amount] > 100),
  Sales[Amount] * RELATED(Product[Margin])
)
```

**Why:** The FILTER creates a virtual table, then SUMX iterates it with a
RELATED lookup. Both the virtual table and the RELATED require multi-step
query structure that doesn't map to a single aggregation.

**Action:** Mark as `untranslatable`. Write a human-readable note suggesting
the user decompose this into: (1) a derived table or view in DuckDB,
(2) a Malloy measure on that view.

### CALCULATE with context stacking

```dax
CALCULATE(
  SUM(Sales[Revenue]),
  ALL(Product),
  KEEPFILTERS(Date[Year] = 2024)
)
```

**Why:** Context removal (ALL) combined with context intersection (KEEPFILTERS)
has no SQL equivalent. SQL window functions handle ALL patterns but can't
simultaneously preserve some filters while removing others in the way DAX does.

**Action:** Mark as `untranslatable`. Note the business question being asked
and suggest a multi-query approach: one query with the KEEPFILTERS condition,
one without Product, handled in BI.

### USERELATIONSHIP (inactive relationship)

```dax
CALCULATE(SUM(Sales[Revenue]), USERELATIONSHIP(Sales[ShipDateKey], 'Date'[DateKey]))
```

**Why:** DAX allows switching between active and inactive relationships at
query time. Malloy requires separate join definitions. This can be modelled
but not translated automatically.

**Workaround:** The converter should detect inactive relationships and add
them as additional joins with distinct names:

```malloy
join_one: order_date_dim is date_dim with order_date_key
join_one: ship_date_dim is date_dim with ship_date_key
```

Then the DAX measure becomes a Malloy measure referencing the right join:

```malloy
measure: ship_date_revenue is revenue.sum() { where: ship_date_dim.year = 2024 }
```

The converter can handle this if the inactive relationship clearly maps to
one specific role-playing join. It becomes untranslatable only when the DAX
measure dynamically switches relationships.

### Arbitrary DAX expressions

Examples:
- `"TotalRevenue" * 0.5 + "OtherMeasure"` (measure chaining)
- Complex `VAR` / `RETURN` blocks
- `SUMMARIZE` / `ADDCOLUMNS` / `SUMMARIZECOLUMNS` as intermediate steps

**Why:** These are programming patterns, not algebraic aggregations.

**Action:** Mark as `untranslatable`. LLM should write a brief explanation of
what the DAX is computing and suggest a relational decomposition.

---

## 10. Proxy config output format

### Complete example

```jsonc
{
  "catalog": "SALES_MODEL",
  "cube": "Sales",
  "source_name": "sales",
  "table_name": "sales",
  "dialect": "duckdb",
  "malloy_model_file": "model.malloy",
  "db_path": null,

  // Each dimension maps a Malloy column to an Excel-visible field
  "dimensions": [
    {
      "id": "Category",
      "malloy_name": "category",
      "physical_field": "category",
      "caption": "Category",
      "description": "Product category",
      "hierarchy_name": "Category",
      "all_level_name": "(All)",
      "leaf_level_name": "Category",
      "ordinal": 1,
      "visible": true,
      "has_all": true,
      "cardinality_hint": 20
    }
  ],

  // Each measure appears in Excel's field list
  "measures": [
    {
      "id": "TotalRevenue",
      "type": "simple",
      "malloy_name": "total_revenue",
      "physical_expr": "revenue.sum()",
      "sql_expr": "SUM(revenue)",
      "caption": "Total Revenue",
      "display_name": "Total Revenue (USD)",
      "description": "Sum of all sales revenue",
      "format_string": "#,##0.00",
      "units": "USD",
      "ordinal": 1,
      "visible": true,
      "measure_group_name": "Sales"
    },
    {
      "id": "ElectronicsShare",
      "type": "sql_complex",
      "malloy_name": null,
      "physical_expr": null,
      "sql_expr": "complex_measures/electronics_share.sql",
      "caption": "Electronics Share",
      "display_name": "% Electronics",
      "description": "Electronics revenue as percent of total",
      "format_string": "0.00%",
      "units": "",
      "ordinal": 2,
      "visible": true,
      "measure_group_name": "Sales"
    },
    {
      "id": "DynamicRankMeasure",
      "type": "untranslatable",
      "malloy_name": null,
      "physical_expr": null,
      "sql_expr": null,
      "caption": "Dynamic Rank",
      "display_name": "Dynamic Rank",
      "description": "UNTRANSLATABLE: RANKX + CALCULATE with dynamic context. Requires manual rewrite.",
      "format_string": "#,##0",
      "units": "",
      "ordinal": 99,
      "visible": false,
      "measure_group_name": "Sales"
    }
  ]
}
```

### Field reference

| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | Internal identifier, must be unique |
| `type` | No | `"simple"` (Malloy), `"sql_complex"` (DuckDB SQL), `"untranslatable"` (manual). Defaults to `"simple"` |
| `malloy_name` | For `simple` | Name in the `.malloy` file |
| `physical_expr` | For `simple` | Malloy expression (e.g., `revenue.sum()`) |
| `sql_expr` | For `simple` and `sql_complex` | For `simple`: the equivalent SQL. For `sql_complex`: path to SQL template file |
| `caption` | Yes | Short Excel label |
| `display_name` | Yes | Longer label, shown in PivotTable field list |
| `format_string` | Yes | Excel number format |
| `visible` | Yes | `true` for measures, `false` for calculation helpers |
| `ordinal` | Yes | Sort order in Excel field list (1, 2, 3...) |
| `measure_group_name` | Yes | Groups measures under a folder in Excel |

### Format string mapping

| `.bim` formatString | Proxy config equivalent |
|---|---|
| `#,##0` | `#,##0` |
| `#,##0.00` | `#,##0.00` |
| `0%` | `0%` |
| `0.00%` | `0.00%` |
| `$#,##0.00` | `$#,##0.00` |
| `mm/dd/yyyy` | Ignore — measures are numeric |
| Custom formats | Pass through unchanged |

---

## 11. Full worked example

### Input `.bim` (simplified)

```jsonc
{
  "model": {
    "name": "AdventureWorks",
    "tables": [
      {
        "name": "Internet Sales",
        "columns": [
          { "name": "SalesAmount", "dataType": "double" },
          { "name": "OrderQuantity", "dataType": "int64" },
          { "name": "ProductKey", "dataType": "int64", "isHidden": true },
          { "name": "OrderDateKey", "dataType": "int64", "isHidden": true },
          { "name": "TotalSales",
            "type": "calculated",
            "expression": "SUM('Internet Sales'[SalesAmount])",
            "formatString": "#,##0.00" },
          { "name": "OrderCount",
            "type": "calculated",
            "expression": "COUNTROWS('Internet Sales')",
            "formatString": "#,##0" },
          { "name": "ElectronicsSales",
            "type": "calculated",
            "expression": "CALCULATE(SUM('Internet Sales'[SalesAmount]), Product[Category]=\"Electronics\")",
            "formatString": "#,##0.00" },
          { "name": "YTD Revenue",
            "type": "calculated",
            "expression": "TOTALYTD(SUM('Internet Sales'[SalesAmount]), "Date'[FullDate])",
            "formatString": "#,##0.00" },
          { "name": "CountryShare",
            "type": "calculated",
            "expression": "DIVIDE(SUM('Internet Sales'[SalesAmount]), CALCULATE(SUM('Internet Sales'[SalesAmount]), ALL(Geography)))",
            "formatString": "0.00%" },
          { "name": "SalesRank",
            "type": "calculated",
            "expression": "RANKX(ALL(Product), SUM('Internet Sales'[SalesAmount]))",
            "formatString": "#,##0" },
          { "name": "ComplexNested",
            "type": "calculated",
            "expression": "CALCULATE(SUM('Internet Sales'[SalesAmount]), FILTER(ALL('Date'), 'Date'[CalendarYear] = 2024), KEEPFILTERS(Product[Category]=\"Electronics\"))",
            "formatString": "#,##0.00" }
        ]
      },
      {
        "name": "Product",
        "columns": [
          { "name": "ProductKey", "dataType": "int64" },
          { "name": "ProductName", "dataType": "string" },
          { "name": "Category", "dataType": "string" },
          { "name": "Subcategory", "dataType": "string" }
        ]
      },
      {
        "name": "Date",
        "columns": [
          { "name": "DateKey", "dataType": "int64" },
          { "name": "FullDate", "dataType": "dateTime" },
          { "name": "CalendarYear", "dataType": "int64" }
        ]
      }
    ],
    "relationships": [
      { "fromTable": "Internet Sales", "fromColumn": "ProductKey",
        "toTable": "Product", "toColumn": "ProductKey" },
      { "fromTable": "Internet Sales", "fromColumn": "OrderDateKey",
        "toTable": "Date", "toColumn": "DateKey" }
    ]
  }
}
```

### Measure classifications

| Measure | Classification | Reason |
|---|---|---|
| `TotalSales` | simple | `SUM(col)` — direct Malloy |
| `OrderCount` | simple | `COUNTROWS` — Malloy `count()` |
| `ElectronicsSales` | simple | `CALCULATE + column filter` on joined dimension — Malloy `where:` on join (see conversion note below) |
| `YTD Revenue` | simple (with date dim) | Time intel modelled via date dimension `ytd_flag` |
| `CountryShare` | sql_complex | `CALCULATE + ALL` — needs DuckDB window function |
| `SalesRank` | sql_complex | `RANKX + ALL` — needs DuckDB `RANK() OVER` |
| `ComplexNested` | untranslatable | Nested CALCULATE with FILTER + KEEPFILTERS |

**Conversion note — `ElectronicsSales`:** The filter `Product[Category]="Electronics"` references
a joined dimension, not a fact-table column. For joined-dimension filters, Malloy's
`{ where: product.category = 'Electronics' }` is the correct approach — Malloy propagates
the filter through the join at query time. If the filter were on a fact-table column
(e.g., `Sales[Amount] > 1000`), the converter would emit a DuckDB generated column instead
(see Section 6).

### Output: `schema.sql` (DuckDB generated columns)

```sql
-- Generated columns for static CALCULATE filters on same-table columns.
-- Filters on joined dimensions (like Product.Category) use Malloy where: clauses
-- instead — no generated column is needed for those.

-- If a CALCULATE like SUM(Revenue) WHERE Amount > 1000 existed,
-- it would generate a column like:
-- ALTER TABLE internet_sales ADD COLUMN is_high_value BOOLEAN
--   GENERATED ALWAYS AS (amount > 1000);
```

### Output: `model.malloy`

```malloy
source: product is duckdb.table('product') extend {
  dimension: product_name is product_name
  dimension: category is category
  dimension: subcategory is subcategory
}

source: date_dim is duckdb.table('date_dim') extend {
  dimension: date_key is date_key
  dimension: full_date is full_date
  dimension: calendar_year is calendar_year
  dimension: ytd_flag is ytd_flag
}

source: internet_sales is duckdb.table('internet_sales') extend {
  join_one: product with product_key
  join_one: date_dim with order_date_key

  measure: total_sales is sales_amount.sum()
  measure: order_count is count()
  measure: electronics_sales is sales_amount.sum() { where: product.category = 'Electronics' }
  measure: ytd_revenue is sales_amount.sum() { where: date_dim.ytd_flag = true }
}
```

### Output: `proxy-config.json`

```jsonc
{
  "catalog": "ADVENTUREWORKS",
  "cube": "Internet Sales",
  "source_name": "internet_sales",
  "table_name": "internet_sales",
  "dialect": "duckdb",
  "malloy_model_file": "model.malloy",
  "db_path": null,
  "dimensions": [
    { "id": "ProductName", "malloy_name": "product_name",
      "physical_field": "product.product_name", "caption": "Product Name",
      "hierarchy_name": "Product Name", "all_level_name": "(All)",
      "leaf_level_name": "Product Name", "ordinal": 1,
      "visible": true, "has_all": true, "cardinality_hint": 500 },
    { "id": "Category", "malloy_name": "category",
      "physical_field": "product.category", "caption": "Category",
      "hierarchy_name": "Category", "all_level_name": "(All)",
      "leaf_level_name": "Category", "ordinal": 2,
      "visible": true, "has_all": true, "cardinality_hint": 20 },
    { "id": "Subcategory", "malloy_name": "subcategory",
      "physical_field": "product.subcategory", "caption": "Subcategory",
      "hierarchy_name": "Subcategory", "all_level_name": "(All)",
      "leaf_level_name": "Subcategory", "ordinal": 3,
      "visible": true, "has_all": true, "cardinality_hint": 50 },
    { "id": "FullDate", "malloy_name": "full_date",
      "physical_field": "date_dim.full_date", "caption": "Date",
      "hierarchy_name": "Date", "all_level_name": "(All)",
      "leaf_level_name": "Date", "ordinal": 4,
      "visible": true, "has_all": true, "cardinality_hint": 2000 }
  ],
  "measures": [
    { "id": "TotalSales", "type": "simple",
      "malloy_name": "total_sales", "physical_expr": "sales_amount.sum()",
      "sql_expr": "SUM(sales_amount)", "caption": "Total Sales",
      "display_name": "Total Sales (USD)", "format_string": "#,##0.00",
      "units": "USD", "ordinal": 1, "visible": true,
      "measure_group_name": "Sales" },
    { "id": "OrderCount", "type": "simple",
      "malloy_name": "order_count", "physical_expr": "count()",
      "sql_expr": "COUNT(*)", "caption": "Order Count",
      "display_name": "Order Count", "format_string": "#,##0",
      "units": "", "ordinal": 2, "visible": true,
      "measure_group_name": "Sales" },
    { "id": "ElectronicsSales", "type": "simple",
      "malloy_name": "electronics_sales", "physical_expr": "sales_amount.sum()",
      "sql_expr": "SUM(sales_amount)", "caption": "Electronics Sales",
      "display_name": "Electronics Sales (USD)", "format_string": "#,##0.00",
      "units": "USD", "ordinal": 3, "visible": true,
      "measure_group_name": "Sales" },
    { "id": "YTD Revenue", "type": "simple",
      "malloy_name": "ytd_revenue", "physical_expr": "sales_amount.sum()",
      "sql_expr": "SUM(sales_amount)", "caption": "YTD Revenue",
      "display_name": "YTD Revenue (USD)", "format_string": "#,##0.00",
      "units": "USD", "ordinal": 4, "visible": true,
      "measure_group_name": "Sales" },
    { "id": "CountryShare", "type": "sql_complex",
      "malloy_name": null, "physical_expr": null,
      "sql_expr": "complex_measures/country_share.sql",
      "caption": "Country Share", "display_name": "% by Country",
      "format_string": "0.00%", "units": "", "ordinal": 5,
      "visible": true, "measure_group_name": "Sales" },
    { "id": "SalesRank", "type": "sql_complex",
      "malloy_name": null, "physical_expr": null,
      "sql_expr": "complex_measures/sales_rank.sql",
      "caption": "Sales Rank", "display_name": "Rank by Product",
      "format_string": "#,##0", "units": "", "ordinal": 6,
      "visible": true, "measure_group_name": "Sales" },
    { "id": "ComplexNested", "type": "untranslatable",
      "malloy_name": null, "physical_expr": null, "sql_expr": null,
      "caption": "Complex Nested", "display_name": "Complex Nested",
      "description": "UNTRANSLATABLE: Nested CALCULATE with KEEPFILTERS. Original DAX: CALCULATE(SUM(Internet Sales[SalesAmount]), FILTER(ALL(Date), Date[CalendarYear]=2024), KEEPFILTERS(Product[Category]='Electronics')). Requires manual decomposition.",
      "format_string": "#,##0.00", "units": "", "ordinal": 99,
      "visible": false, "measure_group_name": "Sales" }
  ]
}
```

### Output: `complex_measures/country_share.sql`

```sql
-- Original DAX: DIVIDE(SUM(Internet Sales[SalesAmount]),
--                      CALCULATE(SUM(Internet Sales[SalesAmount]), ALL(Geography)))
-- Type: CALCULATE + ALL

SELECT
  SUM(sales_amount) AS total_revenue,
  SUM(sales_amount) / SUM(SUM(sales_amount)) OVER () AS country_share
FROM {internet_sales}
```

### Output: `complex_measures/sales_rank.sql`

```sql
-- Original DAX: RANKX(ALL(Product), SUM(Internet Sales[SalesAmount]))
-- Type: RANKX + ALL

SELECT
  product_key,
  product_name,
  SUM(sales_amount) AS total_sales,
  RANK() OVER (ORDER BY SUM(sales_amount) DESC) AS sales_rank
FROM {internet_sales}
GROUP BY product_key, product_name
```

### Notes for the user (log output)

```
Conversion of AdventureWorks.bim:
  7 measures found
  4 → simple (Malloy)
  2 → sql_complex (DuckDB SQL fallback)
  1 → untranslatable (manual rewrite required)

WARNING: YTD Revenue uses TOTALYTD — requires a date_dim table with ytd_flag column.
  Run scripts/create_date_dim.sql to generate the dimension table.

WARNING: ComplexNested is untranslatable — nested CALCULATE with KEEPFILTERS.
  See proxy-config.json for the original DAX expression.

INFO: Inactive relationships detected: none
INFO: Date columns referenced: OrderDateKey. Ensure date_dim is populated.
```

---

## Appendix A: Quick classification checklist for LLMs

When reading a `.bim` measure, check these patterns in order:

```
1. Is it SUM(col), COUNT(col), MIN(col), MAX(col), AVG(col)?
   → simple, Malloy

2. Is it COUNTROWS(table)?
   → simple, Malloy count()

3. Is it DISTINCTCOUNT(col)?
   → simple, Malloy count(distinct)

4. Is it CALCULATE(agg, Table[Col]="value") single static filter?
   → Suggest DuckDB generated column (Section 6) + simple Malloy { where: }
   → If the column is in a joined dimension: simple Malloy { where: join.col = 'value' }

5. Is it CALCULATE(agg, Table[Col]="A" || Table[Col]="B") OR on same column?
   → Suggest DuckDB generated column (Section 6) + simple Malloy { where: }

6. Is it DIVIDE(a, b, alt)?
   → simple if a and b are simple; handled in proxy rendering

7. Does it contain ANY of: ALL(), ALLEXCEPT(), FILTER(), KEEPFILTERS(), RANKX()?
   → sql_complex, DuckDB SQL

8. Does it contain ANY of: TOTALYTD, SAMEPERIODLASTYEAR, DATESYTD, DATESQTD?
   → simple IF date_dim exists; otherwise sql_complex

9. Does it contain SUMX, AVERAGEX, MAXX with a complex expression?
   → Check expression: if just Column * Column, it's simple (arithmetic in Malloy measure).
     If FILTER or RELATED inside, it's untranslatable.

10. Does it contain VAR / RETURN / nested CALCULATE?
    → untranslatable

11. DEFAULT: classify as sql_complex and warn
```

## Appendix B: DuckDB schema generation

The converter should also emit `CREATE TABLE` statements for the user to run:

```sql
-- Generated from AdventureWorks.bim
-- Run: duckdb mydata.db < schema.sql

CREATE TABLE IF NOT EXISTS internet_sales (
    sales_amount  DOUBLE NOT NULL,
    order_quantity BIGINT NOT NULL,
    product_key   BIGINT NOT NULL,
    order_date_key BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS product (
    product_key  BIGINT NOT NULL PRIMARY KEY,
    product_name VARCHAR NOT NULL,
    category     VARCHAR NOT NULL,
    subcategory  VARCHAR NOT NULL
);
```

The user is responsible for loading data into these tables.

---

## Appendix C: M query partitions

When a partition uses `source.type = "m"`, the converter cannot extract table
schema or load data. The converter should:

1. Skip the partition for schema extraction
2. Use the `.bim` column definitions for table schema instead
3. Warn: "Table X has M-query partitions — data loading must be done manually"
4. Still emit CREATE TABLE from column definitions
5. Still build the Malloy model — it just needs the data to exist
