//! Scribbles (`craft-scribbles`): Google Flow (Moppet Maker) app-window launcher, handoff asset
//! ingestor (`handoff.json`, `character.png`, `parts-sheet.png`, `pose-sheet.png`), animated
//! SVG builder, and video clip bridge for Magic Studio.
//!
//! Authentication and UI features stay inside the user's signed-in Google Flow Tool Builder
//! (`Moppet Maker`). This crate launches/focuses the dedicated Chrome `--app` window and
//! bridges its downloaded outputs directly into Magic Studio projects.

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Default Google Flow Moppet Maker Tool URL in the user's `Moppet Animation` project (for character animation: parts sheets, pose sheets, sprite cycles).
pub const DEFAULT_MOPPET_MAKER_URL: &str =
    "https://flow.google.com/project/a16c9456-67b1-41f8-a995-79f4918d6737/tool/c4919fed-dcbe-498c-9839-2073ce1c8609?fromViewSource=tools";

/// Default Google Flow Storyboard & Animation Master Pack Tool URL (for one-pass storyboard page & slot asset generation/recreation with ID name markers).
pub const DEFAULT_STORYBOARD_PACK_TOOL_URL: &str =
    "https://flow.google.com/project/a16c9456-67b1-41f8-a995-79f4918d6737/tool/191f61c7-21f6-434f-b679-4920807e2d62?fromViewSource=tools";

/// Default Google Flow project URL (`Moppet Animation`).
pub const DEFAULT_FLOW_PROJECT_URL: &str = "https://flow.google.com/project/a16c9456-67b1-41f8-a995-79f4918d6737";

#[derive(Debug, thiserror::Error)]
pub enum ScribblesError {
    #[error("{0}")]
    Invalid(String),
    #[error("external tool `{0}` failed: {1}")]
    Tool(String, String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ScribblesError>;

/// Parsed or discovered Moppet Maker handoff bundle from `~/Downloads` or an explicit directory.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IngestedMoppetBundle {
    pub character_id: String,
    pub character_name: String,
    pub view: String,
    pub action: String,
    pub grid: (u32, u32),
    pub source_dir: PathBuf,
    pub dest_dir: PathBuf,
    pub handoff_json: Option<PathBuf>,
    pub parts_json: Option<PathBuf>,
    pub character_png: Option<PathBuf>,
    pub parts_sheet_png: Option<PathBuf>,
    pub pose_sheet_png: Option<PathBuf>,
    pub animated_svg: Option<PathBuf>,
    pub metadata: Value,
}

/// Find an executable on `PATH` or in `~/.local/bin`.
pub fn find_binary(name: &str) -> Option<PathBuf> {
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let local_bin = PathBuf::from(home).join(".local/bin").join(name);
        if local_bin.is_file() {
            return Some(local_bin);
        }
    }
    None
}

/// Locate the user's Chrome / Chromium executable for `--app=<url>` window mode.
pub fn find_chrome_binary() -> Option<PathBuf> {
    for candidate in ["google-chrome-stable", "google-chrome", "chromium", "chromium-browser", "brave-browser"] {
        if let Some(path) = find_binary(candidate) {
            return Some(path);
        }
    }
    None
}

/// Launch or focus Google Flow's Moppet Maker in a dedicated Chrome `--app` window.
/// Because it uses the user's normal Chrome profile, Google handles authentication, cookies,
/// and 0-credit Ultra subscription access directly.
pub fn open_moppet_maker_window(custom_url: Option<&str>) -> Result<Value> {
    let url = custom_url.map(str::trim).filter(|s| !s.is_empty()).unwrap_or(DEFAULT_MOPPET_MAKER_URL);
    if !url.starts_with("https://flow.google.com/") && !url.starts_with("https://labs.google/") {
        return Err(ScribblesError::Invalid(format!("Scribbles app window only opens Google Flow URLs (`https://flow.google.com/...`), got `{url}`")));
    }

    if let Some(hyprctl) = find_binary("hyprctl")
        && let Ok(out) = Command::new(&hyprctl).args(["clients", "-j"]).output()
        && out.status.success()
        && let Ok(clients) = serde_json::from_slice::<Value>(&out.stdout)
        && let Some(arr) = clients.as_array()
    {
        for client in arr {
            let class = client["class"].as_str().unwrap_or_default();
            let title = client["title"].as_str().unwrap_or_default();
            if class.contains("flow.google.com__project_") || title.contains("Moppet") {
                let addr = client["address"].as_str().unwrap_or_default();
                if !addr.is_empty() {
                    let _ = Command::new(&hyprctl).args(["dispatch", "focuswindow", &format!("address:{addr}")]).output();
                    return Ok(json!({
                        "ok": true,
                        "launched": false,
                        "focused": true,
                        "window_open": true,
                        "address": addr,
                        "class": class,
                        "url": url,
                        "project_url": DEFAULT_FLOW_PROJECT_URL,
                    }));
                }
            }
        }
    }

    if let Some(chrome) = find_chrome_binary() {
        let app_arg = format!("--app={url}");
        Command::new(&chrome)
            .arg(&app_arg)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| ScribblesError::Tool(chrome.display().to_string(), e.to_string()))?;
        return Ok(json!({
            "ok": true,
            "launched": true,
            "mode": "chrome-app-window",
            "browser": chrome,
            "url": url,
            "project_url": DEFAULT_FLOW_PROJECT_URL,
        }));
    }

    if let Some(xdg) = find_binary("xdg-open") {
        Command::new(&xdg)
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| ScribblesError::Tool("xdg-open".into(), e.to_string()))?;
        return Ok(json!({
            "ok": true,
            "launched": true,
            "mode": "xdg-open",
            "browser": xdg,
            "url": url,
        }));
    }

    Err(ScribblesError::Tool("chrome".into(), "No Chrome/Chromium or xdg-open binary found on PATH".into()))
}

/// Ensure a downloaded image file is a valid PNG (Google Flow sometimes saves JPEG bytes with a `.png` extension).
pub fn ensure_valid_png(src: &Path, dst: &Path) -> Result<(u32, u32)> {
    let bytes = fs::read(src)?;
    if bytes.len() < 8 {
        return Err(ScribblesError::Invalid(format!("Image file {} is empty or truncated", src.display())));
    }
    let img = image::load_from_memory(&bytes).map_err(|e| ScribblesError::Invalid(format!("Could not decode image {}: {e}", src.display())))?;
    let width = img.width();
    let height = img.height();
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        if src != dst {
            fs::copy(src, dst)?;
        }
    } else {
        img.save_with_format(dst, image::ImageFormat::Png).map_err(|e| ScribblesError::Invalid(format!("Failed to write PNG {}: {e}", dst.display())))?;
    }
    Ok((width, height))
}

/// Locate the most recently modified file matching any of the candidate prefixes in `dir`.
fn newest_matching_file(dir: &Path, prefixes: &[&str], ext: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let lower = name.to_ascii_lowercase();
        if !lower.ends_with(ext) {
            continue;
        }
        if prefixes.iter().any(|p| lower.starts_with(p)) {
            let mtime = entry.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            if best.as_ref().is_none_or(|(t, _)| mtime > *t) {
                best = Some((mtime, path));
            }
        }
    }
    best.map(|(_, p)| p)
}

