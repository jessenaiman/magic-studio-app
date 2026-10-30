//! Rig layer decoding: keyed layer-map plans, layered motion specs, compositing,
//! and alpha/component analysis. Called from [`crate::session::LoadedCutoutRigPlan`]
//! loading.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::session_manifests::render_layer_contact_sheet;
use crate::session::{CutoutRigPart, LoadedCutoutRigPlan, load_control_bindings};
use crate::{EngineError, Result};

/// Older HyperFrames rigs store cropped PNGs as an explicitly ordered layer map.
pub(crate) fn load_keyed_layers(
    path: &Path,
    raw: &Value,
    layers: &serde_json::Map<String, Value>,
) -> Result<LoadedCutoutRigPlan> {
    let (cw, ch) = parse_keyed_canvas(path, raw)?;
    let ids = parse_keyed_layer_order(path, raw, layers)?;
    let base = path.parent().ok_or_else(|| {
        EngineError::InvalidParams(format!(
            "Invalid keyed rig {}: manifest directory missing",
            path.display()
        ))
    })?;
    let decoded = decode_keyed_layers(path, base, cw, ch, &ids, layers)?;
    build_keyed_rig(path, raw, base, cw, ch, decoded)
}

pub(crate) fn load_layered_motion_spec(
    path: &Path,
    raw: &Value,
    canvas_arr: &[Value],
    layers_arr: &[Value],
) -> Result<LoadedCutoutRigPlan> {
    let (cw, ch) = parse_layered_canvas(canvas_arr);
    let character_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("layered_motion")
        .to_string();
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let cache_dir = base_dir.join(".layered_preview_cache").join(&character_id);
    let _ = std::fs::create_dir_all(&cache_dir);

    let mut parts = Vec::with_capacity(layers_arr.len());
    let mut root_file: Option<PathBuf> = None;
    let mut union_bbox: Option<[u32; 4]> = None;
    let mut composite_canvas = image::RgbaImage::new(cw, ch);
    let mut has_composite_pixels = false;

    for (idx, item) in layers_arr.iter().enumerate() {
        let part = decode_layered_spec_item(
            idx,
            item,
            base_dir,
            &cache_dir,
            [cw, ch],
            &mut root_file,
            &mut union_bbox,
            &mut composite_canvas,
            &mut has_composite_pixels,
        );
        parts.push(part);
    }

    let character_bbox = union_bbox.unwrap_or([0, 0, cw, ch]);
    let (source_image, cutout_asset_sheet) = resolve_layered_preview_outputs(
        raw,
        &character_id,
        &cache_dir,
        [cw, ch],
        character_bbox,
        root_file,
        &parts,
        &composite_canvas,
        has_composite_pixels,
    );
    let control_bindings = load_control_bindings(raw, path, &parts)?;
    Ok(LoadedCutoutRigPlan {
        control_bindings,
        plan_path: path.to_path_buf(),
        character_id,
        rig_type: "layered_motion".to_string(),
        source_image,
        canvas_size: [cw, ch],
        character_bbox,
        photocraft_psd: None,
        cutout_asset_sheet,
        parts,
        pose_library: Value::Null,
        effectcraft: raw.clone(),
    })
}

type DecodedKeyedLayer = (String, PathBuf, Vec<u32>, Vec<f32>, image::RgbaImage);

fn keyed_rig_err(path: &Path, message: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidParams(format!("Invalid keyed rig {}: {message}", path.display()))
}

fn parse_keyed_canvas(path: &Path, raw: &Value) -> Result<(u32, u32)> {
    let canvas = raw["canvas"]
        .as_array()
        .filter(|v| v.len() == 2)
        .ok_or_else(|| keyed_rig_err(path, "canvas must contain two positive integers"))?;
    let dims: Vec<u32> = canvas
        .iter()
        .map(|v| {
            v.as_u64()
                .filter(|n| *n > 0 && *n <= 8192)
                .map(|n| n as u32)
                .ok_or_else(|| keyed_rig_err(path, "canvas values must be 1..8192"))
        })
        .collect::<Result<_>>()?;
    Ok((dims[0], dims[1]))
}

