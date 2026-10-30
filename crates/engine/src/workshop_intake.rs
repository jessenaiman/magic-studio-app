//! Workshop file intake, audio/target linking, PhotoCraft panel grid splitting,
//! draft approval, and project file catalog/trash handlers.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};
use crate::project::cmd_project_add_asset;
use crate::video_target::ensure_video_target_project;
use crate::workshop::{
    ASSETS_DIR, IMAGE_EXTENSIONS, STORYBOARD_DIR, STORYBOARD_PDF, VIDEO_TARGETS, files_in,
    panel_files, project_dir, transcript_files, video_project_path, workshop_asset_files,
    workshop_status,
};
use crate::workspace::{has_extension, resolve_workspace_path, run_tool_in, song_audio_path};

/// numbered_prompts parses panel descriptions from a generating prompt.
fn numbered_prompts(text: &str) -> std::collections::BTreeMap<usize, String> {
    let mut out = std::collections::BTreeMap::new();
    let mut current: Option<usize> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        let number = trimmed
            .split_once(". ")
            .and_then(|(n, rest)| n.parse::<usize>().ok().map(|n| (n, rest)));
        match number {
            Some((n, rest)) => {
                let rest = rest.trim_start();
                let rest = if rest.starts_with('(') {
                    rest.split_once(") ").map_or(rest, |(_, r)| r)
                } else {
                    rest
                };
                out.insert(n, rest.to_string());
                current = Some(n);
            }
            None if trimmed.is_empty() => current = None,
            None => {
                if let Some(n) = current
                    && let Some(entry) = out.get_mut(&n)
                {
                    entry.push(' ');
                    entry.push_str(trimmed);
                }
            }
        }
    }
    out
}

/// The audio already connected to this project: the session path, else the copy beside the video.
fn connected_audio(s: &Session, dir: &Path, audio: &Path) -> Option<PathBuf> {
    s.song_audio.clone().filter(|path| path.is_file()).or_else(|| {
        audio
            .file_name()
            .map(|name| dir.join("hyperframes").join(name))
            .filter(|path| path.is_file())
    })
}

/// Same path, or the same bytes: relinking an identical recording is not a change.
fn same_audio(old: &Path, audio: &Path) -> bool {
    old == audio
        || std::fs::read(old)
            .ok()
            .zip(std::fs::read(audio).ok())
            .is_some_and(|(old, new)| old == new)
}

type AudioState = (Option<PathBuf>, String, Option<PathBuf>);

fn audio_state(s: &Session) -> AudioState {
    (
        s.song_audio.clone(),
        s.video_target.clone(),
        s.video_project_path.clone(),
    )
}

fn restore_audio_state(s: &mut Session, previous: AudioState) {
    (s.song_audio, s.video_target, s.video_project_path) = previous;
}

/// The HyperFrames project and its preview server, then the project file that records both.
fn hyperframes_setup(s: &mut Session, params: &Value) -> Result<Value> {
    let video = crate::video_target::cmd_video_init(
        s,
        &json!({"target":"hyperframes","captions":false,"resolve_video_cli":params["resolve_video_cli"]}),
    )?;
    let preview = crate::video_target::cmd_video_server_start(
        s,
        &json!({"resolve_video_cli":params["resolve_video_cli"]}),
    )?;
    crate::project::write_project_file(s)?;
    Ok(json!({"ok":true,"video":video,"preview":preview}))
}

pub fn cmd_workshop_set_audio(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let raw = params
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::InvalidParams("missing audio 'path'".into()))?;
    let audio = song_audio_path(s, raw)?;
    let exists = ensure_video_target_project(s, &dir, "hyperframes").is_ok();
    let connected = connected_audio(s, &dir, &audio);
    let unchanged = connected.as_ref().is_some_and(|old| same_audio(old, &audio));
    if exists && !unchanged {
        return Err(EngineError::InvalidParams("This HyperFrames project already has a timeline. Update its audio in HyperFrames before relinking the recording; the existing project was preserved".into()));
    }
    let previous = audio_state(s);
    s.song_audio = Some(if exists && unchanged {
        connected.ok_or_else(|| {
            EngineError::InvalidParams("Existing audio connection is unavailable".into())
        })?
    } else {
        audio
    });
    s.video_target = "hyperframes".into();
    let setup = hyperframes_setup(s, params);
    if setup.is_err() {
        restore_audio_state(s, previous);
        crate::project::write_project_file(s)?;
    }
    setup
}