/// Sanitize a string into a clean snake_case identifier matching `magic_engine::workspace::sanitize_asset_id`.
pub fn sanitize_id(raw: &str) -> String {
    let mut out = String::new();
    let mut last_sep = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_sep = false;
        } else if !last_sep && !out.is_empty() {
            out.push('_');
            last_sep = true;
        }
    }
    out.trim_matches('_').to_string()
}

/// Scan `source_dir` (defaults to `~/Downloads`) for Moppet Maker outputs (`handoff.json`,
/// `character.png`, `parts-sheet.png`, `pose-sheet.png`) and ingest them into
/// `<project_root>/assets/characters/<character_id>/`.
pub fn ingest_moppet_downloads(project_root: &Path, source_dir: Option<&Path>, requested_character_id: Option<&str>) -> Result<IngestedMoppetBundle> {
    let downloads = match source_dir {
        Some(d) => d.to_path_buf(),
        None => {
            let home = std::env::var_os("HOME").ok_or_else(|| ScribblesError::Invalid("HOME is unset".into()))?;
            PathBuf::from(home).join("Downloads")
        }
    };
    if !downloads.is_dir() {
        return Err(ScribblesError::Invalid(format!("Downloads directory {} does not exist", downloads.display())));
    }

    let handoff_src = newest_matching_file(&downloads, &["handoff", "moppet-handoff"], ".json");
    let mut meta = Value::Object(Default::default());
    if let Some(ref h_path) = handoff_src
        && let Ok(raw) = fs::read_to_string(h_path)
        && let Ok(parsed) = serde_json::from_str::<Value>(&raw)
    {
        meta = parsed;
    }

    let inferred_name = requested_character_id
        .filter(|s| !s.trim().is_empty())
        .or_else(|| meta["characterName"].as_str())
        .or_else(|| meta["name"].as_str())
        .or_else(|| meta["recipe"]["animal"].as_str())
        .unwrap_or("moppet");
    let char_id = {
        let clean = sanitize_id(inferred_name);
        if clean.is_empty() { "moppet".to_string() } else { clean }
    };

    let dest_dir = project_root.join("assets/characters").join(&char_id);
    fs::create_dir_all(&dest_dir)?;

    let view = meta["view"]
        .as_str()
        .or_else(|| meta["direction"].as_str())
        .map(|v| {
            let l = v.to_ascii_lowercase();
            if l.contains("right") || l.contains("left") || l.contains("side") {
                "side"
            } else if l.contains("back") || l.contains("up") {
                "back"
            } else {
                "front"
            }
        })
        .unwrap_or("front")
        .to_string();

    let action = meta["action"]
        .as_str()
        .or_else(|| meta["spriteAction"].as_str())
        .map(sanitize_id)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| if view == "side" { "walk_right".into() } else { "walk_down".into() });

    let cols = meta["columns"].as_u64().or_else(|| meta["grid"][0].as_u64()).unwrap_or(2) as u32;
    let rows = meta["rows"].as_u64().or_else(|| meta["grid"][1].as_u64()).unwrap_or(2) as u32;

    let mut bundle = IngestedMoppetBundle {
        character_id: char_id.clone(),
        character_name: inferred_name.to_string(),
        view: view.clone(),
        action: action.clone(),
        grid: (cols.max(1), rows.max(1)),
        source_dir: downloads.clone(),
        dest_dir: dest_dir.clone(),
        handoff_json: None,
        parts_json: None,
        character_png: None,
        parts_sheet_png: None,
        pose_sheet_png: None,
        animated_svg: None,
        metadata: meta.clone(),
    };

    if let Some(src) = handoff_src {
        let dst = dest_dir.join("handoff.json");
        if src != dst {
            let _ = fs::copy(&src, &dst);
        }
        bundle.handoff_json = Some(dst);
    }

    // Ingest `parts.json` (either standalone download or embedded in `handoff.json` under `parts` / `rig`)
    let view_dir = dest_dir.join(&view);
    fs::create_dir_all(&view_dir)?;
    if let Some(src) = newest_matching_file(&downloads, &["parts", "moppet-parts-rig"], ".json") {
        let dst = view_dir.join("parts.json");
        if src != dst {
            let _ = fs::copy(&src, &dst);
        }
        bundle.parts_json = Some(dst);
    } else if meta.get("parts").and_then(Value::as_array).is_some() {
        let dst = view_dir.join("parts.json");
        let parts_doc = json!({
            "image": format!("{char_id}-{view}-drawing.png"),
            "view": view,
            "size": meta.get("size").cloned().unwrap_or_else(|| json!([1024, 1024])),
            "skeleton": meta.get("skeleton").and_then(Value::as_str).unwrap_or("plush-9"),
            "status": "ingested from Flow Moppet Maker; ready for character.cut and character.rig",
            "parts": meta["parts"],
        });
        if let Ok(bytes) = serde_json::to_vec_pretty(&parts_doc) {
            let _ = craft_atomic::atomic_write(&dst, &bytes);
            bundle.parts_json = Some(dst);
        }
    }

    if let Some(src) = newest_matching_file(&downloads, &["character", "moppet-character"], ".png") {
        let dst = dest_dir.join("flow-original.png");
        if ensure_valid_png(&src, &dst).is_ok() {
            let view_drawing = view_dir.join(format!("{char_id}-{view}-drawing.png"));
            let _ = fs::copy(&dst, &view_drawing);
            bundle.character_png = Some(dst);
        }
    }

    if let Some(src) = newest_matching_file(&downloads, &["parts-sheet", "parts_sheet", "moppet-parts"], ".png") {
        let dst = dest_dir.join("parts-sheet.png");
        if ensure_valid_png(&src, &dst).is_ok() {
            bundle.parts_sheet_png = Some(dst);
        }
    }

    if let Some(src) = newest_matching_file(&downloads, &["pose-sheet", "pose_sheet", "sprite-sheet", "moppet-poses"], ".png") {
        let dst = dest_dir.join("pose-sheet.png");
        if ensure_valid_png(&src, &dst).is_ok() {
            bundle.pose_sheet_png = Some(dst.clone());
            let svg_out = dest_dir.join("sprites").join(&action).join(format!("{char_id}-{action}.svg"));
            if let Ok(svg_path) = build_animated_svg(&dst, &svg_out, 6) {
                bundle.animated_svg = Some(svg_path);
            }
        }
    }

    Ok(bundle)
}

