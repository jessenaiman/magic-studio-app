//! Storyboard chapters, scene packets (`storyboard/scene-N.md`), PDF builder (`pdfcraft-cli`),
//! and pipeline storyboard mirror.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::workshop::{
    ASSETS_DIR, STORYBOARD_DIR, STORYBOARD_PDF, files_in, panel_files, project_dir, song_seconds,
    transcript_files,
};
use crate::workspace::{resolve_workspace_path, run_tool_in};
use crate::{EngineError, Result, Session};

/// One editable native template and the existing DeckCraft generator own storyboards.
pub(crate) const DEFAULT_STORYBOARD_TEMPLATE: &str = "magic-studio";
pub(crate) fn resolve_storyboard_template(s: &Session, raw: &str) -> Result<PathBuf> {
    let template = crate::workspace::workspace_root(s)
        .join("pipeline/templates/studio-template.deckcraft")
        .canonicalize()?;
    if raw != DEFAULT_STORYBOARD_TEMPLATE
        && resolve_workspace_path(s, raw).canonicalize().ok().as_ref() != Some(&template)
    {
        return Err(EngineError::InvalidParams(
            "Magic Studio uses only pipeline/templates/studio-template.deckcraft; edit that native template instead of selecting another storyboard type".into()));
    }
    Ok(template)
}

/// A scene's packet sections, in the order they print.
pub(crate) const SCENE_SECTIONS: &[(&str, &str)] = &[
    ("skills", "Skills"),
    ("action", "Action"),
    ("reference_prompt", "Reference prompt"),
    ("animation_prompt", "Animation prompt"),
    ("layer_stack", "Layer stack"),
    ("animated_text", "Animated text"),
    ("teaching_cues", "Teaching cues"),
    ("visual_review", "Visual review expectations"),
    ("build_steps", "Build steps"),
    ("acceptance_checks", "Acceptance checks"),
    ("plan_vs_actual", "Plan vs actual"),
];
/// Valid scene statuses.
pub(crate) const SCENE_STATUSES: &[&str] = &["draft", "needs review", "approved"];

/// One sung phrase per line with its start time.
pub(crate) fn lyric_lines(transcript: &Value) -> Vec<(f64, String)> {
    let words: Vec<(f64, &str)> = transcript
        .get("words")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|w| Some((w.get("start")?.as_f64()?, w.get("text")?.as_str()?.trim())))
        .collect();
    let mut lines: Vec<(f64, String)> = Vec::new();
    let mut sentence_end = false;
    for (start, text) in words {
        let capital = text.chars().next().is_some_and(char::is_uppercase)
            && !matches!(
                text.trim_end_matches([',', '.', '!', '?']),
                "I" | "I'm" | "I'll" | "I've" | "I'd"
            );
        match lines.last_mut() {
            Some((_, line)) if !(capital || sentence_end) => {
                line.push(' ');
                line.push_str(text);
            }
            _ => lines.push((start, text.to_string())),
        }
        sentence_end = text.ends_with(['.', '!', '?']);
    }
    lines
}

