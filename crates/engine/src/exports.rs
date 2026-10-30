//! FilmCraft, Blender, cutout video, layered motion, and motion validation exports.

use std::path::PathBuf;

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

use crate::workspace::{resolve_workspace_path, run_python_json, run_tool_in, workspace_root};

/// Export a FilmCraft project: locate the rendered overlay/animation, then run `filmcraft-load`.
pub fn cmd_export_filmcraft(s: &mut Session, params: &Value) -> Result<Value> {
    let active_id = s
        .active_asset
        .clone()
        .unwrap_or_else(|| "track_b".to_string());

    let media_path = if let Some(media_raw) = params
        .get("media")
        .and_then(Value::as_str)
        .filter(|m| !m.trim().is_empty())
    {
        let p = resolve_workspace_path(s, media_raw);
        if !p.is_file() {
            return Err(EngineError::InvalidParams(format!(
                "media file does not exist: {}",
                p.display()
            )));
        }
        p
    } else {
        let asset = s.current_asset().ok_or_else(|| {
            EngineError::InvalidParams(
                "no active asset; provide 'media' or load an asset with rendered animation".into(),
            )
        })?;
        let id = &asset.id;
        let mut candidates: Vec<PathBuf> = Vec::new();
        // Prefer transparent RGBA WebM overlay tracks first so FilmCraft video tracks preserve alpha
        if let Some(rig) = &asset.cutout_rig
            && let Some(webm) = rig
                .effectcraft
                .get("webm_alpha_path")
                .and_then(Value::as_str)
        {
            candidates.push(resolve_workspace_path(s, webm));
        }
        candidates.push(resolve_workspace_path(
            s,
            &format!("exports/pose_and_parts/{id}/{id}_effectcraft_cutout_alpha.webm"),
        ));
        if let Some(rig) = &asset.cutout_rig
            && let Some(mp4) = rig.effectcraft.get("mp4_path").and_then(Value::as_str)
        {
            candidates.push(resolve_workspace_path(s, mp4));
        }
        candidates.push(resolve_workspace_path(
            s,
            &format!("exports/{id}_cutout.mp4"),
        ));
        candidates.push(resolve_workspace_path(
            s,
            &format!("exports/pose_and_parts/{id}/{id}_effectcraft_cutout.mp4"),
        ));
        candidates.push(resolve_workspace_path(
            s,
            &format!("exports/blender_characters/{id}/{id}_blender53.mp4"),
        ));
        if id == "hopper" {
            candidates.push(resolve_workspace_path(
                s,
                "exports/hopper_4dir_blender53.mp4",
            ));
        }
        candidates
            .into_iter()
            .find(|p| p.is_file())
            .ok_or_else(|| {
                EngineError::InvalidParams(format!(
                    "no rendered overlay/animation found for active asset '{id}'; build a cutout rig first or pass 'media'"
                ))
            })?
    };

    let default_proj = format!("exports/{active_id}_filmcraft.fcproj");
    let default_frame = format!("exports/{active_id}_filmcraft_frame.png");
    let project_raw = params
        .get("output")
        .or_else(|| params.get("project"))
        .and_then(Value::as_str)
        .unwrap_or(&default_proj);
    let frame_raw = params
        .get("frame_out")
        .and_then(Value::as_str)
        .unwrap_or(&default_frame);

    let project_path = resolve_workspace_path(s, project_raw);
    let frame_path = resolve_workspace_path(s, frame_raw);

    let args = vec![
        media_path.to_string_lossy().into_owned(),
        "--project".into(),
        project_path.to_string_lossy().into_owned(),
        "--frame-out".into(),
        frame_path.to_string_lossy().into_owned(),
    ];
    let pipeline_out = run_python_json(s, "pipeline.filmcraft_bridge", &args)?;
    let imported_ok = pipeline_out
        .get("importedToFilmCraft")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && pipeline_out
            .get("projectExists")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    if !imported_ok {
        return Err(EngineError::CommandFailed(
            "export.filmcraft".into(),
            format!("FilmCraft import verification failed: {pipeline_out}"),
        ));
    }

    Ok(json!({
        "ok": true,
        "command": "export.filmcraft",
        "asset_id": active_id,
        "media": media_path.to_string_lossy(),
        "output": project_path.to_string_lossy(),
        "frame_out": frame_path.to_string_lossy(),
        "pipeline": pipeline_out,
    }))
}