/// Slice a 2×2 chroma-green (`#00FF00`) pose sheet into a looping animated SVG via `flow-clip --mode svg`.
pub fn build_animated_svg(sheet_png: &Path, out_svg: &Path, fps: u64) -> Result<PathBuf> {
    if !sheet_png.is_file() {
        return Err(ScribblesError::Invalid(format!("Pose sheet {} does not exist", sheet_png.display())));
    }
    if let Some(parent) = out_svg.parent() {
        fs::create_dir_all(parent)?;
    }
    let cli = find_binary("flow-clip").ok_or_else(|| ScribblesError::Tool("flow-clip".into(), "flow-clip not found on PATH".into()))?;
    let output = Command::new(&cli)
        .args(["--mode", "svg", "--sheet"])
        .arg(sheet_png)
        .args(["--fps", &fps.to_string(), "--out"])
        .arg(out_svg)
        .output()
        .map_err(|e| ScribblesError::Tool("flow-clip".into(), e.to_string()))?;
    if !output.status.success() {
        return Err(ScribblesError::Tool(
            "flow-clip".into(),
            format!("{}{}", String::from_utf8_lossy(&output.stderr), String::from_utf8_lossy(&output.stdout)),
        ));
    }
    Ok(out_svg.to_path_buf())
}

/// Parameters for generating a short AI video clip through Google Flow (`flow-clip --mode video`).
pub struct VideoClipRequest<'a> {
    pub prompt: &'a str,
    pub out_mp4: &'a Path,
    pub model: Option<&'a str>,
    pub aspect: Option<&'a str>,
    pub duration: Option<&'a str>,
    pub resolution: Option<&'a str>,
    pub ingredient: Option<&'a str>,
}

/// Generate a short AI video clip (`.mp4`) via Google Flow (`flow-clip --mode video`).
pub fn generate_video_clip(req: &VideoClipRequest<'_>) -> Result<PathBuf> {
    if let Some(parent) = req.out_mp4.parent() {
        fs::create_dir_all(parent)?;
    }
    let cli = find_binary("flow-clip").ok_or_else(|| ScribblesError::Tool("flow-clip".into(), "flow-clip not found on PATH".into()))?;
    let mut cmd = Command::new(&cli);
    cmd.args(["--mode", "video", "--prompt", req.prompt, "--out"]).arg(req.out_mp4);
    if let Some(m) = req.model.filter(|v| !v.trim().is_empty()) {
        cmd.args(["--model", m]);
    }
    if let Some(a) = req.aspect.filter(|v| !v.trim().is_empty()) {
        cmd.args(["--aspect", a]);
    }
    if let Some(d) = req.duration.filter(|v| !v.trim().is_empty()) {
        cmd.args(["--duration", d]);
    }
    if let Some(r) = req.resolution.filter(|v| !v.trim().is_empty()) {
        cmd.args(["--resolution", r]);
    }
    if let Some(i) = req.ingredient.filter(|v| !v.trim().is_empty()) {
        cmd.args(["--ingredient", i]);
    }
    let output = cmd.output().map_err(|e| ScribblesError::Tool("flow-clip".into(), e.to_string()))?;
    if !output.status.success() {
        return Err(ScribblesError::Tool(
            "flow-clip".into(),
            format!("exit {:?}: {}{}", output.status.code(), String::from_utf8_lossy(&output.stderr), String::from_utf8_lossy(&output.stdout)),
        ));
    }
    Ok(req.out_mp4.to_path_buf())
}

/// Extracted single part from a chroma-green (`#00FF00`) `parts-sheet.png`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtractedPart {
    pub id: String,
    pub parent: Option<String>,
    pub file: PathBuf,
    pub bounds: [u32; 4],
    pub joint: [f64; 2],
    pub pins: Vec<[f64; 2]>,
    pub outline: Vec<[u32; 2]>,
}

/// Check if an RGBA pixel is chroma-green background (`#00FF00` with tolerance for anti-aliased edges).
fn is_chroma_green(r: u8, g: u8, b: u8, a: u8) -> bool {
    if a < 16 {
        return true;
    }
    let rf = i32::from(r);
    let gf = i32::from(g);
    let bf = i32::from(b);
    (gf > 110 && (gf - rf) > 32 && (gf - bf) > 32) || (gf > 65 && (gf - rf) > 42 && (gf - bf) > 42)
}

/// De-spill green fringe on semi-transparent/edge pixels after chroma-keying.
fn despill_rgba(r: u8, g: u8, b: u8, a: u8) -> [u8; 4] {
    if is_chroma_green(r, g, b, a) {
        return [0, 0, 0, 0];
    }
    let rf = i32::from(r);
    let gf = i32::from(g);
    let bf = i32::from(b);
    let max_rb = r.max(b);
    let excess_g = gf - i32::from(max_rb);
    if excess_g > 12 {
        let avg_rb = ((rf + bf) / 2).clamp(0, 255) as u8;
        let clean_a = if excess_g > 35 { a.saturating_sub(((excess_g - 35) * 3).min(255) as u8) } else { a };
        return [r, avg_rb, b, clean_a];
    }
    [r, g, b, a]
}

/// Decode an image file (PNG or JPEG, regardless of file extension) into an RGBA8 buffer.
fn open_rgba_flexible(path: &Path) -> Result<image::RgbaImage> {
    let bytes = fs::read(path).map_err(ScribblesError::Io)?;
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| ScribblesError::Invalid(format!("Failed to guess format of {}: {e}", path.display())))?;
    Ok(reader.decode().map_err(|e| ScribblesError::Invalid(format!("Failed to decode {}: {e}", path.display())))?.to_rgba8())
}

/// Cleanly extract individual 2D sprite frames from a grid sprite sheet (`cols x rows`).
///
/// 1. Keys out chroma-green (`#00FF00`) or top-left corner background color and de-spills green fringes.
/// 2. Strips full-width horizontal floor/divider lines that bleed across the bottom of cells.
fn find_best_cell_component(mask: &[bool], gw: usize, gh: usize) -> (Vec<usize>, usize) {
    let mut visited = vec![false; gw * gh];
    let mut comp_id = vec![0usize; gw * gh];
    let mut best_comp = 0usize;
    let mut best_score = 0.0_f64;
    let mut cur_id = 0usize;

    for gy in 0..gh {
        for gx in 0..gw {
            let start = gy * gw + gx;
            if !mask[start] || visited[start] {
                continue;
            }
            cur_id += 1;
            let mut stack = vec![(gx, gy)];
            visited[start] = true;
            let mut cells = Vec::new();
            let mut sum_x = 0.0_f64;
            while let Some((cx, cy)) = stack.pop() {
                comp_id[cy * gw + cx] = cur_id;
                cells.push((cx, cy));
                sum_x += cx as f64;
                for (nx, ny) in [(cx.wrapping_sub(1), cy), (cx + 1, cy), (cx, cy.wrapping_sub(1)), (cx, cy + 1)] {
                    if nx < gw && ny < gh {
                        let ni = ny * gw + nx;
                        if mask[ni] && !visited[ni] {
                            visited[ni] = true;
                            stack.push((nx, ny));
                        }
                    }
                }
            }
            let mean_x = sum_x / (cells.len().max(1) as f64);
            let center_dist = ((mean_x - (gw as f64 * 0.5)).abs() / (gw as f64 * 0.5)).min(1.0);
            let centrality = 1.15 - 0.75 * center_dist;
            let score = (cells.len() as f64) * centrality;
            if score > best_score {
                best_score = score;
                best_comp = cur_id;
            }
        }
    }
    (comp_id, best_comp)
}

