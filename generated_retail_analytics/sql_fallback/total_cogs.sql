-- Total COGS: Cost of Goods Sold for non-return transactions
-- DAX: SUMX(FILTER('Sales', 'Sales'[Is Return] = 0), 'Sales'[Sales Quantity] * RELATED('Products'[Unit Cost]))
SELECT COALESCE(SUM(CASE WHEN s.isreturn = 0 THEN s.qty * CAST(p.unitcost AS DOUBLE) ELSE 0 END), 0) AS value
FROM sales s JOIN products p ON s.productid = p.productid
