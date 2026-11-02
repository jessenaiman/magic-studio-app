use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Matte one reference through PhotoCraft (`layer.removeBackground` + `layer.matting.colorDecontaminate`).
pub(crate) fn matte_image(s: &Session, source: &Path, p: &Value) -> Result<Value> {
    if p["reuse_alpha"].as_bool() == Some(true) {
        let pixels = image::open(source)?.into_rgba8();
        if !pixels.pixels().any(|px| px[3] < 255) || !pixels.pixels().any(|px| px[3] > 0) {
            return Err(EngineError::InvalidParams(
                "The prepared reference needs visible pixels and transparency".into(),
            ));
        }
        return Ok(json!({
            "ok": true,
            "image": source,
            "source_image": source,
            "width": pixels.width(),
            "height": pixels.height(),
            "has_alpha": true,
            "reused_alpha": true
        }));
    }
    let root = crate::workshop::project_dir(s)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| EngineError::InvalidParams(e.to_string()))?
        .as_nanos();
    let directory = root.join("assets/reference-previews");
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
    let tool_output = crate::workspace::run_tool_in(
        &crate::workspace::workspace_root(s),
        "photocraft-cli",
        &args,
    )?;
    let pixels = image::open(&out)?.into_rgba8();
    let (width, height) = pixels.dimensions();
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
            .ok_or_else(|| EngineError::InvalidParams("no active asset or `source` image".into()))?,
    };
    if !source.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "source image not found: {}",
            source.display()
        )));
    }
    let mut result = matte_image(s, &source, params)?;
    let attach = params.get("attach").and_then(Value::as_bool).unwrap_or(true);
    if attach
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

fn fallback_sheet_layout(sheet_path: &Path, kind_raw: &str) -> Result<Value> {
    let (w, h) = image::image_dimensions(sheet_path)?;
    let (cols, rows) = (2u32, 2u32);
    let cw = (w / cols).max(16);
    let ch = (h / rows).max(16);
    let mut frames = Vec::new();
    for idx in 0..4u32 {
        let col = idx % cols;
        let row = idx / cols;
        let l = col * cw;
        let t = row * ch;
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
        let bytes = std::fs::read(&p)?;
        return serde_json::from_slice(&bytes)
            .map_err(|e| EngineError::InvalidParams(format!("invalid layout JSON: {e}")));
    }
    if let Some(layout) = crate::sheets::layout::for_sheet(sheet_path)? {
        return Ok(layout);
    }
    let kind_raw = params.get("kind").and_then(Value::as_str).unwrap_or("action");
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
    let bw = r.saturating_sub(l).max(16);
    let bh = b.saturating_sub(t).max(16);
    Ok([l, t, bw, bh])
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
    let need_pad = (bw, bh) != (cw, ch);
    let crop_target = if need_pad {
        out_dir.join(format!("frame_{index:02}_crop.png"))
    } else {
        final_png.clone()
    };
    crate::workspace::run_tool_in(
        root,
        "photocraft-cli",
        &[
            "run".into(),
            sheet_path.to_string_lossy().into_owned(),
            "--cmd".into(),
            "image.crop".into(),
            "--params".into(),
            json!({"x": x, "y": y, "width": bw, "height": bh, "deleteCroppedPixels": true})
                .to_string(),
            "--cmd".into(),
            "layer.removeBackground".into(),
            "--params".into(),
            "{\"refine\":true}".into(),
            "--cmd".into(),
            "layer.matting.colorDecontaminate".into(),
            "--params".into(),
            "{\"amount\":100,\"radius\":4}".into(),
            "--out".into(),
            crop_target.to_string_lossy().into_owned(),
        ],
    )?;
    if need_pad {
        crate::workspace::run_tool_in(
            root,
            "photocraft-cli",
            &[
                "run".into(),
                "--new".into(),
                json!({"width": cw, "height": ch, "background": "transparent"}).to_string(),
                "--cmd".into(),
                "file.placeEmbedded".into(),
                "--params".into(),
                json!({
                    "path": crop_target,
                    "scale": 100.0,
                    "fit": false,
                    "center": [cw as f64 / 2.0, ch as f64 / 2.0]
                })
                .to_string(),
                "--out".into(),
                final_png.to_string_lossy().into_owned(),
            ],
        )?;
        let _ = std::fs::remove_file(&crop_target);
    }
    Ok(file_name)
}

