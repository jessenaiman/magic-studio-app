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
use designcraft_engine::doc::Content;
use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

const TEMPLATE: &[u8] = include_bytes!("../../../templates/storyboard.designcraft");
const MAX_REFS: usize = 12;

/// The page archetypes the template defines, each identified by one marker label.
/// A page's archetype is read from the document, so the template can gain a page type
/// without touching this table; the table only says which marker to look for.
pub const ARCHETYPES: &[(&str, &str)] = &[
    ("cover", "cover_title"),
    ("world", "world_heading"),
    ("char", "char_heading"),
    ("scene", "scene_id"),
];

/// A page's labelled frames: script label -> frame id.
type Labels = HashMap<String, u64>;

/// The archetype of a page, from the marker labels it carries.
fn archetype_of(labels: &Labels) -> &'static str {
    for (name, marker) in ARCHETYPES {
        if labels.contains_key(*marker) {
            return name;
        }
    }
    "scene"
}

/// One labelled slot on a page: a text frame, an image frame, or a plain shape.
struct Slot {
    label: String,
    id: u64,
    kind: &'static str,
    text: Option<String>,
    image: Option<String>,
    bounds: Option<[f64; 4]>,
    hidden: bool,
}

/// One page of the storyboard: its archetype, the section it belongs to, and its slots.
struct PageSlots {
    index: usize,
    archetype: String,
    section: Option<String>,
    slots: Vec<Slot>,
}

impl PageSlots {
    fn labels(&self) -> Labels {
        self.slots.iter().map(|s| (s.label.clone(), s.id)).collect()
    }

    fn slot(&self, label: &str) -> Option<&Slot> {
        self.slots.iter().find(|s| s.label == label)
    }

    fn to_json(&self) -> Value {
        json!({
            "page": self.index,
            "archetype": self.archetype,
            "section": self.section,
            "slots": self.slots_json(),
        })
    }

    fn slots_json(&self) -> Value {
        Value::Array(
            self.slots
                .iter()
                .map(|s| {
                    json!({
                        "label": s.label,
                        "id": s.id,
                        "kind": s.kind,
                        "text": s.text,
                        "image": s.image,
                        "bounds": s.bounds,
                        "hidden": s.hidden,
                    })
                })
                .collect(),
        )
    }
}

/// Text frames a scene exposes.
pub const TEXT_FIELDS: &[&str] = &[
    "running_title",
    "running_sub",
    "badge",
    "section",
    "hand_note",
    "scene_id",
    "title",
    "cue",
    "time",
    "purpose",
    "staging_notes",
    "revision",
    "overall_design",
    "location_ref",
    "character_shared",
    "character_lead",
    "asset_geometry",
    "match_note",
    "exact_prompt",
    "strip_note",
    "source_image_caption",
    "result_image_caption",
    "page_no_left",
    "page_no_right",
];

fn fail(e: impl std::fmt::Display) -> EngineError {
    EngineError::CommandFailed("storyboard".into(), e.to_string())
}

fn bad(msg: &str) -> EngineError {
    EngineError::InvalidParams(msg.into())
}

