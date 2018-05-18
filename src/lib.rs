#![allow(dead_code)]

extern crate chrono;

#[macro_use]
extern crate quick_error;

mod errors;

#[derive(Debug)]
struct SpotifyAuth<State> {
    state: State,
}

//  no auth code, no nothing
#[derive(Debug)]
struct Unauthenticated;

// waiting for the user to login in their browser and
// enter the url they are forwarded to
// TODO do this automatically
#[derive(Debug)]
struct Authenticating;

type ClientId = String;
type RedirectUri = String;
type AuthCode = String;
type Token = String;
type Scope = String; // TODO more strongly typed
type ExpiryTime = chrono::DateTime<chrono::Local>;

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
    fn default() -> Self {
        SpotifyAuth {
            state: Unauthenticated,
        }
    }

    fn authenticate<S: Into<Option<bool>>>(
        self,
        client_id: ClientId,
        redirect_uri: RedirectUri,
        scope: Scope,
        show_dialog: S,
    ) -> Result<SpotifyAuth<Authenticating>, (Self, errors::AuthError)> {
        // TODO send request to /authorize
        Ok(SpotifyAuth {
            state: Authenticating {},
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let auth = SpotifyAuth::default();
    }

    #[test]
    fn authentication() {
        let auth = SpotifyAuth::default();
        auth.authenticate(String::new(), String::new(), String::new(), None);
    }
}
