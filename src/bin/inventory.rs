fn main() {
    std::process::exit(xmla_proxy::tools::inventory::run(
        std::env::args().collect(),
    ));
}
