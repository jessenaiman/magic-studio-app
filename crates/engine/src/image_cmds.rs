//! Photocraft image-breaking commands: break apart into polygonal parts, puppet warp,
//! color-to-alpha, and content-aware fill. Each resolves the active asset, shells out to
//! `photocraft-cli`, and writes a derived PNG.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{EngineError, Result, Session};
pub(crate) mod medium;
pub use medium::cmd_image_apply_medium;

use crate::workspace::{
    image_batch_args, resolve_tool, resolve_workspace_path, run_tool_in, workspace_root,
};

fn register_pair(value: &Value) -> Result<[f64; 2]> {
    let a = value.as_array().filter(|a| a.len() == 2).ok_or_else(|| {
        EngineError::InvalidParams("Pivots and canvas must be two-number arrays".into())
    })?;
    let number = |v: &Value| {
        v.as_f64()
            .filter(|v| v.is_finite())
            .ok_or_else(|| EngineError::InvalidParams("Coordinates must be finite numbers".into()))
    };
    Ok([number(&a[0])?, number(&a[1])?])
}

fn register_paths(
    s: &Session,
    p: &Value,
) -> Result<(std::path::PathBuf, std::path::PathBuf, std::path::PathBuf)> {
    let source = resolve_workspace_path(
        s,
        p["source"]
            .as_str()
            .ok_or_else(|| EngineError::InvalidParams("source image is required".into()))?,
    );
    let out = resolve_workspace_path(
        s,
        p["out"].as_str().ok_or_else(|| {
            EngineError::InvalidParams("out is required; originals are never overwritten".into())
        })?,
    );
    if !out
        .extension()
        .and_then(|v| v.to_str())
        .is_some_and(|v| v.eq_ignore_ascii_case("png"))
    {
        return Err(EngineError::InvalidParams(
            "out must be PNG to preserve pose transparency".into(),
        ));
    }
    let receipt = out.with_extension("registration.json");
    if out.exists() || receipt.exists() {
        return Err(EngineError::InvalidParams(
            "Registration output already exists; choose a new version".into(),
        ));
    }
    Ok((source, out, receipt))
}

fn register_canvas(value: &Value) -> Result<[f64; 2]> {
    let canvas = register_pair(value)?;
    if canvas
        .iter()
        .any(|v| v.fract() != 0.0 || !(16.0..=8192.0).contains(v))
        || canvas[0] * canvas[1] > 16_777_216.0
    {
        return Err(EngineError::InvalidParams(
            "canvas must use integer dimensions 16..8192 and at most 16777216 pixels".into(),
        ));
    }
    Ok(canvas)
}

fn register_scale(p: &Value) -> Result<f64> {
    Ok(p.get("scale")
        .map(|v| {
            v.as_f64()
                .filter(|v| v.is_finite() && *v > 0.0 && *v <= 16.0)
                .ok_or_else(|| EngineError::InvalidParams("scale must be in (0,16]".into()))
        })
        .transpose()?
        .unwrap_or(1.0))
}

fn visible_bounds(w: u32, h: u32, pixels: &image::RgbaImage) -> Result<[u32; 4]> {
    let mut bounds = [w, h, 0, 0];
    for (x, y, _) in pixels.enumerate_pixels().filter(|(_, _, p)| p[3] > 0) {
        bounds = [
            bounds[0].min(x),
            bounds[1].min(y),
            bounds[2].max(x + 1),
            bounds[3].max(y + 1),
        ];
    }
    if bounds[2] == 0 {
        return Err(EngineError::InvalidParams(
            "Source has no visible pixels".into(),
        ));
    }
    Ok(bounds)
}

fn register_placed(
    bounds: [u32; 4],
    from: [f64; 2],
    to: [f64; 2],
    scale: f64,
    canvas: [f64; 2],
) -> Result<[f64; 4]> {
    let placed = [
        to[0] + (bounds[0] as f64 - from[0]) * scale,
        to[1] + (bounds[1] as f64 - from[1]) * scale,
        to[0] + (bounds[2] as f64 - from[0]) * scale,
        to[1] + (bounds[3] as f64 - from[1]) * scale,
    ];
    if placed[0] < 0.0 || placed[1] < 0.0 || placed[2] > canvas[0] || placed[3] > canvas[1] {
        return Err(EngineError::InvalidParams(
            "Registered silhouette would be clipped; enlarge canvas or adjust target_pivot/scale"
                .into(),
        ));
    }
    Ok(placed)
}