/// The document path: `path`, else `storyboard.designcraft` beside the open project.
fn doc_path(s: &Session, p: &Value) -> Result<PathBuf> {
    if let Some(raw) = p
        .get("path")
        .and_then(Value::as_str)
        .filter(|r| !r.trim().is_empty())
    {
        let mut path = crate::workspace::resolve_workspace_path(s, raw);
        if path
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("designcraft"))
        {
            path.as_mut_os_string().push(".designcraft");
        }
        return Ok(path);
    }
    let project = s
        .project_path
        .as_ref()
        .ok_or_else(|| bad("give a `path`, or open a project"))?;
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
        dc.execute("file.open", &json!({"path": path}))
            .map_err(fail)?;
        Ok(Self {
            dc,
            path: path.to_path_buf(),
        })
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
    fn frames(&mut self, labels: &[String]) -> Result<Vec<Labels>> {
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
            for id in found["ids"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_u64)
            {
                if let Some(&sp) = owner.get(&id) {
                    out[sp].insert(label.to_string(), id);
                }
            }
        }
        Ok(out)
    }

    /// Spread indexes that are scenes (hold a `scene_id` frame), in document order.
    fn scenes(&mut self, labels: &[String]) -> Result<(Vec<usize>, Vec<Labels>)> {
        let frames = self.frames(labels)?;
        let scenes = (0..frames.len())
            .filter(|&i| frames[i].contains_key("scene_id"))
            .collect();
        Ok((scenes, frames))
    }

    fn text(&mut self, frame: u64) -> Result<String> {
        let r = self.run("story.get", json!({"frame": frame}))?;
        Ok(r["text"].as_str().unwrap_or_default().to_string())
    }

    fn set_text(&mut self, frame: u64, text: &str) -> Result<()> {
        let r = self.run("story.get", json!({"frame": frame}))?;
        let story = r
            .get("story")
            .or_else(|| r.get("id"))
            .cloned()
            .ok_or_else(|| fail("frame has no story"))?;
        self.run("story.setText", json!({"story": story, "text": text}))?;
        Ok(())
    }

    /// Lock a frame so it cannot be edited afterwards.
    fn lock(&mut self, frame: u64) -> Result<()> {
        self.run("object.setFlags", json!({"ids": [frame], "locked": true}))?;
        Ok(())
    }

    fn set_image(&mut self, frame: u64, hint: Option<u64>, path: &str) -> Result<()> {
        self.run("place.load", json!({"path": path}))?;
        self.run("place.drop", json!({"x": 0, "y": 0, "frame": frame}))?;
        self.run(
            "object.fit",
            json!({"mode": "fillProportionally", "ids": [frame]}),
        )?;
        if let Some(h) = hint {
            self.run("object.setFlags", json!({"ids": [h], "hidden": true}))?;
        }
        Ok(())
    }

    /// Every page of the document with the labelled slots it carries.
    ///
    /// The slots come from the loaded document, so the template may gain a slot without this
    /// table changing; the bounds come from the engine's own inspect, which reports them in
    /// spread space.
    fn pages(&mut self) -> Result<Vec<PageSlots>> {
        let inspect = self.run("document.inspect", json!({}))?;
        let mut bounds: HashMap<u64, [f64; 4]> = HashMap::new();
        for spread in inspect["spreads"].as_array().into_iter().flatten() {
            for item in spread["items"].as_array().into_iter().flatten() {
                let (Some(id), Some(b)) = (item["id"].as_u64(), item["bounds"].as_array()) else {
                    continue;
                };
                if b.len() == 4 {
                    bounds.insert(
                        id,
                        [
                            b[0].as_f64().unwrap_or(0.0),
                            b[1].as_f64().unwrap_or(0.0),
                            b[2].as_f64().unwrap_or(0.0),
                            b[3].as_f64().unwrap_or(0.0),
                        ],
                    );
                }
            }
        }
        let state = self.dc.doc().map_err(fail)?;
        let doc = &state.doc;
        // Parent spread id -> its label ("A-Parent"), for the section a page belongs to.
        let parents: HashMap<u64, String> = doc
            .parents
            .iter()
            .filter_map(|p| p.parent.as_ref().map(|info| (p.id.0, info.label())))
            .collect();
        let mut pages = Vec::new();
        for (index, spread) in doc.spreads.iter().enumerate() {
            let mut labels: Labels = HashMap::new();
            let mut slots = Vec::new();
            for item in &spread.items {
                if item.label.is_empty() {
                    continue;
                }
                let (kind, text, image) = match &item.content {
                    Content::Text(frame) => {
                        let text = doc.stories.get(&frame.story).map(|s| s.text.clone());
                        ("text", text, None)
                    }
                    Content::Graphic(graphic) => {
                        let image = doc.assets.get(&graphic.asset).and_then(|a| a.link.clone());
                        ("image", None, image)
                    }
                    _ => ("other", None, None),
                };
                labels.insert(item.label.clone(), item.id.0);
                slots.push(Slot {
                    label: item.label.clone(),
                    id: item.id.0,
                    kind,
                    text,
                    image,
                    bounds: bounds.get(&item.id.0).copied(),
                    hidden: item.hidden,
                });
            }
            for slot in &mut slots {
                if slot.kind == "other" && labels.contains_key(&format!("{}_hint", slot.label)) {
                    slot.kind = "image";
                }
            }
            let section = spread
                .pages
                .first()
                .and_then(|p| p.parent)
                .and_then(|id| parents.get(&id.0).cloned());
            pages.push(PageSlots {
                index,
                archetype: archetype_of(&labels).to_string(),
                section,
                slots,
            });
        }
        Ok(pages)
    }
}

/// Slot rectangles `[x0, y0, x1, y1]` for the contact strip with `n` pictures, and its band.
fn strip_layout(n: usize, strip_bounds: Option<[f64; 4]>) -> ([f64; 4], Vec<[f64; 4]>) {
    let [x0, base_y0, x1, base_y1] = strip_bounds
        .filter(|b| b[2] > b[0] && b[3] > b[1])
        .unwrap_or([846.0, 800.0, 1556.0, 912.0]);
    let (pad, gap) = (10.0, 6.0);
    let single = n <= 6;
    let (band, cols, top, bottom) = if single {
        (
            [x0, base_y0, x1, base_y1],
            n.max(1),
            base_y0 + 14.0,
            base_y1 - 14.0,
        )
    } else {
        let y0 = base_y0 - 8.0;
        let y1 = base_y1 + 6.0;
        (
            [x0, y0, x1, y1],
            if n > 8 { 6 } else { 4 },
            y0 + 16.0,
            y1 - 16.0,
        )
    };
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
    b.run(
        "transform.set",
        json!({"x": r[0], "y": r[1], "width": r[2] - r[0], "height": r[3] - r[1], "ids": [frame]}),
    )?;
    Ok(())
}

