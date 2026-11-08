//! `character.cut`: PhotoCraft cuts each marked part onto its own layer in place.
//!
//! Instead of punching trenches across neck/shoulder/hip joints with `edit.contentAwareFill` on the
//! root layer, each child part is copied from the source drawing along its outline, parts listed
//! behind the root in `parts.json` (back-to-front order) are arranged behind `torso`, and `torso`
//! is trimmed cleanly to its own outline (or replaced by a Scribbles clean-torso infill plate when
//! `torso_image` is present in `parts.json`).

use serde_json::{Value, json};

use super::{View, bad, part_list, root_part};
use crate::{Result, Session};

fn cmd(args: &mut Vec<(&'static str, Value)>, id: &'static str, params: Option<Value>) {
    args.push((id, params.unwrap_or_else(|| json!({}))));
}

/// Copy one part from the source layer onto its own named layer. If `behind_root` is true (the part
/// appears before `torso` in back-to-front `parts.json`), send it behind `torso` so hip/tail
/// sockets tuck cleanly under the body. Optional `clone_stamp` and `spot_healing` in `parts.json`
/// repair crossing regions directly in PhotoCraft.
fn cut_part(args: &mut Vec<(&'static str, Value)>, part: &Value, root: &str, behind_root: bool) -> Result<()> {
    let id = part["id"].as_str().ok_or_else(|| bad("a part has no id"))?;
    let outline = part["outline"].as_array().filter(|o| o.len() >= 3).ok_or_else(|| bad(format!("part {id} needs an outline of 3 or more points")))?;
    let lasso = json!({"points": outline, "antiAlias": true});
    cmd(args, "layer.select", Some(json!({"name": root})));
    cmd(args, "select.lasso", Some(lasso));
    cmd(args, "layer.new.layerViaCopy", None);
    cmd(args, "layer.renameLayer", Some(json!({"name": id})));
    if behind_root {
        cmd(args, "layer.arrange.sendToBack", None);
    }
    cmd(args, "select.deselect", None);
    if let Some(clone_params) = part.get("clone_stamp").filter(|v| v.is_object()) {
        cmd(args, "layer.select", Some(json!({"name": id})));
        cmd(args, "paint.cloneStamp", Some(clone_params.clone()));
    }
    if let Some(heal_params) = part.get("spot_healing").filter(|v| v.is_object()) {
        cmd(args, "layer.select", Some(json!({"name": id})));
        cmd(args, "paint.spotHealing", Some(heal_params.clone()));
    }
    Ok(())
}

/// Trim the root (`torso`) layer to its own polygon outline so no stray limb fragments remain
/// outside the body while keeping the neck/shoulder/hip joint overlap intact.
fn trim_root_part(args: &mut Vec<(&'static str, Value)>, root_part_val: &Value, root: &str) -> Result<()> {
    let outline =
        root_part_val["outline"].as_array().filter(|o| o.len() >= 3).ok_or_else(|| bad(format!("root part {root} needs an outline of 3 or more points")))?;
    cmd(args, "layer.select", Some(json!({"name": root})));
    cmd(args, "select.lasso", Some(json!({"points": outline, "antiAlias": true})));
    cmd(args, "select.inverse", None);
    cmd(args, "edit.clear", None);
    cmd(args, "select.deselect", None);
    Ok(())
}

fn photocraft_cli() -> Result<std::path::PathBuf> {
    crate::workspace::find_executable("photocraft-cli").map_err(|e| crate::EngineError::ExternalTool("photocraft-cli".into(), e.to_string()))
}

pub(crate) fn photocraft_run(input: &std::path::Path, steps: &[(&str, Value)], output: Option<&std::path::Path>) -> Result<()> {
    if let Some(out) = output {
        if out == input {
            return Err(crate::EngineError::InvalidParams(format!("refusing to overwrite input {}", input.display())));
        }
        let mut doc = crate::craft_hub::CraftDoc::from_source(crate::CraftTarget::Photo, input, out)?;
        doc.batch(steps)?;
        doc.save()?;
    } else {
        let mut doc = crate::craft_hub::CraftDoc::open(crate::CraftTarget::Photo, input)?;
        doc.batch(steps)?;
    }
    Ok(())
}

/// Document dimensions `(width, height)` from `photocraft-cli info`.
pub(crate) fn photocraft_dimensions(path: &std::path::Path) -> Result<(u32, u32)> {
    let cli = photocraft_cli()?;
    let out = std::process::Command::new(&cli)
        .arg("info")
        .arg(path)
        .output()
        .map_err(|e| crate::EngineError::ExternalTool("photocraft-cli".into(), e.to_string()))?;
    let doc: Value = serde_json::from_slice(&out.stdout).map_err(|e| crate::EngineError::ExternalTool("photocraft-cli".into(), e.to_string()))?;
    let w = doc["width"].as_u64().unwrap_or(1024) as u32;
    let h = doc["height"].as_u64().unwrap_or(1024) as u32;
    Ok((w, h))
}

/// Trace a raster image in VectorCraft (`imageTrace.makeAndExpand` → `object.pathfinder.unite` →
/// `object.path.offsetPath` → `object.path.simplify {"straightLines": true}` → `document.node`),
/// exporting an `.svg` silhouette and returning simplified polygon coordinates in pixel space.
pub(crate) fn vectorcraft_trace_outline(image: &std::path::Path, svg_out: &std::path::Path, offset_px: f64) -> Result<Vec<Value>> {
    use std::io::Write;
    let (w, h) = photocraft_dimensions(image)?;
    let cli = crate::workspace::find_executable("vectorcraft-cli").map_err(|e| crate::EngineError::ExternalTool("vectorcraft-cli".into(), e.to_string()))?;
    let script = [
        json!({"command": "file.new", "params": {"width": w, "height": h}}),
        json!({"command": "file.place", "params": {"path": image, "x": 0, "y": 0}}),
        json!({"command": "object.setBounds", "params": {"x": 0, "y": 0, "width": w, "height": h}}),
        json!({"command": "imageTrace.makeAndExpand", "params": {"preset": "silhouettes", "params": {"threshold": 245, "ignoreWhite": true}}}),
        json!({"command": "object.ungroup", "params": {}}),
        json!({"command": "select.all", "params": {}}),
        json!({"command": "object.pathfinder.unite", "params": {}}),
        json!({"command": "object.path.offsetPath", "params": {"offset": offset_px}}),
        json!({"command": "object.path.simplify", "params": {"tolerance": 4.0, "straightLines": true}}),
        json!({"command": "document.node", "params": {"id": 14}}),
    ];
    let mut child = std::process::Command::new(&cli)
        .arg("run")
        .arg("--commands")
        .arg("-")
        .arg("--export")
        .arg(svg_out)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| crate::EngineError::ExternalTool("vectorcraft-cli".into(), e.to_string()))?;
    if let Some(mut stdin) = child.stdin.take() {
        for line in &script {
            let _ = writeln!(stdin, "{line}");
        }
    }
    let out = child.wait_with_output().map_err(|e| crate::EngineError::ExternalTool("vectorcraft-cli".into(), e.to_string()))?;
    if !out.status.success() {
        return Err(crate::EngineError::ExternalTool("vectorcraft-cli".into(), String::from_utf8_lossy(&out.stderr).into_owned()));
    }
    let cx = f64::from(w) / 2.0;
    let cy = f64::from(h) / 2.0;
    let mut points = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Ok(v) = serde_json::from_str::<Value>(line)
            && v["command"].as_str() == Some("document.node")
            && let Some(anchors) = v.pointer("/result/kind/children/0/kind/path/subpaths/0/anchors").and_then(Value::as_array)
        {
            for a in anchors {
                if let Some(p) = a["p"].as_array()
                    && p.len() == 2
                {
                    let x = (p[0].as_f64().unwrap_or(0.0) + cx).round() as i64;
                    let y = (p[1].as_f64().unwrap_or(0.0) + cy).round() as i64;
                    points.push(json!([x, y]));
                }
            }
        }
    }
    Ok(points)
}

/// Each layer's bounds `[x, y, w, h]` from the saved PSD, by layer name.
fn layer_bounds(psd: &std::path::Path) -> Result<serde_json::Map<String, Value>> {
    let cli = photocraft_cli()?;
    let out =
        std::process::Command::new(&cli).arg("info").arg(psd).output().map_err(|e| crate::EngineError::ExternalTool("photocraft-cli".into(), e.to_string()))?;
    let doc: Value = serde_json::from_slice(&out.stdout).map_err(|e| crate::EngineError::ExternalTool("photocraft-cli".into(), e.to_string()))?;
    let mut out = serde_json::Map::new();
    for layer in doc["layers"].as_array().into_iter().flatten() {
        if let (Some(name), Some(bounds)) = (layer["name"].as_str(), layer.get("bounds")) {
            out.insert(name.into(), bounds.clone());
        }
    }
    Ok(out)
}

/// Each layer as a trimmed PNG (PhotoCraft Layers to Files), for Studio's layer list.
fn export_layers(view: &View, psd: &std::path::Path) -> Result<serde_json::Map<String, Value>> {
    let dir = view.dir.join("layers");
    if dir.is_dir() {
        std::fs::remove_dir_all(&dir)?;
    }
    std::fs::create_dir_all(&dir)?;
    let mut args = Vec::new();
    let params = json!({"dir": dir, "format": "png", "prefix": "", "trim": true});
    cmd(&mut args, "file.export.layersToFiles", Some(params));
    photocraft_run(psd, &args, None)?;
    let mut out = serde_json::Map::new();
    for entry in std::fs::read_dir(&dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // PhotoCraft names each file layer_<index>_<layer name>.png.
        if let Some(id) = name.strip_suffix(".png").and_then(|n| n.splitn(3, '_').nth(2)) {
            out.insert(id.to_string(), json!(entry.path()));
        }
    }
    Ok(out)
}

/// Resolve an optional clean body-plate image (without overlapping front fins/limbs) for the root layer.
fn resolve_body_plate(view: &View, parts: &Value) -> Option<std::path::PathBuf> {
    parts["body_plate"]
        .as_str()
        .or_else(|| parts["torso_image"].as_str())
        .map(|p| {
            let path = std::path::Path::new(p);
            if path.is_absolute() { path.to_path_buf() } else { view.dir.join(path) }
        })
        .filter(|p| p.is_file())
        .or_else(|| view.dir.parent().map(|c| c.join("parts/source/body-plate_v1.png")).filter(|p| p.is_file()))
}

pub fn cmd_character_cut(s: &mut Session, p: &Value) -> Result<Value> {
    if p["parts"].is_array() {
        super::cmd_character_save_marks(s, p)?;
    }
    let view = View::from_params(s, p)?;
    let mut parts = view.parts()?;
    let list = part_list(&parts)?.clone();
    let root = root_part(&list)?.to_string();
    let psd = view.file("psd");
    let pcraft = view.file("pcraft");
    let pre_cut: Vec<(String, std::path::PathBuf)> = list
        .iter()
        .filter_map(|part| {
            let id = part["id"].as_str()?.to_string();
            let raw = part["source"].as_str()?;
            let path = std::path::Path::new(raw);
            let full = if path.is_absolute() { path.to_path_buf() } else { view.dir.join(path) };
            full.is_file().then_some((id, full))
        })
        .collect();
    let image = if pre_cut.len() == list.len() && !pre_cut.is_empty() {
        let drawing = view.dir.join(format!("{}-{}-drawing.png", view.id, view.view));
        let mut args = vec![("layer.renameLayer", json!({"name": &pre_cut[0].0}))];
        for (id, file) in &pre_cut[1..] {
            cmd(&mut args, "file.placeEmbedded", Some(json!({"path": file})));
            cmd(&mut args, "layer.renameLayer", Some(json!({"name": id})));
        }
        cmd(&mut args, "layer.rasterize.allLayers", None);
        cmd(&mut args, "file.saveACopy", Some(json!({"path": &psd, "overwrite": true})));
        cmd(&mut args, "file.saveACopy", Some(json!({"path": &drawing, "overwrite": true})));
        photocraft_run(&pre_cut[0].1, &args, Some(&pcraft))?;
        parts["image"] = json!(drawing.file_name().map(|n| n.to_string_lossy().into_owned()));
        drawing
    } else {
        let image = parts["image"].as_str().map(|i| view.dir.join(i)).filter(|i| i.is_file()).ok_or_else(|| bad("parts.json image is missing"))?;
        let root_idx = list.iter().position(|p| p["id"].as_str() == Some(&root)).unwrap_or(0);
        let mut args = Vec::new();
        cmd(&mut args, "layer.renameLayer", Some(json!({"name": root})));
        for part in list[root_idx + 1..].iter().rev() {
            cut_part(&mut args, part, &root, false)?;
        }
        for part in list[..root_idx].iter().rev() {
            cut_part(&mut args, part, &root, true)?;
        }
        if let Some(plate) = resolve_body_plate(&view, &parts) {
            cmd(&mut args, "layer.select", Some(json!({"name": root})));
            cmd(&mut args, "file.placeEmbedded", Some(json!({"path": plate})));
            cmd(&mut args, "layer.rasterize.layer", None);
            cmd(&mut args, "layer.mergeDown", None);
            cmd(&mut args, "layer.renameLayer", Some(json!({"name": root})));
        }
        trim_root_part(&mut args, &list[root_idx], &root)?;
        cmd(&mut args, "file.saveACopy", Some(json!({"path": &psd, "overwrite": true})));
        photocraft_run(&image, &args, Some(&pcraft))?;
        image
    };
    let bounds = layer_bounds(&psd)?;
    let files = export_layers(&view, &psd)?;
    for part in parts["parts"].as_array_mut().into_iter().flatten() {
        let id = part["id"].as_str().unwrap_or_default().to_string();
        part["bounds"] = bounds.get(&id).cloned().unwrap_or(Value::Null);
        part["file"] = files.get(&id).cloned().unwrap_or(Value::Null);
    }
    let svg = view.file("svg");
    if let Ok(silhouette_pts) = vectorcraft_trace_outline(&image, &svg, 2.0)
        && !silhouette_pts.is_empty()
    {
        parts["silhouette_svg"] = json!(svg);
        parts["silhouette_points"] = json!(silhouette_pts.len());
    }
    view.write_parts(&parts)?;
    let mut reply = json!({"ok": true, "psd": psd, "pcraft": pcraft, "layers": bounds,
        "next": format!("character.rig character={} view={}", view.id, view.view)});
    if p["rig"] == true {
        let rigged = super::cmd_character_rig(s, p)?;
        reply["ecproj"] = rigged["ecproj"].clone();
        reply["comp"] = rigged["comp"].clone();
        reply["next"] = rigged["next"].clone();
    }
    Ok(reply)
}
