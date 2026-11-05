//! Project lifecycle: `.magic` create/open/save/close and connect/load/remove/update assets.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

/// Identifies a Magic Studio project file (`.magic`, JSON).
const PROJECT_FORMAT: &str = "magic.project";

fn saved_path(path: &Path, project: &Path) -> PathBuf {
    let parent = project.parent().unwrap_or(Path::new("."));
    if let Ok(rel) = path.strip_prefix(parent) {
        return rel.to_path_buf();
    }
    if let (Ok(canon_path), Ok(canon_parent)) = (path.canonicalize(), parent.canonicalize())
        && let Ok(rel) = canon_path.strip_prefix(&canon_parent)
    {
        return rel.to_path_buf();
    }
    path.to_path_buf()
}

fn opened_path(path: &str, project: &Path) -> PathBuf {
    if let Ok(resolved) = craft_docpath::resolve_absolute(path, Some(project))
        && resolved.exists()
    {
        return resolved;
    }
    let parent = project.parent().unwrap_or(Path::new("."));
    let raw = Path::new(path);
    if raw.is_absolute() {
        if raw.exists() {
            return raw.to_path_buf();
        }
        if let Some(idx) = path.find("/hyperframes/") {
            let candidate = parent.join(&path[idx + 1..]);
            if candidate.exists() {
                return candidate;
            }
        }
        return raw.to_path_buf();
    }
    parent.join(raw)
}

/// The keys this code writes; anything else already in the file is kept as it is.
const MANAGED_KEYS: [&str; 7] = [
    "format", "version", "name", "assets", "workshop", "mode", "song",
];

/// The persisted project state, shared by Save and unsaved-change detection.
pub fn project_document(s: &Session) -> Value {
    let Some(path) = &s.project_path else {
        return Value::Null;
    };
    let name = path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("Untitled");
    let mut doc = json!({"format": PROJECT_FORMAT, "version": 1, "name": name, "assets": s.project_assets,
        "workshop": {"audio": s.song_audio, "draft_approved": s.draft_approved, "target": s.video_target, "video_project": s.video_project_path}});
    let song = json!(s.song);
    if song.as_object().is_some_and(|fields| !fields.is_empty()) {
        doc["song"] = song;
    }
    if let Some(mode) = &s.project_mode {
        doc["mode"] = json!(mode);
    }
    normalize_connections(&mut doc, path);
    doc
}

/// The file on disk with this session's managed keys replaced.
fn merged_document(s: &Session, path: &Path) -> Value {
    let current = project_document(s);
    let mut doc = read_project_document(path)
        .ok()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    for key in MANAGED_KEYS {
        match current.get(key) {
            Some(value) => doc[key] = value.clone(),
            None => {
                if let Some(map) = doc.as_object_mut() {
                    map.remove(key);
                }
            }
        }
    }
    doc
}

fn normalize_connections(doc: &mut Value, project: &Path) {
    for asset in doc["assets"].as_array_mut().into_iter().flatten() {
        if let Some(path) = asset["path"].as_str() {
            asset["path"] = json!(saved_path(Path::new(path), project));
        }
    }
    for key in ["audio", "video_project"] {
        if let Some(path) = doc["workshop"][key].as_str() {
            doc["workshop"][key] = json!(saved_path(Path::new(path), project));
        }
    }
}

pub fn project_is_dirty(s: &Session, saved: &Value) -> bool {
    let mut saved = saved.clone();
    if let Some(path) = &s.project_path {
        normalize_connections(&mut saved, path);
    }
    let current = project_document(s);
    ["assets", "workshop", "mode", "song"]
        .iter()
        .any(|key| saved[*key] != current[*key])
}

/// Writes the open project's asset connections to its `.magic` file.
pub fn write_project_file(s: &Session) -> Result<()> {
    let Some(path) = &s.project_path else {
        return Ok(());
    };
    let doc = merged_document(s, path);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(&doc)
        .map_err(|e| EngineError::InvalidParams(format!("cannot encode project: {e}")))?;
    let written = write_preserving_lease(path, text.as_bytes());
    // A failed write can still have left partial effects if rollback failed.
    if s.record_user_state {
        crate::change_marker::invalidate();
    }
    written?;
    s.project_saves
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if s.record_user_state {
        crate::activity::stored(path, text.len());
    }
    Ok(())
}