fn fill_refs(
    b: &mut Board,
    frames: &Labels,
    strip_bounds: Option<[f64; 4]>,
    refs: &[String],
) -> Result<()> {
    if refs.len() > MAX_REFS {
        return Err(bad("at most 12 `refs`"));
    }
    let (band, slots) = strip_layout(refs.len(), strip_bounds);
    if let Some(&strip) = frames.get("contact_strip") {
        place_rect(b, strip, band)?;
    }
    for n in 1..=MAX_REFS {
        let Some(&frame) = frames.get(&format!("ref_{n}")) else {
            continue;
        };
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
fn fill_scene(
    s: &Session,
    b: &mut Board,
    page: &PageSlots,
    frames: &Labels,
    p: &Value,
) -> Result<()> {
    for (label, value) in p
        .get("fields")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        let text = value
            .as_str()
            .ok_or_else(|| bad("`fields` values are strings"))?;
        let frame = *frames
            .get(label)
            .ok_or_else(|| bad(&format!("this page has no slot labelled `{label}`")))?;
        b.set_text(frame, text)?;
        // A field written from a source is locked, so the document keeps it read-only.
        if p.get("lock").and_then(Value::as_bool).unwrap_or(false) {
            b.lock(frame)?;
        }
    }
    for (label, value) in p
        .get("images")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        let raw = value
            .as_str()
            .ok_or_else(|| bad("`images` values are file paths"))?;
        let resolved = crate::workspace::resolve_workspace_path(s, raw);
        let frame = *frames
            .get(label)
            .ok_or_else(|| bad(&format!("this page has no slot labelled `{label}`")))?;
        b.set_image(
            frame,
            frames.get(&format!("{label}_hint")).copied(),
            &resolved.to_string_lossy(),
        )?;
    }
    if let Some(refs) = p.get("refs").and_then(Value::as_array) {
        let refs: Vec<String> = refs
            .iter()
            .filter_map(|r| {
                r.as_str().map(|raw| {
                    crate::workspace::resolve_workspace_path(s, raw)
                        .to_string_lossy()
                        .into_owned()
                })
            })
            .collect();
        let strip_bounds = page.slot("contact_strip").and_then(|sl| sl.bounds);
        fill_refs(b, frames, strip_bounds, &refs)?;
    }
    Ok(())
}

fn scene_summary(b: &mut Board, number: usize, spread: usize, frames: &Labels) -> Result<Value> {
    let mut fields = serde_json::Map::new();
    for label in TEXT_FIELDS {
        if let Some(&id) = frames.get(*label) {
            fields.insert((*label).to_string(), json!(b.text(id)?));
        }
    }
    Ok(json!({"scene": number, "spread": spread, "fields": fields}))
}

/// The page a request targets.
///
/// `page` is a page index, `label` names a slot and the page that holds it, and `scene` is the
/// scene number (1-based) as the older commands used.
fn page_index(pages: &[PageSlots], p: &Value) -> Result<usize> {
    if let Some(n) = p.get("page").and_then(Value::as_u64) {
        let index = n as usize;
        return if index < pages.len() {
            Ok(index)
        } else {
            Err(bad(&format!(
                "no page {n}; the storyboard has {} pages",
                pages.len()
            )))
        };
    }
    if let Some(label) = p.get("label").and_then(Value::as_str) {
        return pages
            .iter()
            .find(|pg| pg.slot(label).is_some())
            .map(|pg| pg.index)
            .ok_or_else(|| bad(&format!("no page holds a slot labelled `{label}`")));
    }
    if let Some(n) = p.get("scene").and_then(Value::as_u64) {
        let n = n as usize;
        return pages
            .iter()
            .filter(|pg| pg.archetype == "scene")
            .nth(n.wrapping_sub(1))
            .map(|pg| pg.index)
            .ok_or_else(|| bad(&format!("no scene {n}")));
    }
    Err(bad(
        "give `page` (index), `label` (a slot name) or `scene` (number)",
    ))
}

/// A section of the song is verse or chorus. Everything else (spoken parts, outros) is
/// never pulled from a recording.
fn is_verse_or_chorus(section: &str) -> bool {
    let s = section.to_ascii_lowercase();
    s.starts_with("verse") || s.starts_with("chorus")
}

/// Read a JSON document from a path resolved against the workspace.
fn read_json(s: &Session, raw: &str) -> Result<Value> {
    let path = crate::workspace::resolve_workspace_path(s, raw);
    let text =
        std::fs::read_to_string(&path).map_err(|e| bad(&format!("{}: {e}", path.display())))?;
    serde_json::from_str(&text).map_err(|e| bad(&format!("{}: {e}", path.display())))
}

