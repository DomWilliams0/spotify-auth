use curl;
use json;
use std::{io, string};

error_chain! {
    types {
        Error, ErrorKind, ResultExt, AuthError;
    }

    foreign_links {
        Curl(curl::Error);
        Utf8(string::FromUtf8Error);
        Io(io::Error);
    }

    errors {
        PlaceholderError(s: &'static str) {
            display("{}", s)
        }

        HttpError(code: u32) {
            display("HTTP response code {}", code)
        }

        HttpErrorJson(code: u32, json: json::JsonValue) {
            display("HTTP response code {}", code)
        }
    }
}
