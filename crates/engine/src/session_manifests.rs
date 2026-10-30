//! Parsed cut manifests and measured motion/registration telemetry.
//!
//! Also hosts the layer contact-sheet rendering helpers used when a layered
//! rig plan decodes, since the sheet documents manifest-level part QA.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::session::CutoutRigPart;
use crate::{EngineError, Result};

/// Visibility, solo, and verification state for a cut part from the manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartState {
    pub name: String,
    pub visible: bool,
    pub solo: bool,
    #[serde(default)]
    pub verified: bool,
}

/// A parsed `manifest.json` produced by `pose_and_part_cutter.py` or `codex_accuracy_optimizer.py`.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LoadedManifest {
    pub manifest_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub sheet_type: String,
    pub status: String,
    pub matting_engine: Option<String>,
    #[serde(default)]
    pub source_image: Option<PathBuf>,
    #[serde(default)]
    pub source_size: Option<[u32; 2]>,
    pub frames: Vec<ManifestFrame>,
    pub part_names: Vec<String>,
}

/// One cutout part entry inside a [`ManifestFrame`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManifestPart {
    pub name: String,
    pub file: String,
    pub resolved_path: PathBuf,
    pub size: [u32; 2],
    pub pivot: [f32; 2],
    pub bind_position: [f32; 2],
}

/// One frame entry from `manifest.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManifestFrame {
    #[serde(default)]
    pub qa_issues: Vec<String>,
    pub index: usize,
    pub label: String,
    /// Relative file path inside `manifest_dir` (e.g. `frames/00_standing.png`).
    pub file: String,
    /// Resolved path on disk (`manifest_dir.join(&file)`).
    pub resolved_path: PathBuf,
    pub size: [u32; 2],
    #[serde(default)]
    pub source_box: Option<[u32; 4]>,
    pub anchor: Option<[f32; 2]>,
    #[serde(default)]
    pub parts: BTreeMap<String, ManifestPart>,
}

/// Measured silhouette motion envelope loaded from `motion_range.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasuredMotionRange {
    /// Original measured JSON. Preserve nulls rather than reporting UI fallback zeroes as evidence.
    #[serde(default)]
    pub raw: Value,
    pub source_file: PathBuf,
    pub still_label: String,
    pub still_height_px: f32,
    pub width_min_delta_px: f32,
    pub width_max_delta_px: f32,
    pub width_max_pose: String,
    pub height_min_delta_px: f32,
    pub height_max_delta_px: f32,
    pub left_min_delta_px: f32,
    pub right_max_delta_px: f32,
    pub top_min_delta_px: f32,
    pub top_max_delta_px: f32,
    pub foot_lift_min_px: f32,
    pub foot_lift_max_px: f32,
    pub foot_lift_max_pose: String,
    pub poses: BTreeMap<String, PoseMotionMetrics>,
}

/// Per-pose measured silhouette metrics from `motion_range.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseMotionMetrics {
    pub label: String,
    pub frame_index: usize,
    pub width_px: f32,
    pub height_px: f32,
    pub foot_lift_px: f32,
    pub delta_width_px: f32,
    pub delta_height_px: f32,
}

/// Measured turnaround registration loaded from `turnaround_registration.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasuredTurnaroundRegistration {
    /// Original registration evidence as loaded, including unavailable fields.
    #[serde(default)]
    pub raw: Value,
    pub source_file: PathBuf,
    pub asset_root: PathBuf,
    pub reference_direction: String,
    pub target_height_px: u32,
    pub max_vertical_drift_px: f32,
    pub before_crown_drift_px: f32,
    pub before_foot_drift_px: f32,
    pub registration_passed: bool,
    pub frames: Vec<ManifestFrame>,
}

impl LoadedManifest {
    /// Load and parse a `manifest.json` from a file or its parent directory.
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let manifest_path = if path.is_dir() {
            path.join("manifest.json")
        } else {
            path.to_path_buf()
        };
        let manifest_dir = manifest_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let text = std::fs::read_to_string(&manifest_path)?;
        let raw: Value = serde_json::from_str(&text)
            .map_err(|e| EngineError::InvalidParams(format!("invalid manifest JSON: {e}")))?;

