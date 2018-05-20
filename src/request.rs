use errors::*;
use types::*;

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

use base64;
use curl::easy;
use json::{self, JsonValue};
use querystring;
use url::form_urlencoded::Serializer;
use webbrowser;

const URL_AUTHORIZE: &str = "https://accounts.spotify.com/authorize";
const URL_TOKEN: &str = "https://accounts.spotify.com/api/token";
const CALLBACK_HOST: &str = "localhost";

fn redirect_uri(port: u16) -> String {
    format!("http://{}:{}", CALLBACK_HOST, port)
}

fn make_query_url(url: &str, params: querystring::QueryParams) -> String {
    format!("{}?{}", url, querystring::stringify(params))
}

#[derive(Debug)]
struct TokenResponse {
    access_token: Token,
    scope: Scope,
    expiry_time: ExpiryTime,
    refresh_token: Option<Token>,
}

impl TokenResponse {
    fn convert_to_tokens(self, old: Tokens) -> Tokens {
        Tokens {
            refresh_token: self.refresh_token.unwrap_or(old.refresh_token),
            ..old
        }
    }
}

fn parse_response(line: &str) -> Result<String, Error> {
    let mut query_str = line.split(' ')
        .nth(1)
        .ok_or(ErrorKind::SpotifyAPIError(format!(
            "Bad callback request: {}",
            line
        )))?;
    if query_str.starts_with("/?") {
        query_str = &query_str[2..];
    }
    let mut query: HashMap<&str, &str> = querystring::querify(&query_str).iter().cloned().collect();
    // let state = query.get("state"); // TODO check state
    match (query.remove("code"), query.remove("error")) {
        (Some(code), None) => Ok(code.to_owned()),
        (None, Some(err)) => Err(ErrorKind::AuthenticationError(err.to_owned()).into()),
        _ => Err(ErrorKind::SpotifyAPIError(format!("Bad response: {}", query_str)).into()),
    }
}

fn wait_for_auth_callback(port: u16) -> Result<String, Error> {
    let server = TcpListener::bind((CALLBACK_HOST, port))?;
    let (mut stream, _) = server.accept()?; // blocks
    let line = {
        let mut buf = BufReader::new(stream.try_clone()?);
        let mut s = String::new();
        buf.read_line(&mut s)?;
        s
    };
    stream.write_all(b"All done, now go back to your application")?;
    parse_response(&line)
}

pub fn authorize(
    client_id: &ClientId,
    callback_port: u16,
    scope: &Scope,
    show_dialog: Option<bool>,
) -> Result<AuthCode, Error> {
    // TODO put state in unauth state
    // let state = "make_me_random"; // TODO randomised

    let uri = redirect_uri(callback_port);
    let mut params: querystring::QueryParams = vec![
        ("client_id", &client_id),
        ("response_type", "code"),
        ("redirect_uri", &uri),
        // ("state", &state),
        ("scope", &scope),
    ];

    if let Some(sd) = show_dialog {
        params.push(("show_dialog", if sd { "true" } else { "false" }));
    }

    let url = make_query_url(URL_AUTHORIZE, params);

    // open browser
    // TODO make this optional
    println!("Opening the browser, go there to sign in");
    if webbrowser::open(&url).is_err() {
        println!("Navigate to the following url in your browser:\n{}", url);
    }

    // TODO return future instead, with a feature
    let url = wait_for_auth_callback(callback_port)?;

    Ok(url)
}

pub fn request_token(
    auth_code: &AuthCode,
    callback_port: u16,
    client_secret: &ClientSecret,
    client_id: &ClientId,
) -> Result<Tokens, Error> {
    let redirect = redirect_uri(callback_port);
    let params = vec![
        ("grant_type", "authorization_code"),
        ("code", auth_code),
        ("redirect_uri", &redirect),
        ("client_id", client_id),
        ("client_secret", client_secret),
    ];

    let response = send_api_request(
        RequestMethod::Post,
        Some(params),
        easy::List::new(),
        URL_TOKEN,
    )?;

    let token_resp = parse_token_response(response)?;
    match token_resp.refresh_token {
        None => Err(ErrorKind::SpotifyAPIError(format!(
            "Refresh token not returned by {}",
            URL_TOKEN
        )).into()),
        Some(refresh) => Ok(Tokens {
            access_token: token_resp.access_token,
            scope: token_resp.scope,
            expiry_time: token_resp.expiry_time,
            refresh_token: refresh,
        }),
    }
}

