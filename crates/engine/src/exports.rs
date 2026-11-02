//! Blender scene rendering and EffectCraft 2.5D Puppet & Deform Rig layered motion export (Issue #55).
//!
//! Uses native `effectcraft-cli run --commands` batch execution with Controller Nulls
//! (`layer.newNull`), anatomical FK parent chains (`layer.setParent`), Puppet Pins
//! (`puppet.addPin`: `position`, `advanced`, `bend`, `starch`), ground-locked stance
//! keyframes (`prop.addKey` + `keys.interpolation`), and secondary overlap (`puppet.follow`).

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::workspace::{resolve_tool, resolve_workspace_path, run_tool_in, workspace_root};
use crate::{EngineError, Result, Session};

/// Render an existing registered Blender source through the native animation job system.
pub fn cmd_render_scene_source(s: &mut Session, params: &Value) -> Result<Value> {
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
    result["command"] = json!("export.scene_render");
    Ok(result)
}

fn layered_spec(s: &Session, params: &Value) -> Result<PathBuf> {
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
    Ok(spec_path)
}

fn layered_out_dir(s: &Session, params: &Value, stem: &str) -> PathBuf {
    match params.get("out_dir").and_then(Value::as_str) {
        Some(dir) if !dir.trim().is_empty() => resolve_workspace_path(s, dir),
        _ => workspace_root(s).join("exports/layered_motion").join(stem),
    }
}

fn parse_comp_geometry(spec: &Value) -> Result<([f64; 2], [u32; 2], [f64; 2], f64)> {
    let canvas = spec["canvas"]
        .as_array()
        .filter(|a| a.len() == 2)
        .ok_or_else(|| EngineError::InvalidParams("spec needs canvas: [w, h]".into()))?;
    let cw = canvas[0].as_f64().unwrap_or(1080.0);
    let ch = canvas[1].as_f64().unwrap_or(1080.0);
    let fps = spec["fps"].as_f64().unwrap_or(24.0).max(1.0);
    let m: [f64; 4] = spec["margin"]
        .as_array()
        .filter(|a| a.len() == 4)
        .map(|a| {
            [
                a[0].as_f64().unwrap_or(0.0),
                a[1].as_f64().unwrap_or(0.0),
                a[2].as_f64().unwrap_or(0.0),
                a[3].as_f64().unwrap_or(0.0),
            ]
        })
        .unwrap_or([0.0, 0.0, 0.0, 0.0]);
    let comp_size = [
        (cw + m[0] + m[2]).round() as u32,
        (ch + m[1] + m[3]).round() as u32,
    ];
    Ok(([cw, ch], comp_size, [m[0], m[1]], fps))
}

fn is_secondary_overlap(lid: &str, layer: &Value) -> bool {
    let low = lid.to_ascii_lowercase();
    layer.get("follow_through").is_some()
        || low.contains("tail")
        || low.contains("ear")
        || low.contains("hair")
        || low.contains("cape")
        || low.contains("coat")
        || low.contains("skirt")
        || low.contains("wing")
        || low.contains("prop")
}

fn is_rigid_limb_segment(lid: &str) -> bool {
    let low = lid.to_ascii_lowercase();
    low.contains("shin")
        || low.contains("forearm")
        || low.contains("lower_leg")
        || low.contains("lower_arm")
        || low.contains("calf")
}

fn append_controller_nulls(cmds: &mut Vec<Value>, cw: f64, ch: f64, shift: [f64; 2]) {
    let cx = cw / 2.0 + shift[0];
    let hip_y = ch * 0.55 + shift[1];
    let ground_y = ch * 0.88 + shift[1];
    let nulls = [
        ("CTRL_Root", [cx, ground_y, 0.0], None),
        ("CTRL_Hip", [cx, hip_y, 0.0], Some("CTRL_Root")),
        (
            "CTRL_Hand_L",
            [cx - cw * 0.18, hip_y, 0.0],
            Some("CTRL_Hip"),
        ),
        (
            "CTRL_Hand_R",
            [cx + cw * 0.18, hip_y, 0.0],
            Some("CTRL_Hip"),
        ),
        (
            "CTRL_Foot_L",
            [cx - cw * 0.08, ground_y, 0.0],
            Some("CTRL_Root"),
        ),
        (
            "CTRL_Foot_R",
            [cx + cw * 0.08, ground_y, 0.0],
            Some("CTRL_Root"),
        ),
    ];
    for (name, pos, parent) in nulls {
        cmds.push(json!({"command": "layer.newNull", "params": {"name": name}}));
        cmds.push(json!({
            "command": "prop.set",
            "params": {"layer": name, "path": "transform/anchor", "value": [0.0, 0.0, 0.0]}
        }));
        cmds.push(json!({
            "command": "prop.set",
            "params": {"layer": name, "path": "transform/position", "value": pos}
        }));
        if let Some(p) = parent {
            cmds.push(json!({
                "command": "layer.setParent",
                "params": {"layers": [name], "parent": p, "compensate": true}
            }));
        }
    }
}

