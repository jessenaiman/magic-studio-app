//! `character.tripo.*`: Tripo Studio 3D character breakout, rigging, animation setup,
//! Multi-Stage Motion authoring, and split `.glb` + `.anim`/`.fbx` + `.blend` asset storage.
//!
//! Works alongside the 2D `character.*` (Scribbles / PhotoCraft / EffectCraft) workflow inside
//! `<project>/assets/characters/<id>/3d/` (and discovers legacy `<project>/assets/<id>/3d/` +
//! `<project>/3d/<id>-actor/` folders).

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::bad;
use crate::{EngineError, Result, Session};

const TRIPO_CATALOG: &str = include_str!("tripo_catalog.json");

pub(crate) fn catalog_json() -> Result<Value> {
    serde_json::from_str(TRIPO_CATALOG).map_err(|e| bad(format!("tripo_catalog.json: {e}")))
}

fn character_id(p: &Value) -> Result<String> {
    let raw = p["character"]
        .as_str()
        .ok_or_else(|| bad("character=<id> is required"))?;
    let id = crate::workspace::sanitize_asset_id(raw);
    if id.is_empty() {
        Err(bad("character=<id> is required"))
    } else {
        Ok(id)
    }
}

fn resolve_project_path(s: &Session, project_dir: &Path, raw: &str) -> PathBuf {
    let p = Path::new(raw);
    if p.is_absolute() {
        return p.to_path_buf();
    }
    let in_proj = project_dir.join(p);
    if in_proj.exists() {
        return in_proj;
    }
    crate::workspace::resolve_workspace_path(s, raw)
}

/// Canonical 3D directory for a character: `<project>/assets/characters/<id>/3d/`.
pub(crate) fn character_3d_dir(project_dir: &Path, id: &str) -> PathBuf {
    project_dir.join("assets/characters").join(id).join("3d")
}

fn candidate_3d_dirs(project_dir: &Path, id: &str) -> Vec<PathBuf> {
    vec![
        character_3d_dir(project_dir, id),
        project_dir.join("assets").join(id).join("3d"),
        project_dir.join("3d").join(format!("{id}-actor")),
        project_dir.join("3d").join(id),
    ]
}

fn has_matching_ext(path: &Path, exts: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| exts.iter().any(|w| e.eq_ignore_ascii_case(w)))
}

fn collect_files_with_ext(root: &Path, exts: &[&str], max_depth: usize) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() && depth < max_depth {
                stack.push((path, depth + 1));
            } else if path.is_file() && has_matching_ext(&path, exts) {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

fn collect_unique_ext(dirs: &[PathBuf], exts: &[&str], max_depth: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in dirs {
        for p in collect_files_with_ext(dir, exts, max_depth) {
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

fn read_json_optional(path: &Path) -> Option<Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
}

fn write_json_pretty(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| bad(e.to_string()))?;
    std::fs::write(path, format!("{text}\n"))?;
    Ok(())
}

fn anim_clip_entry(path: &Path, doc: &Value) -> Value {
    let fallback = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("clip")
        .trim_end_matches(".anim.json");
    json!({
        "id": doc["id"].as_str().unwrap_or(fallback),
        "name": doc["name"].as_str().unwrap_or(fallback),
        "source": doc["source"],
        "category": doc["category"],
        "duration": doc["duration"],
        "in_place": doc["in_place"],
        "fbx": doc["fbx"],
        "glb": doc["glb"],
        "blend": doc["blend"],
        "preview": doc["preview"],
        "stages": doc["stages"],
        "file": path,
    })
}

fn scan_json_metadata(dirs: &[PathBuf]) -> (Vec<Value>, Value, Value, Value) {
    let mut clips = Vec::new();
    let mut import_meta = Value::Null;
    let mut repair_meta = Value::Null;
    let mut multistage_meta = Value::Null;
    for p in collect_unique_ext(dirs, &["json"], 3) {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let Some(doc) = read_json_optional(&p) else {
            continue;
        };
        if name.ends_with(".anim.json") {
            clips.push(anim_clip_entry(&p, &doc));
        } else if name == "motion-library-import.json" && import_meta.is_null() {
            import_meta = doc;
        } else if name == "rigid-case-repair.json" && repair_meta.is_null() {
            repair_meta = doc;
        } else if name == "multistage-motion-request.json" && multistage_meta.is_null() {
            multistage_meta = doc;
        }
    }
    (clips, import_meta, repair_meta, multistage_meta)
}

fn append_imported_actions(
    clips: &mut Vec<Value>,
    import_meta: &Value,
    fbx: Option<&PathBuf>,
    glb: Option<&PathBuf>,
    blend: Option<&PathBuf>,
    preview: Option<&PathBuf>,
) {
    let Some(actions) = import_meta["actions"].as_array() else {
        return;
    };
    let fps = import_meta["fps"].as_f64().unwrap_or(24.0).max(1.0);
    for (idx, act) in actions.iter().enumerate() {
        let clip_id = format!("blender_action_{}", idx + 1);
        if clips.iter().any(|c| c["id"] == clip_id) {
            continue;
        }
        let start = act["range"][0].as_f64().unwrap_or(1.0);
        let end = act["range"][1].as_f64().unwrap_or(48.0);
        clips.push(json!({
            "id": clip_id,
            "name": act["name"].as_str().unwrap_or("action"),
            "source": "tripo_motion_library",
            "category": "interactive",
            "duration": ((end - start + 1.0) / fps * 100.0).round() / 100.0,
            "in_place": true,
            "fbx": fbx,
            "glb": glb,
            "blend": blend,
            "preview": preview,
        }));
    }
}

fn pick_primary_glb(manifest: &Value, glb_files: &[PathBuf]) -> Option<PathBuf> {
    manifest["base_model_glb"]
        .as_str()
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .or_else(|| {
            glb_files
                .iter()
                .find(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.contains("rigid-case") || n.contains("rigged"))
                })
                .cloned()
        })
        .or_else(|| glb_files.first().cloned())
}
/// The glTF JSON of a `.glb` (its first chunk) or a `.gltf` file.
fn gltf_document(path: &Path) -> Option<Value> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() >= 20 && &bytes[0..4] == b"glTF" {
        let chunk_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        if &bytes[16..20] == b"JSON" && bytes.len() >= 20 + chunk_len {
            return serde_json::from_slice(&bytes[20..20 + chunk_len]).ok();
        }
    }
    serde_json::from_slice(&bytes).ok()
}

