# Conversion Report — SEMANTICMODEL

## Summary

- Fact table: Sales
- Dimensions: 5
- Date-role tables: 1
- Relationships: 5
- Measures: 4 (simple: 0, sql_fallback: 4, manual: 0)
- M-partition tables: 0 (all must be loaded manually)

## Join Map

| Fact Column | Dimension Table | Join Column |
|---|---|---|
| Date Key | Dates | Date Key |
| Store ID | Stores | Store ID |
| Product ID | Products | Product ID |
| Promotion ID | Promotions | Promotion ID |
| Customer ID | Customers | Customer ID |

## Simple measures (Malloy)

| Measure | DAX | Malloy |
|---|---|---|

## SQL fallback measures

| Measure | DAX pattern | Fallback file |
|---|---|---|
| Gross Margin % |  = DIVIDE ( [Gross Profit], [Total Revenue], 0 ) // Gross margin as a percentage (profit / revenue) | sql_fallback/gross_margin_%.sql |
| Gross Profit |  = [Total Revenue] - [Total COGS] // Gross profit = revenue - COGS | sql_fallback/gross_profit.sql |
| Total COGS |  = SUMX ( FILTER ( 'Sales', 'Sales'[Is Return] = 0 ), 'Sales'[Sales Quantity] * RELATED ( 'Products'[Unit Cost] ) ) // Cost of Goods Sold (COGS) for non-return transactions | sql_fallback/total_cogs.sql |
| Total Revenue |  = CALCULATE ( SUM ( 'Sales'[Net Sales (Revenue)] ), 'Sales'[Is Return] = 0 ) // Total revenue excluding returns | sql_fallback/total_revenue.sql |

## Data loading checklist

All tables use M (Power Query) partitions and must be loaded into DuckDB manually.

**Quick start (with date-dimension bootstrap):**

```
duckdb data/sales.db < bootstrap.sql
```

This creates the schema, seeds a populated `date_dim` calendar table, and
sets `db_path` in `proxy-config.json`. Then load your own data into the
listed tables below.

Run `schema.sql` to create the tables, then load data via:

- DuckDB CLI: `INSERT INTO ... SELECT ... FROM 'source.csv'`
- Or export your SSAS source to Parquet/CSV and import into DuckDB.

### Tables to load

- [ ] `sales` (fact)
- [ ] `dates` (date-role)
- [ ] `customers` (lookup)
- [ ] `products` (lookup)
- [ ] `promotions` (lookup)
- [ ] `stores` (lookup)
