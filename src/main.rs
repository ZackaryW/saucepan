fn main() {
    match saucepan::cli::run_from(std::env::args_os()) {
        Ok(value) => println!("{value}"),
        Err(error) => {
            if let Some(error) = error.downcast_ref::<clap::Error>() {
                error.exit();
            }
            eprintln!("saucepan: {error:#}");
            std::process::exit(1);
        }
    }
}
