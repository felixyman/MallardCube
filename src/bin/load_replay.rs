fn main() {
    std::process::exit(mallardcube::tools::load_replay::run(
        std::env::args().collect(),
    ));
}
