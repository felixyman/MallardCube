-- Gross Margin %: Gross Profit / Total Revenue (0 if revenue is 0)
-- DAX: DIVIDE([Gross Profit], [Total Revenue], 0)
WITH revenue AS (
    SELECT COALESCE(SUM(CASE WHEN isreturn = 0 THEN CAST(net AS DOUBLE) ELSE 0 END), 0) AS val FROM sales
),
cogs AS (
    SELECT COALESCE(SUM(CASE WHEN s.isreturn = 0 THEN s.qty * CAST(p.unitcost AS DOUBLE) ELSE 0 END), 0) AS val
    FROM sales s JOIN products p ON s.productid = p.productid
),
profit AS (
    SELECT revenue.val - cogs.val AS val FROM revenue, cogs
)
SELECT CASE WHEN revenue.val = 0 THEN 0 ELSE profit.val / revenue.val END AS value FROM revenue, profit
