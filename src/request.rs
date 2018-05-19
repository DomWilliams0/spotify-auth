use errors::*;
use types::*;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::collections::HashMap;

use querystring;
use webbrowser;

const URL_AUTHORIZE: &str = "https://accounts.spotify.com/authorize";

fn make_query_url(url: &str, params: querystring::QueryParams) -> String {
    format!("{}?{}", url, querystring::stringify(params))
}

fn parse_response(line: &str) -> Result<String, Error> {
    // GET /?lala HTTP/1.1
    // TODO error type for spotify protocol error, eg they changed api
    // TODO otherwise parse it as query string and can return code/state or error/reason from that
    let mut query_str = line.split(' ')
        .nth(1)
        .ok_or(ErrorKind::SpotifyAPIError(format!("Bad callback request: {}", line)))?;
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
    let server = TcpListener::bind(("localhost", port))?;
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

    let redirect_uri = format!("http://localhost:{}", callback_port);
    let mut params: querystring::QueryParams = vec![
        ("client_id", &client_id),
        ("response_type", "code"),
        ("redirect_uri", &redirect_uri),
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
