//! Storyboard: one lightweight DesignCraft document, edited by object label.
//!
//! The document is a `.designcraft` file made from the embedded neutral template. Every
//! editable frame carries an `object.label` (`scene_id`, `title`, `cue`, `hero_image`,
//! `ref_1`..`ref_12` ...), and a scene is one spread holding a `scene_id` frame. The commands
//! here open the file in `designcraft-engine`, change labelled frames, and save it back.
//! There is no second data model: the file is the storyboard.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use designcraft_engine::Session as Dc;
use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

const TEMPLATE: &[u8] = include_bytes!("../templates/storyboard.designcraft");
/// The scene spread of the template (cover, world and character spreads come first).
const TEMPLATE_SCENE_SPREAD: usize = 3;
const MAX_REFS: usize = 12;

/// Text frames a scene exposes.
pub const TEXT_FIELDS: &[&str] = &[
    "running_title", "running_sub", "badge", "section", "hand_note", "scene_id", "title", "cue", "time",
    "purpose", "staging_notes", "revision", "overall_design", "location_ref", "character_shared",
    "character_lead", "asset_geometry", "match_note", "exact_prompt", "strip_note", "source_image_caption",
    "result_image_caption", "page_no_left", "page_no_right",
];

fn fail(e: impl std::fmt::Display) -> EngineError {
    EngineError::CommandFailed("storyboard".into(), e.to_string())
}

fn bad(msg: &str) -> EngineError {
    EngineError::InvalidParams(msg.into())
}

/// The document path: `path`, else `storyboard.designcraft` beside the open project.
fn doc_path(s: &Session, p: &Value) -> Result<PathBuf> {
    if let Some(raw) = p.get("path").and_then(Value::as_str).filter(|r| !r.trim().is_empty()) {
        let mut path = crate::workspace::resolve_workspace_path(s, raw);
        if path.extension().is_none_or(|e| !e.eq_ignore_ascii_case("designcraft")) {
            path.as_mut_os_string().push(".designcraft");
        }
        return Ok(path);
    }
    let project = s.project_path.as_ref().ok_or_else(|| bad("give a `path`, or open a project"))?;
    Ok(project.with_file_name("storyboard.designcraft"))
}

/// An open storyboard document and the frames of each of its scenes.
struct Board {
    dc: Dc,
    path: PathBuf,
}

impl Board {
    fn open(path: &Path) -> Result<Self> {
        let mut dc = Dc::new();
        dc.execute("file.open", &json!({"path": path})).map_err(fail)?;
        Ok(Self { dc, path: path.to_path_buf() })
    }

    fn run(&mut self, id: &str, params: Value) -> Result<Value> {
        self.dc.execute(id, &params).map_err(fail)
    }

    fn save(&mut self) -> Result<()> {
        let path = self.path.clone();
        self.run("file.save", json!({"path": path}))?;
        Ok(())
    }

    /// Label -> frame id per spread, for `labels` and `scene_id` (which marks a scene spread).
    fn frames(&mut self, labels: &[String]) -> Result<Vec<HashMap<String, u64>>> {
        let inspect = self.run("document.inspect", json!({}))?;
        let spreads = inspect["spreads"].as_array().cloned().unwrap_or_default();
        let mut owner: HashMap<u64, usize> = HashMap::new();
        for (i, sp) in spreads.iter().enumerate() {
            for item in sp["items"].as_array().into_iter().flatten() {
                if let Some(id) = item["id"].as_u64() {
                    owner.insert(id, i);
                }
            }
        }
        let mut out = vec![HashMap::new(); spreads.len()];
        for label in labels.iter().map(String::as_str).chain(["scene_id"]) {
            let found = self.run("object.findByLabel", json!({"label": label}))?;
            for id in found["ids"].as_array().into_iter().flatten().filter_map(Value::as_u64) {
                if let Some(&sp) = owner.get(&id) {
                    out[sp].insert(label.to_string(), id);
                }
            }
        }
        Ok(out)
    }

    /// Spread indexes that are scenes (hold a `scene_id` frame), in document order.
    fn scenes(&mut self, labels: &[String]) -> Result<(Vec<usize>, Vec<HashMap<String, u64>>)> {
        let frames = self.frames(labels)?;
        let scenes = (0..frames.len()).filter(|&i| frames[i].contains_key("scene_id")).collect();
        Ok((scenes, frames))
    }

    fn text(&mut self, frame: u64) -> Result<String> {
        let r = self.run("story.get", json!({"frame": frame}))?;
        Ok(r["text"].as_str().unwrap_or_default().to_string())
    }

    fn set_text(&mut self, frame: u64, text: &str) -> Result<()> {
        let r = self.run("story.get", json!({"frame": frame}))?;
        let story = r.get("story").or_else(|| r.get("id")).cloned().ok_or_else(|| fail("frame has no story"))?;
        self.run("story.setText", json!({"story": story, "text": text}))?;
        Ok(())
    }

