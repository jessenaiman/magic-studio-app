use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn read_photocraft_info(root: &Path, image_path: &Path) -> Result<(u32, u32, [u32; 4])> {
    let raw = crate::workspace::run_tool_in(
        root,
        "photocraft-cli",
        &[
            "info".into(),
            "--compact".into(),
            image_path.to_string_lossy().into_owned(),
        ],
    )?;
    let info: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
    let w = info["width"].as_u64().unwrap_or(256) as u32;
    let h = info["height"].as_u64().unwrap_or(256) as u32;
    let b = info["layers"]
        .as_array()
        .and_then(|l| l.first())
        .and_then(|l| l["bounds"].as_array())
        .filter(|a| a.len() == 4)
        .map(|a| {
            [
                a[0].as_u64().unwrap_or(0) as u32,
                a[1].as_u64().unwrap_or(0) as u32,
                a[2].as_u64().unwrap_or(w as u64) as u32,
                a[3].as_u64().unwrap_or(h as u64) as u32,
            ]
        })
        .unwrap_or([0, 0, w, h]);
    Ok((w, h, b))
}

/// Matte one reference through PhotoCraft (`layer.removeBackground` + `layer.matting.colorDecontaminate`).
pub(crate) fn matte_image(s: &Session, source: &Path, p: &Value) -> Result<Value> {
    let root = crate::workspace::workspace_root(s);
    if p["reuse_alpha"].as_bool() == Some(true) {
        let (w, h, _) = read_photocraft_info(&root, source)?;
        return Ok(json!({
            "ok": true,
            "image": source,
            "source_image": source,
            "width": w,
            "height": h,
            "has_alpha": true,
            "reused_alpha": true
        }));
    }
    let proj_dir = crate::workshop::project_dir(s)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| EngineError::InvalidParams(e.to_string()))?
        .as_nanos();
    let directory = proj_dir.join("assets/reference-previews");
    std::fs::create_dir_all(&directory)?;
    let out = directory.join(format!("matte-{stamp}.png"));
    let mut args = vec!["run".into(), source.to_string_lossy().into_owned()];
    if let Some(rect) = p.get("rect") {
        args.extend([
            "--cmd".into(),
            "layer.new.layerFromBackground".into(),
            "--cmd".into(),
            "select.object".into(),
            "--params".into(),
            json!({"rect": rect}).to_string(),
            "--cmd".into(),
            "select.inverse".into(),
            "--cmd".into(),
            "edit.clear".into(),
        ]);
    } else {
        args.extend([
            "--cmd".into(),
            "layer.removeBackground".into(),
            "--params".into(),
            "{\"refine\":true}".into(),
            "--cmd".into(),
            "layer.matting.colorDecontaminate".into(),
            "--params".into(),
            "{\"amount\":100,\"radius\":4}".into(),
        ]);
    }
    args.extend(["--out".into(), out.to_string_lossy().into_owned()]);
    let tool_output = crate::workspace::run_tool_in(&root, "photocraft-cli", &args)?;
    let (width, height, _) = read_photocraft_info(&root, &out)?;
    let result = json!({
        "ok": true,
        "source_image": source,
        "image": out,
        "width": width,
        "height": height,
        "has_alpha": true,
        "tool": "photocraft-cli",
        "tool_output": tool_output,
    });
    std::fs::write(out.with_extension("matte.json"), result.to_string())?;
    Ok(result)
}

/// Remove background from an asset or reference image via `photocraft-cli` and optionally attach it.
pub fn cmd_asset_make_transparent(s: &mut Session, params: &Value) -> Result<Value> {
    activate_requested_asset(s, params)?;
    let source = resolve_source_or_active(s, params, "no active asset or `source` image")?;
    let mut result = matte_image(s, &source, params)?;
    if params
        .get("attach")
        .and_then(Value::as_bool)
        .unwrap_or(true)
        && let Some(out_str) = result["image"].as_str()
    {
        let out_path = PathBuf::from(out_str);
        if let Some(asset) = s.current_asset_mut() {
            let id = asset.id.clone();
            asset.path.clone_from(&out_path);
            if let Some(entry) = s.project_assets.iter_mut().find(|a| a.id == id) {
                entry.path = Some(out_path);
            }
            result["attached"] = json!(true);
        }
    }
    Ok(result)
}