pub fn cmd_workshop_set_target(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let target = params
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !VIDEO_TARGETS.contains(&target) {
        return Err(EngineError::InvalidParams(format!(
            "target must be one of {VIDEO_TARGETS:?}"
        )));
    }
    s.video_target = target.into();
    // The new target's project replaces the old one; failing to scaffold it leaves none, never the previous target's.
    s.video_project_path = ensure_video_target_project(s, &dir, target).ok();
    crate::project::write_project_file(s)?;
    workshop_status(s)
}

/// The template folder a kind maps to, and the extensions it accepts.
fn add_folder(params: &Value) -> Result<(&'static str, Option<&'static [&'static str]>)> {
    match params.get("kind").and_then(Value::as_str) {
        Some("liner_notes") => Ok(("liner-notes", None)),
        Some("panels") => Ok((STORYBOARD_DIR, Some(IMAGE_EXTENSIONS))),
        Some("assets") => Ok((ASSETS_DIR, Some(IMAGE_EXTENSIONS))),
        _ => Err(EngineError::InvalidParams(
            "kind must be liner_notes, panels or assets".into(),
        )),
    }
}

fn requested_paths(s: &Session, params: &Value) -> Result<Vec<PathBuf>> {
    let paths: Vec<PathBuf> = params
        .get("paths")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|p| resolve_workspace_path(s, p))
        .collect();
    if paths.is_empty() {
        return Err(EngineError::InvalidParams("pass 'paths': [files]".into()));
    }
    Ok(paths)
}

/// A source file must exist, and must be an image when the folder only takes images.
fn require_addable(path: &Path, only: Option<&[&str]>) -> Result<()> {
    if !path.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "file not found: {}",
            path.display()
        )));
    }
    if let Some(exts) = only
        && !has_extension(path, exts)
    {
        return Err(EngineError::InvalidParams(format!(
            "not an image: {}",
            path.display()
        )));
    }
    Ok(())
}

