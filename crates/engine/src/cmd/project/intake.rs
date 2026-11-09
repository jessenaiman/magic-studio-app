//! Workshop file intake, audio/target linking, PhotoCraft panel grid splitting,
//! draft approval, and project file catalog/trash handlers.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::cmd::project::cmd_project_add_asset;
use crate::cmd::project::workshop::{
    ASSETS_DIR, IMAGE_EXTENSIONS, STORYBOARD_DIR, STORYBOARD_PDF, VIDEO_TARGETS, files_in, panel_files, project_dir, transcript_files, video_project_path,
    workshop_asset_files, workshop_status,
};
use crate::workspace::{has_extension, resolve_workspace_path, song_audio_path};
use crate::{EngineError, Result, Session};

fn numbered_prompts(text: &str) -> std::collections::BTreeMap<usize, String> {
    let mut out = std::collections::BTreeMap::new();
    let mut current: Option<usize> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        let number = trimmed.split_once(". ").and_then(|(n, rest)| n.parse::<usize>().ok().map(|n| (n, rest)));
        match number {
            Some((n, rest)) => {
                let rest = rest.trim_start();
                let rest = if rest.starts_with('(') { rest.split_once(") ").map_or(rest, |(_, r)| r) } else { rest };
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

pub fn cmd_workshop_set_audio(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let raw = params.get("path").and_then(Value::as_str).ok_or_else(|| EngineError::InvalidParams("missing audio 'path'".into()))?;
    let audio = song_audio_path(s, raw)?;
    s.song_audio = Some(audio.clone());
    s.video_target = "filmcraft".into();
    crate::cmd::project::write_project_file(s)?;
    crate::cmd::film::assembly::ensure_song_film(s, &audio)?;
    let sb = dir.join("storyboard.designcraft");
    if !sb.is_file() {
        let _ = crate::cmd::storyboard::cmd_storyboard_new(s, &json!({"path": sb, "overwrite": false}));
    } else {
        let _ = crate::cmd::storyboard::cmd_storyboard_export(s, &json!({"path": sb, "pdf": dir.join("storyboard.pdf"), "pptx": dir.join("storyboard.pptx")}));
    }
    let mut status = workshop_status(s)?;
    status["audio"] = json!(audio);
    Ok(status)
}

pub fn cmd_workshop_set_target(s: &mut Session, params: &Value) -> Result<Value> {
    project_dir(s)?;
    let target = params.get("target").and_then(Value::as_str).unwrap_or("filmcraft");
    if !VIDEO_TARGETS.contains(&target) {
        return Err(EngineError::InvalidParams(format!("target must be one of {VIDEO_TARGETS:?}")));
    }
    s.video_target = target.into();
    crate::cmd::project::write_project_file(s)?;
    workshop_status(s)
}

fn add_folder(params: &Value) -> Result<(&'static str, Option<&'static [&'static str]>)> {
    match params.get("kind").and_then(Value::as_str) {
        Some("liner_notes") => Ok(("liner-notes", None)),
        Some("panels") => Ok((STORYBOARD_DIR, Some(IMAGE_EXTENSIONS))),
        Some("assets") => Ok((ASSETS_DIR, Some(IMAGE_EXTENSIONS))),
        _ => Err(EngineError::InvalidParams("kind must be liner_notes, panels or assets".into())),
    }
}

fn requested_paths(s: &Session, params: &Value) -> Result<Vec<PathBuf>> {
    let paths: Vec<PathBuf> =
        params.get("paths").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(|p| resolve_workspace_path(s, p)).collect();
    if paths.is_empty() {
        return Err(EngineError::InvalidParams("pass 'paths': [files]".into()));
    }
    Ok(paths)
}

fn require_addable(path: &Path, only: Option<&[&str]>) -> Result<()> {
    if !path.is_file() {
        return Err(EngineError::InvalidParams(format!("file not found: {}", path.display())));
    }
    if let Some(exts) = only
        && !has_extension(path, exts)
    {
        return Err(EngineError::InvalidParams(format!("not an image: {}", path.display())));
    }
    Ok(())
}

fn destination_for(path: &Path, folder: &str, target: &Path, next_panel: &mut u32) -> Result<PathBuf> {
    if folder != STORYBOARD_DIR {
        let name = path.file_name().ok_or_else(|| EngineError::InvalidParams("bad file name".into()))?;
        return Ok(target.join(name));
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("png");
    let name = format!("panel-{:02}.{ext}", *next_panel);
    *next_panel += 1;
    Ok(target.join(name))
}

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

fn connect_subjects(s: &mut Session, params: &Value, added: &[PathBuf], folder: &str) -> Result<Vec<String>> {
    let animate = params.get("animate").and_then(Value::as_bool).unwrap_or(false) || params.get("subject").and_then(Value::as_str).is_some();
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
    crate::cmd::project::write_project_file(s)?;
    Ok(connected)
}

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

fn grid_dims(params: &Value) -> (u32, u32, u32) {
    let columns = params.get("columns").and_then(Value::as_u64).unwrap_or(3).max(1) as u32;
    let rows = params.get("rows").and_then(Value::as_u64).unwrap_or(2).max(1) as u32;
    let inset = params.get("inset").and_then(Value::as_u64).unwrap_or(8) as u32;
    (columns, rows, inset)
}

fn crop_panels(_dir: &Path, image: &Path, out_dir: &Path, columns: u32, rows: u32, inset: u32, (cw, ch): (u32, u32)) -> Result<Vec<PathBuf>> {
    let src = image::open(image)?;
    let mut panels = Vec::new();
    for r in 0..rows {
        for c in 0..columns {
            let n = r * columns + c + 1;
            let out = out_dir.join(format!("panel-{n:02}.png"));
            let x = c * cw + inset;
            let y = r * ch + inset;
            let w = cw - 2 * inset;
            let h = ch - 2 * inset;
            let cropped = src.crop_imm(x, y, w, h);
            cropped.save(&out)?;
            panels.push(out);
        }
    }
    Ok(panels)
}

fn write_prompts(s: &Session, params: &Value, panels: &[PathBuf], image: &Path) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    let Some(prompt_file) = params.get("prompts").and_then(Value::as_str).map(|p| resolve_workspace_path(s, p)) else {
        return Ok(written);
    };
    let numbered = numbered_prompts(&std::fs::read_to_string(&prompt_file)?);
    let source = prompt_file.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
    for (i, panel) in panels.iter().enumerate() {
        let Some(text) = numbered.get(&(i + 1)) else {
            continue;
        };
        let out = panel.with_extension("prompt.md");
        crate::doc_state::write_file(
            &out,
            format!("{text}\n\nSource: {source} (panel {} of {})\n", i + 1, image.file_name().and_then(|n| n.to_str()).unwrap_or_default()),
        )?;
        written.push(out);
    }
    Ok(written)
}

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
        return Err(EngineError::InvalidParams("panels too small for that grid and inset".into()));
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

pub fn cmd_workshop_approve_draft(s: &mut Session, params: &Value) -> Result<Value> {
    project_dir(s)?;
    let approved = params.get("approved").and_then(Value::as_bool).unwrap_or(true);
    s.draft_approved = approved;
    crate::cmd::project::write_project_file(s)?;
    workshop_status(s)
}

fn add_catalog_row(rows: &mut Vec<Value>, dir: &Path, kind: &str, path: PathBuf, filter: &str) {
    let rel = path.strip_prefix(dir).unwrap_or(&path).to_string_lossy().into_owned();
    if !filter.is_empty() && !rel.to_ascii_lowercase().contains(filter) && !kind.to_ascii_lowercase().contains(filter) {
        return;
    }
    let bytes = std::fs::metadata(&path).map(|m| m.len()).ok();
    rows.push(json!({"kind": kind, "name": rel, "path": path, "bytes": bytes}));
}

pub fn cmd_workshop_files(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let filter = params.get("filter").and_then(Value::as_str).map(str::to_ascii_lowercase).unwrap_or_default();
    let mut rows = Vec::new();
    if let Some(audio) = s.song_audio.clone().filter(|p| p.is_file()) {
        add_catalog_row(&mut rows, &dir, "audio", audio, &filter);
    }
    for p in files_in(&dir.join("liner-notes"), None) {
        add_catalog_row(&mut rows, &dir, "liner_notes", p, &filter);
    }
    for p in transcript_files(&dir) {
        add_catalog_row(&mut rows, &dir, "transcript", p, &filter);
    }
    for p in panel_files(&dir) {
        add_catalog_row(&mut rows, &dir, "panel", p, &filter);
    }
    let pdf = dir.join(STORYBOARD_PDF);
    if pdf.is_file() {
        add_catalog_row(&mut rows, &dir, "storyboard_pdf", pdf, &filter);
    }
    for p in workshop_asset_files(&dir) {
        add_catalog_row(&mut rows, &dir, "asset", p, &filter);
    }
    if let Some(vp) = video_project_path(&dir) {
        add_catalog_row(&mut rows, &dir, "video_project", vp, &filter);
    }
    Ok(json!({"ok": true, "project_dir": dir, "count": rows.len(), "files": rows}))
}

fn trash_destination(trash_dir: &Path, file_name: &std::ffi::OsStr) -> PathBuf {
    let mut dest = trash_dir.join(file_name);
    let mut n = 2u32;
    while dest.exists() {
        let stem = Path::new(file_name).file_stem().and_then(|s| s.to_str()).unwrap_or("file");
        let ext = Path::new(file_name).extension().and_then(|s| s.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
        dest = trash_dir.join(format!("{stem}-{n}{ext}"));
        n += 1;
    }
    dest
}

pub fn cmd_workshop_remove(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let raw = params.get("path").and_then(Value::as_str).ok_or_else(|| EngineError::InvalidParams("missing 'path' to remove".into()))?;
    let target = resolve_workspace_path(s, raw);
    let canon_dir = dir.canonicalize()?;
    let canon_target = target.canonicalize().map_err(|_| EngineError::InvalidParams(format!("file not found: {}", target.display())))?;
    if !canon_target.starts_with(&canon_dir) || canon_target == canon_dir {
        return Err(EngineError::InvalidParams("can only remove files inside the open project folder".into()));
    }
    if Some(&canon_target) == s.project_path.as_ref().and_then(|p| p.canonicalize().ok()).as_ref() {
        return Err(EngineError::InvalidParams("refusing to remove the .magic project file itself".into()));
    }
    let trash_dir = dir.join(".trash");
    std::fs::create_dir_all(&trash_dir)?;
    let file_name = canon_target.file_name().ok_or_else(|| EngineError::InvalidParams("invalid file name".into()))?;
    let moved_to = trash_destination(&trash_dir, file_name);
    std::fs::rename(&canon_target, &moved_to)?;
    if s.song_audio.as_ref().and_then(|p| p.canonicalize().ok()).as_ref() == Some(&canon_target) {
        s.song_audio = None;
    }
    s.project_assets.retain(|a| a.path.as_ref().and_then(|p| p.canonicalize().ok()).as_ref() != Some(&canon_target));
    crate::cmd::project::write_project_file(s)?;
    let mut status = workshop_status(s)?;
    status["removed"] = json!(canon_target);
    status["trashed_to"] = json!(moved_to);
    Ok(status)
}