fn append_layer_items_and_hierarchy(
    cmds: &mut Vec<Value>,
    layers: &[Value],
    base_dir: &Path,
    canvas: [f64; 2],
    shift: [f64; 2],
    duration: f64,
) {
    let mut paths = Vec::new();
    for layer in layers {
        let file_raw = layer["file"].as_str().unwrap_or_default();
        let file_path = if Path::new(file_raw).is_absolute() {
            PathBuf::from(file_raw)
        } else {
            base_dir.join(file_raw)
        };
        paths.push(file_path.to_string_lossy().into_owned());
    }
    cmds.push(json!({
        "command": "file.import",
        "params": {"paths": paths, "addToComp": false}
    }));
    for layer in layers {
        let lid = layer["id"].as_str().unwrap_or("part");
        let file_raw = layer["file"].as_str().unwrap_or_default();
        let item_name = Path::new(file_raw)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file_raw);
        let px = layer["pivot"][0].as_f64().unwrap_or(canvas[0] / 2.0);
        let py = layer["pivot"][1].as_f64().unwrap_or(canvas[1] / 2.0);
        cmds.push(json!({
            "command": "layer.addItem",
            "params": {
                "item": item_name,
                "position": [px + shift[0], py + shift[1]],
                "anchor": [px, py],
                "duration": duration
            }
        }));
        cmds.push(json!({"command": "layer.rename", "params": {"name": lid}}));
    }
    for layer in layers {
        let lid = layer["id"].as_str().unwrap_or("part");
        let parent = layer["parent"]
            .as_str()
            .filter(|p| !p.is_empty())
            .unwrap_or("CTRL_Hip");
        cmds.push(json!({
            "command": "layer.setParent",
            "params": {"layers": [lid], "parent": parent, "compensate": true}
        }));
    }
}

fn append_layer_puppet_pins(cmds: &mut Vec<Value>, layers: &[Value], canvas: [f64; 2]) {
    for layer in layers {
        let lid = layer["id"].as_str().unwrap_or("part");
        let px = layer["pivot"][0].as_f64().unwrap_or(canvas[0] / 2.0);
        let py = layer["pivot"][1].as_f64().unwrap_or(canvas[1] / 2.0);
        let tail = layer["rest_tail"]
            .as_array()
            .filter(|a| a.len() == 2)
            .and_then(|a| Some([a[0].as_f64()?, a[1].as_f64()?]))
            .unwrap_or([px, (py + 48.0).min(canvas[1] - 4.0)]);
        cmds.push(json!({
            "command": "puppet.addPin",
            "params": {"layer": lid, "kind": "position", "position": [px, py]}
        }));
        if is_rigid_limb_segment(lid) {
            let mid = [(px + tail[0]) / 2.0, (py + tail[1]) / 2.0];
            cmds.push(json!({
                "command": "puppet.addPin",
                "params": {"layer": lid, "kind": "starch", "position": mid}
            }));
            cmds.push(json!({
                "command": "puppet.addPin",
                "params": {"layer": lid, "kind": "bend", "position": tail}
            }));
        } else if is_secondary_overlap(lid, layer) {
            cmds.push(json!({
                "command": "puppet.addPin",
                "params": {"layer": lid, "kind": "advanced", "position": tail}
            }));
            let delay = layer
                .pointer("/follow_through/delay")
                .and_then(Value::as_f64)
                .unwrap_or(0.10);
            cmds.push(json!({
                "command": "puppet.follow",
                "params": {
                    "layer": lid,
                    "leader": "Puppet Pin 1",
                    "pins": ["Puppet Pin 2"],
                    "delay": delay,
                    "amount": 100,
                    "cascade": true
                }
            }));
        }
    }
}

fn map_ease_to_interpolation(ease: &str) -> &'static str {
    match ease {
        "hold" => "hold",
        "linear" => "linear",
        _ => "bezier",
    }
}

