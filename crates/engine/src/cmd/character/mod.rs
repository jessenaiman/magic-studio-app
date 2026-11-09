//! Characters as app documents. Scribbles (Codex) marks the parts, PhotoCraft cuts them,
//! EffectCraft rigs, animates and renders them through its native Rust APIs; the `.psd`, `.pcraft` and `.ecproj` files open directly in the apps.
//!
//! Layout: `<project>/assets/characters/<id>/<view>/` holds `parts.json`, `<id>-<view>.psd`,
//! `<id>-<view>.pcraft`, `<id>-<view>.ecproj` and rendered `frames/`.

mod action;
pub(crate) mod cut;
pub mod effect;
pub mod keys;
mod mark;
mod meta;
pub mod path;
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
pub use effect::{
    cmd_expression_set, cmd_keyframe_easy_ease, cmd_keyframe_velocity, cmd_layer_new_3d_model, cmd_layer_set_blend_mode, cmd_layer_set_parent, cmd_mask_add,
    cmd_puppet_add_pin, cmd_puppet_move_pin, cmd_puppet_remove_pin, cmd_time_remap,
};
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
    let folder_name = char_folder.file_name().and_then(|n| n.to_str()).unwrap_or_default();
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
    for named in [
        char_folder.join("assembled.png"),
        char_folder.join("cutout.png"),
        char_folder.join(format!("{folder_name}-clean.png")),
        char_folder.join(format!("{folder_name}-drawing.png")),
        char_folder.join("flow-original.png"),
    ] {
        if named.is_file() {
            return Some(named);
        }
    }
    let parts_dir = char_folder.join("parts");
    if let Ok(rd) = std::fs::read_dir(&parts_dir) {
        let mut files: Vec<PathBuf> =
            rd.flatten().map(|e| e.path()).filter(|p| p.is_file() && crate::workspace::has_extension(p, &["png", "webp", "jpg", "jpeg"])).collect();
        files.sort();
        if let Some(first) = files.into_iter().next() {
            return Some(first);
        }
    }
    let sing_frame = char_folder.join("sprites/sing/frames/frame_0001.png");
    if sing_frame.is_file() {
        return Some(sing_frame);
    }
    if let Some(sp) =
        std::fs::read_dir(char_folder.join("sprites")).ok().and_then(|rd| rd.flatten().map(|e| e.path().join("frames/frame_0001.png")).find(|p| p.is_file()))
    {
        return Some(sp);
    }
    if let Ok(rd) = std::fs::read_dir(char_folder) {
        let mut direct: Vec<PathBuf> =
            rd.flatten().map(|e| e.path()).filter(|p| p.is_file() && crate::workspace::has_extension(p, &["png", "webp", "jpg", "jpeg"])).collect();
        direct.sort();
        if let Some(first) = direct.into_iter().next() {
            return Some(first);
        }
    }
    None
}

pub(crate) fn find_dir_ecproj(dir: &Path, id: &str, folder_name: &str, view: &str, is_flat: bool) -> Option<PathBuf> {
    let mut candidates = vec![dir.join(format!("{id}-{view}.ecproj")), dir.join(format!("{folder_name}-{view}.ecproj"))];
    if is_flat {
        candidates.extend([
            dir.join(format!("{folder_name}-rig.ecproj")),
            dir.join(format!("{id}-rig.ecproj")),
            dir.join(format!("{folder_name}.ecproj")),
            dir.join(format!("{id}.ecproj")),
        ]);
    }
    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }
    let mut all: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "ecproj"))
        .collect();
    all.sort();
    all.iter().find(|p| p.file_stem().and_then(|s| s.to_str()).is_none_or(|s| !s.contains("before-"))).cloned().or_else(|| all.into_iter().next())
}

