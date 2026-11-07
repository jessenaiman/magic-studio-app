//! The Lyrics designer (`apps/magic-cli/ui/lyrics.html`) as a native webview inside the Magic window.
//!
//! The page is served on loopback and runs every engine command through the live egui session (the
//! same `ControlRequest` channel `--control` uses), so `words.read`, `words.save` and local
//! backdrops behave exactly as in `magic-cli ui`. wry's child webview needs X11, so `main` runs
//! the window through XWayland when the desktop is Wayland.

use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
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
    let bridge = Arc::new(bridge);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let bridge = Arc::clone(&bridge);
            std::thread::spawn(move || {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                let _ = handle(&bridge, stream);
            });
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
    let mut range = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().unwrap_or(0);
            } else if name.eq_ignore_ascii_case("range") {
                range = Some(value.trim().to_owned());
            }
        }
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path == "/file" && matches!(method, "GET" | "HEAD") {
        let file = param(query, "p");
        return local_file(
            &mut stream,
            Path::new(&file),
            method == "HEAD",
            range.as_deref(),
        );
    }
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
        ("GET", _) => static_file(&bridge.ui_root, path),
        _ => ("404 Not Found", "text/plain", b"not found".to_vec()),
    };
    write_response_head(&mut stream, status, kind, body.len() as u64, "")?;
    stream.write_all(&body)
}

fn write_response_head(
    stream: &mut TcpStream,
    status: &str,
    kind: &str,
    length: u64,
    extra: &str,
) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {length}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, HEAD, POST, OPTIONS\r\nAccess-Control-Allow-Headers: *\r\nCache-Control: no-store\r\nConnection: close\r\n{extra}\r\n"
    )
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

fn local_file(
    stream: &mut TcpStream,
    path: &Path,
    head_only: bool,
    range: Option<&str>,
) -> std::io::Result<()> {
    let Ok(mut file) = std::fs::File::open(path).and_then(|file| {
        if file.metadata()?.is_file() {
            Ok(file)
        } else {
            Err(std::io::ErrorKind::NotFound.into())
        }
    }) else {
        write_response_head(stream, "404 Not Found", "text/plain", 9, "")?;
        return if head_only {
            Ok(())
        } else {
            stream.write_all(b"not found")
        };
    };
    let total = file.metadata()?.len();
    let kind = content_type(path);
    let (start, length) = if let Some(range) = range.filter(|_| !head_only) {
        let Some((start, end)) = byte_range(range, total) else {
            return write_response_head(
                stream,
                "416 Range Not Satisfiable",
                kind,
                0,
                &format!("Accept-Ranges: bytes\r\nContent-Range: bytes */{total}\r\n"),
            );
        };
        let length = end - start + 1;
        write_response_head(
            stream,
            "206 Partial Content",
            kind,
            length,
            &format!("Accept-Ranges: bytes\r\nContent-Range: bytes {start}-{end}/{total}\r\n"),
        )?;
        (start, length)
    } else {
        write_response_head(stream, "200 OK", kind, total, "Accept-Ranges: bytes\r\n")?;
        (0, total)
    };
    if !head_only {
        file.seek(SeekFrom::Start(start))?;
        std::io::copy(&mut file.take(length), stream)?;
    }
    Ok(())
}

