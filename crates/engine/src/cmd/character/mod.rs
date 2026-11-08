//! Characters as app documents. Scribbles (Codex) marks the parts, PhotoCraft cuts them,
//! EffectCraft rigs, animates and renders them through its native Rust APIs; the `.psd`, `.pcraft` and `.ecproj` files open directly in the apps.
//!
//! Layout: `<project>/assets/characters/<id>/<view>/` holds `parts.json`, `<id>-<view>.psd`,
//! `<id>-<view>.pcraft`, `<id>-<view>.ecproj` and rendered `frames/`.

mod action;
pub(crate) mod cut;
mod mark;
mod meta;
pub mod review;
mod rig;
mod shot;
mod sprite;
pub mod tripo;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

pub use action::{cmd_character_action, cmd_character_actions};
pub use cut::cmd_character_cut;
pub use mark::cmd_character_mark;
pub use meta::{cmd_character_meta, cmd_character_save_marks};
pub use rig::cmd_character_rig;
pub use shot::cmd_character_shot;
pub use sprite::cmd_character_sprite;
pub use tripo::{cmd_tripo_animate, cmd_tripo_breakout, cmd_tripo_catalog, cmd_tripo_rig};
/// The shared skeleton and its key poses, applied to every character.
pub(crate) const POSES: &str = include_str!("poses.json");

fn bad(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

/// Resolve the character folder under `<project>/assets/characters/`, matching existing directories case-insensitively.
pub(crate) fn character_folder(project_dir: &Path, id: &str) -> PathBuf {
    let root = project_dir.join("assets/characters");
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.eq_ignore_ascii_case(id)
                || crate::workspace::sanitize_asset_id(&name) == id
                || id.starts_with(&format!("{}_", crate::workspace::sanitize_asset_id(&name)))
            {
                return entry.path();
            }
        }
    }
    root.join(id)
}