/// Joints of the model's largest skin.
fn glb_joint_count(doc: &Value) -> u64 {
    doc["skins"]
        .as_array()
        .and_then(|skins| {
            skins
                .iter()
                .filter_map(|s| s["joints"].as_array().map(|j| j.len() as u64))
                .max()
        })
        .unwrap_or(0)
}

/// Names of the animation clips stored in the model.
fn glb_clip_names(doc: &Value) -> Vec<String> {
    doc["animations"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| a["name"].as_str().map(str::to_string))
        .collect()
}

/// Inspect 3D models, rigged FBX/GLB files, Blender scenes, split `.anim.json` clips,
/// and breakout sheets for one character across canonical and companion 3D folders.
pub(crate) fn inspect_character_3d(project_dir: &Path, id: &str) -> Value {
    let canonical = character_3d_dir(project_dir, id);
    let dirs = candidate_3d_dirs(project_dir, id);
    let glb_files = collect_unique_ext(&dirs, &["glb", "gltf"], 3);
    let fbx_files = collect_unique_ext(&dirs, &["fbx"], 3);
    let blend_files = collect_unique_ext(&dirs, &["blend"], 3);
    let tcraft_files = collect_unique_ext(&dirs, &["tcraft"], 3);
    let zip_files = collect_unique_ext(&dirs, &["zip"], 2);
    let video_previews = collect_unique_ext(&dirs, &["mp4", "webm", "mov"], 2);
    let (mut anim_clips, import_meta, repair_meta, multistage_meta) = scan_json_metadata(&dirs);
    append_imported_actions(
        &mut anim_clips,
        &import_meta,
        fbx_files.first(),
        glb_files.first(),
        blend_files.first(),
        video_previews.first(),
    );
    let manifest = read_json_optional(&canonical.join("tripo.json")).unwrap_or_else(|| json!({}));
    let primary_glb = pick_primary_glb(&manifest, &glb_files);
    let model_doc = primary_glb.as_deref().and_then(gltf_document);
    let glb_joints = model_doc.as_ref().map(glb_joint_count).unwrap_or(0);
    let model_clips = model_doc.as_ref().map(glb_clip_names).unwrap_or_default();
    let bone_count = import_meta["bones"].as_u64().unwrap_or(0).max(glb_joints);
    let rigged = bone_count > 0;
    let has_3d = primary_glb.is_some() || !fbx_files.is_empty() || !blend_files.is_empty();
    json!({
        "available": has_3d,
        "directory": canonical,
        "primary_glb": primary_glb,
        "glb_files": glb_files,
        "fbx_files": fbx_files,
        "blend_files": blend_files,
        "tcraft_files": tcraft_files,
        "zip_archives": zip_files,
        "video_previews": video_previews,
        "clips": anim_clips,
        "model_clips": model_clips,
        "rigged": rigged,
        "bone_count": bone_count,
        "rig_type": manifest["rig_type"].as_str().unwrap_or("biped"),
        "skeleton_preset": manifest["skeleton_preset"].as_str().unwrap_or("mixamo"),
        "animate_in_place": manifest["animate_in_place"].as_bool().unwrap_or(true),
        "fbx_preset": manifest["fbx_preset"].as_str().unwrap_or("blender"),
        "import_summary": import_meta,
        "rig_repair": repair_meta,
        "multistage_request": multistage_meta,
        "breakouts": manifest.get("breakouts").cloned().unwrap_or_else(|| json!([])),
        "precheck": manifest.get("precheck").cloned().unwrap_or(Value::Null),
    })
}

