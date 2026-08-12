-- Dummy data generation script
-- Generated from Tabular Editor model: DATABASE
-- Fact tables: 10000 rows, Dimension tables: 1000 rows
-- Date tables: use seed_date_dim.sql instead

-- Table: sales (fact, 10000 rows)
INSERT INTO sales (currencykey, customerkey, delivery_date, grossmargin, grossmarginpct, net_price, order_date, order_line_number, order_number, orderdatekey, productkey, promotionkey, quantity, salesamount, storekey, totalproductcost, unit_cost, unit_discount, unit_price)
SELECT
    i AS currencykey,
    (i % 1000) + 1 AS customerkey,
    TIMESTAMP '2020-01-01 00:00:00' + (i % 365) * INTERVAL '1 day' AS delivery_date,
    round((random() * 1000)::DECIMAL(10,2), 2) AS grossmargin,
    round((random() * 1000)::DECIMAL(10,2), 2) AS grossmarginpct,
    round((random() * 1000)::DECIMAL(10,2), 2) AS net_price,
    TIMESTAMP '2020-01-01 00:00:00' + (i % 365) * INTERVAL '1 day' AS order_date,
    i AS order_line_number,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS order_number,
    i AS orderdatekey,
    (i % 1000) + 1 AS productkey,
    (i % 1000) + 1 AS promotionkey,
    i AS quantity,
    round((random() * 1000)::DECIMAL(10,2), 2) AS salesamount,
    (i % 1000) + 1 AS storekey,
    round((random() * 1000)::DECIMAL(10,2), 2) AS totalproductcost,
    round((random() * 1000)::DECIMAL(10,2), 2) AS unit_cost,
    round((random() * 1000)::DECIMAL(10,2), 2) AS unit_discount,
    round((random() * 1000)::DECIMAL(10,2), 2) AS unit_price
FROM generate_series(1, 10000) t(i);

-- Table: customer (dimension, 1000 rows)
INSERT INTO customer (address_line_1, address_line_2, birth_date, cars_owned, children_at_home, city, company_name, continent, countryregion, customer_code, customer_type, customerkey, date_first_purchase, education, gender, house_ownership, marital_status, name, occupation, phone, state, title, total_children, yearly_income)
SELECT
    'Item_' || lpad(i::VARCHAR, 4, '0') AS address_line_1,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS address_line_2,
    TIMESTAMP '2020-01-01 00:00:00' + (i % 365) * INTERVAL '1 day' AS birth_date,
    i AS cars_owned,
    i AS children_at_home,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS city,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS company_name,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS continent,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS countryregion,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS customer_code,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS customer_type,
    i AS customerkey,
    TIMESTAMP '2020-01-01 00:00:00' + (i % 365) * INTERVAL '1 day' AS date_first_purchase,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS education,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS gender,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS house_ownership,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS marital_status,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS name,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS occupation,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS phone,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS state,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS title,
    i AS total_children,
    round((random() * 1000)::DECIMAL(10,2), 2) AS yearly_income
FROM generate_series(1, 1000) t(i);

-- Table: date (dimension, 1000 rows)
INSERT INTO date (asia_season, calendar_year, calendar_year_month, calendar_year_month_number, calendar_year_number, calendar_year_quarter, calendar_year_quarter_number, date, datekey, day_of_week, day_of_week_number, europe_season, fiscal_month, fiscal_month_number, fiscal_quarter, fiscal_quarter_number, fiscal_year, fiscal_year_number, fiscal_year_quarter, fiscal_year_quarter_number, holiday_name, is_holiday, month, month_number, north_america_season, working_day)
SELECT
    'Item_' || lpad(i::VARCHAR, 4, '0') AS asia_season,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS calendar_year,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS calendar_year_month,
    i AS calendar_year_month_number,
    i AS calendar_year_number,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS calendar_year_quarter,
    i AS calendar_year_quarter_number,
    TIMESTAMP '2020-01-01 00:00:00' + (i % 365) * INTERVAL '1 day' AS date,
    i AS datekey,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS day_of_week,
    i AS day_of_week_number,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS europe_season,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS fiscal_month,
    i AS fiscal_month_number,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS fiscal_quarter,
    i AS fiscal_quarter_number,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS fiscal_year,
    i AS fiscal_year_number,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS fiscal_year_quarter,
    i AS fiscal_year_quarter_number,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS holiday_name,
    i % 2 = 0 AS is_holiday,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS month,
    i AS month_number,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS north_america_season,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS working_day
FROM generate_series(1, 1000) t(i);

-- Calculated table 'Metric' is computed by DAX, not loaded

-- Table: product (dimension, 1000 rows)
INSERT INTO product (brand, category, color, manufacturer, product_code, product_name, productkey, subcategory)
SELECT
    'Item_' || lpad(i::VARCHAR, 4, '0') AS brand,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS category,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS color,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS manufacturer,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS product_code,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS product_name,
    i AS productkey,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS subcategory
FROM generate_series(1, 1000) t(i);

-- Table: promotion (dimension, 1000 rows)
INSERT INTO promotion (discount, end_date, promotion, promotion_category, promotion_code, promotion_type, promotionkey, start_date)
SELECT
    round((random() * 1000)::DECIMAL(10,2), 2) AS discount,
    TIMESTAMP '2020-01-01 00:00:00' + (i % 365) * INTERVAL '1 day' AS end_date,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS promotion,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS promotion_category,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS promotion_code,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS promotion_type,
    i AS promotionkey,
    TIMESTAMP '2020-01-01 00:00:00' + (i % 365) * INTERVAL '1 day' AS start_date
FROM generate_series(1, 1000) t(i);

-- Table: store (dimension, 1000 rows)
INSERT INTO store (address_line_1, address_line_2, city, close_date, close_reason, continent, countryregion, employees, last_remodel_date, open_date, selling_area, state, status, store_name, store_phone, store_type, storekey, zip_code, zip_code_extension)
SELECT
    'Item_' || lpad(i::VARCHAR, 4, '0') AS address_line_1,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS address_line_2,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS city,
    TIMESTAMP '2020-01-01 00:00:00' + (i % 365) * INTERVAL '1 day' AS close_date,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS close_reason,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS continent,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS countryregion,
    i AS employees,
    TIMESTAMP '2020-01-01 00:00:00' + (i % 365) * INTERVAL '1 day' AS last_remodel_date,
    TIMESTAMP '2020-01-01 00:00:00' + (i % 365) * INTERVAL '1 day' AS open_date,
    round((random() * 1000)::DECIMAL(10,2), 2) AS selling_area,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS state,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS status,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS store_name,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS store_phone,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS store_type,
    i AS storekey,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS zip_code,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS zip_code_extension
FROM generate_series(1, 1000) t(i);

-- Calculated table 'Time Intelligence' is computed by DAX, not loaded

-- Table: info (dimension, 1000 rows)
INSERT INTO info (label, text, timestamp)
SELECT
    'Item_' || lpad(i::VARCHAR, 4, '0') AS label,
    'Item_' || lpad(i::VARCHAR, 4, '0') AS text,
    TIMESTAMP '2020-01-01 00:00:00' + (i % 365) * INTERVAL '1 day' AS timestamp
FROM generate_series(1, 1000) t(i);