fn resolve_source_or_active(s: &Session, params: &Value, err_msg: &str) -> Result<PathBuf> {
    let source = match params
        .get("source")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        Some(raw) => crate::workspace::resolve_workspace_path(s, raw),
        None => s
            .current_asset()
            .map(|a| a.path.clone())
            .ok_or_else(|| EngineError::InvalidParams(err_msg.into()))?,
    };
    if !source.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "source image not found: {}",
            source.display()
        )));
    }
    Ok(source)
}

fn fallback_sheet_layout(sheet_path: &Path, kind_raw: &str) -> Result<Value> {
    let (w, h) = image::image_dimensions(sheet_path)?;
    let (cols, rows) = (2u32, 2u32);
    let cw = (w / cols).max(16);
    let ch = (h / rows).max(16);
    let mut frames = Vec::new();
    for idx in 0..4u32 {
        let (col, row) = (idx % cols, idx / cols);
        let (l, t) = (col * cw, row * ch);
        frames.push(json!({
            "label": format!("panel_{}", idx + 1),
            "index": idx,
            "box": [l, t, (l + cw).min(w), (t + ch).min(h)],
            "gridRow": row,
            "gridColumn": col,
            "holdFrames": 1
        }));
    }
    let sheet_type = match kind_raw {
        "turnaround" => "turnaround",
        "skeleton" | "exploded" => "exploded",
        _ => "action",
    };
    Ok(json!({
        "sheetType": sheet_type,
        "grid": {"columns": cols, "rows": rows},
        "frames": frames,
        "preserveRegistration": true,
        "animation": if sheet_type == "action" {
            json!({"template": "action", "fps": 12, "loop": true})
        } else {
            Value::Null
        }
    }))
}

fn resolve_cut_layout(s: &Session, sheet_path: &Path, params: &Value) -> Result<Value> {
    if let Some(raw) = params
        .get("layout")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let p = crate::workspace::resolve_workspace_path(s, raw);
        return serde_json::from_slice(&std::fs::read(&p)?)
            .map_err(|e| EngineError::InvalidParams(format!("invalid layout JSON: {e}")));
    }
    if let Some(layout) = crate::sheets::layout::for_sheet(sheet_path)? {
        return Ok(layout);
    }
    let kind_raw = params
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("action");
    fallback_sheet_layout(sheet_path, kind_raw)
}

fn parse_panel_box(panel: &Value) -> Result<[u32; 4]> {
    let arr = panel["box"]
        .as_array()
        .filter(|a| a.len() == 4)
        .ok_or_else(|| EngineError::InvalidParams("sheet panel missing 4-element `box`".into()))?;
    let l = arr[0].as_u64().unwrap_or(0) as u32;
    let t = arr[1].as_u64().unwrap_or(0) as u32;
    let r = arr[2].as_u64().unwrap_or(0) as u32;
    let b = arr[3].as_u64().unwrap_or(0) as u32;
    Ok([
        l,
        t,
        r.saturating_sub(l).max(16),
        b.saturating_sub(t).max(16),
    ])
}

fn cut_single_panel(
    root: &Path,
    sheet_path: &Path,
    out_dir: &Path,
    index: usize,
    rect: [u32; 4],
    canvas: [u32; 2],
) -> Result<String> {
    let [x, y, bw, bh] = rect;
    let [cw, ch] = canvas;
    let file_name = format!("frame_{index:02}.png");
    let final_png = out_dir.join(&file_name);
    let mut args = vec![
        "run".into(),
        sheet_path.to_string_lossy().into_owned(),
        "--cmd".into(),
        "image.crop".into(),
        "--params".into(),
        json!({"x": x, "y": y, "width": bw, "height": bh, "deleteCroppedPixels": true}).to_string(),
        "--cmd".into(),
        "layer.removeBackground".into(),
        "--params".into(),
        "{\"refine\":true}".into(),
        "--cmd".into(),
        "layer.matting.colorDecontaminate".into(),
        "--params".into(),
        "{\"amount\":100,\"radius\":4}".into(),
    ];
    if (bw, bh) != (cw, ch) {
        args.extend([
            "--cmd".into(),
            "image.canvasSize".into(),
            "--params".into(),
            json!({"width": cw, "height": ch, "anchor": "center"}).to_string(),
        ]);
    }
    args.extend(["--out".into(), final_png.to_string_lossy().into_owned()]);
    crate::workspace::run_tool_in(root, "photocraft-cli", &args)?;
    Ok(file_name)
}

