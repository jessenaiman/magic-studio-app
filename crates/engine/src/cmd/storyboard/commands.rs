//! The storyboard commands: calls into [`Storyboard`] via `CraftHub` (`designcraft` / `designcraft-cli`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{Labels, Page, Storyboard, fail};
use crate::{EngineError, Result, Session};

const TEMPLATE: &[u8] = include_bytes!("../../../templates/storyboard.designcraft");
const MAX_REFS: usize = 12;

fn bad(msg: &str) -> EngineError {
    EngineError::InvalidParams(msg.into())
}

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

fn push_image_steps(steps: &mut Vec<(&'static str, Value)>, frame: u64, hint: Option<u64>, path: &str, fit_mode: &str) {
    steps.push(("object.setFlags", json!({"ids": [frame], "locked": false, "hidden": false})));
    steps.push(("place.load", json!({"path": path})));
    steps.push(("place.drop", json!({"x": 0, "y": 0, "frame": frame})));
    steps.push(("object.fit", json!({"mode": fit_mode, "ids": [frame]})));
    if let Some(h) = hint {
        steps.push(("object.setFlags", json!({"ids": [h], "locked": false, "hidden": true})));
    }
}

fn push_scene_steps(s: &Session, stories: &HashMap<u64, u64>, frames: &Labels, p: &Value, steps: &mut Vec<(&'static str, Value)>) -> Result<()> {
    let lock = p.get("lock").and_then(Value::as_bool).unwrap_or(false);
    let mut text_entries: Vec<(&str, &str)> = Vec::new();
    for key in ["fields", "text"] {
        if let Some(obj) = p.get(key).and_then(Value::as_object) {
            for (label, value) in obj {
                let text = value.as_str().ok_or_else(|| bad(&format!("`{key}` values are strings")))?;
                text_entries.push((label.as_str(), text));
            }
        }
    }
    let slot_or_label = p.get("slot").or_else(|| p.get("label")).and_then(Value::as_str);
    if let (Some(label), Some(text)) = (slot_or_label, p.get("text").and_then(Value::as_str)) {
        text_entries.push((label, text));
    }
    for (label, text) in text_entries {
        let frame = *frames.get(label).ok_or_else(|| bad(&format!("this page has no slot labelled `{label}`")))?;
        let story = *stories.get(&frame).ok_or_else(|| fail(format!("frame `{label}` has no story")))?;
        steps.push(("object.setFlags", json!({"ids": [frame], "locked": false})));
        steps.push(("story.setText", json!({"story": story, "text": text})));
        if lock {
            steps.push(("object.setFlags", json!({"ids": [frame], "locked": true})));
        }
    }

    let fit_mode = p.get("fit").and_then(Value::as_str).unwrap_or("fillProportionally");
    let mut image_entries: Vec<(&str, &str)> = Vec::new();
    for key in ["images", "image"] {
        if let Some(obj) = p.get(key).and_then(Value::as_object) {
            for (label, value) in obj {
                let raw = value.as_str().ok_or_else(|| bad(&format!("`{key}` values are file paths")))?;
                image_entries.push((label.as_str(), raw));
            }
        }
    }
    if let (Some(label), Some(raw)) = (slot_or_label, p.get("image").and_then(Value::as_str)) {
        image_entries.push((label, raw));
    }
    for (label, raw) in image_entries {
        let resolved = crate::workspace::resolve_workspace_path(s, raw);
        let frame = *frames.get(label).ok_or_else(|| bad(&format!("this page has no slot labelled `{label}`")))?;
        push_image_steps(steps, frame, frames.get(&format!("{label}_hint")).copied(), &resolved.to_string_lossy(), fit_mode);
    }

    if let Some(refs) = p.get("refs").and_then(Value::as_array) {
        if refs.len() > MAX_REFS {
            return Err(bad("at most 12 `refs`"));
        }
        for n in 1..=MAX_REFS {
            let Some(&frame) = frames.get(&format!("ref_{n}")) else { continue };
            if let Some(raw) = refs.get(n - 1).and_then(Value::as_str) {
                let resolved = crate::workspace::resolve_workspace_path(s, raw);
                push_image_steps(steps, frame, None, &resolved.to_string_lossy(), "fillProportionally");
            } else {
                steps.push(("object.setFlags", json!({"ids": [frame], "locked": false, "hidden": true})));
            }
        }
    }
    Ok(())
}

fn page_index(pages: &[Page], p: &Value) -> Result<usize> {
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

fn is_verse_or_chorus(section: &str) -> bool {
    let s = section.to_ascii_lowercase();
    s.starts_with("verse") || s.starts_with("chorus")
}

fn read_json(s: &Session, raw: &str) -> Result<Value> {
    let path = crate::workspace::resolve_workspace_path(s, raw);
    let text = std::fs::read_to_string(&path).map_err(|e| bad(&format!("{}: {e}", path.display())))?;
    serde_json::from_str(&text).map_err(|e| bad(&format!("{}: {e}", path.display())))
}

fn clock(seconds: f64) -> String {
    let m = (seconds / 60.0).floor() as u64;
    let s = seconds - (m as f64) * 60.0;
    format!("{m}:{:05.2}", s)
}

/// `storyboard.cues`: the song's verse and chorus, with the timing of each line.
pub fn cmd_storyboard_cues(s: &mut Session, p: &Value) -> Result<Value> {
    let lyrics = p.get("lyrics").and_then(Value::as_str).ok_or_else(|| bad("give `lyrics` (the canonical lyrics sheet .json)"))?;
    let sheet = read_json(s, lyrics)?;
    let wanted: Option<Vec<String>> =
        p.get("sections").and_then(Value::as_str).map(|list| list.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect());
    let structure: Vec<String> =
        sheet["structure_as_printed"].as_array().map(|list| list.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let lines = sheet["lines"].as_array().cloned().unwrap_or_default();
    let mut timings: HashMap<String, (f64, f64)> = HashMap::new();
    if let Some(transcript) = p.get("transcript").and_then(Value::as_str) {
        for para in read_json(s, transcript)?["paragraphs"].as_array().into_iter().flatten() {
            let text = para["text"].as_str().unwrap_or_default().trim().to_string();
            let (Some(start), Some(end)) = (para["start"].as_f64(), para["end"].as_f64()) else { continue };
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
                json!({"id": l["id"], "text": text, "start": timing.map(|t| t.0), "end": timing.map(|t| t.1)})
            })
            .collect();
        sections.push(json!({"section": section, "lines": section_lines}));
    }
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

#[derive(Debug)]
pub(crate) struct CompletionCache {
    path: PathBuf,
    stamp: crate::link_watch::FileStamp,
    report: Value,
}

pub fn cmd_storyboard_check(s: &mut Session, p: &Value) -> Result<Value> {
    completion(s, p)
}

pub(crate) fn completion(s: &Session, p: &Value) -> Result<Value> {
    let path = doc_path(s, p)?;
    let stamp = crate::link_watch::FileStamp::of(&path);
    if let Some(cached) = s.storyboard_completion.borrow().as_ref().filter(|c| c.path == path && Some(c.stamp) == stamp) {
        return Ok(cached.report.clone());
    }
    let mut b = Storyboard::open(&path)?;
    let pages = b.pages()?;
    let mut report = Vec::new();
    let mut count = 0usize;
    for page in &pages {
        let mut unfilled = Vec::new();
        for slot in &page.slots {
            if slot.hidden || slot.locked {
                continue;
            }
            if let Some(text) = &slot.text
                && text.lines().map(str::trim).any(|line| line.starts_with('[') && line.ends_with(']'))
            {
                unfilled.push(json!({"label": slot.label, "kind": slot.kind, "text": text.trim()}));
                count += 1;
            }
        }
        let status = if unfilled.is_empty() { "complete" } else { "incomplete" };
        report.push(json!({
            "page": page.index,
            "archetype": page.archetype,
            "section": page.section,
            "status": status,
            "unfilled": unfilled,
            "issues": [],
            "untraced": [],
        }));
    }
    let complete = !pages.is_empty() && count == 0;
    let out = json!({
        "ok": true,
        "path": b.path(),
        "count": count,
        "untraced": 0,
        "page_count": pages.len(),
        "complete": complete,
        "status": if complete { "complete" } else { "incomplete" },
        "pages": report,
        "preflight": {"errors": 0, "issues": []},
    });
    *s.storyboard_completion.borrow_mut() = stamp.map(|stamp| CompletionCache { path, stamp, report: out.clone() });
    Ok(out)
}

/// `storyboard.lyrics`: insert the song's lyrics into the scene pages, locked.
pub fn cmd_storyboard_lyrics(s: &mut Session, p: &Value) -> Result<Value> {
    let cues_res = cmd_storyboard_cues(s, p)?;
    let cues = cues_res["cues"].as_array().cloned().unwrap_or_default();
    let mut b = Storyboard::open(&doc_path(s, p)?)?;
    let pages = b.pages()?;
    let stories = b.frame_stories()?;
    let scene_pages: Vec<usize> = pages.iter().filter(|pg| pg.archetype == "scene").map(|pg| pg.index).collect();
    let mut written = Vec::new();
    let mut steps = Vec::new();
    for (n, index) in scene_pages.iter().enumerate() {
        let Some(cue) = cues.get(n) else { break };
        let timecode = match (cue["start"].as_f64(), cue["end"].as_f64()) {
            (Some(a), Some(b)) => format!("{} – {}", clock(a), clock(b)),
            _ => String::new(),
        };
        let mut fields = serde_json::Map::new();
        fields.insert("cue".into(), json!(format!("\u{201c}{}\u{201d}", cue["text"].as_str().unwrap_or_default())));
        if !timecode.is_empty() {
            fields.insert("time".into(), json!(timecode));
        }
        let request = json!({"fields": Value::Object(fields), "lock": true});
        let labels = pages[*index].labels();
        push_scene_steps(s, &stories, &labels, &request, &mut steps)?;
        written.push(json!({"page": index, "scene": n + 1, "section": cue["section"], "cue": cue["text"], "time": timecode}));
    }
    if !steps.is_empty() {
        b.run_batch(&steps)?;
        b.save()?;
    }
    Ok(json!({"ok": true, "path": b.path(), "cues": cues.len(), "written": written}))
}

/// `storyboard.new`: a new storyboard from the embedded canonical template, plus exported PDF and DeckCraft/PPTX companions.
pub fn cmd_storyboard_new(s: &mut Session, p: &Value) -> Result<Value> {
    let path = doc_path(s, p)?;
    if path.exists() && !p.get("overwrite").and_then(Value::as_bool).unwrap_or(false) {
        return Err(bad(&format!("{} exists; pass overwrite=true to replace it", path.display())));
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    craft_atomic::atomic_write(&path, TEMPLATE)?;
    let pdf = path.with_file_name("storyboard.pdf");
    let pptx = path.with_file_name("storyboard.pptx");
    if let Ok(mut b) = Storyboard::open(&path) {
        let _ = b.run_batch(&[("file.exportPdf", json!({"path": pdf})), ("file.exportPptx", json!({"path": pptx}))]);
    }
    Ok(json!({"ok": true, "path": path, "pdf": pdf, "pptx": pptx, "scenes": 1}))
}

fn scene_number_for(pages: &[Page], index: usize) -> Option<usize> {
    (pages.get(index)?.archetype == "scene").then(|| pages.iter().filter(|pg| pg.archetype == "scene").position(|pg| pg.index == index).map(|n| n + 1))?
}

/// `storyboard.pages`: one line per page; `page` (or `label` / `scene`) gives that page's slots, `full` every page's.
pub fn cmd_storyboard_pages(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Storyboard::open(&doc_path(s, p)?)?;
    let pages = b.pages()?;
    let filter_label = p.get("label").and_then(Value::as_str).filter(|l| !l.is_empty());
    let list: Vec<Value> = if p.get("full").and_then(Value::as_bool) == Some(true) {
        pages.iter().map(Page::to_json).collect()
    } else if p.get("page").is_some() || p.get("scene").is_some() || filter_label.is_some() {
        let mut page_json = pages[page_index(&pages, p)?].to_json();
        if let Some(lbl) = filter_label
            && let Some(slots) = page_json["slots"].as_array_mut()
        {
            slots.retain(|sl| sl["label"].as_str() == Some(lbl));
        }
        vec![page_json]
    } else {
        pages.iter().map(|page| json!({"page": page.index, "archetype": page.archetype, "title": page.title(), "slots": page.slots.len()})).collect()
    };
    Ok(json!({"ok": true, "path": b.path(), "pages": list}))
}

/// `storyboard.locations` / `locations`: inspect or update the storyboard's location / world pages (`world` archetype) and registered background plates.
pub fn cmd_storyboard_locations(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Storyboard::open(&doc_path(s, p)?)?;
    let pages = b.pages()?;
    let stories = b.frame_stories()?;
    let world_indices: Vec<usize> = pages.iter().filter(|pg| pg.archetype == "world").map(|pg| pg.index).collect();
    if p.get("fields").is_some() || p.get("images").is_some() || p.get("text").is_some() || p.get("image").is_some() || p.get("refs").is_some() {
        let target = if p.get("page").is_some() || p.get("label").is_some() {
            page_index(&pages, p)?
        } else {
            *world_indices.first().ok_or_else(|| bad("this storyboard has no `world` / location page"))?
        };
        let labels = pages[target].labels();
        let mut steps = Vec::new();
        push_scene_steps(s, &stories, &labels, p, &mut steps)?;
        if !steps.is_empty() {
            b.run_batch(&steps)?;
            b.save()?;
        }
    }
    let pages = b.pages()?;
    let locations: Vec<Value> = pages.iter().filter(|pg| pg.archetype == "world").map(Page::to_json).collect();
    let backgrounds: Vec<Value> = s
        .project_assets
        .iter()
        .filter(|a| a.path.as_ref().is_some_and(|p| p.to_string_lossy().contains("/backgrounds/")))
        .map(|a| json!({"id": a.id, "name": a.name, "path": a.path}))
        .collect();
    Ok(json!({
        "ok": true,
        "path": b.path(),
        "locations": locations,
        "backgrounds": backgrounds,
    }))
}

/// `storyboard.set`: change one page's text, images and contact strip (or a multi-page `pages` array), then save in one batch.
pub fn cmd_storyboard_set(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Storyboard::open(&doc_path(s, p)?)?;
    let pages = b.pages()?;
    let stories = b.frame_stories()?;
    if let Some(batch) = p.get("pages").and_then(Value::as_array) {
        let mut steps = Vec::new();
        let mut updated = Vec::new();
        for entry in batch {
            let index = page_index(&pages, entry)?;
            let labels = pages[index].labels();
            push_scene_steps(s, &stories, &labels, entry, &mut steps)?;
            updated.push(index);
        }
        if !steps.is_empty() {
            b.run_batch(&steps)?;
            b.save()?;
        }
        return Ok(json!({"ok": true, "pages": updated, "path": b.path()}));
    }
    let index = page_index(&pages, p)?;
    let labels = pages[index].labels();
    let mut steps = Vec::new();
    push_scene_steps(s, &stories, &labels, p, &mut steps)?;
    if !steps.is_empty() {
        b.run_batch(&steps)?;
        b.save()?;
    }
    let page = &pages[index];
    Ok(json!({
        "ok": true,
        "page": page.index,
        "scene": scene_number_for(&pages, index),
        "archetype": page.archetype,
        "path": b.path()
    }))
}

/// `storyboard.add`: copy a template page (default: the last one of that kind) and fill it.
pub fn cmd_storyboard_add(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Storyboard::open(&doc_path(s, p)?)?;
    let pages = b.pages()?;
    let archetype = p.get("archetype").and_then(Value::as_str).unwrap_or("scene");
    let from = if p.get("page").is_some() || p.get("scene").is_some() || p.get("label").is_some() {
        page_index(&pages, p)?
    } else if let Some(n) = p.get("from").and_then(Value::as_u64) {
        let index = n as usize;
        if index < pages.len() { index } else { return Err(bad(&format!("no page {n}; the storyboard has {} pages", pages.len()))) }
    } else {
        pages
            .iter()
            .filter(|pg| pg.archetype == archetype)
            .map(|pg| pg.index)
            .next_back()
            .ok_or_else(|| bad(&format!("this storyboard has no `{archetype}` page to copy")))?
    };
    let mut params = json!({"spread": from});
    match p.get("at").and_then(Value::as_u64) {
        Some(at) => params["to"] = json!(at),
        None => params["atEnd"] = json!(true),
    }
    let dup = b.run("layout.pages.duplicateSpread", params)?;
    let index = dup["page"].as_u64().ok_or_else(|| fail("no page index from duplicateSpread"))? as usize;
    let pages = b.pages()?;
    let stories = b.frame_stories()?;
    let page = pages.get(index).ok_or_else(|| fail("the new page was not found"))?;
    let labels = page.labels();
    let mut steps = Vec::new();
    push_scene_steps(s, &stories, &labels, p, &mut steps)?;
    let scene_number = scene_number_for(&pages, index);
    if archetype == "scene"
        && p.get("fields").and_then(Value::as_object).is_none_or(|f| !f.contains_key("scene_id"))
        && let (Some(number), Some(id)) = (scene_number, labels.get("scene_id"))
        && let Some(&story_id) = stories.get(id)
    {
        steps.push(("object.setFlags", json!({"ids": [id], "locked": false})));
        steps.push(("story.setText", json!({"story": story_id, "text": format!("SCENE {number:02}")})));
    }
    if !steps.is_empty() {
        b.run_batch(&steps)?;
    }
    b.save()?;
    Ok(json!({"ok": true, "page": index, "scene": scene_number, "archetype": archetype, "path": b.path()}))
}

/// `storyboard.section`: add or rename a section (a parent page) and apply it to pages.
pub fn cmd_storyboard_section(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Storyboard::open(&doc_path(s, p)?)?;
    let doc = super::read_document_json(b.work_path())?;
    let existing = super::parent_labels(&doc);
    let raw_name = p.get("name").and_then(Value::as_str).ok_or_else(|| bad("give `name` (the section prefix, e.g. \"B\" or \"B-Parent\")"))?.trim();
    let prefix = raw_name.split_once('-').map_or(raw_name, |(pfx, _)| pfx).trim();
    let title = raw_name.split_once('-').map(|(_, rest)| rest.trim()).filter(|r| !r.is_empty()).unwrap_or("Parent");
    let mut steps = Vec::new();
    let label = if let Some((_, found)) = existing.iter().find(|(_, lbl)| *lbl == raw_name || lbl.starts_with(&format!("{prefix}-"))) {
        if raw_name.contains('-') && found != raw_name {
            steps.push(("layout.parents.options", json!({"parent": found, "prefix": prefix, "name": title})));
            format!("{prefix}-{title}")
        } else {
            found.clone()
        }
    } else {
        steps.push(("layout.parents.new", json!({"prefix": prefix, "name": title})));
        format!("{prefix}-{title}")
    };
    if let Some(base) = p.get("based_on").and_then(Value::as_str).filter(|b| !b.is_empty()) {
        steps.push(("layout.parents.options", json!({"parent": label, "basedOn": base})));
    }
    let mut pages: Vec<u64> = p.get("pages").and_then(Value::as_array).map(|list| list.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
    if let Some(single) = p.get("page").and_then(Value::as_u64)
        && !pages.contains(&single)
    {
        pages.push(single);
    }
    if !pages.is_empty() {
        steps.push(("layout.pages.applyParent", json!({"pages": pages, "parent": label})));
    }
    if !steps.is_empty() {
        b.run_batch(&steps)?;
        b.save()?;
    }
    Ok(json!({"ok": true, "section": label, "pages": pages, "path": b.path()}))
}

fn render_png(b: &mut Storyboard, page: usize, scale: f64) -> Result<Vec<u8>> {
    let cli = crate::workspace::find_executable("designcraft-cli").map_err(fail)?;
    let tmp = crate::craft_hub::ScopedTempDir::new("magic-sb-page")?;
    let out_path = tmp.path().join("page.png");
    let out = std::process::Command::new(&cli)
        .arg("run")
        .arg("--in")
        .arg(b.work_path())
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

fn modified_ms(path: &Path) -> Result<u64> {
    let at = std::fs::metadata(path)?.modified()?;
    Ok(at.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)))
}

fn preview_dir(doc: &Path) -> PathBuf {
    doc.with_file_name("storyboard").join(".preview")
}

fn render_rail(doc: &Path, scale: f64) -> Result<Value> {
    let dir = preview_dir(doc).join(format!("rail-{}", (scale * 100.0).round()));
    let source = modified_ms(doc)?;
    let marker = dir.join(".source-modified");
    let fresh = std::fs::read_to_string(&marker).ok().and_then(|t| t.trim().parse::<u64>().ok()) == Some(source);
    if !fresh {
        let b = Storyboard::open(doc)?;
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        let cli = crate::workspace::find_executable("designcraft-cli").map_err(fail)?;
        let out = std::process::Command::new(&cli)
            .arg("run")
            .arg("--in")
            .arg(b.work_path())
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

fn render_preview_page(doc: &Path, index: usize, scale: f64) -> Result<Value> {
    let out = preview_dir(doc).join(format!("page-{index}-{}.png", (scale * 100.0).round()));
    let source = modified_ms(doc)?;
    let fresh = modified_ms(&out).is_ok_and(|rendered| rendered >= source);
    if !fresh {
        let mut b = Storyboard::open(doc)?;
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
    let mut b = Storyboard::open(&doc)?;
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
        return Ok(json!({"ok": true, "out_dir": out_dir, "count": files.len(), "files": files}));
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

/// `storyboard.export`: export the storyboard to PDF (`pdf`), PowerPoint/DeckCraft (`pptx`), and/or PNG spreads (`png_dir`).
pub fn cmd_storyboard_export(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = Storyboard::open(&doc_path(s, p)?)?;
    let mut done = serde_json::Map::new();
    let mut steps = Vec::new();
    if let Some(raw_pdf) = p.get("pdf").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
        let pdf = crate::workspace::resolve_workspace_path(s, raw_pdf);
        if let Some(dir) = pdf.parent() {
            std::fs::create_dir_all(dir)?;
        }
        steps.push(("file.exportPdf", json!({"path": pdf})));
        done.insert("pdf".into(), json!(pdf));
    }
    if let Some(raw_pptx) = p.get("pptx").or_else(|| p.get("deck")).and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
        let pptx = crate::workspace::resolve_workspace_path(s, raw_pptx);
        if let Some(dir) = pptx.parent() {
            std::fs::create_dir_all(dir)?;
        }
        steps.push(("file.exportPptx", json!({"path": pptx})));
        done.insert("pptx".into(), json!(pptx));
    }
    if !steps.is_empty() {
        b.run_batch(&steps)?;
    }
    if let Some(raw_dir) = p.get("png_dir").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
        let dir = crate::workspace::resolve_workspace_path(s, raw_dir);
        std::fs::create_dir_all(&dir)?;
        let scale = p.get("scale").and_then(Value::as_f64).unwrap_or(1.0);
        let cli = crate::workspace::find_executable("designcraft-cli").map_err(fail)?;
        let out = std::process::Command::new(&cli)
            .arg("run")
            .arg("--in")
            .arg(b.work_path())
            .arg("--scale")
            .arg(scale.to_string())
            .arg("--all-pages")
            .arg(&dir)
            .output()
            .map_err(fail)?;
        if !out.status.success() {
            return Err(fail(String::from_utf8_lossy(&out.stderr).trim()));
        }
        done.insert("png_dir".into(), json!(dir));
    }
    if done.is_empty() {
        return Err(bad("give `pdf`, `pptx` and/or `png_dir`"));
    }
    Ok(json!({"ok": true, "exported": done}))
}

/// Open the canonical storyboard in the installed DesignCraft editor with live control.
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