/// Find an initial cutout drawing for `<character_folder>` when no `<view>/parts.json` exists yet.
/// Prefers the intact cast/lineup composite over partial layer files in `parts/`.
pub(crate) fn find_initial_cutout(char_folder: &Path) -> Option<PathBuf> {
    if let (Some(name), Some(project)) = (char_folder.file_name().and_then(|n| n.to_str()), char_folder.parent().and_then(Path::parent).and_then(Path::parent))
    {
        for candidate in
            [project.join("assets/characters/approved-crops").join(format!("{name}_from-lineup-v2.png")), char_folder.join("parts/source/body-plate_v1.png")]
        {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let parts_dir = char_folder.join("parts");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&parts_dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && crate::workspace::has_extension(p, &["png", "webp", "jpg", "jpeg"]))
        .collect();
    files.sort();
    files.into_iter().next()
}

/// One drawn view of one character.
pub(crate) struct View {
    pub id: String,
    pub view: String,
    pub dir: PathBuf,
}

impl View {
    pub(crate) fn from_params(s: &Session, p: &Value) -> Result<Self> {
        let raw_id =
            p["character"].as_str().map(crate::workspace::sanitize_asset_id).filter(|id| !id.is_empty()).ok_or_else(|| bad("character=<id> is required"))?;
        let view = p["view"].as_str().unwrap_or("front").to_string();
        if !matches!(view.as_str(), "front" | "back" | "side") {
            return Err(bad("view must be front, back or side"));
        }
        let project_dir = crate::workshop::project_dir(s)?;
        let char_dir = character_folder(&project_dir, &raw_id);
        let id = char_dir.file_name().map(|n| crate::workspace::sanitize_asset_id(&n.to_string_lossy())).filter(|s| !s.is_empty()).unwrap_or(raw_id);
        let dir = char_dir.join(&view);
        Ok(Self { id, view, dir })
    }

    pub(crate) fn file(&self, ext: &str) -> PathBuf {
        self.dir.join(format!("{}-{}.{ext}", self.id, self.view))
    }

    pub(crate) fn comp(&self) -> String {
        format!("{}-{}", self.id, self.view)
    }

    pub(crate) fn parts(&self) -> Result<Value> {
        let path = self.dir.join("parts.json");
        let text = std::fs::read_to_string(&path).map_err(|_| bad(format!("{} has no parts.json; run character.mark first", self.dir.display())))?;
        serde_json::from_str(&text).map_err(|e| bad(format!("{}: {e}", path.display())))
    }

    /// Write `parts.json` atomically. Not an undoable session document: the commands that change
    /// it also change PhotoCraft and EffectCraft files that undo cannot restore.
    pub(crate) fn write_parts(&self, parts: &Value) -> Result<()> {
        crate::doc_state::write_json(&self.dir.join("parts.json"), parts)?;
        Ok(())
    }

    pub(crate) fn require(&self, ext: &str, step: &str) -> Result<PathBuf> {
        let path = self.file(ext);
        if path.is_file() { Ok(path) } else { Err(bad(format!("{} is missing; run {step} first", path.display()))) }
    }
}

fn resolve_effectcraft_params(base: &Path, id: &str, params: &Value) -> Value {
    if !matches!(id, "file.open" | "file.save" | "file.saveAs" | "file.import" | "layer.newModel") {
        return params.clone();
    }
    let mut out = params.clone();
    if let Some(p) = out.get("path").and_then(Value::as_str).filter(|s| !s.is_empty()) {
        let path = Path::new(p);
        let full = if path.is_absolute() { path.to_path_buf() } else { base.join(path) };
        out["path"] = json!(full);
    }
    if let Some(paths) = out.get("paths").and_then(Value::as_array) {
        let resolved: Vec<Value> = paths
            .iter()
            .map(|v| match v.as_str() {
                Some(s) if !s.is_empty() => {
                    let path = Path::new(s);
                    let full = if path.is_absolute() { path.to_path_buf() } else { base.join(path) };
                    json!(full)
                }
                _ => v.clone(),
            })
            .collect();
        out["paths"] = json!(resolved);
    }
    out
}

/// Runs `steps` in the open EffectCraft window, or `None` if it does not have `path` open.
pub(crate) fn effect_in_window(path: &Path, steps: &[(String, Value)], save: bool) -> Option<Result<Vec<Value>>> {
    let (first, rest) = steps.split_first()?;
    let first_result = crate::CraftHub::try_live(crate::CraftTarget::Effect, path, &first.0, &first.1, save && rest.is_empty())?;
    let mut out = match first_result {
        Ok(value) => vec![value],
        Err(error) => return Some(Err(error)),
    };
    for (index, (id, params)) in rest.iter().enumerate() {
        let last = index + 1 == rest.len();
        match crate::CraftHub::try_live(crate::CraftTarget::Effect, path, id, params, save && last) {
            Some(Ok(value)) => out.push(value),
            Some(Err(error)) => return Some(Err(error)),
            None => return Some(Err(EngineError::ExternalTool("effectcraft".into(), format!("EffectCraft closed during {id}")))),
        }
    }
    Some(Ok(out))
}

/// Run EffectCraft commands in the open EffectCraft window when it has this project, otherwise
/// through [`crate::craft_hub::CraftDoc`] and the installed CLI.
pub(crate) fn effectcraft(dir: &Path, project: Option<&Path>, steps: &[Value], save: bool) -> Result<Vec<Value>> {
    let base = project.and_then(Path::parent).unwrap_or(dir);
    let mut resolved = Vec::with_capacity(steps.len());
    for step in steps {
        let id = step["id"].as_str().or_else(|| step["command"].as_str()).ok_or_else(|| bad(format!("missing command id in {step}")))?;
        let raw_params = step.get("params").cloned().unwrap_or_else(|| json!({}));
        let params = resolve_effectcraft_params(base, id, &raw_params);
        resolved.push((id.to_string(), params));
    }

    if let Some(path) = project.filter(|p| p.is_file())
        && let Some(first) = resolved.first()
        && let Some(first_res) = crate::CraftHub::try_live(crate::CraftTarget::Effect, path, &first.0, &first.1, false)
    {
        let mut out = Vec::with_capacity(resolved.len());
        out.push(first_res?);
        for (idx, (id, params)) in resolved.iter().enumerate().skip(1) {
            let save_step = save && idx + 1 == resolved.len();
            let v = crate::CraftHub::try_live(crate::CraftTarget::Effect, path, id, params, save_step)
                .unwrap_or_else(|| Err(EngineError::ExternalTool("effectcraft".into(), format!("lost live control connection during {id}"))))?;
            out.push(v);
        }
        if save && resolved.len() == 1 {
            let _ = crate::CraftHub::try_live(crate::CraftTarget::Effect, path, "file.save", &json!({}), false);
        }
        return Ok(out);
    }

    if let Some(path) = project.filter(|p| p.is_file())
        && let Some(result) = effect_in_window(path, &resolved, save)
    {
        return result;
    }

    let target_path = project.map(Path::to_path_buf).unwrap_or_else(|| dir.join("scratch.ecproj"));
    let mut doc = crate::craft_hub::CraftDoc::open_or_create(crate::CraftTarget::Effect, &target_path)?;
    let borrowed: Vec<(&str, Value)> = resolved.iter().map(|(id, params)| (id.as_str(), params.clone())).collect();
    let out = doc.batch(&borrowed)?;
    if save && project.is_some() {
        doc.save()?;
    }
    Ok(out)
}

pub(crate) fn effectcraft_frame(project: &Path, comp: &str, seconds: f64, max_side: u32, output: &Path) -> Result<()> {
    // Via installed effectcraft-cli render-frame (system app pattern)
    let cli = crate::workspace::find_executable("effectcraft-cli").map_err(|e| EngineError::ExternalTool("effectcraft-cli".into(), e.to_string()))?;
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let out = std::process::Command::new(&cli)
        .arg("--project")
        .arg(project)
        .arg("render-frame")
        .arg("--comp")
        .arg(comp)
        .arg("--time")
        .arg(seconds.max(0.0).to_string())
        .arg("--max-side")
        .arg(max_side.to_string())
        .arg("--transparent")
        .arg("--out")
        .arg(output)
        .output()
        .map_err(|e| EngineError::ExternalTool("effectcraft-cli".into(), e.to_string()))?;
    if !out.status.success() {
        return Err(EngineError::ExternalTool("effectcraft-cli".into(), format!("exit {:?}: {}", out.status.code(), String::from_utf8_lossy(&out.stderr))));
    }
    Ok(())
}

pub(crate) fn effectcraft_render(project: &Path, params: Value) -> Result<Value> {
    let cli = crate::workspace::find_executable("effectcraft-cli").map_err(|e| EngineError::ExternalTool("effectcraft-cli".into(), e.to_string()))?;
    let output_str = params["output"].as_str().ok_or_else(|| bad("output path is required for effectcraft_render"))?;
    let output_path = Path::new(output_str);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut cmd = std::process::Command::new(&cli);
    cmd.arg("--json").arg("--project").arg(project).arg("render");
    if let Some(comp) = params["comp"].as_str().filter(|s| !s.is_empty()) {
        cmd.arg("--comp").arg(comp);
    } else if let Some(comp_id) = params["comp"].as_u64() {
        cmd.arg("--comp").arg(comp_id.to_string());
    }
    cmd.arg("--out").arg(output_path);
    if let Some(fmt) = params["format"].as_str() {
        cmd.arg("--format").arg(fmt);
    }
    if let Some(start) = params["start"].as_f64() {
        cmd.arg("--start").arg(start.to_string());
    }
    if let Some(end) = params["end"].as_f64() {
        cmd.arg("--end").arg(end.to_string());
    }
    if let Some(fps) = params["frameRate"].as_f64().or_else(|| params["fps"].as_f64()) {
        cmd.arg("--fps").arg(fps.to_string());
    }
    if let Some(res) = params["resolution"].as_str() {
        cmd.arg("--resolution").arg(res);
    }
    if let Some(channels) = params["channels"].as_str() {
        cmd.arg("--channels").arg(channels);
    }
    if let Some(prores) = params["proresProfile"].as_str() {
        cmd.arg("--prores").arg(prores);
    }
    let out = cmd.output().map_err(|e| EngineError::ExternalTool("effectcraft-cli".into(), e.to_string()))?;
    if !out.status.success() {
        return Err(EngineError::ExternalTool(
            "effectcraft-cli".into(),
            format!("exit {:?}: {}", out.status.code(), String::from_utf8_lossy(&out.stderr).trim()),
        ));
    }
    serde_json::from_slice(&out.stdout).or_else(|_| Ok(json!({"ok": true, "output": output_path})))
}

pub(crate) fn path_arg(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub(crate) fn step(command: &str, params: Value) -> Value {
    json!({"command": command, "params": params})
}

/// The parts in back-to-front order, each with `id`, `parent`, `joint` and `outline`.
pub(crate) fn part_list(parts: &Value) -> Result<&Vec<Value>> {
    parts["parts"].as_array().filter(|a| !a.is_empty()).ok_or_else(|| bad("parts.json lists no parts"))
}

/// The one part without a parent: the body everything else hangs from.
pub(crate) fn root_part(parts: &[Value]) -> Result<&str> {
    let mut roots = parts.iter().filter(|p| p["parent"].is_null());
    let root = roots.next().and_then(|p| p["id"].as_str());
    match (root, roots.next()) {
        (Some(id), None) => Ok(id),
        _ => Err(bad("parts.json needs exactly one part without a parent")),
    }
}

pub(crate) fn point(value: &Value) -> Option<[f64; 2]> {
    Some([value.get(0)?.as_f64()?, value.get(1)?.as_f64()?])
}

/// List characters and which app files each view has.
pub fn cmd_character_list(s: &mut Session, _: &Value) -> Result<Value> {
    let project_dir = crate::workshop::project_dir(s)?;
    let cast: Vec<String> = s
        .project_path
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|d| d["animation_cast"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    let root = project_dir.join("assets/characters");
    // Character folders in the project, then 3D models registered where they live.
    let mut ids: Vec<String> = std::fs::read_dir(&root).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    for asset in s.project_assets.iter().filter(|a| a.kind == "model_3d") {
        let id = asset.id.replace('_', "-");
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    let mut out = Vec::new();
    for id in ids {
        if !cast.is_empty() && !cast.contains(&id) {
            continue;
        }
        let folder = root.join(&id);
        let canonical_id = crate::workspace::sanitize_asset_id(&id);
        let mut views: Vec<Value> = ["front", "side", "back"]
            .iter()
            .filter(|v| folder.join(v).is_dir())
            .map(|v| {
                let dir = folder.join(v);
                let has = |ext: &str| dir.join(format!("{canonical_id}-{v}.{ext}")).is_file() || dir.join(format!("{id}-{v}.{ext}")).is_file();
                let drawing = std::fs::read_to_string(dir.join("parts.json"))
                    .ok()
                    .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                    .and_then(|p| p["image"].as_str().map(|i| dir.join(i)))
                    .or_else(|| {
                        let staged = dir.join(format!("{canonical_id}-{v}-drawing.png"));
                        staged.is_file().then_some(staged)
                    });
                json!({"view": v, "parts": dir.join("parts.json").is_file(), "drawing": drawing,
                    "psd": has("psd"), "ecproj": has("ecproj")})
            })
            .collect();
        if views.is_empty()
            && let Some(cutout) = find_initial_cutout(&folder)
        {
            views.push(json!({"view": "front", "parts": false, "drawing": cutout, "psd": false, "ecproj": false}));
        }
        let tripo_3d = tripo::inspect_character_3d(s, &project_dir, &canonical_id);
        if !views.is_empty() || tripo_3d["available"].as_bool() == Some(true) {
            out.push(json!({"id": canonical_id, "views": views, "tripo_3d": tripo_3d}));
        }
    }
    Ok(json!({"ok": true, "characters": out}))
}

/// Render one frame of the rigged character with EffectCraft.
pub fn cmd_character_frame(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    let time = p["time"].as_f64().unwrap_or(0.0).max(0.0);
    let side = p["max_side"].as_u64().unwrap_or(900).clamp(64, 4096);
    let frames = view.dir.join("frames");
    std::fs::create_dir_all(&frames)?;
    let out = frames.join(format!("{}-{:07.3}.png", view.comp(), time));
    effectcraft_frame(&project, &view.comp(), time, side as u32, &out)?;
    Ok(json!({"ok": true, "frame": out, "time": time, "project": project}))
}

fn rendered_pngs(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut frames: Vec<PathBuf> = std::fs::read_dir(dir)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "png")).collect();
    frames.sort();
    Ok(frames)
}

/// The folder, `.ecproj` and comp to render: a sprite (`sprite=<name>`) or a rigged view.
fn render_target(s: &Session, p: &Value) -> Result<(PathBuf, PathBuf, String)> {
    if let Some(name) = p["sprite"].as_str() {
        let id = p["character"].as_str().map(crate::workspace::sanitize_asset_id).ok_or_else(|| bad("character=<id> is required"))?;
        let dir = sprite::sprite_dir(s, &id, name)?;
        let comp = format!("{id}-{name}");
        let project = dir.join(format!("{comp}.ecproj"));
        if !project.is_file() {
            return Err(bad(format!("{} is missing; run character.sprite first", project.display())));
        }
        return Ok((dir, project, comp));
    }
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    Ok((view.dir.clone(), project, view.comp()))
}

/// Render a time range as a PNG sequence (canvas playback) or a ProRes 4444 clip with alpha
/// (for the film), in one EffectCraft render.
pub fn cmd_character_render(s: &mut Session, p: &Value) -> Result<Value> {
    let external = p["effectcraft_project"].as_str();
    let (base, project, name, comp) = if let Some(path) = external {
        let dir = crate::workshop::project_dir(s)?;
        let project = dir.join(path).canonicalize()?;
        if !project.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("ecproj")) {
            return Err(bad("Choose an EffectCraft .ecproj file"));
        }
        let name = project.file_stem().and_then(|stem| stem.to_str()).unwrap_or("animation");
        let name = crate::workspace::sanitize_asset_id(name);
        (dir.join("film"), project, name, p["comp"].clone())
    } else {
        let (base, project, comp) = render_target(s, p)?;
        (base, project, comp.clone(), json!(comp))
    };
    let start = p["start"].as_f64().unwrap_or(0.0).max(0.0);
    let end = p["end"].as_f64().filter(|e| *e > start).unwrap_or(start + 1.0);
    let fps = p["fps"].as_u64().unwrap_or(24).clamp(1, 60);
    let fmt_req = p["format"].as_str().unwrap_or("png");
    let clip = fmt_req == "clip" || fmt_req == "prores";
    let gif = fmt_req == "gif";
    let name = if external.is_some() {
        let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|error| bad(error.to_string()))?.as_nanos();
        format!("{name}-{timestamp}")
    } else {
        format!("{name}-{start:.2}-{end:.2}")
    };
    let (dir, out) = if clip {
        let dir = base.join("clips");
        (dir.clone(), dir.join(format!("{name}.mov")))
    } else if gif {
        let dir = base.join("clips");
        (dir.clone(), dir.join(format!("{name}.gif")))
    } else {
        let dir = base.join("frames").join(&name);
        (dir.clone(), dir.join("frame.png"))
    };
    std::fs::create_dir_all(&dir)?;
    effectcraft_render(
        &project,
        json!({
            "comp": comp, "output": out, "start": start, "end": end, "frameRate": fps,
            "channels": "rgba", "format": if clip { "prores" } else if gif { "gif" } else { "png" },
            "proresProfile": "4444", "resolution": if clip { "full" } else { "half" }
        }),
    )?;
    let frames = if clip || gif { Vec::new() } else { rendered_pngs(&dir)? };
    Ok(json!({"ok": true, "start": start, "end": end, "fps": fps,
        "clip": (clip || gif).then_some(&out), "frames": frames}))
}