/// Render an existing registered Blender source through the native animation job system.
pub fn cmd_export_blender(s: &mut Session, params: &Value) -> Result<Value> {
    let source = params["id"]
        .as_str()
        .and_then(|id| s.animation_sources.get(id))
        .ok_or_else(|| {
            EngineError::InvalidParams("Choose a registered Blender source with id".into())
        })?;
    if source["backend"] != "blender" {
        return Err(EngineError::InvalidParams(
            "The selected source is not a Blender source".into(),
        ));
    }
    let mut result = crate::animation_links::render(s, params)?;
    result["command"] = json!("export.blender");
    Ok(result)
}

/// Render the lineup using its owning pipeline module.
pub fn cmd_scene_farm_school_lineup(s: &mut Session, params: &Value) -> Result<Value> {
    let out_dir_raw = params
        .get("out_dir")
        .and_then(Value::as_str)
        .unwrap_or("exports/farm_school_milestone");
    let out_dir = resolve_workspace_path(s, out_dir_raw);
    let mut args = vec![
        "--out-dir".to_string(),
        out_dir.to_string_lossy().into_owned(),
    ];
    if let Some(only_char) = params.get("character").and_then(Value::as_str) {
        args.push("--character".to_string());
        args.push(only_char.to_string());
    }
    let pipeline_out = run_python_json(s, "pipeline.farm_school_scene_builder", &args)?;
    Ok(json!({
        "ok": true,
        "command": "scene.farm_school_lineup",
        "out_dir": out_dir.to_string_lossy(),
        "pipeline": pipeline_out,
    }))
}

