//! Tiny localhost HTTP endpoint used by the Chrome extension.
//!
//! The server knows nothing about windows: when a batch of URLs arrives it
//! calls the `wake` callback supplied by the interface, so each UI can decide
//! how to surface itself (viewport commands in egui, `ShowWindow` in GPUI).
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

pub(crate) const PORT: u16 = 52137;

/// A batch of links from the browser, and whether the user asked for an
/// immediate download instead of the review window.
pub(crate) struct RemoteBatch {
    pub(crate) urls: Vec<String>,
    pub(crate) direct: bool,
}

pub(crate) fn start(
    tx: Sender<RemoteBatch>,
    wake: Arc<dyn Fn() + Send + Sync>,
) {
    std::thread::spawn(move || {
        let Ok(listener) = TcpListener::bind(("127.0.0.1", PORT)) else {
            return;
        };
        for stream in listener.incoming().flatten() {
            let tx = tx.clone();
            let wake = wake.clone();
            std::thread::spawn(move || handle(stream, tx, wake));
        }
    });
}

fn handle(
    mut stream: TcpStream,
    tx: Sender<RemoteBatch>,
    wake: Arc<dyn Fn() + Send + Sync>,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
    let Ok(reader_stream) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(reader_stream);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
        return;
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    let mut content_length = 0usize;
    let mut origin = String::new();
    let mut has_token = false;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let value = value.trim();
            match name.trim().to_ascii_lowercase().as_str() {
                "content-length" => content_length = value.parse().unwrap_or(0),
                "origin" => origin = value.to_string(),
                "x-dvidm" => has_token = value.eq_ignore_ascii_case("chrome"),
                _ => {}
            }
        }
    }

    let origin_allowed = origin.is_empty() || origin.starts_with("chrome-extension://");

    if method == "OPTIONS" {
        if !origin_allowed {
            respond(&mut stream, 403, "Forbidden", "text/plain", &cors(&origin));
        } else {
            respond(&mut stream, 200, "", "text/plain", &cors(&origin));
        }
        return;
    }

    if !origin_allowed {
        respond(&mut stream, 403, "Forbidden", "text/plain", &cors(&origin));
        return;
    }

    if method == "GET" && path == "/ping" {
        respond(&mut stream, 200, "DVIDM", "text/plain", &cors(&origin));
        return;
    }

    if method == "POST" && path == "/add" {
        if !has_token {
            respond(
                &mut stream,
                403,
                "Missing X-DVIDM header",
                "text/plain",
                &cors(&origin),
            );
            return;
        }
        if content_length == 0 || content_length > 64 * 1024 {
            respond(&mut stream, 413, "Bad size", "text/plain", &cors(&origin));
            return;
        }
        let mut body = vec![0u8; content_length];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
        let value = serde_json::from_slice::<serde_json::Value>(&body).ok();
        let direct = value
            .as_ref()
            .and_then(|v| v.get("direct"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let mut urls: Vec<String> = Vec::new();
        if let Some(value) = value {
            if let Some(list) = value.get("urls").and_then(|v| v.as_array()) {
                for entry in list {
                    if let Some(url) = entry.as_str() {
                        urls.push(url.trim().to_string());
                    }
                }
            } else if let Some(url) = value.get("url").and_then(|v| v.as_str()) {
                urls.push(url.trim().to_string());
            }
        }
        let mut seen = std::collections::HashSet::new();
        urls.retain(|url| {
            (url.starts_with("http://") || url.starts_with("https://"))
                && seen.insert(url.clone())
        });
        if urls.is_empty() {
            respond(
                &mut stream,
                400,
                "Only http(s) URLs",
                "text/plain",
                &cors(&origin),
            );
            return;
        }
        urls.truncate(200);
        // The UI may be idle while hidden in the tray, so surface it from here;
        // the UI's next tick picks the batch up.
        wake();
        if tx
            .send(RemoteBatch {
                urls,
                direct,
            })
            .is_err()
        {
            respond(&mut stream, 500, "App closed", "text/plain", &cors(&origin));
            return;
        }
        respond(
            &mut stream,
            200,
            r#"{"ok":true}"#,
            "application/json",
            &cors(&origin),
        );
        return;
    }

    respond(&mut stream, 404, "Not found", "text/plain", &cors(&origin));
}

fn cors(origin: &str) -> String {
    let allow = if origin.is_empty() { "*" } else { origin };
    format!(
        "Access-Control-Allow-Origin: {allow}\r\n\
         Access-Control-Allow-Methods: POST, OPTIONS\r\n\
         Access-Control-Allow-Headers: content-type, x-dvidm\r\n\
         Access-Control-Max-Age: 600\r\n"
    )
}

fn respond(stream: &mut TcpStream, status: u16, body: &str, content_type: &str, extra: &str) {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         {extra}\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
    let _ = stream.flush();
}