pub(crate) fn ensure_ecproj_footage_paths(ecproj_path: &Path) {
    let Ok(text) = std::fs::read_to_string(ecproj_path) else { return };
    let Ok(mut doc) = serde_json::from_str::<Value>(&text) else { return };
    let Some(ecproj_dir) = ecproj_path.parent() else { return };
    let mut changed = false;
    if let Some(items) = doc.get_mut("items").and_then(Value::as_object_mut) {
        for item in items.values_mut() {
            let Some(kind) = item.get_mut("kind").and_then(Value::as_object_mut) else { continue };
            if kind.get("type").and_then(Value::as_str) != Some("Footage") {
                continue;
            }
            let Some(raw_path) = kind.get("path").and_then(Value::as_str).map(str::to_owned) else { continue };
            if raw_path.is_empty() || Path::new(&raw_path).is_file() {
                continue;
            }
            let file_name = Path::new(&raw_path).file_name();
            let mut candidates = Vec::new();
            if let Some(fname) = file_name {
                candidates.push(ecproj_dir.join(fname));
                candidates.push(ecproj_dir.join("layers").join(fname));
            }
            if raw_path.contains("/magic/") {
                candidates.push(PathBuf::from(raw_path.replace("/magic/", "/magic-studio/")));
            }
            if let Some(found) = candidates.into_iter().find(|p| p.is_file()) {
                kind.insert("path".to_string(), json!(found));
                changed = true;
            }
        }
    }
    if changed {
        let _ = crate::doc_state::write_json(ecproj_path, &doc);
    }
}

pub(crate) fn ecproj_active_comp_name(ecproj_path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(ecproj_path).ok()?;
    let doc: Value = serde_json::from_str(&text).ok()?;
    let items = doc.get("items")?.as_object()?;
    if let Some(active_id) = doc.get("activeComp").and_then(Value::as_u64)
        && let Some(name) = items.get(&active_id.to_string()).and_then(|it| it.get("name")).and_then(Value::as_str).filter(|s| !s.is_empty())
    {
        return Some(name.to_owned());
    }
    for item in items.values() {
        if item.get("kind").and_then(|k| k.get("type")).and_then(Value::as_str) == Some("Comp")
            && let Some(name) = item.get("name").and_then(Value::as_str).filter(|s| !s.is_empty())
        {
            return Some(name.to_owned());
        }
    }
    None
}