/// Export the character package using its owning pipeline module.
pub fn cmd_export_character_iteration(s: &mut Session, params: &Value) -> Result<Value> {
    let character = params
        .get("character")
        .or_else(|| params.get("asset"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| s.active_asset.clone())
        .unwrap_or_else(|| "hopper".to_string());
    let out_root = params
        .get("out_root")
        .and_then(Value::as_str)
        .unwrap_or("exports/design_share");
    let resolved_out = resolve_workspace_path(s, out_root);
    let args = vec![
        "--character".to_string(),
        character.clone(),
        "--out-root".to_string(),
        resolved_out.to_string_lossy().into_owned(),
    ];
    let pipeline_out = run_python_json(s, "pipeline.character_iteration_exporter", &args)?;
    Ok(json!({
        "ok": true,
        "command": "export.character_iteration",
        "character": character,
        "out_root": resolved_out.to_string_lossy(),
        "pipeline": pipeline_out,
    }))
}

/// Export a layered motion spec via `pipeline/layered_motion.py`.
pub fn cmd_export_layered_motion(s: &mut Session, params: &Value) -> Result<Value> {
    let spec_path = if let Some(spec_raw) = params.get("spec").and_then(Value::as_str)
        && !spec_raw.trim().is_empty()
    {
        resolve_workspace_path(s, spec_raw)
    } else if let Some(rig) = s.current_asset().and_then(|a| a.cutout_rig.as_ref())
        && rig.rig_type == "layered_motion"
    {
        rig.plan_path.clone()
    } else {
        return Err(EngineError::InvalidParams(
            "export.layered_motion requires `spec`: path to a layered motion spec (JSON)".into(),
        ));
    };
    if !spec_path.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "layered motion spec not found: {}",
            spec_path.display()
        )));
    }
    let stem = spec_path
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("layered_motion");
    let root = workspace_root(s);
    let out_dir = match params.get("out_dir").and_then(Value::as_str) {
        Some(dir) if !dir.trim().is_empty() => resolve_workspace_path(s, dir),
        _ => root.join("exports/layered_motion").join(stem),
    };
    let script = root.join("pipeline/layered_motion.py");
    let mut args = vec![
        script.to_string_lossy().into_owned(),
        "--spec".into(),
        spec_path.to_string_lossy().into_owned(),
        "--out-dir".into(),
        out_dir.to_string_lossy().into_owned(),
    ];
    if let Some(tracks) = params.get("tracks") {
        let list = match tracks {
            Value::Array(items) => items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(","),
            other => other.as_str().unwrap_or_default().to_string(),
        };
        if !list.is_empty() {
            args.extend(["--tracks".into(), list]);
        }
    }
    if let Some(count) = params.get("stills").and_then(Value::as_u64) {
        args.extend(["--stills".into(), count.to_string()]);
    }
    if params.get("video").and_then(Value::as_bool) == Some(false) {
        args.push("--no-video".into());
    }
    if params.get("gpu").and_then(Value::as_bool) == Some(false) {
        args.push("--cpu".into());
    }
    if let Some(crf) = params.get("crf").and_then(Value::as_u64) {
        args.extend(["--crf".into(), crf.to_string()]);
    }
    if let Some(res) = params.get("resolution").and_then(Value::as_str)
        && !res.trim().is_empty()
    {
        args.extend(["--resolution".into(), res.to_string()]);
    }
    if let Some(qual) = params.get("quality").and_then(Value::as_str)
        && !qual.trim().is_empty()
    {
        args.extend(["--quality".into(), qual.to_string()]);
    }
    let stdout = run_tool_in(&root, "python3", &args)?;
    let mut result: Value = stdout
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str(line).ok())
        .ok_or_else(|| {
            EngineError::CommandFailed(
                "export.layered_motion".into(),
                "layered_motion.py returned no result".into(),
            )
        })?;
    if let Some(obj) = result.as_object_mut() {
        obj.insert("command".into(), json!("export.layered_motion"));
        obj.insert("out_dir".into(), json!(out_dir.to_string_lossy()));
    }
    Ok(result)
}

/// Validate a rendered motion clip via `pipeline/motion_validate.py`.
pub fn cmd_validate_motion(s: &mut Session, params: &Value) -> Result<Value> {
    let video_raw = params.get("video").and_then(Value::as_str).ok_or_else(|| {
        EngineError::InvalidParams(
            "validate.motion requires `video`: a rendered transparent clip".into(),
        )
    })?;
    let video = resolve_workspace_path(s, video_raw);
    if !video.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "clip not found: {}",
            video.display()
        )));
    }
    let root = workspace_root(s);
    let script = root.join("pipeline/motion_validate.py");
    let mut args = vec![
        script.to_string_lossy().into_owned(),
        "--video".into(),
        video.to_string_lossy().into_owned(),
    ];
    if let Some(frames) = params.get("frames").and_then(Value::as_u64) {
        args.extend(["--frames".into(), frames.to_string()]);
    }
    if let Some(fps) = params.get("fps").and_then(Value::as_f64) {
        args.extend(["--fps".into(), fps.to_string()]);
    }
    if params.get("loop").and_then(Value::as_bool) == Some(true) {
        args.push("--loop".into());
    }
    for key in ["reference", "spec"] {
        if let Some(path) = params.get(key).and_then(Value::as_str)
            && !path.trim().is_empty()
        {
            args.extend([
                format!("--{key}"),
                resolve_workspace_path(s, path)
                    .to_string_lossy()
                    .into_owned(),
            ]);
        }
    }
    let stdout = run_tool_in(&root, "python3", &args)?;
    let mut report: Value = stdout
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str(line).ok())
        .ok_or_else(|| {
            EngineError::CommandFailed(
                "validate.motion".into(),
                "motion_validate.py returned no result".into(),
            )
        })?;
    if let Some(obj) = report.as_object_mut() {
        obj.insert("command".into(), json!("validate.motion"));
    }
    Ok(report)
}
