fn main() {
    std::process::exit(mallardcube::tools::trace_replay::run(
        std::env::args().collect(),
    ));
}
