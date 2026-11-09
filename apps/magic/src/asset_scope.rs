//! Asset protocol grants derived from the active engine session and returned media.
use magic::Session;
use serde_json::Value;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use tauri::Manager;

#[derive(Default, Debug)]
struct Grants {
    directories: BTreeSet<PathBuf>,
    files: BTreeSet<PathBuf>,
}

fn media_path(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| {
        matches!(
            ext.to_ascii_lowercase().as_str(),
            "png"
                | "jpg"
                | "jpeg"
                | "webp"
                | "gif"
                | "svg"
                | "avif"
                | "mp4"
                | "mov"
                | "webm"
                | "wav"
                | "mp3"
                | "flac"
                | "ogg"
                | "m4a"
                | "glb"
                | "gltf"
                | "ttf"
                | "otf"
                | "woff"
                | "woff2"
        )
    })
}

// GLTF URIs use percent encoding; reject schemes, absolute paths, queries and fragments.
fn local_uri(uri: &str) -> Option<PathBuf> {
    if uri.contains([':', '?', '#', '\\']) {
        return None;
    }
    let bytes = uri.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    let path = PathBuf::from(String::from_utf8(decoded).ok()?);
    if path.is_absolute() || path.as_os_str().is_empty() { None } else { Some(path) }
}

fn safe_directory(path: &Path) -> bool {
    if path.parent().is_none() {
        return false;
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    !home.as_deref().is_some_and(|home| path == home || matches!((path.canonicalize(), home.canonicalize()), (Ok(a), Ok(b)) if a == b))
}

impl Grants {
    fn media(&mut self, path: PathBuf) {
        if !path.is_absolute() || !media_path(&path) {
            return;
        }
        if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("gltf")) {
            // Grant only resources explicitly named by this model, not its whole parent.
            if let Ok(bytes) = std::fs::read(&path)
                && let Ok(model) = serde_json::from_slice::<Value>(&bytes)
                && let Some(parent) = path.parent()
            {
                for field in ["buffers", "images"] {
                    if let Some(entries) = model.get(field).and_then(Value::as_array) {
                        for entry in entries {
                            if let Some(uri) = entry.get("uri").and_then(Value::as_str)
                                && let Some(relative) = local_uri(uri)
                            {
                                self.files.insert(parent.join(relative));
                            }
                        }
                    }
                }
            }
        }
        self.files.insert(path);
    }

    fn returned_media(&mut self, value: &Value) {
        match value {
            Value::String(path) => self.media(PathBuf::from(path)),
            Value::Array(values) => values.iter().for_each(|value| self.returned_media(value)),
            Value::Object(values) => values.values().for_each(|value| self.returned_media(value)),
            _ => {}
        }
    }
}

fn grants(session: &Session, result: &Value) -> Grants {
    let mut plan = Grants::default();
    if let Some(parent) = session.project_path.as_deref().and_then(Path::parent)
        && safe_directory(parent)
    {
        plan.directories.insert(parent.to_owned());
        if let Ok(canon) = parent.canonicalize()
            && safe_directory(&canon)
        {
            plan.directories.insert(canon);
        }
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                if let Ok(canon) = entry.path().canonicalize()
                    && canon.is_dir()
                    && safe_directory(&canon)
                {
                    plan.directories.insert(canon);
                }
            }
        }
    }
    if let Some(workspace) = &session.lyrics_workspace
        && safe_directory(workspace)
    {
        plan.directories.insert(workspace.clone());
    }
    let resolve = |path: &Path| {
        if path.is_absolute() { path.to_owned() } else { session.project_path.as_deref().and_then(Path::parent).unwrap_or(&session.cwd).join(path) }
    };
    if let Some(path) = &session.song_audio {
        plan.media(resolve(path));
    }
    for asset in &session.project_assets {
        if let Some(path) = &asset.path {
            plan.media(resolve(path));
        }
    }
    for asset in session.assets.values() {
        plan.media(resolve(&asset.path));
    }
    plan.returned_media(result);
    plan
}

/// Tauri grants accumulate for this app session. Credential deny patterns still
/// take precedence; switching projects must not permanently forbid reopening one.
pub fn allow(app: &tauri::AppHandle, session: &Session, result: &Value) -> Result<(), String> {
    let scope = app.asset_protocol_scope();
    let plan = grants(session, result);
    for path in plan.directories {
        scope.allow_directory(&path, true).map_err(|error| format!("Asset directory {}: {error}", path.display()))?;
    }
    for path in plan.files {
        scope.allow_file(&path).map_err(|error| format!("Asset file {}: {error}", path.display()))?;
    }
    Ok(())
}
