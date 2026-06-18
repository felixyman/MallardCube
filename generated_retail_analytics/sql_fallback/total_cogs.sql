-- Auto-generated from DAX: SUMX(FILTER('Sales', 'Sales'[Is Return] = 0), 'Sales'[Sales Quantity] * RELATED('Products'[Unit Cost]))
-- SUMX(FILTER(...), qty * RELATED(dim.col))

SELECT COALESCE(SUM(f.qty * CAST(d.unitcost AS DOUBLE)), 0) AS value
FROM sales f
JOIN products d ON f.productid = d.productid
WHERE f.isreturn = 0;