    fn set_image(&mut self, frame: u64, hint: Option<u64>, path: &str) -> Result<()> {
        self.run("place.load", json!({"path": path}))?;
        self.run("place.drop", json!({"x": 0, "y": 0, "frame": frame}))?;
        self.run("object.fit", json!({"mode": "fillProportionally", "ids": [frame]}))?;
        if let Some(h) = hint {
            self.run("object.setFlags", json!({"ids": [h], "hidden": true}))?;
        }
        Ok(())
    }
}

/// Every label a request names: `fields`, `images` (with their `_hint` frames) and the contact strip.
fn wanted(p: &Value) -> Vec<String> {
    let keys = |k: &str| -> Vec<String> { p.get(k).and_then(Value::as_object).map(|o| o.keys().cloned().collect()).unwrap_or_default() };
    let mut v = keys("fields");
    for image in keys("images") {
        v.push(format!("{image}_hint"));
        v.push(image);
    }
    if p.get("refs").is_some() {
        v.push("contact_strip".into());
        v.extend((1..=MAX_REFS).map(|n| format!("ref_{n}")));
    }
    v
}

/// Slot rectangles `[x0, y0, x1, y1]` for the contact strip with `n` pictures, and its band.
fn strip_layout(n: usize) -> ([f64; 4], Vec<[f64; 4]>) {
    let (x0, x1, pad, gap) = (846.0, 1556.0, 10.0, 6.0);
    let single = n <= 6;
    let (band, cols, top, bottom) = if single { ([x0, 800.0, x1, 912.0], n.max(1), 814.0, 898.0) } else { ([x0, 792.0, x1, 918.0], if n > 8 { 6 } else { 4 }, 808.0, 918.0 - 16.0) };
    let rows = n.div_ceil(cols).max(1);
    let w = (x1 - x0 - 2.0 * pad - gap * (cols as f64 - 1.0)) / cols as f64;
    let h = (bottom - top - gap * (rows as f64 - 1.0)) / rows as f64;
    let slots = (0..n)
        .map(|k| {
            let (r, c) = (k / cols, k % cols);
            let sx = x0 + pad + c as f64 * (w + gap);
            let sy = top + r as f64 * (h + gap);
            [sx, sy, sx + w, sy + h]
        })
        .collect();
    (band, slots)
}

fn place_rect(b: &mut Board, frame: u64, r: [f64; 4]) -> Result<()> {
    b.run("transform.set", json!({"x": r[0], "y": r[1], "width": r[2] - r[0], "height": r[3] - r[1], "ids": [frame]}))?;
    Ok(())
}

fn fill_refs(b: &mut Board, frames: &HashMap<String, u64>, refs: &[String]) -> Result<()> {
    if refs.len() > MAX_REFS {
        return Err(bad("at most 12 `refs`"));
    }
    let (band, slots) = strip_layout(refs.len());
    if let Some(&strip) = frames.get("contact_strip") {
        place_rect(b, strip, band)?;
    }
    for n in 1..=MAX_REFS {
        let Some(&frame) = frames.get(&format!("ref_{n}")) else { continue };
        let used = n <= refs.len();
        b.run("object.setFlags", json!({"ids": [frame], "hidden": !used}))?;
        if used {
            place_rect(b, frame, slots[n - 1])?;
            b.set_image(frame, None, &refs[n - 1])?;
        }
    }
    Ok(())
}

/// Apply `fields`, `images` and `refs` from `p` to the scene whose frames are `frames`.
fn fill_scene(b: &mut Board, frames: &HashMap<String, u64>, p: &Value) -> Result<()> {
    for (label, value) in p.get("fields").and_then(Value::as_object).into_iter().flatten() {
        let text = value.as_str().ok_or_else(|| bad("`fields` values are strings"))?;
        let frame = *frames.get(label).ok_or_else(|| bad(&format!("no frame labelled `{label}` in this scene")))?;
        b.set_text(frame, text)?;
    }
    for (label, value) in p.get("images").and_then(Value::as_object).into_iter().flatten() {
        let path = value.as_str().ok_or_else(|| bad("`images` values are file paths"))?;
        let frame = *frames.get(label).ok_or_else(|| bad(&format!("no frame labelled `{label}` in this scene")))?;
        b.set_image(frame, frames.get(&format!("{label}_hint")).copied(), path)?;
    }
    if let Some(refs) = p.get("refs").and_then(Value::as_array) {
        let refs: Vec<String> = refs.iter().filter_map(|r| r.as_str().map(str::to_string)).collect();
        fill_refs(b, frames, &refs)?;
    }
    Ok(())
}

fn scene_summary(b: &mut Board, number: usize, spread: usize, frames: &HashMap<String, u64>) -> Result<Value> {
    let mut fields = serde_json::Map::new();
    for label in TEXT_FIELDS {
        if let Some(&id) = frames.get(*label) {
            fields.insert((*label).to_string(), json!(b.text(id)?));
        }
    }
    Ok(json!({"scene": number, "spread": spread, "fields": fields}))
}

fn pick_scene(scenes: &[usize], p: &Value) -> Result<(usize, usize)> {
    let n = p.get("scene").and_then(Value::as_u64).ok_or_else(|| bad("missing `scene` (1-based)"))? as usize;
    scenes.get(n.wrapping_sub(1)).map(|&sp| (n, sp)).ok_or_else(|| bad(&format!("no scene {n}; the storyboard has {}", scenes.len())))
}