// Preserve the inode holding Studio's existing writer lease. Replacing it with a renamed
// temporary file would release exclusivity on the new path. Encoding precedes all disk writes;
// an ordinary write failure restores the old bytes. This is not a crash-atomic transaction.
fn write_preserving_lease(path: &Path, bytes: &[u8]) -> Result<()> {
    write_preserving_lease_using(path, bytes, store_project_bytes)
}
fn store_project_bytes(file: &mut std::fs::File, content: &[u8]) -> std::io::Result<()> {
    use std::io::{Seek, SeekFrom, Write};
    file.seek(SeekFrom::Start(0))?;
    file.write_all(content)?;
    file.set_len(content.len() as u64)?;
    file.sync_data()
}
fn write_preserving_lease_using(
    path: &Path,
    bytes: &[u8],
    mut store: impl FnMut(&mut std::fs::File, &[u8]) -> std::io::Result<()>,
) -> Result<()> {
    use std::io::Read;
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(EngineError::InvalidParams(
            "Project destination must be a regular file; no write performed".into(),
        ));
    }
    let mut before = Vec::new();
    file.read_to_end(&mut before)?;
    if before == bytes {
        return Ok(());
    }
    if let Err(error) = store(&mut file, bytes) {
        if let Err(rollback) = store(&mut file, &before) {
            return Err(EngineError::Io(std::io::Error::other(format!(
                "Project write failed: {error}; restoring prior bytes also failed: {rollback}. Preserve this file and repair storage before retrying"
            ))));
        }
        return Err(EngineError::Io(std::io::Error::other(format!(
            "Project write failed; prior bytes restored: {error}"
        ))));
    }
    Ok(())
}

/// Resolve and canonicalize a project path, appending `.magic` when missing.
pub(crate) fn project_file_path(s: &Session, params: &Value) -> Result<PathBuf> {
    let raw = params
        .get("path")
        .and_then(Value::as_str)
        .filter(|p| !p.trim().is_empty())
        .ok_or_else(|| EngineError::InvalidParams("missing project 'path'".into()))?;
    let mut path = super::workspace::resolve_workspace_path(s, raw);
    if path
        .extension()
        .is_none_or(|e| !e.eq_ignore_ascii_case("magic"))
    {
        path.as_mut_os_string().push(".magic");
    }
    if path.exists() {
        return std::fs::canonicalize(&path).map_err(EngineError::from);
    }
    Ok(path)
}

/// A fresh session for a project: no assets loaded, nothing active.
fn reset_for_project(s: &mut Session, path: PathBuf, assets: Vec<crate::ProjectAsset>) {
    let owner = if s.project_path.as_ref() == Some(&path) {
        s.project_owner.clone()
    } else {
        None
    };
    *s = Session {
        cwd: s.cwd.clone(),
        record_user_state: s.record_user_state,
        agent_selection: s.agent_selection.clone(),
        project_assets: assets,
        project_path: Some(path),
        project_owner: owner,
        ..Session::default()
    };
}

/// Position of a connected project asset by id.
fn project_index(s: &Session, params: &Value) -> Result<usize> {
    let id = params
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::InvalidParams("missing project asset 'id'".into()))?;
    s.project_assets
        .iter()
        .position(|a| a.id == id)
        .ok_or_else(|| EngineError::InvalidParams(format!("project asset '{id}' is not connected")))
}

/// List connected project assets and the active one.
pub fn cmd_project_list_assets(s: &mut Session, _: &Value) -> Result<Value> {
    Ok(
        json!({"assets": s.project_assets, "active_asset": s.active_asset, "project": s.project_path}),
    )
}

// Navigation history is optional global state; a valid project remains created/open if it is unavailable.
fn recent_warning(s: &Session, path: &Path, requested: bool) -> Vec<Value> {
    if !s.record_user_state || !requested {
        return Vec::new();
    }
    match crate::project_catalog::record(&path.to_string_lossy()) {
        Ok(()) => Vec::new(),
        Err(error) => vec![
            json!({"code":"recent_history_unavailable","stage":"update_recent_projects","path":crate::project_catalog::file(),"error":error,"project":path,"effect":"Project is available; Home recent history was not updated","next_action":"Continue using the returned project path; repair only the named navigation-history location if needed"}),
        ],
    }
}

/// Migrate one legacy `.acproj` to `.magic`: rename the file and write the current format
/// inside it. Explicit, reversible (the old name is reported) and never touches other files.
pub fn cmd_project_migrate(_s: &mut Session, p: &Value) -> Result<Value> {
    let raw = p["path"]
        .as_str()
        .ok_or_else(|| EngineError::InvalidParams("Give path=<legacy .acproj file>".into()))?;
    let source = std::path::PathBuf::from(raw);
    if !source.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "{} is not a file",
            source.display()
        )));
    }
    if source
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("magic"))
    {
        return Err(EngineError::InvalidParams(
            "already a .magic project".into(),
        ));
    }
    if !source
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("acproj"))
    {
        return Err(EngineError::InvalidParams(
            "only a legacy .acproj can be migrated".into(),
        ));
    }
    let target = source.with_extension("magic");
    if target.exists() {
        return Err(EngineError::InvalidParams(format!(
            "{} already exists; nothing was changed",
            target.display()
        )));
    }
    let mut doc: Value = serde_json::from_slice(&std::fs::read(&source)?)
        .map_err(|e| EngineError::InvalidParams(format!("not a project file: {e}")))?;
    let before = doc["format"].as_str().unwrap_or_default().to_string();
    doc["format"] = json!(PROJECT_FORMAT);
    std::fs::rename(&source, &target)?;
    std::fs::write(
        &target,
        serde_json::to_vec_pretty(&doc)
            .map_err(|_| EngineError::InvalidParams("cannot encode the migrated project".into()))?,
    )?;
    Ok(
        json!({"ok":true,"from":source,"to":target,"format_before":before,"format_after":PROJECT_FORMAT,"note":"The legacy file name is gone; rename it back to restore the old one."}),
    )
}