fn resolve_cut_asset_id(s: &Session, params: &Value) -> String {
    params
        .get("asset")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .or_else(|| s.current_asset().map(|a| a.id.clone()))
        .unwrap_or_else(|| "sheet".into())
}

/// Extract reviewed sheet panels into uniform transparent PNGs + `manifest.json` via `photocraft-cli`.
pub fn cmd_asset_cut_sheet(s: &mut Session, params: &Value) -> Result<Value> {
    activate_requested_asset(s, params)?;
    let asset_id = resolve_cut_asset_id(s, params);
    let sheet_path = resolve_source_or_active(s, params, "no active asset or `source` sheet")?;
    if s.project_assets.iter().any(|a| a.id == asset_id) {
        crate::sheets::require_sheet_review(s, &asset_id, &sheet_path)?;
    }
    let layout = resolve_cut_layout(s, &sheet_path, params)?;
    let panels = layout["frames"]
        .as_array()
        .filter(|f| !f.is_empty())
        .ok_or_else(|| EngineError::InvalidParams("sheet layout has no frames".into()))?;
    let rects: Vec<[u32; 4]> = panels.iter().map(parse_panel_box).collect::<Result<_>>()?;
    let canvas_w = rects.iter().map(|r| r[2]).max().unwrap_or(256);
    let canvas_h = rects.iter().map(|r| r[3]).max().unwrap_or(256);
    let out_dir = match params
        .get("out_dir")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
    {
        Some(raw) => crate::workspace::resolve_workspace_path(s, raw),
        None => prepare_dir(s, &asset_id).join("pieces"),
    };
    std::fs::create_dir_all(&out_dir)?;
    let root = crate::workspace::workspace_root(s);
    let mut out_frames = Vec::with_capacity(panels.len());
    for (idx, (panel, rect)) in panels.iter().zip(rects.iter()).enumerate() {
        let file_name = cut_single_panel(
            &root,
            &sheet_path,
            &out_dir,
            idx,
            *rect,
            [canvas_w, canvas_h],
        )?;
        let mut entry = panel.clone();
        entry["file"] = json!(file_name);
        entry["canvas"] = json!([canvas_w, canvas_h]);
        out_frames.push(entry);
    }
    let sheet_type = layout["sheetType"].as_str().unwrap_or("action");
    let animation = layout.get("animation").cloned().unwrap_or(Value::Null);
    let manifest_path = out_dir.join("manifest.json");
    let manifest = json!({
        "sheetType": sheet_type,
        "sourceImage": sheet_path,
        "canvas": [canvas_w, canvas_h],
        "grid": layout["grid"],
        "preserveRegistration": true,
        "frames": out_frames,
        "animation": animation,
        "tool": "photocraft-cli"
    });
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest)
            .map_err(|e| EngineError::InvalidParams(e.to_string()))?,
    )?;
    Ok(json!({
        "ok": true,
        "asset_id": asset_id,
        "manifest": manifest_path,
        "out_dir": out_dir,
        "sheetType": sheet_type,
        "animation": animation,
        "frames": out_frames,
        "tool": "photocraft-cli"
    }))
}

fn find_jsonl_cmd_result(raw: &str, cmd_name: &str) -> Option<Value> {
    raw.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|v| v["command"].as_str() == Some(cmd_name))
        .map(|v| v["result"].clone())
}

