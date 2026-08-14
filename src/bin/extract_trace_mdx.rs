fn main() {
    std::process::exit(mallardcube::tools::extract_trace_mdx::run(
        std::env::args().collect(),
    ));
}