/// One drawn view of one character.
pub(crate) struct View {
    pub id: String,
    pub folder_name: String,
    pub view: String,
    pub dir: PathBuf,
    pub is_flat: bool,
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
        let folder_name = char_dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| raw_id.clone());
        let id = char_dir.file_name().map(|n| crate::workspace::sanitize_asset_id(&n.to_string_lossy())).filter(|s| !s.is_empty()).unwrap_or(raw_id);
        let sub_dir = char_dir.join(&view);
        let (dir, is_flat) = if sub_dir.is_dir() {
            (sub_dir, false)
        } else if view == "front"
            && char_dir.is_dir()
            && (char_dir.join("parts.json").is_file() || find_dir_ecproj(&char_dir, &id, &folder_name, &view, true).is_some())
        {
            (char_dir, true)
        } else {
            (sub_dir, false)
        };
        Ok(Self { id, folder_name, view, dir, is_flat })
    }

    pub(crate) fn file(&self, ext: &str) -> PathBuf {
        if ext == "ecproj"
            && let Some(found) = find_dir_ecproj(&self.dir, &self.id, &self.folder_name, &self.view, self.is_flat)
        {
            return found;
        }
        if ext == "psd" {
            for candidate in [
                self.dir.join(format!("{}-{}.psd", self.id, self.view)),
                self.dir.join(format!("{}-{}.psd", self.folder_name, self.view)),
                self.dir.join(format!("{}-parts.psd", self.folder_name)),
                self.dir.join(format!("{}.psd", self.folder_name)),
            ] {
                if candidate.is_file() {
                    return candidate;
                }
            }
        }
        self.dir.join(format!("{}-{}.{ext}", self.id, self.view))
    }

    pub(crate) fn comp(&self) -> String {
        let ecproj = self.file("ecproj");
        if ecproj.is_file()
            && let Some(comp_name) = ecproj_active_comp_name(&ecproj)
        {
            return comp_name;
        }
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

    if let Some(p) = project.filter(|p| p.is_file()) {
        ensure_ecproj_footage_paths(p);
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
    ensure_ecproj_footage_paths(project);
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let scale = (max_side as f64 / 1920.0).clamp(0.1, 1.0);
    let steps = [
        json!({"id": "comp.open", "params": {"comp": comp}}),
        json!({"id": "comp.saveFrameAs", "params": {"path": output, "time": seconds.max(0.0), "scale": scale, "transparent": true}}),
    ];
    let dir = project.parent().unwrap_or_else(|| Path::new("."));
    let _ = effectcraft(dir, Some(project), &steps, false)?;
    Ok(())
}

pub(crate) fn effectcraft_render(project: &Path, params: Value) -> Result<Value> {
    ensure_ecproj_footage_paths(project);
    let output_str = params["output"].as_str().ok_or_else(|| bad("output path is required for effectcraft_render"))?;
    let output_path = Path::new(output_str);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut queue_params = params.clone();
    if let Some(obj) = queue_params.as_object_mut() {
        obj.insert("output".into(), json!(output_path));
    }
    let steps = [json!({"id": "render.queue", "params": queue_params}), json!({"id": "render.start", "params": {}})];
    let dir = project.parent().unwrap_or_else(|| Path::new("."));
    let results = effectcraft(dir, Some(project), &steps, false)?;
    Ok(results.into_iter().last().unwrap_or_else(|| json!({"ok": true, "output": output_path})))
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
    let skip_names = ["approved-crops", "lineup", "expressions", "contact-parts-v1", "cast"];
    let mut ids: Vec<String> = std::fs::read_dir(&root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| !skip_names.iter().any(|skip| name.eq_ignore_ascii_case(skip)))
        .collect();
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
                    .and_then(|p| p["image"].as_str().or_else(|| p["source"].as_str()).map(|i| dir.join(i)))
                    .filter(|p| p.is_file())
                    .or_else(|| {
                        let staged = dir.join(format!("{canonical_id}-{v}-drawing.png"));
                        staged.is_file().then_some(staged)
                    })
                    .or_else(|| {
                        let staged = dir.join(format!("{id}-{v}-drawing.png"));
                        staged.is_file().then_some(staged)
                    });
                json!({"view": v, "parts": dir.join("parts.json").is_file(), "drawing": drawing,
                    "psd": has("psd"), "ecproj": find_dir_ecproj(&dir, &canonical_id, &id, v, false).is_some()})
            })
            .collect();
        if views.is_empty() {
            let flat_ecproj = find_dir_ecproj(&folder, &canonical_id, &id, "front", true);
            let flat_parts = folder.join("parts.json").is_file();
            let flat_psd = folder.join(format!("{id}-parts.psd")).is_file() || folder.join(format!("{id}.psd")).is_file();
            let cutout = std::fs::read_to_string(folder.join("parts.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .and_then(|p| p["image"].as_str().or_else(|| p["source"].as_str()).map(|i| folder.join(i)))
                .filter(|p| p.is_file())
                .or_else(|| find_initial_cutout(&folder));
            if flat_ecproj.is_some() || flat_parts || cutout.is_some() {
                views.push(json!({"view": "front", "parts": flat_parts, "drawing": cutout, "psd": flat_psd, "ecproj": flat_ecproj.is_some()}));
            }
        }
        let tripo_3d = tripo::inspect_character_3d(s, &project_dir, &canonical_id);
        let sprites = sprite_summaries(&folder);
        if !views.is_empty() || !sprites.is_empty() || tripo_3d["available"].as_bool() == Some(true) {
            let (animation_style, animation_styles) = classify_animation_style(&views, &sprites, &tripo_3d);
            out.push(json!({"id": canonical_id, "name": id, "animation_style": animation_style, "animation_styles": animation_styles,
                "views": views, "sprites": sprites, "tripo_3d": tripo_3d}));
        }
    }
    Ok(json!({"ok": true, "characters": out}))
}

