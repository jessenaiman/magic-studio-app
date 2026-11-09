//! Scribbles engine commands — thin dispatch to [`craft_scribbles`] (L4).
//!
//! Launches/focuses Google Flow's **Moppet Maker** in a dedicated Chrome `--app` window (where
//! Google handles all authentication and Ultra subscription credits), ingests downloaded
//! Moppet Maker packages (`handoff.json`, `character.png`, `parts-sheet.png`, `pose-sheet.png`),
//! builds animated looping SVGs + EffectCraft sprite cycles, and generates short AI video clips.

use crate::{EngineError, Result, Session};
use craft_scribbles::{DEFAULT_FLOW_PROJECT_URL, DEFAULT_MOPPET_MAKER_URL, VideoClipRequest};
use serde_json::{Value, json};
use std::path::PathBuf;

/// Launch or focus Google Flow's Moppet Maker in a dedicated Chrome `--app` window, and
/// if a project is currently open, automatically ingest any new Moppet Maker assets in `~/Downloads`.
pub fn cmd_scribbles_open(s: &mut Session, p: &Value) -> Result<Value> {
    let custom_url = p["url"].as_str();
    let mut out = craft_scribbles::open_moppet_maker_window(custom_url).map_err(|e| EngineError::InvalidParams(e.to_string()))?;
    if p["ingest"].as_bool().unwrap_or(true)
        && s.project_path.is_some()
        && let Ok(ingested) = cmd_scribbles_ingest(s, p)
    {
        out["ingested"] = ingested;
    }
    Ok(out)
}

/// Scan `~/Downloads` (or `dir`) for Moppet Maker outputs (`handoff.json`, `character.png`,
/// `parts-sheet.png`, `pose-sheet.png`), copy & validate them into the open Magic Studio
/// project (`assets/characters/<id>/`), build an animated looping SVG if a pose sheet is present,
/// and register the assets in the active [`Session`].
pub fn cmd_scribbles_ingest(s: &mut Session, p: &Value) -> Result<Value> {
    let project = s.project_path.clone().ok_or_else(|| EngineError::InvalidParams("Open a project before ingesting Moppet Maker assets".into()))?;
    let root = project.parent().ok_or_else(|| EngineError::InvalidParams("Project has no parent directory".into()))?;
    let source_dir = p["dir"].as_str().filter(|d| !d.trim().is_empty()).map(PathBuf::from);
    let requested_char = p["character"].as_str().filter(|c| !c.trim().is_empty());

    let bundle =
        craft_scribbles::ingest_moppet_downloads(root, source_dir.as_deref(), requested_char).map_err(|e| EngineError::InvalidParams(e.to_string()))?;

    let mut registered = Vec::new();
    let char_id = &bundle.character_id;

    if let Some(ref char_png) = bundle.character_png
        && let Ok(res) = s.execute(
            "project.add_asset",
            json!({
                "path": char_png,
                "kind": "image",
                "character": char_id,
                "name": format!("{char_id}-character"),
                "load": false
            }),
        )
    {
        registered.push(res);
    }

    if let Some(ref parts_png) = bundle.parts_sheet_png
        && let Ok(res) = s.execute(
            "project.add_asset",
            json!({
                "path": parts_png,
                "kind": "image",
                "character": char_id,
                "name": format!("{char_id}-parts-sheet"),
                "load": false
            }),
        )
    {
        registered.push(res);
    }

    if let Some(ref pose_png) = bundle.pose_sheet_png {
        if let Ok(res) = s.execute(
            "project.add_asset",
            json!({
                "path": pose_png,
                "kind": "image",
                "character": char_id,
                "name": format!("{char_id}-pose-sheet"),
                "load": false
            }),
        ) {
            registered.push(res);
        }
        let _ = s.execute(
            "character.sprite",
            json!({
                "character": char_id,
                "action": &bundle.action,
                "name": &bundle.action,
                "sheet": pose_png,
                "columns": bundle.grid.0,
                "rows": bundle.grid.1,
                "fps": 6
            }),
        );
    }

    if bundle.parts_json.is_some() && bundle.character_png.is_some() {
        let _ = s.execute(
            "character.cut",
            json!({
                "character": char_id,
                "view": &bundle.view,
                "rig": true
            }),
        );
    }

    if let Some(ref svg_path) = bundle.animated_svg
        && let Ok(res) = s.execute(
            "project.add_asset",
            json!({
                "path": svg_path,
                "kind": "image",
                "character": char_id,
                "name": format!("{char_id}-{}-svg", bundle.action),
                "load": false
            }),
        )
    {
        registered.push(res);
    }

    Ok(json!({
        "ok": true,
        "character_id": bundle.character_id,
        "character_name": bundle.character_name,
        "view": bundle.view,
        "action": bundle.action,
        "grid": [bundle.grid.0, bundle.grid.1],
        "source_dir": bundle.source_dir,
        "dest_dir": bundle.dest_dir,
        "handoff_json": bundle.handoff_json,
        "parts_json": bundle.parts_json,
        "character_png": bundle.character_png,
        "parts_sheet_png": bundle.parts_sheet_png,
        "pose_sheet_png": bundle.pose_sheet_png,
        "animated_svg": bundle.animated_svg,
        "registered_count": registered.len(),
        "registered": registered,
    }))
}