fn link_existing_video_project(s: &mut Session, dir: &Path, steps: &mut Vec<Value>) {
    if !dir.join("hyperframes/index.html").is_file() {
        return;
    }
    let _ = super::video_target::ensure_music_audio_role(&dir.join("hyperframes"));
    s.video_project_path = super::video_target::target_project_path(s, dir, &s.video_target);
    if s.song_audio.is_none() {
        s.song_audio = super::video_target::discover_contained_audio(dir);
    }
    let _ = super::video_target::ensure_native_captions(s, dir);
    steps.push(json!({"command": "video.link_existing", "status": "linked existing hyperframes project and contained audio"}));
}

/// Auto-initialize HyperFrames and process audio when `project.new` is given `audio` (or link existing `hyperframes/`).
fn bootstrap_new_project_workshop(s: &mut Session, path: &Path, params: &Value) -> Vec<Value> {
    let mut steps = Vec::new();
    let Some(dir) = path.parent() else {
        return steps;
    };
    let Some(raw_audio) = params
        .get("audio")
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
    else {
        link_existing_video_project(s, dir, &mut steps);
        return steps;
    };
    if params["skip_audio_setup"] == true {
        return steps;
    }
    match super::workshop::cmd_workshop_set_audio(s, &json!({"path": raw_audio, "resolve_video_cli": params["resolve_video_cli"]})) {
        Ok(_) => steps.push(json!({"command": "workshop.set_audio", "status": "complete; HyperFrames initialized and audio linked"})),
        Err(e) => {
            steps.push(json!({"command": "workshop.set_audio", "status": "failed", "error": e.to_string()}));
            return steps;
        }
    }
    if let Some(t) = params
        .get("transcript")
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
    {
        let _ = super::workshop::cmd_workshop_import_transcript(
            s,
            &json!({"path": t, "captions": params["captions"], "audiomap": params["audiomap"], "replace": true}),
        );
        steps.push(json!({"command": "workshop.import_transcript", "status": "complete"}));
    } else if params
        .get("process_audio")
        .and_then(Value::as_bool)
        .unwrap_or(true)
        && params["prepare_only"] != true
    {
        let model = params
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("large-v3");
        match super::workshop::cmd_audio_analyze(s, &json!({"analyze_beats": true, "model": model, "resolve_video_cli": params["resolve_video_cli"]})) {
            Ok(_) => steps.push(json!({"command": "audio.analyze", "status": "complete; transcribed, analyzed sections/onsets/moments, and generated beat grid"})),
            Err(e) => steps.push(json!({"command": "audio.analyze", "status": "failed", "error": e.to_string()})),
        }
    }
    steps
}

fn check_overwrite(path: &Path, overwrite: bool) -> Result<()> {
    if path.exists() && !overwrite {
        return Err(EngineError::InvalidParams(format!(
            "project already exists: {} (pass \"overwrite\": true to replace it)",
            path.display()
        )));
    }
    Ok(())
}

fn project_audio_source(s: &Session, params: &Value) -> Result<Option<PathBuf>> {
    match params.get("audio").and_then(Value::as_str) {
        Some(raw) => Ok(Some(super::workspace::song_audio_path(s, raw)?)),
        None => Ok(None),
    }
}

fn claim_new_project(s: &Session, path: &Path) -> Result<std::sync::Arc<std::fs::File>> {
    if path.exists() {
        claim(s, path)
    } else {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)?;
        file.try_lock()
            .map_err(|e| EngineError::InvalidParams(format!("Cannot own new project: {e}")))?;
        Ok(std::sync::Arc::new(file))
    }
}