fn byte_range(header: &str, length: u64) -> Option<(u64, u64)> {
    let last = length.checked_sub(1)?;
    let (first, end) = header.strip_prefix("bytes=")?.split_once('-')?;
    if first.is_empty() {
        let count: u64 = end.parse().ok()?;
        return (count != 0).then_some((length.saturating_sub(count), last));
    }
    let first = first.parse().ok()?;
    let end = if end.is_empty() {
        last
    } else {
        end.parse::<u64>().ok()?.min(last)
    };
    (first <= end).then_some((first, end))
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

struct WorkspaceView {
    view: wry::WebView,
    project: String,
    page: WebWorkspacePage,
    visible: bool,
}

/// Child webviews sharing the live Magic bridge.
pub struct LyricsWebView {
    bridge: Option<Bridge>,
    url: Option<String>,
    views: [Option<WorkspaceView>; 2],
    shown: [bool; 2],
}

impl LyricsWebView {
    pub fn new(tx: Sender<ControlRequest>, wake: WakeHandle, ui_root: PathBuf) -> Self {
        Self {
            bridge: Some(Bridge { tx, wake, ui_root }),
            url: None,
            views: [None, None],
            shown: [false; 2],
        }
    }

    fn create(
        &mut self,
        frame: &eframe::Frame,
        bounds: wry::Rect,
        page: WebWorkspacePage,
    ) -> Result<wry::WebView, String> {
        // GTK starts only now: initialised before the egui window exists it breaks winit's GLX setup.
        gtk::gdk::set_allowed_backends("x11");
        gtk::init().map_err(|e| e.to_string())?;
        if self.url.is_none() {
            let bridge = self.bridge.take().ok_or("Workspace webview unavailable")?;
            self.url = spawn_bridge(bridge);
        }
        let url = self
            .url
            .as_deref()
            .ok_or("Workspace bridge could not bind a port")?;
        wry::WebViewBuilder::new()
            .with_url(page_url(url, page))
            .with_bounds(bounds)
            .build_as_child(frame)
            .map_err(|e| e.to_string())
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
        WebWorkspacePage::Storyboard => "storyboard-workspace.html",
        WebWorkspacePage::Lyrics => "lyrics.html",
        WebWorkspacePage::Motion => "motion.html",
        WebWorkspacePage::Studio => "studio.html",
        WebWorkspacePage::AnimationAssets => "animation-controls.html#assets",
        WebWorkspacePage::AnimationInspector => "animation-controls.html#inspector",
        WebWorkspacePage::Animation => "animation.html",
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
        let index = usize::from(page == WebWorkspacePage::AnimationInspector);
        if let Some(slot) = self.views[index].as_mut() {
            let _ = slot.view.set_bounds(bounds);
            if (!slot.visible || slot.page != page)
                && let Some(url) = self.url.as_deref()
            {
                let _ = slot.view.load_url(&page_url(url, page));
            } else if slot.project != project {
                let _ = slot.view.reload();
            }
            if !slot.visible {
                let _ = slot.view.set_visible(true);
            }
            slot.visible = true;
            slot.page = page;
            project.clone_into(&mut slot.project);
        } else {
            match self.create(frame, bounds, page) {
                Ok(view) => {
                    self.views[index] = Some(WorkspaceView {
                        view,
                        project: project.into(),
                        page,
                        visible: true,
                    })
                }
                Err(error) => eprintln!("magic: Workspace webview: {error}"),
            }
        }
        self.shown[index] = true;
        ctx.request_repaint_after(Duration::from_millis(16));
    }

    fn finish_frame(&mut self) {
        for (slot, shown) in self.views.iter_mut().zip(&mut self.shown) {
            if let Some(slot) = slot.as_mut()
                && slot.visible
                && !*shown
            {
                let _ = slot.view.set_visible(false);
                let _ = slot.view.load_url("about:blank");
                slot.visible = false;
            }
            *shown = false;
        }
        if self.url.is_some() {
            pump_gtk();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::byte_range;

    #[test]
    fn media_ranges_handle_seeks_suffixes_and_end_of_file() {
        assert_eq!(byte_range("bytes=40-49", 80), Some((40, 49)));
        assert_eq!(byte_range("bytes=40-", 80), Some((40, 79)));
        assert_eq!(byte_range("bytes=-8", 80), Some((72, 79)));
        assert_eq!(byte_range("bytes=78-900", 80), Some((78, 79)));
        assert_eq!(byte_range("bytes=80-", 80), None);
        assert_eq!(byte_range("bytes=0-", 0), None);
    }
}