/// `character.tripo.catalog`: Return the full Craft Anything-captured Tripo Studio catalog.
pub fn cmd_tripo_catalog(_: &mut Session, p: &Value) -> Result<Value> {
    let mut cat = catalog_json()?;
    if let Some(category) = p["category"].as_str().filter(|s| !s.is_empty())
        && let Some(biped) = cat
            .pointer_mut("/animation/presets/biped")
            .and_then(Value::as_array_mut)
    {
        biped.retain(|item| item["category"].as_str() == Some(category));
    }
    Ok(json!({"ok": true, "catalog": cat}))
}

fn run_photocraft_cutout(breakout_dir: &Path, id: &str, src: &Path) -> Value {
    let out_pcraft = breakout_dir.join(format!("{id}-cutout.pcraft"));
    let out_png = breakout_dir.join(format!("{id}-cutout.png"));
    let layers_dir = breakout_dir.join(format!("{id}-split-layers"));
    let _ = std::fs::create_dir_all(&layers_dir);
    let args = vec![
        "run".into(),
        super::path_arg(src),
        "--cmd".into(),
        "layer.removeBackground".into(),
        "--cmd".into(),
        "file.export.layersToFiles".into(),
        "--params".into(),
        json!({"dir": layers_dir, "format": "png", "prefix": id, "trim": true}).to_string(),
        "--cmd".into(),
        "file.saveACopy".into(),
        "--params".into(),
        json!({"path": out_png, "overwrite": true}).to_string(),
        "--out".into(),
        super::path_arg(&out_pcraft),
    ];
    match crate::workspace::run_tool_in(breakout_dir, "photocraft-cli", &args) {
        Ok(_) => {
            let exported = collect_files_with_ext(&layers_dir, &["png"], 1);
            json!({
                "tool": "photocraft-cli",
                "status": "executed_local_cutout_and_trimmed_layers",
                "pcraft": out_pcraft,
                "png": out_png,
                "layers_dir": layers_dir,
                "exported_layers": exported,
            })
        }
        Err(e) => json!({"tool": "photocraft-cli", "status": "failed", "error": e.to_string()}),
    }
}

fn template_provider_meta(provider: &str, tpl: &Value) -> (&'static str, Value) {
    match provider {
        "google_flow" => (
            "https://flow.google.com/ (Nano Banana 2.1 · 0 credits via browser relay)",
            json!("nano_banana_2_1"),
        ),
        "codex_luna" => ("codex exec (gpt-6-luna draft)", json!("gpt-6-luna")),
        _ => (
            "/v2/studio/image/gen_image_v2",
            tpl["recommended_model"].clone(),
        ),
    }
}

