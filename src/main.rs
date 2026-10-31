fn main() {
    // Clap's derive parsing of the full command tree needs more than the
    // 1 MiB Windows main-thread stack in debug builds.
    let cli = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(saucepan::cli::run)
        .expect("spawn CLI thread");
    if let Err(error) = cli
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    {
        saucepan::cli::report_error(&error);
        std::process::exit(1);
    }
}