        let sheet_type = raw
            .get("sheetType")
            .or_else(|| raw.get("kind"))
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let status = raw
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("loaded")
            .to_string();
        let matting_engine = raw
            .get("mattingEngine")
            .and_then(Value::as_str)
            .map(str::to_string);
        let source_image = resolve_manifest_source_image(&raw, &manifest_dir);
        let source_size = parse_u32_pair(raw.get("sourceSize")).filter(|sz| sz[0] > 0 && sz[1] > 0);

        let mut frames = Vec::new();
        let mut part_set = Vec::new();

        if let Some(arr) = raw.get("frames").and_then(Value::as_array) {
            for (idx, item) in arr.iter().enumerate() {
                frames.push(parse_manifest_frame(idx, item, &manifest_dir, &mut part_set));
            }
        }

        apply_fallback_puppet_parts(&raw, &manifest_dir, &mut part_set, &mut frames);

        Ok(Self {
            manifest_dir,
            manifest_path,
            sheet_type,
            status,
            matting_engine,
            source_image,
            source_size,
            frames,
            part_names: part_set,
        })
    }
}

fn parse_u32_pair(val: Option<&Value>) -> Option<[u32; 2]> {
    val.and_then(Value::as_array).map(|s| {
        [
            s.first().and_then(Value::as_u64).unwrap_or(0) as u32,
            s.get(1).and_then(Value::as_u64).unwrap_or(0) as u32,
        ]
    })
}

fn parse_f32_pair(val: Option<&Value>) -> Option<[f32; 2]> {
    val.and_then(Value::as_array).map(|a| {
        [
            a.first().and_then(Value::as_f64).unwrap_or(0.0) as f32,
            a.get(1).and_then(Value::as_f64).unwrap_or(0.0) as f32,
        ]
    })
}