#[allow(clippy::too_many_arguments)]
fn run_registration(
    s: &Session,
    source: &std::path::Path,
    out: &std::path::Path,
    receipt: &std::path::Path,
    canvas: [f64; 2],
    from: [f64; 2],
    to: [f64; 2],
    scale: f64,
    w: u32,
    h: u32,
    bounds: [u32; 4],
    placed: [f64; 4],
) -> Result<Value> {
    let center = [
        to[0] + (w as f64 / 2.0 - from[0]) * scale,
        to[1] + (h as f64 / 2.0 - from[1]) * scale,
    ];
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let root = workspace_root(s);
    run_tool_in(
        &root,
        "photocraft-cli",
        &[
            "run".into(),
            "--new".into(),
            json!({"width":canvas[0] as u32,"height":canvas[1] as u32,"background":"transparent"})
                .to_string(),
            "--cmd".into(),
            "file.placeEmbedded".into(),
            "--params".into(),
            json!({"path":source,"scale":scale*100.0,"fit":false,"center":center}).to_string(),
            "--out".into(),
            out.to_string_lossy().into_owned(),
        ],
    )?;
    let decoded = image::open(out)?;
    if [decoded.width(), decoded.height()] != [canvas[0] as u32, canvas[1] as u32]
        || !decoded.color().has_alpha()
    {
        return Err(EngineError::InvalidParams(
            "PhotoCraft output does not match the requested transparent canvas; inspect retained output".into(),
        ));
    }
    let result = json!({"ok":true,"source":source,"output":out,"canvas":canvas,"source_pivot":from,
        "target_pivot":to,"scale":scale,"source_bounds":bounds,"placed_bounds":placed,
        "source_sha256":format!("{:x}",Sha256::digest(std::fs::read(source)?)),"receipt":receipt});
    std::fs::write(
        receipt,
        serde_json::to_vec_pretty(&result)
            .map_err(|e| EngineError::InvalidParams(e.to_string()))?,
    )?;
    Ok(result)
}

/// Register a reviewed pose onto a shared canvas without distorting or clipping its silhouette.
pub fn cmd_image_register_frame(s: &mut Session, p: &Value) -> Result<Value> {
    let (source, out, receipt) = register_paths(s, p)?;
    let canvas = register_canvas(&p["canvas"])?;
    let from = register_pair(&p["source_pivot"])?;
    let to = if p.get("target_pivot").is_some() {
        register_pair(&p["target_pivot"])?
    } else {
        [canvas[0] / 2.0, canvas[1] / 2.0]
    };
    let scale = register_scale(p)?;
    let (w, h) = image::image_dimensions(&source)?;
    if u64::from(w) * u64::from(h) > 16_777_216 {
        return Err(EngineError::InvalidParams(
            "Source exceeds 16777216 pixels".into(),
        ));
    }
    let pixels = image::open(&source)?.to_rgba8();
    let bounds = visible_bounds(w, h, &pixels)?;
    let placed = register_placed(bounds, from, to, scale, canvas)?;
    run_registration(
        s, &source, &out, &receipt, canvas, from, to, scale, w, h, bounds, placed,
    )
}

/// Resolve the target asset ID and validated source image path for an `image.*` command.
fn resolve_asset_source(
    s: &Session,
    params: &Value,
    verb: &str,
) -> Result<(String, std::path::PathBuf)> {
    let asset_id = params
        .get("asset")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| s.active_asset.clone())
        .ok_or_else(|| EngineError::InvalidParams(format!("no active asset to {verb}")))?;

    let existing_asset = s.assets.get(&asset_id).cloned().ok_or_else(|| {
        EngineError::InvalidParams(format!("asset `{asset_id}` is not loaded in session"))
    })?;

    let source_path = if let Some(src_override) = params.get("source").and_then(Value::as_str) {
        resolve_workspace_path(s, src_override)
    } else {
        existing_asset.path.clone()
    };

    if !source_path.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "source image for asset `{asset_id}` does not exist: {}",
            source_path.display()
        )));
    }
    Ok((asset_id, source_path))
}

fn break_apart_regions(params: &Value) -> Result<&serde_json::Map<String, Value>> {
    // Each part is the source masked by an explicit polygon from `regions`
    // (part id -> polygon points). Without regions every "part" would be a copy of
    // the whole image, so regions are required and any failed step fails that part.
    params
        .get("regions")
        .and_then(Value::as_object)
        .filter(|r| !r.is_empty())
        .ok_or_else(|| {
            EngineError::InvalidParams(
                "image.break_apart requires `regions`: a map of part id to a polygon, or {polygons, keep}".into(),
            )
        })
}

fn break_apart_part(
    root: &std::path::Path,
    bin: &str,
    source: &std::path::Path,
    out_dir: &std::path::Path,
    part_id: &str,
    region: &Value,
) -> std::result::Result<Value, String> {
    let part_out = out_dir.join(format!("{part_id}.png"));
    // One PhotoCraft session per part. A region is a polygon ([[x, y], ...]) or an object
    // {"polygons": [polygon, ...], "keep": "inside" | "outside"}: select the polygon(s), then clear
    // everything else (inside) or the selection itself (outside). The source is already matted at
    // intake, so no background removal happens here.
    let (polygons, keep_outside): (Vec<Value>, bool) = match region {
        Value::Object(obj) => (
            obj.get("polygons")
                .and_then(Value::as_array)
                .cloned()
                .or_else(|| obj.get("points").map(|p| vec![p.clone()]))
                .unwrap_or_default(),
            obj.get("keep").and_then(Value::as_str) == Some("outside"),
        ),
        other => (vec![other.clone()], false),
    };
    if polygons.is_empty() {
        return Err(format!("{part_id}: region has no polygon"));
    }
    let mut args = vec!["run".to_string(), source.to_string_lossy().into_owned()];
    for (i, polygon) in polygons.iter().enumerate() {
        args.extend([
            "--cmd".to_string(),
            "select.lasso".to_string(),
            "--params".to_string(),
            json!({"points": polygon, "mode": if i == 0 { "replace" } else { "add" }}).to_string(),
        ]);
    }
    if !keep_outside {
        args.extend(["--cmd".to_string(), "select.inverse".to_string()]);
    }
    args.extend([
        "--cmd".to_string(),
        "edit.clear".to_string(),
        "--out".to_string(),
        part_out.to_string_lossy().into_owned(),
    ]);
    if let Err(e) = run_tool_in(root, bin, &args) {
        return Err(format!("{part_id}: {e}"));
    }
    if part_out.is_file() {
        let (w, h) = image::image_dimensions(&part_out).unwrap_or((0, 0));
        Ok(json!({
            "part": part_id,
            "label": part_id,
            "file": part_out.to_string_lossy(),
            "width": w,
            "height": h,
        }))
    } else {
        Err(format!("{part_id}: no output file written"))
    }
}

