mod cli;

fn main() -> std::process::ExitCode {
    match cli::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}", error.message());
            std::process::ExitCode::from(error.exit_code() as u8)
        }
    }
}
