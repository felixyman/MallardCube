fn main() {
    std::process::exit(xmla_proxy::tools::load_replay::run(
        std::env::args().collect(),
    ));
}
