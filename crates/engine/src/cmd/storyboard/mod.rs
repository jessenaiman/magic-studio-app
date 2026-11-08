//! Storyboard: one lightweight DesignCraft document, edited by object label.
//!
//! The document is a `.designcraft` file made from the embedded neutral template. Every
//! editable frame carries an `object.label` (`scene_id`, `title`, `cue`, `hero_image`,
//! `ref_1`..`ref_12` ...), and a scene is one spread holding a `scene_id` frame. The commands
//! here open the file in `designcraft-engine`, change labelled frames, and save it back.
//! There is no second data model: the file is the storyboard.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

const TEMPLATE: &[u8] = include_bytes!("../../../templates/storyboard.designcraft");
const MAX_REFS: usize = 12;

/// The page archetypes the template defines, each identified by one marker label.
/// A page's archetype is read from the document, so the template can gain a page type
/// without touching this table; the table only says which marker to look for.
pub const ARCHETYPES: &[(&str, &str)] = &[("cover", "cover_title"), ("world", "world_heading"), ("char", "char_heading"), ("scene", "scene_id")];

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
    empty: bool,
}

/// One page of the storyboard: its archetype, the section it belongs to, and its slots.
struct PageSlots {
    index: usize,
    archetype: String,
    section: Option<String>,
    /// The spread's own rectangle `[x0, y0, x1, y1]`, the space slot `bounds` are given in.
    bounds: [f64; 4],
    slots: Vec<Slot>,
}

/// Inspector groups, in the order the Storyboard view lists them.
///
/// These come from slot label conventions in the embedded template, not from DesignCraft: the
/// document carries no semantic grouping, so this table is presentation only.
const SLOT_GROUPS: &[&str] = &["Scene map", "Locations & references", "Design brief", "Artwork", "Notes and sources", "Template formatting"];

/// The marker labels whose text names a page, in priority order.
const TITLE_LABELS: &[&str] = &["cover_title", "world_heading", "char_heading", "scene_title", "scene_id"];

/// `scene_` followed by a scene number 1-9 anywhere in the label (`scene_1_title`).
fn has_scene_number(label: &str) -> bool {
    label.match_indices("scene_").any(|(at, m)| label[at + m.len()..].bytes().next().is_some_and(|c| (b'1'..=b'9').contains(&c)))
}

/// The inspector group a slot belongs to, from its label and kind.
fn slot_group(label: &str, kind: &str) -> &'static str {
    const FORMATTING_SUFFIXES: &[&str] = &["_heading", "_kicker", "_badge", "_revision", "_tag", "_hint"];
    if label.contains("page_no_") || FORMATTING_SUFFIXES.iter().any(|s| label.ends_with(s)) {
        "Template formatting"
    } else if label.contains("reg_") || label.contains("ref_") || label.contains("reference") {
        "Locations & references"
    } else if label.contains("scene_map") || label.contains("map_") || has_scene_number(label) {
        "Scene map"
    } else if kind == "image" {
        "Artwork"
    } else if ["note", "caption", "credit", "source"].iter().any(|w| label.contains(w)) {
        "Notes and sources"
    } else {
        "Design brief"
    }
}

/// The readable name of a slot label (`cover_songmap_3` -> `Scene 3 · visual`).
fn slot_display(label: &str) -> String {
    let rest = ["cover_", "world_", "char_", "scene_"].iter().find_map(|p| label.strip_prefix(p)).unwrap_or(label);
    let numbered = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
    if let Some(n) = rest.strip_prefix("songmap_action_").filter(|n| numbered(n)) {
        return format!("Scene {n} · action and location");
    }
    if let Some(n) = rest.strip_prefix("songmap_").filter(|n| numbered(n)) {
        return format!("Scene {n} · visual");
    }
    rest.replace('_', " ")
}

impl PageSlots {
    /// The page's name: the text of its first title marker, or its archetype and number.
    fn title(&self) -> String {
        TITLE_LABELS
            .iter()
            .find_map(|marker| self.slot(marker).and_then(|s| s.text.as_deref()).map(str::trim).filter(|t| !t.is_empty()))
            .map_or_else(|| format!("{} {}", self.archetype, self.index + 1), str::to_string)
    }

    /// The groups this page's editable slots fall into, in [`SLOT_GROUPS`] order.
    fn groups(&self) -> Vec<&'static str> {
        SLOT_GROUPS.iter().copied().filter(|g| self.slots.iter().any(|s| s.editable() && slot_group(&s.label, s.kind) == *g)).collect()
    }
}

