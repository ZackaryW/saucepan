fn main() {
    match saucepan::core::launch::shim_main() {
        Ok(status) => std::process::exit(status),
        Err(error) => {
            eprintln!("saucepan-shim: {error:#}");
            std::process::exit(1);
        }
    }
}
