fn main() {
    std::process::exit(mallardcube::tools::inventory::run(
        std::env::args().collect(),
    ));
}