/// `storyboard.cues`: the song's verse and chorus, with the timing of each line.
///
/// The words come from the canonical lyrics sheet (the artist's printed words). The timings
/// come from the transcript, and each printed line takes its **first** occurrence: a recording
/// that sings the chorus twice contributes one set of lines, not two.
pub fn cmd_storyboard_cues(s: &mut Session, p: &Value) -> Result<Value> {
    let lyrics = p
        .get("lyrics")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("give `lyrics` (the canonical lyrics sheet .json)"))?;
    let sheet = read_json(s, lyrics)?;
    let wanted: Option<Vec<String>> = p.get("sections").and_then(Value::as_str).map(|list| {
        list.split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    });
    // The sheet's printed structure gives the order.
    let structure: Vec<String> = sheet["structure_as_printed"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let lines = sheet["lines"].as_array().cloned().unwrap_or_default();
    // Timings: the first time the recording says each printed line.
    let mut timings: HashMap<String, (f64, f64)> = HashMap::new();
    if let Some(transcript) = p.get("transcript").and_then(Value::as_str) {
        for para in read_json(s, transcript)?["paragraphs"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let text = para["text"].as_str().unwrap_or_default().trim().to_string();
            let (Some(start), Some(end)) = (para["start"].as_f64(), para["end"].as_f64()) else {
                continue;
            };
            timings.entry(text).or_insert((start, end));
        }
    }
    let mut sections = Vec::new();
    for section in &structure {
        match &wanted {
            Some(list) if !list.iter().any(|s| s == section) => continue,
            None if !is_verse_or_chorus(section) => continue,
            _ => {}
        }
        let section_lines: Vec<Value> = lines
            .iter()
            .filter(|l| l["section"].as_str() == Some(section.as_str()))
            .map(|l| {
                let text = l["text"].as_str().unwrap_or_default();
                let timing = timings.get(text.trim()).copied();
                json!({
                    "id": l["id"],
                    "text": text,
                    "start": timing.map(|t| t.0),
                    "end": timing.map(|t| t.1),
                })
            })
            .collect();
        sections.push(json!({"section": section, "lines": section_lines}));
    }
    // Scene cues: consecutive lines of a section, the way the storyboard's scenes group them.
    let per = p.get("per").and_then(Value::as_u64).unwrap_or(1).max(1) as usize;
    let mut cues = Vec::new();
    for section in &sections {
        let section_lines = section["lines"].as_array().cloned().unwrap_or_default();
        for chunk in section_lines.chunks(per) {
            cues.push(json!({
                "section": section["section"],
                "text": chunk.iter().filter_map(|l| l["text"].as_str()).collect::<Vec<_>>().join(" / "),
                "start": chunk.first().and_then(|l| l["start"].as_f64()),
                "end": chunk.last().and_then(|l| l["end"].as_f64()),
            }));
        }
    }
    Ok(json!({
        "ok": true,
        "lyrics": lyrics,
        "sections": sections,
        "cues": cues,
        "as_printed": sheet["structure_as_printed"],
        "as_sung": sheet["structure_as_sung"],
        "as_sung_source": sheet["structure_as_sung_source"],
    }))
}

