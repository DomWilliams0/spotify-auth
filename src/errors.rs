use curl;
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
        SpotifyAPIError(s: String) {
            display("Spotify API is incompatible: {}", s)
            display("Spotify API is incompatible")
        }

        AuthenticationError(s: String) {
            display("{}", s)
            description("Authentication error"),
        }

        HttpError(code: u32) {
            display("HTTP response code {}", code)
            description("HTTP error"),
        }

        HttpErrorJson(code: u32, json: String) {
            display("HTTP error {}: {}", code, json),
            description("HTTP error"),
        }
    }
}
