//! Inflated silhouette mesh approximation and GLB/OBJ serialization.

use std::path::PathBuf;

use image::RgbaImage;
use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

use crate::magic_design::{cmd_asset_load_manifest, decontaminate_sprite_rgba};

/// The shape settings this call asked for, each clamped to its usable range.
struct MeshParams {
    inflation_depth_pct: u32,
    relief_detail_pct: u32,
    mesh_res: u32,
    proj_blend_pct: u32,
    bump_intensity: u32,
}

impl MeshParams {
    fn from_params(params: &Value) -> Self {
        let clamp = |key: &str, low: u64, high: u64, default: u32| {
            params
                .get(key)
                .and_then(Value::as_u64)
                .map(|v| v.clamp(low, high) as u32)
                .unwrap_or(default)
        };
        Self {
            inflation_depth_pct: clamp("inflation_depth_pct", 10, 100, 58),
            relief_detail_pct: clamp("relief_detail_pct", 0, 100, 44),
            mesh_res: clamp("mesh_res", 12, 64, 32),
            proj_blend_pct: clamp("proj_blend_pct", 0, 100, 65),
            bump_intensity: clamp("bump_intensity", 0, 100, 35),
        }
    }

    fn settings(&self) -> crate::mesh_geometry::ShapeSettings {
        crate::mesh_geometry::ShapeSettings {
            inflation_depth_pct: self.inflation_depth_pct,
            relief_detail_pct: self.relief_detail_pct,
            mesh_res: self.mesh_res,
            proj_blend_pct: self.proj_blend_pct,
            bump_intensity: self.bump_intensity,
        }
    }
}

/// The asset this call names (or the active one), loaded first when it is only a preset.
fn resolved_asset(
    s: &mut Session,
    params: &Value,
) -> Result<(String, crate::session::Asset)> {
    let preset = params
        .get("preset")
        .or_else(|| params.get("character"))
        .or_else(|| params.get("asset"))
        .and_then(Value::as_str)
        .map(str::to_string);
    if preset.is_none() && s.active_asset.is_none() {
        return Err(EngineError::NotImplemented);
    }
    if let Some(ref preset) = preset
        && !s.assets.contains_key(preset)
    {
        let _ = cmd_asset_load_manifest(s, &json!({ "preset": preset }));
    }
    let asset_id = preset
        .or_else(|| s.active_asset.clone())
        .ok_or(EngineError::NotImplemented)?;
    let asset = s
        .assets
        .get(&asset_id)
        .cloned()
        .ok_or_else(|| EngineError::InvalidParams(format!("asset `{asset_id}` not loaded")))?;
    Ok((asset_id, asset))
}

/// Where the decontaminated views, poses and ingest receipt for one asset live.
struct IngestDirs {
    turnaround: PathBuf,
    poses: PathBuf,
    ingest: PathBuf,
}

fn ingest_dirs(s: &Session, asset_id: &str) -> Result<IngestDirs> {
    let root = crate::workshop::project_dir(s)?;
    let ingest = root.join("exports/blender_ingest").join(asset_id);
    let dirs = IngestDirs {
        turnaround: ingest.join("turnaround"),
        poses: ingest.join("poses"),
        ingest,
    };
    std::fs::create_dir_all(&dirs.turnaround)?;
    std::fs::create_dir_all(&dirs.poses)?;
    Ok(dirs)
}

fn frame_out(dir: &std::path::Path, index: usize, label: &str) -> PathBuf {
    dir.join(format!(
        "{index:02}_{}.png",
        super::workspace::sanitize_asset_id(label)
    ))
}

fn save_clean(clean: &RgbaImage, out: &std::path::Path) -> Result<()> {
    clean
        .save(out)
        .map_err(|e| EngineError::Io(std::io::Error::other(e.to_string())))
}

/// Decontaminate every turnaround frame into the ingest folder.
fn clean_turnaround(
    asset: &crate::session::Asset,
    out_dir: &std::path::Path,
) -> Result<Vec<(String, PathBuf, RgbaImage)>> {
    let mut imgs = Vec::new();
    let Some(tm) = &asset.turnaround_manifest else {
        return Ok(imgs);
    };
    for frame in &tm.frames {
        let Ok(dyn_img) = image::open(&frame.resolved_path) else {
            continue;
        };
        let (clean, _bbox) = decontaminate_sprite_rgba(&dyn_img.into_rgba8(), None);
        let out = frame_out(out_dir, frame.index, &frame.label);
        save_clean(&clean, &out)?;
        imgs.push((frame.label.clone(), out, clean));
    }
    Ok(imgs)
}