fn parse_keyed_layer_order<'a>(
    path: &Path,
    raw: &'a Value,
    layers: &serde_json::Map<String, Value>,
) -> Result<Vec<&'a str>> {
    let order = raw["layer_order"]
        .as_array()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| keyed_rig_err(path, "nonempty layer_order is required"))?;
    let ids: Vec<&str> = order
        .iter()
        .map(|v| {
            v.as_str()
                .ok_or_else(|| keyed_rig_err(path, "layer_order entries must be IDs"))
        })
        .collect::<Result<_>>()?;
    let unique: std::collections::BTreeSet<_> = ids.iter().copied().collect();
    if layers.is_empty()
        || unique.len() != ids.len()
        || unique.len() != layers.len()
        || layers.keys().any(|k| !unique.contains(k.as_str()))
    {
        return Err(keyed_rig_err(
            path,
            "layer_order must contain each nonempty layer ID exactly once",
        ));
    }
    Ok(ids)
}

fn parse_keyed_layer_box(path: &Path, id: &str, layer: &Value, cw: u32, ch: u32) -> Result<Vec<u32>> {
    let b = layer["box"]
        .as_array()
        .filter(|v| v.len() == 4)
        .ok_or_else(|| keyed_rig_err(path, format!("{id}: box must be [x,y,width,height]")))?;
    let b: Vec<u32> = b
        .iter()
        .map(|v| {
            v.as_u64()
                .filter(|n| *n <= 8192)
                .map(|n| n as u32)
                .ok_or_else(|| keyed_rig_err(path, format!("{id}: box requires nonnegative integers")))
        })
        .collect::<Result<_>>()?;
    if b[2] == 0 || b[3] == 0 || b[0] + b[2] > cw || b[1] + b[3] > ch {
        return Err(keyed_rig_err(
            path,
            format!("{id}: box exceeds canvas or has zero size"),
        ));
    }
    Ok(b)
}

fn parse_keyed_layer_pivot(path: &Path, id: &str, layer: &Value, cw: u32, ch: u32) -> Result<Vec<f32>> {
    let p = layer["pivot"]
        .as_array()
        .filter(|v| v.len() == 2)
        .ok_or_else(|| keyed_rig_err(path, format!("{id}: pivot must be [x,y]")))?;
    let p: Vec<f32> = p
        .iter()
        .map(|v| {
            v.as_f64()
                .filter(|n| n.is_finite())
                .map(|n| n as f32)
                .ok_or_else(|| keyed_rig_err(path, format!("{id}: pivot must be finite")))
        })
        .collect::<Result<_>>()?;
    if p[0] < 0.0 || p[1] < 0.0 || p[0] > cw as f32 || p[1] > ch as f32 {
        return Err(keyed_rig_err(
            path,
            format!("{id}: pivot lies outside canvas"),
        ));
    }
    Ok(p)
}

fn decode_keyed_layers(
    path: &Path,
    base: &Path,
    cw: u32,
    ch: u32,
    ids: &[&str],
    layers: &serde_json::Map<String, Value>,
) -> Result<Vec<DecodedKeyedLayer>> {
    let mut decoded = Vec::new();
    for id in ids {
        if id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            return Err(keyed_rig_err(path, format!("unsafe layer ID {id}")));
        }
        let layer = &layers[*id];
        let b = parse_keyed_layer_box(path, id, layer, cw, ch)?;
        let p = parse_keyed_layer_pivot(path, id, layer, cw, ch)?;
        let file = base.join(format!("{id}.png"));
        let image = image::open(&file)
            .map_err(|e| keyed_rig_err(path, format!("{id}: cannot decode {}: {e}", file.display())))?;
        if image.width() != b[2] || image.height() != b[3] || !image.color().has_alpha() {
            return Err(keyed_rig_err(
                path,
                format!(
                    "{id}: PNG dimensions/alpha do not match declared box {}x{}",
                    b[2], b[3]
                ),
            ));
        }
        decoded.push(((*id).to_string(), file, b, p, image.into_rgba8()));
    }
    Ok(decoded)
}