/// Mask the active asset's source into per-part PNGs from explicit `regions` polygons.
pub fn cmd_image_break_apart(s: &mut Session, params: &Value) -> Result<Value> {
    let (asset_id, source_path) = resolve_asset_source(s, params, "break apart")?;

    let root = workspace_root(s);
    let out_dir = if let Some(dir_str) = params.get("out_dir").and_then(Value::as_str) {
        resolve_workspace_path(s, dir_str)
    } else {
        root.join("exports/pose_and_parts")
            .join(format!("{asset_id}_broken_apart"))
    };
    std::fs::create_dir_all(&out_dir)?;

    let regions = break_apart_regions(params)?;
    let parts: Vec<String> = regions.keys().cloned().collect();

    let bin = resolve_tool("photocraft-cli");

    let mut produced_parts: Vec<Value> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for part_id in &parts {
        match break_apart_part(
            &root,
            &bin,
            &source_path,
            &out_dir,
            part_id,
            &regions[part_id.as_str()],
        ) {
            Ok(row) => produced_parts.push(row),
            Err(error) => errors.push(error),
        }
    }

    // `rig_order` (part ids, back to front) also writes the parts' layered rig, `rig.json`, for
    // project.add_asset kind=cutout_rig: the largest part is the root, the others ride on it, and each
    // pivot starts at its part's centre (refined later in the Animation view). It carries a gentle
    // "rest" track for motion.path to draw on.
    let rig = match params.get("rig_order").and_then(Value::as_array) {
        Some(order) if errors.is_empty() => {
            Some(write_parts_rig(&out_dir, &asset_id, order, None)?)
        }
        Some(_) => {
            errors.push("rig.json not written: some parts failed".into());
            None
        }
        None => None,
    };

    Ok(json!({
        "ok": !produced_parts.is_empty(),
        "command": "image.break_apart",
        "asset_id": asset_id,
        "source": source_path.to_string_lossy(),
        "out_dir": out_dir.to_string_lossy(),
        "parts_produced": produced_parts.len(),
        "parts": produced_parts,
        "rig": rig,
        "errors": errors,
    }))
}

/// `joints` (rig.from_sheet's placed parts: id, parent, pivot) replaces the centre pivots and the
/// largest-part root with the placed joints and parents, and seeds initial timed 2D tracks
/// (`rest`, `walk`, `wave`, `jump`) so the cutout rig immediately has fluid multi-part motion.
type RigLayer = (String, std::path::PathBuf, [f64; 2], u64);

fn rig_layers(out_dir: &std::path::Path, order: &[Value]) -> Result<(Vec<RigLayer>, (u32, u32))> {
    let mut layers = Vec::new();
    let mut canvas = (0, 0);
    for id in order {
        let id = id
            .as_str()
            .ok_or_else(|| EngineError::InvalidParams("rig_order holds part ids".into()))?;
        let file = out_dir.join(format!("{id}.png"));
        let image = image::open(&file)
            .map_err(|e| EngineError::InvalidParams(format!("rig_order: {id}: {e}")))?
            .into_rgba8();
        canvas = image.dimensions();
        let (mut min, mut max, mut area) = ((u32::MAX, u32::MAX), (0, 0), 0u64);
        for (x, y, _) in image.enumerate_pixels().filter(|(_, _, p)| p[3] > 16) {
            min = (min.0.min(x), min.1.min(y));
            max = (max.0.max(x), max.1.max(y));
            area += 1;
        }
        if area == 0 {
            return Err(EngineError::InvalidParams(format!(
                "rig_order: part {id} is empty"
            )));
        }
        layers.push((
            id.to_string(),
            file,
            [(min.0 + max.0) as f64 / 2.0, (min.1 + max.1) as f64 / 2.0],
            area,
        ));
    }
    Ok((layers, canvas))
}