/// Extend a rotational joint on a segmented part with a circular hinge cap ($R = D/2$) via `photocraft-cli`
/// (`select.rect` ellipse -> `select.modify.expand` -> `select.modify.smooth` -> `select.modify.feather` ->
/// `edit.contentAwareFill`) and save `.pcraft` alongside the PNG (Issue #55 Stage 1).
pub(crate) fn extend_circular_hinge_cap(
    root: &Path,
    part_png: &Path,
    pivot: [f64; 2],
    diameter: f64,
    pcraft_out: &Path,
) -> Result<u32> {
    let radius = (diameter / 2.0).round().clamp(6.0, 160.0) as i32;
    let rx = (pivot[0].round() as i32) - radius;
    let ry = (pivot[1].round() as i32) - radius;
    let span = (radius * 2) as u32;
    let smooth_r = (radius / 4).clamp(2, 32);
    crate::workspace::run_tool_in(
        root,
        "photocraft-cli",
        &[
            "run".into(),
            "--in-place".into(),
            part_png.to_string_lossy().into_owned(),
            "--cmd".into(),
            "select.rect".into(),
            "--params".into(),
            json!({"x": rx, "y": ry, "width": span, "height": span, "ellipse": true}).to_string(),
            "--cmd".into(),
            "select.modify.expand".into(),
            "--params".into(),
            "{\"radius\":2}".into(),
            "--cmd".into(),
            "select.modify.smooth".into(),
            "--params".into(),
            json!({"radius": smooth_r}).to_string(),
            "--cmd".into(),
            "select.modify.feather".into(),
            "--params".into(),
            "{\"radius\":2}".into(),
            "--cmd".into(),
            "edit.contentAwareFill".into(),
            "--params".into(),
            "{\"sampling\":\"auto\",\"output\":\"current\"}".into(),
            "--cmd".into(),
            "file.saveACopy".into(),
            "--params".into(),
            json!({"path": pcraft_out, "overwrite": true}).to_string(),
            "--out".into(),
            part_png.to_string_lossy().into_owned(),
        ],
    )?;
    Ok(radius as u32)
}

fn isolate_and_trace_character(
    root: &Path,
    src_path: &Path,
    matted_path: &Path,
    cropped_path: &Path,
    svg_path: &Path,
) -> Result<(String, String)> {
    let pc_out = crate::workspace::run_tool_in(
        root,
        "photocraft-cli",
        &[
            "run".into(),
            src_path.to_string_lossy().into_owned(),
            "--cmd".into(),
            "layer.removeBackground".into(),
            "--params".into(),
            "{\"refine\":true}".into(),
            "--cmd".into(),
            "layer.matting.colorDecontaminate".into(),
            "--params".into(),
            "{\"amount\":100,\"radius\":4}".into(),
            "--cmd".into(),
            "image.detectFaces".into(),
            "--params".into(),
            "{\"minScore\":0.5,\"sampleAllLayers\":true}".into(),
            "--out".into(),
            matted_path.to_string_lossy().into_owned(),
        ],
    )?;
    let _ = crate::workspace::run_tool_in(
        root,
        "photocraft-cli",
        &[
            "run".into(),
            matted_path.to_string_lossy().into_owned(),
            "--cmd".into(),
            "image.trim".into(),
            "--params".into(),
            "{\"basedOn\":\"transparent\"}".into(),
            "--out".into(),
            cropped_path.to_string_lossy().into_owned(),
        ],
    )?;
    let vc_out = crate::workspace::run_tool_in(
        root,
        "vectorcraft-cli",
        &[
            "run".into(),
            "--in".into(),
            matted_path.to_string_lossy().into_owned(),
            "--cmd".into(),
            "select.all".into(),
            "--cmd".into(),
            "imageTrace.makeAndExpand".into(),
            "--params".into(),
            "{\"preset\":\"16 Colors\",\"enhanced\":true,\"params\":{\"ignoreWhite\":true}}".into(),
            "--cmd".into(),
            "object.puppetWarp.pins".into(),
            "--params".into(),
            "{}".into(),
            "--export".into(),
            svg_path.to_string_lossy().into_owned(),
        ],
    )?;
    Ok((pc_out, vc_out))
}

