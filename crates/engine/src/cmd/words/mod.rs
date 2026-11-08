//! Words: editable visible lyric animations built from the artist's canonical lines, timed by
//! the hidden transcript reference in [`Tick`]s (`filmcraft_time::Tick`).
//! The reference (heard words, their order and times) is never edited or exported; mismatches
//! against the artist's words are reported, never substituted.
use crate::time::{TICKS_PER_SECOND, Tick, parse_tick};
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub mod compose;
pub mod lettering;
pub mod lyrics;
pub(crate) mod lyrics_layout;
pub(crate) mod motion_curve;
pub(crate) mod native_motion;
pub(crate) mod preview;
pub(crate) mod stage;
pub(crate) mod validate;

pub use compose::{cmd_words_apply, cmd_words_compose, cmd_words_design, cmd_words_effect_install, cmd_words_track};
pub(crate) use stage::one_of;
pub use stage::{cmd_words_effects, cmd_words_link, cmd_words_lyrics_view, cmd_words_read, cmd_words_save};

pub(crate) const ACTIONS: &[&str] = &["none", "wave", "tiny", "swim", "no", "cry", "big", "float"];
pub(crate) const ENTRANCES: &[&str] = &["pop", "fade", "rise", "none"];
pub(crate) const EXITS: &[&str] = &["fade", "drop", "none"];
pub(crate) const MOVES: &[&str] = &["quiet", "sleep", "settle", "fall", "pop", "rise", "east", "west", "loop"];
pub(crate) const FONTS: &[&str] = &["song", "serif", "sans"];
/// 0.4 seconds in FilmCraft ticks (`254_016_000_000 * 2 / 5`).
pub(crate) const LEAD_IN_TICK: Tick = Tick(101_606_400_000);
/// 1.0 second in FilmCraft ticks.
pub(crate) const HOLD_TICK: Tick = Tick(TICKS_PER_SECOND);
pub(crate) const KINETIC_LIBRARY: &str = include_str!("../../../../../pipeline/templates/kinetic-library.json");

pub(crate) fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

pub(crate) fn workspace_dir(s: &Session) -> Result<PathBuf> {
    if let Some(folder) = &s.lyrics_workspace {
        return Ok(folder.clone());
    }
    crate::workshop::project_dir(s)
}

/// Select an existing lyrics folder without opening or writing a `.magic` project file.
pub fn cmd_words_workspace(s: &mut Session, params: &Value) -> Result<Value> {
    if let Some(raw) = params["path"].as_str() {
        if s.project_path.is_some() {
            return Err(invalid("Close the Magic project before opening a standalone lyrics folder"));
        }
        let folder = crate::workspace::resolve_workspace_path(s, raw).canonicalize()?;
        if !folder.is_dir() {
            return Err(invalid("Choose a lyrics folder"));
        }
        let sheet = lyrics::song_lyrics_in(&folder)?;
        if sheet["sheet"].as_str().is_none() {
            return Err(invalid("This folder has no canonical lyric sheet"));
        }
        let (_, _, words) = reference(&folder)?;
        if words.is_empty() {
            return Err(invalid("This folder has no words in its timed transcript"));
        }
        let audio = if let Some(raw) = params["audio"].as_str() {
            Some(crate::workspace::song_audio_path(s, raw)?)
        } else {
            {
                let files = [&folder, &folder.join("audio")]
                    .into_iter()
                    .flat_map(|dir| fs::read_dir(dir).into_iter().flatten().flatten())
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.is_file()
                            && path
                                .extension()
                                .and_then(|ext| ext.to_str())
                                .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "mp3" | "wav" | "flac" | "m4a" | "ogg" | "aac"))
                    })
                    .collect::<Vec<_>>();
                (files.len() == 1).then(|| files[0].clone())
            }
        };
        if s.lyrics_workspace.as_ref() != Some(&folder) {
            s.close_documents();
            s.lyrics_workspace = Some(folder);
        }
        s.song_audio = audio;
    }
    Ok(json!({"ok":true,"standalone":s.lyrics_workspace.is_some(),"path":s.lyrics_workspace,"audio":s.song_audio}))
}