fn parse_qa_issues(item: &Value) -> Vec<String> {
    item.pointer("/qa/issues")
        .and_then(Value::as_array)
        .map(|v| {
            v.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn resolve_manifest_source_image(raw: &Value, manifest_dir: &Path) -> Option<PathBuf> {
    raw.get("sourceImage")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .or_else(|| resolve_parent_manifest_source(raw))
        .or_else(|| {
            let atlas = manifest_dir.join("atlas.png");
            atlas.is_file().then_some(atlas)
        })
}

fn resolve_parent_manifest_source(raw: &Value) -> Option<PathBuf> {
    let first_src = raw
        .get("frames")
        .and_then(Value::as_array)?
        .first()?
        .get("sourceFile")
        .and_then(Value::as_str)?;
    let raw_manifest_dir = Path::new(first_src).parent()?.parent()?;
    let raw_manifest_json = raw_manifest_dir.join("manifest.json");
    if let Ok(txt) = std::fs::read_to_string(&raw_manifest_json)
        && let Ok(v) = serde_json::from_str::<Value>(&txt)
        && let Some(si) = v.get("sourceImage").and_then(Value::as_str)
    {
        let p = PathBuf::from(si);
        if p.is_file() {
            return Some(p);
        }
    }
    let raw_atlas = raw_manifest_dir.join("atlas.png");
    raw_atlas.is_file().then_some(raw_atlas)
}

fn parse_manifest_part(key: &str, pval: &Value, manifest_dir: &Path) -> Option<ManifestPart> {
    let pfile = pval.get("file").and_then(Value::as_str).unwrap_or("").to_string();
    if pfile.is_empty() {
        return None;
    }
    Some(ManifestPart {
        name: key.to_string(),
        resolved_path: manifest_dir.join(&pfile),
        file: pfile,
        size: parse_u32_pair(pval.get("size")).unwrap_or([0, 0]),
        pivot: parse_f32_pair(pval.get("pivot")).unwrap_or([0.0, 0.0]),
        bind_position: parse_f32_pair(pval.get("bindPosition")).unwrap_or([0.0, 0.0]),
    })
}

fn parse_manifest_frame(
    idx: usize,
    item: &Value,
    manifest_dir: &Path,
    part_set: &mut Vec<String>,
) -> ManifestFrame {
    let index = item.get("index").and_then(Value::as_u64).unwrap_or(idx as u64) as usize;
    let label = item
        .get("label")
        .and_then(Value::as_str)
        .unwrap_or("frame")
        .to_string();
    let file = item
        .get("file")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let resolved_path = manifest_dir.join(&file);
    let size = parse_u32_pair(item.get("size")).unwrap_or([0, 0]);
    let source_box = item
        .get("sourceBox")
        .and_then(Value::as_array)
        .map(|b| {
            [
                b.first().and_then(Value::as_u64).unwrap_or(0) as u32,
                b.get(1).and_then(Value::as_u64).unwrap_or(0) as u32,
                b.get(2).and_then(Value::as_u64).unwrap_or(0) as u32,
                b.get(3).and_then(Value::as_u64).unwrap_or(0) as u32,
            ]
        })
        .filter(|b| b[2] > 0 && b[3] > 0);
    let anchor = parse_f32_pair(item.get("anchor"));
    let mut frame_parts = BTreeMap::new();
    if let Some(parts_obj) = item.get("parts").and_then(Value::as_object) {
        for (k, pval) in parts_obj {
            if let Some(part) = parse_manifest_part(k, pval, manifest_dir) {
                if !part_set.contains(k) {
                    part_set.push(k.clone());
                }
                frame_parts.insert(k.clone(), part);
            }
        }
    }
    ManifestFrame {
        qa_issues: parse_qa_issues(item),
        index,
        label,
        file,
        resolved_path,
        size,
        source_box,
        anchor,
        parts: frame_parts,
    }
}

fn apply_fallback_puppet_parts(
    raw: &Value,
    manifest_dir: &Path,
    part_set: &mut Vec<String>,
    frames: &mut [ManifestFrame],
) {
    if !part_set.is_empty() {
        return;
    }
    let Some(puppet_parts) = raw
        .get("puppet")
        .and_then(|p| p.get("parts"))
        .and_then(Value::as_object)
    else {
        return;
    };
    let mut parsed_puppet_parts = BTreeMap::new();
    for (k, pval) in puppet_parts {
        if !part_set.contains(k) {
            part_set.push(k.clone());
        }
        if let Some(part) = parse_manifest_part(k, pval, manifest_dir) {
            parsed_puppet_parts.insert(k.clone(), part);
        }
    }
    if !parsed_puppet_parts.is_empty() {
        for frame in frames {
            if frame.parts.is_empty() {
                frame.parts = parsed_puppet_parts.clone();
            }
        }
    }
}

impl MeasuredMotionRange {
    /// Load and parse a `motion_range.json` produced by `codex_accuracy_optimizer.py`.
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let file_path = if path.is_dir() {
            path.join("motion_range.json")
        } else {
            path.to_path_buf()
        };
        let text = std::fs::read_to_string(&file_path)?;
        let raw: Value = serde_json::from_str(&text)
            .map_err(|e| EngineError::InvalidParams(format!("invalid motion_range JSON: {e}")))?;

        let still = raw.get("still").unwrap_or(&Value::Null);
        let still_label = still
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or("standing")
            .to_string();
        let still_height_px = still.get("heightPx").and_then(Value::as_f64).unwrap_or(0.0) as f32;

        let limits = raw.get("limits").unwrap_or(&Value::Null);
        let f_num = |section: &str, key: &str| -> f32 {
            limits
                .get(section)
                .and_then(|s| s.get(key))
                .and_then(Value::as_f64)
                .unwrap_or(0.0) as f32
        };
        let f_str = |section: &str, key: &str| -> String {
            limits
                .get(section)
                .and_then(|s| s.get(key))
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string()
        };

        let mut poses = BTreeMap::new();
        if let Some(poses_obj) = raw.get("poses").and_then(Value::as_object) {
            for (label, p_val) in poses_obj {
                let frame_index =
                    p_val.get("frameIndex").and_then(Value::as_u64).unwrap_or(0) as usize;
                let metrics = p_val.get("metrics").unwrap_or(&Value::Null);
                let delta = p_val.get("deltaFromStill").unwrap_or(&Value::Null);
                let width_px = metrics
                    .get("widthPx")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0) as f32;
                let height_px = metrics
                    .get("heightPx")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0) as f32;
                let foot_lift_px = metrics
                    .get("footLiftPx")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0) as f32;
                let delta_width_px =
                    delta.get("widthPx").and_then(Value::as_f64).unwrap_or(0.0) as f32;
                let delta_height_px =
                    delta.get("heightPx").and_then(Value::as_f64).unwrap_or(0.0) as f32;
                poses.insert(
                    label.clone(),
                    PoseMotionMetrics {
                        label: label.clone(),
                        frame_index,
                        width_px,
                        height_px,
                        foot_lift_px,
                        delta_width_px,
                        delta_height_px,
                    },
                );
            }
        }

        Ok(Self {
            source_file: file_path,
            still_label,
            raw: raw.clone(),
            still_height_px,
            width_min_delta_px: f_num("widthPx", "minDeltaFromStill"),
            width_max_delta_px: f_num("widthPx", "maxDeltaFromStill"),
            width_max_pose: f_str("widthPx", "maxPose"),
            height_min_delta_px: f_num("heightPx", "minDeltaFromStill"),
            height_max_delta_px: f_num("heightPx", "maxDeltaFromStill"),
            left_min_delta_px: f_num("leftPx", "minDeltaFromStill"),
            right_max_delta_px: f_num("rightPx", "maxDeltaFromStill"),
            top_min_delta_px: f_num("topPx", "minDeltaFromStill"),
            top_max_delta_px: f_num("topPx", "maxDeltaFromStill"),
            foot_lift_min_px: f_num("footLiftPx", "min"),
            foot_lift_max_px: f_num("footLiftPx", "max"),
            foot_lift_max_pose: f_str("footLiftPx", "maxPose"),
            poses,
        })
    }
}

