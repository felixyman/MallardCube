-- Generated from Tabular Editor model
-- Data loading via M partitions must be done manually.

CREATE TABLE IF NOT EXISTS sales (
    customerid BIGINT,
    datekey BIGINT,
    deliverydays BIGINT,
    discountamount VARCHAR,
    grossvalue VARCHAR,
    discountapplied BIGINT,
    isreturn BIGINT,
    net VARCHAR,
    payment VARCHAR,
    productid BIGINT,
    promoid BIGINT,
    hour BIGINT,
    returnreason VARCHAR,
    channel VARCHAR,
    qty BIGINT,
    shipcost VARCHAR,
    shipweight VARCHAR,
    storeid BIGINT,
    taxamount VARCHAR,
    tax_rate VARCHAR,
    salesid BIGINT,
    unitprice VARCHAR
);
-- FACT TABLE: Sales

CREATE TABLE IF NOT EXISTS dates (
    fulldate TIMESTAMP,
    datekey BIGINT,
    isholiday BIGINT,
    isweekend BIGINT,
    monthname VARCHAR,
    monthnumber BIGINT,
    quartername VARCHAR,
    quarternumber BIGINT,
    weekdayname VARCHAR,
    weekdaynumber BIGINT,
    year BIGINT,
    yearmonth VARCHAR,
    yearmonthnumber BIGINT,
    yearquarter VARCHAR,
    yearquarternumber BIGINT,
    yearweek VARCHAR,
    yearweeknumber BIGINT
);

CREATE TABLE IF NOT EXISTS customers (
    age BIGINT,
    annualincome VARCHAR,
    childrencount BIGINT,
    city VARCHAR,
    customerid BIGINT,
    fullname VARCHAR,
    education VARCHAR,
    email VARCHAR,
    gender VARCHAR,
    hassubscription BIGINT,
    incomebracket VARCHAR,
    isactive BIGINT,
    lang VARCHAR,
    points BIGINT,
    loyaltysegment VARCHAR,
    tier VARCHAR,
    maritalstatus VARCHAR,
    preferredcontact VARCHAR,
    dayssincelastpurchase BIGINT,
    regdate TIMESTAMP,
    satisfactionscore VARCHAR,
    spendmultiplier VARCHAR,
    totalspend VARCHAR
);

CREATE TABLE IF NOT EXISTS products (
    brand VARCHAR,
    category VARCHAR,
    color VARCHAR,
    ecoscore BIGINT,
    haswarranty BIGINT,
    stockstatus VARCHAR,
    isactive BIGINT,
    isdiscontinued BIGINT,
    ecofriendly BIGINT,
    material VARCHAR,
    productid BIGINT,
    name VARCHAR,
    productrating VARCHAR,
    unitprice VARCHAR,
    releaseyear BIGINT,
    skucount BIGINT,
    minstock BIGINT,
    seasonalityfactor VARCHAR,
    supplierid BIGINT,
    margin_pct VARCHAR,
    tax_rate VARCHAR,
    unitcost VARCHAR,
    warrantymonths BIGINT,
    weight VARCHAR
);

CREATE TABLE IF NOT EXISTS promotions (
    budget VARCHAR,
    discount_pct VARCHAR,
    promoname VARCHAR,
    maxdiscountcap VARCHAR,
    enddate TIMESTAMP,
    discount_fixed VARCHAR,
    isactive BIGINT,
    isstackable BIGINT,
    channel VARCHAR,
    minspend BIGINT,
    type VARCHAR,
    promoupliftfactor VARCHAR,
    promoid BIGINT,
    redemption_rate VARCHAR,
    coderequired BIGINT,
    startdate TIMESTAMP,
    targetaudience VARCHAR
);

CREATE TABLE IF NOT EXISTS stores (
    annualrentcost VARCHAR,
    distancetocitycenterkm VARCHAR,
    staff BIGINT,
    floornumber BIGINT,
    hascafe BIGINT,
    hasdeliveryservice BIGINT,
    renovationyear BIGINT,
    openingyear BIGINT,
    parkingspots BIGINT,
    sizem2 BIGINT,
    storesizemultiplier VARCHAR,
    city VARCHAR,
    storeid BIGINT,
    storename VARCHAR,
    storerating VARCHAR,
    region VARCHAR,
    type VARCHAR
);


-- Calculated tables (see calculated_tables.sql)