/// Generate an image through the configured provider (defaults to `"flow"` via Google Flow)
/// or open Moppet Maker when called without a prompt.
pub fn cmd_scribbles_generate(s: &mut Session, p: &Value) -> Result<Value> {
    if p["prompt"].as_str().is_none_or(|v| v.trim().is_empty()) {
        return cmd_scribbles_open(s, p);
    }
    let mut req = p.clone();
    if req.get("provider").is_none_or(Value::is_null) {
        req["provider"] = json!("flow");
    }
    crate::image_generation::generate(s, &req)
}

/// Inspect Scribbles / Google Flow Moppet Maker status and URLs.
pub fn cmd_scribbles_templates(_: &mut Session, _: &Value) -> Result<Value> {
    Ok(json!({
        "ok": true,
        "provider": "flow",
        "tool": "Moppet Maker",
        "url": DEFAULT_MOPPET_MAKER_URL,
        "project_url": DEFAULT_FLOW_PROJECT_URL,
        "outputs": ["character.png", "parts-sheet.png", "pose-sheet.png", "handoff.json"],
    }))
}

/// Generate a short AI video clip through Google Flow (`flow-clip --mode video`) and register it in the project.
pub fn cmd_scribbles_video(s: &mut Session, p: &Value) -> Result<Value> {
    let prompt = p["prompt"]
        .as_str()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| EngineError::InvalidParams("scribbles.video needs prompt=<motion description>".into()))?;
    let project = s.project_path.clone().ok_or_else(|| EngineError::InvalidParams("Open a project before generating video clips".into()))?;
    let root = project.parent().ok_or_else(|| EngineError::InvalidParams("Project has no parent directory".into()))?;
    let clips_dir = root.join("assets/clips");

    let name =
        p["name"].as_str().map(crate::workspace::sanitize_asset_id).filter(|v| !v.is_empty()).unwrap_or_else(|| format!("flow-clip-{}", std::process::id()));
    let out_mp4 = clips_dir.join(format!("{name}.mp4"));

    let req = VideoClipRequest {
        prompt,
        out_mp4: &out_mp4,
        model: p["model"].as_str(),
        aspect: p["aspect"].as_str(),
        duration: p["duration"].as_str(),
        resolution: p["resolution"].as_str(),
        ingredient: p["ingredient"].as_str().or_else(|| p["reference"].as_str()),
    };
    craft_scribbles::generate_video_clip(&req).map_err(|e| EngineError::InvalidParams(e.to_string()))?;

    let attached = s.execute(
        "project.add_asset",
        json!({
            "path": out_mp4,
            "kind": "video",
            "name": p["name"].as_str().unwrap_or("Flow video clip"),
            "load": false
        }),
    )?;

    Ok(json!({
        "ok": true,
        "format": "magic.flow-video",
        "provider": "flow",
        "prompt": prompt,
        "video": out_mp4,
        "asset_id": attached["asset_id"],
        "status": "ready"
    }))
}