fn center_cell_frame(cell_buf: &image::RgbaImage, bounds: [u32; 4], cw: u32, ch: u32, lock_baseline: bool) -> image::RgbaImage {
    let [min_x, min_y, max_x, max_y] = bounds;
    let mut final_frame = image::RgbaImage::new(cw, ch);
    if min_x <= max_x && min_y <= max_y {
        let bw = max_x - min_x + 1;
        let bh = max_y - min_y + 1;
        let dst_x0 = (cw.saturating_sub(bw)) / 2;
        let dst_y0 = if lock_baseline { ch.saturating_sub(bh).saturating_sub(4) } else { (ch.saturating_sub(bh)) / 2 };
        for sy in min_y..=max_y {
            for sx in min_x..=max_x {
                let px = *cell_buf.get_pixel(sx, sy);
                if px.0[3] > 0 {
                    let dx = dst_x0 + (sx - min_x);
                    let dy = dst_y0 + (sy - min_y);
                    if dx < cw && dy < ch {
                        final_frame.put_pixel(dx, dy, px);
                    }
                }
            }
        }
    }
    final_frame
}

fn compute_divider_rows<F>(img: &image::RgbaImage, w: u32, h: u32, is_bg: &F) -> Vec<bool>
where
    F: Fn(u8, u8, u8, u8) -> bool,
{
    let mut full_row_fg = vec![0u32; h as usize];
    for y in 0..h {
        let mut count = 0u32;
        for x in 0..w {
            let p = img.get_pixel(x, y).0;
            if !is_bg(p[0], p[1], p[2], p[3]) {
                count += 1;
            }
        }
        full_row_fg[y as usize] = count;
    }
    let mut is_divider_row = vec![false; h as usize];
    for y in 0..h {
        if full_row_fg[y as usize] * 100 > w * 92 {
            let y_above = y.saturating_sub(3) as usize;
            let y_below = (y + 3).min(h - 1) as usize;
            if full_row_fg[y_above] * 100 < w * 70 && full_row_fg[y_below] * 100 < w * 70 {
                for ny in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                    is_divider_row[ny as usize] = true;
                }
            }
        }
    }
    is_divider_row
}

fn compute_col_splits<F>(img: &image::RgbaImage, w: u32, h: u32, cols: u32, cw: u32, is_divider_row: &[bool], is_bg: &F) -> Vec<u32>
where
    F: Fn(u8, u8, u8, u8) -> bool,
{
    let mut col_fg = vec![0u32; w as usize];
    for x in 0..w {
        let mut count = 0u32;
        for y in 0..h {
            if is_divider_row[y as usize] {
                continue;
            }
            let p = img.get_pixel(x, y).0;
            if !is_bg(p[0], p[1], p[2], p[3]) {
                count += 1;
            }
        }
        col_fg[x as usize] = count;
    }
    let mut col_splits = Vec::with_capacity((cols + 1) as usize);
    col_splits.push(0u32);
    for c in 1..cols {
        let ideal = c * cw;
        let search_r = (cw / 8).clamp(8, 32);
        let x0 = ideal.saturating_sub(search_r);
        let x1 = (ideal + search_r).min(w - 1);
        let mut best_x = ideal;
        let mut best_cost = u32::MAX;
        for x in x0..=x1 {
            let dist_penalty = x.abs_diff(ideal) / 4;
            let cost = col_fg[x as usize] * 4 + dist_penalty;
            if cost < best_cost {
                best_cost = cost;
                best_x = x;
            }
        }
        col_splits.push(best_x);
    }
    col_splits.push(w);
    col_splits
}

/// 3. Isolates the primary central connected component in each `(col, row)` cell so neighboring
///    frames' snouts or tails that cross the cell boundary are removed.
/// 4. Re-centers horizontally and locks the baseline (for grounded actions) or centers vertically
///    (for airborne actions) inside `cw x ch` frame PNGs written via `craft_atomic::atomic_write`.
pub fn clean_and_crop_sprite_sheet(sheet_png: &Path, frames_dir: &Path, grid: (u32, u32), lock_baseline: bool) -> Result<Vec<PathBuf>> {
    let img = open_rgba_flexible(sheet_png)?;
    let (w, h) = img.dimensions();
    let (cols, rows) = (grid.0.max(1), grid.1.max(1));
    let (cw, ch) = ((w / cols).max(1), (h / rows).max(1));

    let corner = img.get_pixel(2.min(w - 1), 2.min(h - 1)).0;
    let corner_is_green = is_chroma_green(corner[0], corner[1], corner[2], corner[3]);
    let is_bg = |r: u8, g: u8, b: u8, a: u8| -> bool {
        if a < 20 || is_chroma_green(r, g, b, a) {
            return true;
        }
        if !corner_is_green && corner[3] > 200 {
            let dr = (i32::from(r) - i32::from(corner[0])).abs();
            let dg = (i32::from(g) - i32::from(corner[1])).abs();
            let db = (i32::from(b) - i32::from(corner[2])).abs();
            if dr + dg + db < 54 {
                return true;
            }
        }
        false
    };

    if frames_dir.is_dir() {
        let _ = fs::remove_dir_all(frames_dir);
    }
    fs::create_dir_all(frames_dir)?;

    let is_divider_row = compute_divider_rows(&img, w, h, &is_bg);
    let col_splits = compute_col_splits(&img, w, h, cols, cw, &is_divider_row, &is_bg);

    let mut out = Vec::with_capacity((cols * rows) as usize);
    for idx in 0..(cols * rows) {
        let col = idx % cols;
        let row = idx / cols;
        let ox = col_splits[col as usize];
        let cell_w = (col_splits[(col + 1) as usize].saturating_sub(ox)).max(1);
        let oy = row * ch;

        let step = 4u32;
        let gw = cell_w.div_ceil(step) as usize;
        let gh = ch.div_ceil(step) as usize;
        let mut mask = vec![false; gw * gh];
        for gy in 0..gh {
            for gx in 0..gw {
                let mut fg_votes = 0u32;
                for dy in 0..step {
                    for dx in 0..step {
                        let lx = gx as u32 * step + dx;
                        let py = (oy + gy as u32 * step + dy).min(h - 1);
                        if lx < cell_w && !is_divider_row[py as usize] {
                            let p = img.get_pixel((ox + lx).min(w - 1), py).0;
                            if !is_bg(p[0], p[1], p[2], p[3]) {
                                fg_votes += 1;
                            }
                        }
                    }
                }
                mask[gy * gw + gx] = fg_votes >= 3;
            }
        }

        let (comp_id, best_comp) = find_best_cell_component(&mask, gw, gh);

        let mut cell_buf = image::RgbaImage::new(cell_w, ch);
        let mut bounds = [cell_w, ch, 0u32, 0u32];

        for ly in 0..ch {
            let py = (oy + ly).min(h - 1);
            if is_divider_row[py as usize] {
                continue;
            }
            for lx in 0..cell_w {
                let p = img.get_pixel((ox + lx).min(w - 1), py).0;
                if is_bg(p[0], p[1], p[2], p[3]) {
                    continue;
                }
                let (gx, gy) = ((lx / step) as usize, (ly / step) as usize);
                let keep = best_comp > 0
                    && (gy.saturating_sub(1)..=(gy + 1).min(gh - 1))
                        .any(|ny| (gx.saturating_sub(1)..=(gx + 1).min(gw - 1)).any(|nx| comp_id[ny * gw + nx] == best_comp));
                if !keep {
                    continue;
                }
                let clean = despill_rgba(p[0], p[1], p[2], p[3]);
                if clean[3] > 0 {
                    cell_buf.put_pixel(lx, ly, image::Rgba(clean));
                    bounds[0] = bounds[0].min(lx);
                    bounds[1] = bounds[1].min(ly);
                    bounds[2] = bounds[2].max(lx);
                    bounds[3] = bounds[3].max(ly);
                }
            }
        }

        let final_frame = center_cell_frame(&cell_buf, bounds, cw, ch, lock_baseline);
        let frame_path = frames_dir.join(format!("frame_{:04}.png", idx + 1));
        let mut png_bytes = std::io::Cursor::new(Vec::new());
        final_frame
            .write_to(&mut png_bytes, image::ImageFormat::Png)
            .map_err(|e| ScribblesError::Invalid(format!("Failed to encode {}: {e}", frame_path.display())))?;
        craft_atomic::atomic_write(&frame_path, &png_bytes.into_inner())?;
        out.push(frame_path);
    }

    Ok(out)
}