impl MeasuredTurnaroundRegistration {
    /// Load and parse a `turnaround_registration.json` produced by `codex_accuracy_optimizer.py`.
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let file_path = if path.is_dir() {
            path.join("turnaround_registration.json")
        } else {
            path.to_path_buf()
        };
        let text = std::fs::read_to_string(&file_path)?;
        let raw: Value = serde_json::from_str(&text).map_err(|e| {
            EngineError::InvalidParams(format!("invalid turnaround_registration JSON: {e}"))
        })?;

        let parent_dir = file_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let asset_root = raw
            .get("assetRoot")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .unwrap_or_else(|| parent_dir.join("registered_turnaround"));

        let reference_direction = raw
            .get("referenceDirection")
            .and_then(Value::as_str)
            .unwrap_or("front")
            .to_string();
        let target_height_px = raw
            .get("target")
            .and_then(|t| t.get("heightPx"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32;
        let max_vertical_drift_px = raw
            .get("maxVerticalDriftPx")
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as f32;
        let before = raw.get("before").unwrap_or(&Value::Null);
        let before_crown_drift_px = before
            .get("crownDriftPx")
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as f32;
        let before_foot_drift_px = before
            .get("footDriftPx")
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as f32;
        let registration_passed = raw
            .get("rasterRegistrationPassed")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let frames = raw
            .get("frames")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .enumerate()
                    .map(|(idx, item)| parse_turnaround_frame(idx, item, &asset_root))
                    .collect()
            })
            .unwrap_or_default();