fn default_rig_tracks() -> Value {
    json!([
        {
            "id": "rest",
            "label": "Rest · keyframed pose table",
            "duration": 2.0,
            "loop": true,
            "motion": {
                "body": {
                    "position": [{
                        "kind": "keys",
                        "ease": "bezier",
                        "keys": [[0.0, [0.0, 0.0]], [1.0, [0.0, -4.0]], [2.0, [0.0, 0.0]]]
                    }],
                    "rotation": [{
                        "kind": "keys",
                        "ease": "bezier",
                        "keys": [[0.0, 0.0], [1.0, 1.2], [2.0, 0.0]]
                    }]
                }
            }
        }
    ])
}

#[allow(clippy::too_many_arguments)]
fn build_cutout_plan_json(
    id: &str,
    src_path: &Path,
    pcraft_path: &Path,
    matted_path: &Path,
    cropped_path: &Path,
    svg_path: &Path,
    w: u32,
    h: u32,
    bbox: [u32; 4],
    pivot_f: [f64; 2],
    rest_tail: [f64; 2],
    hinge_r: u32,
    faces_res: Value,
    trace_res: Value,
    pins_res: Value,
) -> Value {
    let pivot = [pivot_f[0].round() as u32, pivot_f[1].round() as u32];
    json!({
        "ok": true,
        "rig_type": "layered_motion",
        "character_id": id,
        "source_image": src_path,
        "pcraft_path": pcraft_path,
        "canvas": [w, h],
        "canvas_size": [w, h],
        "fps": 24,
        "faces": faces_res,
        "vector_trace": trace_res,
        "puppet_pins": pins_res,
        "layers": [{
            "id": "body",
            "file": matted_path,
            "svg": svg_path,
            "pivot": pivot_f,
            "rest_tail": rest_tail,
            "hinge_radius_px": hinge_r,
            "puppet_pins": pins_res,
            "parent": Value::Null,
        }],
        "parts": [{
            "id": "body",
            "label": "Body",
            "z_index": 0,
            "bbox": bbox,
            "pivot": pivot,
            "rest_tail": rest_tail,
            "hinge_radius_px": hinge_r,
            "puppet_pins": pins_res,
            "cropped_path": cropped_path,
            "full_layer_path": matted_path,
            "svg_path": svg_path,
        }],
        "tracks": default_rig_tracks(),
    })
}

fn write_and_attach_cutout_plan(
    s: &mut Session,
    id: &str,
    out_dir: &Path,
    plan_json: &Value,
) -> Result<(PathBuf, PathBuf)> {
    let plan_path = out_dir.join(format!("{id}_cutout_rig_plan.json"));
    let rig_path = out_dir.join("rig.json");
    let manifest_path = out_dir.join("rig_manifest.json");
    let encoded = serde_json::to_vec_pretty(plan_json)
        .map_err(|e| EngineError::InvalidParams(e.to_string()))?;
    for path in [&plan_path, &rig_path, &manifest_path] {
        std::fs::write(path, &encoded)?;
    }
    let plan = crate::session::LoadedCutoutRigPlan::load_from_path(&plan_path)?;
    if let Some(asset) = s.current_asset_mut() {
        asset.attached_asset_sheet = plan.cutout_asset_sheet.clone();
        asset.cutout_rig = Some(plan);
        asset.sync_parts_from_manifests();
    }
    Ok((rig_path, manifest_path))
}