fn norm(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

pub(crate) fn document_path(dir: &Path) -> PathBuf {
    dir.join("design/words.json")
}

fn heard_text(w: &Value) -> &str {
    w["text"].as_str().or_else(|| w["word"].as_str()).unwrap_or_default().trim()
}

pub(crate) fn word_tick(w: &Value, tick_key: &str, sec_key: &str) -> Option<Tick> {
    w.get(tick_key).and_then(Value::as_i64).map(Tick).or_else(|| w.get(sec_key).and_then(parse_tick))
}

pub(crate) fn reference(dir: &Path) -> Result<(PathBuf, String, Vec<Value>)> {
    let path =
        crate::workshop::transcript_files(dir).into_iter().next().ok_or_else(|| invalid("No transcript in this project; the timing reference is missing"))?;
    let bytes = fs::read(&path)?;
    let doc: Value = serde_json::from_slice(&bytes).map_err(|e| invalid(e.to_string()))?;
    let words = doc
        .as_array()
        .or_else(|| doc["words"].as_array())
        .cloned()
        .unwrap_or_else(|| doc["segments"].as_array().into_iter().flatten().flat_map(|seg| seg["words"].as_array().cloned().unwrap_or_default()).collect());
    Ok((path, format!("{:x}", Sha256::digest(&bytes)), words))
}

fn align(canonical: &[String], heard: &[String]) -> Vec<Option<usize>> {
    let (n, m) = (canonical.len(), heard.len());
    let mut cost = vec![vec![0u32; m + 1]; n + 1];
    for (i, row) in cost.iter_mut().enumerate().take(n + 1) {
        row[0] = i as u32;
    }
    for (j, cell) in cost[0].iter_mut().enumerate().take(m + 1) {
        *cell = j as u32;
    }
    for i in 1..=n {
        for j in 1..=m {
            let sub = cost[i - 1][j - 1] + u32::from(canonical[i - 1] != heard[j - 1]);
            cost[i][j] = sub.min(cost[i - 1][j] + 1).min(cost[i][j - 1] + 1);
        }
    }
    let mut out = vec![None; n];
    let (mut i, mut j) = (n, m);
    while i > 0 && j > 0 {
        let sub = cost[i - 1][j - 1] + u32::from(canonical[i - 1] != heard[j - 1]);
        if cost[i][j] == sub {
            out[i - 1] = Some(j - 1);
            i -= 1;
            j -= 1;
        } else if cost[i][j] == cost[i - 1][j] + 1 {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    out
}

fn collect_canonical_words(lyrics: &Value) -> Vec<Value> {
    let mut words = Vec::new();
    for section in lyrics["sections"].as_array().into_iter().flatten() {
        for line in section["lines"].as_array().into_iter().flatten() {
            let sung = line["sung_id"].as_str().or(line["id"].as_str()).unwrap_or_default();
            let timed = line["words"].as_array().cloned().unwrap_or_default();
            let texts: Vec<String> = if timed.is_empty() {
                line["text"].as_str().unwrap_or_default().split_whitespace().map(str::to_owned).collect()
            } else {
                timed.iter().filter_map(|w| w["text"].as_str().map(str::to_owned)).collect()
            };
            for (k, text) in texts.into_iter().enumerate() {
                words.push(json!({
                    "word_id": format!("{sung}/w{k}"),
                    "text": text,
                    "line_id": line["id"],
                    "sung_id": sung,
                    "section": section["section"],
                    "occurrence": section["occurrence"],
                    "forced": timed.get(k).map(|w| json!([w["start"], w["end"]]))
                }));
            }
        }
    }
    words
}

fn attach_reference_timing(words: &mut [Value], heard: &[Value], heard_norm: &[String], map: &[Option<usize>]) {
    for (word, found) in words.iter_mut().zip(map) {
        let status = match found {
            Some(j) if canonical_eq(word, &heard_norm[*j]) => "match",
            Some(_) => "substituted",
            None => "missing",
        };
        let (start, end, source) = match (found, status) {
            (Some(j), "match") => (word_tick(&heard[*j], "start_tick", "start"), word_tick(&heard[*j], "end_tick", "end"), "reference"),
            _ => match word["forced"].as_array() {
                Some(f) => (f.first().and_then(parse_tick), f.get(1).and_then(parse_tick), "forced_alignment"),
                None => (None, None, "none"),
            },
        };
        word["ref_index"] = json!(found);
        word["heard"] = found.map_or(Value::Null, |j| json!(heard_text(&heard[j])));
        word["status"] = json!(status);
        word["start_tick"] = start.map_or(Value::Null, |t| json!(t.0));
        word["end_tick"] = end.map_or(Value::Null, |t| json!(t.0));
        word["start"] = start.map_or(Value::Null, |t| json!(t.seconds()));
        word["end"] = end.map_or(Value::Null, |t| json!(t.seconds()));
        word["timing_source"] = json!(source);
    }
}

pub(crate) fn aligned(s: &mut Session) -> Result<Value> {
    aligned_in(&crate::cmd::words::workspace_dir(s)?)
}

/// The artist's words in sung order with transcript timing in `Tick`s, for a project folder.
pub(crate) fn aligned_in(dir: &Path) -> Result<Value> {
    let dir = dir.to_path_buf();
    let lyrics = crate::cmd::words::lyrics::song_lyrics_in(&dir)?;
    if lyrics["source"] != "canonical" {
        return Err(invalid("No artist lyric sheet; Words needs the artist's canonical lines"));
    }
    let (ref_path, ref_sha, heard) = reference(&dir)?;
    let mut words = collect_canonical_words(&lyrics);
    let canonical: Vec<String> = words.iter().map(|w| norm(w["text"].as_str().unwrap_or_default())).collect();
    let heard_norm: Vec<String> = heard.iter().map(|w| norm(heard_text(w))).collect();
    let map = align(&canonical, &heard_norm);
    attach_reference_timing(&mut words, &heard, &heard_norm, &map);
    let mismatches: Vec<Value> = words
        .iter()
        .filter(|w| w["status"] != "match")
        .map(|w| json!({"word_id":w["word_id"],"line_id":w["line_id"],"canonical":w["text"],"heard":w["heard"],"kind":w["status"]}))
        .collect();
    let half_second = TICKS_PER_SECOND / 2;
    let disagreements: Vec<Value> = words
        .iter()
        .filter(|w| w["timing_source"] == "reference")
        .filter_map(|w| {
            let forced = w["forced"].as_array()?;
            let rs = word_tick(w, "start_tick", "start")?;
            let fs = forced.first().and_then(parse_tick)?;
            ((rs.0 - fs.0).abs() > half_second).then(|| json!({"word_id":w["word_id"],"text":w["text"],"reference":[w["start"], w["end"]],"measured":forced}))
        })
        .collect();
    for word in &mut words {
        word.as_object_mut().ok_or_else(|| invalid("Aligned word is not an object"))?.remove("forced");
    }
    let heard_used: Vec<usize> = map.iter().flatten().copied().collect();
    let unmatched_heard: Vec<Value> =
        (0..heard.len()).filter(|j| !heard_used.contains(j)).map(|j| json!({"ref_index":j,"heard":heard_text(&heard[j])})).collect();
    Ok(json!({
        "reference": {"kind":"transcript","path":ref_path,"sha256":ref_sha,"word_count":heard.len()},
        "canonical": {"sheet":lyrics["sheet"],"original":lyrics["original"]["path"]},
        "words": words,
        "mismatches": mismatches,
        "timing_disagreements": disagreements,
        "unmatched_heard": unmatched_heard
    }))
}

fn canonical_eq(word: &Value, heard: &str) -> bool {
    norm(word["text"].as_str().unwrap_or_default()) == heard
}

pub fn cmd_words_align(s: &mut Session, _: &Value) -> Result<Value> {
    let mut report = aligned(s)?;
    let total = report["words"].as_array().map_or(0, Vec::len);
    let matched = report["words"].as_array().into_iter().flatten().filter(|w| w["status"] == "match").count();
    report["summary"] = json!({
        "canonical_words": total,
        "matched": matched,
        "mismatched": total - matched,
        "timing_disagreements": report["timing_disagreements"].as_array().map_or(0, Vec::len)
    });
    Ok(report)
}

/// Inspect the project's lyric timing track in `Tick`s and report section overlaps.
pub fn cmd_words_studio_track(s: &mut Session, _params: &Value) -> Result<Value> {
    let dir = crate::cmd::words::workspace_dir(s)?;
    let source = document_path(&dir);
    let (doc, _) = match load(s, &dir)? {
        Some(doc) => (with_sung_sections(doc, &aligned(s)?)?, true),
        None => (default_document(&aligned(s)?)?, false),
    };
    let mut sections = Vec::new();
    let mut clip_count = 0usize;
    for anim in doc["animations"].as_array().into_iter().flatten() {
        let scope = &anim["scope"];
        let mut clips = Vec::new();
        for w in anim["words"].as_array().into_iter().flatten() {
            let Some(start) = word_tick(w, "start_tick", "start") else { continue };
            let end = word_tick(w, "end_tick", "end").unwrap_or(start);
            let dur = Tick((end.0 - start.0).max(0));
            let wid = w["word_id"].as_str().unwrap_or_default();
            clips.push(json!({
                "clip_id": wid.replace("/w", "-"),
                "word_id": wid,
                "line_id": w["line_id"],
                "text": w["text"],
                "start_tick": start.0,
                "duration_tick": dur.0,
                "start": start.seconds(),
                "duration": dur.seconds()
            }));
        }
        if clips.is_empty() {
            continue;
        }
        clip_count += clips.len();
        let start_tick = anim["song_anchor_tick"].as_i64().map(Tick).or_else(|| anim["song_anchor"].as_f64().map(Tick::from_seconds_f64)).unwrap_or(Tick::ZERO);
        let dur_tick =
            anim["authored_duration_tick"].as_i64().map(Tick).or_else(|| anim["authored_duration"].as_f64().map(Tick::from_seconds_f64)).unwrap_or(Tick::ZERO);
        let end_tick = start_tick + dur_tick;
        sections.push(json!({
            "animation_id": anim["animation_id"],
            "section": scope["section"],
            "occurrence": scope["occurrence"],
            "start_tick": start_tick.0,
            "end_tick": end_tick.0,
            "start": start_tick.seconds(),
            "end": end_tick.seconds(),
            "clips": clips
        }));
    }
    let mut overlaps = Vec::new();
    let tolerance = TICKS_PER_SECOND / 20; // 50ms in ticks
    for i in 0..sections.len().saturating_sub(1) {
        let cur_end = sections[i]["end_tick"].as_i64().unwrap_or(0);
        let next_start = sections[i + 1]["start_tick"].as_i64().unwrap_or(i64::MAX);
        if cur_end > next_start + tolerance {
            let mid = Tick((next_start + cur_end) / 2);
            overlaps.push(json!({
                "first_section": sections[i]["animation_id"],
                "second_section": sections[i + 1]["animation_id"],
                "start_tick": next_start,
                "end_tick": cur_end,
                "at_tick": mid.0,
                "start": Tick(next_start).seconds(),
                "end": Tick(cur_end).seconds(),
                "at": mid.seconds()
            }));
        }
    }
    Ok(json!({
        "source": source,
        "clip_count": clip_count,
        "sections": sections,
        "overlaps": overlaps,
        "unmatched": []
    }))
}

pub fn cmd_words_overlaps(s: &mut Session, p: &Value) -> Result<Value> {
    let track = cmd_words_studio_track(s, p)?;
    let overlaps = track["overlaps"].as_array().cloned().unwrap_or_default();
    Ok(json!({
        "source": track["source"],
        "overlap_count": overlaps.len(),
        "overlaps": overlaps
    }))
}

pub(crate) fn with_sung_sections(mut doc: Value, report: &Value) -> Result<Value> {
    let fresh = default_document(report)?["animations"].as_array().cloned().unwrap_or_default();
    let ids: Vec<Value> = fresh.iter().map(|a| a["animation_id"].clone()).collect();
    if let Some(list) = doc["animations"].as_array_mut() {
        for (k, anim) in fresh.into_iter().enumerate() {
            if list.iter().any(|a| a["animation_id"] == anim["animation_id"]) {
                continue;
            }
            let at = list.iter().position(|a| ids.iter().position(|id| *id == a["animation_id"]).is_some_and(|j| j > k)).unwrap_or(list.len());
            list.insert(at, anim);
        }
    }
    Ok(doc)
}

pub(crate) fn default_document(report: &Value) -> Result<Value> {
    let mut animations: Vec<Value> = Vec::new();
    for w in report["words"].as_array().into_iter().flatten() {
        let id = format!("words-{}-{}", w["section"].as_str().unwrap_or("lyrics").replace("_", "-"), w["occurrence"].as_u64().unwrap_or(1));
        if animations.last().is_none_or(|a| a["animation_id"] != json!(id)) {
            animations.push(json!({
                "animation_id": id,
                "scope": {"kind":"section","section":w["section"],"occurrence":w["occurrence"],"line_ids":[],"sung_ids":[]},
                "source_revision": 0,
                "origin": null,
                "style": {},
                "kinetic": {
                    "layout": "karaoke",
                    "entrance": "pop",
                    "acts": {},
                    "draws": [],
                    "overlay": "none",
                    "band": "top",
                    "seed": 1,
                    "style": {"font":"Fraunces","hero_font":"Fraunces","weight":600,"hero_weight":700,"hero_italic":true,"size":58.0,"hero_size":92.0,"color":"#FFF6E3","hero_color":"#FFD27D","outline":"#143042"}
                },
                "words": []
            }));
        }
        let anim = animations.last_mut().ok_or_else(|| invalid("Missing generated animation"))?;
        for (key, field) in [("line_ids", "line_id"), ("sung_ids", "sung_id")] {
            let scope = anim["scope"][key].as_array_mut().ok_or_else(|| invalid("Generated animation scope is not an array"))?;
            if !scope.contains(&w[field]) {
                scope.push(w[field].clone());
            }
        }
        anim["words"].as_array_mut().ok_or_else(|| invalid("Generated animation words are not an array"))?.push(json!({
            "word_id": w["word_id"],
            "text": w["text"],
            "line_id": w["line_id"],
            "sung_id": w["sung_id"],
            "ref_index": w["ref_index"],
            "start_tick": w["start_tick"],
            "end_tick": w["end_tick"],
            "start": w["start"],
            "end": w["end"],
            "timing_source": w["timing_source"],
            "emphasis": false,
            "action": "none",
            "entrance": "pop",
            "exit": "fade"
        }));
    }
    for anim in &mut animations {
        fill_untimed(anim);
        timing(anim);
    }
    Ok(json!({
        "format": "magic.words",
        "version": 1,
        "reference": report["reference"],
        "canonical": report["canonical"],
        "mismatches": report["mismatches"],
        "animations": animations
    }))
}

pub(crate) fn word_time_key(scope: &Value, id: &str) -> String {
    format!("{}#{}:{id}", scope["section"].as_str().unwrap_or(""), scope["occurrence"].as_u64().unwrap_or(1))
}

pub(crate) fn studio_times(s: &mut Session) -> std::collections::HashMap<String, (Tick, Tick)> {
    let Ok(dir) = workspace_dir(s) else {
        return std::collections::HashMap::new();
    };
    let track_file = dir.join("design/words-track.json");
    let Ok(bytes) = fs::read(track_file) else {
        return std::collections::HashMap::new();
    };
    let Ok(track) = serde_json::from_slice::<Value>(&bytes) else {
        return std::collections::HashMap::new();
    };
    track["clips"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let key = word_time_key(c, c["word_id"].as_str()?);
            let start = word_tick(c, "start_tick", "start")?;
            let end = word_tick(c, "end_tick", "end")?;
            Some((key, (start, end)))
        })
        .collect()
}

pub(crate) fn fill_untimed(anim: &mut Value) {
    let Some(words) = anim["words"].as_array_mut() else {
        return;
    };
    let timed = |w: &Value| word_tick(w, "start_tick", "start").is_some() && word_tick(w, "end_tick", "end").is_some();
    for w in words.iter_mut() {
        if let (Some(st), Some(et)) = (word_tick(w, "start_tick", "start"), word_tick(w, "end_tick", "end")) {
            w["start_tick"] = json!(st.0);
            w["end_tick"] = json!(et.0);
            w["start"] = json!(st.seconds());
            w["end"] = json!(et.seconds());
        }
    }
    let default_word_step = Tick(TICKS_PER_SECOND * 7 / 20); // 0.35s in ticks
    let mut i = 0;
    while i < words.len() {
        if timed(&words[i]) {
            i += 1;
            continue;
        }
        let run_end = (i..words.len()).find(|&k| timed(&words[k])).unwrap_or(words.len());
        let before = (0..i).rev().find(|&k| timed(&words[k])).and_then(|k| word_tick(&words[k], "end_tick", "end"));
        let after = words.get(run_end).and_then(|w| word_tick(w, "start_tick", "start"));
        let n = (run_end - i) as i64;
        let span = Tick(default_word_step.0 * n);
        let (from, to) = match (before, after) {
            (Some(a), Some(b)) if b > a => (a, b),
            (Some(a), _) => (a, a + span),
            (None, Some(b)) => (Tick((b.0 - span.0).max(0)), b),
            (None, None) => return,
        };
        let step = Tick((to.0 - from.0) / n.max(1));
        for (k, w) in words[i..run_end].iter_mut().enumerate() {
            let start = Tick(from.0 + step.0 * k as i64);
            let end = start + step;
            w["start_tick"] = json!(start.0);
            w["end_tick"] = json!(end.0);
            w["start"] = json!(start.seconds());
            w["end"] = json!(end.seconds());
            w["timing_source"] = json!("interpolated");
        }
        i = run_end;
    }
}

pub(crate) fn timing(anim: &mut Value) {
    let times: Vec<(Tick, Tick)> =
        anim["words"].as_array().into_iter().flatten().filter_map(|w| Some((word_tick(w, "start_tick", "start")?, word_tick(w, "end_tick", "end")?))).collect();
    if let (Some(first), Some(last)) = (times.iter().map(|t| t.0).min(), times.iter().map(|t| t.1).max()) {
        let lead_in = LEAD_IN_TICK.min(first);
        let anchor = first - lead_in;
        let duration = (last + HOLD_TICK) - anchor;
        anim["lead_in_tick"] = json!(lead_in.0);
        anim["hold_tick"] = json!(HOLD_TICK.0);
        anim["song_anchor_tick"] = json!(anchor.0);
        anim["authored_duration_tick"] = json!(duration.0);
        anim["lead_in"] = json!(lead_in.seconds());
        anim["hold"] = json!(HOLD_TICK.seconds());
        anim["song_anchor"] = json!(anchor.seconds());
        anim["authored_duration"] = json!(duration.seconds());
    }
}

/// The lyric design (`design/words.json`).
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub(crate) struct WordsDesign(pub Value);

impl crate::doc_state::ProjectDoc for WordsDesign {
    const KIND: &'static str = "words";
    fn path(dir: &Path) -> std::path::PathBuf {
        document_path(dir)
    }
}

pub(crate) fn load(s: &mut Session, dir: &Path) -> Result<Option<Value>> {
    Ok(s.doc::<WordsDesign>(dir)?.map(|d| d.0))
}

pub(crate) fn save(s: &mut Session, dir: &Path, doc: &Value) -> Result<()> {
    s.save_doc(dir, &WordsDesign(doc.clone()))
}

fn group_caption_lines(src: &[&Value]) -> Vec<[usize; 2]> {
    let mut groups = Vec::new();
    let mut start = 0;
    for k in 0..src.len() {
        let line_ends = k + 1 == src.len() || src[k + 1]["sung_id"] != src[k]["sung_id"] || src[k + 1]["occurrence"] != src[k]["occurrence"];
        if line_ends {
            let n = k + 1 - start;
            if n > 6 {
                groups.push([start, start + n.div_ceil(2) - 1]);
                groups.push([start + n.div_ceil(2), k]);
            } else {
                groups.push([start, k]);
            }
            start = k + 1;
        }
    }
    groups
}

pub(crate) fn printed_caption_words(aligned: &Value) -> (Vec<Value>, Vec<[usize; 2]>) {
    let src: Vec<&Value> = aligned["words"].as_array().into_iter().flatten().collect();
    let mut words: Vec<Value> = src
        .iter()
        .map(|w| {
            let st = word_tick(w, "start_tick", "start");
            let et = word_tick(w, "end_tick", "end");
            json!({
                "text": w["text"],
                "start_tick": st.map(|t| t.0),
                "end_tick": et.map(|t| t.0),
                "start": st.map(Tick::seconds),
                "end": et.map(Tick::seconds),
                "word_id": w["word_id"],
                "line": w["sung_id"],
                "section": w["section"],
                "occurrence": w["occurrence"]
            })
        })
        .collect();
    let step_300ms = TICKS_PER_SECOND * 3 / 10;
    let step_350ms = TICKS_PER_SECOND * 7 / 20;
    let mut i = 0;
    while i < words.len() {
        if words[i]["start_tick"].is_i64() {
            i += 1;
            continue;
        }
        let j = (i..words.len()).find(|&k| words[k]["start_tick"].is_i64()).unwrap_or(words.len());
        let count = (j - i) as i64;
        let before = if i > 0 { words[i - 1]["end_tick"].as_i64().unwrap_or(0) } else { 0 };
        let next = words.get(j).and_then(|w| w["start_tick"].as_i64()).unwrap_or(before + step_300ms * count);
        let prev = before.max(next - step_350ms * count);
        let step = (next - prev).max(0) / count.max(1);
        for (n, w) in words[i..j].iter_mut().enumerate() {
            let st = Tick(prev + step * n as i64);
            let et = Tick(prev + step * (n as i64 + 1));
            w["start_tick"] = json!(st.0);
            w["end_tick"] = json!(et.0);
            w["start"] = json!(st.seconds());
            w["end"] = json!(et.seconds());
        }
        i = j;
    }
    (words, group_caption_lines(&src))
}

fn build_caption_line_map(aligned: &Value, by_line: bool) -> std::collections::HashMap<u64, String> {
    aligned["words"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|w| {
            let key = w["ref_index"].as_u64()?;
            let val = if by_line { w["sung_id"].as_str()?.to_owned() } else { format!("{}#{}", w["section"].as_str()?, w["occurrence"].as_u64()?) };
            Some((key, val))
        })
        .collect()
}

fn format_srt_cues(cues: &[(String, Vec<&Value>)]) -> String {
    let clock = |t: Tick| {
        let ms = (t.0.max(0) as u128 * 1000 / TICKS_PER_SECOND as u128) as u64;
        format!("{:02}:{:02}:{:02},{:03}", ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000)
    };
    let min_dur = Tick(TICKS_PER_SECOND / 10);
    let mut srt = String::new();
    for (n, (_, words)) in cues.iter().enumerate() {
        let start = words.iter().filter_map(|w| word_tick(w, "start_tick", "start")).min().unwrap_or(Tick::ZERO);
        let end = words.iter().filter_map(|w| word_tick(w, "end_tick", "end")).max().unwrap_or(start);
        let text = words.iter().filter_map(|w| w["text"].as_str().or_else(|| w["word"].as_str())).collect::<Vec<_>>().join(" ");
        srt.push_str(&format!("{}\n{} --> {}\n{text}\n\n", n + 1, clock(start), clock(end.max(start + min_dur))));
    }
    srt
}

pub fn cmd_words_captions(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::cmd::words::workspace_dir(s)?;
    let aligned = aligned(s)?;
    let (_, _, heard) = reference(&dir)?;
    let by_line = match p["by"].as_str().unwrap_or("section") {
        "section" => false,
        "line" => true,
        other => return Err(invalid(format!("by must be section or line, not {other}"))),
    };
    let line_of = build_caption_line_map(&aligned, by_line);
    let mut cues: Vec<(String, Vec<&Value>)> = Vec::new();
    for (i, w) in heard.iter().enumerate() {
        let line = line_of.get(&(i as u64)).cloned().or_else(|| cues.last().map(|(id, _)| id.clone())).unwrap_or_default();
        if cues.last().is_none_or(|(id, _)| *id != line) {
            cues.push((line, Vec::new()));
        }
        if let Some((_, group)) = cues.last_mut() {
            group.push(w);
        }
    }
    let srt = format_srt_cues(&cues);
    let out = p["out"].as_str().map(PathBuf::from).unwrap_or_else(|| dir.join("lyrics.captions.srt"));
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    crate::doc_state::write_file(&out, &srt)?;
    Ok(json!({"ok":true,"path":out,"cues":cues.len(),"words":heard.len()}))
}