fn rig_root(joints: Option<&[Value]>, layers: &[RigLayer]) -> String {
    joints
        .and_then(|j| {
            j.iter()
                .find(|p| p["parent"].is_null())
                .and_then(|p| p["id"].as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| {
            layers
                .iter()
                .max_by_key(|l| l.3)
                .map(|l| l.0.clone())
                .unwrap_or_default()
        })
}

fn seed_root_motion(
    id: &str,
    rest_motion: &mut serde_json::Map<String, Value>,
    walk_motion: &mut serde_json::Map<String, Value>,
    wave_motion: &mut serde_json::Map<String, Value>,
    jump_motion: &mut serde_json::Map<String, Value>,
) {
    rest_motion.insert(id.to_string(), json!({
        "position": [{"kind": "oscillate", "axis": 1, "amplitude": 4.0, "hz": 0.5, "phase": 0.0}],
        "rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 1.2, "hz": 0.5, "phase": 0.25}]
    }));
    walk_motion.insert(id.to_string(), json!({
        "position": [{"kind": "oscillate", "axis": 1, "amplitude": 6.0, "hz": 2.0, "phase": 0.0}],
        "rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 2.5, "hz": 1.0, "phase": 0.0}]
    }));
    wave_motion.insert(id.to_string(), json!({
        "position": [{"kind": "oscillate", "axis": 1, "amplitude": 3.0, "hz": 0.75, "phase": 0.0}],
        "rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 1.8, "hz": 0.75, "phase": 0.15}]
    }));
    jump_motion.insert(id.to_string(), json!({
        "position": [{"kind": "oscillate", "axis": 1, "amplitude": 22.0, "hz": 1.0, "phase": 0.0}],
        "rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 3.5, "hz": 1.0, "phase": 0.2}]
    }));
}

fn seed_head_motion(
    id: &str,
    rest_motion: &mut serde_json::Map<String, Value>,
    walk_motion: &mut serde_json::Map<String, Value>,
    wave_motion: &mut serde_json::Map<String, Value>,
    jump_motion: &mut serde_json::Map<String, Value>,
) {
    rest_motion.insert(id.to_string(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 1.8, "hz": 0.5, "phase": 0.15}]}));
    walk_motion.insert(id.to_string(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 3.0, "hz": 1.0, "phase": 0.2}]}));
    wave_motion.insert(id.to_string(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 4.5, "hz": 1.2, "phase": 0.1}]}));
    jump_motion.insert(id.to_string(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 5.0, "hz": 1.0, "phase": 0.25}]}));
}

fn seed_secondary_motion(
    id: &str,
    phase: f64,
    rest_motion: &mut serde_json::Map<String, Value>,
    walk_motion: &mut serde_json::Map<String, Value>,
    wave_motion: &mut serde_json::Map<String, Value>,
    jump_motion: &mut serde_json::Map<String, Value>,
) {
    rest_motion.insert(id.to_string(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 3.5, "hz": 0.5, "phase": phase}]}));
    walk_motion.insert(id.to_string(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 9.0, "hz": 1.0, "phase": phase}]}));
    wave_motion.insert(id.to_string(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 6.5, "hz": 1.2, "phase": phase}]}));
    jump_motion.insert(id.to_string(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 12.0, "hz": 1.0, "phase": phase}]}));
}

fn seed_arm_motion(
    id: &str,
    is_left: bool,
    walk_motion: &mut serde_json::Map<String, Value>,
    wave_motion: &mut serde_json::Map<String, Value>,
    jump_motion: &mut serde_json::Map<String, Value>,
) {
    let phase = if is_left { 0.0 } else { 0.5 };
    walk_motion.insert(id.to_string(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 16.0, "hz": 1.0, "phase": phase}]}));
    if is_left {
        wave_motion.insert(id.to_string(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 26.0, "hz": 1.8, "phase": 0.0}]}));
    }
    jump_motion.insert(id.to_string(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 22.0, "hz": 1.0, "phase": 0.1}]}));
}

fn seed_leg_motion(
    id: &str,
    is_left: bool,
    walk_motion: &mut serde_json::Map<String, Value>,
    jump_motion: &mut serde_json::Map<String, Value>,
) {
    let phase = if is_left { 0.5 } else { 0.0 };
    walk_motion.insert(id.to_string(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 18.0, "hz": 1.0, "phase": phase}]}));
    jump_motion.insert(id.to_string(), json!({"rotation": [{"kind": "oscillate", "axis": 0, "amplitude": 14.0, "hz": 1.0, "phase": 0.0}]}));
}

fn seed_part_motion(
    id: &str,
    root: &str,
    rest_motion: &mut serde_json::Map<String, Value>,
    walk_motion: &mut serde_json::Map<String, Value>,
    wave_motion: &mut serde_json::Map<String, Value>,
    jump_motion: &mut serde_json::Map<String, Value>,
) {
    let low = id.to_ascii_lowercase();
    if id == root {
        seed_root_motion(id, rest_motion, walk_motion, wave_motion, jump_motion);
    } else if low.contains("head") {
        seed_head_motion(id, rest_motion, walk_motion, wave_motion, jump_motion);
    } else if low.contains("ear")
        || low.contains("tail")
        || low.contains("hair")
        || low.contains("prop")
    {
        // Secondary follow-through with phase lag relative to parent gesture
        let phase = if low.contains("_r") || low.contains("right") {
            0.42
        } else {
            0.32
        };
        seed_secondary_motion(
            id,
            phase,
            rest_motion,
            walk_motion,
            wave_motion,
            jump_motion,
        );
    } else if low.contains("arm") || low.contains("wing") {
        seed_arm_motion(
            id,
            low.contains("_l") || low.contains("left"),
            walk_motion,
            wave_motion,
            jump_motion,
        );
    } else if low.contains("leg") || low.contains("foot") {
        seed_leg_motion(
            id,
            low.contains("_l") || low.contains("left"),
            walk_motion,
            jump_motion,
        );
    }
}