impl Slot {
    /// A visible text or image frame the Storyboard view offers for editing.
    fn editable(&self) -> bool {
        !self.hidden && self.kind != "other"
    }
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
            "title": self.title(),
            "bounds": self.bounds,
            "groups": self.groups(),
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
                        "group": slot_group(&s.label, s.kind),
                        "display": slot_display(&s.label),
                        "editable": s.editable(),
                        "text": s.text,
                        "image": s.image,
                        "bounds": s.bounds,
                        "hidden": s.hidden,
                        "empty": s.empty,
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

/// Extract `document.json` from a `.designcraft` ZIP archive via `flate2`.
fn read_designcraft_document_json(path: &Path) -> Result<Value> {
    use std::io::Read;
    let bytes = std::fs::read(path)?;
    if bytes.len() < 22 || &bytes[0..4] != b"PK\x03\x04" {
        return Err(fail("not a valid .designcraft archive"));
    }
    let eocd = (0..=bytes.len() - 22).rev().find(|&i| &bytes[i..i + 4] == b"PK\x05\x06").ok_or_else(|| fail("corrupt .designcraft zip central directory"))?;
    let total_entries = u16::from_le_bytes([bytes[eocd + 10], bytes[eocd + 11]]) as usize;
    let cd_offset = u32::from_le_bytes([bytes[eocd + 16], bytes[eocd + 17], bytes[eocd + 18], bytes[eocd + 19]]) as usize;
    let mut pos = cd_offset;
    for _ in 0..total_entries {
        if pos + 46 > bytes.len() || &bytes[pos..pos + 4] != b"PK\x01\x02" {
            break;
        }
        let method = u16::from_le_bytes([bytes[pos + 10], bytes[pos + 11]]);
        let comp_size = u32::from_le_bytes([bytes[pos + 20], bytes[pos + 21], bytes[pos + 22], bytes[pos + 23]]) as usize;
        let name_len = u16::from_le_bytes([bytes[pos + 28], bytes[pos + 29]]) as usize;
        let extra_len = u16::from_le_bytes([bytes[pos + 30], bytes[pos + 31]]) as usize;
        let comment_len = u16::from_le_bytes([bytes[pos + 32], bytes[pos + 33]]) as usize;
        let local_offset = u32::from_le_bytes([bytes[pos + 42], bytes[pos + 43], bytes[pos + 44], bytes[pos + 45]]) as usize;
        let name = std::str::from_utf8(&bytes[pos + 46..pos + 46 + name_len]).unwrap_or("");
        if name == "document.json" {
            let l_name_len = u16::from_le_bytes([bytes[local_offset + 26], bytes[local_offset + 27]]) as usize;
            let l_extra_len = u16::from_le_bytes([bytes[local_offset + 28], bytes[local_offset + 29]]) as usize;
            let data_start = local_offset + 30 + l_name_len + l_extra_len;
            let data = &bytes[data_start..data_start + comp_size];
            let raw = match method {
                0 => data.to_vec(),
                8 => {
                    let mut out = Vec::new();
                    flate2::read::DeflateDecoder::new(data).read_to_end(&mut out).map_err(fail)?;
                    out
                }
                other => return Err(fail(format!("unsupported zip compression method {other}"))),
            };
            return serde_json::from_slice(&raw).map_err(fail);
        }
        pos += 46 + name_len + extra_len + comment_len;
    }
    Err(fail("document.json not found in .designcraft archive"))
}

use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

struct ScopedTempDir(PathBuf);

impl ScopedTempDir {
    fn new(prefix: &str) -> Result<Self> {
        let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("{prefix}-{}-{seq}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(fail)?;
        Ok(Self(dir))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScopedTempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An open storyboard document and the frames of each of its scenes.
struct Board {
    path: PathBuf,
    work_path: PathBuf,
    _temp_dir: ScopedTempDir,
    stamp: Option<crate::link_watch::FileStamp>,
}

impl Board {
    fn open(path: &Path) -> Result<Self> {
        let stamp = crate::link_watch::FileStamp::of(path);
        let temp_dir = ScopedTempDir::new("magic-storyboard-work")?;
        let work_path = temp_dir.path().join("storyboard.designcraft");
        std::fs::copy(path, &work_path).map_err(fail)?;
        let _ = read_designcraft_document_json(&work_path)?;
        crate::link_watch::save_guard(path, stamp)?;
        Ok(Self { path: path.to_path_buf(), work_path, _temp_dir: temp_dir, stamp })
    }

    fn run(&mut self, id: &str, params: Value) -> Result<Value> {
        self.run_batch(&[(id, params)])?.into_iter().next().ok_or_else(|| fail(format!("{id}: no result")))
    }

    fn run_batch(&mut self, steps: &[(&str, Value)]) -> Result<Vec<Value>> {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let mutates = steps.iter().any(|(id, _)| !matches!(*id, "document.inspect" | "object.findByLabel" | "story.get" | "preflight.run"));
        let cli = crate::workspace::find_executable("designcraft-cli").map_err(fail)?;
        let mut cmd = Command::new(&cli);
        cmd.arg("script").arg("-").arg("--in").arg(&self.work_path);
        if mutates {
            cmd.arg("--save").arg(&self.work_path);
        }
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(fail)?;
        let payload: Vec<Value> = steps.iter().map(|(id, params)| json!({"command": id, "params": params})).collect();
        let bytes = serde_json::to_vec(&payload).map_err(fail)?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&bytes).map_err(fail)?;
        }
        let out = child.wait_with_output().map_err(fail)?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if let Ok(report) = serde_json::from_slice::<Value>(&out.stdout)
                && let Some(err) = report["failed"]["error"].as_str()
            {
                return Err(fail(err));
            }
            return Err(fail(stderr.trim()));
        }
        let report: Value = serde_json::from_slice(&out.stdout).map_err(fail)?;
        if let Some(err) = report["failed"]["error"].as_str() {
            return Err(fail(err));
        }
        Ok(report["results"].as_array().cloned().unwrap_or_default())
    }

    fn save(&mut self) -> Result<()> {
        let path = self.path.clone();
        crate::link_watch::save_guard(&path, self.stamp)?;
        let bytes = std::fs::read(&self.work_path)?;
        craft_atomic::atomic_write(&path, &bytes)?;
        self.stamp = crate::link_watch::FileStamp::of(&path);
        Ok(())
    }

    /// Label -> frame id per spread, for `labels` and `scene_id` (which marks a scene spread).
    fn frames(&mut self, labels: &[String]) -> Result<Vec<Labels>> {
        let doc = read_designcraft_document_json(&self.work_path)?;
        let spreads = doc["spreads"].as_array().cloned().unwrap_or_default();
        let want: std::collections::HashSet<&str> = labels.iter().map(String::as_str).chain(["scene_id"]).collect();
        let mut out = vec![HashMap::new(); spreads.len()];
        for (i, sp) in spreads.iter().enumerate() {
            for item in sp["items"].as_array().into_iter().flatten() {
                if let (Some(id), Some(label)) = (item["id"].as_u64(), item["label"].as_str())
                    && !label.is_empty()
                    && want.contains(label)
                {
                    out[i].insert(label.to_string(), id);
                }
            }
        }
        Ok(out)
    }

    /// Spread indexes that are scenes (hold a `scene_id` frame), in document order.
    fn scenes(&mut self, labels: &[String]) -> Result<(Vec<usize>, Vec<Labels>)> {
        let frames = self.frames(labels)?;
        let scenes = (0..frames.len()).filter(|&i| frames[i].contains_key("scene_id")).collect();
        Ok((scenes, frames))
    }

    fn text(&mut self, frame: u64) -> Result<String> {
        let doc = read_designcraft_document_json(&self.work_path)?;
        for spread in doc["spreads"].as_array().into_iter().flatten() {
            for item in spread["items"].as_array().into_iter().flatten() {
                if item["id"].as_u64() == Some(frame) {
                    if let Some(story_id) = item["content"]["story"].as_u64() {
                        return Ok(doc["stories"][story_id.to_string()]["text"].as_str().unwrap_or_default().to_string());
                    }
                    return Ok(String::new());
                }
            }
        }
        let r = self.run("story.get", json!({"frame": frame}))?;
        Ok(r["text"].as_str().unwrap_or_default().to_string())
    }