/// Determine a character's primary `animation_style` (`"sprite_2d"`, `"cutout_2_5d"`, `"rig_3d"`, or `"cutout"`)
/// and the list of `animation_styles` present on disk.
pub(crate) fn classify_animation_style(views: &[Value], sprites: &[Value], tripo_3d: &Value) -> (&'static str, Vec<&'static str>) {
    let has_2_5d_rig = views.iter().any(|v| {
        v["rigged"].as_bool() == Some(true)
            || v["ecproj"].as_bool() == Some(true)
            || v["ecproj"].is_string()
            || v["cut"].as_bool() == Some(true)
            || v["psd"].as_bool() == Some(true)
            || v["marked"].as_bool() == Some(true)
            || v["parts"].as_bool() == Some(true)
            || v["parts"].is_array()
    });
    let has_2d_sprites = sprites.iter().any(|s| s["animation_style"].as_str() == Some("sprite_2d"));
    let has_any_sprites = !sprites.is_empty();
    let has_3d = tripo_3d["available"].as_bool() == Some(true);
    let tripo_is_2_5d = tripo_3d["animation_style"].as_str() == Some("cutout_2_5d");

    let mut styles = Vec::new();
    if has_2_5d_rig || (has_3d && tripo_is_2_5d) {
        styles.push("cutout_2_5d");
    }
    if has_2d_sprites || (!has_2_5d_rig && has_any_sprites) {
        styles.push("sprite_2d");
    }
    if has_3d {
        styles.push("rig_3d");
    }
    if styles.is_empty() {
        styles.push("cutout");
    }

    let primary = if has_3d && !tripo_is_2_5d && !has_2_5d_rig {
        "rig_3d"
    } else if has_2_5d_rig {
        "cutout_2_5d"
    } else if has_2d_sprites || has_any_sprites {
        "sprite_2d"
    } else if has_3d {
        "rig_3d"
    } else {
        "cutout"
    };
    (primary, styles)
}

/// Render one frame of the rigged character with EffectCraft.
pub fn cmd_character_frame(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    let time = p["time"].as_f64().unwrap_or(0.0).max(0.0);
    let side = p["max_side"].as_u64().unwrap_or(900).clamp(64, 4096);
    let frames = view.dir.join("frames");
    std::fs::create_dir_all(&frames)?;
    let out = frames.join(format!("{}-{}-{:07.3}.png", view.id, view.view, time));
    effectcraft_frame(&project, &view.comp(), time, side as u32, &out)?;
    Ok(json!({"ok": true, "frame": out, "time": time, "project": project}))
}

fn rendered_pngs(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut frames: Vec<PathBuf> = std::fs::read_dir(dir)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "png")).collect();
    frames.sort();
    Ok(frames)
}