fn build_breakout_record(
    kind: &str,
    p: &Value,
    reference: &Option<PathBuf>,
    breakout_dir: &Path,
    id: &str,
) -> Result<Value> {
    let catalog = catalog_json()?;
    let matched_tpl = catalog["breakout_templates"]
        .as_array()
        .and_then(|a| a.iter().find(|t| t["id"].as_str() == Some(kind)).cloned());
    let matched_op = catalog["breakout_operations"]
        .as_array()
        .and_then(|a| a.iter().find(|o| o["id"].as_str() == Some(kind)).cloned());
    let granularity = p["granularity"].as_str().unwrap_or("medium");
    if !matches!(granularity, "low" | "medium" | "high") {
        return Err(bad("granularity must be low, medium, or high"));
    }
    let completion_mode = p["completion_mode"].as_str().unwrap_or("quick_cap");
    if !matches!(completion_mode, "quick_cap" | "ai_completion") {
        return Err(bad("completion_mode must be quick_cap or ai_completion"));
    }
    let provider = p["provider"].as_str().unwrap_or("tripo");
    if !["tripo", "google_flow", "codex_luna"].contains(&provider) {
        return Err(bad("provider must be tripo, google_flow, or codex_luna"));
    }
    if let Some(tpl) = matched_tpl {
        let (endpoint, model) = template_provider_meta(provider, &tpl);
        return Ok(json!({
            "kind": kind,
            "provider": provider,
            "stage": "image_template",
            "endpoint": endpoint,
            "template_id": tpl["template_id"],
            "title": tpl["title"],
            "category": tpl["category"],
            "recommended_model": model,
            "recommended_ratio": tpl["recommended_ratio"],
            "t_pose": kind == "t_pose",
            "reference": reference,
            "prompt": p["prompt"].as_str().unwrap_or_else(|| tpl["description"].as_str().unwrap_or_default()),
        }));
    }
    if let Some(op) = matched_op {
        let craft_cli = match (kind, reference.as_deref()) {
            ("split_image", Some(src)) => run_photocraft_cutout(breakout_dir, id, src),
            _ => Value::Null,
        };
        return Ok(json!({
            "kind": kind,
            "provider": provider,
            "stage": "breakout_operation",
            "endpoint": op["endpoint"],
            "label": op["label"],
            "granularity": (kind == "ai_segmentation").then_some(granularity),
            "completion_mode": (kind == "mesh_completion").then_some(completion_mode),
            "views": op.get("views"),
            "reference": reference,
            "craft_cli": craft_cli,
        }));
    }
    Err(bad(format!("unknown Tripo breakout kind: {kind}")))
}

/// `character.tripo.breakout`: Prepare or run a post-initial-image Tripo breakout step.
pub fn cmd_tripo_breakout(s: &mut Session, p: &Value) -> Result<Value> {
    let id = character_id(p)?;
    let kind = p["kind"].as_str().unwrap_or("t_pose");
    let project_dir = crate::workshop::project_dir(s)?;
    let dir = character_3d_dir(&project_dir, &id);
    let breakout_dir = dir.join("breakouts");
    std::fs::create_dir_all(&breakout_dir)?;

    let reference = p["reference"]
        .as_str()
        .filter(|r| !r.trim().is_empty())
        .map(|r| resolve_project_path(s, &project_dir, r));
    if let Some(path) = &reference
        && !path.is_file()
    {
        return Err(bad(format!(
            "reference image does not exist: {}",
            path.display()
        )));
    }
    let record = build_breakout_record(kind, p, &reference, &breakout_dir, &id)?;
    let receipt_path = breakout_dir.join(format!("breakout-{kind}.json"));
    write_json_pretty(&receipt_path, &record)?;

    let manifest_path = dir.join("tripo.json");
    let mut manifest =
        read_json_optional(&manifest_path).unwrap_or_else(|| json!({"character": id}));
    if !manifest["breakouts"].is_array() {
        manifest["breakouts"] = json!([]);
    }
    if let Some(arr) = manifest["breakouts"].as_array_mut() {
        arr.retain(|entry| entry["kind"] != kind);
        arr.push(record.clone());
    }
    write_json_pretty(&manifest_path, &manifest)?;
    Ok(json!({
        "ok": true,
        "character": id,
        "breakout": record,
        "receipt": receipt_path,
        "manifest": manifest_path,
    }))
}

fn validate_rig_options(p: &Value) -> Result<(&str, &str, &str, u64)> {
    let rig_type = p["rig_type"].as_str().unwrap_or("biped");
    if ![
        "biped",
        "quadruped",
        "hexapod",
        "octopod",
        "avian",
        "serpentine",
        "aquatic",
        "others",
    ]
    .contains(&rig_type)
    {
        return Err(bad("invalid rig_type"));
    }
    let skel = p["skeleton_preset"].as_str().unwrap_or("mixamo");
    if !["mixamo", "actorcore", "unreal", "vrm", "unity"].contains(&skel) {
        return Err(bad("invalid skeleton_preset"));
    }
    let topology = p["topology"].as_str().unwrap_or("quad");
    if !["quad", "triangle"].contains(&topology) {
        return Err(bad("topology must be quad or triangle"));
    }
    let max_faces = if topology == "quad" { 50_000 } else { 80_000 };
    Ok((rig_type, skel, topology, max_faces))
}

fn probe_mesh_with_texturecraft(dir: &Path, model_path: &Option<PathBuf>) -> Value {
    let Some(mesh) = model_path.as_ref().filter(|m| m.is_file()) else {
        return Value::Null;
    };
    crate::workspace::run_tool_in(
        dir,
        "texturecraft-cli",
        &["info".into(), super::path_arg(mesh)],
    )
    .ok()
    .and_then(|out| serde_json::from_str::<Value>(&out).ok())
    .unwrap_or(Value::Null)
}