/// The project's own animation documents, which live outside the character folders.
fn animation_documents(s: &Session) -> Result<Vec<Value>> {
    let root = crate::workshop::project_dir(s)?;
    let mut paths: Vec<PathBuf> = ["motion", "lettering"]
        .iter()
        .flat_map(|folder| std::fs::read_dir(root.join(folder)).into_iter().flatten().flatten())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "ecproj"))
        .collect();
    paths.sort();
    Ok(paths.iter().map(|path| json!({"name": path.file_stem().map(|n| n.to_string_lossy().into_owned()), "path": path})).collect())
}

/// What the Animation tab shows: the characters, and the selection with its frame and last clip.
pub fn cmd_animation_view(s: &mut Session, p: &Value) -> Result<Value> {
    let list = cmd_character_list(s, &json!({}))?;
    let all = list["characters"].as_array().cloned().unwrap_or_default();
    let views_of = |c: &Value| c["views"].as_array().into_iter().flatten().filter_map(|v| v["view"].as_str().map(str::to_owned)).collect::<Vec<_>>();
    let characters: Vec<Value> = all.iter().map(|c| json!({"id": c["id"], "views": views_of(c)})).collect();
    let documents = animation_documents(s)?;
    let chosen = p["character"].as_str().and_then(|id| all.iter().find(|c| c["id"] == id)).or_else(|| all.iter().find(|c| !views_of(c).is_empty()));
    let Some(character) = chosen else {
        return Ok(json!({"ok": true, "characters": characters, "documents": documents, "selected": null}));
    };
    let id = character["id"].as_str().unwrap_or_default();
    let view = p["view"].as_str().map(str::to_owned).or_else(|| views_of(character).first().cloned()).unwrap_or_else(|| "front".into());
    let time = p["time"].as_f64().unwrap_or(0.0).max(0.0);
    let frame = cmd_character_frame(s, &json!({"character": id, "view": view, "time": time})).ok().map(|f| f["frame"].clone());
    let folder = character_folder(&crate::workshop::project_dir(s)?, id);
    let clip = [folder.join(&view).join("clips"), folder.join("clips")]
        .iter()
        .flat_map(|dir| std::fs::read_dir(dir).into_iter().flatten().flatten())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "mov" || ext == "mp4"))
        .max_by_key(|path| std::fs::metadata(path).and_then(|m| m.modified()).ok());
    Ok(
        json!({"ok": true, "characters": characters, "documents": documents, "selected": {"character": id, "view": view, "time": time, "frame": frame, "clip": clip}}),
    )
}