type Island = ([u32; 4], Vec<(u32, u32)>);

struct AssignedPart {
    part_id: &'static str,
    parent: Option<&'static str>,
    target_joint: [f64; 2],
    pivot_uv: [f64; 2],
    owner_idx: usize,
    bbox: [u32; 4],
}

struct PartsGrid {
    gw: u32,
    gh: u32,
    cell_owner: Vec<usize>,
    boxes: Vec<(usize, [u32; 4])>,
}

fn discover_parts_islands(decoded: &image::RgbaImage, w: u32, h: u32, step: u32) -> PartsGrid {
    let gw = (w / step).max(1);
    let gh = (h / step).max(1);
    let mut mask = vec![false; (gw * gh) as usize];
    for gy in 0..gh {
        for gx in 0..gw {
            let px = decoded.get_pixel(gx * step, gy * step);
            if !is_chroma_green(px[0], px[1], px[2], px[3]) {
                mask[(gy * gw + gx) as usize] = true;
            }
        }
    }

    let mut visited = vec![false; (gw * gh) as usize];
    let mut islands: Vec<Island> = Vec::new();
    for gy in 0..gh {
        for gx in 0..gw {
            let idx = (gy * gw + gx) as usize;
            if !mask[idx] || visited[idx] {
                continue;
            }
            let mut stack = vec![(gx, gy)];
            let mut cells = Vec::new();
            visited[idx] = true;
            let (mut min_x, mut min_y, mut max_x, mut max_y) = (gx, gy, gx, gy);
            while let Some((cx, cy)) = stack.pop() {
                cells.push((cx, cy));
                min_x = min_x.min(cx);
                min_y = min_y.min(cy);
                max_x = max_x.max(cx);
                max_y = max_y.max(cy);
                for (nx, ny) in [(cx.saturating_sub(1), cy), ((cx + 1).min(gw - 1), cy), (cx, cy.saturating_sub(1)), (cx, (cy + 1).min(gh - 1))] {
                    let nidx = (ny * gw + nx) as usize;
                    if mask[nidx] && !visited[nidx] {
                        visited[nidx] = true;
                        stack.push((nx, ny));
                    }
                }
            }
            if cells.len() >= 35 {
                let x0 = (min_x * step).saturating_sub(step);
                let y0 = (min_y * step).saturating_sub(step);
                let x1 = ((max_x + 2) * step).min(w);
                let y1 = ((max_y + 2) * step).min(h);
                islands.push(([x0, y0, x1 - x0, y1 - y0], cells));
            }
        }
    }

    islands.sort_by_key(|(_, cells)| std::cmp::Reverse(cells.len()));
    let mut cell_owner = vec![usize::MAX; (gw * gh) as usize];
    for (owner_idx, (_, cells)) in islands.iter().enumerate() {
        for &(cx, cy) in cells {
            cell_owner[(cy * gw + cx) as usize] = owner_idx;
        }
    }
    let boxes: Vec<(usize, [u32; 4])> = islands.iter().enumerate().map(|(i, (b, _))| (i, *b)).collect();
    PartsGrid { gw, gh, cell_owner, boxes }
}

fn assign_plush9_parts(boxes: &[(usize, [u32; 4])]) -> Vec<AssignedPart> {
    let mut assigned = Vec::new();
    let Some(&(torso_owner, torso_box)) = boxes.first() else {
        return assigned;
    };
    assigned.push(AssignedPart {
        part_id: "torso",
        parent: None,
        target_joint: [500.0, 560.0],
        pivot_uv: [0.50, 0.50],
        owner_idx: torso_owner,
        bbox: torso_box,
    });
    let mut rest: Vec<(usize, [u32; 4])> = boxes.iter().skip(1).take(8).copied().collect();
    rest.sort_by_key(|(_, b)| b[2] * b[3]);
    if rest.len() >= 2 {
        let mut ears = [rest[0], rest[1]];
        ears.sort_by_key(|(_, b)| b[0] + b[1] / 2);
        assigned.push(AssignedPart {
            part_id: "ear_L",
            parent: Some("head"),
            target_joint: [560.0, 225.0],
            pivot_uv: [0.55, 0.85],
            owner_idx: ears[0].0,
            bbox: ears[0].1,
        });
        assigned.push(AssignedPart {
            part_id: "ear_R",
            parent: Some("head"),
            target_joint: [680.0, 225.0],
            pivot_uv: [0.45, 0.85],
            owner_idx: ears[1].0,
            bbox: ears[1].1,
        });
        rest.drain(0..2);
    }
    rest.sort_by_key(|(_, b)| b[1] + b[3] / 2);
    if rest.len() >= 2 {
        let mut top2 = [rest[0], rest[1]];
        top2.sort_by_key(|(_, b)| b[0]);
        assigned.push(AssignedPart {
            part_id: "tail",
            parent: Some("torso"),
            target_joint: [360.0, 520.0],
            pivot_uv: [0.88, 0.35],
            owner_idx: top2[0].0,
            bbox: top2[0].1,
        });
        assigned.push(AssignedPart {
            part_id: "head",
            parent: Some("torso"),
            target_joint: [615.0, 480.0],
            pivot_uv: [0.25, 0.82],
            owner_idx: top2[1].0,
            bbox: top2[1].1,
        });
        rest.drain(0..2);
    }
    if rest.len() >= 2 {
        let mut mid2 = [rest[0], rest[1]];
        mid2.sort_by_key(|(_, b)| b[0]);
        assigned.push(AssignedPart {
            part_id: "arm_L",
            parent: Some("torso"),
            target_joint: [455.0, 535.0],
            pivot_uv: [0.70, 0.22],
            owner_idx: mid2[0].0,
            bbox: mid2[0].1,
        });
        assigned.push(AssignedPart {
            part_id: "arm_R",
            parent: Some("torso"),
            target_joint: [605.0, 540.0],
            pivot_uv: [0.22, 0.22],
            owner_idx: mid2[1].0,
            bbox: mid2[1].1,
        });
        rest.drain(0..2);
    }
    if rest.len() >= 2 {
        let mut bot2 = [rest[0], rest[1]];
        bot2.sort_by_key(|(_, b)| b[0]);
        assigned.push(AssignedPart {
            part_id: "leg_L",
            parent: Some("torso"),
            target_joint: [435.0, 665.0],
            pivot_uv: [0.48, 0.22],
            owner_idx: bot2[0].0,
            bbox: bot2[0].1,
        });
        assigned.push(AssignedPart {
            part_id: "leg_R",
            parent: Some("torso"),
            target_joint: [575.0, 665.0],
            pivot_uv: [0.48, 0.22],
            owner_idx: bot2[1].0,
            bbox: bot2[1].1,
        });
    }
    assigned
}

