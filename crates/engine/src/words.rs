//! Words: editable visible lyric animations built from the artist's canonical lines, timed by
//! the hidden transcript reference. The reference (heard words, their order and times) is never
//! edited or exported; mismatches against the artist's words are reported, never substituted.
//! Text and timing are not editable here: `words.save` changes only style, emphasis, motion and
//! each line's design (where its words sit, their lettering and moves, and drawn strokes).
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub use crate::words_compose::{
    cmd_words_apply, cmd_words_compose, cmd_words_design, cmd_words_effect_install, cmd_words_track,
};

pub(crate) const ACTIONS: &[&str] = &["none", "wave", "tiny", "swim", "no", "cry", "big", "float"];
pub(crate) const ENTRANCES: &[&str] = &["pop", "fade", "rise", "none"];
pub(crate) const EXITS: &[&str] = &["fade", "drop", "none"];
pub(crate) const MOVES: &[&str] = &[
    "quiet", "sleep", "settle", "fall", "pop", "rise", "east", "west", "loop",
];
pub(crate) const FONTS: &[&str] = &["song", "serif", "sans"];
const LEAD_IN: f64 = 0.4;
pub(crate) const HOLD: f64 = 1.0;
pub(crate) const KINETIC_LIBRARY: &str =
    include_str!("../../../pipeline/templates/kinetic-library.json");

pub(crate) fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

fn norm(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

pub(crate) fn document_path(dir: &Path) -> PathBuf {
    dir.join("design/words.json")
}

pub(crate) fn reference(dir: &Path) -> Result<(PathBuf, String, Vec<Value>)> {
    let path = crate::workshop::transcript_files(dir)
        .into_iter()
        .next()
        .ok_or_else(|| invalid("No transcript in this project; the timing reference is missing"))?;
    let bytes = fs::read(&path)?;
    let doc: Value = serde_json::from_slice(&bytes).map_err(|e| invalid(e.to_string()))?;
    let words = doc
        .as_array()
        .or_else(|| doc["words"].as_array())
        .cloned()
        .unwrap_or_default();
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
            let sung = line["sung_id"]
                .as_str()
                .or(line["id"].as_str())
                .unwrap_or_default();
            let timed = line["words"].as_array().cloned().unwrap_or_default();
            let texts: Vec<String> = if timed.is_empty() {
                line["text"]
                    .as_str()
                    .unwrap_or_default()
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect()
            } else {
                timed
                    .iter()
                    .filter_map(|w| w["text"].as_str().map(str::to_owned))
                    .collect()
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
            (Some(j), "match") => (heard[*j]["start"].clone(), heard[*j]["end"].clone(), "reference"),
            _ => match word["forced"].as_array() {
                Some(f) => (f[0].clone(), f[1].clone(), "forced_alignment"),
                None => (Value::Null, Value::Null, "none"),
            },
        };
        word["ref_index"] = json!(found);
        word["heard"] = found.map_or(Value::Null, |j| heard[j]["text"].clone());
        word["status"] = json!(status);
        word["start"] = start;
        word["end"] = end;
        word["timing_source"] = json!(source);
    }
}

pub(crate) fn aligned(s: &mut Session) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let lyrics = crate::lyrics::cmd_song_lyrics(s, &json!({}))?;
    if lyrics["source"] != "canonical" {
        return Err(invalid(
            "No artist lyric sheet; Words needs the artist's canonical lines",
        ));
    }
    let (ref_path, ref_sha, heard) = reference(&dir)?;
    let mut words = collect_canonical_words(&lyrics);
    let canonical: Vec<String> = words
        .iter()
        .map(|w| norm(w["text"].as_str().unwrap_or_default()))
        .collect();
    let heard_norm: Vec<String> = heard
        .iter()
        .map(|w| norm(w["text"].as_str().unwrap_or_default()))
        .collect();
    let map = align(&canonical, &heard_norm);
    attach_reference_timing(&mut words, &heard, &heard_norm, &map);
    let mismatches: Vec<Value> = words
        .iter()
        .filter(|w| w["status"] != "match")
        .map(|w| json!({"word_id":w["word_id"],"line_id":w["line_id"],"canonical":w["text"],"heard":w["heard"],"kind":w["status"]}))
        .collect();
    let disagreements: Vec<Value> = words
        .iter()
        .filter(|w| w["timing_source"] == "reference")
        .filter_map(|w| {
            let forced = w["forced"].as_array()?;
            let (rs, fs) = (w["start"].as_f64()?, forced[0].as_f64()?);
            ((rs - fs).abs() > 0.5).then(|| json!({"word_id":w["word_id"],"text":w["text"],"reference":[w["start"], w["end"]],"measured":forced}))
        })
        .collect();
    for word in &mut words {
        word.as_object_mut()
            .ok_or_else(|| invalid("Aligned word is not an object"))?
            .remove("forced");
    }
    let heard_used: Vec<usize> = map.iter().flatten().copied().collect();
    let unmatched_heard: Vec<Value> = (0..heard.len())
        .filter(|j| !heard_used.contains(j))
        .map(|j| json!({"ref_index":j,"heard":heard[j]["text"]}))
        .collect();
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
    let matched = report["words"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|w| w["status"] == "match")
        .count();
    report["summary"] = json!({
        "canonical_words": total,
        "matched": matched,
        "mismatched": total - matched,
        "timing_disagreements": report["timing_disagreements"].as_array().map_or(0, Vec::len)
    });
    Ok(report)
}