fn build_precheck_value(
    p: &Value,
    current: &Value,
    tc_report: &Value,
    topology: &str,
    max_faces: u64,
    t_pose_ready: bool,
) -> (bool, Value) {
    let observed_faces = p["face_count"]
        .as_u64()
        .or_else(|| tc_report["mesh"]["triangles"].as_u64())
        .or_else(|| tc_report["triangles"].as_u64())
        .or_else(|| current["import_summary"]["faces"].as_u64())
        .unwrap_or(10_000);
    let observed_vertices = tc_report["mesh"]["vertices"].as_u64();
    let within_limit = observed_faces <= max_faces;
    let fallback = if !within_limit {
        Some("retopologize")
    } else if !t_pose_ready {
        Some("generate_t_pose")
    } else {
        None
    };
    let riggable = within_limit && t_pose_ready;
    (
        riggable,
        json!({
            "endpoint": "/v2/studio/operation/pre_rig_check",
            "model_version": "v3.0-20260909",
            "topology": topology,
            "face_count": observed_faces,
            "vertex_count": observed_vertices,
            "face_limit": max_faces,
            "within_polygon_limit": within_limit,
            "t_pose_ready": t_pose_ready,
            "riggable": riggable,
            "texturecraft_info": tc_report,
            "recommended_fallback": fallback,
        }),
    )
}

/// `character.tripo.rig`: Configure and validate Tripo 3D Pre-Rig Check + Auto-Rigging.
pub fn cmd_tripo_rig(s: &mut Session, p: &Value) -> Result<Value> {
    let id = character_id(p)?;
    let (rig_type, skeleton_preset, topology, max_faces) = validate_rig_options(p)?;
    let project_dir = crate::workshop::project_dir(s)?;
    let current = inspect_character_3d(&project_dir, &id);
    let t_pose_ready = p["t_pose_ready"].as_bool().unwrap_or(true);
    let dir = character_3d_dir(&project_dir, &id);
    std::fs::create_dir_all(&dir)?;

    let model_path = p["model"]
        .as_str()
        .filter(|m| !m.trim().is_empty())
        .map(|m| resolve_project_path(s, &project_dir, m))
        .or_else(|| current["primary_glb"].as_str().map(PathBuf::from));
    let tc_report = probe_mesh_with_texturecraft(&dir, &model_path);
    let (riggable, precheck) =
        build_precheck_value(p, &current, &tc_report, topology, max_faces, t_pose_ready);
    let rig_payload = json!({
        "endpoint": "/v2/studio/operation/rigging_model",
        "model_version": "v3.0-20260909",
        "rig_type": rig_type,
        "spec": (rig_type == "biped").then_some(skeleton_preset),
        "credits": 20,
    });
    let model_joints = model_path
        .as_deref()
        .and_then(gltf_document)
        .map(|d| glb_joint_count(&d))
        .unwrap_or(0);
    let bone_count = current["bone_count"]
        .as_u64()
        .unwrap_or(0)
        .max(model_joints);
    let verified_skeleton = bone_count > 0;
    let manifest_path = dir.join("tripo.json");
    let mut manifest =
        read_json_optional(&manifest_path).unwrap_or_else(|| json!({"character": id}));
    manifest["character"] = json!(id);
    manifest["rigged"] = json!(verified_skeleton && riggable);
    manifest["bone_count"] = json!(bone_count);
    manifest["precheck_passed"] = json!(riggable);
    manifest["rig_type"] = json!(rig_type);
    manifest["skeleton_preset"] = json!(skeleton_preset);
    manifest["topology"] = json!(topology);
    manifest["precheck"] = precheck.clone();
    manifest["rig_request"] = rig_payload.clone();
    if let Some(model) = &model_path {
        manifest["base_model_glb"] = json!(model);
    }
    write_json_pretty(&manifest_path, &manifest)?;
    Ok(json!({
        "ok": true,
        "character": id,
        "rigged": verified_skeleton && riggable,
        "bone_count": bone_count,
        "precheck_passed": riggable,
        "rigging_execution": if verified_skeleton {
            "verified_local_skeleton"
        } else {
            "precheck_only_cloud_rigging_requires_tripo_studio"
        },
        "precheck": precheck,
        "rig_request": rig_payload,
        "manifest": manifest_path,
    }))
}

