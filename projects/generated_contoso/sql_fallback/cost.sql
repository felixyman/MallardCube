-- SQL fallback for: Cost
-- Original DAX: SUMX(Sales, Sales[Quantity] * Sales[Unit Cost])
-- Plain SUMX without RELATED — row-context multiplication
SELECT SUM(Quantity * UnitCost) AS value FROM sales
