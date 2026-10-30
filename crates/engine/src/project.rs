//! Project lifecycle: `.magic` create/open/save/close and connect/load/remove/update assets.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

/// Identifies a Magic Studio project file (`.magic`, JSON).
const PROJECT_FORMAT: &str = "magicstudio.project";
/// A character manifest (`character.magic`); it opens as an animation project.
const CHARACTER_FORMAT: &str = "magicstudio.character";
/// The keys an animation owns inside a character file; the rest is the character manifest.
const CHARACTER_ANIMATION_KEYS: [&str; 3] = ["theatre", "animation_sources", "parts"];

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

/// The persisted project state, shared by Save and unsaved-change detection.
pub fn project_document(s: &Session) -> Value {
    let Some(path) = &s.project_path else {
        return Value::Null;
    };
    if s.character_file {
        return character_document(s);
    }
    let name = path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("Untitled");
    // Loaded assets save their current part states; unloaded ones keep what was saved before.
    let assets: Vec<crate::ProjectAsset> = s
        .project_assets
        .iter()
        .map(|entry| {
            let mut entry = entry.clone();
            if let Some(loaded) = s.assets.get(&entry.id) {
                entry.parts = loaded.parts.clone();
            }
            entry
        })
        .collect();
    let mut doc = json!({"format": PROJECT_FORMAT, "version": 1, "name": name, "assets": assets,
        "workshop": {"audio": s.song_audio, "draft_approved": s.draft_approved, "target": s.video_target, "video_project": s.video_project_path}});
    if let Some(state) = &s.theatre_state {
        doc["theatre"] = state.clone();
    }
    if !s.animation_sources.is_empty() {
        doc["animation_sources"] = json!(s.animation_sources);
    }
    if let Some(mode) = &s.project_mode {
        doc["mode"] = json!(mode);
    }
    normalize_connections(&mut doc, path);
    doc
}

/// The animation state a character file carries: Theatre motion, native sources and part states.
fn character_document(s: &Session) -> Value {
    let mut doc = json!({"format": CHARACTER_FORMAT});
    if let Some(state) = &s.theatre_state {
        doc["theatre"] = state.clone();
    }
    if !s.animation_sources.is_empty() {
        doc["animation_sources"] = json!(s.animation_sources);
    }
    if let Some(entry) = s.project_assets.first() {
        let parts = s.assets.get(&entry.id).map_or(&entry.parts, |a| &a.parts);
        if !parts.is_empty() {
            doc["parts"] = json!(parts);
        }
    }
    doc
}

/// The character manifest as it is on disk, with the animation's keys replaced.
fn character_file_document(s: &Session, path: &Path) -> Result<Value> {
    let mut doc = read_project_document(path)?;
    let current = character_document(s);
    let manifest = doc
        .as_object_mut()
        .ok_or_else(|| EngineError::InvalidParams("character file is not a JSON object".into()))?;
    for key in CHARACTER_ANIMATION_KEYS {
        match current.get(key) {
            Some(value) => manifest.insert(key.into(), value.clone()),
            None => manifest.remove(key),
        };
    }
    Ok(doc)
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
    ["assets", "workshop", "theatre", "animation_sources", "mode", "parts"]
        .iter()
        .any(|key| saved[*key] != current[*key])
}

