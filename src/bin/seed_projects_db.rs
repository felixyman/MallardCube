fn main() {
    std::process::exit(mallardcube::tools::seed_projects_db::run(
        std::env::args().collect(),
    ));
}