/// Create a new `.magic` session.
pub fn cmd_project_new(s: &mut Session, params: &Value) -> Result<Value> {
    let path = project_file_path(s, params)?;
    let mode = project_mode(params)?;
    let overwrite = params
        .get("overwrite")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    check_overwrite(&path, overwrite)?;
    let audio = project_audio_source(s, params)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let owner = claim_new_project(s, &path)?;
    reset_for_project(s, path.clone(), Vec::new());
    s.project_owner = Some(owner);
    s.song_audio = audio;
    s.project_mode = mode;
    write_project_file(s)?;
    // An animation study has no song or storyboard to prepare.
    let steps = if s.project_mode.is_some() {
        Vec::new()
    } else {
        bootstrap_new_project_workshop(s, &path, params)
    };
    write_project_file(s)?;
    let warnings = recent_warning(s, &path, true);
    let workshop = super::workshop::workshop_status(s)?;
    Ok(json!({
        "ok": true,
        "project": path,
        "mode": s.project_mode,
        "warnings": warnings,
        "steps": steps,
        "assets": s.project_assets,
        "workshop": workshop,
        "next_steps": workshop["next_steps"],
    }))
}

/// The project's mode: `animation` for a character study, absent for a song project.
fn project_mode(value: &Value) -> Result<Option<String>> {
    match value.get("mode").and_then(Value::as_str) {
        None | Some("project") => Ok(None),
        Some("animation") => Ok(Some("animation".into())),
        Some(other) => Err(EngineError::InvalidParams(format!(
            "mode must be animation or project, not {other}"
        ))),
    }
}

fn read_project_document(path: &Path) -> Result<Value> {
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(&text)
        .map_err(|e| EngineError::InvalidParams(format!("invalid project file: {e}")))
}

fn project_assets_from_doc(path: &Path, doc: &Value) -> Result<Vec<crate::ProjectAsset>> {
    let mut assets: Vec<crate::ProjectAsset> =
        serde_json::from_value(doc.get("assets").cloned().unwrap_or_else(|| json!([])))
            .map_err(|e| EngineError::InvalidParams(format!("invalid project assets: {e}")))?;
    for asset in &mut assets {
        if let Some(source) = &asset.path
            && !source.is_absolute()
        {
            let parent = path.parent().ok_or_else(|| {
                EngineError::InvalidParams("Project path has no parent directory".into())
            })?;
            asset.path = Some(parent.join(source));
        }
    }
    Ok(assets)
}

fn apply_opened_state(
    s: &mut Session,
    path: &Path,
    doc: &Value,
    assets: Vec<crate::ProjectAsset>,
    owner: Option<std::sync::Arc<std::fs::File>>,
) -> Result<()> {
    let song = serde_json::from_value(doc.get("song").cloned().unwrap_or_else(|| json!({})))
        .map_err(|e| EngineError::InvalidParams(format!("invalid song metadata: {e}")))?;
    reset_for_project(s, path.to_path_buf(), assets);
    if let Some(owner) = owner {
        s.project_owner = Some(owner);
    }
    s.project_mode = project_mode(doc)?;
    s.song = song;
    s.song_audio = doc
        .pointer("/workshop/audio")
        .and_then(Value::as_str)
        .map(|source| opened_path(source, path));
    s.draft_approved = doc
        .pointer("/workshop/draft_approved")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if let Some(target) = doc
        .pointer("/workshop/target")
        .and_then(Value::as_str)
        .filter(|t| super::workshop::VIDEO_TARGETS.contains(t))
    {
        s.video_target = target.into();
    }
    s.video_project_path = doc
        .pointer("/workshop/video_project")
        .and_then(Value::as_str)
        .map(|source| opened_path(source, path));
    Ok(())
}

/// Open an existing `.magic` session.
pub fn cmd_project_open(s: &mut Session, params: &Value) -> Result<Value> {
    let path = project_file_path(s, params)?;
    let owner = if params["claim_owner"] == true {
        Some(claim(s, &path)?)
    } else {
        None
    };
    let doc = read_project_document(&path)?;
    if doc.get("format").and_then(Value::as_str) != Some(PROJECT_FORMAT) {
        return Err(EngineError::InvalidParams(format!(
            "{} is not a Magic Studio project",
            path.display()
        )));
    }
    let assets = project_assets_from_doc(&path, &doc)?;
    // Report connections whose source files have moved instead of dropping them.
    let missing: Vec<&str> = assets
        .iter()
        .filter(|a| a.path.as_ref().is_some_and(|p| !p.exists()))
        .map(|a| a.id.as_str())
        .collect::<Vec<_>>();
    let missing = json!(missing);
    apply_opened_state(s, &path, &doc, assets, owner)?;
    repair_opened_workshop_links(s, &path);
    let warnings = recent_warning(s, &path, params["record_recent"] != false);
    let workshop = super::workshop::workshop_status(s)?;
    Ok(json!({
        "ok": true,
        "project": path,
        "mode": s.project_mode,
        "warnings": warnings,
        "assets": s.project_assets,
        "missing": missing,
        "workshop": workshop,
        "next_steps": workshop["next_steps"],
    }))
}

