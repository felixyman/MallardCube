MODEL (
  name main.fact_sales,
  kind FULL,
  grain (l_orderkey, l_linenumber),
  columns (
    l_orderkey BIGINT,
    l_linenumber INTEGER,
    customer_key BIGINT,
    part_key BIGINT,
    supplier_key BIGINT,
    nation_key BIGINT,
    order_date_key INTEGER,
    l_quantity DECIMAL(15,2),
    l_extendedprice DECIMAL(15,2),
    l_discount DECIMAL(15,2),
    net_revenue DECIMAL(18,2),
    is_line INTEGER
  ),
  audits (
    not_null(columns := (l_orderkey, l_linenumber, customer_key, part_key, supplier_key,
                         nation_key, order_date_key, l_quantity, l_extendedprice, l_discount,
                         net_revenue, is_line)),
    unique_combination_of_columns(columns := (l_orderkey, l_linenumber)),
    relationships(column := customer_key, reference := main.dim_customer,
                  reference_column := c_custkey),
    relationships(column := part_key, reference := main.dim_part,
                  reference_column := p_partkey),
    relationships(column := supplier_key, reference := main.dim_supplier,
                  reference_column := s_suppkey),
    relationships(column := nation_key, reference := main.dim_nation,
                  reference_column := n_nationkey),
    relationships(column := order_date_key, reference := main.dim_date,
                  reference_column := date_key)
  )
);

-- A conformed additive fact: every measure column is a plain SUM upstream.
SELECT l.l_orderkey,
       l.l_linenumber,
       o.o_custkey AS customer_key,
       l.l_partkey AS part_key,
       l.l_suppkey AS supplier_key,
       c.c_nationkey AS nation_key,
       CAST(strftime(o.o_orderdate, '%Y%m%d') AS INTEGER) AS order_date_key,
       l.l_quantity,
       l.l_extendedprice,
       l.l_discount,
       l.l_extendedprice * (1 - l.l_discount) AS net_revenue,
       1 AS is_line
FROM raw.lineitem AS l
JOIN raw.orders AS o ON l.l_orderkey = o.o_orderkey
JOIN raw.customer AS c ON o.o_custkey = c.c_custkey;
