# Conversion Report — DATABASE

## Summary

- Fact table: Sales
- Dimensions: 8
- Date-role tables: 0
- Relationships: 6
- Measures: 31 (simple: 0, sql_fallback: 4, manual: 27)
- M-partition tables: 1 (load_data.sql attempts automated loading, see load_data.sql for details)

## Join Map

| Fact Column | Dimension Table | Join Column |
|---|---|---|
| CustomerKey | Customer | CustomerKey |
| Order Date | Date | Date |
| Delivery Date | Date | Date |
| ProductKey | Product | ProductKey |
| PromotionKey | Promotion | PromotionKey |
| StoreKey | Store | StoreKey |

## Simple measures

| Measure | DAX | SQL |
|---|---|---|

## SQL fallback measures

| Measure | DAX pattern | Fallback file |
|---|---|---|
| Cost | SUMX ( Sales, Sales[Quantity] * Sales[Unit Cost] ) | sql_fallback/cost.sql |
| Margin | [Sales Amount] - [Cost] | sql_fallback/margin.sql |
| Sales Amount | SUMX ( Sales, Sales[Quantity] * Sales[Net Price] ) | sql_fallback/sales_amount.sql |
| Sales Quantity | SUM ( Sales[Quantity] ) | sql_fallback/sales_quantity.sql |

## Manual review required

| Measure | DAX pattern |
|---|---|
| Cylinder | SELECTEDVALUE( State[State] ) |
| Faces | SELECTEDVALUE( State[State] ) |
| Five Bars Colored | SELECTEDVALUE( State[State] ) |
| Five Boxes Colored | SELECTEDVALUE( State[State] ) |
| Gauge | SELECTEDVALUE( State[State] ) |
| Gauge - Ascending | SELECTEDVALUE( State[State] ) |
| Gauge - Descending | SELECTEDVALUE( State[State] ) |
| Reversed Gauge | SELECTEDVALUE( State[State] ) |
| Reversed status arrow | SELECTEDVALUE( State[State] ) |
| Road Signs | SELECTEDVALUE( State[State] ) |
| Shapes | SELECTEDVALUE( State[State] ) |
| Smiley | SELECTEDVALUE( State[State] ) |
| Smiley Face | SELECTEDVALUE( State[State] ) |
| Standard Arrow | SELECTEDVALUE( State[State] ) |
| Status Arrow | SELECTEDVALUE( State[State] ) |
| Status Arrow - Ascending | SELECTEDVALUE( State[State] ) |
| Status Arrow - Descending | SELECTEDVALUE( State[State] ) |
| Thermometer | SELECTEDVALUE( State[State] ) |
| Three Circles Colored | SELECTEDVALUE( State[State] ) |
| Three Flags Colored | SELECTEDVALUE( State[State] ) |
| Three Stars Colored | SELECTEDVALUE( State[State] ) |
| Three Symbols Uncircled Colored | SELECTEDVALUE( State[State] ) |
| Three Triangles | SELECTEDVALUE( State[State] ) |
| Traffic Light | SELECTEDVALUE( State[State] ) |
| Traffic Light - Multiple | SELECTEDVALUE( State[State] ) |
| Traffic Light - Single | SELECTEDVALUE( State[State] ) |
| Variance Arrow | SELECTEDVALUE( State[State] ) |

## Data loading

The converter generates three SQL files for data loading:

- `schema.sql` — CREATE TABLE statements (run first)
- `load_data.sql` — loads real data from source databases (requires DuckDB extensions or CSV files)
- `load_dummy_data.sql` — generates synthetic data for testing (always works)

### Quick start

```
duckdb data/sales.db < bootstrap.sql
```

This creates the schema, seeds `date_dim` (if needed), and loads dummy data.
For real data, edit `bootstrap.sql` to use `load_data.sql` instead.

### Tables to load

- [ ] `sales` (fact)
- [ ] `customer` (lookup)
- [ ] `date` (lookup)
- [ ] `metric` (lookup)
- [ ] `product` (lookup)
- [ ] `promotion` (lookup)
- [ ] `store` (lookup)
- [ ] `time_intelligence` (lookup)
- [ ] `info` (lookup)

## Roles

1 roles detected

### Role 1 (read)

**Members:**

| Name | Type |
|---|---|
| phil@contoso.com |  |
| analysts@contoso.com | group |

**Table permissions:**

| Table | SQL filter | DAX filter | Metadata permission | Status |
|---|---|---|---|---|
| Customer | (empty) | - | read | No filter — full access |
| Product | (empty) | - | none | OLS — table hidden |
| Date | (empty) | [Calendar Year] > 2009 | read | DAX filter preserved, SQL filter empty — manual SQL translation required |