/// Reassemble an exploded `#00FF00` chroma-green `parts-sheet.png` into:
/// 1. Individual transparent full-canvas layer PNGs (`<view_dir>/layers/<part_id>.png`) positioned
///    on the character's skeleton so `character.cut` / `character.rig` can assemble a `.psd`, `.pcraft`, and `.ecproj`.
/// 2. `<view_dir>/parts.json` with `plush-9` hierarchy (`torso`, `head`, `ear_L`, `ear_R`, `arm_L`, `arm_R`,
///    `leg_L`, `leg_R`, `tail`), `[x, y]` joint pivots, and `pins` (`puppet.addPin` / `puppet.follow`).
pub fn reassemble_parts_sheet(parts_sheet_png: &Path, view_dir: &Path, char_id: &str, view: &str) -> Result<Value> {
    let decoded = open_rgba_flexible(parts_sheet_png)?;
    let (w, h) = decoded.dimensions();
    fs::create_dir_all(view_dir)?;
    let layers_dir = view_dir.join("layers");
    fs::create_dir_all(&layers_dir)?;

    let step = 4u32;
    let PartsGrid { gw, gh, cell_owner, boxes } = discover_parts_islands(&decoded, w, h, step);
    let assigned = assign_plush9_parts(&boxes);

    let mut extracted = Vec::new();
    let mut assembled_canvas = image::RgbaImage::new(w, h);

    for AssignedPart { part_id, parent, target_joint, pivot_uv, owner_idx, bbox } in &assigned {
        let [bx, by, bw, bh] = *bbox;

        // Place each extracted island on a full-canvas (w x h) transparent layer so its local socket `pivot_uv` lands on `target_joint`
        let mut layer_img = image::RgbaImage::new(w, h);
        let dst_x0 = (target_joint[0] - f64::from(bw) * pivot_uv[0]).round().clamp(0.0, f64::from(w.saturating_sub(bw))) as u32;
        let dst_y0 = (target_joint[1] - f64::from(bh) * pivot_uv[1]).round().clamp(0.0, f64::from(h.saturating_sub(bh))) as u32;

        // Track actual non-transparent pixel medians inside this part for accurate Puppet Pins
        let mut top_row = None::<(f64, f64)>;
        let mut mid_row = None::<(f64, f64)>;
        let mut bot_row = None::<(f64, f64)>;
        for dy in 0..bh {
            let mut xs = Vec::new();
            for dx in 0..bw {
                let sx = bx + dx;
                let sy = by + dy;
                let gx = (sx / step).min(gw - 1);
                let gy = (sy / step).min(gh - 1);
                let mut belongs = false;
                for ngy in gy.saturating_sub(1)..=(gy + 1).min(gh - 1) {
                    for ngx in gx.saturating_sub(1)..=(gx + 1).min(gw - 1) {
                        if cell_owner[(ngy * gw + ngx) as usize] == *owner_idx {
                            belongs = true;
                            break;
                        }
                    }
                    if belongs {
                        break;
                    }
                }
                if !belongs {
                    continue;
                }
                let src_px = decoded.get_pixel(sx, sy);
                let rgba = despill_rgba(src_px[0], src_px[1], src_px[2], src_px[3]);
                if rgba[3] > 64 {
                    layer_img.put_pixel(dst_x0 + dx, dst_y0 + dy, image::Rgba(rgba));
                    assembled_canvas.put_pixel(dst_x0 + dx, dst_y0 + dy, image::Rgba(rgba));
                    if dx >= 6 && dx + 6 < bw && dy >= 6 && dy + 6 < bh {
                        xs.push(dst_x0 + dx);
                    }
                }
            }
            if xs.len() >= 12 {
                let mid_x = f64::from(xs[xs.len() / 2]);
                let cy = f64::from(dst_y0 + dy);
                if top_row.is_none() && dy >= bh / 5 {
                    top_row = Some((mid_x, cy));
                }
                if dy >= bh / 2 && mid_row.is_none() {
                    mid_row = Some((mid_x, cy));
                }
                if dy <= (bh * 4) / 5 {
                    bot_row = Some((mid_x, cy));
                }
            }
        }
        let pins = match *part_id {
            "tail" | "leg_L" | "leg_R" => {
                if let (Some((x1, y1)), Some((x2, y2)), Some((x3, y3))) = (top_row, mid_row, bot_row) {
                    vec![[x1.round(), y1.round()], [x2.round(), y2.round()], [x3.round(), y3.round()]]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        };

        let mut layer_file = layers_dir.join(format!("{part_id}.png"));
        let source_dir = view_dir.join("source");
        fs::create_dir_all(&source_dir)?;
        let source_file = source_dir.join(format!("{part_id}.png"));
        let mut buf = std::io::Cursor::new(Vec::new());
        layer_img
            .write_to(&mut buf, image::ImageFormat::Png)
            .map_err(|e| ScribblesError::Invalid(format!("Failed to encode part source {}: {e}", source_file.display())))?;
        let png_bytes = buf.into_inner();
        craft_atomic::atomic_write(&source_file, &png_bytes)?;
        craft_atomic::atomic_write(&layer_file, &png_bytes)?;
        layer_file = source_file;

        let outline = vec![[dst_x0, dst_y0], [dst_x0 + bw, dst_y0], [dst_x0 + bw, dst_y0 + bh], [dst_x0, dst_y0 + bh]];
        extracted.push(ExtractedPart {
            id: (*part_id).to_string(),
            parent: parent.map(str::to_string),
            file: layer_file,
            bounds: [dst_x0, dst_y0, bw, bh],
            joint: *target_joint,
            pins: pins.clone(),
            outline,
        });
    }

    // Order back-to-front (`tail`, `leg_L`, `arm_L`, `torso`, `leg_R`, `arm_R`, `head`, `ear_L`, `ear_R`)
    let z_order = ["tail", "leg_L", "arm_L", "torso", "leg_R", "arm_R", "head", "ear_L", "ear_R"];
    extracted.sort_by_key(|p| z_order.iter().position(|&z| z == p.id).unwrap_or(99));

    let drawing_name = format!("{char_id}-{view}-drawing.png");
    let drawing_path = view_dir.join(&drawing_name);
    let mut draw_buf = std::io::Cursor::new(Vec::new());
    if assembled_canvas.write_to(&mut draw_buf, image::ImageFormat::Png).is_ok() {
        let _ = craft_atomic::atomic_write(&drawing_path, &draw_buf.into_inner());
    }

    let parts_json_val: Vec<Value> = extracted
        .iter()
        .map(|p| {
            let mut obj = json!({
                "id": p.id,
                "parent": p.parent,
                "source": p.file,
                "file": p.file,
                "bounds": p.bounds,
                "joint": p.joint,
                "outline": p.outline,
            });
            if !p.pins.is_empty() {
                obj["pins"] = json!(p.pins);
            }
            obj
        })
        .collect();

    let parts_doc = json!({
        "image": drawing_name,
        "view": view,
        "size": [w, h],
        "skeleton": "plush-9",
        "animation_style": "cutout_2_5d",
        "status": "reassembled in Rust from chroma-green parts-sheet.png; ready for character.cut and character.rig",
        "parts": parts_json_val,
    });
    let parts_json_path = view_dir.join("parts.json");
    craft_atomic::atomic_write(&parts_json_path, &serde_json::to_vec_pretty(&parts_doc).unwrap_or_default())?;

    Ok(json!({
        "ok": true,
        "character": char_id,
        "view": view,
        "animation_style": "cutout_2_5d",
        "parts_sheet": parts_sheet_png,
        "parts_json": parts_json_path,
        "layers_dir": layers_dir,
        "parts_count": extracted.len(),
        "parts": parts_json_val,
    }))
}

/// Canonical 10 Workshop Moppets (`assets/characters/moppets/`) with their locked visual designs and reference images.
/// Lore: Electronic devices with AI hearts and brains that act like autonomous puppets.
pub const LOCKED_MOPPETS: &[(&str, &str, &str, &str)] = &[
    (
        "tabby",
        "Tabby",
        "assets/characters/tripo-input/18-moppet-tabby.png",
        "Tabby Cat (Electronic Device with AI Heart & Brain acting as an Autonomous Puppet): 4:3 silver-framed LCD screen head with glowing cyan pixel eyes and pink felt nose/whiskers, sitting cat body wrapped in colorful rainbow ribbon cables and patchwork fabric socks.",
    ),
    (
        "toasty",
        "Toasty",
        "assets/characters/tripo-input/19-moppet-toasty.png",
        "Toasty Dog (Electronic Device with AI Heart & Brain acting as an Autonomous Puppet): brushed stainless-steel 2-slice toaster body on four curved chrome tubular legs with warm orange heating-element underglow (its AI heart), golden-tan plush dog head with floppy felt ears, black button nose, and toaster lever tail.",
    ),
    (
        "clank",
        "Clank",
        "assets/characters/tripo-input/20-moppet-clank.png",
        "Clank Bear (Electronic Device with AI Heart & Brain acting as an Autonomous Puppet): tall big-hearted biped with beige CRT monitor head showing green pixel eyes/smile, weathered teal PC tower torso wrapped in grey duct tape, canvas work-shirt arms, khaki pants, and work boots.",
    ),
    (
        "beep",
        "Beep",
        "assets/characters/tripo-input/21-moppet-beep.png",
        "Beep Rodent (Electronic Device with AI Heart & Brain acting as an Autonomous Puppet): tiny 1990s beige two-button computer mouse body with pink-lined rounded mouse ears, black bead eyes, pink nose, wire whiskers, and coiled PS/2 cable tail.",
    ),
    (
        "babel",
        "Babel",
        "assets/characters/tripo-input/22-moppet-babel.png",
        "Babel Owl (Electronic Device with AI Heart & Brain acting as an Autonomous Puppet): vertical wood-grain vintage tabletop radio body with two large round tuning-dial eyes, tiny yellow beak between the dials, round metal speaker grille belly, twin telescoping rabbit-ear antennas, and brass claw feet.",
    ),
    (
        "scribe",
        "Scribe",
        "assets/characters/tripo-input/23-moppet-scribe.png",
        "Scribe Rabbit (Electronic Device with AI Heart & Brain acting as an Autonomous Puppet): vintage black manual typewriter chassis body with round metal keycaps, grey rabbit head rising from the carriage with buck teeth and long pink-lined ribbon ears.",
    ),
    (
        "flash",
        "Flash",
        "assets/characters/tripo-input/24-moppet-flash.png",
        "Flash Monkey (Electronic Device with AI Heart & Brain acting as an Autonomous Puppet): silver-and-black 35mm SLR camera head with large glass lens center and glowing warm lightbulb flash on top, tan burlap/fleece monkey body in a miniature safari jacket.",
    ),
    (
        "vee",
        "Vee",
        "assets/characters/tripo-input/25-moppet-vee.png",
        "Vee Fox (Electronic Device with AI Heart & Brain acting as an Autonomous Puppet): upright black VHS cassette / deck body with orange-brown flocked felt fox head, cassette-reel eyes, hinged jaw lined with white keyboard-key teeth, and pointed felt ears.",
    ),
    (
        "watt",
        "Watt",
        "assets/characters/tripo-input/26-moppet-watt.png",
        "Watt Squirrel (Electronic Device with AI Heart & Brain acting as an Autonomous Puppet): vertical beige 6-outlet power-strip body with glowing orange rocker switch on chest (its AI heart), plush brown squirrel head, and a huge bushy tail made of rainbow insulated copper wires.",
    ),
    (
        "chip",
        "Chip",
        "assets/characters/tripo-input/27-moppet-chip.png",
        "Chip Robin (Electronic Device with AI Heart & Brain acting as an Autonomous Puppet): geometric polyhedral origami/plastic bird body in purple, yellow, and teal with green printed circuit-board (PCB) wings and crest, round blue-ringed eyes, yellow beak, and thin orange wire legs.",
    ),
];

/// A single asset item in a Storyboard Master Pack (`storyboard-pack.json`), carrying its
/// deterministic ID marker (`<ID>__<slot>`), target page/slot, prompt, references, and animation style.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StoryboardPackItem {
    pub id_marker: String,
    pub code: String,
    pub page: usize,
    pub archetype: String,
    pub slot: String,
    pub kind: String,
    pub title: String,
    pub prompt: String,
    pub animation_style: String,
    pub references: Vec<String>,
    pub reference_image: Option<String>,
    pub existing_image: Option<String>,
}

/// Classify the kinetic/animation style from a page's `overall_design`, `purpose`, or `exact_prompt`.
pub fn classify_animation_style(design: &str, prompt: &str) -> String {
    let text = format!("{design} {prompt}").to_ascii_lowercase();
    if text.contains("variable") || text.contains("breathe") || text.contains("300-900") {
        "variable_font_breathe".into()
    } else if text.contains("splittext") || text.contains("stagger") || text.contains("40ms") {
        "split_text_stagger".into()
    } else if text.contains("morphsvg") || text.contains("morph") {
        "morph_svg".into()
    } else if text.contains("falloff") || text.contains("forge dynamics") || text.contains("wave") {
        "falloff_wave".into()
    } else if text.contains("slit-scan") || text.contains("slit_scan") || text.contains("time warp") {
        "slit_scan".into()
    } else if text.contains("roughen") || text.contains("mask") || text.contains("paper-cut") {
        "mask_roughen".into()
    } else if text.contains("matter.js") || text.contains("repulsion") || text.contains("cursor") {
        "matter_repulsion".into()
    } else if text.contains("msdf") || text.contains("dissolve") || text.contains("webgpu") {
        "shader_dissolve".into()
    } else if text.contains("cylinder") || text.contains("rotatex") || text.contains("scroll") {
        "scroll_cylinder".into()
    } else if text.contains("griddler") || text.contains("procedural") || text.contains("ticker") {
        "procedural_loop".into()
    } else if text.contains("spreadsheet") || text.contains("cavalry") || text.contains("ripple") {
        "spreadsheet_ripple".into()
    } else if text.contains("ai empty") || text.contains("ai plate") || text.contains("text in post") {
        "ai_plate_post".into()
    } else if text.contains("parts") || text.contains("cutout") || text.contains("plush-9") {
        "cutout_rig".into()
    } else {
        "storyboard_plate".into()
    }
}

/// Compose 1 to N reference image files into a single clean 16:9 (`1920x1080`) composite reference sheet (`PNG`).
/// When a single image is passed, it is fitted proportionally into the 1920x1080 canvas; when multiple
/// ideas/references (e.g. a Location + Character(s) or the 10 Workshop Moppets) are passed, they are tiled
/// into a clean even grid on a neutral studio background so Google Flow receives a single unified reference image.
pub fn build_composite_reference(sources: &[PathBuf], out_png: &Path) -> Result<PathBuf> {
    let valid: Vec<image::RgbaImage> = sources.iter().filter(|p| p.is_file()).filter_map(|p| open_rgba_flexible(p).ok()).collect();
    if valid.is_empty() {
        return Err(ScribblesError::Invalid("No valid source images found to build a composite reference".into()));
    }
    let (canvas_w, canvas_h) = (1920u32, 1080u32);
    let mut canvas = image::RgbaImage::from_pixel(canvas_w, canvas_h, image::Rgba([28, 30, 36, 255]));

    let n = valid.len();
    let cols = match n {
        1 => 1,
        2 => 2,
        3 | 4 => 2,
        5 | 6 => 3,
        7..=9 => 3,
        _ => 5,
    };
    let rows = (n as u32).div_ceil(cols);
    let pad = 16u32;
    let cell_w = (canvas_w.saturating_sub(pad * (cols + 1))) / cols.max(1);
    let cell_h = (canvas_h.saturating_sub(pad * (rows + 1))) / rows.max(1);

    for (idx, src) in valid.iter().enumerate() {
        let col = (idx as u32) % cols;
        let row = (idx as u32) / cols;
        if row >= rows {
            break;
        }
        let (sw, sh) = src.dimensions();
        if sw == 0 || sh == 0 {
            continue;
        }
        let scale = (f64::from(cell_w) / f64::from(sw)).min(f64::from(cell_h) / f64::from(sh));
        let tw = ((f64::from(sw) * scale).round() as u32).clamp(1, cell_w);
        let th = ((f64::from(sh) * scale).round() as u32).clamp(1, cell_h);
        let resized = image::imageops::resize(src, tw, th, image::imageops::FilterType::Triangle);

        let ox = pad + col * (cell_w + pad) + (cell_w - tw) / 2;
        let oy = pad + row * (cell_h + pad) + (cell_h - th) / 2;
        image::imageops::overlay(&mut canvas, &resized, i64::from(ox), i64::from(oy));
    }

    if let Some(parent) = out_png.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut buf = std::io::Cursor::new(Vec::new());
    canvas
        .write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| ScribblesError::Invalid(format!("Failed to encode composite {}: {e}", out_png.display())))?;
    craft_atomic::atomic_write(out_png, &buf.into_inner())?;
    Ok(out_png.to_path_buf())
}

