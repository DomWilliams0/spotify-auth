#![allow(dead_code)]

extern crate chrono;
extern crate curl;
extern crate json;
extern crate querystring;
extern crate webbrowser;

#[macro_use]
extern crate error_chain;

mod errors;
mod request;
mod types;

use types::*;

use errors::*;

#[derive(Debug)]
struct SpotifyAuth<State> {
    client_id: ClientId,
    callback_port: u16,
    state: State,
}

//  no auth code, no nothing
#[derive(Debug)]
struct Unauthenticated;

// waiting for the user to login in their browser and
// enter the url they are forwarded to
// TODO do this automatically
// #[derive(Debug)]
// struct Authenticating;

// user has logged in
#[derive(Debug)]
struct Authenticated {
    auth_code: AuthCode,
}

// access token has been fetched, and the api can be used
#[derive(Debug)]
struct TokenBearing {
    auth_code: AuthCode,
    access: Token,
    refresh: Token,
    expiry_time: ExpiryTime,
}

// transitions

impl SpotifyAuth<Unauthenticated> {
    pub fn new(client_id: ClientId) -> Self {
        SpotifyAuth {
            client_id,
            callback_port: 30405,
            state: Unauthenticated,
        }
    }

    fn authenticate<S: Into<Option<bool>>>(
        self,
        scope: &Scope,
        show_dialog: S,
    ) -> Result<SpotifyAuth<Authenticated>, (Self, Error)> {
        match request::authorize(&self.client_id, self.callback_port, &scope, show_dialog.into()) {
            Ok(code) => Ok(SpotifyAuth {
                client_id: self.client_id,
                callback_port: self.callback_port,
                state: Authenticated { auth_code: code },
            }),
            Err(e) => Err((self, e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn get_client_id() -> ClientId {
        env::var("CLIENT_ID").expect("Missing CLIENT_ID in environment")
    }

    #[test]
    fn creation() {
        let _auth = SpotifyAuth::new(get_client_id());
    }

    #[test]
    #[ignore]
    fn authentication() {
        let auth = SpotifyAuth::new(get_client_id());
        match auth.authenticate(&String::new(), None) {
            Ok(SpotifyAuth{state: Authenticated{auth_code}, ..}) => {},
            Err(e) => panic!("Bad authentication: {:?}", e),
        }
    }
}