/// Decontaminate every pose frame, standing in as the only view when no turnaround exists.
fn clean_poses(
    asset: &crate::session::Asset,
    out_dir: &std::path::Path,
    target_ref_height: Option<u32>,
    turnaround: &mut Vec<(String, PathBuf, RgbaImage)>,
) -> Result<Vec<Value>> {
    let mut files = Vec::new();
    let Some(pm) = &asset.pose_manifest else {
        return Ok(files);
    };
    for frame in &pm.frames {
        let Ok(dyn_img) = image::open(&frame.resolved_path) else {
            continue;
        };
        let (clean, bbox) = decontaminate_sprite_rgba(&dyn_img.into_rgba8(), target_ref_height);
        let out = frame_out(out_dir, frame.index, &frame.label);
        save_clean(&clean, &out)?;
        if turnaround.is_empty() {
            turnaround.push((frame.label.clone(), out.clone(), clean.clone()));
        }
        files.push(json!({
            "index": frame.index,
            "label": frame.label,
            "path": out.to_string_lossy(),
            "size": [clean.width(), clean.height()],
            "bbox": bbox,
        }));
    }
    Ok(files)
}

/// The asset's own reference image, when nothing else can stand in for a front view.
fn reference_view(
    asset: &crate::session::Asset,
    out_dir: &std::path::Path,
) -> Result<Vec<(String, PathBuf, RgbaImage)>> {
    if !asset.path.is_file() {
        return Ok(Vec::new());
    }
    let Ok(dyn_img) = image::open(&asset.path) else {
        return Ok(Vec::new());
    };
    let (clean, _) = decontaminate_sprite_rgba(&dyn_img.into_rgba8(), None);
    let out = out_dir.join("00_front.png");
    save_clean(&clean, &out)?;
    Ok(vec![("front".to_string(), out, clean)])
}

/// The views that carry readable foreground artwork, in the order they were collected.
fn mesh_views(imgs: &[(String, PathBuf, RgbaImage)]) -> Result<Vec<(String, RgbaImage)>> {
    let views: Vec<_> = imgs
        .iter()
        .filter(|(_, _, image)| image.pixels().any(|pixel| pixel[3] >= 96))
        .map(|(label, _, img)| (label.clone(), img.clone()))
        .collect();
    if views.is_empty() {
        return Err(EngineError::InvalidParams(
            "No readable foreground artwork is available to build a mesh".into(),
        ));
    }
    Ok(views)
}

fn turnaround_entries(imgs: &[(String, PathBuf, RgbaImage)]) -> Vec<Value> {
    imgs.iter()
        .enumerate()
        .map(|(idx, (lbl, path, img))| {
            json!({
                "index": idx,
                "label": lbl,
                "path": path.to_string_lossy(),
                "size": [img.width(), img.height()],
            })
        })
        .collect()
}

/// The exported files for this asset, under the ingest folder unless the call names a path.
struct MeshFiles {
    glb: PathBuf,
    gltf: PathBuf,
    obj: PathBuf,
    mtl: PathBuf,
    ingest: PathBuf,
}

fn mesh_files(s: &Session, params: &Value, dirs: &IngestDirs, asset_id: &str) -> MeshFiles {
    let default_glb = dirs.ingest.join(format!("{asset_id}_4dir.glb"));
    let glb = params
        .get("output")
        .or_else(|| params.get("out_glb"))
        .and_then(Value::as_str)
        .map(|p| super::workspace::resolve_workspace_path(s, p))
        .unwrap_or(default_glb);
    MeshFiles {
        gltf: glb.with_extension("gltf"),
        obj: glb.with_extension("obj"),
        mtl: glb.with_extension("mtl"),
        ingest: dirs.ingest.join(format!("{asset_id}_blender_ingest.json")),
        glb,
    }
}

fn measured_telemetry(asset: &crate::session::Asset) -> (Option<f64>, Option<f64>) {
    let foot_lift = asset
        .motion_range
        .as_ref()
        .and_then(|m| m.raw["limits"]["footLiftPx"]["max"].as_f64());
    let drift = asset
        .turnaround_registration
        .as_ref()
        .and_then(|r| r.raw["maxVerticalDriftPx"].as_f64());
    (foot_lift, drift)
}