/// The template's slot texts, by label: what a field still holds before anyone fills it.
///
/// A template default in brackets ("[Project title]") is a slot waiting for content. A plain
/// default ("MATERIAL & SCALE") is a heading that is meant to stay.
fn template_defaults() -> Result<HashMap<String, String>> {
    let dir =
        std::env::temp_dir().join(format!("magic-storyboard-template-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("storyboard.designcraft");
    craft_atomic::atomic_write(&path, TEMPLATE)?;
    let pages = {
        let mut board = Board::open(&path)?;
        board.pages()?
    };
    let _ = std::fs::remove_dir_all(&dir);
    let mut defaults = HashMap::new();
    for page in pages {
        for slot in page.slots {
            let Some(text) = slot.text else { continue };
            let text = text.trim();
            if text.starts_with('[') && text.ends_with(']') {
                defaults.insert(slot.label, text.to_string());
            }
        }
    }
    Ok(defaults)
}

/// The command that fills one slot, for an agent that has to fix it.
fn fix_command(label: &str) -> String {
    format!("magic-cli storyboard set --label {label} --text {label}=<text>")
}

/// `storyboard.check`: the fields that still hold the template's default.
///
/// A slot whose text is still the template's bracketed default has never been filled. A hidden
/// hint is not one of them: the hint is hidden once its image is placed. The command that
/// fills a slot is reported with it, so a missing block traces straight to the CLI call that
/// changes it.
pub fn cmd_storyboard_check(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Board::open(&doc_path(s, p)?)?;
    let pages = b.pages()?;
    let defaults = template_defaults()?;
    let mut report = Vec::new();
    let mut count = 0usize;
    for page in &pages {
        let mut unfilled = Vec::new();
        for slot in &page.slots {
            // A hidden slot holds no visible content; a hidden hint means its image is placed.
            if slot.hidden {
                continue;
            }
            let (Some(text), Some(default)) = (&slot.text, defaults.get(&slot.label)) else {
                continue;
            };
            if text.trim() != default {
                continue;
            }
            // An image slot shows its placeholder in a companion hint; the fix places the image.
            let (label, fix) = match slot.label.strip_suffix("_hint") {
                Some(base) if defaults.contains_key(&format!("{base}_hint")) => (
                    base.to_string(),
                    format!("magic-cli storyboard set --label {base} --image {base}=<path>"),
                ),
                _ => (slot.label.clone(), fix_command(&slot.label)),
            };
            unfilled.push(json!({
                "label": label,
                "kind": slot.kind,
                "text": text.trim(),
                "fix": fix,
            }));
            count += 1;
        }
        report.push(json!({
            "page": page.index,
            "archetype": page.archetype,
            "section": page.section,
            "unfilled": unfilled,
        }));
    }
    Ok(json!({"ok": true, "path": b.path, "count": count, "pages": report}))
}

/// `storyboard.run`: run a command script against the storyboard.
///
/// The script is DesignCraft's script format: a JSON array of steps, JSON lines, or compact
/// `command.id {json}` lines. A later step can use an earlier result with `$0.path`.
///
/// A `storyboard.` command is addressed by label (`storyboard.set {"page": 1, "fields": …}`);
/// any other command id is DesignCraft's own, run on the open document. The document is written
/// only when every step succeeded, so a failed script leaves it as it was.
pub fn cmd_storyboard_run(s: &mut Session, p: &Value) -> Result<Value> {
    let text = match p
        .get("script_file")
        .and_then(Value::as_str)
        .filter(|path| !path.trim().is_empty())
    {
        Some(path) => {
            let resolved = crate::workspace::resolve_workspace_path(s, path);
            std::fs::read_to_string(&resolved)
                .map_err(|e| bad(&format!("{}: {e}", resolved.display())))?
        }
        None => p
            .get("script")
            .and_then(Value::as_str)
            .ok_or_else(|| bad("give `script` (the script text) or `script_file`"))?
            .to_string(),
    };
    let steps = designcraft_engine::script::parse(&text).map_err(|e| bad(&e))?;
    let mut b = Board::open(&doc_path(s, p)?)?;
    let session: &Session = s;
    let report = designcraft_engine::script::run(&steps, |command, params| {
        match command.strip_prefix("storyboard.") {
            Some("set") => storyboard_step_set(session, &mut b, &params),
            _ => b.dc.execute(command, &params).map_err(|e| e.to_string()),
        }
    });
    // A failed step leaves the document unwritten.
    if report.failed.is_none() {
        b.save()?;
    }
    let mut out = report.to_json();
    out["ok"] = json!(report.failed.is_none());
    out["path"] = json!(b.path);
    Ok(out)
}

/// One `storyboard.set` step of a script: text and images, addressed by label.
fn storyboard_step_set(
    s: &Session,
    b: &mut Board,
    params: &Value,
) -> std::result::Result<Value, String> {
    let pages = b.pages().map_err(|e| e.to_string())?;
    let index = page_index(&pages, params).map_err(|e| e.to_string())?;
    let labels = pages[index].labels();
    fill_scene(s, b, &pages[index], &labels, params).map_err(|e| e.to_string())?;
    Ok(json!({"page": index, "archetype": pages[index].archetype}))
}

/// `storyboard.lyrics`: insert the song's lyrics into the scene pages, locked.
///
/// The lyrics sheet is the source of the words and the transcript the source of their timing.
/// **Both are read only**: this command never writes to them. What it puts in the storyboard is
/// locked, so the lyrics in the document stay a read-only projection of the source.
///
/// Only verse and chorus sections are pulled, and each printed line takes its first occurrence
/// in the recording, so a song that repeats its chorus contributes one set of lines.
pub fn cmd_storyboard_lyrics(s: &mut Session, p: &Value) -> Result<Value> {
    let lyrics = p
        .get("lyrics")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("give `lyrics` (the canonical lyrics sheet .json)"))?;
    let sheet = read_json(s, lyrics)?;
    let wanted: Option<Vec<String>> = p.get("sections").and_then(Value::as_str).map(|list| {
        list.split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    });
    // The first time the recording says each printed line.
    let mut timings: HashMap<String, (f64, f64)> = HashMap::new();
    if let Some(transcript) = p.get("transcript").and_then(Value::as_str) {
        for para in read_json(s, transcript)?["paragraphs"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let text = para["text"].as_str().unwrap_or_default().trim().to_string();
            let (Some(start), Some(end)) = (para["start"].as_f64(), para["end"].as_f64()) else {
                continue;
            };
            timings.entry(text).or_insert((start, end));
        }
    }
    let structure: Vec<String> = sheet["structure_as_printed"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let lines = sheet["lines"].as_array().cloned().unwrap_or_default();
    let per = p.get("per").and_then(Value::as_u64).unwrap_or(1).max(1) as usize;
    // The cues, in the printed order.
    let mut cues: Vec<Value> = Vec::new();
    for section in &structure {
        match &wanted {
            Some(list) if !list.iter().any(|s| s == section) => continue,
            None if !is_verse_or_chorus(section) => continue,
            _ => {}
        }
        let section_lines: Vec<&Value> = lines
            .iter()
            .filter(|l| l["section"].as_str() == Some(section.as_str()))
            .collect();
        for chunk in section_lines.chunks(per) {
            let text = chunk
                .iter()
                .filter_map(|l| l["text"].as_str())
                .map(|t| t.trim_end_matches(['.', ',']))
                .collect::<Vec<_>>()
                .join(" / ");
            let start = chunk
                .iter()
                .filter_map(|l| l["text"].as_str())
                .filter_map(|t| timings.get(t.trim()))
                .map(|t| t.0)
                .next();
            let end = chunk
                .iter()
                .filter_map(|l| l["text"].as_str())
                .filter_map(|t| timings.get(t.trim()))
                .map(|t| t.1)
                .next_back();
            cues.push(json!({
                "section": section,
                "text": text,
                "start": start,
                "end": end,
                "timecode": match (start, end) {
                    (Some(a), Some(b)) => format!("{} – {}", clock(a), clock(b)),
                    _ => String::new(),
                },
            }));
        }
    }
    // The scene pages, in document order, take the cues in order.
    let mut b = Board::open(&doc_path(s, p)?)?;
    let pages = b.pages()?;
    let scene_pages: Vec<usize> = pages
        .iter()
        .filter(|pg| pg.archetype == "scene")
        .map(|pg| pg.index)
        .collect();
    let mut written = Vec::new();
    for (n, index) in scene_pages.iter().enumerate() {
        let Some(cue) = cues.get(n) else { break };
        let mut fields = serde_json::Map::new();
        fields.insert(
            "cue".into(),
            json!(format!("\u{201c}{}\u{201d}", cue["text"])),
        );
        if !cue["timecode"].as_str().unwrap_or_default().is_empty() {
            fields.insert("time".into(), json!(cue["timecode"]));
        }
        let request = json!({"fields": Value::Object(fields), "lock": true});
        let labels = pages[*index].labels();
        fill_scene(s, &mut b, &pages[*index], &labels, &request)?;
        written.push(json!({
            "page": index,
            "scene": n + 1,
            "section": cue["section"],
            "cue": cue["text"],
            "time": cue["timecode"],
        }));
    }
    b.save()?;
    Ok(json!({
        "ok": true,
        "path": b.path,
        "lyrics": lyrics,
        "cues": cues.len(),
        "written": written,
    }))
}

/// A time in seconds as `m:ss.ss`.
fn clock(seconds: f64) -> String {
    let m = (seconds / 60.0).floor() as u64;
    let s = seconds - (m as f64) * 60.0;
    format!("{m}:{:05.2}", s)
}

/// `storyboard.new`: a new storyboard from the embedded neutral template.
pub fn cmd_storyboard_new(s: &mut Session, p: &Value) -> Result<Value> {
    let path = doc_path(s, p)?;
    if path.exists() && !p.get("overwrite").and_then(Value::as_bool).unwrap_or(false) {
        return Err(bad(&format!(
            "{} exists; pass overwrite=true to replace it",
            path.display()
        )));
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    craft_atomic::atomic_write(&path, TEMPLATE)?;
    Ok(json!({"ok": true, "path": path, "scenes": 1}))
}

fn scene_number_for(pages: &[PageSlots], index: usize) -> Option<usize> {
    (pages.get(index)?.archetype == "scene").then(|| {
        pages
            .iter()
            .filter(|pg| pg.archetype == "scene")
            .position(|pg| pg.index == index)
            .map(|n| n + 1)
    })?
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

/// `storyboard.pages`: every page with its archetype, section and labelled slots.
pub fn cmd_storyboard_pages(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Board::open(&doc_path(s, p)?)?;
    let pages = b.pages()?;
    let list = pages.iter().map(PageSlots::to_json).collect::<Vec<_>>();
    Ok(json!({"ok": true, "path": b.path, "pages": list}))
}

/// `storyboard.slots`: the labelled slots of one page.
pub fn cmd_storyboard_slots(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Board::open(&doc_path(s, p)?)?;
    let pages = b.pages()?;
    let index = page_index(&pages, p)?;
    let page = &pages[index];
    Ok(json!({
        "ok": true,
        "path": b.path,
        "page": page.index,
        "scene": scene_number_for(&pages, index),
        "archetype": page.archetype,
        "section": page.section,
        "slots": page.slots_json(),
    }))
}

/// `storyboard.set`: change one page's text, images and contact strip, then save.
pub fn cmd_storyboard_set(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Board::open(&doc_path(s, p)?)?;
    let pages = b.pages()?;
    let index = page_index(&pages, p)?;
    let labels = pages[index].labels();
    fill_scene(s, &mut b, &pages[index], &labels, p)?;
    b.save()?;
    let page = &pages[index];
    Ok(json!({
        "ok": true,
        "page": page.index,
        "scene": scene_number_for(&pages, index),
        "archetype": page.archetype,
        "path": b.path
    }))
}

/// The leaf numbers a page carries, when the document numbers two leaves per page.
///
/// The template's first page is 01/02, so page P carries leaves 2P+1 and 2P+2. A document
/// that numbers its pages another way gets no numbers from this.
fn leaf_numbers(pages: &[PageSlots], index: usize) -> Option<(String, String)> {
    let first = pages.first()?;
    let left = first
        .slots
        .iter()
        .find(|s| s.label.ends_with("page_no_left"))
        .and_then(|s| s.text.as_deref())?;
    let right = first
        .slots
        .iter()
        .find(|s| s.label.ends_with("page_no_right"))
        .and_then(|s| s.text.as_deref())?;
    (left.trim() == "01" && right.trim() == "02").then(|| {
        (
            format!("{:02}", 2 * index + 1),
            format!("{:02}", 2 * index + 2),
        )
    })
}

/// `storyboard.add`: copy a template page (default: the last one of that kind) and fill it.
pub fn cmd_storyboard_add(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Board::open(&doc_path(s, p)?)?;
    let pages = b.pages()?;
    let archetype = p
        .get("archetype")
        .and_then(Value::as_str)
        .unwrap_or("scene");
    // Accept `page`, `scene`, or `label` via `page_index`, or `from` (0-based spread index).
    let from = if p.get("page").is_some() || p.get("scene").is_some() || p.get("label").is_some() {
        page_index(&pages, p)?
    } else if let Some(n) = p.get("from").and_then(Value::as_u64) {
        let index = n as usize;
        if index < pages.len() {
            index
        } else {
            return Err(bad(&format!(
                "no page {n}; the storyboard has {} pages",
                pages.len()
            )));
        }
    } else {
        pages
            .iter()
            .filter(|pg| pg.archetype == archetype)
            .map(|pg| pg.index)
            .next_back()
            .ok_or_else(|| {
                bad(&format!(
                    "this storyboard has no `{archetype}` page to copy"
                ))
            })?
    };
    // Duplicate and place in one undo step: `to` is the page index the copy moves to.
    let mut params = json!({"spread": from});
    match p.get("at").and_then(Value::as_u64) {
        Some(at) => params["to"] = json!(at),
        None => params["atEnd"] = json!(true),
    }
    let dup = b.run("layout.pages.duplicateSpread", params)?;
    let index = dup["page"]
        .as_u64()
        .ok_or_else(|| fail("no page index from duplicateSpread"))? as usize;
    let pages = b.pages()?;
    let page = pages
        .get(index)
        .ok_or_else(|| fail("the new page was not found"))?;
    let labels = page.labels();
    fill_scene(s, &mut b, page, &labels, p)?;
    // A copied scene must not claim to be the scene it came from.
    let asked_for = |label: &str| {
        p.get("fields")
            .and_then(Value::as_object)
            .is_some_and(|f| f.contains_key(label))
    };
    let scene_number = scene_number_for(&pages, index);
    if archetype == "scene"
        && !asked_for("scene_id")
        && let (Some(number), Some(id)) = (scene_number, labels.get("scene_id"))
    {
        b.set_text(*id, &format!("SCENE {number:02}"))?;
    }
    // The new page carries its own leaf numbers, when the document numbers them that way.
    if let Some((left, right)) = leaf_numbers(&pages, index) {
        for (suffix, text) in [("page_no_left", &left), ("page_no_right", &right)] {
            if !asked_for(suffix)
                && let Some(slot) = pages[index]
                    .slots
                    .iter()
                    .find(|s| s.label.ends_with(suffix))
            {
                b.set_text(slot.id, text)?;
            }
        }
    }
    b.save()?;
    Ok(json!({
        "ok": true,
        "page": index,
        "scene": scene_number,
        "archetype": archetype,
        "path": b.path
    }))
}

/// `storyboard.section`: add a section (a parent page) and apply it to pages.
pub fn cmd_storyboard_section(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Board::open(&doc_path(s, p)?)?;
    let name = p
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("give `name` (the section prefix, e.g. \"B\")"))?;
    let created = b.run("layout.parents.new", json!({"prefix": name}))?;
    let label = created["label"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| format!("{name}-Parent"));
    if let Some(base) = p.get("based_on").and_then(Value::as_str) {
        b.run(
            "layout.parents.options",
            json!({"parent": label, "basedOn": base}),
        )?;
    }
    let pages: Vec<u64> = p
        .get("pages")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default();
    if !pages.is_empty() {
        b.run(
            "layout.pages.applyParent",
            json!({"pages": pages, "parent": label}),
        )?;
    }
    b.save()?;
    Ok(json!({"ok": true, "section": label, "pages": pages, "path": b.path}))
}

/// `storyboard.overlay`: place new art over a slot, keeping the original underneath for review.
///
/// The revision is a new object on its own layer above the art it covers; the original frame
/// stays in the document, hidden, so it can be shown again to compare.
pub fn cmd_storyboard_overlay(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Board::open(&doc_path(s, p)?)?;
    let pages = b.pages()?;
    let index = page_index(&pages, p)?;
    let label = p
        .get("slot")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("give `slot` (the label of the image slot to cover)"))?;
    let image = p
        .get("image")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("give `image` (a file path)"))?;
    let resolved_image = crate::workspace::resolve_workspace_path(s, image);
    let slot = pages[index]
        .slot(label)
        .ok_or_else(|| bad(&format!("page {index} has no slot `{label}`")))?;
    let bounds = slot
        .bounds
        .ok_or_else(|| bad(&format!("slot `{label}` has no bounds")))?;
    // A layer of its own, so the revision sits over the art and the original reads underneath.
    let layer_name = p.get("layer").and_then(Value::as_str).unwrap_or("Revision");
    let layer = b.run("layer.new", json!({"name": layer_name}))?;
    let layer_id = layer["id"].as_u64();
    if let Some(id) = layer_id {
        b.run("layer.activate", json!({"id": id}))?;
    }
    b.run("place.load", json!({"path": resolved_image}))?;
    let placed = b.run("place.drop", json!({"spread": index, "rect": bounds}))?;
    let overlay = placed["id"].as_u64();
    // The original stays in the document; hide it so the revision reads, unhide it to review.
    if p.get("hide_original")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        b.run("object.setFlags", json!({"ids": [slot.id], "hidden": true}))?;
    }
    b.save()?;
    Ok(json!({
        "ok": true,
        "page": index,
        "slot": label,
        "overlay": overlay,
        "preserved": slot.id,
        "layer": layer_id,
        "path": b.path,
    }))
}