/// `storyboard.new`: a new storyboard from the embedded neutral template.
pub fn cmd_storyboard_new(s: &mut Session, p: &Value) -> Result<Value> {
    let path = doc_path(s, p)?;
    if path.exists() && !p.get("overwrite").and_then(Value::as_bool).unwrap_or(false) {
        return Err(bad(&format!("{} exists; pass overwrite=true to replace it", path.display())));
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, TEMPLATE)?;
    Ok(json!({"ok": true, "path": path, "scenes": 1}))
}

/// `storyboard.scenes`: every scene with its text fields.
pub fn cmd_storyboard_scenes(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Board::open(&doc_path(s, p)?)?;
    let labels: Vec<String> = TEXT_FIELDS.iter().map(|l| (*l).to_string()).collect();
    let (scenes, frames) = b.scenes(&labels)?;
    let mut list = Vec::new();
    for (i, &sp) in scenes.iter().enumerate() {
        list.push(scene_summary(&mut b, i + 1, sp, &frames[sp])?);
    }
    Ok(json!({"ok": true, "path": b.path, "scenes": list}))
}

/// `storyboard.set`: change one scene's text, images and contact strip, then save.
pub fn cmd_storyboard_set(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Board::open(&doc_path(s, p)?)?;
    let (scenes, frames) = b.scenes(&wanted(p))?;
    let (n, sp) = pick_scene(&scenes, p)?;
    fill_scene(&mut b, &frames[sp], p)?;
    b.save()?;
    Ok(json!({"ok": true, "scene": n, "path": b.path}))
}

/// `storyboard.add`: copy a scene spread (default the last) to the end and fill it.
pub fn cmd_storyboard_add(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Board::open(&doc_path(s, p)?)?;
    let (scenes, _) = b.scenes(&[])?;
    let from = match p.get("from").and_then(Value::as_u64) {
        Some(n) => pick_scene(&scenes, &json!({"scene": n}))?.1,
        None => scenes.last().copied().unwrap_or(TEMPLATE_SCENE_SPREAD),
    };
    let dup = b.run("layout.pages.duplicateSpread", json!({"spread": from, "atEnd": true}))?;
    let spread = dup["page"].as_u64().ok_or_else(|| fail("no page index from duplicateSpread"))? as usize;
    let frames = b.frames(&wanted(p))?;
    let new = frames.get(spread).ok_or_else(|| fail("the new spread was not found"))?;
    fill_scene(&mut b, new, p)?;
    b.save()?;
    let (scenes, _) = b.scenes(&[])?;
    Ok(json!({"ok": true, "scene": scenes.len(), "path": b.path}))
}

fn render_png(b: &mut Board, page: usize, scale: f64) -> Result<Vec<u8>> {
    let st = b.dc.doc().map_err(fail)?;
    let mut r = designcraft_render::Renderer::new();
    let opts = designcraft_render::RenderOptions { printing_only: true, ..Default::default() };
    let img = r.render_page(&st.doc, &b.dc.cache, page, scale, true, &opts).ok_or_else(|| bad("no such page"))?;
    Ok(img.to_png())
}

/// `storyboard.thumbnail`: one scene spread as a PNG.
pub fn cmd_storyboard_thumbnail(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Board::open(&doc_path(s, p)?)?;
    let (scenes, _) = b.scenes(&[])?;
    let (n, sp) = pick_scene(&scenes, p)?;
    let out = p.get("out").and_then(Value::as_str).ok_or_else(|| bad("missing `out` .png path"))?;
    let scale = p.get("scale").and_then(Value::as_f64).unwrap_or(0.5);
    let png = render_png(&mut b, sp, scale)?;
    if let Some(dir) = Path::new(out).parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(out, png)?;
    Ok(json!({"ok": true, "scene": n, "out": out}))
}

/// `storyboard.export`: the whole storyboard as a PDF, and optionally every spread as a PNG.
pub fn cmd_storyboard_export(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Board::open(&doc_path(s, p)?)?;
    let mut done = serde_json::Map::new();
    if let Some(pdf) = p.get("pdf").and_then(Value::as_str) {
        b.run("file.exportPdf", json!({"path": pdf}))?;
        done.insert("pdf".into(), json!(pdf));
    }
    if let Some(dir) = p.get("png_dir").and_then(Value::as_str) {
        std::fs::create_dir_all(dir)?;
        let scale = p.get("scale").and_then(Value::as_f64).unwrap_or(1.0);
        let pages = b.frames(&[])?.len();
        for page in 0..pages {
            let png = render_png(&mut b, page, scale)?;
            std::fs::write(Path::new(dir).join(format!("page-{:03}.png", page + 1)), png)?;
        }
        done.insert("png_dir".into(), json!(dir));
    }
    if done.is_empty() {
        return Err(bad("give `pdf` and/or `png_dir`"));
    }
    Ok(json!({"ok": true, "exported": done}))
}