fn ingest_payload(
    asset: &crate::session::Asset,
    asset_id: &str,
    shape: &MeshParams,
    files: &MeshFiles,
    entries: &[Value],
    poses: &[Value],
    target_ref_height: Option<u32>,
    telemetry: (Option<f64>, Option<f64>),
    mesh_counts: [usize; 2],
) -> Value {
    let (foot_lift_max_px, max_vertical_drift_px) = telemetry;
    json!({
        "ok": true,
        "character": asset_id,
        "sources": {
            "reference": asset.path,
            "turnaround_manifest": asset.turnaround_manifest.as_ref().map(|m| &m.manifest_path),
            "pose_manifest": asset.pose_manifest.as_ref().map(|m| &m.manifest_path),
            "registration": asset.turnaround_registration.as_ref().map(|r| &r.source_file),
            "motion_range": asset.motion_range.as_ref().map(|r| &r.source_file),
        },
        "recon_mode": RECON_MODE,
        "reconstruction_method": "inflated_row_spans",
        "rigged": false,
        "reconstruction_params": {
            "inflation_depth_pct": shape.inflation_depth_pct,
            "relief_detail_pct": shape.relief_detail_pct,
            "mesh_res": shape.mesh_res,
            "proj_blend_pct": shape.proj_blend_pct,
            "bump_intensity": shape.bump_intensity,
        },
        "mesh": {
            "glb": files.glb.to_string_lossy(),
            "gltf": files.gltf.to_string_lossy(),
            "obj": files.obj.to_string_lossy(),
            "mtl": files.mtl.to_string_lossy(),
            "vertex_count": mesh_counts[0],
            "triangle_count": mesh_counts[1],
            "ground_plane_min_y": 0.0,
        },
        "joint_landmarks_ratio": null,
        "rigging_status": "requires_authored_joints_and_binding",
        "measured_telemetry": {
            "foot_lift_max_px": foot_lift_max_px,
            "max_vertical_drift_px": max_vertical_drift_px,
            "target_reference_height_px": target_ref_height,
        },
        "decontaminated_turnaround_views": entries,
        "decontaminated_pose_frames": poses,
        "parts": asset.parts,
    })
}

fn result_json(
    asset_id: &str,
    files: &MeshFiles,
    mesh: &crate::mesh_geometry::MeshGeometry,
    telemetry: (Option<f64>, Option<f64>),
    turnaround_count: usize,
    pose_count: usize,
) -> Value {
    let (foot_lift_max_px, max_vertical_drift_px) = telemetry;
    json!({
        "ok": true,
        "command": "mesh.build_4dir",
        "asset_id": asset_id,
        "recon_mode": RECON_MODE,
        "reconstruction_method": "inflated_row_spans",
        "rigged": false,
        "glb": files.glb.to_string_lossy(),
        "gltf": files.gltf.to_string_lossy(),
        "obj": files.obj.to_string_lossy(),
        "mtl": files.mtl.to_string_lossy(),
        "blender_ingest_json": files.ingest.to_string_lossy(),
        "vertex_count": mesh.positions.len(),
        "triangle_count": mesh.indices.len() / 3,
        "decontaminated_turnaround_count": turnaround_count,
        "decontaminated_pose_count": pose_count,
        "foot_lift_max_px": foot_lift_max_px,
        "max_vertical_drift_px": max_vertical_drift_px,
    })
}

const RECON_MODE: &str = "inflated_row_spans";

/// Run `mesh.build_4dir`: prepare silhouette views and export an unrigged row-span approximation.
pub fn cmd_mesh_build_4dir(s: &mut Session, params: &Value) -> Result<Value> {
    let (asset_id, asset) = resolved_asset(s, params)?;
    let shape = MeshParams::from_params(params);
    let dirs = ingest_dirs(s, &asset_id)?;
    let target_ref_height: Option<u32> = asset
        .turnaround_registration
        .as_ref()
        .map(|r| r.target_height_px)
        .filter(|&h| h > 0);
    let mut turnaround = clean_turnaround(&asset, &dirs.turnaround)?;
    let poses = clean_poses(&asset, &dirs.poses, target_ref_height, &mut turnaround)?;
    if turnaround.is_empty() {
        turnaround = reference_view(&asset, &dirs.turnaround)?;
    }
    let views = mesh_views(&turnaround)?;
    let mesh = crate::mesh_geometry::build(&views, shape.settings());
    let files = mesh_files(s, params, &dirs, &asset_id);
    write_binary_glb(&files.glb, &mesh.positions, &mesh.normals, &mesh.uvs, &mesh.colors, &mesh.indices)?;
    write_wavefront_obj(&files.obj, &mesh.positions, &mesh.normals, &mesh.uvs, &mesh.indices)?;
    let entries = turnaround_entries(&turnaround);
    let telemetry = measured_telemetry(&asset);
    let payload = ingest_payload(
        &asset,
        &asset_id,
        &shape,
        &files,
        &entries,
        &poses,
        target_ref_height,
        telemetry,
        [mesh.positions.len(), mesh.indices.len() / 3],
    );
    std::fs::write(
        &files.ingest,
        serde_json::to_string_pretty(&payload).unwrap_or_default(),
    )?;
    Ok(result_json(
        &asset_id,
        &files,
        &mesh,
        telemetry,
        entries.len(),
        poses.len(),
    ))
}