fn render_png(b: &mut Board, page: usize, scale: f64) -> Result<Vec<u8>> {
    let st = b.dc.doc().map_err(fail)?;
    let mut r = designcraft_render::Renderer::new();
    let opts = designcraft_render::RenderOptions {
        printing_only: true,
        ..Default::default()
    };
    let img = r
        .render_page(&st.doc, &b.dc.cache, page, scale, true, &opts)
        .ok_or_else(|| bad("no such page"))?;
    Ok(img.to_png())
}

/// `storyboard.thumbnail`: render one page (`out`) or all scene thumbnails (`out_dir`) as PNG.
pub fn cmd_storyboard_thumbnail(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Board::open(&doc_path(s, p)?)?;
    let pages = b.pages()?;
    let scale = p.get("scale").and_then(Value::as_f64).unwrap_or(0.5);
    if let Some(raw_dir) = p
        .get("out_dir")
        .and_then(Value::as_str)
        .filter(|d| !d.trim().is_empty())
    {
        let out_dir = crate::workspace::resolve_workspace_path(s, raw_dir);
        std::fs::create_dir_all(&out_dir)?;
        let mut files = Vec::new();
        for (scene_idx, pg) in pages
            .iter()
            .filter(|pg| pg.archetype == "scene")
            .enumerate()
        {
            let scene_no = scene_idx + 1;
            let file_path = out_dir.join(format!("thumb-{scene_no}.png"));
            let png = render_png(&mut b, pg.index, scale)?;
            craft_atomic::atomic_write(&file_path, &png)?;
            files.push(file_path.to_string_lossy().into_owned());
        }
        return Ok(json!({
            "ok": true,
            "out_dir": out_dir,
            "count": files.len(),
            "files": files,
        }));
    }
    let index = page_index(&pages, p)?;
    let raw_out = p
        .get("out")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("missing `out` .png path (or `out_dir`)"))?;
    let out = crate::workspace::resolve_workspace_path(s, raw_out);
    let png = render_png(&mut b, index, scale)?;
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    craft_atomic::atomic_write(&out, &png)?;
    Ok(json!({
        "ok": true,
        "page": index,
        "scene": scene_number_for(&pages, index),
        "archetype": pages[index].archetype,
        "out": out
    }))
}