/// The folder, `.ecproj` and comp to render: a sprite (`sprite=<name>`) or a rigged view.
fn render_target(s: &Session, p: &Value) -> Result<(PathBuf, PathBuf, String, String)> {
    if let Some(name) = p["sprite"].as_str() {
        let id = p["character"].as_str().map(crate::workspace::sanitize_asset_id).ok_or_else(|| bad("character=<id> is required"))?;
        let dir = sprite::sprite_dir(s, &id, name)?;
        let comp = format!("{id}-{name}");
        let project = dir.join(format!("{comp}.ecproj"));
        if !project.is_file() {
            return Err(bad(format!("{} is missing; run character.sprite first", project.display())));
        }
        return Ok((dir, project, comp.clone(), comp));
    }
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    let safe_stem = format!("{}-{}", view.id, view.view);
    Ok((view.dir.clone(), project, safe_stem, view.comp()))
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
        let (base, project, safe_stem, comp) = render_target(s, p)?;
        (base, project, safe_stem, json!(comp))
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

/// A character's sprite cycles: name, EffectCraft project and newest animated preview.
pub(crate) fn sprite_summaries(folder: &Path) -> Vec<Value> {
    let newest = |dir: &Path, ext: &str| {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|found| found == ext))
            .max_by_key(|path| std::fs::metadata(path).and_then(|m| m.modified()).ok())
    };
    let mut dirs: Vec<PathBuf> =
        std::fs::read_dir(folder.join("sprites")).into_iter().flatten().flatten().map(|entry| entry.path()).filter(|path| path.is_dir()).collect();
    dirs.sort();
    let mut out: Vec<Value> = dirs
        .iter()
        .filter_map(|dir| {
            let ecproj = newest(dir, "ecproj")?;
            let frames = rendered_pngs(&dir.join("frames")).unwrap_or_default();
            let meta: Value = std::fs::read_to_string(dir.join("sprite.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
            let fps = meta["fps"].as_u64().unwrap_or(24);
            let duration = meta["cycle_seconds"].as_f64().unwrap_or_else(|| frames.len() as f64 / fps.max(1) as f64);
            let style = meta["animation_style"].as_str().unwrap_or("sprite_2d");
            Some(json!({
                "name": dir.file_name().map(|n| n.to_string_lossy().into_owned()),
                "animation_style": style,
                "ecproj": ecproj,
                "preview": newest(&dir.join("clips"), "gif"),
                "frames": frames,
                "fps": fps,
                "duration": duration,
            }))
        })
        .collect();

    // Also include rendered EffectCraft action sequences under `<view>/frames/<range>/` or `<folder>/frames/<range>/`.
    let folder_name = folder.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let id = crate::workspace::sanitize_asset_id(folder_name);
    let mut view_dirs: Vec<(PathBuf, bool, &str)> =
        ["front", "side", "back"].into_iter().filter(|v| folder.join(v).is_dir()).map(|v| (folder.join(v), false, v)).collect();
    if view_dirs.is_empty() && folder.join("frames").is_dir() {
        view_dirs.push((folder.to_path_buf(), true, "front"));
    }
    for (vdir, is_flat, vname) in view_dirs {
        let Some(ecproj) = find_dir_ecproj(&vdir, &id, folder_name, vname, is_flat) else { continue };
        let actions_map = std::fs::read_to_string(vdir.join("parts.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|p| p.get("actions").and_then(Value::as_object).cloned())
            .unwrap_or_default();
        let mut seq_dirs: Vec<PathBuf> =
            std::fs::read_dir(vdir.join("frames")).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        seq_dirs.sort();
        for seq_dir in seq_dirs {
            let frames = rendered_pngs(&seq_dir).unwrap_or_default();
            if frames.is_empty() {
                continue;
            }
            let raw_dir_name = seq_dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let mut cycle_name = raw_dir_name.clone();
            let mut duration = frames.len() as f64 / 24.0;
            for (act_name, spec) in &actions_map {
                if let (Some(st), Some(en)) = (spec.get("start").and_then(Value::as_f64), spec.get("end").and_then(Value::as_f64)) {
                    let suffix = format!("-{st:.2}-{en:.2}");
                    if raw_dir_name.ends_with(&suffix) {
                        cycle_name = act_name.clone();
                        duration = (en - st).max(0.1);
                        break;
                    }
                }
            }
            if out.iter().any(|existing| existing["name"].as_str() == Some(cycle_name.as_str())) {
                continue;
            }
            out.push(json!({
                "name": cycle_name,
                "animation_style": "cutout_2_5d",
                "ecproj": ecproj,
                "preview": newest(&vdir.join("clips"), "gif"),
                "frames": frames,
                "fps": 24,
                "duration": duration,
            }));
        }
    }
    out
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
    let characters: Vec<Value> = all
        .iter()
        .map(|c| json!({"id": c["id"], "animation_style": c["animation_style"], "animation_styles": c["animation_styles"], "views": views_of(c)}))
        .collect();
    let documents = animation_documents(s)?;
    let chosen = p["character"]
        .as_str()
        .and_then(|id| all.iter().find(|c| c["id"] == id))
        .or_else(|| all.iter().find(|c| !views_of(c).is_empty() || c["sprites"].as_array().is_some_and(|s| !s.is_empty())));
    let Some(character) = chosen else {
        return Ok(json!({"ok": true, "characters": characters, "documents": documents, "selected": null}));
    };
    let id = character["id"].as_str().unwrap_or_default();
    let view = p["view"].as_str().map(str::to_owned).or_else(|| views_of(character).first().cloned()).unwrap_or_else(|| "front".into());
    let time = p["time"].as_f64().unwrap_or(0.0).max(0.0);
    let folder = character_folder(&crate::workshop::project_dir(s)?, id);
    let frame = cmd_character_frame(s, &json!({"character": id, "view": view, "time": time}))
        .ok()
        .map(|f| f["frame"].clone())
        .or_else(|| find_initial_cutout(&folder).map(|p| json!(p)));
    let clip = [folder.join(&view).join("clips"), folder.join("clips")]
        .iter()
        .flat_map(|dir| std::fs::read_dir(dir).into_iter().flatten().flatten())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "mov" || ext == "mp4"))
        .max_by_key(|path| std::fs::metadata(path).and_then(|m| m.modified()).ok());
    Ok(
        json!({"ok": true, "characters": characters, "documents": documents, "selected": {"character": id, "view": view, "animation_style": character["animation_style"], "animation_styles": character["animation_styles"], "time": time, "frame": frame, "clip": clip, "sprites": character["sprites"]}}),
    )
}

/// `animation.open {path}`: a project animation document in the EffectCraft window.
pub fn cmd_animation_open(s: &mut Session, p: &Value) -> Result<Value> {
    let path = p["path"]
        .as_str()
        .map(|raw| crate::workspace::resolve_workspace_path(s, raw))
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "ecproj"))
        .ok_or_else(|| bad("path=<existing .ecproj> is required"))?;
    ensure_ecproj_footage_paths(&path);
    crate::CraftHub::open_live(crate::CraftTarget::Effect, &path)?;
    Ok(json!({"ok": true, "app": "effectcraft", "file": path, "document": {"app": "effectcraft", "file": path}}))
}