/// Panels are numbered from the existing count; other folders keep their file name.
fn destination_for(
    path: &Path,
    folder: &str,
    target: &Path,
    next_panel: &mut u32,
) -> Result<PathBuf> {
    if folder != STORYBOARD_DIR {
        let name = path
            .file_name()
            .ok_or_else(|| EngineError::InvalidParams("bad file name".into()))?;
        return Ok(target.join(name));
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("png");
    let name = format!("panel-{:02}.{ext}", *next_panel);
    *next_panel += 1;
    Ok(target.join(name))
}

/// Copy without ever overwriting different bytes at the destination.
fn place_file(path: &Path, dest: &Path) -> Result<()> {
    if dest == path {
        return Ok(());
    }
    if dest.is_file() && std::fs::read(dest)? != std::fs::read(path)? {
        return Err(EngineError::InvalidParams(format!(
            "Material destination exists with different bytes: {}. Rename or explicitly resolve the collision; original retained.",
            dest.display()
        )));
    }
    if !dest.exists() {
        std::fs::copy(path, dest)?;
    }
    Ok(())
}

/// Static assets can become rig subjects when the call asks for animation.
fn connect_subjects(
    s: &mut Session,
    params: &Value,
    added: &[PathBuf],
    folder: &str,
) -> Result<Vec<String>> {
    let animate = params
        .get("animate")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || params.get("subject").and_then(Value::as_str).is_some();
    if folder != ASSETS_DIR || !animate {
        return Ok(Vec::new());
    }
    let explicit_name = params.get("subject").and_then(Value::as_str);
    let mut connected = Vec::new();
    for dest in added {
        let mut add_params = json!({
            "path": dest,
            "kind": "auto",
            "if_not_exists": true,
            "load": true,
        });
        if let Some(name) = explicit_name {
            add_params["name"] = json!(name);
        }
        if let Ok(res) = cmd_project_add_asset(s, &add_params)
            && let Some(id) = res.get("asset_id").and_then(Value::as_str)
        {
            connected.push(id.to_string());
        }
    }
    crate::project::write_project_file(s)?;
    Ok(connected)
}

/// Copies liner notes, storyboard panels or static assets into the template folders.
pub fn cmd_workshop_add(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let (folder, only) = add_folder(params)?;
    let paths = requested_paths(s, params)?;
    let target = dir.join(folder);
    std::fs::create_dir_all(&target)?;
    let mut next_panel = panel_files(&dir).len() as u32 + 1;
    let mut added = Vec::new();
    for path in &paths {
        require_addable(path, only)?;
        let dest = destination_for(path, folder, &target, &mut next_panel)?;
        place_file(path, &dest)?;
        added.push(dest);
    }
    let connected_subjects = connect_subjects(s, params, &added, folder)?;
    let mut status = workshop_status(s)?;
    status["added"] = json!(added);
    if !connected_subjects.is_empty() {
        status["connected_subjects"] = json!(connected_subjects);
    }
    Ok(status)
}

/// Cuts a storyboard grid into storyboard/panel-NN.png with PhotoCraft.
/// Grid shape and inset, defaulting to 3x2 panels with an 8px inset.
fn grid_dims(params: &Value) -> (u32, u32, u32) {
    let columns = params
        .get("columns")
        .and_then(Value::as_u64)
        .unwrap_or(3)
        .max(1) as u32;
    let rows = params
        .get("rows")
        .and_then(Value::as_u64)
        .unwrap_or(2)
        .max(1) as u32;
    let inset = params.get("inset").and_then(Value::as_u64).unwrap_or(8) as u32;
    (columns, rows, inset)
}

/// One PhotoCraft crop per panel, written as storyboard/panel-NN.png.
fn crop_panels(
    dir: &Path,
    image: &Path,
    out_dir: &Path,
    columns: u32,
    rows: u32,
    inset: u32,
    (cw, ch): (u32, u32),
) -> Result<Vec<PathBuf>> {
    let mut panels = Vec::new();
    for r in 0..rows {
        for c in 0..columns {
            let n = r * columns + c + 1;
            let out = out_dir.join(format!("panel-{n:02}.png"));
            let rect = json!({"x": c * cw + inset, "y": r * ch + inset, "width": cw - 2 * inset, "height": ch - 2 * inset});
            run_tool_in(
                dir,
                "photocraft-cli",
                &[
                    "run".into(),
                    image.to_string_lossy().into_owned(),
                    "--cmd".into(),
                    "image.crop".into(),
                    "--params".into(),
                    rect.to_string(),
                    "--out".into(),
                    out.to_string_lossy().into_owned(),
                ],
            )?;
            panels.push(out);
        }
    }
    Ok(panels)
}

/// A numbered prompt beside each panel whose prompt file has that panel number.
fn write_prompts(
    s: &Session,
    params: &Value,
    panels: &[PathBuf],
    image: &Path,
) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    let Some(prompt_file) = params
        .get("prompts")
        .and_then(Value::as_str)
        .map(|p| resolve_workspace_path(s, p))
    else {
        return Ok(written);
    };
    let numbered = numbered_prompts(&std::fs::read_to_string(&prompt_file)?);
    let source = prompt_file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();
    for (i, panel) in panels.iter().enumerate() {
        let Some(text) = numbered.get(&(i + 1)) else {
            continue;
        };
        let out = panel.with_extension("prompt.md");
        std::fs::write(
            &out,
            format!(
                "{text}\n\nSource: {source} (panel {} of {})\n",
                i + 1,
                image
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
            ),
        )?;
        written.push(out);
    }
    Ok(written)
}

/// Cuts a storyboard grid into storyboard/panel-NN.png with PhotoCraft.
pub fn cmd_workshop_split_panels(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let image = params
        .get("image")
        .and_then(Value::as_str)
        .map(|p| resolve_workspace_path(s, p))
        .ok_or_else(|| EngineError::InvalidParams("missing storyboard grid 'image'".into()))?;
    let (columns, rows, inset) = grid_dims(params);
    let (w, h) = image::image_dimensions(&image)?;
    let (cw, ch) = (w / columns, h / rows);
    if cw <= 2 * inset || ch <= 2 * inset {
        return Err(EngineError::InvalidParams(
            "panels too small for that grid and inset".into(),
        ));
    }
    let out_dir = dir.join(STORYBOARD_DIR);
    std::fs::create_dir_all(&out_dir)?;
    let panels = crop_panels(&dir, &image, &out_dir, columns, rows, inset, (cw, ch))?;
    let prompts_written = write_prompts(s, params, &panels, &image)?;
    let mut status = workshop_status(s)?;
    status["panels"] = json!(panels);
    status["prompts"] = json!(prompts_written);
    Ok(status)
}