/// Byte offset and length of one buffer view.
struct Span {
    offset: usize,
    len: usize,
}

/// The packed vertex streams, where each one sits in the buffer, and the position bounds.
struct Geometry {
    bin: Vec<u8>,
    spans: [Span; 5],
    min: [f32; 3],
    max: [f32; 3],
}

fn append_f32_array<const N: usize>(bin: &mut Vec<u8>, rows: &[[f32; N]]) -> Span {
    let offset = bin.len();
    for row in rows {
        for value in row {
            bin.extend_from_slice(&value.to_le_bytes());
        }
    }
    Span {
        offset,
        len: bin.len() - offset,
    }
}

fn append_u32s(bin: &mut Vec<u8>, values: &[u32]) -> Span {
    let offset = bin.len();
    for value in values {
        bin.extend_from_slice(&value.to_le_bytes());
    }
    Span {
        offset,
        len: bin.len() - offset,
    }
}

fn position_bounds(positions: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for p in positions {
        for i in 0..3 {
            min[i] = min[i].min(p[i]);
            max[i] = max[i].max(p[i]);
        }
    }
    (min, max)
}

fn geometry(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    uvs: &[[f32; 2]],
    colors: &[[f32; 4]],
    indices: &[u32],
) -> Geometry {
    let mut bin: Vec<u8> = Vec::new();
    let (min, max) = position_bounds(positions);
    let spans = [
        append_f32_array(&mut bin, positions),
        append_f32_array(&mut bin, normals),
        append_f32_array(&mut bin, uvs),
        append_f32_array(&mut bin, colors),
        append_u32s(&mut bin, indices),
    ];
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    Geometry {
        bin,
        spans,
        min,
        max,
    }
}

fn buffer_view(spans: &[Span; 5], index: usize, target: u32) -> Value {
    json!({
        "buffer": 0,
        "byteOffset": spans[index].offset,
        "byteLength": spans[index].len,
        "target": target,
    })
}

fn gltf_document(geometry: &Geometry, counts: [usize; 5]) -> Value {
    json!({
        "asset": {
            "version": "2.0",
            "generator": "Magic Studio Rust 4-Dir Visual Hull Builder"
        },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "name": "CharacterVisualHull", "mesh": 0 }],
        "meshes": [{
            "name": "CharacterVisualHullMesh",
            "primitives": [{
                "attributes": {
                    "POSITION": 0,
                    "NORMAL": 1,
                    "TEXCOORD_0": 2,
                    "COLOR_0": 3
                },
                "indices": 4,
                "mode": 4
            }]
        }],
        "accessors": [
            {
                "bufferView": 0,
                "componentType": 5126,
                "count": counts[0],
                "type": "VEC3",
                "min": geometry.min,
                "max": geometry.max
            },
            {
                "bufferView": 1,
                "componentType": 5126,
                "count": counts[1],
                "type": "VEC3"
            },
            {
                "bufferView": 2,
                "componentType": 5126,
                "count": counts[2],
                "type": "VEC2"
            },
            {
                "bufferView": 3,
                "componentType": 5126,
                "count": counts[3],
                "type": "VEC4"
            },
            {
                "bufferView": 4,
                "componentType": 5125,
                "count": counts[4],
                "type": "SCALAR"
            }
        ],
        "bufferViews": [
            buffer_view(&geometry.spans, 0, 34962),
            buffer_view(&geometry.spans, 1, 34962),
            buffer_view(&geometry.spans, 2, 34962),
            buffer_view(&geometry.spans, 3, 34962),
            buffer_view(&geometry.spans, 4, 34963)
        ],
        "buffers": [{ "byteLength": geometry.bin.len() }]
    })
}