/// `storyboard.export`: the whole storyboard as a PDF, and optionally every spread as a PNG.
pub fn cmd_storyboard_export(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Board::open(&doc_path(s, p)?)?;
    let mut done = serde_json::Map::new();
    if let Some(raw_pdf) = p.get("pdf").and_then(Value::as_str) {
        let pdf = crate::workspace::resolve_workspace_path(s, raw_pdf);
        if let Some(dir) = pdf.parent() {
            std::fs::create_dir_all(dir)?;
        }
        b.run("file.exportPdf", json!({"path": pdf}))?;
        done.insert("pdf".into(), json!(pdf));
    }
    if let Some(raw_dir) = p.get("png_dir").and_then(Value::as_str) {
        let dir = crate::workspace::resolve_workspace_path(s, raw_dir);
        std::fs::create_dir_all(&dir)?;
        let scale = p.get("scale").and_then(Value::as_f64).unwrap_or(1.0);
        let pages = b.frames(&[])?.len();
        for page in 0..pages {
            let png = render_png(&mut b, page, scale)?;
            craft_atomic::atomic_write(&dir.join(format!("page-{:03}.png", page + 1)), &png)?;
        }
        done.insert("png_dir".into(), json!(dir));
    }
    if done.is_empty() {
        return Err(bad("give `pdf` and/or `png_dir`"));
    }
    Ok(json!({"ok": true, "exported": done}))
}

#[cfg(test)]
#[path = "../../tests/storyboard.rs"]
mod tests;