/// Scenes from the transcript: each sung stanza is a scene, gaps are instrumental scenes.
pub(crate) fn scenes(transcript: &Value, song_seconds: Option<f64>) -> Vec<Value> {
    let lines = lyric_lines(transcript);
    let stanzas: Vec<(f64, f64)> = transcript
        .get("paragraphs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| Some((p.get("start")?.as_f64()?, p.get("end")?.as_f64()?)))
        .collect();
    let mut out = Vec::new();
    let mut cursor = 0.0;
    let mut verse = 0;
    let instrumental =
        |from: f64, to: f64| json!({"kind": "instrumental", "start": from, "end": to, "lines": []});
    for (start, end) in &stanzas {
        if start - cursor >= 2.0 {
            out.push(instrumental(cursor, *start));
        }
        verse += 1;
        let sung: Vec<Value> = lines
            .iter()
            .filter(|(t, _)| *t >= start - 0.01 && *t < end - 0.01)
            .map(|(t, text)| json!({"start": t, "text": text}))
            .collect();
        out.push(
            json!({"kind": "verse", "verse": verse, "start": start, "end": end, "lines": sung}),
        );
        cursor = *end;
    }
    if let Some(total) = song_seconds
        && total - cursor >= 2.0
    {
        out.push(instrumental(cursor, total));
    }
    out
}

/// Seconds of song with no caption that make an instrumental scene.
const CAPTION_GAP: f64 = 1.0;

/// The video target's own caption file (`<target>/transcript.srt`), when it has one.
pub(crate) fn caption_file(dir: &Path) -> Option<PathBuf> {
    Some(dir.join("hyperframes/transcript.srt")).filter(|p| p.is_file())
}

fn parse_srt_seconds(s: &str) -> Option<f64> {
    let (hms, ms) = s.trim().split_once([',', '.'])?;
    let mut parts = hms.split(':').map(|v| v.parse::<f64>().ok());
    let (h, m, sec) = (parts.next()??, parts.next()??, parts.next()??);
    Some(h * 3600.0 + m * 60.0 + sec + ms.parse::<f64>().ok()? / 1000.0)
}

fn parse_srt_cues(text: &str) -> Vec<(usize, f64, f64, String)> {
    text.replace("\r\n", "\n")
        .split("\n\n")
        .filter_map(|block| {
            let lines: Vec<&str> = block
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect();
            let at = lines.iter().position(|l| l.contains("-->"))?;
            let (a, b) = lines[at].split_once("-->")?;
            let number = lines.first().and_then(|n| n.parse().ok()).unwrap_or(0);
            Some((
                number,
                parse_srt_seconds(a)?,
                parse_srt_seconds(b)?,
                lines[at + 1..].join(" "),
            ))
        })
        .collect()
}

fn resolve_caption_total_duration(
    dir: &Path,
    srt: &Path,
    cues: &[(usize, f64, f64, String)],
) -> Option<f64> {
    let audiomap: Option<Value> = std::fs::read_to_string(srt.with_file_name("audiomap.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    audiomap
        .and_then(|m| m["audio"]["duration_sec"].as_f64())
        .or_else(|| {
            std::fs::read(dir.join("transcript-provenance.json"))
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .and_then(|v| v["duration_sec"].as_f64())
        })
        .or_else(|| song_seconds(dir))
        .or_else(|| cues.last().map(|c| c.2))
}

/// Scenes from the video target's own captions (`<target>/transcript.srt`): one scene per
/// caption (its number, time and words), plus an instrumental scene for each stretch of
/// `CAPTION_GAP` seconds or more without one. Scenes tile the song. None without a caption file.
pub(crate) fn caption_scenes(dir: &Path) -> Option<Vec<Value>> {
    let srt = caption_file(dir)?;
    let text = std::fs::read_to_string(&srt).ok()?;
    let cues = parse_srt_cues(&text);
    let total = resolve_caption_total_duration(dir, &srt, &cues)?;
    let instrumental =
        |from: f64, to: f64| json!({"kind": "instrumental", "start": from, "end": to, "lines": []});
    let mut out = Vec::new();
    let mut cursor = 0.0;
    for (i, (number, start, end, words)) in cues.iter().enumerate() {
        if start - cursor >= CAPTION_GAP {
            out.push(instrumental(cursor, *start));
            cursor = *start;
        }
        let next = cues.get(i + 1).map_or(total, |c| c.1);
        let until = if next - end >= CAPTION_GAP {
            *end
        } else {
            next
        };
        out.push(json!({"kind": "verse", "verse": number, "caption": number, "start": cursor, "end": until,
            "lines": [{"start": start, "text": words}]}));
        cursor = until;
    }
    if total - cursor >= CAPTION_GAP {
        out.push(instrumental(cursor, total));
    } else if let Some(last) = out.last_mut() {
        last["end"] = json!(total);
    }
    Some(out)
}

/// When each storyboard panel is on screen: panel N covers line N of every verse.
pub(crate) fn panel_spans(scene_list: &[Value], panels: usize) -> Vec<Vec<(f64, f64)>> {
    let mut spans = vec![Vec::new(); panels];
    for scene in scene_list.iter().filter(|sc| sc["kind"] == json!("verse")) {
        let lines = scene["lines"].as_array().cloned().unwrap_or_default();
        let end = scene["end"].as_f64().unwrap_or(0.0);
        for (j, line) in lines.iter().enumerate().take(panels) {
            let start = line["start"].as_f64().unwrap_or(0.0);
            let next = lines
                .get(j + 1)
                .and_then(|l| l["start"].as_f64())
                .unwrap_or(end);
            spans[j].push((start, next));
        }
    }
    spans
}

/// A scene's source document (`storyboard/scene-N.md`).
#[derive(Default, Clone)]
pub(crate) struct SceneDoc {
    pub title: Option<String>,
    pub status: String,
    pub time: Option<(f64, f64)>,
    pub reference: Option<String>,
    pub teaching_reference: Option<String>,
    pub notes: Vec<String>,
    pub lyrics: std::collections::BTreeMap<usize, String>,
    pub cues: std::collections::BTreeMap<usize, String>,
    pub sections: std::collections::BTreeMap<String, String>,
}

pub(crate) fn scene_doc_path(dir: &Path, n: usize) -> PathBuf {
    dir.join(STORYBOARD_DIR).join(format!("scene-{n}.md"))
}

fn parse_scene_doc_frontmatter(doc: &mut SceneDoc, t: &str) {
    if let Some(st) = t.strip_prefix("status:") {
        doc.status = st.trim().to_string();
    } else if let Some(r) = t.strip_prefix("reference:") {
        doc.reference = Some(r.trim().to_string()).filter(|r| !r.is_empty());
    } else if let Some(tr) = t.strip_prefix("teaching_reference:") {
        doc.teaching_reference = Some(tr.trim().to_string()).filter(|r| !r.is_empty());
    } else if let Some(tm) = t.strip_prefix("time:")
        && let Some((a, b)) = tm.trim().split_once(['-', '–'])
        && let (Ok(a), Ok(b)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>())
    {
        doc.time = Some((a, b));
    }
}

fn parse_scene_doc_section_line(doc: &mut SceneDoc, section: &str, line: &str, t: &str) {
    if section == "notes" {
        if let Some(note) = t.strip_prefix("- ") {
            doc.notes.push(note.to_string());
        }
    } else if section == "lyrics" {
        if let Some((n, words)) = t.split_once(':')
            && let Ok(n) = n.trim().parse::<usize>()
        {
            doc.lyrics.insert(n, words.trim().to_string());
        }
    } else if section == "cues" {
        if let Some((n, cue)) = t.split_once(':')
            && let Ok(n) = n.trim().parse::<usize>()
        {
            doc.cues.insert(n, cue.trim().to_string());
        }
    } else if !t.starts_with("<!--") {
        let entry = doc.sections.entry(section.to_owned()).or_default();
        if !entry.is_empty() {
            entry.push('\n');
        }
        entry.push_str(line);
    }
}

pub(crate) fn read_scene_doc(dir: &Path, n: usize) -> SceneDoc {
    let mut doc = SceneDoc {
        status: "draft".into(),
        ..Default::default()
    };
    let Ok(text) = std::fs::read_to_string(scene_doc_path(dir, n)) else {
        return doc;
    };
    let mut section = String::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(h) = t.strip_prefix("# ") {
            if doc.title.is_none() {
                doc.title = Some(h.trim().to_string());
            }
        } else if let Some(h) = t.strip_prefix("## ") {
            section = SCENE_SECTIONS
                .iter()
                .find(|(_, label)| label.eq_ignore_ascii_case(h.trim()))
                .map(|(k, _)| (*k).to_string())
                .unwrap_or_else(|| h.trim().to_lowercase());
        } else if section.is_empty() {
            parse_scene_doc_frontmatter(&mut doc, t);
        } else {
            parse_scene_doc_section_line(&mut doc, &section, line, t);
        }
    }
    for v in doc.sections.values_mut() {
        *v = v.trim().to_string();
    }
    doc.sections.retain(|_, v| !v.is_empty());
    doc
}

fn write_scene_doc(dir: &Path, n: usize, label: &str, doc: &SceneDoc) -> Result<()> {
    let mut out = format!(
        "# {}\nstatus: {}\n",
        doc.title.as_deref().unwrap_or(label),
        doc.status
    );
    if let Some((a, b)) = doc.time {
        out.push_str(&format!("time: {a}-{b}\n"));
    }
    if let Some(r) = &doc.reference {
        out.push_str(&format!("reference: {r}\n"));
    }
    if let Some(tr) = &doc.teaching_reference {
        out.push_str(&format!("teaching_reference: {tr}\n"));
    }
    out.push_str("\n## Notes\n");
    for note in &doc.notes {
        out.push_str(&format!("- {note}\n"));
    }
    out.push_str("\n## Lyrics\n<!-- corrections only: line number: words (the transcript keeps what was heard) -->\n");
    for (n, words) in &doc.lyrics {
        out.push_str(&format!("{n}: {words}\n"));
    }
    if !doc.cues.is_empty() {
        out.push_str("\n## Cues\n<!-- teaching / parent action cues per line: line number: instruction -->\n");
        for (n, cue) in &doc.cues {
            out.push_str(&format!("{n}: {cue}\n"));
        }
    }
    for (key, heading) in SCENE_SECTIONS {
        if let Some(text) = doc.sections.get(*key) {
            out.push_str(&format!("\n## {heading}\n{text}\n"));
        }
    }
    std::fs::create_dir_all(dir.join(STORYBOARD_DIR))?;
    std::fs::write(scene_doc_path(dir, n), out)?;
    Ok(())
}

fn explicit_scenes_from_docs(docs: &[SceneDoc], doc: &Value) -> Vec<Value> {
    let lines = lyric_lines(doc);
    docs.iter()
        .enumerate()
        .map(|(i, d)| {
            let (a, b) = d.time.unwrap_or_default();
            let sung: Vec<Value> = lines
                .iter()
                .filter(|(t, _)| *t >= a - 0.01 && *t < b - 0.01)
                .map(|(t, text)| json!({"start": t, "text": text}))
                .collect();
            let kind = if sung.is_empty() {
                "instrumental"
            } else {
                "scene"
            };
            json!({"kind": kind, "verse": i + 1, "start": a, "end": b, "lines": sung})
        })
        .collect()
}

fn apply_scene_doc_lines(lines: &mut Vec<Value>, start_sec: f64, sd: &SceneDoc) {
    for (j, line) in lines.iter_mut().enumerate() {
        if let Some(words) = sd.lyrics.get(&(j + 1)) {
            line["heard"] = line["text"].clone();
            line["text"] = json!(words);
        }
        if let Some(cue) = sd.cues.get(&(j + 1)) {
            line["cue"] = json!(cue);
        }
    }
    let max_idx = sd
        .lyrics
        .keys()
        .chain(sd.cues.keys())
        .copied()
        .max()
        .unwrap_or(0);
    for k in (lines.len() + 1)..=max_idx {
        let mut entry = json!({
            "start": start_sec,
            "text": sd.lyrics.get(&k).cloned().unwrap_or_default()
        });
        if let Some(cue) = sd.cues.get(&k) {
            entry["cue"] = json!(cue);
        }
        lines.push(entry);
    }
}

fn apply_scene_doc_overrides(dir: &Path, scene: &mut Value, sd: SceneDoc) {
    let start_sec = scene["start"].as_f64().unwrap_or(0.0);
    if let Some(lines) = scene["lines"].as_array_mut() {
        apply_scene_doc_lines(lines, start_sec, &sd);
        if !lines.is_empty() {
            scene["kind"] = json!("scene");
        }
    }
    scene["status"] = json!(sd.status);
    scene["notes"] = json!(sd.notes);
    scene["title"] = json!(sd.title);
    scene["reference"] = json!(sd.reference.as_ref().map(|r| dir.join(r)));
    scene["teaching_reference"] = json!(sd.teaching_reference.as_ref().map(|r| dir.join(r)));
    scene["sections"] = json!(sd.sections);
}

/// The song's scenes: explicit intervals from scene files when every one has a `time:`,
/// otherwise transcript stanzas and gaps.
pub(crate) fn storyboard_scenes(dir: &Path) -> Result<Vec<Value>> {
    let docs: Vec<SceneDoc> = (1..)
        .take_while(|n| scene_doc_path(dir, *n).is_file())
        .map(|n| read_scene_doc(dir, n))
        .collect();
    let explicit = !docs.is_empty() && docs.iter().all(|d| d.time.is_some());
    let captions = caption_scenes(dir);
    let doc: Value = match &captions {
        Some(_) if !explicit => Value::Null,
        _ => {
            let transcript = transcript_files(dir).into_iter().next().ok_or_else(|| {
                EngineError::InvalidParams(
                    "transcribe or import the transcript first: scenes come from it".into(),
                )
            })?;
            crate::workshop::transcript_document(&transcript)?
        }
    };
    let mut list = if let (Some(list), false) = (captions, explicit) {
        list
    } else if explicit {
        explicit_scenes_from_docs(&docs, &doc)
    } else {
        scenes(&doc, song_seconds(dir))
    };
    for (i, scene) in list.iter_mut().enumerate() {
        let sd = docs
            .get(i)
            .cloned()
            .unwrap_or_else(|| read_scene_doc(dir, i + 1));
        apply_scene_doc_overrides(dir, scene, sd);
    }
    Ok(list)
}

/// written to `out`. None when they don't share a canvas.
pub fn assembled_parts(parts: &[PathBuf], out: &Path) -> Option<PathBuf> {
    let mut canvas: Option<image::RgbaImage> = None;
    for p in parts {
        let layer = image::open(p).ok()?.to_rgba8();
        match &mut canvas {
            None => canvas = Some(layer),
            Some(c) if c.dimensions() == layer.dimensions() => {
                image::imageops::overlay(c, &layer, 0, 0)
            }
            Some(_) => return None,
        }
    }
    let c = canvas?;
    let (w, h) = c.dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for (x, y, px) in c.enumerate_pixels() {
        if px[3] > 8 {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
    }
    if x1 < x0 {
        return None;
    }
    image::imageops::crop_imm(&c, x0, y0, x1 - x0 + 1, y1 - y0 + 1)
        .to_image()
        .save(out)
        .ok()?;
    Some(out.to_path_buf())
}

/// The PDF page of scene 1: after the lyrics page and the Cast & sets board.
fn first_scene_page(dir: &Path) -> usize {
    if let Some(page) = std::fs::read(dir.join("storyboard/movie-board.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v["first_scene_page"].as_u64())
    {
        return page as usize;
    }
    2
}

pub(crate) fn scene_label(i: usize, scene: &Value) -> String {
    let range = format!(
        "{}–{}",
        clock(scene["start"].as_f64().unwrap_or(0.0)),
        clock(scene["end"].as_f64().unwrap_or(0.0))
    );
    if let Some(title) = scene["title"].as_str().filter(|t| !t.starts_with("Scene ")) {
        return format!("Scene {} · {title} · {range}", i + 1);
    }
    if scene.get("caption").is_some() {
        let words = scene["lines"][0]["text"].as_str().unwrap_or_default();
        return format!("Scene {} · {words} · {range}", i + 1);
    }
    match scene["kind"].as_str() {
        Some("verse") => format!("Scene {} · Verse {} · {range}", i + 1, scene["verse"]),
        _ => format!("Scene {} · Instrumental · {range}", i + 1),
    }
}

fn scene_number(params: &Value, count: usize) -> Result<usize> {
    let n = params
        .get("scene")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .ok_or_else(|| {
            EngineError::InvalidParams("missing 'scene' (1-based, see storyboard.toc)".into())
        })?;
    if n == 0 || n > count {
        return Err(EngineError::InvalidParams(format!(
            "scene must be 1..={count}"
        )));
    }
    Ok(n)
}

pub(crate) fn clock(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    format!("{}:{:02}", total / 60, total % 60)
}

/// Newest change among the files the storyboard is built from.
fn storyboard_inputs_changed(dir: &Path) -> Option<std::time::SystemTime> {
    let mut files: Vec<PathBuf> = files_in(&dir.join(STORYBOARD_DIR), None)
        .into_iter()
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("panel-") || n.starts_with("scene-"))
        })
        .collect();
    files.extend(files_in(&dir.join(ASSETS_DIR), None));
    files.extend(files_in(&dir.join("assets/references"), None));
    files.extend(files_in(&dir.join("liner-notes"), None));
    files.extend(transcript_files(dir));
    files.push(dir.join("intent.md"));
    if let Some(parent) = dir.parent() {
        files.push(parent.join("STUDIO.md"));
    }
    files
        .iter()
        .filter_map(|f| std::fs::metadata(f).and_then(|m| m.modified()).ok())
        .max()
}

fn resolve_build_transcript(s: &Session, dir: &Path, params: &Value) -> Result<PathBuf> {
    if let Some(path) = params["transcript"].as_str() {
        if params["draft"] != true {
            return Err(EngineError::InvalidParams(
                "An explicit source transcript requires draft=true; active data is preserved"
                    .into(),
            ));
        }
        return Ok(resolve_workspace_path(s, path).canonicalize()?);
    }
    transcript_files(dir)
        .into_iter()
        .next()
        .or_else(|| {
            crate::storyboard_document::load(dir)
                .ok()
                .and_then(|v| v["source_transcript"].as_str().map(PathBuf::from))
                .filter(|p| p.is_file())
        })
        .ok_or_else(|| EngineError::InvalidParams("Import the existing transcript first".into()))
}

fn resolve_build_output_dir(s: &Session, dir: &Path, params: &Value) -> Result<PathBuf> {
    if let Some(resume) = params["resume"].as_str() {
        let manifest = resolve_workspace_path(s, resume).canonicalize()?;
        if !manifest.starts_with(dir.join("storyboard").canonicalize()?)
            || manifest.file_name().and_then(|n| n.to_str()) != Some("manifest.json")
        {
            return Err(EngineError::InvalidParams(
                "Resume must be this project's storyboard draft manifest".into(),
            ));
        }
        return manifest
            .parent()
            .ok_or_else(|| EngineError::InvalidParams("Draft has no folder".into()))
            .map(Path::to_path_buf);
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    Ok(dir.join("storyboard").join(format!("movie-board-{stamp}")))
}

fn resolve_build_scenes(
    s: &mut Session,
    dir: &Path,
    params: &Value,
    active_board: &Value,
) -> Result<Vec<Value>> {
    if let Some(scenes) = params["_scenes"].as_array() {
        return Ok(scenes.clone());
    }
    if active_board["page_plan"].is_array() && active_board["empty_template"] != true {
        return Ok(crate::storyboard_crud::packets(s)?.2);
    }
    Ok(storyboard_scenes(dir)?
        .into_iter()
        .enumerate()
        .map(|(i, mut scene)| {
            scene["target"] = json!(format!("scene-{}", i + 1));
            scene["scene"] = json!(i + 1);
            scene
        })
        .collect())
}

fn write_build_scene_map(
    s: &mut Session,
    dir: &Path,
    out: &Path,
    params: &Value,
    active_board: &Value,
    transcript: &Path,
    scenes: &[Value],
) -> Result<PathBuf> {
    let scene_map = out.join("scene-map.json");
    let page_content = if params["_page_content"].is_array() {
        params["_page_content"].clone()
    } else {
        crate::storyboard_design::inspect(s, &json!({}))?["text_fields"].clone()
    };
    let previous_board = if params["_previous_board"].is_object() {
        params["_previous_board"].clone()
    } else {
        active_board.clone()
    };
    let provenance = std::fs::read(dir.join("transcript-provenance.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .unwrap_or(Value::Null);
    let recording_duration = song_seconds(dir);
    let timing_extent = recording_duration
        .or_else(|| provenance["duration_sec"].as_f64())
        .or_else(|| scenes.last().and_then(|v| v["end"].as_f64()));
    let captions = if params["transcript"].is_string() {
        Value::Null
    } else if active_board["source_transcript"] == json!(transcript) {
        active_board["source_captions"].clone()
    } else {
        json!(caption_file(dir))
    };
    std::fs::write(
        &scene_map,
        serde_json::to_vec_pretty(&json!({
            "scenes":scenes,"page_content":page_content,"previous_board":previous_board,
            "backgrounds":params["_backgrounds"],"source":"canonical CLI scene spans",
            "transcript":transcript,"captions":captions,"duration_sec":timing_extent,
            "recording_duration_verified":recording_duration.is_some(),
            "audio":s.song_audio,"project":s.project_path,"assets":s.project_assets
        }))
        .map_err(|e| EngineError::InvalidParams(e.to_string()))?,
    )?;
    Ok(scene_map)
}

fn build_movie_board_args(
    s: &Session,
    params: &Value,
    template: &Path,
    transcript: &Path,
    scene_map: &Path,
    out: &Path,
    title: String,
) -> Result<Vec<String>> {
    let script = crate::workspace::workspace_root(s).join("pipeline/movie_board.py");
    let mut args = vec![
        script.to_string_lossy().into_owned(),
        "--template".into(),
        template.to_string_lossy().into_owned(),
        "--transcript".into(),
        transcript.to_string_lossy().into_owned(),
        "--scene-map".into(),
        scene_map.to_string_lossy().into_owned(),
        "--out".into(),
        out.to_string_lossy().into_owned(),
        "--title".into(),
        title,
    ];
    if params["allow_timing_overrun"] == true {
        if params["draft"] != true {
            return Err(EngineError::InvalidParams(
                "Timing-overrun source may only produce an explicitly warned draft".into(),
            ));
        }
        args.push("--allow-timing-overrun".into());
    }
    if let Some(source) = params["source"].as_str() {
        args.extend([
            "--source".into(),
            resolve_workspace_path(s, source)
                .to_string_lossy()
                .into_owned(),
        ]);
    }
    if let Some(panels) = params["panels"].as_str() {
        args.extend(["--panels".into(), panels.into()]);
    }
    Ok(args)
}

fn finalize_storyboard_build_result(
    s: &Session,
    dir: &Path,
    out: &Path,
    params: &Value,
    mut result: Value,
) -> Result<Value> {
    result["project"] = json!(s.project_path);
    result["manifest"] = json!(out.join("manifest.json"));
    result["draft"] =
        json!(params["draft"] == true || result["mapping"]["needs_correction"] == true);
    result["status"] = json!(if result["ok"] == false {
        "draft generation failed; active storyboard unchanged"
    } else if result["timing_valid"] == false {
        "editable draft with raw timing warning; not validated"
    } else if result["mapping"]["needs_correction"] == true {
        "draft needs source mapping correction"
    } else if result["draft"] == true {
        "draft ready for review; active storyboard unchanged"
    } else {
        "saved"
    });
    let executable = crate::command_center::cli_program();
    let proj = crate::command_center::shell_quote(
        &s.project_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
    );
    result["next_commands"] = json!([format!(
        "{} --project {} storyboard.read scene=1",
        crate::command_center::shell_quote(&executable),
        proj
    )]);
    result["resume_command"] = json!(format!(
        "{} --project {} workshop.build_storyboard draft=true resume={} source={}",
        crate::command_center::shell_quote(&executable),
        proj,
        crate::command_center::shell_quote(&out.join("manifest.json").to_string_lossy()),
        crate::command_center::shell_quote(
            params["source"].as_str().unwrap_or(&dir.to_string_lossy())
        ),
    ));
    if let Some(transcript) = params["transcript"].as_str() {
        let command = format!(
            "{} transcript={} allow_timing_overrun={}",
            result["resume_command"].as_str().unwrap_or_default(),
            crate::command_center::shell_quote(transcript),
            params["allow_timing_overrun"] == true
        );
        result["resume_command"] = json!(command);
    }
    std::fs::write(out.join("manifest.json"), result.to_string())?;
    if result["ok"] == true && result["draft"] != true {
        std::fs::write(dir.join("storyboard/movie-board.json"), result.to_string())?;
    }
    let diag = diagnose_storyboard_and_workshop(s, dir);
    result["scene_diagnostics"] = diag.clone();
    if let Some(steps) = diag["next_steps"].as_array()
        && !steps.is_empty()
    {
        result["next_steps"] = json!(steps);
    }
    Ok(result)
}

/// Builds storyboard/storyboard.pdf with pdfcraft-cli.
pub fn cmd_workshop_build_storyboard(s: &mut Session, params: &Value) -> Result<Value> {
    if !params["template"].is_null() && !params["template"].is_string() {
        return Err(EngineError::InvalidParams(
            "Only the native storyboard template is supported".into(),
        ));
    }
    let dir = project_dir(s)?;
    let active_board = crate::storyboard_document::load(&dir)
        .ok()
        .unwrap_or(Value::Null);
    let selected = cmd_storyboard_template(s, &json!({}))?;
    let raw_template = params["template"]
        .as_str()
        .or(selected["path"].as_str())
        .or(active_board["template_file"].as_str())
        .unwrap_or(DEFAULT_STORYBOARD_TEMPLATE);
    let template = resolve_storyboard_template(s, raw_template)?;
    let transcript = resolve_build_transcript(s, &dir, params)?;
    let title = s
        .project_path
        .as_ref()
        .and_then(|p| p.file_stem())
        .and_then(|n| n.to_str())
        .unwrap_or("Movie board")
        .to_owned();
    let out = resolve_build_output_dir(s, &dir, params)?;
    let scenes = resolve_build_scenes(s, &dir, params, &active_board)?;
    std::fs::create_dir_all(&out)?;
    let scene_map =
        write_build_scene_map(s, &dir, &out, params, &active_board, &transcript, &scenes)?;
    let args = build_movie_board_args(s, params, &template, &transcript, &scene_map, &out, title)?;
    let output = run_tool_in(&dir, "python3", &args)?;
    let result: Value =
        serde_json::from_str(&output).map_err(|e| EngineError::InvalidParams(e.to_string()))?;
    finalize_storyboard_build_result(s, &dir, &out, params, result)
}

/// Current storyboard build state: PDF exists, previews, staleness.
pub fn cmd_workshop_storyboard(s: &mut Session, _: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    if let Ok(mut board) = crate::storyboard_document::load(&dir) {
        if board["design"]["placements"].is_array() {
            let base = crate::storyboard_design::base(&board);
            board["previews"] = base["previews"].clone();
        }
        board["layout"] = json!({"page_size":board["page_size"],"links":board["links"]});
        board["exists"] = json!(true);
        board["ok"] = json!(true);
        return Ok(board);
    }
    let pdf = dir.join(STORYBOARD_DIR).join(STORYBOARD_PDF);
    let preview_dir = dir.join(STORYBOARD_DIR).join(".preview");
    let previews = files_in(&preview_dir, Some(&["png"]));
    let layout = std::fs::read_to_string(preview_dir.join("layout.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .unwrap_or_else(|| json!({}));
    let built = std::fs::metadata(&pdf).and_then(|m| m.modified()).ok();
    let can_build = !transcript_files(&dir).is_empty();
    let stale = can_build
        && match (built, storyboard_inputs_changed(&dir)) {
            (None, _) => !panel_files(&dir).is_empty(),
            (Some(b), Some(changed)) => changed > b,
            (Some(_), None) => false,
        };
    Ok(
        json!({"scene_packets_available":scene_doc_path(&dir,1).is_file(),"editable_slide_integration":"Build the DeckCraft deck; Studio displays previews only","ok": true, "pdf": pdf, "exists": pdf.is_file(), "approved": s.draft_approved, "previews": previews,
        "scene_titles": layout.get("scene_titles").cloned().unwrap_or_else(|| json!([])),
        "first_scene_page": layout.get("first_scene_page").cloned().unwrap_or_else(|| json!(2)), "layout": layout, "stale": stale}),
    )
}

/// Inspect the single editable native template; old per-project selections are obsolete.
pub fn cmd_storyboard_template(s: &mut Session, p: &Value) -> Result<Value> {
    let requested = p["path"]
        .as_str()
        .or(p["name"].as_str())
        .or(p["template"].as_str())
        .unwrap_or(DEFAULT_STORYBOARD_TEMPLATE);
    let path = resolve_storyboard_template(s, requested)?;
    Ok(
        json!({"ok":true,"path":path,"editor":"DeckCraft","selected":DEFAULT_STORYBOARD_TEMPLATE,
        "generator":"pipeline/movie_board.py","template_population":"pipeline/storyboard_layout.py"}),
    )
}

fn init_empty_template_board(s: &mut Session, dir: &Path, active: &Path) -> Result<Value> {
    let selected = cmd_storyboard_template(s, &json!({}))?;
    let Some(template) = selected["path"].as_str() else {
        let board = json!({"ok":true,"project":s.project_path,"empty_template":true,"template_required":true,"previews":[],"page_plan":[],"scene_count":0,"design":{"placements":[],"dirty":false}});
        std::fs::write(active, board.to_string())?;
        return Ok(board);
    };
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let out = dir.join(STORYBOARD_DIR).join(format!("template-{stamp}"));
    std::fs::create_dir_all(&out)?;
    let deck = out.join("movie-board.deckcraft");
    let pdf = out.join("movie-board.pdf");
    let pages = out.join("pages");
    run_tool_in(
        dir,
        "deckcraft-cli",
        &[
            "run".into(),
            "--in".into(),
            template.into(),
            "--save".into(),
            deck.to_string_lossy().into_owned(),
            "--export".into(),
            pdf.to_string_lossy().into_owned(),
        ],
    )?;
    run_tool_in(
        dir,
        "deckcraft-cli",
        &[
            "render".into(),
            deck.to_string_lossy().into_owned(),
            "--all".into(),
            "--scale".into(),
            "1".into(),
            pages.to_string_lossy().into_owned(),
        ],
    )?;
    let info: Value = serde_json::from_str(&run_tool_in(
        dir,
        "deckcraft-cli",
        &["info".into(), deck.to_string_lossy().into_owned()],
    )?)
    .map_err(|e| EngineError::InvalidParams(e.to_string()))?;
    let mut previews = std::fs::read_dir(&pages)?
        .filter_map(|e| e.ok().map(|v| v.path()))
        .filter(|v| v.extension().is_some_and(|e| e == "png"))
        .collect::<Vec<_>>();
    previews.sort();
    let board = json!({"ok":true,"project":s.project_path,"deck":deck,"pdf":pdf,"template_file":template,"manifest":out.join("manifest.json"),"empty_template":true,"template_required":false,"previews":previews,"page_size":info["slideSize"],"page_plan":[],"scene_count":0,"first_scene_page":previews.len()+1,"links":[],"design":{"placements":[],"dirty":false}});
    std::fs::write(out.join("manifest.json"), board.to_string())?;
    std::fs::write(active, board.to_string())?;
    Ok(board)
}

pub fn cmd_storyboard_init(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    // The editable DeckCraft document is authoritative once created. Reopening never regenerates it.
    let active = dir.join("storyboard/movie-board.json");
    if let Ok(board) = crate::storyboard_document::load(&dir)
        && board["deck"]
            .as_str()
            .is_some_and(|p| Path::new(p).is_file())
    {
        if board["empty_template"] == true
            && params["empty"] != true
            && !transcript_files(&dir).is_empty()
        {
            let design = crate::storyboard_design::inspect(s, &json!({}))?;
            return cmd_workshop_build_storyboard(
                s,
                &json!({"_previous_board":board,"_page_content":design["text_fields"]}),
            );
        }
        let mut result = cmd_workshop_storyboard(s, &json!({}))?;
        result["reused_existing_document"] = json!(true);
        return Ok(result);
    }
    for folder in [STORYBOARD_DIR, ASSETS_DIR, "liner-notes"] {
        std::fs::create_dir_all(dir.join(folder))?;
    }
    if params["empty"] == true {
        return init_empty_template_board(s, &dir, &active);
    }
    // Canonical scene spans are parsed data, not another editable Markdown storyboard.
    let mut result = cmd_workshop_build_storyboard(s, &json!({}))?;
    result["reused_existing_document"] = json!(false);
    Ok(result)
}

pub fn cmd_storyboard_toc(s: &mut Session, _: &Value) -> Result<Value> {
    if crate::workshop::project_dir(s)?
        .join("storyboard/movie-board.json")
        .is_file()
    {
        return crate::storyboard_crud::read(s, &json!({}));
    }
    let dir = project_dir(s)?;
    let list = storyboard_scenes(&dir)?;
    let panels = panel_files(&dir).len();
    let first_scene_page = first_scene_page(&dir);
    let chapters: Vec<Value> = list
        .iter()
        .enumerate()
        .map(|(i, sc)| {
            let lines = sc["lines"].as_array().map_or(0, Vec::len);
            json!({"target":format!("scene-{}",i+1),"scene": i + 1, "title": scene_label(i, sc), "page": first_scene_page + i, "status": sc["status"], "lines": lines,
                "panels": lines.min(panels), "notes": sc["notes"].as_array().map_or(0, Vec::len), "source": scene_doc_path(&dir, i + 1)})
        })
        .collect();
    let pages = first_scene_page + list.len();
    Ok(
        json!({"ok": true, "pdf": dir.join(STORYBOARD_DIR).join(STORYBOARD_PDF), "lyrics_page": 1, "chapters": chapters,
        "overview_page": 2, "brief_page": pages}),
    )
}

/// Saved input fingerprint only; polling this never runs a diagnosis.
pub(crate) fn doctor_revision(dir: &Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(dir.join("storyboard/movie-board.json")).ok()?;
    let board: Value = serde_json::from_slice(&bytes).ok()?;
    let mut hash = Sha256::new();
    hash.update(&bytes);
    let mut paths = vec![
        dir.join("artifacts/scene_plan.json"),
        dir.join("artifacts/script.json"),
    ];
    if let Some(path) = board["source_transcript"].as_str() {
        paths.push(path.into());
    }
    for path in paths {
        hash.update(path.to_string_lossy().as_bytes());
        if let Ok(bytes) = std::fs::read(&path) {
            hash.update(bytes);
        } else {
            hash.update(b"missing");
        }
    }
    Some(format!("{:x}", hash.finalize()))
}

/// Explicit read-only diagnosis of the loaded project's single storyboard.
pub fn cmd_storyboard_doctor(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let manifest = dir.join("storyboard/movie-board.json");
    if !manifest.is_file() {
        return Err(EngineError::InvalidParams(
            "No saved storyboard for this project".into(),
        ));
    }
    crate::activity::command(
        s.project_path.as_deref(),
        "storyboard.doctor",
        p,
        "dispatch_running",
        None,
    );
    let start = std::time::Instant::now();
    let result = (|| -> Result<Value> {
        let script = crate::workspace::workspace_root(s).join("pipeline/movie_board.py");
        let output = run_tool_in(
            &dir,
            "python3",
            &[
                script.to_string_lossy().into_owned(),
                "--doctor".into(),
                manifest.to_string_lossy().into_owned(),
                "--source".into(),
                dir.to_string_lossy().into_owned(),
            ],
        )?;
        let mut value: Value =
            serde_json::from_str(&output).map_err(|e| EngineError::InvalidParams(e.to_string()))?;
        value["project"] = json!(s.project_path);
        value["storyboard_revision"] = json!(doctor_revision(&dir));
        value["assessed_at_unix_ms"] = json!(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|v| v.as_millis())
                .unwrap_or(0)
        );
        let report = dir.join(".agent/storyboard-doctor.json");
        std::fs::create_dir_all(
            report
                .parent()
                .ok_or_else(|| EngineError::InvalidParams("Missing report folder".into()))?,
        )?;
        value["output"] = json!(report);
        std::fs::write(
            &report,
            serde_json::to_vec_pretty(&value)
                .map_err(|e| EngineError::InvalidParams(e.to_string()))?,
        )?;
        Ok(value)
    })();
    if let Ok(value) = &result {
        crate::activity::result(
            s.project_path.as_deref(),
            "storyboard.doctor",
            value,
            start.elapsed().as_millis(),
        );
    }
    crate::activity::command(
        s.project_path.as_deref(),
        "storyboard.doctor",
        p,
        if result.is_err() {
            "engine_error"
        } else {
            "engine_success"
        },
        result.as_ref().err().map(|e| e.to_string()).as_deref(),
    );
    result
}

pub fn cmd_storyboard_read(s: &mut Session, params: &Value) -> Result<Value> {
    if crate::workshop::project_dir(s)?
        .join("storyboard/movie-board.json")
        .is_file()
    {
        return crate::storyboard_crud::read(s, params);
    }
    let dir = project_dir(s)?;
    let list = storyboard_scenes(&dir)?;
    let n = scene_number(params, list.len())?;
    let scene = &list[n - 1];
    let panels = panel_files(&dir);
    let lines: Vec<Value> = scene["lines"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .enumerate()
        .map(|(j, l)| {
            let panel = panels.get(j);
            json!({"line": j + 1, "start": l["start"], "time": clock(l["start"].as_f64().unwrap_or(0.0)), "lyric": l["text"], "heard": l.get("heard"),
                "cue": l.get("cue"), "panel": panel, "prompt": panel.and_then(|p| std::fs::read_to_string(p.with_extension("prompt.md")).ok())})
        })
        .collect();
    let mut result = json!({"target":format!("scene-{n}"),"ok": true, "scene": n, "title": scene_label(n - 1, scene), "kind": scene["kind"], "start": scene["start"], "end": scene["end"],
        "status": scene["status"], "notes": scene["notes"], "reference": scene["reference"], "teaching_reference": scene["teaching_reference"],
        "sections": scene["sections"], "lines": lines, "source": scene_doc_path(&dir, n), "page": first_scene_page(&dir) + n - 1});
    if let Ok(board) = crate::storyboard_document::load(&dir) {
        let page = board["first_scene_page"].as_u64().unwrap_or(1) + n as u64 - 1;
        // The board's spans describe the scenes it was built from. After the song's scenes change
        // (new captions), a stale board must not hand its old spans back to the rebuild.
        let same_scenes = board["scene_count"].as_u64() == Some(list.len() as u64);
        if let Some(group) = board["page_plan"]
            .as_array()
            .filter(|_| same_scenes)
            .and_then(|pages| {
                pages
                    .iter()
                    .filter_map(|p| p.get("group"))
                    .find(|g| g["id"] == format!("scene-{n}"))
            })
        {
            result["start"] = group["span_start"].clone();
            result["end"] = group["span_end"].clone();
            result["kind"] = group["kind"].clone();
            result["panels"] = group["panels"].clone();
            result["section_label"] = group["section_label"].clone();
            if let Some(label) = group["section_label"].as_str() {
                result["title"] = json!(format!("{label} / Scene {n:02}"));
            }
        }
        result["page"] = json!(page);
        result["deck"] = board["deck"].clone();
        result["preview"] = board["previews"][page as usize - 1].clone();
        result["page_size"] = board["page_size"].clone();
        result["saved_placements"] = json!(
            board["design"]["placements"]
                .as_array()
                .map(|xs| xs
                    .iter()
                    .filter(|p| p["page"].as_u64() == Some(page))
                    .cloned()
                    .collect::<Vec<_>>())
                .unwrap_or_default()
        );
        let design = crate::storyboard_design::inspect(s, &json!({}))?;
        result["text_fields"] = json!(
            design["text_fields"]
                .as_array()
                .map(|xs| xs
                    .iter()
                    .filter(|f| f["page"].as_u64() == Some(page))
                    .cloned()
                    .collect::<Vec<_>>())
                .unwrap_or_default()
        );
        result["draft_placements"] = json!(
            design["placements"]
                .as_array()
                .map(|xs| xs
                    .iter()
                    .filter(|p| p["page"].as_u64() == Some(page))
                    .cloned()
                    .collect::<Vec<_>>())
                .unwrap_or_default()
        );
    }
    Ok(result)
}

/// Changes one scene's source: a lyric correction, a line's panel prompt or image, a note, or the status.
pub fn cmd_storyboard_edit(s: &mut Session, params: &Value) -> Result<Value> {
    crate::storyboard_crud::update(s, params)
}

fn packet_text_of(v: &Value) -> Option<String> {
    match v {
        Value::String(t) => Some(t.clone()),
        Value::Array(items) => Some(
            items
                .iter()
                .map(|i| {
                    format!(
                        "- {}",
                        i.as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| i.to_string())
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

fn build_imported_scene_doc(
    dir: &Path,
    src: &Path,
    n: usize,
    sc: &Value,
    packet: &Value,
) -> Result<SceneDoc> {
    let mut doc = SceneDoc {
        status: "draft".into(),
        ..Default::default()
    };
    doc.title = packet["title"].as_str().map(str::to_owned).or_else(|| {
        sc.get("description")
            .and_then(Value::as_str)
            .map(|d| d.chars().take(60).collect())
    });
    doc.time = sc
        .get("start_seconds")
        .and_then(Value::as_f64)
        .zip(sc.get("end_seconds").and_then(Value::as_f64));
    if let Some(reference) = packet["reference"]
        .as_str()
        .map(|r| src.join(r))
        .filter(|p| p.is_file())
    {
        let ext = reference
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png");
        let rel = format!("{STORYBOARD_DIR}/scene-{n}-reference.{ext}");
        std::fs::copy(&reference, dir.join(&rel))?;
        doc.reference = Some(rel);
    }
    let skills = packet["skills"].as_array().map(|a| {
        a.iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    });
    let skills = match (skills, packet["combination"].as_str()) {
        (Some(k), Some(c)) => Some(format!("{k}\n{c}")),
        (k, c) => k.or(c.map(str::to_owned)),
    };
    for (key, value) in [
        ("skills", skills),
        ("reference_prompt", packet_text_of(&packet["reference_prompt"])),
        ("animation_prompt", packet_text_of(&packet["animation_prompt"])),
        ("layer_stack", packet_text_of(&packet["layer_stack_proposal"])),
        ("build_steps", packet_text_of(&packet["build_steps"])),
        ("acceptance_checks", packet_text_of(&packet["output_checks"])),
        ("plan_vs_actual", packet_text_of(&packet["plan_vs_actual"])),
    ] {
        if let Some(v) = value.filter(|v| !v.trim().is_empty()) {
            doc.sections.insert(key.into(), v);
        }
    }
    if let Some(desc) = sc.get("description").and_then(Value::as_str) {
        doc.notes.push(format!("Scene plan: {desc}"));
    }
    if let Some(status) = packet["status"].as_str() {
        doc.notes.push(format!("Imported status: {status}"));
    }
    Ok(doc)
}

fn write_imported_scene_docs(
    dir: &Path,
    src: &Path,
    plan_scenes: &[Value],
    packets: &Option<Value>,
) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir.join(STORYBOARD_DIR))?;
    let mut written = Vec::new();
    for (i, sc) in plan_scenes.iter().enumerate() {
        let n = i + 1;
        let id = sc.get("id").and_then(Value::as_str).unwrap_or_default();
        let packet = packets
            .as_ref()
            .and_then(|p| p.get("scenes"))
            .and_then(Value::as_array)
            .and_then(|list| list.iter().find(|p| p["id"].as_str() == Some(id)))
            .cloned()
            .unwrap_or(Value::Null);
        let doc = build_imported_scene_doc(dir, src, n, sc, &packet)?;
        write_scene_doc(dir, n, &format!("Scene {n}"), &doc)?;
        written.push(scene_doc_path(dir, n));
    }
    for n in plan_scenes.len() + 1.. {
        let extra = scene_doc_path(dir, n);
        if !extra.is_file() {
            break;
        }
        std::fs::remove_file(extra)?;
    }
    Ok(written)
}

/// Imports an existing pipeline project's scenes into this storyboard template.
pub fn cmd_storyboard_import_magicstudio(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let src = params
        .get("path")
        .and_then(Value::as_str)
        .map(|p| resolve_workspace_path(s, p))
        .ok_or_else(|| {
            EngineError::InvalidParams("missing 'path' (the existing project folder)".into())
        })?;
    if params["preserve_document"] == true {
        return Err(EngineError::InvalidParams("Static document copying was removed. Import scene content normally, then use workshop.build_storyboard draft=true to populate the selected DeckCraft template.".into()));
    }
    let overwrite = params
        .get("overwrite")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let read = |rel: &str| -> Option<Value> {
        serde_json::from_str(&std::fs::read_to_string(src.join(rel)).ok()?).ok()
    };
    let plan = read("artifacts/scene_plan.json").ok_or_else(|| {
        EngineError::InvalidParams(format!("no artifacts/scene_plan.json in {}", src.display()))
    })?;
    let packets = read("review/scene-packets.json");
    let plan_scenes = plan
        .get("scenes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if plan_scenes.is_empty() {
        return Err(EngineError::InvalidParams(
            "scene_plan.json has no scenes".into(),
        ));
    }
    if !overwrite && scene_doc_path(&dir, 1).is_file() {
        return Err(EngineError::InvalidParams(
            "this project already has scene files (pass \"overwrite\": true to replace them)"
                .into(),
        ));
    }
    let written = write_imported_scene_docs(&dir, &src, &plan_scenes, &packets)?;
    let mut toc = cmd_storyboard_toc(s, &json!({}))?;
    toc["imported"] = json!(written);
    toc["next"] = json!([
        "workshop.build_storyboard draft=true to populate the DeckCraft document from native transcript words and imported creative fields",
        "Review the rendered draft and mapping report, then storyboard.save_version draft=<returned manifest>"
    ]);
    Ok(toc)
}

pub fn cmd_storyboard_approve(s: &mut Session, params: &Value) -> Result<Value> {
    let approved = params
        .get("approved")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let status = if approved { "approved" } else { "needs review" };
    let revision = crate::storyboard_crud::read(s, &json!({}))?["revision"].clone();
    cmd_storyboard_edit(
        s,
        &json!({"scene": params.get("scene"), "field": "status", "value": status,"expected_revision":revision}),
    )
}

fn diagnose_video_setup(s: &Session, dir: &Path) -> (Vec<String>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut next = Vec::new();
    let hf = dir.join("hyperframes");
    if !hf.join("hyperframes.json").is_file() {
        warnings.push("HyperFrames project (hyperframes/hyperframes.json) is missing.".into());
        next.push("Run `magic-cli exec workshop.set_audio path=<song.wav>` to scaffold hyperframes/ and link the song.".into());
        return (warnings, next);
    }
    if s.video_project_path.as_ref().is_none_or(|p| !p.exists()) {
        warnings.push("Project file is not linked to hyperframes/hyperframes.json.".into());
        next.push("Run `magic-cli exec video.init target=hyperframes` to link the HyperFrames project.".into());
    }
    if let Ok(html) = std::fs::read_to_string(hf.join("index.html"))
        && html.contains("<audio")
        && !html.contains("data-timeline-role=\"music\"")
    {
        warnings.push("hyperframes/index.html <audio> is missing data-timeline-role=\"music\" (required by `hyperframes beats`).".into());
        next.push("Run `magic-cli exec audio.analyze` to tag the music track and generate beat/section analysis.".into());
    }
    if !hf.join("audio-analysis.json").is_file() {
        warnings.push("hyperframes/audio-analysis.json is missing (sections/onsets/moments not analyzed).".into());
        next.push("Run `magic-cli exec audio.analyze` to generate transcript.json, transcript.srt, audio-analysis.json, and beats/.".into());
    }
    let has_beats = std::fs::read_dir(hf.join("beats"))
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.path().extension().is_some_and(|x| x == "json"));
    if !has_beats {
        warnings.push("hyperframes/beats/*.json beat grid is missing.".into());
        next.push("Run `hyperframes beats hyperframes --json` or `magic-cli exec audio.analyze` to build the Studio beat grid.".into());
    }
    (warnings, next)
}

fn check_scene_reference(
    dir: &Path,
    n: usize,
    doc: &SceneDoc,
    seen_refs: &mut std::collections::BTreeMap<String, usize>,
) -> (Option<String>, bool) {
    let Some(rel) = doc.reference.as_deref().filter(|r| !r.trim().is_empty()) else {
        return (Some(format!("Scene {n:02} is missing a `reference:` plate image.")), false);
    };
    let path = dir.join(rel);
    if !path.is_file() {
        return (Some(format!("Scene {n:02} reference `{rel}` does not exist on disk.")), false);
    }
    let mut issue = None;
    let mut is_16_9 = true;
    if let Ok((w, h)) = image::image_dimensions(&path)
        && (h == 0 || (w as f64 / h as f64) < 1.45)
    {
        issue = Some(format!(
            "Scene {n:02} reference `{rel}` is {w}x{h} (not 16:9 widescreen; will letterbox or crop)."
        ));
        is_16_9 = false;
    }
    if let Some(prev) = seen_refs.insert(rel.to_string(), n) {
        let dup = format!("Scene {n:02} reuses `{rel}` from Scene {prev:02} instead of a dedicated scene plate.");
        issue = Some(match issue {
            Some(existing) => format!("{existing} {dup}"),
            None => dup,
        });
        is_16_9 = false;
    }
    (issue, is_16_9)
}

pub(crate) fn diagnose_storyboard_and_workshop(s: &Session, dir: &Path) -> Value {
    let (mut warnings, mut next_steps) = diagnose_video_setup(s, dir);
    let scene_docs: Vec<(usize, SceneDoc)> = (1..)
        .map(|n| (n, scene_doc_path(dir, n)))
        .take_while(|(_, p)| p.is_file())
        .map(|(n, _)| (n, read_scene_doc(dir, n)))
        .collect();
    if scene_docs.is_empty() {
        warnings.push("No storyboard/scene-N.md files exist yet.".into());
        next_steps.push("Run `magic-cli exec storyboard.init` to create the initial DeckCraft storyboard and scene files.".into());
        return json!({"warnings": warnings, "next_steps": next_steps, "scenes_total": 0, "all_scenes_complete": false});
    }
    let required = ["action", "reference_prompt", "animation_prompt", "layer_stack", "animated_text", "teaching_cues"];
    let mut seen_refs = std::collections::BTreeMap::new();
    let mut art_ok_count = 0usize;
    let mut sections_ok_count = 0usize;
    for (n, doc) in &scene_docs {
        let (ref_issue, ref_ok) = check_scene_reference(dir, *n, doc, &mut seen_refs);
        if let Some(msg) = ref_issue {
            warnings.push(msg);
        }
        if ref_ok {
            art_ok_count += 1;
        }
        let missing: Vec<&str> = required
            .iter()
            .copied()
            .filter(|k| doc.sections.get(*k).is_none_or(|v| v.trim().is_empty()))
            .collect();
        if missing.is_empty() {
            sections_ok_count += 1;
        } else {
            warnings.push(format!("Scene {n:02} is missing sections: {}.", missing.join(", ")));
        }
    }
    let total = scene_docs.len();
    if art_ok_count < total {
        next_steps.push(format!(
            "Generate dedicated 16:9 scene plates for all {total} scenes ({art_ok_count}/{total} unique 16:9 plates ready) and set `reference: assets/backgrounds/scene-NN-....jpg` in storyboard/scene-N.md."
        ));
    }
    if sections_ok_count < total {
        next_steps.push(format!(
            "Fill in all 6 storyboard sections (`Action`, `Reference prompt`, `Animation prompt`, `Layer stack`, `Animated text`, `Teaching cues`) in storyboard/scene-1.md..scene-{total}.md ({sections_ok_count}/{total} complete), then run `magic-cli exec workshop.build_storyboard` and `magic-cli exec words.apply`."
        ));
    }
    json!({
        "scenes_total": total,
        "scenes_with_unique_16_9_art": art_ok_count,
        "scenes_with_complete_sections": sections_ok_count,
        "all_scenes_complete": art_ok_count == total && sections_ok_count == total,
        "warnings": warnings,
        "next_steps": next_steps,
    })
}
