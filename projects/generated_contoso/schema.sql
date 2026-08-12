-- Generated from Tabular Editor model
-- Data loading via M partitions must be done manually.

CREATE TABLE IF NOT EXISTS sales (
    currencykey BIGINT,
    customerkey BIGINT,
    delivery_date TIMESTAMP,
    grossmargin DOUBLE,
    grossmarginpct DOUBLE,
    net_price DOUBLE,
    order_date TIMESTAMP,
    order_line_number BIGINT,
    order_number VARCHAR,
    orderdatekey BIGINT,
    productkey BIGINT,
    promotionkey BIGINT,
    quantity BIGINT,
    salesamount DOUBLE,
    storekey BIGINT,
    totalproductcost DOUBLE,
    unit_cost DOUBLE,
    unit_discount DOUBLE,
    unit_price DOUBLE
);
-- FACT TABLE: Sales

CREATE TABLE IF NOT EXISTS customer (
    address_line_1 VARCHAR,
    address_line_2 VARCHAR,
    birth_date TIMESTAMP,
    cars_owned BIGINT,
    children_at_home BIGINT,
    city VARCHAR,
    company_name VARCHAR,
    continent VARCHAR,
    countryregion VARCHAR,
    customer_code VARCHAR,
    customer_type VARCHAR,
    customerkey BIGINT,
    date_first_purchase TIMESTAMP,
    education VARCHAR,
    gender VARCHAR,
    house_ownership VARCHAR,
    marital_status VARCHAR,
    name VARCHAR,
    occupation VARCHAR,
    phone VARCHAR,
    state VARCHAR,
    title VARCHAR,
    total_children BIGINT,
    yearly_income DOUBLE
);

CREATE TABLE IF NOT EXISTS date (
    asia_season VARCHAR,
    calendar_year VARCHAR,
    calendar_year_month VARCHAR,
    calendar_year_month_number BIGINT,
    calendar_year_number BIGINT,
    calendar_year_quarter VARCHAR,
    calendar_year_quarter_number BIGINT,
    date TIMESTAMP,
    datekey BIGINT,
    day_of_week VARCHAR,
    day_of_week_number BIGINT,
    europe_season VARCHAR,
    fiscal_month VARCHAR,
    fiscal_month_number BIGINT,
    fiscal_quarter VARCHAR,
    fiscal_quarter_number BIGINT,
    fiscal_year VARCHAR,
    fiscal_year_number BIGINT,
    fiscal_year_quarter VARCHAR,
    fiscal_year_quarter_number BIGINT,
    holiday_name VARCHAR,
    is_holiday BOOLEAN,
    month VARCHAR,
    month_number BIGINT,
    north_america_season VARCHAR,
    working_day VARCHAR
);

CREATE TABLE IF NOT EXISTS metric (
    name VARCHAR,
    ordinal BIGINT
);

CREATE TABLE IF NOT EXISTS product (
    brand VARCHAR,
    category VARCHAR,
    color VARCHAR,
    manufacturer VARCHAR,
    product_code VARCHAR,
    product_name VARCHAR,
    productkey BIGINT,
    subcategory VARCHAR
);

CREATE TABLE IF NOT EXISTS promotion (
    discount DOUBLE,
    end_date TIMESTAMP,
    promotion VARCHAR,
    promotion_category VARCHAR,
    promotion_code VARCHAR,
    promotion_type VARCHAR,
    promotionkey BIGINT,
    start_date TIMESTAMP
);

CREATE TABLE IF NOT EXISTS store (
    address_line_1 VARCHAR,
    address_line_2 VARCHAR,
    city VARCHAR,
    close_date TIMESTAMP,
    close_reason VARCHAR,
    continent VARCHAR,
    countryregion VARCHAR,
    employees BIGINT,
    last_remodel_date TIMESTAMP,
    open_date TIMESTAMP,
    selling_area DOUBLE,
    state VARCHAR,
    status VARCHAR,
    store_name VARCHAR,
    store_phone VARCHAR,
    store_type VARCHAR,
    storekey BIGINT,
    zip_code VARCHAR,
    zip_code_extension VARCHAR
);

CREATE TABLE IF NOT EXISTS time_intelligence (
    ordinal BIGINT,
    name VARCHAR
);

CREATE TABLE IF NOT EXISTS info (
    label VARCHAR,
    text VARCHAR,
    timestamp TIMESTAMP
);


-- Calculated tables (see calculated_tables.sql)
