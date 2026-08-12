-- Auto-generated from DAX: CALCULATE(SUM('Sales'[Net Sales(Revenue)]), 'Sales'[Is Return] = 0)
-- CALCULATE(SUM(col), filter)

SELECT COALESCE(SUM(CAST(net AS DOUBLE)), 0) AS value
FROM sales
WHERE isreturn = 0;