/// `animation.open {path}`: a project animation document in the EffectCraft window.
pub fn cmd_animation_open(s: &mut Session, p: &Value) -> Result<Value> {
    let path = p["path"]
        .as_str()
        .map(|raw| crate::workspace::resolve_workspace_path(s, raw))
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "ecproj"))
        .ok_or_else(|| bad("path=<existing .ecproj> is required"))?;
    crate::CraftHub::open_live(crate::CraftTarget::Effect, &path)?;
    Ok(json!({"ok": true, "app": "effectcraft", "file": path, "document": {"app": "effectcraft", "file": path}}))
}

/// Add an image or model file as a character, named after the file.
pub fn cmd_character_import(s: &mut Session, p: &Value) -> Result<Value> {
    let path = p["path"].as_str().ok_or_else(|| bad("path=<file> is required"))?;
    let name = Path::new(path).file_stem().and_then(|n| n.to_str()).unwrap_or("character");
    crate::project::cmd_project_add_asset(s, &json!({"path": path, "character": crate::workspace::sanitize_asset_id(name), "name": name, "kind": "image"}))
}

/// Key one transform value of one part at one time (a canvas drag).
pub fn cmd_character_set(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    let part = p["part"].as_str().ok_or_else(|| bad("part=<layer> is required"))?;
    let prop = p["prop"].as_str().unwrap_or("rotation");
    if !matches!(prop, "position" | "rotation" | "scale" | "anchor" | "opacity") {
        return Err(bad("prop must be position, rotation, scale, anchor or opacity"));
    }
    let value = p.get("value").cloned().ok_or_else(|| bad("value is required"))?;
    let time = p["time"].as_f64().unwrap_or(0.0);
    let key = json!({"layer": part, "path": format!("transform/{prop}"), "time": time,
        "timeBase": "comp", "value": value});
    effectcraft(&view.dir, Some(&project), &[step("comp.open", json!({"comp": view.comp()})), step("prop.addKey", key)], true)?;
    Ok(json!({"ok": true, "project": project, "part": part, "prop": prop, "time": time}))
}

/// Open the character's document in the app that owns it, for edits Studio does not offer.
pub fn cmd_character_open(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    if p["app"].as_str() == Some("blender") {
        return tripo::open_in_blender(s, &view.id);
    }
    let (app, file) = match p["app"].as_str().unwrap_or("effectcraft") {
        "effectcraft" => ("effectcraft", view.require("ecproj", "character.rig")?),
        "photocraft" => ("photocraft", view.require("psd", "character.cut")?),
        other => {
            return Err(bad(format!("app must be effectcraft, photocraft or blender, not {other}")));
        }
    };
    if app == "effectcraft" {
        crate::CraftHub::open_live(crate::CraftTarget::Effect, &file)?;
    } else {
        crate::workspace::open_editor(app, &file)?;
    }
    Ok(json!({"ok": true, "app": app, "file": file, "document": {"app": app, "file": file}}))
}
