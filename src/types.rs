use errors;

use chrono;
use std::ops::Deref;
use std::str::FromStr;

pub type ClientId = String;
pub type ClientSecret = String;
pub type RedirectUri = String;
pub type Endpoint = str; // TODO make all str?
pub type AuthCode = String;
pub type Token = String;
pub type ExpiryTime = chrono::DateTime<chrono::Local>;

#[derive(Debug)]
pub struct TokenResponse {
    pub access_token: Token,
    pub scope: Scope,
    pub expiry_time: ExpiryTime,
    pub refresh_token: Token,
}

// TODO more strongly typed
#[derive(Debug)]
pub struct Scope(String);

impl Scope {
    pub fn new() -> Self {
        Self { 0: String::new() }
    }
}

impl FromStr for Scope {
    type Err = errors::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Scope { 0: s.to_owned() })
    }
}

impl Deref for Scope {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn expiry_time(expires_in: i64) -> ExpiryTime {
    chrono::Local::now() + chrono::Duration::seconds(expires_in)
}

#[derive(Debug, Copy, Clone)]
pub enum RequestMethod {
    Get,
    Post,
    Put,
    Delete,
}