fn build_keyed_rig(
    path: &Path,
    raw: &Value,
    base: &Path,
    cw: u32,
    ch: u32,
    decoded: Vec<DecodedKeyedLayer>,
) -> Result<LoadedCutoutRigPlan> {
    let cache = base.join(".layered_preview_cache/keyed");
    std::fs::create_dir_all(&cache)?;
    let mut rest = image::RgbaImage::new(cw, ch);
    let mut parts = Vec::new();
    let mut bbox = [cw, ch, 0, 0];
    let mut spec_layers = Vec::new();
    for (index, (id, file, b, p, pixels)) in decoded.into_iter().enumerate() {
        let mut full = image::RgbaImage::new(cw, ch);
        image::imageops::overlay(&mut full, &pixels, b[0] as i64, b[1] as i64);
        let full_path = cache.join(format!("{id}.png"));
        full.save(&full_path)?;
        image::imageops::overlay(&mut rest, &pixels, b[0] as i64, b[1] as i64);
        bbox = [
            bbox[0].min(b[0]),
            bbox[1].min(b[1]),
            bbox[2].max(b[0] + b[2]),
            bbox[3].max(b[1] + b[3]),
        ];
        spec_layers.push(serde_json::json!({"id":id,"file":full_path,"pivot":p}));
        parts.push(CutoutRigPart {
            id: id.clone(),
            label: id,
            kind: "layered".into(),
            layer: index as i32,
            parent: None,
            pivot: [p[0], p[1]],
            pivot_uv: [
                (p[0] - b[0] as f32) / b[2] as f32,
                (p[1] - b[1] as f32) / b[3] as f32,
            ],
            crop_box: [b[0], b[1], b[0] + b[2], b[1] + b[3]],
            crop_size: [b[2], b[3]],
            cropped_path: file,
            full_layer_path: full_path,
            ..Default::default()
        });
    }
    let source = cache.join("assembled-rest.png");
    rest.save(&source)?;
    let controls = load_control_bindings(raw, path, &parts)?;
    Ok(LoadedCutoutRigPlan {
        control_bindings: controls,
        plan_path: path.into(),
        character_id: raw["fish"]
            .as_str()
            .or(raw["character_id"].as_str())
            .unwrap_or("keyed_subject")
            .into(),
        rig_type: "layered_motion".into(),
        source_image: source,
        canvas_size: [cw, ch],
        character_bbox: bbox,
        parts,
        effectcraft: serde_json::json!({"canvas":[cw,ch],"fps":30,"layers":spec_layers,"tracks":[]}),
        ..Default::default()
    })
}

fn parse_layered_canvas(canvas_arr: &[Value]) -> (u32, u32) {
    let cw = canvas_arr
        .first()
        .and_then(Value::as_u64)
        .unwrap_or(1920)
        .max(1) as u32;
    let ch = canvas_arr
        .get(1)
        .and_then(Value::as_u64)
        .unwrap_or(1080)
        .max(1) as u32;
    (cw, ch)
}

fn blend_channel(sc: u8, dc: u8, sa: f32, da: f32, out_a: f32) -> u8 {
    (((sc as f32 * sa) + (dc as f32 * da * (1.0 - sa))) / out_a)
        .round()
        .clamp(0.0, 255.0) as u8
}

fn blend_pixel(src_p: image::Rgba<u8>, dst_p: image::Rgba<u8>) -> Option<image::Rgba<u8>> {
    let sa = src_p[3] as f32 / 255.0;
    if sa <= 0.004 {
        return None;
    }
    let da = dst_p[3] as f32 / 255.0;
    let out_a = (sa + da * (1.0 - sa)).max(1e-5);
    Some(image::Rgba([
        blend_channel(src_p[0], dst_p[0], sa, da, out_a),
        blend_channel(src_p[1], dst_p[1], sa, da, out_a),
        blend_channel(src_p[2], dst_p[2], sa, da, out_a),
        (out_a * 255.0).round().clamp(0.0, 255.0) as u8,
    ]))
}