/// Approve or unapprove the workshop draft.
pub fn cmd_workshop_approve_draft(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let approved = params
        .get("approved")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let has_board = dir.join(STORYBOARD_DIR).join(STORYBOARD_PDF).is_file()
        || dir.join("storyboard/movie-board.json").is_file();
    if approved && !has_board {
        return Err(EngineError::InvalidParams(
            "build the storyboard first (workshop.build_storyboard)".into(),
        ));
    }
    s.draft_approved = approved;
    crate::project::write_project_file(s)?;
    workshop_status(s)
}

/// Every file the workshop knows about, grouped by stage.
/// Every stage's files, then any other file in the project folder that none of them named.
fn collected_files(s: &Session, dir: &Path) -> Vec<Value> {
    let mut files = Vec::new();
    if let Some(audio) = &s.song_audio {
        files.push(file_entry("audio", audio, true));
    }
    for f in files_in(&dir.join("liner-notes"), None) {
        files.push(file_entry("liner_notes", &f, true));
    }
    for t in &transcript_files(dir) {
        files.push(file_entry("transcript", t, true));
    }
    if let Some(fcproj) = video_project_path(dir) {
        files.push(file_entry("transcript", &fcproj, true));
    }
    for f in files_in(&dir.join(STORYBOARD_DIR), None) {
        files.push(file_entry("storyboard", &f, true));
    }
    for f in workshop_asset_files(dir) {
        files.push(file_entry("assets", &f, true));
    }
    let listed: Vec<PathBuf> = files
        .iter()
        .filter_map(|f| f["path"].as_str().map(PathBuf::from))
        .collect();
    for f in files_in(dir, None) {
        if !listed.contains(&f) && Some(&f) != s.project_path.as_ref() {
            files.push(file_entry("other", &f, true));
        }
    }
    files
}

fn retain_group(files: &mut Vec<Value>, params: &Value) {
    let Some(group) = params
        .get("group")
        .and_then(Value::as_str)
        .map(str::to_ascii_lowercase)
        .filter(|g| !g.is_empty())
    else {
        return;
    };
    files.retain(|f| {
        f["group"]
            .as_str()
            .is_some_and(|g| g.to_ascii_lowercase() == group)
    });
}

fn retain_query(files: &mut Vec<Value>, params: &Value) {
    let Some(q) = params
        .get("filter")
        .or_else(|| params.get("query"))
        .and_then(Value::as_str)
        .map(str::to_ascii_lowercase)
        .filter(|q| !q.is_empty())
    else {
        return;
    };
    files.retain(|f| {
        ["name", "path", "group"]
            .iter()
            .any(|key| f[key].as_str().is_some_and(|v| v.to_ascii_lowercase().contains(&q)))
    });
}

/// Every file the workshop knows about, grouped by stage.
pub fn cmd_workshop_files(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let mut files = collected_files(s, &dir);
    retain_group(&mut files, params);
    retain_query(&mut files, params);
    Ok(json!({"ok": true, "project_dir": dir, "count": files.len(), "files": files}))
}

/// Removes a workshop file: song audio is only unlinked; project-folder files go to trash.
pub fn cmd_workshop_remove(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let raw = params
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::InvalidParams("missing 'path'".into()))?;
    let path = resolve_workspace_path(s, raw);
    if s.song_audio.as_ref() == Some(&path) {
        s.song_audio = None;
        crate::project::write_project_file(s)?;
        let mut status = workshop_status(s)?;
        status["unlinked"] = json!(path);
        return Ok(status);
    }
    let inside = path
        .canonicalize()
        .ok()
        .zip(dir.canonicalize().ok())
        .is_some_and(|(p, d)| p.starts_with(&d));
    if !inside || Some(&path) == s.project_path.as_ref() || !path.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "only files inside the project folder can be removed: {}",
            path.display()
        )));
    }
    run_tool_in(
        &dir,
        "gio",
        &["trash".into(), path.to_string_lossy().into_owned()],
    )?;
    if path == dir.join(STORYBOARD_DIR).join(STORYBOARD_PDF) {
        s.draft_approved = false;
        crate::project::write_project_file(s)?;
    }
    let mut status = workshop_status(s)?;
    status["trashed"] = json!(path);
    Ok(status)
}

/// One row in the workshop file listing: group, name, path, size, exists, removable.
fn file_entry(group: &str, path: &Path, removable: bool) -> Value {
    let size = std::fs::metadata(path).map(|m| m.len()).ok();
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    json!({"group": group, "name": name, "path": path, "size": size, "exists": path.exists(), "removable": removable})
}
