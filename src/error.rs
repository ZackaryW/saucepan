macro_rules! typed_error {
    ($name:ident) => {
        #[derive(Debug)]
        pub struct $name(pub String);
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for $name {}
    };
}

typed_error!(NotFound);    // exit 1
typed_error!(SourceError); // exit 2
typed_error!(ConfigError); // exit 3
typed_error!(Conflict);    // exit 4
