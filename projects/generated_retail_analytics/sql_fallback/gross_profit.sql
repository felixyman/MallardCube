-- Auto-generated from DAX: [Total Revenue] - [Total COGS]
-- Arithmetic between measures

SELECT COALESCE((-- Auto-generated from DAX: CALCULATE(SUM('Sales'[Net Sales(Revenue)]), 'Sales'[Is Return] = 0)
-- CALCULATE(SUM(col), filter)

SELECT COALESCE(SUM(CAST(net AS DOUBLE)), 0) AS value
FROM sales
WHERE isreturn = 0), 0) - COALESCE((-- Auto-generated from DAX: SUMX(FILTER('Sales', 'Sales'[Is Return] = 0), 'Sales'[Sales Quantity] * RELATED('Products'[Unit Cost]))
-- SUMX(FILTER(...), qty * RELATED(dim.col))

SELECT COALESCE(SUM(f.qty * CAST(d.unitcost AS DOUBLE)), 0) AS value
FROM sales f
JOIN products d ON f.productid = d.productid
WHERE f.isreturn = 0), 0) AS value;