/// Extract reviewed sheet panels into uniform transparent PNGs + `manifest.json` via `photocraft-cli`.
pub fn cmd_asset_cut_sheet(s: &mut Session, params: &Value) -> Result<Value> {
    activate_requested_asset(s, params)?;
    let asset_id = params
        .get("asset")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .or_else(|| s.current_asset().map(|a| a.id.clone()))
        .unwrap_or_else(|| "sheet".into());
    let sheet_path = match params
        .get("source")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        Some(raw) => crate::workspace::resolve_workspace_path(s, raw),
        None => s
            .current_asset()
            .map(|a| a.path.clone())
            .ok_or_else(|| EngineError::InvalidParams("no active asset or `source` sheet".into()))?,
    };
    if !sheet_path.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "sheet not found: {}",
            sheet_path.display()
        )));
    }
    if s.project_assets.iter().any(|a| a.id == asset_id) {
        crate::sheets::require_sheet_review(s, &asset_id, &sheet_path)?;
    }
    let layout = resolve_cut_layout(s, &sheet_path, params)?;
    let panels = layout["frames"]
        .as_array()
        .filter(|f| !f.is_empty())
        .ok_or_else(|| EngineError::InvalidParams("sheet layout has no frames".into()))?;
    let rects: Vec<[u32; 4]> = panels
        .iter()
        .map(parse_panel_box)
        .collect::<Result<Vec<_>>>()?;
    let canvas_w = rects.iter().map(|r| r[2]).max().unwrap_or(256);
    let canvas_h = rects.iter().map(|r| r[3]).max().unwrap_or(256);
    let out_dir = match params
        .get("out_dir")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        Some(raw) => crate::workspace::resolve_workspace_path(s, raw),
        None => prepare_dir(s, &asset_id).join("pieces"),
    };
    std::fs::create_dir_all(&out_dir)?;
    let root = crate::workspace::workspace_root(s);
    let mut out_frames = Vec::with_capacity(panels.len());
    for (idx, (panel, rect)) in panels.iter().zip(rects.iter()).enumerate() {
        let file_name = cut_single_panel(&root, &sheet_path, &out_dir, idx, *rect, [canvas_w, canvas_h])?;
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
        serde_json::to_vec_pretty(&manifest).map_err(|e| EngineError::InvalidParams(e.to_string()))?,
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

fn parse_u32_bounds(res: Option<&Value>, fallback: [u32; 4]) -> [u32; 4] {
    res.and_then(|r| r["bounds"].as_array())
        .filter(|b| b.len() == 4)
        .map(|b| {
            [
                b[0].as_u64().unwrap_or(0) as u32,
                b[1].as_u64().unwrap_or(0) as u32,
                b[2].as_u64().unwrap_or(fallback[2] as u64) as u32,
                b[3].as_u64().unwrap_or(fallback[3] as u64) as u32,
            ]
        })
        .unwrap_or(fallback)
}

/// Build a cutout rig plan from reviewed artwork using native `photocraft-cli` and `vectorcraft-cli`.
pub fn cmd_asset_build_cutout_rig(s: &mut Session, params: &Value) -> Result<Value> {
    activate_requested_asset(s, params)?;
    let asset = s
        .current_asset()
        .ok_or_else(|| EngineError::InvalidParams("no active asset".into()))?;
    let id = asset.id.clone();
    let src_path = asset.path.clone();
    crate::sheets::require_sheet_review(s, &id, &src_path)?;

    let root = crate::workspace::workspace_root(s);
    let out_dir = prepare_dir(s, &id);
    let full_layers_dir = out_dir.join("full_layers");
    let parts_dir = out_dir.join("parts");
    let svg_dir = out_dir.join("vectors");
    std::fs::create_dir_all(&full_layers_dir)?;
    std::fs::create_dir_all(&parts_dir)?;
    std::fs::create_dir_all(&svg_dir)?;

    // 1. Matte and detect face landmarks via photocraft-cli
    let matted_path = full_layers_dir.join("body.png");
    let pc_out = crate::workspace::run_tool_in(
        &root,
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

    // 2. Trim part via photocraft-cli image.trim
    let cropped_path = parts_dir.join("body.png");
    let _ = crate::workspace::run_tool_in(
        &root,
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

    // 3. Vector trace and auto-detect ARAP puppet warp pins via vectorcraft-cli
    let svg_path = svg_dir.join("body.svg");
    let vc_out = crate::workspace::run_tool_in(
        &root,
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

    let (w, h) = image::image_dimensions(&matted_path)?;
    let bg_res = find_jsonl_cmd_result(&pc_out, "layer.removeBackground");
    let faces_res = find_jsonl_cmd_result(&pc_out, "image.detectFaces")
        .and_then(|r| r.get("faces").cloned())
        .unwrap_or_else(|| json!([]));
    let trace_res = find_jsonl_cmd_result(&vc_out, "imageTrace.makeAndExpand").unwrap_or(Value::Null);
    let pins_res = find_jsonl_cmd_result(&vc_out, "object.puppetWarp.pins")
        .and_then(|r| r.get("pins").cloned())
        .unwrap_or_else(|| json!([]));
    let bbox = parse_u32_bounds(bg_res.as_ref(), [0, 0, w, h]);
    let pivot_f = pins_res
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(Value::as_array)
        .filter(|pt| pt.len() == 2)
        .and_then(|pt| Some([pt[0].as_f64()?, pt[1].as_f64()?]))
        .unwrap_or([
            bbox[0] as f64 + bbox[2] as f64 / 2.0,
            bbox[1] as f64 + bbox[3] as f64 / 2.0,
        ]);
    let pivot = [pivot_f[0].round() as u32, pivot_f[1].round() as u32];

    let plan_json = json!({
        "ok": true,
        "rig_type": "layered_motion",
        "character_id": id,
        "source_image": src_path,
        "canvas": [w, h],
        "canvas_size": [w, h],
        "fps": 30,
        "faces": faces_res,
        "vector_trace": trace_res,
        "puppet_pins": pins_res,
        "layers": [
            {
                "id": "body",
                "file": matted_path,
                "svg": svg_path,
                "pivot": pivot_f,
                "puppet_pins": pins_res,
                "parent": Value::Null,
            }
        ],
        "parts": [
            {
                "id": "body",
                "label": "Body",
                "z_index": 0,
                "bbox": bbox,
                "pivot": pivot,
                "puppet_pins": pins_res,
                "cropped_path": cropped_path,
                "full_layer_path": matted_path,
                "svg_path": svg_path,
            }
        ],
        "tracks": [
            {
                "id": "rest",
                "label": "Rest · gentle bob",
                "duration": 4,
                "loop": true,
                "motion": {
                    "body": {
                        "position": [{"kind": "oscillate", "axis": 1, "amplitude": 4.0, "hz": 0.5, "phase": 0.0}]
                    }
                }
            }
        ]
    });

    let plan_path = out_dir.join(format!("{id}_cutout_rig_plan.json"));
    let rig_path = out_dir.join("rig.json");
    let encoded = serde_json::to_vec_pretty(&plan_json)
        .map_err(|e| EngineError::InvalidParams(e.to_string()))?;
    std::fs::write(&plan_path, &encoded)?;
    std::fs::write(&rig_path, &encoded)?;

    let plan = crate::session::LoadedCutoutRigPlan::load_from_path(&plan_path)?;
    if let Some(asset) = s.current_asset_mut() {
        asset.attached_asset_sheet = plan.cutout_asset_sheet.clone();
        asset.cutout_rig = Some(plan);
        asset.sync_parts_from_manifests();
    }
    let synced_video_tracks = sync_placed_tracks(s, &id);
    Ok(json!({
        "ok": true,
        "asset_id": id,
        "output": out_dir,
        "rig": rig_path,
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