fn blend_rgba_onto_composite(
    rgba: &image::RgbaImage,
    crop_box: [u32; 4],
    composite_canvas: &mut image::RgbaImage,
) -> bool {
    let mut painted = false;
    for y in crop_box[1]..crop_box[3] {
        for x in crop_box[0]..crop_box[2] {
            if let Some(out) = blend_pixel(*rgba.get_pixel(x, y), *composite_canvas.get_pixel(x, y))
            {
                composite_canvas.put_pixel(x, y, out);
                painted = true;
            }
        }
    }
    painted
}

fn sanitize_layer_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn save_cropped_layer_preview(
    rgba: &image::RgbaImage,
    crop_box: [u32; 4],
    idx: usize,
    id: &str,
    cache_dir: &Path,
) -> Option<PathBuf> {
    let cw_crop = crop_box[2] - crop_box[0];
    let ch_crop = crop_box[3] - crop_box[1];
    let sub = image::imageops::crop_imm(rgba, crop_box[0], crop_box[1], cw_crop, ch_crop).to_image();
    let safe_id = sanitize_layer_id(id);
    let dest = cache_dir.join(format!("{idx:02}_{safe_id}_crop.png"));
    sub.save(&dest).ok()?;
    Some(dest)
}

fn parse_layered_pivot(item: &Value, canvas: [u32; 2], alpha_box: [u32; 4]) -> [f32; 2] {
    if let Some(piv_arr) = item.get("pivot").and_then(Value::as_array) {
        return [
            piv_arr
                .first()
                .and_then(Value::as_f64)
                .unwrap_or(canvas[0] as f64 * 0.5) as f32,
            piv_arr
                .get(1)
                .and_then(Value::as_f64)
                .unwrap_or(canvas[1] as f64 * 0.5) as f32,
        ];
    }
    let x0 = alpha_box[0] as f32;
    let y0 = alpha_box[1] as f32;
    let x1 = alpha_box[2] as f32;
    let y1 = alpha_box[3] as f32;
    let cx = 0.5 * (x0 + x1);
    let cy = 0.5 * (y0 + y1);
    let Some(piv_obj) = item.get("pivot").and_then(Value::as_object) else {
        return [cx, cy];
    };
    match piv_obj.get("anchor").and_then(Value::as_str).unwrap_or("center") {
        "top" => [cx, y0],
        "bottom" => [cx, y1],
        "left" => [x0, cy],
        "right" => [x1, cy],
        "topLeft" => [x0, y0],
        "topRight" => [x1, y0],
        "bottomLeft" => [x0, y1],
        "bottomRight" => [x1, y1],
        _ => [cx, cy],
    }
}

#[allow(clippy::too_many_arguments)]
fn decode_layered_spec_item(
    idx: usize,
    item: &Value,
    base_dir: &Path,
    cache_dir: &Path,
    canvas: [u32; 2],
    root_file: &mut Option<PathBuf>,
    union_bbox: &mut Option<[u32; 4]>,
    composite_canvas: &mut image::RgbaImage,
    has_composite_pixels: &mut bool,
) -> CutoutRigPart {
    let [cw, ch] = canvas;
    let id = item
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("layer")
        .to_string();
    let file_path = item
        .get("file")
        .and_then(Value::as_str)
        .map(|f| {
            let p = PathBuf::from(f);
            if p.is_absolute() { p } else { base_dir.join(p) }
        })
        .unwrap_or_default();
    let parent = item.get("parent").and_then(Value::as_str).map(ToString::to_string);
    if parent.is_none() && root_file.is_none() && !file_path.as_os_str().is_empty() {
        *root_file = Some(file_path.clone());
    }
    let component_idx = item.get("component").and_then(Value::as_u64).map(|v| v as usize);
    let mut crop_box = [0, 0, cw, ch];
    let mut alpha_box = [0, 0, cw, ch];
    let mut cropped_path = file_path.clone();

    if let Some((crop, alpha, cropped)) = process_layered_spec_image(
        &id,
        &file_path,
        canvas,
        cache_dir,
        idx,
        component_idx,
        union_bbox,
        composite_canvas,
        has_composite_pixels,
    ) {
        crop_box = crop;
        alpha_box = alpha;
        cropped_path = cropped;
    }

    let pivot = parse_layered_pivot(item, canvas, alpha_box);
    let crop_size = [
        (crop_box[2] - crop_box[0]).max(1),
        (crop_box[3] - crop_box[1]).max(1),
    ];
    let pivot_uv = [
        ((pivot[0] - crop_box[0] as f32) / crop_size[0] as f32).clamp(0.0, 1.0),
        ((pivot[1] - crop_box[1] as f32) / crop_size[1] as f32).clamp(0.0, 1.0),
    ];

    CutoutRigPart {
        id: id.clone(),
        label: id,
        kind: "layered".to_string(),
        layer: (idx as i32) + 1,
        parent,
        pivot,
        pivot_uv,
        joint_radius_px: 10,
        completed_overlap_pixels: 0,
        rotation: [-60.0, 60.0],
        crop_box,
        crop_size,
        cropped_path,
        full_layer_path: file_path,
    }
}

