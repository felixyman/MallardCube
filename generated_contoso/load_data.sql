-- Data loading script
-- Generated from Tabular Editor model: DATABASE
-- Requires DuckDB extensions for external database sources

-- Empty M expression for table 'Sales', manual load required

-- Table: customer (M partition, CSV source)
-- CSV source: relative path 'pbi-tools/contoso-sales-model/main/data/Customer.csv'
-- The CSV is served from a Web.Contents source with a parameterized URL that cannot be resolved.
-- If you have a local copy of the CSV files, use:
-- INSERT INTO customer SELECT * FROM read_csv_auto('path/to/Customer.csv');

-- Table: date (M partition, CSV source)
-- CSV source: relative path 'pbi-tools/contoso-sales-model/main/data/Date.csv'
-- The CSV is served from a Web.Contents source with a parameterized URL that cannot be resolved.
-- If you have a local copy of the CSV files, use:
-- INSERT INTO date SELECT * FROM read_csv_auto('path/to/Date.csv');

-- Calculated table 'Metric' is computed by DAX, not loaded

-- Table: product (M partition, CSV source)
-- CSV source: relative path 'pbi-tools/contoso-sales-model/main/data/Product.csv'
-- The CSV is served from a Web.Contents source with a parameterized URL that cannot be resolved.
-- If you have a local copy of the CSV files, use:
-- INSERT INTO product SELECT * FROM read_csv_auto('path/to/Product.csv');

-- Table: promotion (M partition, CSV source)
-- CSV source: relative path 'pbi-tools/contoso-sales-model/main/data/Promotion.csv'
-- The CSV is served from a Web.Contents source with a parameterized URL that cannot be resolved.
-- If you have a local copy of the CSV files, use:
-- INSERT INTO promotion SELECT * FROM read_csv_auto('path/to/Promotion.csv');

-- Table: store (M partition, CSV source)
-- CSV source: relative path 'pbi-tools/contoso-sales-model/main/data/Store.csv'
-- The CSV is served from a Web.Contents source with a parameterized URL that cannot be resolved.
-- If you have a local copy of the CSV files, use:
-- INSERT INTO store SELECT * FROM read_csv_auto('path/to/Store.csv');

-- Calculated table 'Time Intelligence' is computed by DAX, not loaded

-- Table: info (M partition, unrecognized)
-- Unrecognized M expression, manual load required
-- M expression (first 200 chars): let Source = #table(type table [ #"Label"             = text, #"Timestamp"         = datetime, #"Text"              = text ], { { "Data Updated", DateTimeZone.RemoveZone(DateTimeZone.UtcNow()), null }...

