-- SQL fallback for: Margin
-- Original DAX: [Sales Amount] - [Cost]
-- Measure arithmetic — two subqueries
SELECT (SELECT SUM(Quantity * NetPrice) FROM sales) - (SELECT SUM(Quantity * UnitCost) FROM sales) AS value