    fn set_text(&mut self, frame: u64, text: &str) -> Result<()> {
        let doc = read_designcraft_document_json(&self.work_path)?;
        let mut story_id = None;
        for spread in doc["spreads"].as_array().into_iter().flatten() {
            for item in spread["items"].as_array().into_iter().flatten() {
                if item["id"].as_u64() == Some(frame) {
                    story_id = item["content"]["story"].as_u64();
                    break;
                }
            }
        }
        let story = match story_id {
            Some(id) => json!(id),
            None => {
                let r = self.run("story.get", json!({"frame": frame}))?;
                r.get("story").or_else(|| r.get("id")).cloned().ok_or_else(|| fail("frame has no story"))?
            }
        };
        self.run("story.setText", json!({"story": story, "text": text}))?;
        Ok(())
    }

    /// Lock a frame so it cannot be edited afterwards.
    fn lock(&mut self, frame: u64) -> Result<()> {
        self.run("object.setFlags", json!({"ids": [frame], "locked": true}))?;
        Ok(())
    }

    fn set_image(&mut self, frame: u64, hint: Option<u64>, path: &str, fit_mode: &str) -> Result<()> {
        let mut steps = vec![
            ("place.load", json!({"path": path})),
            ("place.drop", json!({"x": 0, "y": 0, "frame": frame})),
            ("object.fit", json!({"mode": fit_mode, "ids": [frame]})),
        ];
        if let Some(h) = hint {
            steps.push(("object.setFlags", json!({"ids": [h], "hidden": true})));
        }
        self.run_batch(&steps)?;
        Ok(())
    }

    /// Every page of the document with the labelled slots it carries.
    ///
    /// The slots come from the loaded document, so the template may gain a slot without this
    /// table changing; the bounds come from the engine's own inspect, which reports them in
    /// spread space.
    fn pages(&mut self) -> Result<Vec<PageSlots>> {
        let inspect = self.run("document.inspect", json!({}))?;
        let rect = |b: &Value| -> Option<[f64; 4]> {
            let b = b.as_array().filter(|b| b.len() == 4)?;
            Some([b[0].as_f64()?, b[1].as_f64()?, b[2].as_f64()?, b[3].as_f64()?])
        };
        // Each spread's rectangle is the union of its pages' bounds.
        let spread_bounds: Vec<Option<[f64; 4]>> = inspect["spreads"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|spread| {
                spread["pages"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|pg| rect(&pg["bounds"]))
                    .reduce(|a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])])
            })
            .collect();
        let mut bounds: HashMap<u64, [f64; 4]> = HashMap::new();
        for spread in inspect["spreads"].as_array().into_iter().flatten() {
            for item in spread["items"].as_array().into_iter().flatten() {
                let (Some(id), Some(b)) = (item["id"].as_u64(), item["bounds"].as_array()) else {
                    continue;
                };
                if b.len() == 4 {
                    bounds.insert(id, [b[0].as_f64().unwrap_or(0.0), b[1].as_f64().unwrap_or(0.0), b[2].as_f64().unwrap_or(0.0), b[3].as_f64().unwrap_or(0.0)]);
                }
            }
        }
        let doc = read_designcraft_document_json(&self.work_path)?;
        // Parent spread id -> its label ("A-Parent"), for the section a page belongs to.
        let parents: HashMap<u64, String> = doc["parents"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| {
                let id = p["id"].as_u64()?;
                let prefix = p["parent"]["prefix"].as_str()?;
                let name = p["parent"]["name"].as_str()?;
                let label = if prefix.is_empty() { name.to_string() } else { format!("{prefix}-{name}") };
                Some((id, label))
            })
            .collect();
        let stories = &doc["stories"];
        let assets = &doc["assets"];
        let mut pages = Vec::new();
        for (index, spread) in doc["spreads"].as_array().into_iter().flatten().enumerate() {
            let mut labels: Labels = HashMap::new();
            let mut slots = Vec::new();
            for item in spread["items"].as_array().into_iter().flatten() {
                let label = item["label"].as_str().unwrap_or_default();
                if label.is_empty() {
                    continue;
                }
                let Some(id) = item["id"].as_u64() else { continue };
                let (kind, text, image) = match item["content"]["type"].as_str() {
                    Some("text") => {
                        let text = item["content"]["story"].as_u64().and_then(|sid| stories[sid.to_string()]["text"].as_str()).map(str::to_string);
                        ("text", text, None)
                    }
                    Some("graphic") => {
                        let image = item["content"]["asset"].as_u64().and_then(|aid| assets[aid.to_string()]["link"].as_str()).map(|raw| {
                            craft_docpath::resolve_absolute(raw, Some(&self.path)).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| raw.to_string())
                        });
                        ("image", None, image)
                    }
                    _ => ("other", None, None),
                };
                labels.insert(label.to_string(), id);
                slots.push(Slot {
                    label: label.to_string(),
                    id,
                    kind,
                    text,
                    image,
                    bounds: bounds.get(&id).copied(),
                    hidden: item["hidden"].as_bool().unwrap_or(false),
                    empty: kind == "other",
                });
            }
            for slot in &mut slots {
                let numbered = |value: &str| !value.is_empty() && value.bytes().all(|c| c.is_ascii_digit());
                let scene_map = slot.label.strip_prefix("cover_songmap_").is_some_and(numbered);
                let reference_thumb = slot.label.rsplit_once("_thumb_").is_some_and(|(_, suffix)| numbered(suffix));
                if slot.kind == "other" && (labels.contains_key(&format!("{}_hint", slot.label)) || scene_map || reference_thumb) {
                    slot.kind = "image";
                }
                slot.empty &= slot.kind == "image";
            }
            let section = spread["pages"].as_array().and_then(|ps| ps.first()).and_then(|p| p["parent"].as_u64()).and_then(|pid| parents.get(&pid).cloned());
            let bounds =
                spread_bounds.get(index).copied().flatten().ok_or_else(|| fail(format!("document.inspect reports no page bounds for spread {index}")))?;
            pages.push(PageSlots { index, archetype: archetype_of(&labels).to_string(), section, bounds, slots });
        }
        Ok(pages)
    }
}

