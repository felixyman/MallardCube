fn main() {
    std::process::exit(mallardcube::tools::seed_generated_db::run(
        std::env::args().collect(),
    ));
}