/// The JSON chunk of a GLB, padded with spaces to a four-byte boundary.
fn padded_json(gltf: &Value) -> Result<Vec<u8>> {
    let mut json_bytes = serde_json::to_vec(gltf)
        .map_err(|e| EngineError::InvalidParams(format!("glTF JSON serialization failed: {e}")))?;
    while !json_bytes.len().is_multiple_of(4) {
        json_bytes.push(b' ');
    }
    Ok(json_bytes)
}

/// The 12-byte GLB header followed by the JSON and BIN chunks.
fn glb_container(json_bytes: &[u8], bin: &[u8]) -> Vec<u8> {
    let total_len = 12 + 8 + json_bytes.len() + 8 + bin.len();
    let mut glb: Vec<u8> = Vec::with_capacity(total_len);
    glb.extend_from_slice(&0x46546C67u32.to_le_bytes()); // "glTF"
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&(total_len as u32).to_le_bytes());

    glb.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // "JSON"
    glb.extend_from_slice(json_bytes);

    glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x004E4942u32.to_le_bytes()); // "BIN\0"
    glb.extend_from_slice(bin);
    glb
}

/// The same document as `.gltf`, with the buffer inlined as a base64 data URI.
fn ascii_document(mut gltf: Value, bin: &[u8]) -> Result<String> {
    if let Some(buffers) = gltf.get_mut("buffers").and_then(Value::as_array_mut)
        && let Some(first_buf) = buffers.first_mut().and_then(Value::as_object_mut)
    {
        first_buf.insert(
            "uri".to_string(),
            json!(format!(
                "data:application/octet-stream;base64,{}",
                encode_base64(bin)
            )),
        );
    }
    serde_json::to_string_pretty(&gltf)
        .map_err(|e| EngineError::InvalidParams(format!("glTF ASCII serialization failed: {e}")))
}

/// Write a binary glTF 2.0 (`.glb`) plus an embedded-JSON `.gltf` sidecar.
fn write_binary_glb(
    path: &Path,
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    uvs: &[[f32; 2]],
    colors: &[[f32; 4]],
    indices: &[u32],
) -> Result<()> {
    let geometry = geometry(positions, normals, uvs, colors, indices);
    let gltf = gltf_document(
        &geometry,
        [
            positions.len(),
            normals.len(),
            uvs.len(),
            colors.len(),
            indices.len(),
        ],
    );
    let json_bytes = padded_json(&gltf)?;
    let glb = glb_container(&json_bytes, &geometry.bin);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, glb)?;
    std::fs::write(path.with_extension("gltf"), ascii_document(gltf, &geometry.bin)?)?;
    Ok(())
}

fn encode_base64(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Write a Wavefront `.obj` + `.mtl` sidecar.
fn write_wavefront_obj(
    path: &Path,
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    uvs: &[[f32; 2]],
    indices: &[u32],
) -> Result<()> {
    let mtl_path = path.with_extension("mtl");
    let mtl_filename = mtl_path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| "character.mtl".to_string());
    let mut out = format!(
        "# Magic Studio 4-Dir Visual Hull OBJ\nmtllib {mtl_filename}\no CharacterVisualHull\nusemtl MAT_Character_Felt\n"
    );
    for p in positions {
        out.push_str(&format!("v {:.5} {:.5} {:.5}\n", p[0], p[1], p[2]));
    }
    for uv in uvs {
        out.push_str(&format!("vt {:.5} {:.5}\n", uv[0], uv[1]));
    }
    for n in normals {
        out.push_str(&format!("vn {:.5} {:.5} {:.5}\n", n[0], n[1], n[2]));
    }
    for tri in indices.as_chunks::<3>().0 {
        let i0 = tri[0] + 1;
        let i1 = tri[1] + 1;
        let i2 = tri[2] + 1;
        out.push_str(&format!("f {i0}/{i0}/{i0} {i1}/{i1}/{i1} {i2}/{i2}/{i2}\n"));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, out)?;
    let mtl_content = "# Magic Studio Wavefront Material Library (.MTL)\nnewmtl MAT_Character_Felt\nKd 0.9451 0.8745 0.7804\nKs 0.0800 0.0800 0.0800\nNs 30.00\nd 1.0\n";
    std::fs::write(mtl_path, mtl_content)?;
    Ok(())
}

use std::path::Path;