/// Slot rectangles `[x0, y0, x1, y1]` for the contact strip with `n` pictures, and its band.
fn strip_layout(n: usize, strip_bounds: Option<[f64; 4]>) -> ([f64; 4], Vec<[f64; 4]>) {
    let [x0, base_y0, x1, base_y1] = strip_bounds.filter(|b| b[2] > b[0] && b[3] > b[1]).unwrap_or([846.0, 800.0, 1556.0, 912.0]);
    let (pad, gap) = (10.0, 6.0);
    let single = n <= 6;
    let (band, cols, top, bottom) = if single {
        ([x0, base_y0, x1, base_y1], n.max(1), base_y0 + 14.0, base_y1 - 14.0)
    } else {
        let y0 = base_y0 - 8.0;
        let y1 = base_y1 + 6.0;
        ([x0, y0, x1, y1], if n > 8 { 6 } else { 4 }, y0 + 16.0, y1 - 16.0)
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
    b.run("transform.set", json!({"x": r[0], "y": r[1], "width": r[2] - r[0], "height": r[3] - r[1], "ids": [frame]}))?;
    Ok(())
}

fn fill_refs(b: &mut Board, frames: &Labels, strip_bounds: Option<[f64; 4]>, refs: &[String]) -> Result<()> {
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
            b.set_image(frame, None, &refs[n - 1], "fillProportionally")?;
        }
    }
    Ok(())
}

/// Apply `fields`, `images` and `refs` from `p` to the scene whose frames are `frames`.
fn fill_scene(s: &Session, b: &mut Board, page: &PageSlots, frames: &Labels, p: &Value) -> Result<()> {
    for (label, value) in p.get("fields").and_then(Value::as_object).into_iter().flatten() {
        let text = value.as_str().ok_or_else(|| bad("`fields` values are strings"))?;
        let frame = *frames.get(label).ok_or_else(|| bad(&format!("this page has no slot labelled `{label}`")))?;
        b.set_text(frame, text)?;
        // A field written from a source is locked, so the document keeps it read-only.
        if p.get("lock").and_then(Value::as_bool).unwrap_or(false) {
            b.lock(frame)?;
        }
    }
    let fit_mode = p.get("fit").and_then(Value::as_str).unwrap_or("fillProportionally");
    for (label, value) in p.get("images").and_then(Value::as_object).into_iter().flatten() {
        let raw = value.as_str().ok_or_else(|| bad("`images` values are file paths"))?;
        let resolved = crate::workspace::resolve_workspace_path(s, raw);
        let frame = *frames.get(label).ok_or_else(|| bad(&format!("this page has no slot labelled `{label}`")))?;
        b.set_image(frame, frames.get(&format!("{label}_hint")).copied(), &resolved.to_string_lossy(), fit_mode)?;
    }
    if let Some(refs) = p.get("refs").and_then(Value::as_array) {
        let refs: Vec<String> =
            refs.iter().filter_map(|r| r.as_str().map(|raw| crate::workspace::resolve_workspace_path(s, raw).to_string_lossy().into_owned())).collect();
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
        return if index < pages.len() { Ok(index) } else { Err(bad(&format!("no page {n}; the storyboard has {} pages", pages.len()))) };
    }
    if let Some(label) = p.get("label").and_then(Value::as_str) {
        return pages.iter().find(|pg| pg.slot(label).is_some()).map(|pg| pg.index).ok_or_else(|| bad(&format!("no page holds a slot labelled `{label}`")));
    }
    if let Some(n) = p.get("scene").and_then(Value::as_u64) {
        let n = n as usize;
        return pages.iter().filter(|pg| pg.archetype == "scene").nth(n.wrapping_sub(1)).map(|pg| pg.index).ok_or_else(|| bad(&format!("no scene {n}")));
    }
    Err(bad("give `page` (index), `label` (a slot name) or `scene` (number)"))
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
    let text = std::fs::read_to_string(&path).map_err(|e| bad(&format!("{}: {e}", path.display())))?;
    serde_json::from_str(&text).map_err(|e| bad(&format!("{}: {e}", path.display())))
}