fn repair_opened_workshop_links(s: &mut Session, path: &Path) {
    let Some(dir) = path.parent() else {
        return;
    };
    if !dir.join("hyperframes/index.html").is_file() {
        return;
    }
    let _ = super::video_target::ensure_music_audio_role(&dir.join("hyperframes"));
    if s.project_owner.is_none() {
        return;
    }
    let mut changed = false;
    if s.video_project_path.as_ref().is_none_or(|p| !p.is_file())
        && let Some(vp) = super::video_target::target_project_path(s, dir, &s.video_target)
    {
        s.video_project_path = Some(vp);
        changed = true;
    }
    if s.song_audio.as_ref().is_none_or(|p| !p.is_file())
        && let Some(audio) = super::video_target::discover_contained_audio(dir)
    {
        s.song_audio = Some(audio);
        changed = true;
    }
    if changed {
        let _ = write_project_file(s);
    }
}

fn claim_save_target(target: &Path) -> Result<std::sync::Arc<std::fs::File>> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Acquire the destination without truncating it, before changing session identity.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(target)?;
    if !file.metadata()?.is_file() {
        return Err(EngineError::InvalidParams(
            "Project destination must be a regular file; no write performed".into(),
        ));
    }
    file.try_lock().map_err(|_| EngineError::InvalidParams(format!(
        "Project is owned by another live writer: {}. No Save As write performed; the original project remains selected.", target.display()
    )))?;
    Ok(std::sync::Arc::new(file))
}

/// Refuse to act when `expected_project` names a different project than the selected one.
fn project_guard(s: &Session, p: &Value) -> Result<()> {
    let selected = s
        .project_path
        .as_ref()
        .ok_or_else(|| EngineError::InvalidParams("Select the intended project".into()))?
        .canonicalize()?;
    let expected = project_file_path(s, &json!({"path": p["expected_project"]}))?.canonicalize()?;
    if selected != expected {
        return Err(EngineError::InvalidParams(
            "expected_project differs from selected project".into(),
        ));
    }
    Ok(())
}

/// Save the open project (or save-as when `path` is given).
pub fn cmd_project_save(s: &mut Session, params: &Value) -> Result<Value> {
    if params.get("expected_project").is_some() {
        project_guard(s, params)?;
    }
    if s.project_path.is_none() {
        return Err(EngineError::InvalidParams(
            "no project is open; use project.new".into(),
        ));
    }
    let target = match params.get("path") {
        Some(_) => Some(project_file_path(s, params)?),
        None => None,
    };
    match target {
        Some(target) if s.project_path.as_ref() != Some(&target) => save_as(s, target),
        _ => {
            write_project_file(s)?;
            Ok(json!({"ok": true, "project": s.project_path, "documents": []}))
        }
    }
}

/// Save to a new file and select it; a failed write keeps the original project selected.
fn save_as(s: &mut Session, target: PathBuf) -> Result<Value> {
    let file = claim_save_target(&target)?;
    let target = std::fs::canonicalize(target)?;
    let previous_path = s.project_path.replace(target);
    let previous_owner = s.project_owner.replace(file);
    if let Err(error) = write_project_file(s) {
        s.project_path = previous_path;
        s.project_owner = previous_owner;
        return Err(error);
    }
    Ok(json!({"ok": true, "project": s.project_path}))
}

/// Close the project and clear session state.
pub fn cmd_project_close(s: &mut Session, _: &Value) -> Result<Value> {
    *s = Session {
        cwd: s.cwd.clone(),
        record_user_state: s.record_user_state,
        agent_selection: s.agent_selection.clone(),
        ..Session::default()
    };
    Ok(json!({"ok": true}))
}

/// Ensure a default isolated-assets `.magic` project is open when working on an isolated asset.
pub fn ensure_default_asset_project(s: &mut Session) -> Result<PathBuf> {
    if let Some(existing) = &s.project_path {
        return Ok(existing.clone());
    }
    let root = super::workspace::workspace_root(s).join("exports/isolated_assets");
    std::fs::create_dir_all(&root)?;
    let default_proj = root.join("isolated_assets.magic");
    if default_proj.is_file() {
        let _ = cmd_project_open(s, &json!({"path": default_proj, "record_recent": false}));
    } else {
        let _ = cmd_project_new(s, &json!({"path": default_proj, "overwrite": true}));
    }
    s.project_path.clone().ok_or_else(|| {
        EngineError::InvalidParams("could not initialize default asset project".into())
    })
}

fn ensure_asset_project(s: &mut Session, params: &Value) -> Result<()> {
    if s.project_path.is_none()
        && (params.get("isolated").and_then(Value::as_bool) == Some(true)
            || params.get("use_default_project").and_then(Value::as_bool) == Some(true))
    {
        let _ = ensure_default_asset_project(s)?;
    }
    Ok(())
}

struct AssetRequest {
    path: Option<PathBuf>,
    kind: String,
    name: String,
    stem: String,
    load: bool,
    if_not_exists: bool,
    explicit_id: Option<String>,
}

