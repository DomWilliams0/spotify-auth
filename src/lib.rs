#![allow(dead_code)]

extern crate chrono;
extern crate curl;
extern crate json;
extern crate querystring;
extern crate url;
extern crate webbrowser;

#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate log;

mod errors;
mod request;
mod types;

use errors::*;
use types::*;

#[derive(Debug)]
struct SpotifyAuth<State> {
    client_id: ClientId,
    client_secret: ClientSecret,
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
    tokens: TokenResponse,
}

// helper
impl<S> SpotifyAuth<S> {
    fn transition<T>(self, new_state: T) -> SpotifyAuth<T> {
        SpotifyAuth {
            client_id: self.client_id,
            callback_port: self.callback_port,
            client_secret: self.client_secret,
            state: new_state,
        }
    }
}

// transitions

impl SpotifyAuth<Unauthenticated> {
    pub fn new(client_id: ClientId, client_secret: ClientSecret) -> Self {
        SpotifyAuth {
            client_id,
            client_secret,
            callback_port: 30405,
            state: Unauthenticated,
        }
    }

    fn authenticate<S: Into<Option<bool>>>(
        self,
        scope: &Scope,
        show_dialog: S,
    ) -> Result<SpotifyAuth<Authenticated>, (Self, Error)> {
        match request::authorize(
            &self.client_id,
            self.callback_port,
            &scope,
            show_dialog.into(),
        ) {
            Ok(code) => {
                debug!("Authenticated with Spotify, auth code = {}", code);
                Ok(self.transition(Authenticated { auth_code: code }))
            }
            Err(e) => Err((self, e)),
        }
    }
}

impl SpotifyAuth<Authenticated> {
    fn request_token(self) -> Result<SpotifyAuth<TokenBearing>, (Self, Error)> {
        // TODO dont copy by moving out of current state, but without making self mut here
        let auth_code = self.state.auth_code.clone();
        match request::request_token(
            &auth_code,
            self.callback_port,
            &self.client_secret,
            &self.client_id,
        ) {
            Ok(tokens) => {
                debug!("Requested token, received {:?}", tokens);
                Ok(self.transition(TokenBearing {
                    auth_code: auth_code,
                    tokens: tokens,
                }))
            }
            Err(e) => Err((self, e)),
        }
    }
}

impl SpotifyAuth<TokenBearing> {
    fn access_api<'a, P>(
        &self,
        method: RequestMethod,
        params: P,
        endpoint: &Endpoint,
    ) -> Result<json::JsonValue, Error>
    where
        P: Into<Option<querystring::QueryParams<'a>>>,
    {
        request::access_api(
            &self.state.tokens.access_token,
            method,
            params.into(),
            endpoint,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    extern crate env_logger;

    fn get_client_details() -> (ClientId, ClientSecret) {
        let cid = env::var("CLIENT_ID").expect("Missing CLIENT_ID in environment");
        let csec = env::var("CLIENT_SECRET").expect("Missing CLIENT_SECRET in environment");
        (cid, csec)
    }

    fn new_auth() -> SpotifyAuth<Unauthenticated> {
        let (cid, csec) = get_client_details();
        SpotifyAuth::new(cid, csec)
    }

    #[test]
    fn creation() {
        env_logger::init();
        new_auth();
    }

    #[test]
    #[ignore]
    fn authentication() {
        env_logger::init();
        let auth = new_auth();
        let auth = match auth.authenticate(&Scope::new(), None) {
            Ok(
                s @ SpotifyAuth {
                    state: Authenticated { .. },
                    ..
                },
            ) => s,
            Err((_, e)) => panic!("Bad authentication: {}", e),
        };

        let auth = match auth.request_token() {
            Ok(
                s @ SpotifyAuth {
                    state: TokenBearing { .. },
                    ..
                },
            ) => s,
            Err((_, e)) => panic!("Bad token request: {}", e),
        };

        match auth.access_api(
            RequestMethod::Get,
            vec![("ids", "4UgQ3EFa8fEeaIEg54uV5b")],
            "https://api.spotify.com/v1/artists/",
        ) {
            Ok(_) => {}
            Err(e) => panic!("Bad API response: {}", e),
        }
    }
}