#[allow(clippy::too_many_arguments)]
fn process_layered_spec_image(
    id: &str,
    file_path: &Path,
    canvas: [u32; 2],
    cache_dir: &Path,
    idx: usize,
    component_idx: Option<usize>,
    union_bbox: &mut Option<[u32; 4]>,
    composite_canvas: &mut image::RgbaImage,
    has_composite_pixels: &mut bool,
) -> Option<([u32; 4], [u32; 4], PathBuf)> {
    if !file_path.is_file() {
        return None;
    }
    let mut rgba = image::open(file_path).ok()?.into_rgba8();
    if let Some(comp_i) = component_idx {
        isolate_connected_component_left_to_right(&mut rgba, comp_i);
    }
    let alpha_box = compute_alpha_bounds(&rgba, 16)?;
    let pad = 2u32;
    let bx0 = alpha_box[0].saturating_sub(pad);
    let by0 = alpha_box[1].saturating_sub(pad);
    let bx1 = (alpha_box[2] + pad).min(rgba.width());
    let by1 = (alpha_box[3] + pad).min(rgba.height());
    let crop_box = [bx0, by0, bx1.max(bx0 + 1), by1.max(by0 + 1)];
    *union_bbox = Some(match *union_bbox {
        Some(u) => [
            u[0].min(crop_box[0]),
            u[1].min(crop_box[1]),
            u[2].max(crop_box[2]),
            u[3].max(crop_box[3]),
        ],
        None => crop_box,
    });
    if !id.ends_with("closed") && rgba.width() == canvas[0] && rgba.height() == canvas[1] {
        *has_composite_pixels |= blend_rgba_onto_composite(&rgba, crop_box, composite_canvas);
    }
    let cropped_path = save_cropped_layer_preview(&rgba, crop_box, idx, id, cache_dir)
        .unwrap_or_else(|| file_path.to_path_buf());
    Some((crop_box, alpha_box, cropped_path))
}

#[allow(clippy::too_many_arguments)]
fn resolve_layered_preview_outputs(
    raw: &Value,
    character_id: &str,
    cache_dir: &Path,
    canvas: [u32; 2],
    character_bbox: [u32; 4],
    root_file: Option<PathBuf>,
    parts: &[CutoutRigPart],
    composite_canvas: &image::RgbaImage,
    has_composite_pixels: bool,
) -> (PathBuf, Option<PathBuf>) {
    let [cw, ch] = canvas;
    let mut source_image = root_file
        .or_else(|| parts.first().map(|p| p.full_layer_path.clone()))
        .unwrap_or_default();
    let mut cutout_asset_sheet = None;
    if has_composite_pixels {
        let pad = 12u32;
        let cx0 = character_bbox[0].saturating_sub(pad);
        let cy0 = character_bbox[1].saturating_sub(pad);
        let cx1 = (character_bbox[2] + pad).min(cw);
        let cy1 = (character_bbox[3] + pad).min(ch);
        if cx1 > cx0 && cy1 > cy0 {
            let comp_crop =
                image::imageops::crop_imm(composite_canvas, cx0, cy0, cx1 - cx0, cy1 - cy0)
                    .to_image();
            let comp_dest = cache_dir.join(format!("{character_id}_composite.png"));
            if comp_crop.save(&comp_dest).is_ok() {
                source_image = comp_dest;
            }
            let sheet_dest = cache_dir.join(format!("{character_id}_layer_contact_sheet.png"));
            if render_layer_contact_sheet(&comp_crop, [cx0, cy0], parts, &sheet_dest) {
                cutout_asset_sheet = Some(sheet_dest);
            }
        }
    }
    if raw["frame_sequence"].is_object()
        && let Some(part) = parts.first()
    {
        source_image = part.full_layer_path.clone();
    }
    (source_image, cutout_asset_sheet)
}