fn append_property_keys(
    cmds: &mut Vec<Value>,
    lid: &str,
    prop_name: &str,
    base_pos: [f64; 2],
    terms: &[Value],
) {
    let path = match prop_name {
        "position" => "transform/position",
        "rotation" => "transform/rotation",
        "scale" => "transform/scale",
        "opacity" => "transform/opacity",
        _ => return,
    };
    for term in terms {
        let Some(keys) = term["keys"].as_array().filter(|k| !k.is_empty()) else {
            continue;
        };
        let interp = map_ease_to_interpolation(term["ease"].as_str().unwrap_or("bezier"));
        let mut key_selectors = Vec::new();
        for k in keys {
            let Some(pair) = k.as_array().filter(|a| a.len() >= 2) else {
                continue;
            };
            let t = pair[0].as_f64().unwrap_or(0.0);
            let val = match prop_name {
                "position" => {
                    let dx = pair[1].get(0).and_then(Value::as_f64).unwrap_or(0.0);
                    let dy = pair[1].get(1).and_then(Value::as_f64).unwrap_or(0.0);
                    json!([base_pos[0] + dx, base_pos[1] + dy, 0.0])
                }
                "scale" => {
                    let sx = pair[1].get(0).and_then(Value::as_f64).unwrap_or(0.0);
                    let sy = pair[1].get(1).and_then(Value::as_f64).unwrap_or(0.0);
                    json!([100.0 + sx, 100.0 + sy, 100.0])
                }
                "opacity" => {
                    let d = pair[1].as_f64().unwrap_or(0.0);
                    json!((100.0 + d).clamp(0.0, 100.0))
                }
                _ => json!(pair[1].as_f64().unwrap_or(0.0)),
            };
            cmds.push(json!({
                "command": "prop.addKey",
                "params": {"layer": lid, "path": path, "time": t, "value": val}
            }));
            key_selectors.push(json!({"layer": lid, "prop": path, "time": t}));
        }
        if !key_selectors.is_empty() {
            cmds.push(json!({"command": "keys.select", "params": {"keys": key_selectors}}));
            cmds.push(json!({
                "command": "keys.interpolation",
                "params": {"interpolation": interp}
            }));
        }
    }
}

fn build_compositor_commands(
    spec: &Value,
    base_dir: &Path,
    selected_tracks: Option<&[String]>,
) -> Result<(Vec<Value>, Vec<(String, f64, usize)>, [u32; 2])> {
    let (canvas, comp_size, shift, fps) = parse_comp_geometry(spec)?;
    let layers_arr = spec["layers"]
        .as_array()
        .filter(|a| !a.is_empty())
        .ok_or_else(|| EngineError::InvalidParams("spec needs non-empty layers array".into()))?;
    let tracks_arr = spec["tracks"]
        .as_array()
        .filter(|a| !a.is_empty())
        .ok_or_else(|| EngineError::InvalidParams("spec needs non-empty tracks array".into()))?;

    let mut cmds = Vec::new();
    let mut track_meta = Vec::new();
    for track in tracks_arr {
        let tid = track["id"].as_str().unwrap_or("track").to_string();
        if let Some(only) = selected_tracks
            && !only.iter().any(|x| x == &tid)
        {
            continue;
        }
        let duration = track["duration"].as_f64().unwrap_or(2.0).max(0.1);
        let frames = (duration * fps).round().max(1.0) as usize;
        cmds.push(json!({
            "command": "comp.new",
            "params": {
                "name": tid,
                "width": comp_size[0],
                "height": comp_size[1],
                "frameRate": fps,
                "duration": duration,
                "open": true
            }
        }));
        append_controller_nulls(&mut cmds, canvas[0], canvas[1], shift);
        append_layer_items_and_hierarchy(&mut cmds, layers_arr, base_dir, canvas, shift, duration);
        append_layer_puppet_pins(&mut cmds, layers_arr, canvas);

        for layer in layers_arr {
            let lid = layer["id"].as_str().unwrap_or("part");
            let px = layer["pivot"][0].as_f64().unwrap_or(canvas[0] / 2.0);
            let py = layer["pivot"][1].as_f64().unwrap_or(canvas[1] / 2.0);
            let layer_motion = &track["motion"][lid];
            for prop in ["position", "rotation", "scale", "opacity"] {
                if let Some(terms) = layer_motion[prop].as_array() {
                    append_property_keys(&mut cmds, lid, prop, [px, py], terms);
                }
            }
        }
        track_meta.push((tid, duration, frames));
    }
    Ok((cmds, track_meta, comp_size))
}

fn parse_track_filter(params: &Value) -> Option<Vec<String>> {
    params.get("tracks").and_then(|tr| match tr {
        Value::Array(arr) => Some(
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
        ),
        Value::String(s_tr) if !s_tr.trim().is_empty() => Some(
            s_tr.split(',')
                .map(str::trim)
                .filter(|x| !x.is_empty())
                .map(str::to_owned)
                .collect(),
        ),
        _ => None,
    })
}

