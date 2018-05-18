error_chain! {
    types {
        Error, ErrorKind, ResultExt, AuthError;
    }

    foreign_links {
    }

    errors {
        PlaceholderError(s: &'static str) {
            display("{}", s)
        }
    }
}