fn asset_request(s: &Session, params: &Value) -> Result<AssetRequest> {
    let path = params
        .get("path")
        .or_else(|| params.get("manifest"))
        .and_then(Value::as_str)
        .map(|p| super::workspace::resolve_workspace_path(s, p))
        .ok_or_else(|| EngineError::InvalidParams("missing 'path' parameter".into()))?;
    if !path.exists() {
        return Err(EngineError::InvalidParams(format!(
            "asset path does not exist: {}",
            path.display()
        )));
    }
    let kind = params.get("kind").and_then(Value::as_str).unwrap_or("auto");
    if ![
        "auto",
        "image",
        "poses",
        "turnaround",
        "cutout_rig",
        "character",
        "model_3d",
        "anim_3d",
    ]
    .contains(&kind)
    {
        return Err(EngineError::InvalidParams(
            "kind must be auto, image, poses, turnaround, cutout_rig, character, model_3d, or anim_3d".into(),
        ));
    }
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("asset")
        .to_owned();
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(&stem)
        .trim()
        .to_owned();
    if name.is_empty() {
        return Err(EngineError::InvalidParams(
            "asset name cannot be empty".into(),
        ));
    }
    let explicit_id = params
        .get("id")
        .and_then(Value::as_str)
        .and_then(super::workspace::try_sanitize_asset_id);
    Ok(AssetRequest {
        path: Some(path),
        kind: kind.to_owned(),
        name,
        stem,
        load: params.get("load").and_then(Value::as_bool).unwrap_or(false),
        if_not_exists: params
            .get("if_not_exists")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        explicit_id,
    })
}

fn outside_project_image(dir: &Path, source: &Path) -> bool {
    !source.starts_with(dir)
        && source
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| {
                [
                    "png", "jpg", "jpeg", "webp", "svg", "gif", "bmp", "tif", "tiff", "exr",
                ]
                .contains(&e.to_ascii_lowercase().as_str())
            })
}

fn asset_destination_folder(dir: &Path, params: &Value) -> Result<(PathBuf, PathBuf)> {
    let dest = |key: &str| {
        params
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            // A plain folder name: nothing that could climb out of assets/.
            .filter(|n| !n.is_empty() && !n.contains(['/', '\\']) && *n != "." && *n != "..")
            .map(str::to_owned)
    };
    match (dest("character"), dest("background"), dest("image")) {
        (Some(c), None, None) => {
            let folder = dir.join("assets/characters").join(&c);
            Ok((folder.clone(), folder.join("parts")))
        }
        (None, Some(b), None) => {
            let folder = dir.join("assets/backgrounds").join(&b);
            Ok((folder.clone(), folder))
        }
        (None, None, Some(name)) => {
            let folder = dir.join("assets/images").join(name);
            Ok((folder.clone(), folder))
        }
        _ => {
            Err(EngineError::InvalidParams(
                "an image from outside the project needs one destination: \
                         character=<name> (copied to assets/characters/<name>/parts/) or \
                         background=<name> (copied to assets/backgrounds/<name>/) or image=<name> (copied to assets/images/<name>/)"
                    .into(),
            ))
        }
    }
}

fn copy_destination(
    s: &Session,
    params: &Value,
    path: &Option<PathBuf>,
) -> Result<Option<(PathBuf, PathBuf)>> {
    let Some(dir) = s.project_path.as_ref().and_then(|p| p.parent()) else {
        return Ok(None);
    };
    let Some(source) = path.as_ref() else {
        return Ok(None);
    };
    if !outside_project_image(dir, source) {
        return Ok(None);
    }
    asset_destination_folder(dir, params).map(Some)
}

fn unique_asset_id(s: &Session, base_id: &str, explicit: bool) -> Result<String> {
    let mut id = base_id.to_owned();
    let mut suffix = 2;
    while s.project_assets.iter().any(|a| a.id == id) || s.assets.contains_key(&id) {
        if explicit {
            return Err(EngineError::InvalidParams(format!(
                "asset id '{id}' already exists"
            )));
        }
        id = format!("{base_id}_{suffix}");
        suffix += 1;
    }
    Ok(id)
}

fn record_provenance(
    folder: &Path,
    copy: &Path,
    source: &Path,
    meta: &std::fs::Metadata,
) -> Result<()> {
    let unix = |t: std::time::SystemTime| {
        t.duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    };
    let provenance = folder.join("provenance.json");
    let mut doc: Value = std::fs::read_to_string(&provenance)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}));
    if !doc["files"].is_array() {
        doc["files"] = json!([]);
    }
    let assets_dir = folder.ancestors().nth(2).unwrap_or(folder);
    doc["files"].as_array_mut().ok_or_else(|| EngineError::InvalidParams("Invalid provenance files list".into()))?.push(json!({
        "file": copy.strip_prefix(assets_dir).unwrap_or(copy), "source": source, "bytes": meta.len(),
        "modified_unix": meta.modified().map(unix).unwrap_or(0),
        "copied_unix": unix(std::time::SystemTime::now())}));
    std::fs::write(&provenance, format!("{doc:#}\n"))?;
    Ok(())
}

