//! The Lyrics designer (`apps/magic-cli/ui/lyrics.html`) as a native webview inside the Magic window.
//!
//! The page is served on loopback and runs every engine command through the live egui session (the
//! same `ControlRequest` channel `--control` uses), so `words.read`, `words.save` and local
//! backdrops behave exactly as in `magic-cli ui`. wry's child webview needs X11, so `main` runs
//! the window through XWayland when the desktop is Wayland.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

use magic_ui_egui::{ControlRequest, LyricsHost, WebWorkspacePage};
use serde_json::{Value, json};

use crate::WakeHandle;

struct Bridge {
    tx: Sender<ControlRequest>,
    wake: WakeHandle,
    ui_root: PathBuf,
}

fn spawn_bridge(bridge: Bridge) -> Option<String> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).ok()?;
    let url = format!("http://{}", listener.local_addr().ok()?);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let _ = handle(&bridge, stream);
        }
    });
    Some(url)
}

fn handle(bridge: &Bridge, mut stream: TcpStream) -> std::io::Result<()> {
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
                exec(bridge, &raw).into_bytes(),
            )
        }
        ("GET", "/file") => local_file(&param(query, "p")),
        ("GET", _) => static_file(&bridge.ui_root, path),
        _ => ("404 Not Found", "text/plain", b"not found".to_vec()),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: *\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&body)
}

fn exec(bridge: &Bridge, raw: &[u8]) -> String {
    let request: Value = match serde_json::from_slice(raw) {
        Ok(v) => v,
        Err(e) => return json!({ "error": format!("bad request: {e}") }).to_string(),
    };
    let method = request["id"].as_str().unwrap_or_default().to_owned();
    let params = request
        .get("params")
        .cloned()
        .filter(|p| !p.is_null())
        .unwrap_or_else(|| json!({}));
    let (reply, outcome) = channel();
    if bridge
        .tx
        .send(ControlRequest {
            method,
            params,
            reply,
        })
        .is_err()
    {
        return json!({ "error": "window closed" }).to_string();
    }
    if let Ok(guard) = bridge.wake.lock()
        && let Some(ctx) = guard.as_ref()
    {
        ctx.request_repaint();
    }
    match outcome.recv() {
        Ok(Ok(result)) => json!({ "result": result }).to_string(),
        Ok(Err(error)) => json!({ "error": error }).to_string(),
        Err(_) => json!({ "error": "window closed" }).to_string(),
    }
}

fn static_file(root: &Path, path: &str) -> (&'static str, &'static str, Vec<u8>) {
    let relative = Path::new(path.trim_start_matches('/'));
    let safe = relative
        .components()
        .all(|c| matches!(c, Component::Normal(_)));
    let file = if relative.as_os_str().is_empty() {
        root.join("lyrics.html")
    } else {
        root.join(relative)
    };
    match std::fs::read(&file) {
        Ok(bytes) if safe && file.is_file() => ("200 OK", content_type(&file), bytes),
        _ => ("404 Not Found", "text/plain", b"not found".to_vec()),
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
        "html" => "text/html; charset=utf-8",
        "css" => "text/css",
        "js" => "text/javascript",
        "json" => "application/json",
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

/// The Lyrics webview, created on first show as a child of the egui window.
pub struct LyricsWebView {
    bridge: Option<Bridge>,
    view: Option<(wry::WebView, String, String, WebWorkspacePage)>,
}

impl LyricsWebView {
    pub fn new(tx: Sender<ControlRequest>, wake: WakeHandle, ui_root: PathBuf) -> Self {
        Self {
            bridge: Some(Bridge { tx, wake, ui_root }),
            view: None,
        }
    }

    fn create(
        &mut self,
        frame: &eframe::Frame,
        bounds: wry::Rect,
        project: &str,
        page: WebWorkspacePage,
    ) -> Result<(), String> {
        // GTK starts only now: initialised before the egui window exists it breaks winit's GLX setup.
        gtk::gdk::set_allowed_backends("x11");
        gtk::init().map_err(|e| e.to_string())?;
        let bridge = self.bridge.take().ok_or("Lyrics webview unavailable")?;
        let url = spawn_bridge(bridge).ok_or("Lyrics bridge could not bind a port")?;
        let view = wry::WebViewBuilder::new()
            .with_url(page_url(&url, page))
            .with_bounds(bounds)
            .build_as_child(frame)
            .map_err(|e| e.to_string())?;
        self.view = Some((view, url, project.to_owned(), page));
        Ok(())
    }
}

fn pump_gtk() {
    while gtk::events_pending() {
        gtk::main_iteration_do(false);
    }
}

fn page_url(base: &str, page: WebWorkspacePage) -> String {
    let file = match page {
        WebWorkspacePage::Projects => "projects.html",
        WebWorkspacePage::Lyrics => "lyrics.html",
        WebWorkspacePage::Motion => "motion.html",
    };
    format!("{base}/{file}")
}

impl LyricsHost for LyricsWebView {
    fn show(
        &mut self,
        frame: &eframe::Frame,
        ctx: &egui::Context,
        rect: egui::Rect,
        project: &str,
        page: WebWorkspacePage,
    ) {
        let bounds = wry::Rect {
            position: wry::dpi::LogicalPosition::new(f64::from(rect.min.x), f64::from(rect.min.y))
                .into(),
            size: wry::dpi::LogicalSize::new(f64::from(rect.width()), f64::from(rect.height()))
                .into(),
        };
        if self.view.is_none()
            && let Err(error) = self.create(frame, bounds, project, page)
        {
            eprintln!("magic: Lyrics webview: {error}");
        }
        if let Some((view, url, shown, shown_page)) = self.view.as_mut() {
            let _ = view.set_bounds(bounds);
            let _ = view.set_visible(true);
            if shown != project || *shown_page != page {
                let _ = view.load_url(&page_url(url, page));
                project.clone_into(shown);
                *shown_page = page;
            }
        }
        pump_gtk();
        ctx.request_repaint_after(Duration::from_millis(16));
    }

    fn hide(&mut self) {
        if let Some((view, ..)) = &self.view {
            let _ = view.set_visible(false);
            pump_gtk();
        }
    }
}