fn write_parts_rig(
    out_dir: &std::path::Path,
    asset_id: &str,
    order: &[Value],
    joints: Option<&[Value]>,
) -> Result<std::path::PathBuf> {
    let (layers, canvas) = rig_layers(out_dir, order)?;
    let joint = |id: &str| joints.and_then(|j| j.iter().find(|p| p["id"].as_str() == Some(id)));
    let root = rig_root(joints, &layers);

    let mut rest_motion = serde_json::Map::new();
    let mut walk_motion = serde_json::Map::new();
    let mut wave_motion = serde_json::Map::new();
    let mut jump_motion = serde_json::Map::new();

    for (id, _, _, _) in &layers {
        seed_part_motion(
            id,
            &root,
            &mut rest_motion,
            &mut walk_motion,
            &mut wave_motion,
            &mut jump_motion,
        );
    }

    let tracks = vec![
        json!({"id": "rest", "label": "Rest · gentle bob", "duration": 4, "loop": true, "motion": rest_motion}),
        json!({"id": "walk", "label": "Walk · stride cycle", "duration": 3, "loop": true, "motion": walk_motion}),
        json!({"id": "wave", "label": "Wave · greeting", "duration": 3, "loop": true, "motion": wave_motion}),
        json!({"id": "jump", "label": "Jump · hop", "duration": 2, "loop": true, "motion": jump_motion}),
    ];
    let rig = json!({"rig_type": "layered_motion", "character_id": asset_id, "canvas": [canvas.0, canvas.1], "fps": 30, "tracks": tracks,
        "layers": layers.iter().map(|(id, file, pivot, _)| match joint(id) {
            Some(j) => json!({"id": id, "file": file, "pivot": j["pivot"], "parent": j["parent"]}),
            None => json!({"id": id, "file": file, "pivot": pivot, "parent": if *id == root { Value::Null } else { json!(root) }}),
        }).collect::<Vec<_>>()});
    let path = out_dir.join("rig.json");
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&rig).map_err(|e| EngineError::InvalidParams(e.to_string()))?,
    )?;
    Ok(path)
}

/// Slice a reviewed rig sheet into canvas-registered part PNGs via `photocraft-cli` (`image.crop` +
/// `layer.removeBackground` + `file.placeEmbedded`), trace vectors via `vectorcraft-cli`, and write `rig.json`.
fn rig_sheet_path(s: &Session, p: &Value) -> Result<(String, std::path::PathBuf)> {
    let sheet_ref = p["sheet"]
        .as_str()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            EngineError::InvalidParams(
                "rig.from_sheet needs sheet=<rig sheet asset id or PNG path>".into(),
            )
        })?;
    let sheet = match s
        .project_assets
        .iter()
        .find(|a| a.id == sheet_ref)
        .and_then(|a| a.path.clone())
    {
        Some(path) => path,
        None => resolve_workspace_path(s, sheet_ref),
    };
    if !sheet.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "rig sheet not found: {}",
            sheet.display()
        )));
    }
    Ok((sheet_ref.to_string(), sheet))
}