fn parse_stage_waypoints(stage: &Value, prev_end: Option<[f64; 2]>) -> Result<([f64; 2], Value)> {
    let Some(wp) = stage.get("waypoints").and_then(Value::as_array) else {
        let end = [
            stage["x"].as_f64().unwrap_or(0.0),
            stage["z"].as_f64().unwrap_or(0.0),
        ];
        let start = prev_end.unwrap_or([0.0, 0.0]);
        return Ok((end, json!([[start[0], start[1]], [end[0], end[1]]])));
    };
    if wp.len() != 2 {
        return Err(bad("waypoints must be [[x0, z0], [x1, z1]]"));
    }
    let p0 = [
        wp[0]
            .get(0)
            .and_then(Value::as_f64)
            .ok_or_else(|| bad("invalid waypoint x0"))?,
        wp[0]
            .get(1)
            .and_then(Value::as_f64)
            .ok_or_else(|| bad("invalid waypoint z0"))?,
    ];
    let p1 = [
        wp[1]
            .get(0)
            .and_then(Value::as_f64)
            .ok_or_else(|| bad("invalid waypoint x1"))?,
        wp[1]
            .get(1)
            .and_then(Value::as_f64)
            .ok_or_else(|| bad("invalid waypoint z1"))?,
    ];
    if let Some(prev) = prev_end
        && ((prev[0] - p0[0]).abs() > 1e-6 || (prev[1] - p0[1]).abs() > 1e-6)
    {
        return Err(bad(
            "stage start waypoint must match previous stage end waypoint",
        ));
    }
    Ok((p1, json!([[p0[0], p0[1]], [p1[0], p1[1]]])))
}

fn validate_multistage_segments(stages: &[Value]) -> Result<Vec<Value>> {
    if stages.is_empty() || stages.len() > 5 {
        return Err(bad("Multi-Stage Motion requires 1 to 5 stages"));
    }
    let mut normalized = Vec::with_capacity(stages.len());
    let mut prev_end: Option<[f64; 2]> = None;
    for (idx, stage) in stages.iter().enumerate() {
        let prompt = stage["prompt"]
            .as_str()
            .or_else(|| stage["action"].as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty() && s.chars().count() <= 1000)
            .ok_or_else(|| bad("every stage needs a prompt/action (1..=1000 chars)"))?;
        let duration = stage["duration_seconds"]
            .as_u64()
            .or_else(|| stage["duration"].as_u64())
            .filter(|d| (1..=10).contains(d))
            .ok_or_else(|| bad("duration_seconds must be 1..=10"))?;
        let (end_pt, waypoints) = parse_stage_waypoints(stage, prev_end)?;
        prev_end = Some(end_pt);
        normalized.push(json!({
            "name": stage["name"].as_str().unwrap_or(&format!("Stage {}", idx + 1)),
            "prompt": prompt,
            "duration_seconds": duration,
            "waypoints": waypoints,
        }));
    }
    Ok(normalized)
}

struct ClipSpec {
    id: String,
    name: String,
    source: &'static str,
    category: String,
    duration: f64,
    stages: Value,
    api_payload: Value,
}