/// Add an image, EffectCraft `.ecproj`, or model file as a character, named after the file.
pub fn cmd_character_import(s: &mut Session, p: &Value) -> Result<Value> {
    let raw_path = p["path"].as_str().ok_or_else(|| bad("path=<file> is required"))?;
    let source = crate::workspace::resolve_workspace_path(s, raw_path);
    if source.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("ecproj")) {
        if !source.is_file() {
            return Err(bad(format!("{} does not exist", source.display())));
        }
        let stem = source.file_stem().and_then(|n| n.to_str()).unwrap_or("character");
        let clean_stem = stem.trim_end_matches("-rig").trim_end_matches("-front");
        let char_id = p["character"]
            .as_str()
            .map(crate::workspace::sanitize_asset_id)
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| crate::workspace::sanitize_asset_id(clean_stem));
        let project_dir = crate::workshop::project_dir(s)?;
        let char_dir = project_dir.join("assets/characters").join(&char_id);
        std::fs::create_dir_all(&char_dir)?;
        let file_name = source.file_name().ok_or_else(|| bad("invalid .ecproj filename"))?;
        let dest = if source.starts_with(project_dir.join("assets/characters")) {
            source.clone()
        } else {
            let d = char_dir.join(file_name);
            if source != d {
                std::fs::copy(&source, &d)?;
            }
            d
        };
        ensure_ecproj_footage_paths(&dest);
        return Ok(json!({"ok": true, "character": char_id, "ecproj": dest}));
    }
    let name = Path::new(raw_path).file_stem().and_then(|n| n.to_str()).unwrap_or("character");
    crate::project::cmd_project_add_asset(s, &json!({"path": raw_path, "character": crate::workspace::sanitize_asset_id(name), "name": name, "kind": "image"}))
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
        ensure_ecproj_footage_paths(&file);
        crate::CraftHub::open_live(crate::CraftTarget::Effect, &file)?;
    } else {
        crate::workspace::open_editor(app, &file)?;
    }
    Ok(json!({"ok": true, "app": app, "file": file, "document": {"app": app, "file": file}}))
}