pub(crate) fn compute_alpha_bounds(rgba: &image::RgbaImage, threshold: u8) -> Option<[u32; 4]> {
    let w = rgba.width();
    let h = rgba.height();
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut found = false;
    for y in 0..h {
        for x in 0..w {
            if rgba.get_pixel(x, y)[3] > threshold {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x + 1);
                max_y = max_y.max(y + 1);
                found = true;
            }
        }
    }
    found.then_some([min_x, min_y, max_x, max_y])
}

fn flood_fill_component(
    rgba: &image::RgbaImage,
    labels: &mut [u32],
    w: usize,
    h: usize,
    seed: (usize, usize),
    label: u32,
) -> (usize, usize) {
    let mut stack = vec![seed];
    labels[seed.1 * w + seed.0] = label;
    let mut area = 0usize;
    let mut min_x = seed.0;
    while let Some((cx, cy)) = stack.pop() {
        area += 1;
        min_x = min_x.min(cx);
        for ny in cy.saturating_sub(1)..=(cy + 1).min(h - 1) {
            for nx in cx.saturating_sub(1)..=(cx + 1).min(w - 1) {
                let nidx = ny * w + nx;
                if labels[nidx] == 0 && rgba.get_pixel(nx as u32, ny as u32)[3] > 16 {
                    labels[nidx] = label;
                    stack.push((nx, ny));
                }
            }
        }
    }
    (area, min_x)
}

fn label_connected_components(
    rgba: &image::RgbaImage,
    labels: &mut [u32],
    w: usize,
    h: usize,
) -> Vec<(u32, usize, usize)> {
    let mut comp_areas: Vec<(u32, usize, usize)> = Vec::new();
    let mut current_label = 0u32;
    for sy in 0..h {
        for sx in 0..w {
            let idx = sy * w + sx;
            if labels[idx] != 0 || rgba.get_pixel(sx as u32, sy as u32)[3] <= 16 {
                continue;
            }
            current_label += 1;
            let (area, min_x) = flood_fill_component(rgba, labels, w, h, (sx, sy), current_label);
            comp_areas.push((current_label, area, min_x));
        }
    }
    comp_areas
}

/// Pick the label of the `index`-th significant component, ordered by leftmost pixel.
fn select_component_label(comp_areas: Vec<(u32, usize, usize)>, index: usize) -> Option<u32> {
    let max_area = comp_areas.iter().map(|c| c.1).max().unwrap_or(0);
    if max_area == 0 {
        return None;
    }
    let min_area = if max_area < 50 {
        max_area
    } else {
        50.max((max_area as f64 * 0.02) as usize)
    };
    let mut valid: Vec<(u32, usize)> = comp_areas
        .into_iter()
        .filter(|c| c.1 >= min_area)
        .map(|(lbl, _, min_x)| (lbl, min_x))
        .collect();
    valid.sort_by_key(|c| c.1);
    valid.get(index).map(|&(lbl, _)| lbl)
}

fn isolate_connected_component_left_to_right(rgba: &mut image::RgbaImage, index: usize) {
    let w = rgba.width() as usize;
    let h = rgba.height() as usize;
    if w == 0 || h == 0 {
        return;
    }
    let mut labels = vec![0u32; w * h];
    let comp_areas = label_connected_components(rgba, &mut labels, w, h);
    let Some(keep_label) = select_component_label(comp_areas, index) else {
        return;
    };
    for (idx, px) in rgba.pixels_mut().enumerate() {
        if labels[idx] != keep_label {
            *px = image::Rgba([0, 0, 0, 0]);
        }
    }
}