pub(crate) fn tag_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!(" {name}=\"");
    let at = tag.find(&needle)? + needle.len();
    tag[at..].split('"').next()
}

type RawClip = (String, String, f64, f64, Option<String>, Option<u64>);

fn strip_html_tags(inner: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for c in inner.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    text.trim().to_owned()
}

fn parse_lyric_word_clips(html: &str) -> Vec<RawClip> {
    let mut clips = Vec::new();
    let mut from = 0;
    while let Some(open) = html[from..].find('<').map(|i| i + from) {
        let close = html[open..].find('>').map_or(html.len(), |i| i + open + 1);
        let tag = &html[open..close];
        from = close;
        if !tag_attr(tag, "class").is_some_and(|c| c.split_whitespace().any(|c| c == "lyric-word-clip")) {
            continue;
        }
        let (Some(id), Some(start), Some(duration)) = (
            tag_attr(tag, "id"),
            tag_attr(tag, "data-start").and_then(|v| v.parse::<f64>().ok()),
            tag_attr(tag, "data-duration").and_then(|v| v.parse::<f64>().ok()),
        ) else {
            continue;
        };
        let inner = html[close..].split("</div>").next().unwrap_or_default();
        clips.push((
            id.to_owned(),
            strip_html_tags(inner),
            start,
            duration,
            tag_attr(tag, "data-section").map(str::to_owned),
            tag_attr(tag, "data-occurrence").and_then(|v| v.parse::<u64>().ok()),
        ));
    }
    clips
}

fn match_line_clips(
    clips: &[RawClip],
    used: &mut [bool],
    section: &Value,
    line: &Value,
) -> Vec<Value> {
    let sung = line["sung_id"]
        .as_str()
        .or(line["id"].as_str())
        .unwrap_or_default();
    if sung.is_empty() {
        return Vec::new();
    }
    let mut by_word: std::collections::BTreeMap<usize, Vec<usize>> = std::collections::BTreeMap::new();
    for (i, (id, _, _, _, scope, occurrence)) in clips.iter().enumerate() {
        let Some(n) = id
            .rsplit_once('-')
            .filter(|(head, _)| *head == sung || head.ends_with(&format!("-{sung}")))
            .and_then(|(_, n)| n.parse::<usize>().ok())
        else {
            continue;
        };
        if scope.as_deref().is_some_and(|v| Some(v) != section["section"].as_str())
            || occurrence.is_some_and(|v| Some(v) != section["occurrence"].as_u64())
        {
            continue;
        }
        by_word.entry(n).or_default().push(i);
    }
    let repeat = section["occurrence"].as_u64().unwrap_or(1).saturating_sub(1) as usize;
    let mut found = Vec::new();
    for (n, mut candidates) in by_word {
        candidates.sort_by(|a, b| clips[*a].2.total_cmp(&clips[*b].2));
        let pick = if candidates.len() == 1 || clips[candidates[0]].5.is_some() { 0 } else { repeat };
        if let Some(&i) = candidates.get(pick) {
            let (id, text, start, duration, _, _) = &clips[i];
            used[i] = true;
            found.push(json!({"clip_id":id,"word_id":format!("{sung}/w{n}"),"line_id":line["id"],"text":text,"start":start,"duration":duration}));
        }
    }
    found
}