/// Scan `source_dir` (defaults to `~/Downloads`) and `<project_root>/storyboard/` for ID-marked
/// storyboard assets (`<CODE>__<slot>.png` such as `L01__world_stage_image.png`, `S04__hero_image.png`,
/// `C01__char_original_image.png`, or a downloaded `storyboard-handoff.json`).
/// Copies each valid PNG into `<project_root>/assets/{backgrounds,characters,images}/<code>/` and returns
/// the resolved `(id_marker, code, slot, dest_path)` list so the engine can register and place them in one batch.
pub fn ingest_id_marked_assets(project_root: &Path, source_dir: Option<&Path>) -> Result<Vec<(String, String, String, PathBuf)>> {
    let mut dirs = Vec::new();
    if let Some(d) = source_dir {
        if d.is_dir() {
            dirs.push(d.to_path_buf());
        }
    } else if let Some(home) = std::env::var_os("HOME") {
        let dl = PathBuf::from(home).join("Downloads");
        if dl.is_dir() {
            dirs.push(dl);
        }
    }
    let sb_dir = project_root.join("storyboard/generated");
    if sb_dir.is_dir() && !dirs.iter().any(|d| d == &sb_dir) {
        dirs.push(sb_dir);
    }

    let mut ingested = Vec::new();
    let mut candidate_files = Vec::new();

    for dir in dirs {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                candidate_files.push(path);
            } else if path.is_dir() {
                // Also scan subdirectories (e.g. unzipped folders in ~/Downloads)
                if let Ok(sub_entries) = fs::read_dir(&path) {
                    for sub in sub_entries.flatten() {
                        let sub_path = sub.path();
                        if sub_path.is_file() {
                            candidate_files.push(sub_path);
                        }
                    }
                }
            }
        }
    }

    for path in candidate_files {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else { continue };
        let Some(ext) = path.extension().and_then(|s| s.to_str()) else { continue };
        if !ext.eq_ignore_ascii_case("png") && !ext.eq_ignore_ascii_case("jpg") && !ext.eq_ignore_ascii_case("jpeg") {
            continue;
        }
        let Some((raw_code, raw_slot)) = stem.split_once("__") else { continue };
        let code = raw_code.trim().to_ascii_uppercase();
        let slot_clean = raw_slot.split([' ', '(', '.']).next().unwrap_or(raw_slot).trim().to_ascii_lowercase();
        if code.is_empty() || slot_clean.is_empty() {
            continue;
        }
        let id_marker = format!("{code}__{slot_clean}");
        let folder_id = sanitize_id(&id_marker);
        let category = if code.starts_with('L') {
            "backgrounds"
        } else if code.starts_with('C') {
            "characters"
        } else {
            "images"
        };
        let dest_dir = project_root.join("assets").join(category).join(&folder_id);
        let dest_png = dest_dir.join(format!("{id_marker}.png"));
        if ensure_valid_png(&path, &dest_png).is_ok() {
            ingested.push((id_marker, code, slot_clean, dest_png));
        }
    }
    ingested.sort_by(|a, b| a.0.cmp(&b.0));
    ingested.dedup_by(|a, b| a.0 == b.0);
    Ok(ingested)
}
