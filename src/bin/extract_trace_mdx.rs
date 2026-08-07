fn main() {
    std::process::exit(xmla_proxy::tools::extract_trace_mdx::run(
        std::env::args().collect(),
    ));
}
