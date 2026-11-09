//! The storyboard: one `.designcraft` file driven via `CraftHub` (`designcraft` / `designcraft-cli`).

mod commands;

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use serde_json::{Value, json};

pub(crate) use commands::{CompletionCache, completion};
pub use commands::{
    cmd_storyboard_add, cmd_storyboard_check, cmd_storyboard_cues, cmd_storyboard_export, cmd_storyboard_locations, cmd_storyboard_lyrics, cmd_storyboard_new,
    cmd_storyboard_open, cmd_storyboard_pages, cmd_storyboard_section, cmd_storyboard_set, cmd_storyboard_thumbnail,
};

use crate::{EngineError, Result};

/// Each page type and the slot label that marks it.
pub const ARCHETYPES: &[(&str, &str)] =
    &[("cover", "cover_title"), ("world", "world_heading"), ("char", "char_heading"), ("motion", "motion_heading"), ("scene", "scene_id")];

const TITLE_LABELS: &[&str] = &["cover_title", "world_heading", "char_heading", "motion_heading", "scene_title", "scene_id", "title"];

fn item_bounds(item: &Value) -> Option<[f64; 4]> {
    let xf = item.get("xf").and_then(Value::as_array);
    let (a, b, c, d, tx, ty) = match xf {
        Some(v) if v.len() == 6 => (
            v[0].as_f64().unwrap_or(1.0),
            v[1].as_f64().unwrap_or(0.0),
            v[2].as_f64().unwrap_or(0.0),
            v[3].as_f64().unwrap_or(1.0),
            v[4].as_f64().unwrap_or(0.0),
            v[5].as_f64().unwrap_or(0.0),
        ),
        _ => (1.0, 0.0, 0.0, 1.0, 0.0, 0.0),
    };
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for sp in item["path"]["subpaths"].as_array().into_iter().flatten() {
        for anchor in sp["anchors"].as_array().into_iter().flatten() {
            if let Some(p) = anchor["p"].as_array()
                && p.len() >= 2
                && let (Some(px), Some(py)) = (p[0].as_f64(), p[1].as_f64())
            {
                let (x, y) = (a * px + c * py + tx, b * px + d * py + ty);
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    (min_x.is_finite() && max_x > min_x && max_y > min_y).then_some([min_x, min_y, max_x, max_y])
}

pub(super) type Labels = HashMap<String, u64>;

pub(super) struct Slot {
    pub(super) label: String,
    pub(super) id: u64,
    pub(super) kind: &'static str,
    pub(super) text: Option<String>,
    pub(super) image: Option<String>,
    pub(super) bounds: Option<[f64; 4]>,
    pub(super) hidden: bool,
    pub(super) locked: bool,
}

impl Slot {
    pub(super) fn to_json(&self) -> Value {
        json!({
            "label": self.label,
            "id": self.id,
            "kind": self.kind,
            "text": self.text,
            "image": self.image,
            "bounds": self.bounds,
            "hidden": self.hidden,
            "locked": self.locked,
        })
    }
}

pub(super) struct Page {
    pub(super) index: usize,
    pub(super) archetype: String,
    pub(super) section: Option<String>,
    pub(super) bounds: [f64; 4],
    pub(super) slots: Vec<Slot>,
}

impl Page {
    pub(super) fn title(&self) -> String {
        TITLE_LABELS
            .iter()
            .find_map(|marker| self.slot(marker).and_then(|s| s.text.as_deref()).map(str::trim).filter(|t| !t.is_empty()))
            .map_or_else(|| format!("{} {}", self.archetype, self.index + 1), str::to_string)
    }

    pub(super) fn labels(&self) -> Labels {
        self.slots.iter().map(|s| (s.label.clone(), s.id)).collect()
    }

    pub(super) fn slot(&self, label: &str) -> Option<&Slot> {
        self.slots.iter().find(|s| s.label == label)
    }

    pub(super) fn to_json(&self) -> Value {
        let slots: Vec<Value> = self.slots.iter().map(Slot::to_json).collect();
        json!({
            "page": self.index,
            "archetype": self.archetype,
            "section": self.section,
            "title": self.title(),
            "bounds": self.bounds,
            "slots": slots,
        })
    }
}

fn fail(e: impl std::fmt::Display) -> EngineError {
    EngineError::CommandFailed("storyboard".into(), e.to_string())
}

fn u16_at(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize
}

fn u32_at(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize
}

pub(super) fn read_document_json(path: &Path) -> Result<Value> {
    let bytes = std::fs::read(path)?;
    if bytes.len() < 22 || &bytes[0..4] != b"PK\x03\x04" {
        return Err(fail("not a valid .designcraft archive"));
    }
    let eocd = (0..=bytes.len() - 22).rev().find(|&i| &bytes[i..i + 4] == b"PK\x05\x06").ok_or_else(|| fail("corrupt .designcraft zip central directory"))?;
    let mut pos = u32_at(&bytes, eocd + 16);
    for _ in 0..u16_at(&bytes, eocd + 10) {
        if pos + 46 > bytes.len() || &bytes[pos..pos + 4] != b"PK\x01\x02" {
            break;
        }
        let (name_len, extra_len, comment_len) = (u16_at(&bytes, pos + 28), u16_at(&bytes, pos + 30), u16_at(&bytes, pos + 32));
        if std::str::from_utf8(&bytes[pos + 46..pos + 46 + name_len]).unwrap_or("") == "document.json" {
            let local = u32_at(&bytes, pos + 42);
            let start = local + 30 + u16_at(&bytes, local + 26) + u16_at(&bytes, local + 28);
            let data = &bytes[start..start + u32_at(&bytes, pos + 20)];
            let raw = match u16_at(&bytes, pos + 10) {
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

pub(super) fn parent_labels(doc: &Value) -> HashMap<u64, String> {
    doc["parents"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let (prefix, name) = (p["parent"]["prefix"].as_str()?, p["parent"]["name"].as_str()?);
            let label = if prefix.is_empty() { name.to_string() } else { format!("{prefix}-{name}") };
            Some((p["id"].as_u64()?, label))
        })
        .collect()
}

/// An open storyboard: a DesignCraft work copy held by CraftHub.
struct Storyboard {
    doc: crate::craft_hub::CraftDoc,
}

impl Storyboard {
    fn open(path: &Path) -> Result<Self> {
        let doc = crate::craft_hub::CraftDoc::open(crate::craft_hub::CraftTarget::Design, path)?;
        read_document_json(doc.work_path())?;
        Ok(Self { doc })
    }

    fn path(&self) -> &Path {
        self.doc.path()
    }

    fn work_path(&self) -> &Path {
        self.doc.work_path()
    }

    fn run(&mut self, id: &str, params: Value) -> Result<Value> {
        self.run_batch(&[(id, params)])?.into_iter().next().ok_or_else(|| fail(format!("{id}: no result")))
    }

    fn run_batch(&mut self, steps: &[(&str, Value)]) -> Result<Vec<Value>> {
        self.doc.batch(steps)
    }

    fn save(&mut self) -> Result<()> {
        self.doc.save()
    }

    fn frame_stories(&self) -> Result<HashMap<u64, u64>> {
        let doc = read_document_json(self.work_path())?;
        Ok(doc["spreads"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|sp| sp["items"].as_array().into_iter().flatten())
            .filter_map(|item| Some((item["id"].as_u64()?, item["content"]["story"].as_u64()?)))
            .collect())
    }

    fn pages(&mut self) -> Result<Vec<Page>> {
        let doc = read_document_json(self.work_path())?;
        let parents = parent_labels(&doc);
        let default_w = doc["settings"]["pageWidth"].as_f64().unwrap_or(1600.0);
        let default_h = doc["settings"]["pageHeight"].as_f64().unwrap_or(1000.0);
        let mut pages = Vec::new();
        for (index, spread) in doc["spreads"].as_array().into_iter().flatten().enumerate() {
            let mut slots = Vec::new();
            for item in spread["items"].as_array().into_iter().flatten() {
                let Some(label) = item["label"].as_str().filter(|l| !l.is_empty()) else { continue };
                let Some(id) = item["id"].as_u64() else { continue };
                let (kind, text, image) = match item["content"]["type"].as_str() {
                    Some("text") => {
                        ("text", item["content"]["story"].as_u64().and_then(|sid| doc["stories"][sid.to_string()]["text"].as_str()).map(str::to_string), None)
                    }
                    Some("graphic") => {
                        let link = item["content"]["asset"].as_u64().and_then(|aid| doc["assets"][aid.to_string()]["link"].as_str());
                        let resolve = |raw: &str| {
                            craft_docpath::resolve_absolute(raw, Some(self.path())).map_or_else(|_| raw.to_string(), |p| p.to_string_lossy().into_owned())
                        };
                        ("image", None, link.map(resolve))
                    }
                    _ => ("image", None, None),
                };
                let hidden = item["hidden"].as_bool().unwrap_or(false);
                let locked = item["locked"].as_bool().unwrap_or(false);
                let bounds = item_bounds(item);
                slots.push(Slot { label: label.to_string(), id, kind, text, image, bounds, hidden, locked });
            }
            let labels: Labels = slots.iter().map(|s| (s.label.clone(), s.id)).collect();
            let archetype = ARCHETYPES.iter().find(|(_, marker)| labels.contains_key(*marker)).map_or("scene", |(name, _)| *name).to_string();
            let section = spread["pages"][0]["parent"].as_u64().and_then(|pid| parents.get(&pid).cloned());
            let bounds = [0.0, 0.0, default_w, default_h];
            pages.push(Page { index, archetype, section, bounds, slots });
        }
        Ok(pages)
    }
}
