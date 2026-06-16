fn main() {
    std::process::exit(xmla_proxy::tools::seed_sql::run(std::env::args().collect()));
}
