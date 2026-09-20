-- Upstream schema for the "orders" demo (plan 044, generic example).
--
-- Everything a semantic layer would own lives upstream: conformed dimensions,
-- additive fact columns, and (in marts.sql) per-grain aggregate marts. The
-- proxy-config.yaml next to this file is a thin projection of these tables —
-- no fallback SQL, no DAX, no measure logic in the proxy.

CREATE TABLE dim_customer (
    customer_key INTEGER PRIMARY KEY,
    customer_name VARCHAR NOT NULL,
    region VARCHAR NOT NULL,
    segment VARCHAR NOT NULL
);

CREATE TABLE dim_product (
    product_key INTEGER PRIMARY KEY,
    product_name VARCHAR NOT NULL,
    category VARCHAR NOT NULL
);

CREATE TABLE dim_date (
    date_key INTEGER PRIMARY KEY,
    full_date DATE NOT NULL,
    year INTEGER NOT NULL,
    quarter INTEGER NOT NULL,
    month INTEGER NOT NULL,
    month_name VARCHAR NOT NULL,
    week INTEGER NOT NULL,
    -- Time-intelligence flags are upstream columns; the proxy only reads them.
    ytd_flag BOOLEAN NOT NULL,
    qtd_flag BOOLEAN NOT NULL,
    mtd_flag BOOLEAN NOT NULL,
    prior_year_ytd_flag BOOLEAN NOT NULL,
    current_year_flag BOOLEAN NOT NULL
);

CREATE TABLE fact_orders (
    order_id INTEGER PRIMARY KEY,
    order_date_key INTEGER NOT NULL,
    customer_key INTEGER NOT NULL,
    product_key INTEGER NOT NULL,
    status VARCHAR NOT NULL,
    -- Additive columns: counts are 0/1 flags, durations are sum/count pairs.
    is_order INTEGER NOT NULL,
    is_open INTEGER NOT NULL,
    is_closed INTEGER NOT NULL,
    is_cancelled INTEGER NOT NULL,
    is_late INTEGER NOT NULL,
    is_within_sla INTEGER NOT NULL,
    lead_time_days INTEGER,           -- NULL while an order is still open
    sum_lead_time DOUBLE NOT NULL,    -- lead_time_days when known, else 0
    count_lead_time INTEGER NOT NULL, -- 1 when known, else 0
    quantity INTEGER NOT NULL,
    revenue DOUBLE NOT NULL
);
