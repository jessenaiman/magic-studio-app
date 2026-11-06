//! Tauri host for the Lyrics Designer view (`magic --lyrics` / `magic --view lyrics`),
//! backed by a live [`magic::Session`] over loopback so `words.read`, `words.save`,
//! and local backdrop images work identically in the standalone Tauri window and in `magic-cli ui`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;

use magic::Session;
use serde_json::{Value, json};

fn spawn_session_bridge(mut session: Session) -> Option<String> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).ok()?;
    let addr = listener.local_addr().ok()?;
    let url = format!("http://{addr}");
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let _ = handle(&mut session, stream);
        }
    });
    Some(url)
}

fn handle(session: &mut Session, mut stream: TcpStream) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or("/"));
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let (status, kind, body) = match (method, path) {
        ("OPTIONS", _) => ("204 No Content", "text/plain", Vec::new()),
        ("POST", "/exec") => {
            let mut raw = vec![0; length];
            reader.read_exact(&mut raw)?;
            (
                "200 OK",
                "application/json",
                exec(session, &raw).into_bytes(),
            )
        }
        ("GET", "/file") => local_file(&param(query, "p")),
        _ => ("404 Not Found", "text/plain", b"not found".to_vec()),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: *\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&body)
}

fn exec(session: &mut Session, raw: &[u8]) -> String {
    let request: Value = match serde_json::from_slice(raw) {
        Ok(v) => v,
        Err(e) => return json!({ "error": format!("bad request: {e}") }).to_string(),
    };
    let id = request["id"].as_str().unwrap_or_default();
    let params = request
        .get("params")
        .cloned()
        .filter(|p| !p.is_null())
        .unwrap_or_else(|| json!({}));
    match session.execute(id, params) {
        Ok(result) => json!({ "result": result }).to_string(),
        Err(e) => json!({ "error": e.to_string() }).to_string(),
    }
}

fn local_file(path: &str) -> (&'static str, &'static str, Vec<u8>) {
    let file = PathBuf::from(path);
    match std::fs::read(&file) {
        Ok(bytes) if file.is_file() => ("200 OK", content_type(&file), bytes),
        _ => ("404 Not Found", "text/plain", b"not found".to_vec()),
    }
}

fn content_type(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn param(query: &str, key: &str) -> String {
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=')
            && k == key
        {
            return decode_uri(v);
        }
    }
    String::new()
}

fn decode_uri(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3])
            && let Ok(b) = u8::from_str_radix(hex, 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn run(session: Session) -> tauri::Result<()> {
    let bridge = spawn_session_bridge(session);
    tauri::Builder::default()
        .setup(move |app| {
            let mut builder = tauri::WebviewWindowBuilder::new(
                app,
                "lyrics",
                tauri::WebviewUrl::App("lyrics.html".into()),
            )
            .title("Magic Studio — Lyrics")
            .inner_size(1440.0, 960.0);
            if let Some(url) = &bridge {
                builder = builder.initialization_script(format!(
                    "window.__MAGIC_BRIDGE_URL__ = {};",
                    json!(url)
                ));
            }
            builder.build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
}