fn parse_token_response(mut response: json::JsonValue) -> Result<TokenResponse, Error> {
    match (
        response["access_token"].take_string(),
        response["token_type"].take_string(),
        response["scope"].take_string(),
        response["expires_in"].take(),
        response["refresh_token"].take_string(),
    ) {
        (Some(access), Some(token_type), Some(scope), JsonValue::Number(expiry), refresh) => {
            if token_type != "Bearer" {
                Err(
                    ErrorKind::SpotifyAPIError(format!("Unknown token type: {}", token_type))
                        .into(),
                )
            } else {
                Ok(TokenResponse {
                    access_token: access,
                    scope: scope.parse().map_err(|_| {
                        ErrorKind::SpotifyAPIError(format!("Invalid scope returned: {}", scope))
                    })?,
                    expiry_time: expiry_time(expiry.into()),
                    refresh_token: refresh,
                })
            }
        }
        x => Err(ErrorKind::SpotifyAPIError(format!(
            "Unknown token response: {} = {:?}",
            response, x
        )).into()),
    }
}

pub fn access_api<'a>(
    access_token: &Token,
    method: RequestMethod,
    params: Option<querystring::QueryParams<'a>>,
    endpoint: &Endpoint,
) -> Result<json::JsonValue, Error> {
    let headers = {
        let mut list = easy::List::new();
        list.append(&format!("Authorization: Bearer {}", access_token))?;
        list
    };
    send_api_request(method, params, headers, endpoint)
}

pub fn refresh_token(
    tokens: &mut Tokens,
    client_id: &ClientId,
    client_secret: &ClientSecret,
) -> Result<(), Error> {
    let refresh_token = tokens.refresh_token.clone(); // TODO possible to remove clone?
    let params = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", &refresh_token),
    ];
    let headers = {
        let mut list = easy::List::new();
        let basic = base64::encode(&format!("{}:{}", client_id, client_secret));
        list.append(&format!("Authorization: Basic {}", basic))?;
        list
    };

    let response = send_api_request(RequestMethod::Post, Some(params), headers, URL_TOKEN)?;
    let token_resp = parse_token_response(response)?;

    // TODO is there a better way?
    tokens.access_token = token_resp.access_token;
    tokens.scope = token_resp.scope;
    tokens.expiry_time = token_resp.expiry_time;
    if let Some(new_refresh) = token_resp.refresh_token {
        tokens.refresh_token = new_refresh;
    }

    Ok(())
}

fn send_api_request<'a>(
    method: RequestMethod,
    params: Option<querystring::QueryParams<'a>>,
    headers: easy::List,
    endpoint: &Endpoint,
) -> Result<json::JsonValue, Error> {
    let mut req = easy::Easy::new();
    let mut url: Cow<str> = endpoint.into();
    debug!(
        "Sending {:?} request to {} with parameters {:?} and headers {:?}",
        method, url, params, headers
    );

    // add params
    if let Some(params) = params {
        match method {
            RequestMethod::Get => {
                // GET query parameters modify the url
                url = make_query_url(endpoint, params).into()
            }
            _ => {
                // other parameters are url encoded in the body
                let body = Serializer::new(String::new()).extend_pairs(params).finish();
                req.post_fields_copy(body.as_bytes())?;
            }
        };
    }

    req.url(&url)?;
    req.http_headers(headers)?;

    let mut response = Vec::new();
    {
        let mut handle = req.transfer();
        handle.write_function(|data| {
            response.extend_from_slice(data);
            Ok(data.len())
        })?;

        handle.perform()?;
    };
    let response = String::from_utf8(response)?;
    let parsed = json::parse(&response).map_err(|_| {
        ErrorKind::SpotifyAPIError(format!("JSON not returned by {} endpoint", endpoint))
    })?;

    match req.response_code()? {
        200 => Ok(parsed),
        err => Err(ErrorKind::HttpErrorJson(err, json::stringify(parsed)).into()),
    }
}
