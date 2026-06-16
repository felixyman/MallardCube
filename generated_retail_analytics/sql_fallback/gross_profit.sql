-- Gross Profit: Total Revenue - Total COGS
-- DAX: [Total Revenue] - [Total COGS]
WITH revenue AS (
    SELECT COALESCE(SUM(CASE WHEN isreturn = 0 THEN CAST(net AS DOUBLE) ELSE 0 END), 0) AS val FROM sales
),
cogs AS (
    SELECT COALESCE(SUM(CASE WHEN s.isreturn = 0 THEN s.qty * CAST(p.unitcost AS DOUBLE) ELSE 0 END), 0) AS val
    FROM sales s JOIN products p ON s.productid = p.productid
)
SELECT revenue.val - cogs.val AS value FROM revenue, cogs
