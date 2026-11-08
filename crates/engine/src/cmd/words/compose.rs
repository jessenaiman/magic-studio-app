//! Words composition and EffectCraft/FilmCraft placement (`words.compose`, `words.apply`, `words.design`, `words.track`).
use crate::time::{TICKS_PER_SECOND, Tick};
use crate::{Result, Session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub use super::validate::cmd_words_effect_install;
pub(crate) use super::validate::{kinetic_design, line_design, text_effect};
use super::{
    HOLD_TICK, aligned, cmd_words_read, cmd_words_save, cmd_words_studio_track, fill_untimed, invalid, printed_caption_words, studio_times, timing, word_tick,
    word_time_key,
};

fn build_kinetic_for_design(a: &Value, recipe_doc: &Value, p: &Value, k: usize) -> Value {
    let kin = &recipe_doc["kinetic"];
    let pick = |list: &Value, key: &str| {
        p[key]
            .as_str()
            .map(|v| json!(v))
            .or_else(|| list.as_array().filter(|l| !l.is_empty()).map(|l| l[(p["seed"].as_u64().unwrap_or(0) as usize + k * 7) % l.len()].clone()))
    };
    let acts: serde_json::Map<String, Value> = a["words"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|w| {
            let key: String = w["text"].as_str()?.chars().filter(|c| c.is_alphabetic()).flat_map(char::to_lowercase).collect();
            kin["acts"][&key].as_str().map(|act| (w["word_id"].as_str().unwrap_or_default().to_owned(), json!(act)))
        })
        .collect();
    json!({
        "layout": pick(&kin["layouts"], "layout").unwrap_or(json!("karaoke")),
        "entrance": pick(&kin["entrances"], "entrance").unwrap_or(json!("fade-up")),
        "overlay": pick(&kin["overlays"], "overlay").unwrap_or(json!("none")),
        "acts": acts,
        "band": recipe_doc["band"],
        "seed": p["seed"].as_u64().unwrap_or(1) + k as u64,
        "style": kin["style"]
    })
}

fn run_lyrics_layout(dir: &Path, recipe_doc: &Value, doc: &Value, times: &std::collections::HashMap<String, (Tick, Tick)>) -> Result<Value> {
    let animations: Vec<Value> = doc["animations"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|a| {
            let words: Vec<Value> = a["words"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|w| {
                    let mut w = w.clone();
                    if let Some(t) = w["word_id"].as_str().and_then(|id| times.get(&word_time_key(&a["scope"], id))) {
                        w["start_tick"] = json!(t.0.0);
                        w["start"] = json!(t.0.seconds());
                    }
                    w
                })
                .collect();
            json!({"animation_id":a["animation_id"],"section":a["scope"]["section"],"words":words})
        })
        .collect();
    let input = dir.join("design/words-design-input.json");
    if let Some(parent) = input.parent() {
        fs::create_dir_all(parent)?;
    }
    crate::doc_state::write_file(&input, serde_json::to_vec(&json!({"animations":animations})).map_err(|e| invalid(e.to_string()))?)?;
    Ok(crate::cmd::words::lyrics_layout::designs(recipe_doc, &animations))
}

fn design_one_section(s: &mut Session, a: &Value, k: usize, style: &Value, designs: &Value, recipe_doc: &Value, p: &Value) -> Result<Value> {
    let id = a["animation_id"].as_str().unwrap_or_default();
    let mut save = json!({
        "animation_id": id,
        "expected_source_revision": a["source_revision"],
        "origin": "words.design",
        "style": style,
        "design": designs[id]
    });
    if recipe_doc["kinetic"].is_object() && p["kinetic"] != false {
        save["kinetic"] = build_kinetic_for_design(a, recipe_doc, p, k);
    }
    let saved = cmd_words_save(s, &save)?;
    let applied = if p["apply"] == false { Value::Null } else { cmd_words_apply(s, &json!({"animation_id":id}))?["in_video"].clone() };
    Ok(json!({
        "animation_id": id,
        "lines": designs[id].as_object().map_or(0, |m| m.len()),
        "revision": saved["animation"]["source_revision"],
        "in_video": applied,
        "kinetic": saved["animation"]["kinetic"].as_object().map(|k| json!({
            "layout": k["layout"],
            "entrance": k["entrance"],
            "overlay": k["overlay"],
            "acts": k["acts"].as_object().map_or(0, |a| a.len())
        }))
    }))
}

/// `words.design`: every sung line designed from the project's lyric recipe (`lyrics-design.json`).
pub fn cmd_words_design(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::cmd::words::workspace_dir(s)?;
    let recipe = p["recipe"].as_str().map(PathBuf::from).unwrap_or_else(|| dir.join("lyrics-design.json"));
    if !recipe.is_file() {
        return Err(invalid(format!("No lyric recipe at {}; write one first", recipe.display())));
    }
    let recipe_doc: Value = serde_json::from_slice(&fs::read(&recipe)?).map_err(|e| invalid(format!("lyric recipe: {e}")))?;
    let doc = cmd_words_read(s, &json!({}))?["document"].clone();
    let times = studio_times(s);
    let designs = run_lyrics_layout(&dir, &recipe_doc, &doc, &times)?;
    let style = recipe_doc["font"].as_str().map(|f| json!({"font": f})).unwrap_or(json!({}));
    let only = p["section"].as_str();
    let mut sections = Vec::new();
    for (k, a) in doc["animations"].as_array().into_iter().flatten().enumerate() {
        let id = a["animation_id"].as_str().unwrap_or_default();
        if only.is_some_and(|o| o != id) {
            continue;
        }
        sections.push(design_one_section(s, a, k, &style, &designs, &recipe_doc, p)?);
    }
    Ok(json!({
        "ok": true,
        "recipe": recipe,
        "sections": sections,
        "lines": sections.iter().filter_map(|x| x["lines"].as_u64()).sum::<u64>(),
        "next": "run words.letter to build lettering/lyrics.ecproj"
    }))
}

/// `words.track`: write the song's lyric timing track (`design/words-track.json`), one clip per printed word in `Tick`s.
pub fn cmd_words_track(s: &mut Session, _: &Value) -> Result<Value> {
    let dir = crate::cmd::words::workspace_dir(s)?;
    let (words, _) = printed_caption_words(&aligned(s)?);
    if words.is_empty() {
        return Err(invalid("No printed words with timing; check words.align"));
    }
    let mut line_end: std::collections::HashMap<String, Tick> = std::collections::HashMap::new();
    for w in &words {
        let end = word_tick(w, "end_tick", "end").unwrap_or(Tick::ZERO);
        let e = line_end.entry(word_time_key(w, w["line"].as_str().unwrap_or_default())).or_insert(Tick::ZERO);
        if end > *e {
            *e = end;
        }
    }
    let min_dur = Tick(TICKS_PER_SECOND / 10);
    let mut clips = Vec::with_capacity(words.len());
    for (i, w) in words.iter().enumerate() {
        let id = w["word_id"].as_str().unwrap_or_default();
        let text = w["text"].as_str().unwrap_or_default();
        let start = word_tick(w, "start_tick", "start").unwrap_or(Tick::ZERO);
        let end = line_end.get(&word_time_key(w, w["line"].as_str().unwrap_or_default())).copied().unwrap_or(start) + HOLD_TICK;
        let dur = Tick((end.0 - start.0).max(min_dur.0));
        clips.push(json!({
            "clip_id": format!("kinetic-r{}-{}", w["occurrence"].as_u64().unwrap_or(1), id.replace("/w", "-")),
            "word_id": id,
            "line": w["line"],
            "section": w["section"],
            "occurrence": w["occurrence"],
            "text": text,
            "track_index": 200 + i,
            "start_tick": start.0,
            "end_tick": (start + dur).0,
            "duration_tick": dur.0,
            "start": start.seconds(),
            "end": (start + dur).seconds(),
            "duration": dur.seconds()
        }));
    }
    let path = dir.join("design/words-track.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_vec_pretty(&json!({"format": "magic.words.track", "version": 1, "clips": clips})).map_err(|e| invalid(e.to_string()))?;
    crate::doc_state::write_file(&path, payload)?;
    Ok(json!({
        "ok": true,
        "clips": words.len(),
        "first": words.first(),
        "last": words.last(),
        "path": path,
        "next": "words.design builds the lyric visuals on these anchors"
    }))
}

pub fn cmd_words_apply(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::cmd::words::workspace_dir(s)?;
    let id = p["animation_id"]
        .as_str()
        .filter(|i| i.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
        .ok_or_else(|| invalid("animation_id is required (letters, digits, '-' or '_')"))?;
    let use_it = p["use"].as_bool().unwrap_or(true);
    let mut receipt = json!({"ok":true,"animation_id":id,"in_video":use_it,"changed":true});
    if use_it {
        let composed = cmd_words_compose(s, &json!({"animation_id":id}))?;
        receipt["src"] = composed["src"].clone();
        receipt["start_tick"] = composed["song_anchor_tick"].clone();
        receipt["start"] = composed["song_anchor"].clone();
        receipt["drawings"] = composed["drawings"].clone();
    }
    let applied_file = dir.join("design/words-applied.json");
    let mut applied: Vec<String> = fs::read(&applied_file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    if use_it {
        if !applied.iter().any(|x| x == id) {
            applied.push(id.to_owned());
        }
    } else {
        applied.retain(|x| x != id);
    }
    if let Some(parent) = applied_file.parent() {
        fs::create_dir_all(parent)?;
    }
    crate::doc_state::write_file(&applied_file, serde_json::to_vec_pretty(&applied).map_err(|e| invalid(e.to_string()))?)?;
    Ok(receipt)
}

fn sync_studio_word_times(s: &mut Session, synced: &mut Value) {
    let track = cmd_words_studio_track(s, &json!({})).unwrap_or(Value::Null);
    let times = studio_times(s);
    let scope = synced["scope"].clone();
    for w in synced["words"].as_array_mut().into_iter().flatten() {
        if let Some((a, b)) = w["word_id"].as_str().and_then(|wid| times.get(&word_time_key(&scope, wid))) {
            w["start_tick"] = json!(a.0);
            w["end_tick"] = json!(b.0);
            w["start"] = json!(a.seconds());
            w["end"] = json!(b.seconds());
            w["timing_source"] = json!("studio");
        }
    }
    fill_untimed(synced);
    timing(synced);
    let next_start = track["sections"].as_array().and_then(|secs| {
        let pos = secs.iter().position(|sec| sec["section"] == scope["section"] && sec["occurrence"] == scope["occurrence"])?;
        secs.get(pos + 1)?["start_tick"].as_i64().map(Tick)
    });
    if let (Some(anchor), Some(ns)) = (synced["song_anchor_tick"].as_i64().map(Tick), next_start) {
        let cur_dur = synced["authored_duration_tick"].as_i64().map(Tick).unwrap_or(Tick::ZERO);
        let gap = Tick(TICKS_PER_SECOND / 25); // 40ms
        let min_dur = Tick(TICKS_PER_SECOND / 5); // 200ms
        let clamped = cur_dur.min(Tick((ns.0 - gap.0 - anchor.0).max(min_dur.0)));
        synced["authored_duration_tick"] = json!(clamped.0);
        synced["authored_duration"] = json!(clamped.seconds());
    }
}

fn build_local_words(anim: &Value, anchor: Tick) -> Result<Vec<Value>> {
    anim["words"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|w| {
            let start = word_tick(w, "start_tick", "start").ok_or_else(|| invalid("Word has no numeric start"))?;
            let end = word_tick(w, "end_tick", "end").ok_or_else(|| invalid("Word has no numeric end"))?;
            let local_start = start - anchor;
            let local_end = end - anchor;
            Ok(json!({
                "word_id": w["word_id"],
                "line_id": w["line_id"],
                "text": w["text"],
                "start_tick": local_start.0,
                "end_tick": local_end.0,
                "start": local_start.seconds(),
                "end": local_end.seconds(),
                "emphasis": w["emphasis"],
                "action": w["action"],
                "entrance": w["entrance"],
                "exit": w["exit"]
            }))
        })
        .collect()
}

struct WordsComposeInputs {
    id: String,
    instance: Option<String>,
    composition_id: String,
}

fn validate_words_compose_inputs(p: &Value, anim: &Value) -> Result<WordsComposeInputs> {
    let id = p["animation_id"].as_str().ok_or_else(|| invalid("animation_id is required"))?.to_owned();
    let instance = match p["instance_id"].as_str() {
        Some(i) if !i.is_empty() && i.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == 95) => Some(i.to_owned()),
        Some(_) => return Err(invalid("instance_id must be letters, digits, '-' or '_'")),
        None => None,
    };
    let composition_id = instance.as_deref().map_or_else(|| id.clone(), |i| format!("{id}--{i}"));
    let untimed: Vec<&str> = anim["words"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|w| word_tick(w, "start_tick", "start").is_none() || word_tick(w, "end_tick", "end").is_none())
        .filter_map(|w| w["word_id"].as_str())
        .collect();
    if !untimed.is_empty() {
        return Err(invalid(format!("Words without timing cannot be placed: {}; resolve them with words.align", untimed.join(", "))));
    }
    Ok(WordsComposeInputs { id, instance, composition_id })
}

/// `words.compose`: write the derived section composition spec (`lettering/sections/<id>.json`) in `Tick`s.
pub fn cmd_words_compose(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::cmd::words::workspace_dir(s)?;
    let id = p["animation_id"].as_str().ok_or_else(|| invalid("animation_id is required"))?;
    let read = cmd_words_read(s, &json!({"animation_id":id}))?;
    let mut synced = read["document"]["animations"][0].clone();
    sync_studio_word_times(s, &mut synced);
    let anim = &synced;
    let inputs = validate_words_compose_inputs(p, anim)?;
    let (id, instance, composition_id) = (inputs.id, inputs.instance, inputs.composition_id);
    let anchor = anim["song_anchor_tick"]
        .as_i64()
        .map(Tick)
        .or_else(|| anim["song_anchor"].as_f64().map(Tick::from_seconds_f64))
        .ok_or_else(|| invalid("Animation has no song_anchor"))?;
    let duration = anim["authored_duration_tick"]
        .as_i64()
        .map(Tick)
        .or_else(|| anim["authored_duration"].as_f64().map(Tick::from_seconds_f64))
        .ok_or_else(|| invalid("Animation has no authored_duration"))?;
    let words = build_local_words(anim, anchor)?;
    let (width, height) = (1920u64, 1080u64);
    let data = json!({
        "composition_id": composition_id,
        "animation_id": id,
        "source_revision": anim["source_revision"],
        "duration_tick": duration.0,
        "duration": duration.seconds(),
        "hold_tick": anim["hold_tick"],
        "hold": anim["hold"],
        "style": anim["style"],
        "kinetic": anim["kinetic"],
        "design": anim["design"],
        "words": words
    });
    let rel_src = format!("lettering/sections/{composition_id}.json");
    let file = dir.join(&rel_src);
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(&data).map_err(|e| invalid(e.to_string()))?;
    crate::doc_state::write_file(&file, &bytes)?;
    let drawings: Vec<Value> = read["graphics"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["kind"] == "drawing")
        .map(|cue| {
            let start = word_tick(cue, "start_tick", "start").unwrap_or(anchor);
            let end = word_tick(cue, "end_tick", "end").unwrap_or(start + HOLD_TICK);
            let dur = end - start;
            let index = cue["index"].as_u64().unwrap_or(0);
            let own = format!("{composition_id}-drawing-{index}");
            json!({
                "composition_id": own,
                "src": format!("lettering/sections/{own}.json"),
                "start_tick": start.0,
                "duration_tick": dur.0,
                "start": start.seconds(),
                "duration": dur.seconds()
            })
        })
        .collect();
    Ok(json!({
        "ok": true,
        "animation_id": id,
        "instance_id": instance,
        "composition_id": composition_id,
        "file": file,
        "drawings": drawings,
        "src": rel_src,
        "sha256": format!("{:x}", Sha256::digest(&bytes)),
        "source_revision": anim["source_revision"],
        "authored_duration_tick": duration.0,
        "song_anchor_tick": anchor.0,
        "authored_duration": duration.seconds(),
        "song_anchor": anchor.seconds(),
        "lead_in_tick": anim["lead_in_tick"],
        "hold_tick": anim["hold_tick"],
        "lead_in": anim["lead_in"],
        "hold": anim["hold"],
        "width": width,
        "height": height,
        "saved_source": read["saved"]
    }))
}
