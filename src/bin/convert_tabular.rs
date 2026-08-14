fn main() {
    std::process::exit(mallardcube::tools::convert_tabular::run(
        std::env::args().collect(),
    ));
}
