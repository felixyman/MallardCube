use xmla_proxy::backend::generate_sales_fact_rows;

fn main() {
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
    println!("-- {} rows", rows.len());
    println!();

    // Batch INSERT with multiple VALUES rows for speed
    for chunk in rows.chunks(500) {
        let values: Vec<String> = chunk.iter().map(|r| {
            format!(
                "('{}', '{}', '{}', '{}', {}, {})",
                r.category, r.territory, r.channel, r.segment,
                r.revenue, r.units
            )
        }).collect();
        println!("INSERT INTO sales_fact VALUES");
        println!("{};", values.join(",\n"));
        println!();
    }

    eprintln!("Generated {} INSERT statements (chunks of 500 rows)", rows.len());
}
