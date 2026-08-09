-- SQL fallback for: Sales Quantity
-- Original DAX: SUM(Sales[Quantity])
-- Simple SUM over the sales fact table
SELECT SUM(Quantity) AS value FROM sales
