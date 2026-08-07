use crate::backend::{generate_inventory_fact_rows, generate_sales_fact_rows};

pub fn run(_args: Vec<String>) -> i32 {
    println!("CREATE TABLE sales_fact (");
    println!("    category   VARCHAR NOT NULL,");
    println!("    territory  VARCHAR NOT NULL,");
    println!("    channel    VARCHAR NOT NULL,");
    println!("    segment    VARCHAR NOT NULL,");
    println!("    revenue    DOUBLE NOT NULL,");
    println!("    units      DOUBLE NOT NULL");
    println!(");");
    println!();
    let rows = generate_sales_fact_rows();
    eprintln!("sales_fact: {} rows", rows.len());
    for chunk in rows.chunks(500) {
        let values: Vec<String> = chunk
            .iter()
            .map(|r| {
                format!(
                    "('{}', '{}', '{}', '{}', {}, {})",
                    r.category, r.territory, r.channel, r.segment, r.revenue, r.units
                )
            })
            .collect();
        println!("INSERT INTO sales_fact VALUES");
        println!("{};", values.join(",\n"));
        println!();
    }

    println!("CREATE TABLE inventory_fact (");
    println!("    category   VARCHAR NOT NULL,");
    println!("    territory  VARCHAR NOT NULL,");
    println!("    warehouse  VARCHAR NOT NULL,");
    println!("    stock_qty  DOUBLE NOT NULL,");
    println!("    stock_cost DOUBLE NOT NULL");
    println!(");");
    println!();
    let rows = generate_inventory_fact_rows();
    eprintln!("inventory_fact: {} rows", rows.len());
    for chunk in rows.chunks(500) {
        let values: Vec<String> = chunk
            .iter()
            .map(|r| {
                format!(
                    "('{}', '{}', '{}', {}, {})",
                    r.category, r.territory, r.warehouse, r.stock_qty, r.stock_cost
                )
            })
            .collect();
        println!("INSERT INTO inventory_fact VALUES");
        println!("{};", values.join(",\n"));
        println!();
    }
    0
}
