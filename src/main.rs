fn main() {
    if let Err(error) = saucepan::cli::run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