/// Slice a 2×2 chroma-green pose sheet into an animated looping SVG (`flow-clip --mode svg`)
/// and build the EffectCraft `.ecproj` sprite cycle via `character.sprite`.
pub fn cmd_scribbles_svg(s: &mut Session, p: &Value) -> Result<Value> {
    let sheet_raw = p["sheet"]
        .as_str()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| EngineError::InvalidParams("scribbles.svg needs sheet=<2x2 pose sheet PNG>".into()))?;
    let sheet = crate::workspace::resolve_workspace_path(s, sheet_raw);
    let project = s.project_path.clone().ok_or_else(|| EngineError::InvalidParams("Open a project before building animated SVGs".into()))?;
    let root = project.parent().ok_or_else(|| EngineError::InvalidParams("Project has no parent directory".into()))?;
    let char_id = p["character"].as_str().map(crate::workspace::sanitize_asset_id).unwrap_or_else(|| "moppet".into());
    let action = p["action"].as_str().unwrap_or("wave");
    let out_svg = root.join("assets/characters").join(&char_id).join("sprites").join(action).join(format!("{char_id}-{action}.svg"));
    let fps = p["fps"].as_u64().unwrap_or(6);

    craft_scribbles::build_animated_svg(&sheet, &out_svg, fps).map_err(|e| EngineError::InvalidParams(e.to_string()))?;

    let sprite_res = s
        .execute(
            "character.sprite",
            json!({
                "character": char_id,
                "action": action,
                "name": action,
                "sheet": sheet,
                "columns": 2,
                "rows": 2,
                "fps": fps
            }),
        )
        .ok();

    let attached = s.execute(
        "project.add_asset",
        json!({
            "path": out_svg,
            "kind": "image",
            "character": char_id,
            "name": format!("{char_id}-{action}-svg"),
            "load": false
        }),
    )?;

    Ok(json!({
        "ok": true,
        "format": "magic.animated-svg",
        "svg": out_svg,
        "sheet": sheet,
        "character": char_id,
        "action": action,
        "fps": fps,
        "asset_id": attached["asset_id"],
        "sprite_project": sprite_res.and_then(|v| v.get("effect_project").cloned()),
    }))
}

/// Reassemble an exploded `#00FF00` chroma-green `parts-sheet.png` in Rust into separate transparent
/// layers + `parts.json` (`plush-9` skeleton + Puppet Pins), then run `character.cut` + `character.rig`
/// + `character.action` so the character is immediately rigged and animated in Magic Studio.
pub fn cmd_scribbles_reassemble(s: &mut Session, p: &Value) -> Result<Value> {
    let project = s.project_path.clone().ok_or_else(|| EngineError::InvalidParams("Open a project before reassembling a parts sheet".into()))?;
    let root = project.parent().ok_or_else(|| EngineError::InvalidParams("Project has no parent directory".into()))?;
    let char_id = p["character"].as_str().map(crate::workspace::sanitize_asset_id).unwrap_or_else(|| "label-pangolin".into());
    let view = p["view"].as_str().unwrap_or("front");
    let char_dir = root.join("assets/characters").join(&char_id);
    let sheet = p["sheet"]
        .as_str()
        .filter(|v| !v.trim().is_empty())
        .map(|raw| crate::workspace::resolve_workspace_path(s, raw))
        .unwrap_or_else(|| char_dir.join("parts-sheet.png"));
    if !sheet.is_file() {
        return Err(EngineError::InvalidParams(format!("parts sheet {} does not exist", sheet.display())));
    }
    let view_dir = char_dir.join(view);
    let mut out = craft_scribbles::reassemble_parts_sheet(&sheet, &view_dir, &char_id, view).map_err(|e| EngineError::InvalidParams(e.to_string()))?;

    if p["rig"].as_bool().unwrap_or(true) {
        if let Ok(cut_res) = s.execute(
            "character.cut",
            json!({
                "character": char_id,
                "view": view,
                "rig": true
            }),
        ) {
            out["cut_and_rig"] = cut_res;
        }
        let action = p["action"].as_str().unwrap_or("idle");
        if let Ok(act_res) = s.execute(
            "character.action",
            json!({
                "character": char_id,
                "view": view,
                "action": action
            }),
        ) {
            out["animated_action"] = act_res;
        }
    }
    Ok(out)
}

/// Build an articulated Blender `.blend` scene (`MoppetRig` armature + layered cut-out planes +
/// 48-frame animated walk/bounce cycle) from a character's reassembled `parts.json` and optionally
/// launch Blender interactively.
pub fn cmd_scribbles_blender(s: &mut Session, p: &Value) -> Result<Value> {
    let project = s.project_path.clone().ok_or_else(|| EngineError::InvalidParams("Open a project before building a Blender Moppet rig".into()))?;
    let root = project.parent().ok_or_else(|| EngineError::InvalidParams("Project has no parent directory".into()))?;
    let char_id = p["character"].as_str().map(crate::workspace::sanitize_asset_id).unwrap_or_else(|| "label-pangolin".into());
    let view = p["view"].as_str().unwrap_or("front");
    let open_gui = p["open"].as_bool().unwrap_or(false);

    let view_dir = root.join("assets/characters").join(&char_id).join(view);
    if !view_dir.join("parts.json").is_file() {
        let _ = cmd_scribbles_reassemble(s, &json!({ "character": char_id, "view": view, "rig": true }));
    }

    craft_scribbles::build_blender_rig_from_parts(root, &char_id, view, open_gui).map_err(|e| EngineError::InvalidParams(e.to_string()))
}
