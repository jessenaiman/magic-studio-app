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

impl Grants {
    fn media(&mut self, path: PathBuf) {
        if !path.is_absolute() || !media_path(&path) {
            return;
        }
        // GLTF can load relative buffers and textures beside the selected model.
        if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("gltf")) {
            if let Some(parent) = path.parent() {
                self.directories.insert(parent.to_owned());
            }
        } else {
            self.files.insert(path);
        }
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
    if let Some(parent) = session.project_path.as_deref().and_then(Path::parent) {
        plan.directories.insert(parent.to_owned());
    }
    if let Some(workspace) = &session.lyrics_workspace {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unopened_session_has_no_workspace_or_home_access() {
        let plan = grants(&Session::default(), &Value::Null);
        assert!(plan.directories.is_empty());
        assert!(plan.files.is_empty());
    }

    #[test]
    fn grants_open_project_and_explicit_external_media_without_their_parent_folders() {
        let session =
            Session { project_path: Some("/production/song/Song.magic".into()), song_audio: Some("/recordings/artist.wav".into()), ..Session::default() };
        let plan = grants(&session, &json!({"preview":"/tmp/render/frame.png","unrelated":"/private/settings.json"}));
        assert!(plan.directories.contains(&PathBuf::from("/production/song")));
        assert!(plan.files.contains(&PathBuf::from("/recordings/artist.wav")));
        assert!(plan.files.contains(&PathBuf::from("/tmp/render/frame.png")));
        assert!(!plan.directories.contains(&PathBuf::from("/recordings")));
        assert!(!plan.directories.contains(&PathBuf::from("/tmp")));
        assert!(!plan.files.contains(&PathBuf::from("/private/settings.json")));
    }
}