#[allow(clippy::too_many_arguments)]
fn render_track_outputs(
    root: &Path,
    ec_bin: &str,
    project_path: &Path,
    out_dir: &Path,
    tid: &str,
    duration: f64,
    frames: usize,
    fps: f64,
    comp_size: [u32; 2],
    params: &Value,
) -> Result<Value> {
    let render_video = params.get("video").and_then(Value::as_bool).unwrap_or(true);
    let stills_count = params.get("stills").and_then(Value::as_u64).unwrap_or(5) as usize;
    let use_gpu = params.get("gpu").and_then(Value::as_bool).unwrap_or(true);
    let resolution = params
        .get("resolution")
        .and_then(Value::as_str)
        .unwrap_or("full");
    let quality = params
        .get("quality")
        .and_then(Value::as_str)
        .unwrap_or("best");

    let mut stills = Vec::new();
    if stills_count > 0 {
        let stills_dir = out_dir.join("stills").join(tid);
        std::fs::create_dir_all(&stills_dir)?;
        for i in 0..stills_count {
            let at = duration * (i as f64) / (stills_count.max(1) as f64);
            let still_path = stills_dir.join(format!("{i:02}.png"));
            let mut args = vec![
                "--project".into(),
                project_path.to_string_lossy().into_owned(),
            ];
            if use_gpu {
                args.push("--gpu".into());
            }
            args.extend([
                "render-frame".into(),
                "--comp".into(),
                tid.to_string(),
                "--time".into(),
                format!("{at:.4}"),
                "--transparent".into(),
                "--out".into(),
                still_path.to_string_lossy().into_owned(),
            ]);
            run_tool_in(root, ec_bin, &args)?;
            stills.push(still_path);
        }
    }
    let mut entry = json!({
        "track": tid,
        "fps": fps,
        "frames": frames,
        "size": comp_size,
        "stills": stills,
    });
    if render_video {
        let webm_path = out_dir.join(format!("{tid}.webm"));
        let mut args = vec![
            "--project".into(),
            project_path.to_string_lossy().into_owned(),
        ];
        if use_gpu {
            args.push("--gpu".into());
        }
        args.extend([
            "render".into(),
            "--comp".into(),
            tid.to_string(),
            "--out".into(),
            webm_path.to_string_lossy().into_owned(),
            "--format".into(),
            "webm".into(),
            "--channels".into(),
            "rgba".into(),
            "--resolution".into(),
            resolution.into(),
            "--quality".into(),
            quality.into(),
        ]);
        run_tool_in(root, ec_bin, &args)?;
        entry["file"] = json!(webm_path);
        entry["validation"] = json!({"ok": webm_path.is_file()});
    }
    Ok(entry)
}

/// Export a 2.5D Puppet & Deform Rig layered motion spec natively via `effectcraft-cli run --commands`.
pub fn cmd_export_layered_motion(s: &mut Session, params: &Value) -> Result<Value> {
    let spec_path = layered_spec(s, params)?;
    let stem = spec_path
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("layered_motion");
    let root = workspace_root(s);
    let out_dir = layered_out_dir(s, params, stem);
    let work_dir = out_dir.join("work");
    std::fs::create_dir_all(&work_dir)?;

    let spec_bytes = std::fs::read(&spec_path)?;
    let spec: Value = serde_json::from_slice(&spec_bytes)
        .map_err(|e| EngineError::InvalidParams(format!("invalid spec JSON: {e}")))?;
    let base_dir = spec_path.parent().unwrap_or(&root);
    let project_path = work_dir.join("layered_motion.ecproj");
    let commands_path = work_dir.join("effectcraft_commands.json");

    let selected = parse_track_filter(params);
    let (commands, tracks, comp_size) =
        build_compositor_commands(&spec, base_dir, selected.as_deref())?;
    std::fs::write(
        &commands_path,
        serde_json::to_vec_pretty(&commands)
            .map_err(|e| EngineError::InvalidParams(e.to_string()))?,
    )?;

    let ec_bin = resolve_tool("effectcraft-cli");
    run_tool_in(
        &root,
        &ec_bin,
        &[
            "--empty".into(),
            "--save-as".into(),
            project_path.to_string_lossy().into_owned(),
            "run".into(),
            "--commands".into(),
            commands_path.to_string_lossy().into_owned(),
        ],
    )?;

    let fps = spec["fps"].as_f64().unwrap_or(24.0);
    let mut outputs = Vec::new();
    for (tid, duration, frames) in tracks {
        outputs.push(render_track_outputs(
            &root,
            &ec_bin,
            &project_path,
            &out_dir,
            &tid,
            duration,
            frames,
            fps,
            comp_size,
            params,
        )?);
    }
    let qa = crate::motion_qa::cmd_validate_motion(s, &json!({"spec": spec_path}))?;

    Ok(json!({
        "ok": true,
        "command": "export.layered_motion",
        "spec": spec_path,
        "project": project_path,
        "commands_file": commands_path,
        "out_dir": out_dir,
        "outputs": outputs,
        "qa": qa,
        "validation_ok": qa["ok"].as_bool().unwrap_or(true),
    }))
}