fn trim_legacy_repeats(sections: &mut [Value]) {
    for i in 0..sections.len() {
        let next = sections
            .get(i + 1)
            .and_then(|sec| sec["start"].as_f64())
            .unwrap_or(f64::INFINITY);
        let snapshot = sections[i]["clips"].as_array().cloned().unwrap_or_default();
        for clip in sections[i]["clips"].as_array_mut().into_iter().flatten() {
            let start = clip["start"].as_f64().unwrap_or(0.0);
            let boundary = snapshot
                .iter()
                .filter(|c| c["line_id"] != clip["line_id"])
                .filter_map(|c| c["start"].as_f64())
                .filter(|t| *t > start)
                .fold(next, f64::min);
            clip["duration"] = json!(clip["duration"].as_f64().unwrap_or(0.0).min((boundary - start).max(0.0)));
        }
        sections[i]["end"] = json!(
            sections[i]["clips"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| Some(c["start"].as_f64()? + c["duration"].as_f64()?))
                .fold(0.0, f64::max)
        );
    }
}

fn detect_design_overlaps(html: &str, sections: &[Value]) -> Vec<Value> {
    let mut placed: std::collections::HashMap<String, (f64, f64)> = std::collections::HashMap::new();
    let mut from = 0;
    while let Some(open) = html[from..].find('<').map(|i| i + from) {
        let close = html[open..].find('>').map_or(html.len(), |i| i + open + 1);
        let tag = &html[open..close];
        from = close;
        if !tag_attr(tag, "class").is_some_and(|c| c.split_whitespace().any(|c| c == "lyric-design-clip")) {
            continue;
        }
        if let (Some(id), Some(s), Some(d)) = (
            tag_attr(tag, "data-magicstudio-words").or_else(|| tag_attr(tag, "id")),
            tag_attr(tag, "data-start").and_then(|v| v.parse::<f64>().ok()),
            tag_attr(tag, "data-duration").and_then(|v| v.parse::<f64>().ok()),
        ) {
            placed.insert(id.to_owned(), (s, s + d));
        }
    }
    let mut overlaps = Vec::new();
    for i in 0..sections.len().saturating_sub(1) {
        let cur_id = format!(
            "words-{}-{}",
            sections[i]["section"].as_str().unwrap_or("lyrics").replace("_", "-"),
            sections[i]["occurrence"].as_u64().unwrap_or(1)
        );
        let next_id = format!(
            "words-{}-{}",
            sections[i + 1]["section"].as_str().unwrap_or("lyrics").replace("_", "-"),
            sections[i + 1]["occurrence"].as_u64().unwrap_or(1)
        );
        let cur_end = placed
            .get(&cur_id)
            .map(|(_, e)| *e)
            .or_else(|| sections[i]["end"].as_f64())
            .unwrap_or(0.0);
        let next_start = sections[i + 1]["start"].as_f64().unwrap_or(f64::INFINITY);
        if cur_end > next_start + 0.05 {
            overlaps.push(json!({
                "first_section": cur_id,
                "second_section": next_id,
                "start": (next_start * 100.0).round() / 100.0,
                "end": (cur_end * 100.0).round() / 100.0,
                "at": (((next_start + cur_end) * 0.5) * 100.0).round() / 100.0
            }));
        }
    }
    overlaps
}

