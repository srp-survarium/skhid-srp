//! skhid-board: serve the status page, or check a data file.
//!
//!   skhid-board [--bind ADDR] [--port N] [--data FILE]   (secret in BOARD_SECRET)
//!   skhid-board --check FILE
//!
//! Each option falls back to its environment variable: BOARD_BIND (0.0.0.0), BOARD_PORT (8095),
//! BOARD_DATA (board.json). The secret is taken from the environment only, so it never shows in
//! the process list; it is never logged.

use skhid_board::{model, server};
use std::process::ExitCode;
use std::time::SystemTime;

const USAGE: &str = "usage: skhid-board [--bind ADDR] [--port N] [--data FILE]  (secret in BOARD_SECRET)\n       skhid-board --check FILE";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut bind = std::env::var("BOARD_BIND").unwrap_or_else(|_| "0.0.0.0".into());
    let mut port = std::env::var("BOARD_PORT").unwrap_or_else(|_| "8095".into());
    let mut data = std::env::var("BOARD_DATA").unwrap_or_else(|_| "board.json".into());

    let mut it = args.iter();
    while let Some(a) = it.next() {
        let Some(v) = it.next() else {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        };
        match a.as_str() {
            "--bind" => bind = v.clone(),
            "--port" => port = v.clone(),
            "--data" => data = v.clone(),
            "--check" => return check(v),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }

    let secret = std::env::var("BOARD_SECRET").unwrap_or_default();
    if secret.len() < 16 || !secret.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_') {
        eprintln!("BOARD_SECRET must be set: at least 16 characters of [A-Za-z0-9_-]");
        return ExitCode::from(2);
    }
    let Ok(port) = port.parse::<u16>() else {
        eprintln!("bad port {port:?}");
        return ExitCode::from(2);
    };

    let http = match tiny_http::Server::http((bind.as_str(), port)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot listen on {bind}:{port}: {e}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!("board: listening on {bind}:{port}, data {data}");

    let mut store = server::Store::new(data.into());
    store.refresh(SystemTime::now());

    for req in http.incoming_requests() {
        let method = req.method().as_str().to_string();
        let has_body = req.body_length().is_some_and(|n| n > 0)
            || req.headers().iter().any(|h| h.field.equiv("Transfer-Encoding"));
        let route = server::route(&method, req.url(), has_body, &secret);
        let resp = server::respond(route, &mut store, &secret, SystemTime::now());

        // tiny_http leaves the body out of a HEAD response itself.
        let mut out = tiny_http::Response::from_data(resp.body.into_bytes()).with_status_code(resp.status);
        let headers = server::HEADERS.iter().map(|(k, v)| (*k, v.to_string())).chain(resp.extra);
        for (k, v) in headers {
            if let Ok(h) = tiny_http::Header::from_bytes(k.as_bytes(), v.as_bytes()) {
                out.add_header(h);
            }
        }
        // Errors here are a client that went away; nothing to log.
        let _ = req.respond(out);
    }
    ExitCode::SUCCESS
}

fn check(path: &str) -> ExitCode {
    let parsed = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {path}: {e}"))
        .and_then(|t| model::parse(&t));
    match parsed {
        Ok(b) => {
            println!("ok: {} item(s), {} notice(s)", b.items.len(), b.notices.len());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{path} rejected:\n{e}");
            ExitCode::FAILURE
        }
    }
}
