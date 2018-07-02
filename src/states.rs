use querystring;

use errors::*;
use request;
use types::*;

#[derive(Debug)]
pub struct StateMachine<State> {
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
    tokens: Tokens,
}

// helper
impl<S> StateMachine<S> {
    fn transition<T>(self, new_state: T) -> StateMachine<T> {
        StateMachine {
            client_id: self.client_id,
            callback_port: self.callback_port,
            client_secret: self.client_secret,
            state: new_state,
        }
    }
}

// transitions

impl StateMachine<Unauthenticated> {
    pub fn new(client_id: ClientId, client_secret: ClientSecret) -> Self {
        StateMachine {
            client_id,
            client_secret,
            callback_port: 30405,
            state: Unauthenticated,
        }
    }

    pub fn authenticate<S: Into<Option<bool>>>(
        self,
        scope: &Scope,
        show_dialog: S,
    ) -> Result<StateMachine<Authenticated>, (Self, Error)> {
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

impl StateMachine<Authenticated> {
    pub fn request_token(self) -> Result<StateMachine<TokenBearing>, (Self, Error)> {
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
                Ok(self.transition(TokenBearing { auth_code, tokens }))
            }
            Err(e) => Err((self, e)),
        }
    }
}

impl StateMachine<TokenBearing> {
    pub fn access_api<'a, P>(
        &self,
        method: RequestMethod,
        params: P,
        endpoint: &Endpoint,
    ) -> Result<ApiResponse, Error>
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

    pub fn refresh_token(&mut self) -> Result<(), Error> {
        request::refresh_token(&mut self.state.tokens, &self.client_id, &self.client_secret)?;
        debug!("Requested token refresh, received {:?}", self.state.tokens);
        Ok(())
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

    fn new_auth() -> StateMachine<Unauthenticated> {
        let (cid, csec) = get_client_details();
        StateMachine::new(cid, csec)
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
                s @ StateMachine {
                    state: Authenticated { .. },
                    ..
                },
            ) => s,
            Err((_, e)) => panic!("Bad authentication: {}", e),
        };

        let mut auth = match auth.request_token() {
            Ok(
                s @ StateMachine {
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

        for _ in 0..4 {
            match auth.refresh_token() {
                Ok(_) => {}
                Err(e) => panic!("Bad refresh: {}", e),
            }
        }
    }
}