        Ok(Self {
            source_file: file_path,
            asset_root,
            raw: raw.clone(),
            reference_direction,
            target_height_px,
            max_vertical_drift_px,
            before_crown_drift_px,
            before_foot_drift_px,
            registration_passed,
            frames,
        })
    }
}

fn parse_turnaround_frame(idx: usize, item: &Value, asset_root: &Path) -> ManifestFrame {
    let index = item.get("index").and_then(Value::as_u64).unwrap_or(idx as u64) as usize;
    let label = item
        .get("label")
        .and_then(Value::as_str)
        .unwrap_or("view")
        .to_string();
    let file = item
        .get("file")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    ManifestFrame {
        qa_issues: parse_qa_issues(item),
        index,
        label,
        resolved_path: asset_root.join(&file),
        file,
        size: parse_u32_pair(item.get("size")).unwrap_or([0, 0]),
        source_box: None,
        anchor: parse_f32_pair(item.get("anchor")),
        parts: BTreeMap::new(),
    }
}

/// Layer contact-sheet rendering: overview panel plus per-part preview cards.
fn draw_rect_border(img: &mut image::RgbaImage, x0: u32, y0: u32, x1: u32, y1: u32, col: [u8; 4]) {
    let x1 = x1.min(img.width().saturating_sub(1));
    let y1 = y1.min(img.height().saturating_sub(1));
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    for x in x0..=x1 {
        img.put_pixel(x, y0, image::Rgba(col));
        img.put_pixel(x, y1, image::Rgba(col));
    }
    for y in y0..=y1 {
        img.put_pixel(x0, y, image::Rgba(col));
        img.put_pixel(x1, y, image::Rgba(col));
    }
}

fn draw_pivot_dot(img: &mut image::RgbaImage, px: i32, py: i32) {
    for dy in -5i32..=5 {
        for dx in -5i32..=5 {
            let x = px + dx;
            let y = py + dy;
            if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
                let d2 = dx * dx + dy * dy;
                if d2 <= 16 {
                    img.put_pixel(x as u32, y as u32, image::Rgba([255, 85, 85, 255]));
                } else if d2 <= 25 {
                    img.put_pixel(x as u32, y as u32, image::Rgba([255, 220, 90, 255]));
                }
            }
        }
    }
}

fn fill_sheet_panel(sheet: &mut image::RgbaImage, x0: u32, y0: u32, w: u32, h: u32, col: [u8; 4]) {
    let max_x = (x0 + w).min(sheet.width());
    let max_y = (y0 + h).min(sheet.height());
    for y in y0..max_y {
        for x in x0..max_x {
            sheet.put_pixel(x, y, image::Rgba(col));
        }
    }
}

fn render_contact_overview_panel(
    sheet: &mut image::RgbaImage,
    comp_crop: &image::RgbaImage,
    comp_origin: [u32; 2],
    parts: &[CutoutRigPart],
) {
    let (lx0, ly0, lw, lh) = (24u32, 24u32, 480u32, 852u32);
    fill_sheet_panel(sheet, lx0, ly0, lw, lh, [28, 34, 46, 255]);
    draw_rect_border(sheet, lx0, ly0, lx0 + lw - 1, ly0 + lh - 1, [68, 84, 112, 255]);

    let cw = comp_crop.width().max(1) as f32;
    let ch = comp_crop.height().max(1) as f32;
    let comp_scale = ((lw - 48) as f32 / cw)
        .min((lh - 48) as f32 / ch)
        .clamp(0.1, 2.5);
    let dw = (cw * comp_scale).round().max(1.0) as u32;
    let dh = (ch * comp_scale).round().max(1.0) as u32;
    let resized_comp =
        image::imageops::resize(comp_crop, dw, dh, image::imageops::FilterType::Triangle);
    let ox = lx0 + (lw.saturating_sub(dw)) / 2;
    let oy = ly0 + (lh.saturating_sub(dh)) / 2;
    image::imageops::overlay(sheet, &resized_comp, ox as i64, oy as i64);

    for part in parts {
        let bx0 = ox as i32
            + (((part.crop_box[0] as i32 - comp_origin[0] as i32) as f32) * comp_scale).round()
                as i32;
        let by0 = oy as i32
            + (((part.crop_box[1] as i32 - comp_origin[1] as i32) as f32) * comp_scale).round()
                as i32;
        let bx1 = ox as i32
            + (((part.crop_box[2] as i32 - comp_origin[0] as i32) as f32) * comp_scale).round()
                as i32;
        let by1 = oy as i32
            + (((part.crop_box[3] as i32 - comp_origin[1] as i32) as f32) * comp_scale).round()
                as i32;
        draw_rect_border(
            sheet,
            bx0.max(lx0 as i32) as u32,
            by0.max(ly0 as i32) as u32,
            bx1.max(0) as u32,
            by1.max(0) as u32,
            [80, 190, 245, 210],
        );
        let px = ox as i32 + ((part.pivot[0] - comp_origin[0] as f32) * comp_scale).round() as i32;
        let py = oy as i32 + ((part.pivot[1] - comp_origin[1] as f32) * comp_scale).round() as i32;
        draw_pivot_dot(sheet, px, py);
    }
}

