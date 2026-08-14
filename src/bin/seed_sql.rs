fn main() {
    std::process::exit(mallardcube::tools::seed_sql::run(
        std::env::args().collect(),
    ));
}
