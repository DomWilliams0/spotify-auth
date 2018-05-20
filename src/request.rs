use errors::*;
use types::*;

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

use curl::easy;
use json::{self, JsonValue};
use querystring;
use url::form_urlencoded::Serializer;
use webbrowser;

const URL_AUTHORIZE: &str = "https://accounts.spotify.com/authorize";
const CALLBACK_HOST: &str = "localhost";

fn redirect_uri(port: u16) -> String {
    format!("http://{}:{}", CALLBACK_HOST, port)
}

fn make_query_url(url: &str, params: querystring::QueryParams) -> String {
    format!("{}?{}", url, querystring::stringify(params))
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
    stream.write_all("All done, now go back to your application".as_bytes())?;
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
) -> Result<TokenResponse, Error> {
    send_token_request(
        auth_code,
        &redirect_uri(callback_port),
        client_secret,
        client_id,
    )
}

fn send_token_request(
    auth_code: &AuthCode,
    redirect_uri: &RedirectUri,
    client_secret: &ClientSecret,
    client_id: &ClientId,
) -> Result<TokenResponse, Error> {
    let url = "https://accounts.spotify.com/api/token";

    let params = vec![
        ("grant_type", "authorization_code"),
        ("code", auth_code),
        ("redirect_uri", redirect_uri),
        ("client_id", client_id),
        ("client_secret", client_secret),
    ];

    let mut response = send_api_request(RequestMethod::Post, Some(params), easy::List::new(), url)?;

    match (
        response["access_token"].take_string(),
        response["token_type"].take_string(),
        response["scope"].take_string(),
        response["expires_in"].take(),
        response["refresh_token"].take_string(),
    ) {
        (Some(access), Some(token_type), Some(scope), JsonValue::Number(expiry), Some(refresh)) => {
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