/// `storyboard.cues`: the song's verse and chorus, with the timing of each line.
///
/// The words come from the canonical lyrics sheet (the artist's printed words). The timings
/// come from the transcript, and each printed line takes its **first** occurrence: a recording
/// that sings the chorus twice contributes one set of lines, not two.
pub fn cmd_storyboard_cues(s: &mut Session, p: &Value) -> Result<Value> {
    let lyrics = p.get("lyrics").and_then(Value::as_str).ok_or_else(|| bad("give `lyrics` (the canonical lyrics sheet .json)"))?;
    let sheet = read_json(s, lyrics)?;
    let wanted: Option<Vec<String>> =
        p.get("sections").and_then(Value::as_str).map(|list| list.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect());
    // The sheet's printed structure gives the order.
    let structure: Vec<String> =
        sheet["structure_as_printed"].as_array().map(|list| list.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let lines = sheet["lines"].as_array().cloned().unwrap_or_default();
    // Timings: the first time the recording says each printed line.
    let mut timings: HashMap<String, (f64, f64)> = HashMap::new();
    if let Some(transcript) = p.get("transcript").and_then(Value::as_str) {
        for para in read_json(s, transcript)?["paragraphs"].as_array().into_iter().flatten() {
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
/// A bracketed template line ("[Project title]"), even after a static prefix line, marks
/// a slot waiting for content. A plain default ("MATERIAL & SCALE") is a heading.
fn template_defaults() -> Result<HashMap<String, String>> {
    let dir = std::env::temp_dir().join(format!("magic-storyboard-template-{}", std::process::id()));
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
            if text.lines().map(str::trim).any(|line| line.starts_with('[') && line.ends_with(']')) {
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
/// A slot whose text still matches a template default containing a bracketed line is unfilled. A hidden
/// hint is not one of them: the hint is hidden once its image is placed. The command that
/// fills a slot is reported with it, so a missing block traces straight to the CLI call that
/// changes it.
pub fn cmd_storyboard_check(s: &mut Session, p: &Value) -> Result<Value> {
    completion(s, p)
}

/// Saved DesignCraft document completeness, shared by CLI and the workspace indicators.
pub(crate) fn completion(s: &Session, p: &Value) -> Result<Value> {
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
                Some(base) if defaults.contains_key(&format!("{base}_hint")) => {
                    (base.to_string(), format!("magic-cli storyboard set --label {base} --image {base}=<path>"))
                }
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
    let preflight = b.run("preflight.run", json!({}))?;
    let errors = preflight["errors"].as_u64().unwrap_or(0);
    for (page, slots) in report.iter_mut().zip(&pages) {
        // Each editable slot: incomplete (unfilled, or an error on its frame), review (other
        // findings on its frame), empty (an image slot nothing requires), or complete.
        let unfilled: std::collections::HashSet<String> =
            page["unfilled"].as_array().into_iter().flatten().filter_map(|u| u["label"].as_str().map(str::to_string)).collect();
        let mut slot_report = serde_json::Map::new();
        for slot in slots.slots.iter().filter(|s| s.editable()) {
            let findings: Vec<&Value> = preflight["issues"].as_array().into_iter().flatten().filter(|issue| issue["item"].as_u64() == Some(slot.id)).collect();
            let missing = unfilled.contains(&slot.label);
            let status = if missing || findings.iter().any(|f| f["severity"] == "error") {
                "incomplete"
            } else if !findings.is_empty() {
                "review"
            } else if slot.kind == "image" && (slot.empty || slot.image.is_none()) {
                "empty"
            } else {
                "complete"
            };
            let messages: Vec<&Value> = findings.iter().map(|f| &f["message"]).collect();
            slot_report.insert(slot.label.clone(), json!({"status": status, "issues": messages}));
        }
        page["slots"] = Value::Object(slot_report);
        let issues: Vec<Value> = preflight["issues"].as_array().into_iter().flatten().filter(|issue| issue["page"] == page["page"]).cloned().collect();
        page["status"] = json!(if page["unfilled"].as_array().is_some_and(|v| !v.is_empty()) {
            "incomplete"
        } else if !issues.is_empty() {
            "review"
        } else {
            "complete"
        });
        page["issues"] = json!(issues);
    }
    let complete = !pages.is_empty() && count == 0 && errors == 0;
    Ok(json!({"ok": true, "path": b.path, "count": count, "page_count":pages.len(),
        "complete":complete, "status":if count > 0 || pages.is_empty() { "incomplete" } else if errors > 0 { "review" } else { "complete" },
        "pages": report, "preflight":preflight}))
}

struct ScriptStep {
    command: String,
    params: Value,
}

fn parse_script_step_object(v: &Value) -> std::result::Result<ScriptStep, String> {
    let command = v.get("command").or_else(|| v.get("id")).and_then(Value::as_str).ok_or_else(|| "step object has no `command`".to_string())?.to_string();
    let params = v.get("params").cloned().unwrap_or_else(|| Value::Object(Default::default()));
    Ok(ScriptStep { command, params })
}

fn parse_script_steps(text: &str) -> std::result::Result<Vec<ScriptStep>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if trimmed.starts_with('[') {
        let arr: Vec<Value> = serde_json::from_str(trimmed).map_err(|e| format!("JSON array script: {e}"))?;
        return arr.iter().map(parse_script_step_object).collect();
    }
    let mut steps = Vec::new();
    for (lineno, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let step = if line.starts_with('{') {
            let v: Value = serde_json::from_str(line).map_err(|e| format!("line {}: {e}", lineno + 1))?;
            parse_script_step_object(&v).map_err(|e| format!("line {}: {e}", lineno + 1))?
        } else {
            let (cmd, rest) = match line.find(char::is_whitespace) {
                Some(i) => (&line[..i], line[i..].trim()),
                None => (line, ""),
            };
            let params = if rest.is_empty() {
                Value::Object(Default::default())
            } else {
                serde_json::from_str(rest).map_err(|e| format!("line {} ({cmd}): {e}", lineno + 1))?
            };
            ScriptStep { command: cmd.to_string(), params }
        };
        steps.push(step);
    }
    Ok(steps)
}

fn resolve_script_ref(r: &str, results: &[Value]) -> std::result::Result<Value, String> {
    let mut parts = r.split('.');
    let idx_str = parts.next().unwrap_or("");
    let idx: usize = idx_str.parse().map_err(|_| format!("invalid step reference `${r}`"))?;
    let mut cur = results.get(idx).ok_or_else(|| format!("`${r}` refers to step {idx}, only {} have run", results.len()))?;
    for key in parts {
        cur = match cur {
            Value::Object(map) => map.get(key).ok_or_else(|| format!("`${r}`: no key `{key}`"))?,
            Value::Array(arr) => {
                let i: usize = key.parse().map_err(|_| format!("`${r}`: `{key}` is not an array index"))?;
                arr.get(i).ok_or_else(|| format!("`${r}`: index {i} out of bounds"))?
            }
            _ => return Err(format!("`${r}`: cannot index into {cur}")),
        };
    }
    Ok(cur.clone())
}

fn resolve_script_value(value: &Value, results: &[Value]) -> std::result::Result<Value, String> {
    match value {
        Value::String(s) if s.starts_with('$') && s.len() > 1 => resolve_script_ref(&s[1..], results),
        Value::Array(items) => items.iter().map(|v| resolve_script_value(v, results)).collect::<std::result::Result<Vec<_>, _>>().map(Value::Array),
        Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, v) in map {
                out.insert(k.clone(), resolve_script_value(v, results)?);
            }
            Ok(Value::Object(out))
        }
        other => Ok(other.clone()),
    }
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
    let text = match p.get("script_file").and_then(Value::as_str).filter(|path| !path.trim().is_empty()) {
        Some(path) => {
            let resolved = crate::workspace::resolve_workspace_path(s, path);
            std::fs::read_to_string(&resolved).map_err(|e| bad(&format!("{}: {e}", resolved.display())))?
        }
        None => p.get("script").and_then(Value::as_str).ok_or_else(|| bad("give `script` (the script text) or `script_file`"))?.to_string(),
    };
    let steps = parse_script_steps(&text).map_err(|e| bad(&e))?;
    let mut b = Board::open(&doc_path(s, p)?)?;
    let session: &Session = s;
    let mut results = Vec::with_capacity(steps.len());
    let mut failed: Option<(usize, String, String)> = None;
    for (i, step) in steps.iter().enumerate() {
        let params = match resolve_script_value(&step.params, &results) {
            Ok(v) => v,
            Err(e) => {
                failed = Some((i, step.command.clone(), e));
                break;
            }
        };
        let res = match step.command.strip_prefix("storyboard.") {
            Some("set") => storyboard_step_set(session, &mut b, &params),
            _ => b.run(&step.command, params).map_err(|e| e.to_string()),
        };
        match res {
            Ok(val) => results.push(val),
            Err(e) => {
                failed = Some((i, step.command.clone(), e));
                break;
            }
        }
    }
    // A failed step leaves the document unwritten.
    if failed.is_none() {
        b.save()?;
    }
    let mut out = json!({
        "ok": failed.is_none(),
        "completed": results.len(),
        "results": results,
        "path": b.path,
    });
    if let Some((step, command, error)) = failed {
        out["failed"] = json!({"step": step, "command": command, "error": error});
    }
    Ok(out)
}

/// One `storyboard.set` step of a script: text and images, addressed by label.
fn storyboard_step_set(s: &Session, b: &mut Board, params: &Value) -> std::result::Result<Value, String> {
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
    let lyrics = p.get("lyrics").and_then(Value::as_str).ok_or_else(|| bad("give `lyrics` (the canonical lyrics sheet .json)"))?;
    let sheet = read_json(s, lyrics)?;
    let wanted: Option<Vec<String>> =
        p.get("sections").and_then(Value::as_str).map(|list| list.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect());
    // The first time the recording says each printed line.
    let mut timings: HashMap<String, (f64, f64)> = HashMap::new();
    if let Some(transcript) = p.get("transcript").and_then(Value::as_str) {
        for para in read_json(s, transcript)?["paragraphs"].as_array().into_iter().flatten() {
            let text = para["text"].as_str().unwrap_or_default().trim().to_string();
            let (Some(start), Some(end)) = (para["start"].as_f64(), para["end"].as_f64()) else {
                continue;
            };
            timings.entry(text).or_insert((start, end));
        }
    }
    let structure: Vec<String> =
        sheet["structure_as_printed"].as_array().map(|list| list.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
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
        let section_lines: Vec<&Value> = lines.iter().filter(|l| l["section"].as_str() == Some(section.as_str())).collect();
        for chunk in section_lines.chunks(per) {
            let text = chunk.iter().filter_map(|l| l["text"].as_str()).map(|t| t.trim_end_matches(['.', ','])).collect::<Vec<_>>().join(" / ");
            let start = chunk.iter().filter_map(|l| l["text"].as_str()).filter_map(|t| timings.get(t.trim())).map(|t| t.0).next();
            let end = chunk.iter().filter_map(|l| l["text"].as_str()).filter_map(|t| timings.get(t.trim())).map(|t| t.1).next_back();
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
    let scene_pages: Vec<usize> = pages.iter().filter(|pg| pg.archetype == "scene").map(|pg| pg.index).collect();
    let mut written = Vec::new();
    for (n, index) in scene_pages.iter().enumerate() {
        let Some(cue) = cues.get(n) else { break };
        let mut fields = serde_json::Map::new();
        fields.insert("cue".into(), json!(format!("\u{201c}{}\u{201d}", cue["text"])));
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
        return Err(bad(&format!("{} exists; pass overwrite=true to replace it", path.display())));
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    craft_atomic::atomic_write(&path, TEMPLATE)?;
    Ok(json!({"ok": true, "path": path, "scenes": 1}))
}

fn scene_number_for(pages: &[PageSlots], index: usize) -> Option<usize> {
    (pages.get(index)?.archetype == "scene").then(|| pages.iter().filter(|pg| pg.archetype == "scene").position(|pg| pg.index == index).map(|n| n + 1))?
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
    let slots = page.slots_json();
    let slot =
        p.get("label").and_then(Value::as_str).and_then(|lbl| slots.as_array().and_then(|arr| arr.iter().find(|s| s["label"].as_str() == Some(lbl)).cloned()));
    Ok(json!({
        "ok": true,
        "path": b.path,
        "page": page.index,
        "scene": scene_number_for(&pages, index),
        "archetype": page.archetype,
        "section": page.section,
        "slot": slot,
        "slots": slots,
    }))
}

/// `storyboard.link`: one line to paste into a chat that names a page or slot and the
/// `magic-cli` call that reads it, addressed to this session: its live `--bridge` when the
/// desktop has a control listener, otherwise the project file.
pub fn cmd_storyboard_link(s: &mut Session, p: &Value) -> Result<Value> {
    let page = p.get("page").and_then(Value::as_u64).ok_or_else(|| bad("give the `page` index"))?;
    let label = p.get("label").and_then(Value::as_str).filter(|l| !l.is_empty());
    if let Some(l) = label
        && !l.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
    {
        return Err(bad("a slot `label` is letters, digits and underscores"));
    }
    let target = match (s.control_address, &s.project_path) {
        (Some(addr), _) => format!("--bridge {addr}"),
        (None, Some(project)) => format!("--project '{}'", project.display()),
        (None, None) => return Err(bad("open a project first")),
    };
    let (name, query) = match label {
        Some(l) => (format!("Storyboard page {}, slot {l}", page + 1), format!("page={page} label={l}")),
        None => (format!("Storyboard page {}", page + 1), format!("page={page}")),
    };
    Ok(json!({"ok": true, "text": format!("{name}: magic-cli {target} exec storyboard.slots {query}")}))
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
    let left = first.slots.iter().find(|s| s.label.ends_with("page_no_left")).and_then(|s| s.text.as_deref())?;
    let right = first.slots.iter().find(|s| s.label.ends_with("page_no_right")).and_then(|s| s.text.as_deref())?;
    (left.trim() == "01" && right.trim() == "02").then(|| (format!("{:02}", 2 * index + 1), format!("{:02}", 2 * index + 2)))
}

/// `storyboard.add`: copy a template page (default: the last one of that kind) and fill it.
pub fn cmd_storyboard_add(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Board::open(&doc_path(s, p)?)?;
    let pages = b.pages()?;
    let archetype = p.get("archetype").and_then(Value::as_str).unwrap_or("scene");
    // Accept `page`, `scene`, or `label` via `page_index`, or `from` (0-based spread index).
    let from = if p.get("page").is_some() || p.get("scene").is_some() || p.get("label").is_some() {
        page_index(&pages, p)?
    } else if let Some(n) = p.get("from").and_then(Value::as_u64) {
        let index = n as usize;
        if index < pages.len() {
            index
        } else {
            return Err(bad(&format!("no page {n}; the storyboard has {} pages", pages.len())));
        }
    } else {
        pages
            .iter()
            .filter(|pg| pg.archetype == archetype)
            .map(|pg| pg.index)
            .next_back()
            .ok_or_else(|| bad(&format!("this storyboard has no `{archetype}` page to copy")))?
    };
    // Duplicate and place in one undo step: `to` is the page index the copy moves to.
    let mut params = json!({"spread": from});
    match p.get("at").and_then(Value::as_u64) {
        Some(at) => params["to"] = json!(at),
        None => params["atEnd"] = json!(true),
    }
    let dup = b.run("layout.pages.duplicateSpread", params)?;
    let index = dup["page"].as_u64().ok_or_else(|| fail("no page index from duplicateSpread"))? as usize;
    let pages = b.pages()?;
    let page = pages.get(index).ok_or_else(|| fail("the new page was not found"))?;
    let labels = page.labels();
    fill_scene(s, &mut b, page, &labels, p)?;
    // A copied scene must not claim to be the scene it came from.
    let asked_for = |label: &str| p.get("fields").and_then(Value::as_object).is_some_and(|f| f.contains_key(label));
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
                && let Some(slot) = pages[index].slots.iter().find(|s| s.label.ends_with(suffix))
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
    let name = p.get("name").and_then(Value::as_str).ok_or_else(|| bad("give `name` (the section prefix, e.g. \"B\")"))?;
    let created = b.run("layout.parents.new", json!({"prefix": name}))?;
    let label = created["label"].as_str().map(str::to_string).unwrap_or_else(|| format!("{name}-Parent"));
    if let Some(base) = p.get("based_on").and_then(Value::as_str) {
        b.run("layout.parents.options", json!({"parent": label, "basedOn": base}))?;
    }
    let pages: Vec<u64> = p.get("pages").and_then(Value::as_array).map(|list| list.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
    if !pages.is_empty() {
        b.run("layout.pages.applyParent", json!({"pages": pages, "parent": label}))?;
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
    let label = p.get("slot").and_then(Value::as_str).ok_or_else(|| bad("give `slot` (the label of the image slot to cover)"))?;
    let image = p.get("image").and_then(Value::as_str).ok_or_else(|| bad("give `image` (a file path)"))?;
    let resolved_image = crate::workspace::resolve_workspace_path(s, image);
    let slot = pages[index].slot(label).ok_or_else(|| bad(&format!("page {index} has no slot `{label}`")))?;
    let bounds = slot.bounds.ok_or_else(|| bad(&format!("slot `{label}` has no bounds")))?;
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
    if p.get("hide_original").and_then(Value::as_bool).unwrap_or(true) {
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
    let cli = crate::workspace::find_executable("designcraft-cli").map_err(fail)?;
    let tmp = ScopedTempDir::new("magic-sb-page")?;
    let out_path = tmp.path().join("page.png");
    let out = std::process::Command::new(&cli)
        .arg("run")
        .arg("--in")
        .arg(&b.work_path)
        .arg("--page")
        .arg(page.to_string())
        .arg("--scale")
        .arg(scale.to_string())
        .arg("--export")
        .arg(&out_path)
        .output()
        .map_err(fail)?;
    if !out.status.success() {
        return Err(fail(String::from_utf8_lossy(&out.stderr).trim()));
    }
    std::fs::read(&out_path).map_err(fail)
}

/// Milliseconds since the epoch at which `path` was last modified.
fn modified_ms(path: &Path) -> Result<u64> {
    let at = std::fs::metadata(path)?.modified()?;
    Ok(at.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)))
}

/// The storyboard's preview folder, beside the document: `storyboard/.preview`.
fn preview_dir(doc: &Path) -> PathBuf {
    doc.with_file_name("storyboard").join(".preview")
}

/// Every page rendered at `scale` into `storyboard/.preview/rail-<scale>/page-NNN.png`, reused
/// while the document is unchanged since the last render.
fn render_rail(doc: &Path, scale: f64) -> Result<Value> {
    let dir = preview_dir(doc).join(format!("rail-{}", (scale * 100.0).round()));
    let source = modified_ms(doc)?;
    let marker = dir.join(".source-modified");
    let fresh = std::fs::read_to_string(&marker).ok().and_then(|t| t.trim().parse::<u64>().ok()) == Some(source);
    if !fresh {
        let b = Board::open(doc)?;
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        let cli = crate::workspace::find_executable("designcraft-cli").map_err(fail)?;
        let out = std::process::Command::new(&cli)
            .arg("run")
            .arg("--in")
            .arg(&b.work_path)
            .arg("--scale")
            .arg(scale.to_string())
            .arg("--all-pages")
            .arg(&dir)
            .output()
            .map_err(fail)?;
        if !out.status.success() {
            return Err(fail(String::from_utf8_lossy(&out.stderr).trim()));
        }
        craft_atomic::atomic_write(&marker, source.to_string().as_bytes())?;
    }
    let mut files: Vec<PathBuf> =
        std::fs::read_dir(&dir)?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "png")).collect();
    files.sort();
    Ok(json!({"ok": true, "rail": true, "files": files, "modified": source, "cached": fresh}))
}

/// One page rendered at `scale` into `storyboard/.preview/page-<index>-<scale>.png`, reused
/// while it is newer than the document.
fn render_preview_page(doc: &Path, index: usize, scale: f64) -> Result<Value> {
    let out = preview_dir(doc).join(format!("page-{index}-{}.png", (scale * 100.0).round()));
    let source = modified_ms(doc)?;
    let fresh = modified_ms(&out).is_ok_and(|rendered| rendered >= source);
    if !fresh {
        let mut b = Board::open(doc)?;
        let png = render_png(&mut b, index, scale)?;
        std::fs::create_dir_all(preview_dir(doc))?;
        craft_atomic::atomic_write(&out, &png)?;
    }
    Ok(json!({"ok": true, "page": index, "out": out, "modified": modified_ms(&out)?, "cached": fresh}))
}

/// `storyboard.thumbnail`: render one page (`out`), all scene thumbnails (`out_dir`), every page
/// for the page rail (`rail`), or one page into the storyboard's own preview folder (`page` alone).
pub fn cmd_storyboard_thumbnail(s: &mut Session, p: &Value) -> Result<Value> {
    let doc = doc_path(s, p)?;
    let has = |key: &str| p.get(key).and_then(Value::as_str).is_some_and(|v| !v.trim().is_empty());
    if p.get("rail").and_then(Value::as_bool) == Some(true) {
        return render_rail(&doc, p.get("scale").and_then(Value::as_f64).unwrap_or(0.16));
    }
    if !has("out") && !has("out_dir") {
        let index = p.get("page").and_then(Value::as_u64).ok_or_else(|| bad("give `page`, `rail`, `out` or `out_dir`"))?;
        let index = usize::try_from(index).map_err(|_| bad("`page` is out of range"))?;
        return render_preview_page(&doc, index, p.get("scale").and_then(Value::as_f64).unwrap_or(0.5));
    }
    let mut b = Board::open(&doc)?;
    let pages = b.pages()?;
    let scale = p.get("scale").and_then(Value::as_f64).unwrap_or(0.5);
    if let Some(raw_dir) = p.get("out_dir").and_then(Value::as_str).filter(|d| !d.trim().is_empty()) {
        let out_dir = crate::workspace::resolve_workspace_path(s, raw_dir);
        std::fs::create_dir_all(&out_dir)?;
        let mut files = Vec::new();
        for (scene_idx, pg) in pages.iter().filter(|pg| pg.archetype == "scene").enumerate() {
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
    let raw_out = p.get("out").and_then(Value::as_str).ok_or_else(|| bad("missing `out` .png path (or `out_dir`)"))?;
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

/// Open the canonical storyboard in the installed DesignCraft editor with live control.
///
/// The endpoint is DesignCraft's suite-standard `--control` port (CraftHub: `DESIGNCRAFT_CONTROL_PORT`
/// or `9877`); `control` overrides it. A listener on the port is verified through the Craft control
/// protocol (`document.inspect`) before it is trusted with `file.open` — a connection on the port
/// alone does not prove identity — and a dirty document open there is never replaced silently.
pub fn cmd_storyboard_open(s: &mut Session, p: &Value) -> Result<Value> {
    use crate::craft_hub::{CraftHub, CraftTarget};
    use std::process::{Command, Stdio};
    use std::time::Duration;
    let path = doc_path(s, p)?;
    if !path.is_file() {
        return Err(bad("Create the DesignCraft storyboard first with storyboard.new"));
    }
    let port = match p.get("control").map(|v| v.as_u64().filter(|n| (1..=u16::MAX as u64).contains(n)).map(|n| n as u16)) {
        None => CraftHub::control_addr(CraftTarget::Design)?.port(),
        Some(Some(port)) => port,
        Some(None) => return Err(bad("control must be a port from 1 to 65535")),
    };
    let mut receipt = json!({"ok":true,"path":path,"control":port});
    receipt["inspect"] = json!(format!("designcraft-cli app --port {port} --method document.inspect"));
    receipt["mcp"] = json!(format!("designcraft-cli mcp --connect {port}"));

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let reused = std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok();
    receipt["reused"] = json!(reused);
    if !reused {
        let app = crate::workspace::find_executable("designcraft")?;
        let mut child =
            Command::new(app).arg("--control").arg(port.to_string()).arg(&path).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
        std::thread::spawn(move || child.wait());
        return Ok(receipt);
    }

    // Verify identity through the Craft control envelope before trusting the listener.
    // `CommandFailed` means the endpoint speaks the protocol but reports no document
    // (nothing open, or nothing to inspect); anything else is an untrusted endpoint.
    let info = match CraftHub::live_execute(CraftTarget::Design, Some(port), "document.inspect", &json!({})) {
        Ok(info) => Some(info),
        Err(EngineError::CommandFailed(_, _)) => None,
        Err(e) => return Err(e),
    };
    let previous = info.as_ref().and_then(|v| v["path"].as_str()).map(PathBuf::from);
    if previous.as_deref().is_some_and(|prev| crate::craft_hub::same_canonical_path(prev, &path)) {
        receipt["already_open"] = json!(true);
        return Ok(receipt);
    }
    if info.as_ref().is_some_and(|v| v["dirty"] == json!(true)) {
        let shown = previous.as_ref().map(|prev| prev.display().to_string()).unwrap_or_else(|| "an unnamed document".to_owned());
        return Err(bad(&format!("DesignCraft has unsaved changes in {shown}; save or close it there first")));
    }
    let opened = CraftHub::live_execute(CraftTarget::Design, Some(port), "file.open", &json!({"path": path}))?;
    receipt["already_open"] = json!(false);
    if info.is_some() {
        receipt["previous"] = info.unwrap_or(Value::Null);
    }
    receipt["opened"] = opened;
    Ok(receipt)
}

/// Registered project images used by the DesignCraft storyboard, for the Lyrics backdrop picker.
pub fn cmd_storyboard_backgrounds(s: &mut Session, p: &Value) -> Result<Value> {
    let path = doc_path(s, p)?;
    let used: Vec<String> =
        if path.is_file() { Board::open(&path)?.pages()?.into_iter().flat_map(|page| page.slots).filter_map(|slot| slot.image).collect() } else { Vec::new() };
    let images: Vec<Value> = s
        .project_assets
        .iter()
        .filter_map(|asset| {
            let file = asset.path.as_ref()?;
            let extension = file.extension()?.to_str()?.to_ascii_lowercase();
            if !matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif") {
                return None;
            }
            Some(json!({"id":asset.id,"name":asset.name,"path":file,
            "in_storyboard":used.iter().any(|source| std::path::Path::new(source) == file)}))
        })
        .collect();
    Ok(json!({"ok":true,"path":path,"images":images}))
}

/// Scene choices for the preserved animation placement controls, from the native document.
pub fn cmd_storyboard_toc(s: &mut Session, p: &Value) -> Result<Value> {
    let path = doc_path(s, p)?;
    let pages = Board::open(&path)?.pages()?;
    let mut scene = 0usize;
    let chapters: Vec<Value> = pages
        .iter()
        .map(|page| {
            let number = if page.archetype == "scene" {
                scene += 1;
                Some(scene)
            } else {
                None
            };
            let title = ["title", "scene_title", "scene_id", "cover_title", "world_heading", "char_heading"]
                .iter()
                .find_map(|label| page.slot(label).and_then(|slot| slot.text.as_deref()))
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{} {}", page.archetype, page.index + 1));
            json!({"page":page.index,"scene":number,"title":title,"archetype":page.archetype})
        })
        .collect();
    Ok(json!({"ok":true,"path":path,"chapters":chapters}))
}
