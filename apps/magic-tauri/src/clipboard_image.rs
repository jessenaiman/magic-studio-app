//! Import clipboard artwork through the normal project asset pipeline.
use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use magic::Session;
use serde_json::{Value, json};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use tauri_plugin_clipboard_manager::ClipboardExt;

fn encode_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let expected = (width as usize).checked_mul(height as usize).and_then(|n| n.checked_mul(4));
    if width == 0 || height == 0 || expected != Some(rgba.len()) {
        return Err("Clipboard image has invalid dimensions or pixel data".into());
    }
    let mut png = Vec::new();
    PngEncoder::new(&mut png).write_image(rgba, width, height, ColorType::Rgba8.into()).map_err(|e| format!("Could not encode clipboard image: {e}"))?;
    Ok(png)
}

struct TempImage {
    dir: PathBuf,
    path: PathBuf,
}
impl Drop for TempImage {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl TempImage {
    fn create(project: &Path, bytes: &[u8]) -> Result<Self, String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let parent = project.parent().ok_or("Project has no parent directory")?;
        let parent = parent.canonicalize().map_err(|e| format!("Cannot resolve project directory: {e}"))?;
        let candidates = [Some(std::env::temp_dir()), std::env::var_os("HOME").map(PathBuf::from), std::env::current_dir().ok()];
        for base in candidates.into_iter().flatten() {
            let Ok(base) = base.canonicalize() else { continue };
            if base.starts_with(&parent) {
                continue;
            }
            for _ in 0..32 {
                let dir = base.join(format!("magic-clipboard-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
                let mut builder = std::fs::DirBuilder::new();
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    builder.mode(0o700);
                }
                match builder.create(&dir) {
                    Ok(()) => {
                        let temporary = Self { path: dir.join("clipboard.png"), dir };
                        let mut file = std::fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&temporary.path)
                            .map_err(|e| format!("Cannot create temporary clipboard image: {e}"))?;
                        file.write_all(bytes).map_err(|e| format!("Cannot write temporary clipboard image: {e}"))?;
                        return Ok(temporary);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(_) => break,
                }
            }
        }
        Err("Cannot create a temporary image outside the project directory".into())
    }
}

/// Call on a worker thread: the clipboard plugin warns that reads can deadlock the Linux UI thread.
pub fn import_image(app: &tauri::AppHandle, session: &mut Session, params: &Value) -> Result<Value, String> {
    let expected = params.get("expected_project").and_then(Value::as_str).ok_or("expected_project is required")?;
    let project = session.project_path.as_ref().ok_or("No project is open")?;
    if project.to_str() != Some(expected) {
        return Err("The active project changed; clipboard import was not run".into());
    }
    let name = params.get("name").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty()).unwrap_or("Clipboard artwork");
    let image = app.clipboard().read_image().map_err(|e| format!("Cannot read an image from the clipboard: {e}"))?;
    let png = encode_png(image.rgba(), image.width(), image.height())?;
    let temp = TempImage::create(project, &png)?;
    session.execute("project.add_asset", json!({"path": temp.path, "kind": "image", "name": name, "image": name})).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn encodes_rgba_as_png_without_losing_alpha() {
        let png = super::encode_png(&[255, 0, 20, 128, 0, 200, 0, 255], 2, 1).unwrap();
        let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (2, 1));
        assert_eq!(decoded.as_raw(), &[255, 0, 20, 128, 0, 200, 0, 255]);
    }

    #[test]
    fn rejects_invalid_pixel_length() {
        assert!(super::encode_png(&[1, 2, 3], 1, 1).is_err());
    }
}
