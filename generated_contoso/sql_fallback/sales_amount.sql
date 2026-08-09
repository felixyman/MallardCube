-- SQL fallback for: Sales Amount
-- Original DAX: SUMX(Sales, Sales[Quantity] * Sales[Net Price])
-- Plain SUMX without RELATED — row-context multiplication
SELECT SUM(Quantity * NetPrice) AS value FROM sales
