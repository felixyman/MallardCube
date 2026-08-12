fn main() {
    std::process::exit(xmla_proxy::tools::seed_projects_db::run(
        std::env::args().collect(),
    ));
}
