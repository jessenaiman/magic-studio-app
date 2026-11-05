//! `magic-cli ui`: serve the Magic web UI (`apps/magic-cli/ui`) on loopback, backed by one engine
//! session.
//!
//! The page calls `POST /exec {"id", "params"}` for every engine command (see `ui/tauri-shim.js`)
//! and loads local media through `GET /file?p=<absolute path>`. Requests are handled one at a time
//! on the calling thread, so the session needs no locking.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};

use serde_json::{Value, json};

/// Serve until the process is stopped. `open` launches the default browser on the page.
pub fn serve(session: &mut magic::Session, port: u16, open: bool) -> Result<(), String> {
    let root = magic::workspace::workspace_root(session).join("apps/magic-cli/ui");
    if !root.join("index.html").is_file() {
        return Err(format!("the web UI is missing: {}", root.display()));
    }
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("127.0.0.1:{port}: {e}"))?;
    let url = format!("http://127.0.0.1:{port}/");
    eprintln!("Magic web UI: {url}  (serving {})", root.display());
    if open {
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    }
    for stream in listener.incoming().flatten() {
        if let Err(e) = handle(session, &root, stream) {
            eprintln!("ui: {e}");
        }
    }
    Ok(())
}

fn handle(session: &mut magic::Session, root: &Path, mut stream: TcpStream) -> Result<(), String> {
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    let mut parts = line.split_whitespace();
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or("/"));
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).map_err(|e| e.to_string())? == 0 || header.trim().is_empty() {
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
        ("POST", "/exec") => {
            let mut raw = vec![0; length];
            reader.read_exact(&mut raw).map_err(|e| e.to_string())?;
            ("200 OK", "application/json", exec(session, &raw).into_bytes())
        }
        ("GET", "/file") => local_file(&param(query, "p")),
        ("GET", _) => static_file(root, &decode(path)),
        _ => ("405 Method Not Allowed", "text/plain", b"method not allowed".to_vec()),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).and_then(|()| stream.write_all(&body)).map_err(|e| e.to_string())
}

/// Run one engine command; the page reads `result` or `error`.
fn exec(session: &mut magic::Session, raw: &[u8]) -> String {
    let request: Value = match serde_json::from_slice(raw) {
        Ok(v) => v,
        Err(e) => return json!({ "error": format!("bad request: {e}") }).to_string(),
    };
    let id = request["id"].as_str().unwrap_or_default();
    let params = request.get("params").cloned().filter(|p| !p.is_null()).unwrap_or_else(|| json!({}));
    match session.execute(id, params) {
        Ok(result) => json!({ "result": result }).to_string(),
        Err(e) => json!({ "error": e.to_string() }).to_string(),
    }
}

fn static_file(root: &Path, path: &str) -> (&'static str, &'static str, Vec<u8>) {
    let relative = Path::new(path.trim_start_matches('/'));
    if relative.components().any(|c| !matches!(c, Component::Normal(_))) {
        return ("404 Not Found", "text/plain", b"not found".to_vec());
    }
    let file = if path == "/" { root.join("index.html") } else { root.join(relative) };
    local_file(&file.to_string_lossy())
}

fn local_file(path: &str) -> (&'static str, &'static str, Vec<u8>) {
    let file = PathBuf::from(path);
    match std::fs::read(&file) {
        Ok(bytes) if file.is_file() => ("200 OK", content_type(&file), bytes),
        _ => ("404 Not Found", "text/plain", b"not found".to_vec()),
    }
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "m4a" => "audio/mp4",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        _ => "application/octet-stream",
    }
}

/// One query parameter, percent-decoded.
fn param(query: &str, key: &str) -> String {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| *k == key)
        .map(|(_, v)| decode(v))
        .unwrap_or_default()
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(b) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
