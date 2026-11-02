//! Characters as app documents. Scribbles (Codex) marks the parts, PhotoCraft cuts them,
//! EffectCraft rigs, animates and renders them. These commands only chain the apps' own CLI
//! commands; the `.psd`, `.pcraft` and `.ecproj` files open directly in the apps.
//!
//! Layout: `<project>/assets/characters/<id>/<view>/` holds `parts.json`, `<id>-<view>.psd`,
//! `<id>-<view>.pcraft`, `<id>-<view>.ecproj` and rendered `frames/`.

mod action;
mod cut;
mod mark;
mod meta;
mod rig;
mod sprite;

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

pub use action::{cmd_character_action, cmd_character_actions};
pub use cut::cmd_character_cut;
pub use mark::cmd_character_mark;
pub use meta::{cmd_character_meta, cmd_character_save_marks};
pub use rig::cmd_character_rig;
pub use sprite::cmd_character_sprite;

/// The shared skeleton and its key poses, applied to every character.
pub(crate) const POSES: &str = include_str!("character/poses.json");

fn bad(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

/// One drawn view of one character.
pub(crate) struct View {
    pub id: String,
    pub view: String,
    pub dir: PathBuf,
}

impl View {
    pub(crate) fn from_params(s: &Session, p: &Value) -> Result<Self> {
        let id = p["character"]
            .as_str()
            .map(crate::workspace::sanitize_asset_id)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| bad("character=<id> is required"))?;
        let view = p["view"].as_str().unwrap_or("front").to_string();
        if !matches!(view.as_str(), "front" | "back" | "side") {
            return Err(bad("view must be front, back or side"));
        }
        let dir = crate::workshop::project_dir(s)?
            .join("assets/characters")
            .join(&id)
            .join(&view);
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
        let text = std::fs::read_to_string(&path).map_err(|_| {
            bad(format!(
                "{} has no parts.json; run character.mark first",
                self.dir.display()
            ))
        })?;
        serde_json::from_str(&text).map_err(|e| bad(format!("{}: {e}", path.display())))
    }

    pub(crate) fn write_parts(&self, parts: &Value) -> Result<()> {
        let text = serde_json::to_string_pretty(parts).map_err(|e| bad(e.to_string()))?;
        std::fs::write(self.dir.join("parts.json"), text)?;
        Ok(())
    }

    pub(crate) fn require(&self, ext: &str, step: &str) -> Result<PathBuf> {
        let path = self.file(ext);
        if path.is_file() {
            Ok(path)
        } else {
            Err(bad(format!(
                "{} is missing; run {step} first",
                path.display()
            )))
        }
    }
}

/// Run EffectCraft steps (`{"command", "params"}` objects) in one headless session.
pub(crate) fn effectcraft(dir: &Path, project: Option<&Path>, steps: &[Value]) -> Result<String> {
    let file = dir.join(".effectcraft-steps.jsonl");
    let lines: Vec<String> = steps.iter().map(Value::to_string).collect();
    std::fs::write(&file, lines.join("\n"))?;
    let mut args: Vec<String> = vec!["run".into(), "--commands".into(), path_arg(&file)];
    match project {
        Some(project) => args.extend(["--project".into(), path_arg(project), "--save".into()]),
        None => args.push("--empty".into()),
    }
    let out = crate::workspace::run_tool_in(dir, "effectcraft-cli", &args);
    let _ = std::fs::remove_file(&file);
    out
}

