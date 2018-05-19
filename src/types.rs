use chrono;

pub type ClientId = String;
pub type RedirectUri = String;
pub type AuthCode = String;
pub type Token = String;
pub type Scope = String; // TODO more strongly typed
pub type ExpiryTime = chrono::DateTime<chrono::Local>;