fn copy_asset_file(folder: &Path, files: &Path, source: &Path) -> Result<PathBuf> {
    let copy = files.join(source.file_name().unwrap_or_default());
    if copy.exists() {
        return Err(EngineError::InvalidParams(format!(
            "{} is already in the project; connect that file instead",
            copy.display()
        )));
    }
    std::fs::create_dir_all(files)?;
    std::fs::copy(source, &copy)?;
    // provenance.json beside the destination lists where every copied file came from.
    let meta = std::fs::metadata(source)?;
    record_provenance(folder, &copy, source, &meta)?;
    Ok(copy)
}

fn reuse_existing_asset(
    s: &mut Session,
    path: &Option<PathBuf>,
    load: bool,
    if_not_exists: bool,
) -> Result<Option<Value>> {
    if !if_not_exists {
        return Ok(None);
    }
    let Some(existing_id) = s
        .project_assets
        .iter()
        .find(|a| path.is_some() && a.path == *path)
        .map(|a| a.id.clone())
    else {
        return Ok(None);
    };
    if load {
        cmd_project_load_asset(s, &json!({"id": existing_id}))?;
    }
    Ok(Some(
        json!({"ok": true, "asset_id": existing_id, "loaded": load, "assets": s.project_assets}),
    ))
}

fn load_and_save_asset(s: &mut Session, params: &Value, id: &str, load: bool) -> Result<()> {
    if load {
        cmd_project_load_asset(s, &json!({"id": id}))?;
    }
    if params["save"].as_bool() == Some(true) {
        cmd_project_save(s, &json!({}))?;
    }
    Ok(())
}

/// Connect an asset by path to the open project.
pub fn cmd_project_add_asset(s: &mut Session, params: &Value) -> Result<Value> {
    ensure_asset_project(s, params)?;
    let AssetRequest {
        path,
        kind,
        name,
        stem,
        load,
        if_not_exists,
        explicit_id,
    } = asset_request(s, params)?;
    if let Some(reused) = reuse_existing_asset(s, &path, load, if_not_exists)? {
        return Ok(reused);
    }
    let base_id = explicit_id
        .clone()
        .or_else(|| super::workspace::try_sanitize_asset_id(&name))
        .unwrap_or_else(|| super::workspace::sanitize_asset_id(&stem));
    // With a project open, an image from outside it is copied to its destination in the workshop
    // layout: a character's parts or a background. No destination, no copy: the asset is refused.
    let copy_into = copy_destination(s, params, &path)?;
    let id = unique_asset_id(s, &base_id, explicit_id.is_some())?;
    let mut stored = path.clone();
    if let (Some((folder, files)), Some(source)) = (&copy_into, &path) {
        stored = Some(copy_asset_file(folder, files, source)?);
    }
    s.project_assets.push(crate::ProjectAsset {
        id: id.clone(),
        name,
        preset: None,
        path: stored.clone(),
        kind,
        extra: Default::default(),
    });
    load_and_save_asset(s, params, &id, load)?;
    Ok(
        json!({"ok": true, "asset_id": id, "loaded": load, "assets": s.project_assets,
        "copied_to": copy_into.is_some().then_some(&stored)}),
    )
}

/// Remove a connected project asset and unload it.
pub fn cmd_project_remove_asset(s: &mut Session, params: &Value) -> Result<Value> {
    let index = project_index(s, params)?;
    let entry = s.project_assets.remove(index);
    s.assets.remove(&entry.id);
    if s.active_asset.as_deref() == Some(&entry.id) {
        s.active_asset = s
            .project_assets
            .iter()
            .find(|e| s.assets.contains_key(&e.id))
            .map(|e| e.id.clone());
    }
    Ok(json!({"ok": true, "removed": entry.id, "active_asset": s.active_asset}))
}

/// Rename or reorder a connected project asset.
pub fn cmd_project_update_asset(s: &mut Session, params: &Value) -> Result<Value> {
    let index = project_index(s, params)?;
    let name = params
        .get("name")
        .map(|v| {
            v.as_str()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .ok_or_else(|| EngineError::InvalidParams("asset name cannot be empty".into()))
        })
        .transpose()?;
    let target = params
        .get("index")
        .map(|v| {
            v.as_u64()
                .filter(|i| *i < s.project_assets.len() as u64)
                .map(|i| i as usize)
                .ok_or_else(|| EngineError::InvalidParams("asset index out of range".into()))
        })
        .transpose()?;
    if let Some(name) = name {
        s.project_assets[index].name = name.into();
    }
    if let Some(target) = target {
        let entry = s.project_assets.remove(index);
        s.project_assets.insert(target, entry);
    }
    cmd_project_list_assets(s, params)
}