/// Writes the open project's asset connections to its `.magic` file.
pub fn write_project_file(s: &Session) -> Result<()> {
    let Some(path) = &s.project_path else {
        return Ok(());
    };
    let doc = if s.character_file {
        character_file_document(s, path)?
    } else {
        project_document(s)
    };
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
        blender_available: s.blender_available,
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
    let Some(raw_audio) = params.get("audio").and_then(Value::as_str).filter(|v| !v.is_empty()) else {
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
    if let Some(t) = params.get("transcript").and_then(Value::as_str).filter(|v| !v.is_empty()) {
        let _ = super::workshop::cmd_workshop_import_transcript(s, &json!({"path": t, "captions": params["captions"], "audiomap": params["audiomap"], "replace": true}));
        steps.push(json!({"command": "workshop.import_transcript", "status": "complete"}));
    } else if params.get("process_audio").and_then(Value::as_bool).unwrap_or(true) && params["prepare_only"] != true {
        let model = params.get("model").and_then(Value::as_str).unwrap_or("large-v3");
        match super::workshop::cmd_audio_analyze(s, &json!({"analyze_beats": true, "model": model, "resolve_video_cli": params["resolve_video_cli"]})) {
            Ok(_) => steps.push(json!({"command": "audio.analyze", "status": "complete; transcribed, analyzed sections/onsets/moments, and generated beat grid"})),
            Err(e) => steps.push(json!({"command": "audio.analyze", "status": "failed", "error": e.to_string()})),
        }
    }
    if !super::workshop::transcript_files(dir).is_empty() && !dir.join("storyboard/movie-board.json").is_file() && params["prepare_only"] != true && super::storyboard::cmd_storyboard_init(s, &json!({})).is_ok() {
        steps.push(json!({"command": "storyboard.init", "status": "complete"}));
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
    let animation_sources = serde_json::from_value(
        doc.get("animation_sources")
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or_else(|| json!({})),
    )
    .map_err(|e| EngineError::InvalidParams(format!("invalid animation sources: {e}")))?;
    reset_for_project(s, path.to_path_buf(), assets);
    if let Some(owner) = owner {
        s.project_owner = Some(owner);
    }
    s.theatre_state = doc.get("theatre").filter(|state| !state.is_null()).cloned();
    s.animation_sources = animation_sources;
    s.project_mode = project_mode(doc)?;
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
    if doc.get("format").and_then(Value::as_str) == Some(CHARACTER_FORMAT) {
        return open_character_file(s, &path, &doc, owner, params);
    }
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

/// The character's id: its manifest id, else its folder name.
fn character_id(path: &Path, doc: &Value) -> String {
    let folder = path.parent().and_then(Path::file_name).and_then(|n| n.to_str());
    doc["id"]
        .as_str()
        .filter(|v| !v.trim().is_empty())
        .or(folder)
        .unwrap_or("character")
        .to_string()
}

/// A character file opens as an animation project whose one asset is the character itself.
fn open_character_file(
    s: &mut Session,
    path: &Path,
    doc: &Value,
    owner: Option<std::sync::Arc<std::fs::File>>,
    params: &Value,
) -> Result<Value> {
    let id = character_id(path, doc);
    let name = doc["name"].as_str().filter(|v| !v.trim().is_empty()).unwrap_or(&id).to_string();
    let parts = serde_json::from_value(doc.get("parts").cloned().unwrap_or_else(|| json!([])))
        .map_err(|e| EngineError::InvalidParams(format!("invalid character part states: {e}")))?;
    let entry = crate::ProjectAsset {
        id: id.clone(),
        name: name.clone(),
        preset: None,
        path: Some(path.to_path_buf()),
        kind: "auto".into(),
        parts,
    };
    apply_opened_state(s, path, doc, vec![entry], owner)?;
    s.project_mode = Some("animation".into());
    s.character_file = true;
    let mut warnings = recent_warning(s, path, params["record_recent"] != false);
    if let Err(error) = cmd_project_load_asset(s, &json!({"id": id})) {
        warnings.push(json!(format!("{name} did not load: {error}")));
    }
    Ok(json!({
        "ok": true,
        "project": path,
        "name": name,
        "mode": s.project_mode,
        "character_file": true,
        "warnings": warnings,
        "assets": s.project_assets,
        "active_asset": s.active_asset,
        "missing": [],
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

/// Save the open project (or save-as when `path` is given).
pub fn cmd_project_save(s: &mut Session, params: &Value) -> Result<Value> {
    if params.get("expected_project").is_some() {
        crate::motion_authoring::project_guard(s, params)?;
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
            Ok(json!({"ok": true, "project": s.project_path}))
        }
    }
}

/// Save to a new file and select it; a failed write keeps the original project selected.
fn save_as(s: &mut Session, target: PathBuf) -> Result<Value> {
    if s.character_file {
        return Err(EngineError::InvalidParams(
            "Save As does not copy a character: its images and rigs live in its folder. Copy the character folder instead.".into(),
        ));
    }
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
        blender_available: s.blender_available,
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
    preset: Option<String>,
    path: Option<PathBuf>,
    kind: String,
    name: String,
    stem: String,
    load: bool,
    if_not_exists: bool,
    explicit_id: Option<String>,
}

fn asset_request(s: &Session, params: &Value) -> Result<AssetRequest> {
    let preset = params
        .get("preset")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let path = params
        .get("path")
        .or_else(|| params.get("manifest"))
        .and_then(Value::as_str)
        .map(|p| super::workspace::resolve_workspace_path(s, p));
    if preset.is_some() == path.is_some() {
        return Err(EngineError::InvalidParams(
            "provide exactly one of 'path' or 'preset'".into(),
        ));
    }
    if let Some(path) = &path
        && !path.exists()
    {
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
    ]
    .contains(&kind)
    {
        return Err(EngineError::InvalidParams(
            "kind must be auto, image, poses, turnaround, cutout_rig, or character".into(),
        ));
    }
    let stem = preset
        .as_deref()
        .or_else(|| path.as_ref()?.file_stem()?.to_str())
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
        preset,
        path,
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
        && source.extension().and_then(|e| e.to_str()).is_some_and(|e| {
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
            .filter(|n| {
                !n.is_empty() && !n.contains(['/', '\\']) && *n != "." && *n != ".."
            })
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

fn matte_new_asset(
    s: &mut Session,
    params: &Value,
    kind: &str,
    stored: &mut Option<PathBuf>,
) -> Result<Value> {
    // Character/image import prepares the cutout before registration. Background plates,
    // manifests, and already transparent artwork retain their supplied pixels.
    let mut matting = Value::Null;
    if params["remove_background"].as_bool().unwrap_or(true)
        && params.get("background").is_none()
        && matches!(kind, "image" | "auto" | "character")
        && let Some(source) = stored.as_ref()
        && let Ok(image) = image::ImageReader::open(source)
            .and_then(|r| r.with_guessed_format())
            .map_err(EngineError::from)
            .and_then(|r| r.decode().map_err(EngineError::from))
        && !image.to_rgba8().pixels().any(|p| p[3] < 255)
    {
        matting = crate::magic_design::matte_image(s, source, params)?;
        *stored = matting["image"].as_str().map(PathBuf::from);
    }
    Ok(matting)
}

fn reuse_existing_asset(
    s: &mut Session,
    path: &Option<PathBuf>,
    preset: &Option<String>,
    load: bool,
    if_not_exists: bool,
) -> Result<Option<Value>> {
    if !if_not_exists {
        return Ok(None);
    }
    let Some(existing_id) = s
        .project_assets
        .iter()
        .find(|a| (path.is_some() && a.path == *path) || (preset.is_some() && a.preset == *preset))
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

/// Connect an asset (by path or preset) to the open project.
pub fn cmd_project_add_asset(s: &mut Session, params: &Value) -> Result<Value> {
    ensure_asset_project(s, params)?;
    let AssetRequest {
        preset,
        path,
        kind,
        name,
        stem,
        load,
        if_not_exists,
        explicit_id,
    } = asset_request(s, params)?;
    if let Some(reused) = reuse_existing_asset(s, &path, &preset, load, if_not_exists)? {
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
    let matting = matte_new_asset(s, params, &kind, &mut stored)?;
    s.project_assets.push(crate::ProjectAsset {
        id: id.clone(),
        name,
        preset,
        path: stored.clone(),
        kind,
        parts: Vec::new(),
    });
    load_and_save_asset(s, params, &id, load)?;
    Ok(
        json!({"ok": true, "asset_id": id, "loaded": load, "assets": s.project_assets,
        "copied_to": copy_into.is_some().then_some(&stored),"matting":matting}),
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

fn manifest_path_for(path: &Path) -> PathBuf {
    if path.is_dir() {
        let char_magic = path.join("character.magic");
        if char_magic.is_file() {
            char_magic
        } else {
            path.join("manifest.json")
        }
    } else {
        path.to_path_buf()
    }
}

fn is_rig_manifest(raw: &Value, kind: &str) -> bool {
    kind == "cutout_rig"
        || raw.get("rig_type").is_some()
        || (raw.get("layers").is_some() && raw.get("canvas").is_some())
}

fn stage_preset_asset(
    staged: &mut Session,
    s: &Session,
    entry: &crate::ProjectAsset,
) -> Result<()> {
    let preset = entry.preset.as_deref().unwrap_or_default();
    let mut loader = Session {
        cwd: s.cwd.clone(),
        record_user_state: s.record_user_state,
        agent_selection: s.agent_selection.clone(),
        ..Session::default()
    };
    crate::magic_design::cmd_asset_load_manifest(&mut loader, &json!({"preset": preset}))?;
    let loaded_id = loader
        .active_asset
        .clone()
        .ok_or_else(|| EngineError::InvalidParams("preset did not load an asset".into()))?;
    let mut asset = loader
        .assets
        .remove(&loaded_id)
        .ok_or_else(|| EngineError::InvalidParams("preset asset missing".into()))?;
    if let Some(previous) = staged.assets.get(&entry.id)
        && asset.cutout_rig.is_none()
    {
        asset.cutout_rig = previous.cutout_rig.clone();
    }
    asset.id = entry.id.clone();
    staged.assets.insert(entry.id.clone(), asset);
    Ok(())
}

/// The rig for the character's active angle, else its `rig.json`; `None` before it is rigged.
fn character_rig_path(raw: &Value, proj_root: &Path, char_dir: &Path) -> Option<PathBuf> {
    let angle = raw["active_angle"].as_str().unwrap_or("three_quarter_right");
    [format!("/angles/{angle}/rig"), format!("/rigs/{angle}"), "/rigs/active".into()]
        .iter()
        .find_map(|pointer| raw.pointer(pointer).and_then(Value::as_str))
        .map(|rel| crate::character::resolve_character_path(rel, proj_root, char_dir))
        .or_else(|| Some(char_dir.join("rig.json")).filter(|p| p.is_file()))
}

fn stage_character_magic(
    staged: &mut Session,
    entry: &crate::ProjectAsset,
    raw: &Value,
    manifest_path: &Path,
) -> Result<()> {
    let char_dir = manifest_path.parent().unwrap_or(Path::new("."));
    let proj_root = staged
        .project_path
        .as_deref()
        .and_then(Path::parent)
        .unwrap_or(char_dir);
    let Some(rig_path) = character_rig_path(raw, proj_root, char_dir) else {
        // A character with no rig yet is its reference image, ready to break apart.
        let reference = raw["reference"]
            .as_str()
            .map(|rel| crate::character::resolve_character_path(rel, proj_root, char_dir))
            .filter(|p| p.is_file())
            .ok_or_else(|| EngineError::InvalidParams("character has neither a rig nor a reference image".into()))?;
        return stage_plain_image(staged, entry, &reference);
    };
    let mut plan = crate::session::LoadedCutoutRigPlan::load_from_path(&rig_path)?;
    let rig_base = rig_path.parent().unwrap_or(char_dir);
    if plan.source_image.as_os_str().is_empty() {
        plan.source_image = rig_base.to_path_buf();
    } else if !plan.source_image.is_absolute() {
        let source = plan.source_image.to_string_lossy().into_owned();
        let found = crate::character::resolve_character_path(&source, proj_root, char_dir);
        if found.is_file() {
            plan.source_image = found;
        }
    }
    plan.resolve_paths(rig_base);
    let image = image::open(&plan.source_image)?;
    let mut asset = staged.assets.remove(&entry.id).unwrap_or_default();
    asset.id = entry.id.clone();
    asset.path = plan.source_image.clone();
    asset.width = image.width();
    asset.height = image.height();
    asset.has_alpha = image.color().has_alpha();
    asset.attached_asset_sheet = raw
        .pointer("/sheets/poses")
        .and_then(Value::as_str)
        .map(|rel| crate::character::resolve_character_path(rel, proj_root, char_dir))
        .filter(|p| p.is_file())
        .or_else(|| plan.cutout_asset_sheet.clone());
    asset.attached_turnaround_sheet = raw
        .pointer("/sheets/turnaround")
        .and_then(Value::as_str)
        .map(|rel| crate::character::resolve_character_path(rel, proj_root, char_dir))
        .filter(|p| p.is_file());
    asset.cutout_rig = Some(plan);
    staged.assets.insert(entry.id.clone(), asset);
    Ok(())
}

fn stage_cutout_plan(
    staged: &mut Session,
    entry: &crate::ProjectAsset,
    manifest_path: &Path,
) -> Result<()> {
    let mut plan = crate::session::LoadedCutoutRigPlan::load_from_path(manifest_path)?;
    let base = manifest_path.parent().unwrap_or(Path::new("."));
    if plan.source_image.as_os_str().is_empty() {
        plan.source_image = base.to_path_buf();
    }
    plan.resolve_paths(base);
    let image = image::open(&plan.source_image)?;
    let mut asset = staged.assets.remove(&entry.id).unwrap_or_default();
    asset.id = entry.id.clone();
    asset.path = plan.source_image.clone();
    asset.width = image.width();
    asset.height = image.height();
    asset.has_alpha = image.color().has_alpha();
    asset.attached_asset_sheet = plan.cutout_asset_sheet.clone();
    asset.cutout_rig = Some(plan);
    staged.assets.insert(entry.id.clone(), asset);
    Ok(())
}

fn stage_plain_image(staged: &mut Session, entry: &crate::ProjectAsset, path: &Path) -> Result<()> {
    let image = image::open(path)?;
    let mut asset = staged.assets.remove(&entry.id).unwrap_or_default();
    asset.id = entry.id.clone();
    asset.path = path.to_path_buf();
    if let Ok(bytes) = std::fs::read(path.with_extension("matte.json"))
        && let Ok(receipt) = serde_json::from_slice::<Value>(&bytes)
    {
        asset.raw_original_path = receipt["source_image"].as_str().map(PathBuf::from);
    }
    asset.width = image.width();
    asset.height = image.height();
    asset.has_alpha = image.color().has_alpha();
    match entry.kind.as_str() {
        "poses" => asset.attached_asset_sheet = Some(path.to_path_buf()),
        "turnaround" => asset.attached_turnaround_sheet = Some(path.to_path_buf()),
        _ => {}
    }
    staged.assets.insert(entry.id.clone(), asset);
    Ok(())
}

fn stage_path_asset(
    staged: &mut Session,
    entry: &crate::ProjectAsset,
    path: &Path,
) -> Result<()> {
    let manifest_path = manifest_path_for(path);
    if manifest_path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("json") || e.eq_ignore_ascii_case("magic"))
    {
        let raw: Value = serde_json::from_str(&std::fs::read_to_string(&manifest_path)?)
            .map_err(|e| EngineError::InvalidParams(format!("invalid asset JSON: {e}")))?;
        if raw.get("format").and_then(Value::as_str) == Some("magicstudio.character") {
            stage_character_magic(staged, entry, &raw, &manifest_path)?;
        } else if is_rig_manifest(&raw, &entry.kind) {
            stage_cutout_plan(staged, entry, &manifest_path)?;
        } else {
            if raw.get("frames").and_then(Value::as_array).is_none() {
                return Err(EngineError::InvalidParams(
                    "expected a pose/turnaround manifest or cutout rig plan".into(),
                ));
            }
            crate::magic_design::cmd_asset_load_manifest(
                staged,
                &json!({"asset": entry.id, "manifest": manifest_path}),
            )?;
        }
    } else {
        stage_plain_image(staged, entry, path)?;
    }
    Ok(())
}

/// Load a connected project asset into the session (staged so failures preserve the previous asset).
pub fn cmd_project_load_asset(s: &mut Session, params: &Value) -> Result<Value> {
    let entry = s.project_assets[project_index(s, params)?].clone();
    let reload = params
        .get("reload")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if s.assets.contains_key(&entry.id) && !reload {
        s.active_asset = Some(entry.id.clone());
        return Ok(json!({"ok": true, "asset_id": entry.id}));
    }
    let first_load = !s.assets.contains_key(&entry.id);
    // Failed decoding/reloading must leave the previous active asset and its rig intact.
    let theatre_state = s.theatre_state.take();
    let last_result = s.last_result.take();
    let mut staged = s.clone();
    s.theatre_state = theatre_state;
    s.last_result = last_result;
    if entry.preset.is_some() {
        stage_preset_asset(&mut staged, s, &entry)?;
    } else if let Some(path) = &entry.path {
        stage_path_asset(&mut staged, &entry, path)?;
    }
    staged.active_asset = Some(entry.id.clone());
    if let Some(asset) = staged.current_asset_mut() {
        asset.sync_parts_from_manifests();
        // A reload keeps the session's part states; a first load restores the saved ones.
        if first_load {
            for saved in &entry.parts {
                if let Some(part) = asset.parts.iter_mut().find(|p| p.name == saved.name) {
                    (part.visible, part.solo, part.verified) =
                        (saved.visible, saved.solo, saved.verified);
                }
            }
        }
    }
    staged.theatre_state = s.theatre_state.take();
    staged.last_result = s.last_result.take();
    *s = staged;
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