fn build_clip_spec(p: &Value, rig_type: &str) -> Result<ClipSpec> {
    if let Some(stages_arr) = p.get("stages").and_then(Value::as_array) {
        if rig_type != "biped" {
            return Err(bad("Multi-Stage Motion requires rig_type=biped"));
        }
        let stages = validate_multistage_segments(stages_arr)?;
        let dur: u64 = stages
            .iter()
            .filter_map(|s| s["duration_seconds"].as_u64())
            .sum();
        let raw_name = p["name"].as_str().unwrap_or("multi_stage_motion");
        let cid = crate::workspace::sanitize_asset_id(raw_name);
        return Ok(ClipSpec {
            id: if cid.is_empty() {
                "multi_stage".into()
            } else {
                cid
            },
            name: raw_name.to_string(),
            source: "multi_stage_motion",
            category: "interactive".into(),
            duration: dur as f64,
            stages: json!(stages),
            api_payload: json!({
                "generate_endpoint": "/v2/studio/motion/generate",
                "segments": stages,
                "retarget_endpoint": "/v2/studio/operation/retarget_model",
                "rig_type": rig_type,
                "credits": 20,
            }),
        });
    }
    if let Some(prompt) = p["prompt"].as_str().filter(|s| !s.trim().is_empty()) {
        if rig_type != "biped" {
            return Err(bad("AI Text-to-Motion requires rig_type=biped"));
        }
        let dur = p["duration"].as_u64().unwrap_or(4).clamp(1, 10);
        let stages = validate_multistage_segments(&[json!({
            "name": "Start",
            "prompt": prompt,
            "duration_seconds": dur,
        })])?;
        let raw_name = p["name"].as_str().unwrap_or(prompt);
        let cid = crate::workspace::sanitize_asset_id(raw_name);
        return Ok(ClipSpec {
            id: if cid.is_empty() {
                "ai_motion".into()
            } else {
                cid
            },
            name: raw_name.chars().take(64).collect(),
            source: "ai_animation",
            category: "interactive".into(),
            duration: dur as f64,
            stages: json!(stages),
            api_payload: json!({
                "generate_endpoint": "/v2/studio/motion/generate",
                "segments": stages,
                "retarget_endpoint": "/v2/studio/operation/retarget_model",
                "rig_type": rig_type,
                "credits": 20,
            }),
        });
    }
    let catalog = catalog_json()?;
    let presets = catalog
        .pointer(&format!("/animation/presets/{rig_type}"))
        .and_then(Value::as_array)
        .ok_or_else(|| bad(format!("no presets for rig_type={rig_type}")))?;
    let default_preset = if rig_type == "biped" {
        "wave_goodbye_01"
    } else {
        presets
            .first()
            .and_then(|item| item["name"].as_str())
            .unwrap_or("walk")
    };
    let preset_name = p["preset"].as_str().unwrap_or(default_preset);
    let found = presets
        .iter()
        .find(|item| item["name"].as_str() == Some(preset_name))
        .ok_or_else(|| bad(format!("unknown preset '{preset_name}'")))?;
    Ok(ClipSpec {
        id: crate::workspace::sanitize_asset_id(preset_name),
        name: preset_name.replace('_', " "),
        source: "preset",
        category: found["category"].as_str().unwrap_or("basic").into(),
        duration: p["duration"].as_f64().unwrap_or(3.0).max(0.5),
        stages: Value::Null,
        api_payload: json!({
            "retarget_endpoint": "/v2/studio/operation/retarget_model",
            "rig_type": rig_type,
            "animations": [format!("preset:{rig_type}:{preset_name}")],
            "model_version": "default",
        }),
    })
}

fn build_effectcraft_3d_comp(
    clips_dir: &Path,
    id: &str,
    clip_id: &str,
    duration: f64,
    model_file: Option<&PathBuf>,
) -> (Value, Value) {
    let Some(file) = model_file.filter(|f| f.is_file()) else {
        return (Value::Null, Value::Null);
    };
    let comp_name = format!("{id}-{clip_id}-3d");
    let ec_path = clips_dir.join(format!("{comp_name}.ecproj"));
    let frame_path = clips_dir.join(format!("{comp_name}-frame.png"));
    let steps = vec![
        super::step(
            "comp.new",
            json!({
                "name": comp_name,
                "width": 1280,
                "height": 720,
                "frameRate": 24,
                "duration": duration,
                "renderer": "advanced3D",
                "open": true
            }),
        ),
        super::step(
            "layer.newModel",
            json!({"path": file, "name": id, "position": [640, 360]}),
        ),
        super::step("file.saveAs", json!({"path": ec_path})),
    ];
    if super::effectcraft(clips_dir, None, &steps).is_err() {
        return (Value::Null, Value::Null);
    }
    let render_args = vec![
        "render-frame".into(),
        "--project".into(),
        super::path_arg(&ec_path),
        "--comp".into(),
        comp_name,
        "--time".into(),
        (duration * 0.5).min(1.0).to_string(),
        "--transparent".into(),
        "--out".into(),
        super::path_arg(&frame_path),
    ];
    let rendered = crate::workspace::run_tool_in(clips_dir, "effectcraft-cli", &render_args)
        .ok()
        .filter(|_| frame_path.is_file())
        .map(|_| json!(frame_path))
        .unwrap_or(Value::Null);
    (json!(ec_path), rendered)
}