/// Read an image's size or a Craft document's (`craft_doc::info`) canvas size.
fn loaded_asset(entry: &crate::ProjectAsset, path: &Path) -> Result<crate::Asset> {
    let mut asset = crate::Asset {
        id: entry.id.clone(),
        path: path.to_path_buf(),
        ..Default::default()
    };
    if let Ok(reader) = image::ImageReader::open(path).and_then(|r| r.with_guessed_format())
        && reader.format().is_some()
    {
        let image = reader.decode()?;
        asset.width = image.width();
        asset.height = image.height();
        asset.has_alpha = image.color().has_alpha();
    } else if let Ok(bytes) = std::fs::read(path)
        && craft_doc::sniff(&bytes).is_some()
    {
        if let Ok(outline) = craft_doc::outline(&bytes, &craft_doc::Limits::default()) {
            if let Some(c) = outline.canvas {
                asset.width = c.width;
                asset.height = c.height;
            } else if let Some(page) = outline.pages.first() {
                asset.width = page.width.round() as u32;
                asset.height = page.height.round() as u32;
            }
        }
    } else if !path.exists() {
        return Err(EngineError::InvalidParams(format!(
            "project asset file is missing: {}",
            path.display()
        )));
    }
    Ok(asset)
}

/// Load a connected project asset into the session; a failure leaves the previous asset active.
pub fn cmd_project_load_asset(s: &mut Session, params: &Value) -> Result<Value> {
    let entry = s.project_assets[project_index(s, params)?].clone();
    let reload = params
        .get("reload")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !s.assets.contains_key(&entry.id) || reload {
        let path = entry.path.as_ref().ok_or_else(|| {
            EngineError::InvalidParams(format!("project asset '{}' has no file path", entry.id))
        })?;
        let asset = loaded_asset(&entry, path)?;
        s.assets.insert(entry.id.clone(), asset);
    }
    s.active_asset = Some(entry.id.clone());
    Ok(json!({"ok": true, "asset_id": entry.id}))
}

/// Canonical paths identify existing projects without changing their file format.
pub fn cmd_project_identity(s: &mut Session, p: &Value) -> Result<Value> {
    let path = project_file_path(s, p)?;
    let path = std::fs::canonicalize(path)?;
    Ok(json!({"ok":true,"project":path,"project_id":path,"identity_kind":"canonical path"}))
}
pub(crate) fn claim(s: &Session, path: &Path) -> Result<std::sync::Arc<std::fs::File>> {
    if s.project_path.as_deref() == Some(path)
        && let Some(owner) = &s.project_owner
    {
        return Ok(owner.clone());
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?;
    file.try_lock().map_err(|_|EngineError::InvalidParams(format!("Project is owned by another live writer: {}. Inspect project.catalog and attach to its existing Studio; no competing write performed.",path.display())))?;
    Ok(std::sync::Arc::new(file))
}

/// Attach disk metadata without a UI switch, writer lease, action receipt or navigation-history change.
pub fn cmd_project_attach(s: &mut Session, p: &Value) -> Result<Value> {
    let mut p = p.clone();
    p["record_recent"] = json!(false);
    p["claim_owner"] = json!(false);
    let mut value = cmd_project_open(s, &p)?;
    value["attachment"] = json!("read-only disk metadata; Studio unchanged");
    Ok(value)
}

pub fn theatre_state(s: &mut Session, params: &Value) -> Result<Value> {
    let path = s
        .project_path
        .as_ref()
        .ok_or_else(|| EngineError::InvalidParams("Open a project".into()))?;
    if params["expected_project"].as_str() != path.to_str() {
        return Err(EngineError::InvalidParams("Project changed".into()));
    }
    let doc = read_project_document(path)?;
    Ok(json!({"state":doc["theatre"],"status":if doc["theatre"].is_object(){"saved"}else{"empty"}}))
}
pub fn save_theatre_state(s: &mut Session, params: &Value) -> Result<Value> {
    theatre_state(s, params)?;
    if !params["state"]["sheetsById"].is_object() {
        return Err(EngineError::InvalidParams(
            "Invalid Theatre snapshot".into(),
        ));
    }
    let path = s
        .project_path
        .as_ref()
        .ok_or_else(|| EngineError::InvalidParams("Open a project".into()))?;
    let mut doc = read_project_document(path)?;
    doc["theatre"] = params["state"].clone();
    let bytes =
        serde_json::to_vec_pretty(&doc).map_err(|e| EngineError::InvalidParams(e.to_string()))?;
    write_preserving_lease(path, &bytes)?;
    Ok(json!({"saved":true,"project":path}))
}