/// Build a 2.5D Puppet & Deform Rig plan from reviewed artwork (Issue #55 Stage 1).
pub fn cmd_asset_build_cutout_rig(s: &mut Session, params: &Value) -> Result<Value> {
    activate_requested_asset(s, params)?;
    let asset = s
        .current_asset()
        .ok_or_else(|| EngineError::InvalidParams("no active asset".into()))?;
    let (id, src_path) = (asset.id.clone(), asset.path.clone());
    crate::sheets::require_sheet_review(s, &id, &src_path)?;

    let root = crate::workspace::workspace_root(s);
    let out_dir = prepare_dir(s, &id);
    for sub in ["full_layers", "parts", "vectors"] {
        std::fs::create_dir_all(out_dir.join(sub))?;
    }
    let matted_path = out_dir.join("full_layers/body.png");
    let cropped_path = out_dir.join("parts/body.png");
    let svg_path = out_dir.join("vectors/body.svg");
    let pcraft_path = out_dir.join(format!("{id}.pcraft"));
    let (pc_out, vc_out) =
        isolate_and_trace_character(&root, &src_path, &matted_path, &cropped_path, &svg_path)?;

    let (w, h, bbox) = read_photocraft_info(&root, &matted_path)?;
    let faces_res = find_jsonl_cmd_result(&pc_out, "image.detectFaces")
        .and_then(|r| r.get("faces").cloned())
        .unwrap_or_else(|| json!([]));
    let trace_res =
        find_jsonl_cmd_result(&vc_out, "imageTrace.makeAndExpand").unwrap_or(Value::Null);
    let pins_res = find_jsonl_cmd_result(&vc_out, "object.puppetWarp.pins")
        .and_then(|r| r.get("pins").cloned())
        .unwrap_or_else(|| json!([]));
    let pivot_f = [
        bbox[0] as f64 + bbox[2] as f64 / 2.0,
        bbox[1] as f64 + bbox[3] as f64 * 0.55,
    ];
    let rest_tail = [pivot_f[0], (bbox[1] + bbox[3]) as f64];
    let hinge_r = extend_circular_hinge_cap(
        &root,
        &matted_path,
        pivot_f,
        (bbox[2].min(bbox[3]) as f64) * 0.35,
        &pcraft_path,
    )?;

    let plan_json = build_cutout_plan_json(
        &id,
        &src_path,
        &pcraft_path,
        &matted_path,
        &cropped_path,
        &svg_path,
        w,
        h,
        bbox,
        pivot_f,
        rest_tail,
        hinge_r,
        faces_res,
        trace_res,
        pins_res,
    );
    let (rig_path, manifest_path) = write_and_attach_cutout_plan(s, &id, &out_dir, &plan_json)?;
    let synced_video_tracks = sync_placed_tracks(s, &id);
    Ok(json!({
        "ok": true,
        "asset_id": id,
        "output": out_dir,
        "rig": rig_path,
        "rig_manifest": manifest_path,
        "pcraft": pcraft_path,
        "svg": svg_path,
        "synced_video_tracks": synced_video_tracks,
    }))
}

fn prepare_dir(s: &Session, id: &str) -> PathBuf {
    if s.character_file
        && let Some(dir) = s.project_path.as_deref().and_then(Path::parent)
    {
        return dir.join("rig");
    }
    crate::workspace::workspace_root(s)
        .join("exports/pose_and_parts")
        .join(id)
}

fn activate_requested_asset(s: &mut Session, params: &Value) -> Result<()> {
    if s.project_path.is_none()
        && (params.get("isolated").and_then(Value::as_bool) == Some(true)
            || params.get("use_default_project").and_then(Value::as_bool) == Some(true))
    {
        let _ = crate::project::ensure_default_asset_project(s)?;
    }
    if let Some(asset_id) = params
        .get("asset")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        if !s.assets.contains_key(asset_id) {
            let _ = crate::project::cmd_project_load_asset(s, &json!({"id": asset_id}));
        }
        if s.assets.contains_key(asset_id) {
            s.active_asset = Some(asset_id.to_string());
        }
    }
    Ok(())
}

fn sync_placed_tracks(s: &mut Session, id: &str) -> Vec<Value> {
    let mut synced_video_tracks = Vec::new();
    if let Some(dir) = s.project_path.as_ref().and_then(|p| p.parent()) {
        let placed = crate::video_target::read_placed_tracks(dir);
        let mut tracks_to_sync: Vec<String> = Vec::new();
        for item in &placed {
            if item["character"].as_str() == Some(id)
                && let Some(tr) = item["track"].as_str().filter(|t| !t.is_empty())
                && !tracks_to_sync.iter().any(|existing| existing == tr)
            {
                tracks_to_sync.push(tr.to_string());
            }
        }
        for tr in tracks_to_sync {
            if let Ok(exported) = crate::motion::cmd_motion_export_track(
                s,
                &json!({"character": id, "track": tr, "include_theatre": true}),
            ) && let Some(arr) = exported["synced_placements"].as_array()
            {
                synced_video_tracks.extend(arr.iter().cloned());
            }
        }
    }
    synced_video_tracks
}