fn first_or_param_path(
    s: &Session,
    project_dir: &Path,
    p: &Value,
    key: &str,
    fallback: &Value,
) -> Option<PathBuf> {
    p[key]
        .as_str()
        .filter(|v| !v.is_empty())
        .map(|v| resolve_project_path(s, project_dir, v))
        .or_else(|| {
            fallback
                .as_str()
                .or_else(|| fallback.as_array()?.first()?.as_str())
                .map(PathBuf::from)
        })
}
fn clip_specific_glb(clips_dir: &Path, id: &str, clip_id: &str, rig_type: &str) -> Option<PathBuf> {
    let candidates = [
        clips_dir.join(format!("{id}-preset_{rig_type}_{clip_id}.glb")),
        clips_dir.join(format!("{id}-{clip_id}.glb")),
        clips_dir.join(format!("{clip_id}.glb")),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

/// `character.tripo.animate`: Author a split `.anim.json` clip and EffectCraft 3D comp.
pub fn cmd_tripo_animate(s: &mut Session, p: &Value) -> Result<Value> {
    let id = character_id(p)?;
    let project_dir = crate::workshop::project_dir(s)?;
    let current = inspect_character_3d(&project_dir, &id);
    let rig_type = p["rig_type"]
        .as_str()
        .or_else(|| current["rig_type"].as_str())
        .unwrap_or("biped");
    let animate_in_place = p["animate_in_place"].as_bool().unwrap_or(true);
    let fbx_preset = p["fbx_preset"].as_str().unwrap_or("blender");
    if !["blender", "3dsmax", "maya", "unreal", "unity"].contains(&fbx_preset) {
        return Err(bad("invalid fbx_preset"));
    }
    let spec = build_clip_spec(p, rig_type)?;
    let dir = character_3d_dir(&project_dir, &id);
    let clips_dir = dir.join("clips");
    std::fs::create_dir_all(&clips_dir)?;

    let fbx_path = first_or_param_path(s, &project_dir, p, "fbx", &current["fbx_files"]);
    let glb_path = p["glb"]
        .as_str()
        .filter(|v| !v.is_empty())
        .map(|v| resolve_project_path(s, &project_dir, v))
        .or_else(|| clip_specific_glb(&clips_dir, &id, &spec.id, rig_type))
        .or_else(|| first_or_param_path(s, &project_dir, p, "glb", &current["primary_glb"]));
    let blend_path = first_or_param_path(s, &project_dir, p, "blend", &current["blend_files"]);
    let preview_path =
        first_or_param_path(s, &project_dir, p, "preview", &current["video_previews"]);
    let (ecproj_3d, rendered_frame) = build_effectcraft_3d_comp(
        &clips_dir,
        &id,
        &spec.id,
        spec.duration,
        glb_path.as_ref().or(fbx_path.as_ref()),
    );
    let clip_doc = json!({
        "schema": "magicstudio.tripo.anim.v1",
        "id": spec.id,
        "character": id,
        "name": spec.name,
        "source": spec.source,
        "category": spec.category,
        "rig_type": rig_type,
        "duration": spec.duration,
        "in_place": animate_in_place,
        "fbx_preset": fbx_preset,
        "glb": glb_path,
        "fbx": fbx_path,
        "blend": blend_path,
        "ecproj_3d": ecproj_3d,
        "rendered_frame": rendered_frame,
        "retarget_execution": if ecproj_3d.is_string() {
            "composed_in_effectcraft_advanced3d_from_existing_rigged_mesh"
        } else {
            "manifest_only_cloud_retarget_requires_tripo_studio"
        },
        "preview": preview_path,
        "stages": spec.stages,
        "tripo_api": spec.api_payload,
        "export_spec": {
            "endpoint": "/v2/studio/operation/export",
            "format": p["format"].as_str().unwrap_or("fbx"),
            "fbx_preset": fbx_preset,
            "animate_in_place": animate_in_place,
            "with_animation": true,
            "export_orientation": "-y",
            "texture_size": p["texture_size"].as_str().unwrap_or("2K"),
            "texture_packaging": "zip",
        },
    });
    let clip_file = clips_dir.join(format!("{}.anim.json", spec.id));
    write_json_pretty(&clip_file, &clip_doc)?;

    let manifest_path = dir.join("tripo.json");
    let mut manifest =
        read_json_optional(&manifest_path).unwrap_or_else(|| json!({"character": id}));
    manifest["character"] = json!(id);
    manifest["animate_in_place"] = json!(animate_in_place);
    manifest["fbx_preset"] = json!(fbx_preset);
    if let Some(g) = &glb_path {
        manifest["base_model_glb"] = json!(g);
    }
    write_json_pretty(&manifest_path, &manifest)?;
    Ok(json!({
        "ok": true,
        "character": id,
        "clip": clip_doc,
        "clip_file": clip_file,
        "manifest": manifest_path,
    }))
}

/// `character.open` helper for `app="blender"`: open the character's `.blend` file in Blender.
pub(crate) fn open_in_blender(s: &Session, id: &str) -> Result<Value> {
    let project_dir = crate::workshop::project_dir(s)?;
    let info = inspect_character_3d(&project_dir, id);
    let blend = info["blend_files"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| bad(format!("character '{id}' has no .blend scene yet")))?;
    let exe = crate::workspace::resolve_tool("blender");
    std::process::Command::new(&exe)
        .arg(&blend)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| EngineError::ExternalTool("blender".into(), e.to_string()))?;
    Ok(json!({"ok": true, "app": "blender", "file": blend}))
}