pub(crate) fn path_arg(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub(crate) fn step(command: &str, params: Value) -> Value {
    json!({"command": command, "params": params})
}

/// The parts in back-to-front order, each with `id`, `parent`, `joint` and `outline`.
pub(crate) fn part_list(parts: &Value) -> Result<&Vec<Value>> {
    parts["parts"]
        .as_array()
        .filter(|a| !a.is_empty())
        .ok_or_else(|| bad("parts.json lists no parts"))
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
    let root = crate::workshop::project_dir(s)?.join("assets/characters");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&root).into_iter().flatten().flatten() {
        let id = entry.file_name().to_string_lossy().into_owned();
        let views: Vec<Value> = ["front", "side", "back"]
            .iter()
            .filter(|v| entry.path().join(v).is_dir())
            .map(|v| {
                let dir = entry.path().join(v);
                let has = |ext: &str| dir.join(format!("{id}-{v}.{ext}")).is_file();
                let drawing = std::fs::read_to_string(dir.join("parts.json"))
                    .ok()
                    .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                    .and_then(|p| p["image"].as_str().map(|i| dir.join(i)));
                json!({"view": v, "parts": dir.join("parts.json").is_file(), "drawing": drawing,
                    "psd": has("psd"), "ecproj": has("ecproj")})
            })
            .collect();
        if !views.is_empty() {
            out.push(json!({"id": id, "views": views}));
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
    let args = vec![
        "render-frame".into(),
        path_arg(&project),
        "--comp".into(),
        view.comp(),
        "--time".into(),
        time.to_string(),
        "--max-side".into(),
        side.to_string(),
        "--transparent".into(),
        "--out".into(),
        path_arg(&out),
    ];
    crate::workspace::run_tool_in(&view.dir, "effectcraft-cli", &args)?;
    Ok(json!({"ok": true, "frame": out, "time": time, "project": project}))
}

fn render_args(
    comp: &str,
    project: &Path,
    out: &Path,
    range: (f64, f64, u64),
    clip: bool,
) -> Vec<String> {
    let (start, end, fps) = range;
    let mut args: Vec<String> = vec![
        "render".into(),
        path_arg(project),
        "--comp".into(),
        comp.to_string(),
    ];
    args.extend([
        "--out".into(),
        path_arg(out),
        "--start".into(),
        start.to_string(),
    ]);
    args.extend([
        "--end".into(),
        end.to_string(),
        "--fps".into(),
        fps.to_string(),
    ]);
    args.extend(["--channels".into(), "rgba".into()]);
    if clip {
        args.extend([
            "--format".into(),
            "prores".into(),
            "--prores".into(),
            "4444".into(),
        ]);
    } else {
        args.extend([
            "--format".into(),
            "png".into(),
            "--resolution".into(),
            "half".into(),
        ]);
    }
    args
}

fn rendered_pngs(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut frames: Vec<PathBuf> = std::fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .collect();
    frames.sort();
    Ok(frames)
}

/// The folder, `.ecproj` and comp to render: a sprite (`sprite=<name>`) or a rigged view.
fn render_target(s: &Session, p: &Value) -> Result<(PathBuf, PathBuf, String)> {
    if let Some(name) = p["sprite"].as_str() {
        let id = p["character"]
            .as_str()
            .map(crate::workspace::sanitize_asset_id)
            .ok_or_else(|| bad("character=<id> is required"))?;
        let dir = sprite::sprite_dir(s, &id, name)?;
        let comp = format!("{id}-{name}");
        let project = dir.join(format!("{comp}.ecproj"));
        if !project.is_file() {
            return Err(bad(format!(
                "{} is missing; run character.sprite first",
                project.display()
            )));
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
    let (base, project, comp) = render_target(s, p)?;
    let start = p["start"].as_f64().unwrap_or(0.0).max(0.0);
    let end = p["end"]
        .as_f64()
        .filter(|e| *e > start)
        .unwrap_or(start + 1.0);
    let fps = p["fps"].as_u64().unwrap_or(24).clamp(1, 60);
    let clip = p["format"].as_str() == Some("clip");
    let name = format!("{comp}-{start:.2}-{end:.2}");
    let (dir, out) = if clip {
        let dir = base.join("clips");
        (dir.clone(), dir.join(format!("{name}.mov")))
    } else {
        let dir = base.join("frames").join(&name);
        (dir.clone(), dir.join("frame.png"))
    };
    std::fs::create_dir_all(&dir)?;
    let args = render_args(&comp, &project, &out, (start, end, fps), clip);
    crate::workspace::run_tool_in(&base, "effectcraft-cli", &args)?;
    let frames = if clip {
        Vec::new()
    } else {
        rendered_pngs(&dir)?
    };
    Ok(json!({"ok": true, "start": start, "end": end, "fps": fps,
        "clip": clip.then_some(&out), "frames": frames}))
}

/// Key one transform value of one part at one time (a canvas drag).
pub fn cmd_character_set(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    let part = p["part"]
        .as_str()
        .ok_or_else(|| bad("part=<layer> is required"))?;
    let prop = p["prop"].as_str().unwrap_or("rotation");
    if !matches!(
        prop,
        "position" | "rotation" | "scale" | "anchor" | "opacity"
    ) {
        return Err(bad(
            "prop must be position, rotation, scale, anchor or opacity",
        ));
    }
    let value = p
        .get("value")
        .cloned()
        .ok_or_else(|| bad("value is required"))?;
    let time = p["time"].as_f64().unwrap_or(0.0);
    let key = json!({"layer": part, "path": format!("transform/{prop}"), "time": time,
        "timeBase": "comp", "value": value});
    effectcraft(
        &view.dir,
        Some(&project),
        &[
            step("comp.open", json!({"comp": view.comp()})),
            step("prop.addKey", key),
        ],
    )?;
    Ok(json!({"ok": true, "project": project, "part": part, "prop": prop, "time": time}))
}

/// Open the character's document in the app that owns it, for edits Studio does not offer.
pub fn cmd_character_open(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let (app, file) = match p["app"].as_str().unwrap_or("effectcraft") {
        "effectcraft" => ("effectcraft", view.require("ecproj", "character.rig")?),
        "photocraft" => ("photocraft", view.require("psd", "character.cut")?),
        other => {
            return Err(bad(format!(
                "app must be effectcraft or photocraft, not {other}"
            )));
        }
    };
    let exe = crate::workspace::resolve_tool(app);
    std::process::Command::new(&exe)
        .arg(&file)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| EngineError::ExternalTool(app.into(), e.to_string()))?;
    Ok(json!({"ok": true, "app": app, "file": file}))
}