fn render_contact_layer_cards(sheet: &mut image::RgbaImage, parts: &[CutoutRigPart]) {
    let (gx0, gy0, gw, gh) = (524u32, 24u32, 892u32, 852u32);
    let cols = 3u32;
    let rows = ((parts.len() as u32).div_ceil(cols)).max(1);
    let pad = 12u32;
    let cell_w = (gw.saturating_sub(pad * (cols - 1))) / cols;
    let cell_h = (gh.saturating_sub(pad * (rows - 1))) / rows;

    for (idx, part) in parts.iter().enumerate() {
        let col = (idx as u32) % cols;
        let row = (idx as u32) / cols;
        let cx0 = gx0 + col * (cell_w + pad);
        let cy0 = gy0 + row * (cell_h + pad);
        fill_sheet_panel(sheet, cx0, cy0, cell_w, cell_h, [30, 36, 48, 255]);
        draw_rect_border(
            sheet,
            cx0,
            cy0,
            cx0 + cell_w - 1,
            cy0 + cell_h - 1,
            [72, 92, 124, 255],
        );
        if let Ok(pimg) = image::open(&part.cropped_path) {
            let prgba = pimg.to_rgba8();
            let pw = prgba.width().max(1) as f32;
            let ph = prgba.height().max(1) as f32;
            let pscale = (((cell_w.saturating_sub(28)) as f32 / pw)
                .min((cell_h.saturating_sub(28)) as f32 / ph))
            .clamp(0.1, 2.2);
            let tdw = (pw * pscale).round().max(1.0) as u32;
            let tdh = (ph * pscale).round().max(1.0) as u32;
            let resized_part =
                image::imageops::resize(&prgba, tdw, tdh, image::imageops::FilterType::Triangle);
            let pox = cx0 + (cell_w.saturating_sub(tdw)) / 2;
            let poy = cy0 + (cell_h.saturating_sub(tdh)) / 2;
            image::imageops::overlay(sheet, &resized_part, pox as i64, poy as i64);
            let piv_x = pox as i32 + (part.pivot_uv[0] * tdw as f32).round() as i32;
            let piv_y = poy as i32 + (part.pivot_uv[1] * tdh as f32).round() as i32;
            draw_pivot_dot(sheet, piv_x, piv_y);
        }
    }
}

pub(crate) fn render_layer_contact_sheet(
    comp_crop: &image::RgbaImage,
    comp_origin: [u32; 2],
    parts: &[CutoutRigPart],
    dest: &Path,
) -> bool {
    let mut sheet = image::RgbaImage::from_pixel(1440, 900, image::Rgba([20, 24, 32, 255]));
    render_contact_overview_panel(&mut sheet, comp_crop, comp_origin, parts);
    render_contact_layer_cards(&mut sheet, parts);
    sheet.save(dest).is_ok()
}