pub fn cmd_rig_from_sheet(s: &mut Session, p: &Value) -> Result<Value> {
    let invalid = |message: String| EngineError::InvalidParams(message);
    let (sheet_ref, sheet) = rig_sheet_path(s, p)?;
    let review_asset = s
        .project_assets
        .iter()
        .find(|a| a.id == sheet_ref || a.path.as_ref() == Some(&sheet))
        .map(|a| a.id.as_str())
        .ok_or_else(|| {
            invalid(
                "Register the complete sheet as a project asset and review it before building a rig"
                    .into(),
            )
        })?;
    crate::sheets::require_sheet_review(s, review_asset, &sheet)?;

    let receipt_path = sheet.with_file_name("generation.json");
    let receipt: Value = if receipt_path.is_file() {
        serde_json::from_slice(&std::fs::read(&receipt_path)?).unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    let parts_val = p
        .get("parts")
        .or_else(|| p.get("rig_parts"))
        .filter(|v| !v.is_null())
        .or_else(|| receipt.pointer("/rig_sheet/parts"))
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .ok_or_else(|| {
            invalid(format!(
                "{} has no adjacent generation.json with rig_sheet.parts; pass `parts=[{{\"id\":...,\"parent\":...,\"pivot\":[u,v]}}]`",
                sheet.display()
            ))
        })?;

    let name = crate::workspace::sanitize_asset_id(p["name"].as_str().unwrap_or("rig"));
    let out_dir = match p["out_dir"].as_str() {
        Some(dir) => resolve_workspace_path(s, dir),
        None => crate::workshop::project_dir(s)?
            .join("assets/characters")
            .join(&name)
            .join("rig"),
    };
    std::fs::create_dir_all(&out_dir)?;

    let (sheet_w, sheet_h) = image::image_dimensions(&sheet)?;
    let total_cells = parts_val.len() + 1;
    let cols = receipt
        .pointer("/rig_sheet/columns")
        .and_then(Value::as_u64)
        .map(|c| c as u32)
        .unwrap_or_else(|| (total_cells as f64).sqrt().ceil() as u32)
        .max(1);
    let rows = receipt
        .pointer("/rig_sheet/rows")
        .and_then(Value::as_u64)
        .map(|r| r as u32)
        .unwrap_or_else(|| (total_cells as u32).div_ceil(cols))
        .max(1);
    let cell_w = sheet_w / cols;
    let cell_h = sheet_h / rows;

    let root = workspace_root(s);
    let pc_bin = resolve_tool("photocraft-cli");
    let vc_bin = resolve_tool("vectorcraft-cli");

    let mut order: Vec<Value> = Vec::new();
    let mut joints: Vec<Value> = Vec::new();

    for (idx, part) in parts_val.iter().enumerate() {
        let part_id = part["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| invalid("Each part in `parts` needs a non-empty `id`".into()))?;
        let cell_idx = (idx + 1) as u32;
        let col = cell_idx % cols;
        let row = cell_idx / cols;
        let cx = col * cell_w;
        let cy = row * cell_h;

        let part_png = out_dir.join(format!("{part_id}.png"));
        let cell_raw = out_dir.join(format!("{part_id}_cell.png"));

        // 1. Crop the part's cell and remove background via photocraft-cli
        let pc_out = run_tool_in(
            &root,
            &pc_bin,
            &[
                "run".into(),
                sheet.to_string_lossy().into_owned(),
                "--cmd".into(),
                "image.crop".into(),
                "--params".into(),
                json!({
                    "x": cx,
                    "y": cy,
                    "width": cell_w,
                    "height": cell_h,
                    "deleteCroppedPixels": true
                })
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
                cell_raw.to_string_lossy().into_owned(),
            ],
        )?;

        // 2. Place onto shared cell canvas (or explicit `places` [px, py] center) via photocraft-cli
        let center = p
            .get("places")
            .and_then(|pl| pl.get(part_id))
            .and_then(Value::as_array)
            .filter(|a| a.len() == 2)
            .and_then(|a| Some([a[0].as_f64()?, a[1].as_f64()?]))
            .unwrap_or([cell_w as f64 / 2.0, cell_h as f64 / 2.0]);

        run_tool_in(
            &root,
            &pc_bin,
            &[
                "run".into(),
                "--new".into(),
                json!({"width": cell_w, "height": cell_h, "background": "transparent"}).to_string(),
                "--cmd".into(),
                "file.placeEmbedded".into(),
                "--params".into(),
                json!({"path": cell_raw, "scale": 100.0, "fit": false, "center": center})
                    .to_string(),
                "--out".into(),
                part_png.to_string_lossy().into_owned(),
            ],
        )?;
        let _ = std::fs::remove_file(&cell_raw);

        // 3. Vector trace the part and compute ARAP puppet warp pins via vectorcraft-cli
        let part_svg = out_dir.join(format!("{part_id}.svg"));
        let vc_out = run_tool_in(
            &root,
            &vc_bin,
            &[
                "run".into(),
                "--in".into(),
                part_png.to_string_lossy().into_owned(),
                "--cmd".into(),
                "select.all".into(),
                "--cmd".into(),
                "imageTrace.makeAndExpand".into(),
                "--params".into(),
                "{\"preset\":\"16 Colors\",\"enhanced\":true,\"params\":{\"ignoreWhite\":true}}"
                    .into(),
                "--cmd".into(),
                "object.puppetWarp.pins".into(),
                "--params".into(),
                "{}".into(),
                "--export".into(),
                part_svg.to_string_lossy().into_owned(),
            ],
        )?;

        // Read PhotoCraft subject bounds + VectorCraft ARAP puppet-warp pins directly from JSONL
        let cell_bounds = pc_out
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find(|v| v["command"].as_str() == Some("layer.removeBackground"))
            .and_then(|v| v["result"]["bounds"].as_array().cloned())
            .filter(|b| b.len() == 4);
        let puppet_pins = vc_out
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find(|v| v["command"].as_str() == Some("object.puppetWarp.pins"))
            .and_then(|v| v["result"]["pins"].clone().into())
            .unwrap_or_else(|| json!([]));
        let explicit_uv = p
            .get("pivots")
            .and_then(|pv| pv.get(part_id))
            .or_else(|| part.get("pivot"))
            .and_then(Value::as_array)
            .filter(|a| a.len() == 2)
            .and_then(|a| Some([a[0].as_f64()?, a[1].as_f64()?]));
        let offset = [
            center[0] - cell_w as f64 / 2.0,
            center[1] - cell_h as f64 / 2.0,
        ];
        let pivot_px = if let Some(uv) = explicit_uv {
            if let Some(ref b) = cell_bounds {
                let bx = b[0].as_f64().unwrap_or(0.0) + offset[0];
                let by = b[1].as_f64().unwrap_or(0.0) + offset[1];
                let bw = b[2].as_f64().unwrap_or(cell_w as f64);
                let bh = b[3].as_f64().unwrap_or(cell_h as f64);
                [bx + bw * uv[0], by + bh * uv[1]]
            } else {
                [cell_w as f64 * uv[0], cell_h as f64 * uv[1]]
            }
        } else {
            puppet_pins
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(Value::as_array)
                .filter(|pt| pt.len() == 2)
                .and_then(|pt| Some([pt[0].as_f64()?, pt[1].as_f64()?]))
                .unwrap_or(center)
        };

        order.push(json!(part_id));
        joints.push(json!({
            "id": part_id,
            "parent": part.get("parent").cloned().unwrap_or(Value::Null),
            "pivot": pivot_px,
            "puppet_pins": puppet_pins,
            "file": part_png,
            "svg": part_svg,
        }));
    }

    if let Some(explicit_order) = p.get("order").and_then(Value::as_array)
        && !explicit_order.is_empty()
    {
        order = explicit_order.clone();
    }

    let rig = write_parts_rig(&out_dir, &name, &order, Some(&joints))?;
    Ok(json!({
        "ok": true,
        "command": "rig.from_sheet",
        "rig": rig,
        "canvas": [cell_w, cell_h],
        "order": order,
        "parts": joints,
        "next": format!("project.add_asset path={} kind=cutout_rig id={name} name={name}", rig.display()),
    }))
}

/// Warp the active asset's source with explicit src/dst pins via PhotoCraft puppet warp.
pub fn cmd_image_puppet_warp(s: &mut Session, params: &Value) -> Result<Value> {
    let (asset_id, source_path) = resolve_asset_source(s, params, "warp")?;

    let root = workspace_root(s);
    let out_path = if let Some(out_raw) = params.get("out").and_then(Value::as_str) {
        resolve_workspace_path(s, out_raw)
    } else {
        root.join("exports/pose_and_parts")
            .join(format!("{asset_id}_warped.png"))
    };
    std::fs::create_dir_all(out_path.parent().unwrap_or(&root))?;

    // Pins come from the caller (e.g. rig joints); there is no sensible default.
    let pins: Vec<Value> = params
        .get("pins")
        .and_then(Value::as_array)
        .filter(|p| !p.is_empty())
        .cloned()
        .ok_or_else(|| {
            EngineError::InvalidParams(
                "image.puppet_warp requires `pins`: [{\"src\":[x,y],\"dst\":[x,y]}, ...]".into(),
            )
        })?;

    let warp_params = json!({
        "pins": pins,
        "mode": params.get("mode").and_then(Value::as_str).unwrap_or("normal"),
        "density": params.get("density").and_then(Value::as_str).unwrap_or("normal"),
        "expansion": params.get("expansion").and_then(Value::as_f64).unwrap_or(2.0),
    });
    let args = image_batch_args(
        &source_path.to_string_lossy(),
        "edit.puppetWarp",
        Some(&warp_params.to_string()),
        &out_path.to_string_lossy(),
    );

    let bin = resolve_tool("photocraft-cli");
    run_tool_in(&root, &bin, &args)?;

    let (w, h) = if out_path.is_file() {
        image::image_dimensions(&out_path).unwrap_or((0, 0))
    } else {
        (0, 0)
    };

    Ok(json!({
        "ok": out_path.is_file(),
        "command": "image.puppet_warp",
        "asset_id": asset_id,
        "source": source_path.to_string_lossy(),
        "output": out_path.to_string_lossy(),
        "width": w,
        "height": h,
        "pins": pins,
    }))
}

/// Convert a color to alpha on the active asset's source via PhotoCraft.
pub fn cmd_image_color_to_alpha(s: &mut Session, params: &Value) -> Result<Value> {
    let (asset_id, source_path) = resolve_asset_source(s, params, "convert")?;

    let root = workspace_root(s);
    let out_path = if let Some(out_raw) = params.get("out").and_then(Value::as_str) {
        resolve_workspace_path(s, out_raw)
    } else {
        root.join("exports/pose_and_parts")
            .join(format!("{asset_id}_alpha.png"))
    };
    std::fs::create_dir_all(out_path.parent().unwrap_or(&root))?;

    let color_raw = params
        .get("color")
        .and_then(Value::as_str)
        .unwrap_or("#FFFFFF");

    let transparency_threshold = params
        .get("transparencyThreshold")
        .and_then(Value::as_f64)
        .map(|v| v.clamp(0.0, 1.0))
        .unwrap_or(0.0);
    let opacity_threshold = params
        .get("opacityThreshold")
        .and_then(Value::as_f64)
        .map(|v| v.clamp(0.0, 1.0))
        .unwrap_or(1.0);

    let color_params = json!({
        "color": color_raw,
        "transparencyThreshold": transparency_threshold,
        "opacityThreshold": opacity_threshold,
    });
    let args = image_batch_args(
        &source_path.to_string_lossy(),
        "filter.other.colorToAlpha",
        Some(&color_params.to_string()),
        &out_path.to_string_lossy(),
    );

    let bin = resolve_tool("photocraft-cli");
    run_tool_in(&root, &bin, &args)?;

    let (w, h) = if out_path.is_file() {
        image::image_dimensions(&out_path).unwrap_or((0, 0))
    } else {
        (0, 0)
    };

    Ok(json!({
        "ok": out_path.is_file(),
        "command": "image.color_to_alpha",
        "asset_id": asset_id,
        "source": source_path.to_string_lossy(),
        "output": out_path.to_string_lossy(),
        "width": w,
        "height": h,
        "color": color_raw,
        "transparencyThreshold": transparency_threshold,
        "opacityThreshold": opacity_threshold,
    }))
}

/// Content-aware fill on a named selection of the active asset's source via PhotoCraft.
pub fn cmd_image_content_aware_fill(s: &mut Session, params: &Value) -> Result<Value> {
    let (asset_id, source_path) = resolve_asset_source(s, params, "fill")?;

    let root = workspace_root(s);
    let out_path = if let Some(out_raw) = params.get("out").and_then(Value::as_str) {
        resolve_workspace_path(s, out_raw)
    } else {
        root.join("exports/pose_and_parts")
            .join(format!("{asset_id}_filled.png"))
    };
    std::fs::create_dir_all(out_path.parent().unwrap_or(&root))?;

    // edit.fill acts on the current selection, so the caller names the selection command
    // (e.g. select.rect, select.colorRange) that runs first in the same document session.
    let select = params
        .get("select")
        .filter(|v| v.get("id").and_then(Value::as_str).is_some())
        .ok_or_else(|| {
            EngineError::InvalidParams(
                "image.content_aware_fill requires `select`: {\"id\":\"select.*\",\"params\":{...}}".into(),
            )
        })?;
    let select_id = select["id"].as_str().unwrap_or_default();
    let fill_contents = params
        .get("contents")
        .and_then(Value::as_str)
        .unwrap_or("contentAware");
    let color = params.get("color").and_then(Value::as_str);

    let mut fill_params = serde_json::Map::new();
    fill_params.insert("contents".to_string(), json!(fill_contents));
    if let Some(c) = color {
        fill_params.insert("color".to_string(), json!(c));
    }

    let fill_params_json = serde_json::to_string(&fill_params)
        .map_err(|e| EngineError::InvalidParams(format!("cannot serialize fill params: {e}")))?;
    let mut args = vec![
        "run".to_string(),
        source_path.to_string_lossy().into_owned(),
        "--cmd".to_string(),
        select_id.to_string(),
    ];
    if let Some(p) = select.get("params") {
        args.push("--params".to_string());
        args.push(p.to_string());
    }
    args.extend([
        "--cmd".to_string(),
        "edit.fill".to_string(),
        "--params".to_string(),
        fill_params_json,
        "--out".to_string(),
        out_path.to_string_lossy().into_owned(),
    ]);

    let bin = resolve_tool("photocraft-cli");
    run_tool_in(&root, &bin, &args)?;

    let (w, h) = if out_path.is_file() {
        image::image_dimensions(&out_path).unwrap_or((0, 0))
    } else {
        (0, 0)
    };

    Ok(json!({
        "ok": out_path.is_file(),
        "command": "image.content_aware_fill",
        "asset_id": asset_id,
        "source": source_path.to_string_lossy(),
        "output": out_path.to_string_lossy(),
        "width": w,
        "height": h,
        "select": select_id,
        "contents": fill_contents,
        "color": color,
    }))
}

/// Normalise a sheet to the contract canvas via PhotoCraft `image.imageSize`, then report it.
/// Generated artwork arrives at whatever size the image tool chose; the sheet contract is fixed.
pub fn cmd_image_image_size(s: &mut Session, params: &Value) -> Result<Value> {
    let source = params
        .get("source")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::InvalidParams("image.image_size requires `source`".into()))?;
    let source_path = resolve_workspace_path(s, source);
    if !source_path.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "image not found: {}",
            source_path.display()
        )));
    }
    let width = params
        .get("width")
        .and_then(Value::as_u64)
        .unwrap_or(2048)
        .clamp(16, 8192);
    let height = params
        .get("height")
        .and_then(Value::as_u64)
        .unwrap_or(2048)
        .clamp(16, 8192);
    let anchor = params
        .get("anchor")
        .and_then(Value::as_str)
        .unwrap_or("center");
    let extension = params
        .get("extensionColor")
        .and_then(Value::as_str)
        .unwrap_or("#ffffff");
    let out_path = match params
        .get("out")
        .and_then(Value::as_str)
        .filter(|o| !o.is_empty())
    {
        Some(out) => resolve_workspace_path(s, out),
        None => source_path.with_file_name(format!(
            "{}-{width}x{height}.png",
            source_path
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or("image")
        )),
    };
    let root = workspace_root(s);
    if let Some(dir) = out_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let body =
        json!({"width": width, "height": height, "anchor": anchor, "extensionColor": extension})
            .to_string();
    let args = image_batch_args(
        &source_path.to_string_lossy(),
        "image.imageSize",
        Some(&body),
        &out_path.to_string_lossy(),
    );
    let bin = resolve_tool("photocraft-cli");
    run_tool_in(&root, &bin, &args)?;
    let (w, h) = if out_path.is_file() {
        image::image_dimensions(&out_path).unwrap_or((0, 0))
    } else {
        (0, 0)
    };
    Ok(json!({
        "ok": out_path.is_file() && w == width as u32 && h == height as u32,
        "command": "image.image_size",
        "source": source_path.to_string_lossy(),
        "output": out_path.to_string_lossy(),
        "width": w,
        "height": h,
    }))
}