pub fn cmd_words_studio_track(s: &mut Session, _: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let path = dir.join("hyperframes/index.html");
    let html = fs::read_to_string(&path)
        .map_err(|_| invalid("This project has no HyperFrames video yet; run video.init first"))?;
    let clips = parse_lyric_word_clips(&html);
    let lyrics = crate::lyrics::cmd_song_lyrics(s, &json!({}))?;
    let mut used = vec![false; clips.len()];
    let mut sections = Vec::new();
    for section in lyrics["sections"].as_array().into_iter().flatten() {
        let mut found: Vec<Value> = section["lines"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|line| match_line_clips(&clips, &mut used, section, line))
            .collect();
        if found.is_empty() {
            continue;
        }
        found.sort_by(|a, b| {
            a["start"]
                .as_f64()
                .partial_cmp(&b["start"].as_f64())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let start = found.iter().filter_map(|c| c["start"].as_f64()).fold(f64::INFINITY, f64::min);
        let end = found
            .iter()
            .filter_map(|c| Some(c["start"].as_f64()? + c["duration"].as_f64()?))
            .fold(0.0, f64::max);
        sections.push(json!({"section":section["section"],"label":section["label"],"occurrence":section["occurrence"],"start":start,"end":end,"clips":found}));
    }
    let overlaps = detect_design_overlaps(&html, &sections);
    trim_legacy_repeats(&mut sections);
    let unmatched: Vec<&str> = clips
        .iter()
        .zip(&used)
        .filter(|(_, u)| !**u)
        .map(|((id, ..), _)| id.as_str())
        .collect();
    Ok(json!({"source":path,"clip_count":clips.len(),"sections":sections,"overlaps":overlaps,"unmatched":unmatched}))
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

pub use crate::words_stage::{
    cmd_words_effects, cmd_words_link, cmd_words_lyrics_view, cmd_words_read, cmd_words_save,
};
pub(crate) use crate::words_stage::one_of;

pub(crate) fn with_sung_sections(mut doc: Value, report: &Value) -> Result<Value> {
    let fresh = default_document(report)?["animations"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let ids: Vec<Value> = fresh.iter().map(|a| a["animation_id"].clone()).collect();
    if let Some(list) = doc["animations"].as_array_mut() {
        for (k, anim) in fresh.into_iter().enumerate() {
            if list.iter().any(|a| a["animation_id"] == anim["animation_id"]) {
                continue;
            }
            let at = list
                .iter()
                .position(|a| {
                    ids.iter()
                        .position(|id| *id == a["animation_id"])
                        .is_some_and(|j| j > k)
                })
                .unwrap_or(list.len());
            list.insert(at, anim);
        }
    }
    Ok(doc)
}

pub(crate) fn default_document(report: &Value) -> Result<Value> {
    let mut animations: Vec<Value> = Vec::new();
    for w in report["words"].as_array().into_iter().flatten() {
        let id = format!(
            "words-{}-{}",
            w["section"].as_str().unwrap_or("lyrics").replace("_", "-"),
            w["occurrence"].as_u64().unwrap_or(1)
        );
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
        let anim = animations
            .last_mut()
            .ok_or_else(|| invalid("Missing generated animation"))?;
        for (key, field) in [("line_ids", "line_id"), ("sung_ids", "sung_id")] {
            let scope = anim["scope"][key]
                .as_array_mut()
                .ok_or_else(|| invalid("Generated animation scope is not an array"))?;
            if !scope.contains(&w[field]) {
                scope.push(w[field].clone());
            }
        }
        anim["words"]
            .as_array_mut()
            .ok_or_else(|| invalid("Generated animation words are not an array"))?
            .push(json!({
                "word_id": w["word_id"],
                "text": w["text"],
                "line_id": w["line_id"],
                "sung_id": w["sung_id"],
                "ref_index": w["ref_index"],
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
        "format": "magicstudio.words",
        "version": 1,
        "reference": report["reference"],
        "canonical": report["canonical"],
        "mismatches": report["mismatches"],
        "animations": animations
    }))
}

pub(crate) fn word_time_key(scope: &Value, id: &str) -> String {
    format!(
        "{}#{}:{id}",
        scope["section"].as_str().unwrap_or(""),
        scope["occurrence"].as_u64().unwrap_or(1)
    )
}

pub(crate) fn studio_times(s: &mut Session) -> std::collections::HashMap<String, (f64, f64)> {
    let track = cmd_words_studio_track(s, &json!({})).unwrap_or(Value::Null);
    track["sections"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|sec| {
            sec["clips"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(move |c| {
                    Some((
                        word_time_key(sec, c["word_id"].as_str()?),
                        (
                            c["start"].as_f64()?,
                            c["start"].as_f64()? + c["duration"].as_f64()?,
                        ),
                    ))
                })
        })
        .collect()
}

pub(crate) fn fill_untimed(anim: &mut Value) {
    let Some(words) = anim["words"].as_array_mut() else {
        return;
    };
    let timed = |w: &Value| w["start"].is_number() && w["end"].is_number();
    let mut i = 0;
    while i < words.len() {
        if timed(&words[i]) {
            i += 1;
            continue;
        }
        let run_end = (i..words.len()).find(|&k| timed(&words[k])).unwrap_or(words.len());
        let before = (0..i)
            .rev()
            .find(|&k| timed(&words[k]))
            .and_then(|k| words[k]["end"].as_f64());
        let after = words.get(run_end).and_then(|w| w["start"].as_f64());
        let n = (run_end - i) as f64;
        let (from, to) = match (before, after) {
            (Some(a), Some(b)) if b > a => (a, b),
            (Some(a), _) => (a, a + 0.35 * n),
            (None, Some(b)) => ((b - 0.35 * n).max(0.0), b),
            (None, None) => return,
        };
        let step = (to - from) / n;
        for (k, w) in words[i..run_end].iter_mut().enumerate() {
            let start = from + step * k as f64;
            w["start"] = json!((start * 1e6).round() / 1e6);
            w["end"] = json!(((start + step) * 1e6).round() / 1e6);
            w["timing_source"] = json!("interpolated");
        }
        i = run_end;
    }
}

pub(crate) fn timing(anim: &mut Value) {
    let times: Vec<(f64, f64)> = anim["words"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|w| Some((w["start"].as_f64()?, w["end"].as_f64()?)))
        .collect();
    if let (Some(first), Some(last)) = (
        times.iter().map(|t| t.0).reduce(f64::min),
        times.iter().map(|t| t.1).reduce(f64::max),
    ) {
        let lead_in = LEAD_IN.min(first);
        anim["lead_in"] = json!(lead_in);
        anim["hold"] = json!(HOLD);
        anim["song_anchor"] = json!(first - lead_in);
        anim["authored_duration"] = json!((last + HOLD) - (first - lead_in));
    }
}

pub(crate) fn load(dir: &Path) -> Result<Option<Value>> {
    match fs::read(document_path(dir)) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes).map_err(|e| {
            invalid(format!("design/words.json is not valid JSON: {e}"))
        })?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn group_caption_lines(src: &[&Value]) -> Vec<[usize; 2]> {
    let mut groups = Vec::new();
    let mut start = 0;
    for k in 0..src.len() {
        let line_ends = k + 1 == src.len()
            || src[k + 1]["sung_id"] != src[k]["sung_id"]
            || src[k + 1]["occurrence"] != src[k]["occurrence"];
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
            json!({
                "text": w["text"],
                "start": w["start"],
                "end": w["end"],
                "word_id": w["word_id"],
                "line": w["sung_id"],
                "section": w["section"],
                "occurrence": w["occurrence"]
            })
        })
        .collect();
    let mut i = 0;
    while i < words.len() {
        if words[i]["start"].is_f64() {
            i += 1;
            continue;
        }
        let j = (i..words.len())
            .find(|&k| words[k]["start"].is_f64())
            .unwrap_or(words.len());
        let before = if i > 0 {
            words[i - 1]["end"].as_f64().unwrap_or(0.0)
        } else {
            0.0
        };
        let next = words
            .get(j)
            .and_then(|w| w["start"].as_f64())
            .unwrap_or(before + 0.3 * (j - i) as f64);
        let prev = before.max(next - 0.35 * (j - i) as f64);
        let step = (next - prev).max(0.0) / (j - i) as f64;
        for (n, w) in words[i..j].iter_mut().enumerate() {
            w["start"] = json!(prev + step * n as f64);
            w["end"] = json!(prev + step * (n + 1) as f64);
        }
        i = j;
    }
    (words, group_caption_lines(&src))
}

pub(crate) fn captions_to_video(s: &Session, dir: &Path, aligned: &Value) -> Result<Value> {
    let index = dir.join("hyperframes/index.html");
    let host = fs::read_to_string(&index).unwrap_or_default();
    let designed: std::collections::BTreeSet<&str> = host
        .split("data-magicstudio-words=\"")
        .skip(1)
        .filter_map(|r| r.split("\"").next())
        .collect();
    let open: Vec<Value> = aligned["words"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|w| {
            let id = format!(
                "words-{}-{}",
                w["section"].as_str().unwrap_or("lyrics").replace("_", "-"),
                w["occurrence"].as_u64().unwrap_or(1)
            );
            !designed.contains(id.as_str())
        })
        .cloned()
        .collect();
    let (words, groups) = printed_caption_words(&json!({"words": open}));
    let comp = dir.join("hyperframes/compositions");
    let dest = comp.join("captions.html");
    if words.is_empty() {
        let marker = "data-composition-src=\"compositions/captions.html\"";
        let kept: Vec<&str> = host.lines().filter(|l| !l.contains(marker)).collect();
        if kept.len() != host.lines().count() {
            fs::write(&index, kept.join("\n") + "\n")?;
        }
        return Ok(json!({"ok":true,"caption_track":"removed: every section is designed","source":dest,"designed":designed}));
    }
    let kept = if dest.is_file() {
        let version = (1..)
            .map(|n| comp.join(format!("captions.v{n}.html")))
            .find(|p| !p.exists())
            .ok_or_else(|| invalid("No available caption version"))?;
        fs::copy(&dest, &version)?;
        Some(version)
    } else {
        None
    };
    let duration = crate::video_target::probe_audio_duration(s.song_audio.as_deref()).max(
        words
            .iter()
            .filter_map(|w| w["end"].as_f64())
            .fold(0.0, f64::max),
    );
    crate::video_target::write_caption_source(&dest, &words, &groups, duration)?;
    let digest = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&words).map_err(|e| invalid(e.to_string()))?)
    );
    let receipt = dir.join("hyperframes/.magicstudio-captions.json");
    fs::write(
        &receipt,
        serde_json::to_vec_pretty(&json!({
            "source_kind": "artist_lines",
            "input_sha256": digest,
            "word_count": words.len(),
            "groups": groups.len(),
            "duration": duration,
            "previous_source": kept,
            "grouping": "one group per printed line; long lines in two halves"
        }))
        .map_err(|e| invalid(e.to_string()))?,
    )?;
    Ok(json!({
        "ok": true,
        "source": dest,
        "previous_source": kept,
        "provenance": receipt,
        "words": words.len(),
        "groups": groups.len(),
        "first": words.first(),
        "last": words.last(),
        "next": "hyperframes snapshot <project>/hyperframes --at <t> to check, then render"
    }))
}

fn build_caption_line_map(aligned: &Value, by_line: bool) -> std::collections::HashMap<u64, String> {
    aligned["words"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|w| {
            let key = w["ref_index"].as_u64()?;
            let val = if by_line {
                w["sung_id"].as_str()?.to_owned()
            } else {
                format!("{}#{}", w["section"].as_str()?, w["occurrence"].as_u64()?)
            };
            Some((key, val))
        })
        .collect()
}

fn format_srt_cues(cues: &[(String, Vec<&Value>)]) -> String {
    let clock = |t: f64| {
        let ms = (t * 1000.0).round() as u64;
        format!(
            "{:02}:{:02}:{:02},{:03}",
            ms / 3_600_000,
            ms / 60_000 % 60,
            ms / 1000 % 60,
            ms % 1000
        )
    };
    let mut srt = String::new();
    for (n, (_, words)) in cues.iter().enumerate() {
        let start = words.iter().filter_map(|w| w["start"].as_f64()).reduce(f64::min).unwrap_or(0.0);
        let end = words.iter().filter_map(|w| w["end"].as_f64()).reduce(f64::max).unwrap_or(start);
        let text = words
            .iter()
            .filter_map(|w| w["text"].as_str().or_else(|| w["word"].as_str()))
            .collect::<Vec<_>>()
            .join(" ");
        srt.push_str(&format!(
            "{}\n{} --> {}\n{text}\n\n",
            n + 1,
            clock(start),
            clock(end.max(start + 0.1))
        ));
    }
    srt
}

pub fn cmd_words_captions(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
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
        let line = line_of
            .get(&(i as u64))
            .cloned()
            .or_else(|| cues.last().map(|(id, _)| id.clone()))
            .unwrap_or_default();
        if cues.last().is_none_or(|(id, _)| *id != line) {
            cues.push((line, Vec::new()));
        }
        if let Some((_, group)) = cues.last_mut() {
            group.push(w);
        }
    }
    if p["to"] == "video" {
        return captions_to_video(s, &dir, &aligned);
    }
    let srt = format_srt_cues(&cues);
    let out = p["out"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| dir.join("lyrics.captions.srt"));
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&out, &srt)?;
    Ok(json!({"ok":true,"path":out,"cues":cues.len(),"words":heard.len()}))
}

