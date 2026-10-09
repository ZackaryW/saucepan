fn main() {
    if let Err(error) = saucepan::cli::run() {
        saucepan::cli::report_error(&error);
        std::process::exit(1);
    }
}
