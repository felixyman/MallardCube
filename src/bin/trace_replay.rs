fn main() {
    std::process::exit(xmla_proxy::tools::trace_replay::run(
        std::env::args().collect(),
    ));
}
