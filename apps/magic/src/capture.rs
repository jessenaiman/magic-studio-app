//! `ui.screenshot`: a PNG of the live webview, taken by the platform web engine itself.
//!
//! The picture is what the window shows (DOM, canvas, video frames and embedded workspaces),
//! not a DOM re-render. Only WebKitGTK (Linux, the supported desktop target) is implemented;
//! other platforms report an explicit error rather than a substitute image.
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How long a snapshot may take: a hidden window or a sleeping display presents no frame.
const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(10);

#[cfg(target_os = "linux")]
pub fn snapshot(window: &tauri::WebviewWindow) -> Result<Vec<u8>, String> {
    use std::sync::mpsc;
    use webkit2gtk::{SnapshotOptions, SnapshotRegion, WebViewExt};

    let (tx, rx) = mpsc::channel();
    // The closure runs on the GTK main thread, where WebKit snapshots must be requested.
    window
        .with_webview(move |view| {
            view.inner().snapshot(SnapshotRegion::Visible, SnapshotOptions::NONE, None::<&webkit2gtk::gio::Cancellable>, move |result| {
                let png = result.map_err(|e| e.to_string()).and_then(|surface| {
                    let mut png = Vec::new();
                    surface.write_to_png(&mut png).map_err(|e| format!("PNG encoding failed: {e}"))?;
                    Ok(png)
                });
                let _ = tx.send(png);
            });
        })
        .map_err(|e| e.to_string())?;
    rx.recv_timeout(SNAPSHOT_TIMEOUT).map_err(|_| "no frame was presented within 10 seconds (window hidden or display asleep)".to_string())?
}

#[cfg(not(target_os = "linux"))]
pub fn snapshot(_window: &tauri::WebviewWindow) -> Result<Vec<u8>, String> {
    Err("ui.screenshot is implemented for Linux (WebKitGTK) only".into())
}

/// Pixel size from the PNG header (`IHDR` follows the 8-byte signature).
pub fn png_size(png: &[u8]) -> Option<(u32, u32)> {
    let width = u32::from_be_bytes(png.get(16..20)?.try_into().ok()?);
    let height = u32::from_be_bytes(png.get(20..24)?.try_into().ok()?);
    Some((width, height))
}

/// Where the PNG goes: the caller's absolute `.png` path, or a fresh file in the temp folder.
pub fn target_path(requested: Option<&str>) -> Result<PathBuf, String> {
    let Some(requested) = requested else {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis());
        return Ok(std::env::temp_dir().join(format!("magic-screenshot-{stamp}.png")));
    };
    let path = Path::new(requested);
    if !path.is_absolute() {
        return Err("path must be absolute".into());
    }
    if path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() != Some("png") {
        return Err("path must end in .png".into());
    }
    match path.parent() {
        Some(parent) if parent.is_dir() => Ok(path.to_path_buf()),
        _ => Err("the folder for path does not exist".into()),
    }
}
