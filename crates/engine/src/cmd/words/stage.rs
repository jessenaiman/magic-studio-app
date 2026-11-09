//! Words stage views, reading, saving, and drawing link management.
use super::compose::{kinetic_design, line_design, text_effect};
use crate::cmd::words::{
    ACTIONS, ENTRANCES, EXITS, HOLD_TICK, KINETIC_LIBRARY, aligned, default_document, document_path, fill_untimed, invalid, load, reference, save,
    studio_times, timing, with_sung_sections, word_tick, word_time_key,
};
use crate::time::{Tick, parse_tick};
use crate::{Result, Session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

fn is_image_file(p: &Path) -> bool {
    p.is_file() && p.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "webp"))
}

fn background_entry_from_dir(entry: &Path) -> Option<(String, Value)> {
    if is_image_file(entry) {
        let stem = entry.file_stem()?.to_str()?.to_owned();
        let name = entry.file_name()?.to_str()?.to_owned();
        return Some((stem.clone(), json!({"id":stem,"asset_id":stem,"name":name,"path":entry})));
    }
    let id = entry.file_name()?.to_str()?.to_owned();
    let img = fs::read_dir(entry).ok()?.flatten().map(|e| e.path()).find(|p| is_image_file(p))?;
    let panel = fs::read_to_string(entry.join("provenance.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v["from_panel"].as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("z-{id}"));
    let name = img.file_name()?.to_str()?.to_owned();
    Some((format!("{panel}:{id}"), json!({"id":id,"asset_id":id,"name":name,"path":img})))
}

fn push_unique_image(out: &mut Vec<Value>, seen: &mut std::collections::HashSet<String>, item: Value) {
    let id = item["id"].as_str().unwrap_or_default().to_owned();
    let path = item["path"].as_str().unwrap_or_default().to_owned();
    if !id.is_empty() && !path.is_empty() && seen.insert(id) && seen.insert(path) {
        out.push(item);
    }
}

pub(crate) fn project_backdrop_images(s: &Session, dir: &Path) -> Vec<Value> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut bg_entries: Vec<(String, Value)> =
        fs::read_dir(dir.join("assets/backgrounds")).into_iter().flatten().flatten().filter_map(|e| background_entry_from_dir(&e.path())).collect();
    bg_entries.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, item) in bg_entries {
        push_unique_image(&mut out, &mut seen, item);
    }
    for p in crate::workshop::panel_files(dir) {
        if let Some((_, item)) = background_entry_from_dir(&p) {
            push_unique_image(&mut out, &mut seen, item);
        }
    }
    for a in s.project_assets.iter().filter(|a| a.path.as_deref().is_some_and(is_image_file)) {
        push_unique_image(&mut out, &mut seen, json!({"id":a.id,"asset_id":a.id,"name":a.name,"path":a.path}));
    }
    out
}

fn assign_default_backgrounds(doc: &mut Value, images: &[Value]) {
    let Some(anims) = doc["animations"].as_array_mut() else {
        return;
    };
    if images.is_empty() || anims.is_empty() {
        return;
    }
    let total = anims.len();
    for (idx, anim) in anims.iter_mut().enumerate() {
        let current = anim["background_asset"].as_str().unwrap_or_default();
        if current.is_empty() || !images.iter().any(|img| img["id"].as_str() == Some(current)) {
            let pick = ((idx * 2 + 1) * images.len()) / (total * 2);
            anim["background_asset"] = images[pick.min(images.len() - 1)]["id"].clone();
        }
    }
}

fn inspect_lettering_ecproj(ecproj: &Path) -> Value {
    let Ok(bytes) = fs::read(ecproj) else {
        return Value::Null;
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return Value::Null;
    };
    let Ok(head) = craft_ecproj_format::Head::read(text) else {
        return Value::Null;
    };
    if craft_ecproj_format::check_schema(head.schema).is_err() {
        return Value::Null;
    }
    let outline = craft_doc::outline(&bytes, &craft_doc::Limits::default()).ok();
    let comp_count = outline.as_ref().map_or(0, |o| o.pages.len());
    let layer_count: usize = outline.as_ref().map_or(0, |o| o.pages.iter().map(|p| p.layers.len()).sum());
    json!({
        "path": ecproj,
        "saved_by": head.saved_by,
        "schema": head.schema,
        "comp_count": comp_count,
        "layer_count": layer_count
    })
}

pub fn cmd_words_lyrics_view(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::cmd::words::workspace_dir(s)?;
    let show = p["show"].as_str().unwrap_or("lyrics");
    if !matches!(show, "still" | "lyrics" | "scene" | "stage") {
        return Err(invalid(format!("show must be stage, still, lyrics or scene, not {show}")));
    }
    if show == "still" && p["animation_id"].as_str().is_none() {
        return Err(invalid("still needs animation_id"));
    }
    let ecproj = dir.join("lettering/lyrics.ecproj");
    let doc_path = document_path(&dir);
    let bytes = fs::read(&ecproj).or_else(|_| fs::read(&doc_path)).unwrap_or_default();
    Ok(json!({
        "ok": true,
        "show": show,
        "src": format!("lettering/{show}.ecproj"),
        "file": ecproj,
        "ecproj": inspect_lettering_ecproj(&ecproj),
        "written": false,
        "video_sha256": format!("{:x}", Sha256::digest(&bytes))
    }))
}

fn runtime_effectcraft_tools() -> Value {
    let steps = [
        json!({"command":"text.presets","params":{}}),
        json!({"command":"text.animatorProperties","params":{}}),
        json!({"command":"paint.presets","params":{}}),
        json!({"command":"effect.list","params":{}}),
    ];
    let results = crate::cmd::character::effectcraft(std::path::Path::new("."), None, &steps, false).unwrap_or_default();
    let get_data = |idx: usize| results.get(idx).cloned().map(|v| v.get("data").cloned().unwrap_or(v)).unwrap_or(Value::Null);
    let as_list = |v: Value, key: &str| {
        if v.is_array() { v } else { v.get(key).cloned().unwrap_or_else(|| json!([])) }
    };
    let text_presets = as_list(get_data(0), "presets");
    let animator_properties = as_list(get_data(1), "properties");
    let paint_presets = as_list(get_data(2), "presets");
    let effects: Vec<Value> = get_data(3)
        .as_array()
        .into_iter()
        .flatten()
        .map(|fx| {
            json!({
                "id": fx.get("id").or_else(|| fx.get("match_name")).unwrap_or(&Value::Null),
                "match_name": fx.get("match_name").or_else(|| fx.get("id")).unwrap_or(&Value::Null),
                "name": fx["name"],
                "category": fx["category"],
                "gpu": fx["gpu"]
            })
        })
        .collect();
    json!({
        "text_presets": text_presets,
        "animator_properties": animator_properties,
        "paint_presets": paint_presets,
        "effects": effects
    })
}

fn runtime_filmcraft_tools() -> Value {
    let run = |id: &str| -> Value { crate::CraftHub::execute(crate::CraftTarget::Film, None, id, &json!({})).unwrap_or(Value::Null) };
    let effects: Vec<Value> = run("effects.list")
        .as_array()
        .cloned()
        .into_iter()
        .flatten()
        .map(|fx| json!({"id": fx["id"], "name": fx["name"], "category": fx.get("folder").or_else(|| fx.get("category")).unwrap_or(&Value::Null), "kind": fx["kind"]}))
        .collect();
    let presets = run("presets.list");
    let export_presets = run("export.presets.list");
    json!({
        "effects": effects,
        "presets": if presets.is_array() { presets } else { json!([]) },
        "export_presets": if export_presets.is_array() { export_presets } else { json!([]) }
    })
}

pub(crate) fn runtime_words_tools(lib: &Value) -> Value {
    let move_meta = |m: &'static str| -> (&'static str, &'static str, &'static str) {
        match m {
            "quiet" => ("quiet", "Quiet", "Fades in and holds"),
            "sleep" => ("sleep", "Sleep", "Letters sink one by one"),
            "settle" => ("settle", "Settle", "Drops and settles into place"),
            "fall" => ("fall", "Fall", "Drops down from above"),
            "pop" => ("pop", "Pop", "Swells on the beat"),
            "rise" => ("rise", "Rise", "Arcs up from below"),
            "east" => ("east", "Slide East", "Slides right"),
            "west" => ("west", "Slide West", "Slides left"),
            "loop" => ("loop", "Loop", "Continuous wave"),
            _ => (m, m, "Motion act"),
        }
    };
    let moves: Vec<Value> = ["quiet", "pop", "loop", "fall", "rise", "east", "west", "settle", "sleep"]
        .into_iter()
        .map(|key| {
            let (id, name, does) = move_meta(key);
            let desc = lib["acts"][id]["does"].as_str().unwrap_or(does);
            json!({"id": id, "name": name, "does": desc})
        })
        .collect();
    let letterings = json!([
        {"id": "song", "name": "Song font", "does": "The video's own lettering", "font": "song", "weight": 400, "italic": false},
        {"id": "song-bold", "name": "Song bold", "does": "The video's lettering, heavy", "font": "song", "weight": 700, "italic": false},
        {"id": "serif-italic", "name": "Serif italic", "does": "A hero word, expressive", "font": "serif", "weight": 400, "italic": true},
        {"id": "serif-bold", "name": "Serif bold", "does": "A hero word, strong", "font": "serif", "weight": 700, "italic": false},
        {"id": "sans", "name": "Sans, quiet", "does": "Context words, small and calm", "font": "sans", "weight": 400, "italic": false}
    ]);
    let strokes: Vec<Value> = lib["strokes"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(id, s)| {
            json!({
                "id": id,
                "name": s["name"],
                "does": s["does"],
                "track": s["track"].as_str().unwrap_or("graphics"),
                "view_box": s["view_box"],
                "w": s["width"].as_f64().unwrap_or(320.0),
                "h": s["height"].as_f64().unwrap_or(90.0),
                "draw": s["draw"].as_f64().unwrap_or(0.9),
                "d": s["d"]
            })
        })
        .collect();
    let choreographies = json!([
        {"id": "cascade", "preview": "cascade", "name": "Cascade", "does": "Diagonal staircase"},
        {"id": "scatter", "preview": "scatter", "name": "Spiral / Scatter", "does": "Tilted dynamic spread"},
        {"id": "poster", "preview": "giant", "name": "Zoom Blast", "does": "Hero word giant & tilted"},
        {"id": "stack", "preview": "stack", "name": "Stack", "does": "Vertical stepped column"},
        {"id": "plain", "preview": "karaoke", "name": "Plain", "does": "Clean horizontal row"}
    ]);
    let easings = crate::animation_model::easing_catalog();
    let properties = crate::animation_model::property_catalog();
    json!({
        "moves": moves,
        "letterings": letterings,
        "strokes": strokes,
        "choreographies": choreographies,
        "easings": easings,
        "properties": properties,
        "symbol_loop_modes": ["loop", "play_once", "single_frame"],
        "actions": ACTIONS,
        "entrances": ENTRANCES,
        "exits": EXITS,
        "effectcraft": runtime_effectcraft_tools(),
        "filmcraft": runtime_filmcraft_tools()
    })
}

pub fn cmd_words_effects(_s: &mut Session, p: &Value) -> Result<Value> {
    if let Some(kind) = p["kind"].as_str()
        && kind != "text"
    {
        return Err(invalid("kind must be one of text"));
    }
    let kinetic_library: Value = serde_json::from_str(KINETIC_LIBRARY).map_err(|e| invalid(e.to_string()))?;
    let tools = runtime_words_tools(&kinetic_library);
    let text_presets = tools["effectcraft"]["text_presets"].clone();
    Ok(json!({
        "source": "effectcraft + filmcraft",
        "kinds": {
            "text": {
                "query": "word by word text reveal",
                "tier": "native",
                "effects": text_presets
            }
        },
        "tools": tools
    }))
}

fn append_kinetic_cues(anim: &Value, words: &[Value], lib: &Value, anchor: Tick, section_end: Tick, cues: &mut Vec<Value>) {
    let k = &anim["kinetic"];
    for (i, d) in k["draws"].as_array().into_iter().flatten().enumerate() {
        let Some(word) = words.iter().find(|w| w["word_id"] == d["word_id"]) else {
            continue;
        };
        let Some(word_anchor) = word_tick(word, "start_tick", "start") else {
            continue;
        };
        let offset = d.get("offset").and_then(parse_tick).unwrap_or(Tick::ZERO);
        let start = Tick((word_anchor.0 + offset.0).max(0));
        let Some(stroke) = d.get("drawing").filter(|v| v.is_object()).or_else(|| d["stroke"].as_str().and_then(|key| lib["strokes"].get(key))) else {
            continue;
        };
        if d["track"] == "lettering" || (d["track"].is_null() && stroke["track"] == "lettering") {
            continue;
        }
        let duration = d.get("duration").and_then(parse_tick).or_else(|| stroke.get("draw").and_then(parse_tick)).unwrap_or(HOLD_TICK);
        let hold = d.get("hold").and_then(parse_tick).unwrap_or(HOLD_TICK);
        let end = start + duration + hold;
        let draw_end = start + duration;
        cues.push(json!({
            "id": format!("{}-draw-{i}", anim["animation_id"].as_str().unwrap_or("words")),
            "kind": "drawing",
            "animation_id": anim["animation_id"],
            "line_id": word["line_id"],
            "word_ids": [word["word_id"]],
            "word": word["text"],
            "name": stroke["name"],
            "stroke": stroke,
            "index": i,
            "anchor_tick": word_anchor.0,
            "anchor": word_anchor.seconds(),
            "color": d.get("color").unwrap_or(&k["style"]["hero_color"]),
            "start_tick": start.0,
            "end_tick": end.0,
            "draw_end_tick": draw_end.0,
            "start": start.seconds(),
            "end": end.seconds(),
            "draw_end": draw_end.seconds()
        }));
    }
    if let Some(key) = k["overlay"].as_str().filter(|key| *key != "none")
        && let Some(overlay) = lib["overlays"].get(key)
    {
        let offset = k.get("overlay_offset").and_then(parse_tick).unwrap_or(Tick::ZERO);
        let start = Tick((anchor.0 + offset.0).max(0));
        let duration = k.get("overlay_duration").and_then(parse_tick).unwrap_or(section_end - anchor);
        let end = start + duration;
        cues.push(json!({
            "id": format!("{}-overlay", anim["animation_id"].as_str().unwrap_or("words")),
            "kind": "overlay",
            "animation_id": anim["animation_id"],
            "name": overlay["name"],
            "overlay": key,
            "anchor_tick": anchor.0,
            "anchor": anchor.seconds(),
            "start_tick": start.0,
            "end_tick": end.0,
            "start": start.seconds(),
            "end": end.seconds(),
            "word_ids": []
        }));
    }
}

fn append_static_design_cues(anim: &Value, words: &[Value], section_end: Tick, cues: &mut Vec<Value>) {
    for (line_id, design) in anim["design"].as_object().into_iter().flatten() {
        let line: Vec<_> = words.iter().filter(|w| w["line_id"] == *line_id).collect();
        let Some(start) = line.first().and_then(|w| word_tick(w, "start_tick", "start")) else {
            continue;
        };
        let end = words
            .iter()
            .find(|w| w["line_id"] != *line_id && word_tick(w, "start_tick", "start").is_some_and(|t| t > start))
            .and_then(|w| word_tick(w, "start_tick", "start"))
            .unwrap_or(section_end);
        for (i, d) in design["draws"].as_array().into_iter().flatten().enumerate() {
            let paths = d["paths"].as_array().into_iter().flatten();
            let from = paths.clone().filter_map(|p| p.get("from").and_then(parse_tick)).min().unwrap_or(Tick::ZERO);
            let to = paths.clone().filter_map(|p| p.get("to").and_then(parse_tick)).max().unwrap_or(from);
            let cue_start = start + from;
            let draw_end = (start + to).min(end);
            cues.push(json!({
                "id": format!("{}-{line_id}-draw-{i}", anim["animation_id"].as_str().unwrap_or("words")),
                "kind": "drawing",
                "animation_id": anim["animation_id"],
                "line_id": line_id,
                "word_ids": line.iter().map(|w| w["word_id"].clone()).collect::<Vec<_>>(),
                "name": d["name"],
                "stroke": {"view_box": d["view_box"], "d": paths.filter_map(|p| p["d"].as_str()).collect::<Vec<_>>().join(" ")},
                "start_tick": cue_start.0,
                "end_tick": end.0,
                "draw_end_tick": draw_end.0,
                "start": cue_start.seconds(),
                "end": end.seconds(),
                "draw_end": draw_end.seconds()
            }));
        }
    }
}

fn hydrate_animation_times(anim: &mut Value, times: &std::collections::HashMap<String, (Tick, Tick)>) {
    let scope = anim["scope"].clone();
    for w in anim["words"].as_array_mut().into_iter().flatten() {
        if let Some((a, b)) = w["word_id"].as_str().and_then(|id| times.get(&word_time_key(&scope, id))) {
            w["start_tick"] = json!(a.0);
            w["end_tick"] = json!(b.0);
            w["start"] = json!(a.seconds());
            w["end"] = json!(b.seconds());
        }
    }
    fill_untimed(anim);
    timing(anim);
}

fn graphic_cues(doc: &Value, lib: &Value, times: &std::collections::HashMap<String, (Tick, Tick)>) -> Vec<Value> {
    let mut cues = Vec::new();
    for source in doc["animations"].as_array().into_iter().flatten() {
        let mut anim = source.clone();
        hydrate_animation_times(&mut anim, times);
        let words = anim["words"].as_array().cloned().unwrap_or_default();
        let anchor = anim["song_anchor_tick"].as_i64().map(Tick).or_else(|| anim["song_anchor"].as_f64().map(Tick::from_seconds_f64)).unwrap_or(Tick::ZERO);
        let dur =
            anim["authored_duration_tick"].as_i64().map(Tick).or_else(|| anim["authored_duration"].as_f64().map(Tick::from_seconds_f64)).unwrap_or(Tick::ZERO);
        let section_end = anchor + dur;
        if anim["kinetic"].is_object() {
            append_kinetic_cues(&anim, &words, lib, anchor, section_end, &mut cues);
        } else {
            append_static_design_cues(&anim, &words, section_end, &mut cues);
        }
    }
    cues.sort_by_key(|a| a["start_tick"].as_i64().unwrap_or(0));
    cues
}

fn load_or_default_doc(s: &mut Session, dir: &Path) -> Result<(Value, bool)> {
    let (mut doc, saved) = match load(s, dir)? {
        Some(doc) => (with_sung_sections(doc, &aligned(s)?)?, true),
        None => (default_document(&aligned(s)?)?, false),
    };
    let times = studio_times(s);
    for anim in doc["animations"].as_array_mut().into_iter().flatten() {
        hydrate_animation_times(anim, &times);
    }
    assign_default_backgrounds(&mut doc, &project_backdrop_images(s, dir));
    Ok((doc, saved))
}

fn apply_display_text_for_read(doc: &mut Value) {
    for a in doc["animations"].as_array_mut().into_iter().flatten() {
        for w in a["words"].as_array_mut().into_iter().flatten() {
            if let Some(text) = w["display_text"].as_str().map(str::to_owned) {
                w["source_text"] = w["text"].clone();
                w["text"] = json!(text);
            }
        }
    }
}

fn project_scene_clips(dir: &Path, doc: &Value) -> Vec<Value> {
    let motion_dir = dir.join("motion");
    let Ok(entries) = fs::read_dir(&motion_dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("mp4")) && p.file_stem().is_some_and(|st| st.to_string_lossy().starts_with("Scene "))
        })
        .collect();
    files.sort();
    let animations = doc["animations"].as_array().cloned().unwrap_or_default();
    let n_sec = animations.len().max(1);
    files
        .into_iter()
        .enumerate()
        .map(|(idx, path)| {
            let stem = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
            let id = stem.strip_prefix("Scene ").unwrap_or(&stem).to_string();
            let sec_idx = idx.min(n_sec - 1);
            let anim = animations.get(sec_idx);
            let sec_name = anim.and_then(|a| a["scope"]["section"].as_str()).unwrap_or("Scene");
            let start = anim.and_then(|a| a["song_anchor"].as_f64()).unwrap_or(0.0);
            json!({
                "id": id,
                "label": format!("{id} · {sec_name}"),
                "sectionIndex": sec_idx,
                "start": start,
                "path": path
            })
        })
        .collect()
}

pub fn cmd_words_read(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::cmd::words::workspace_dir(s)?;
    let (mut doc, saved) = load_or_default_doc(s, &dir)?;
    let (_, current_sha, _) = reference(&dir)?;
    let stale = saved && doc["reference"]["sha256"].as_str() != Some(current_sha.as_str());
    if let Some(id) = p["animation_id"].as_str() {
        let anim = doc["animations"]
            .as_array()
            .and_then(|a| a.iter().find(|x| x["animation_id"] == id))
            .cloned()
            .ok_or_else(|| invalid(format!("No Words animation {id}")))?;
        doc["animations"] = json!([anim]);
    }
    apply_display_text_for_read(&mut doc);
    let in_video: Vec<String> = doc["animations"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|a| a["in_video"].as_bool() != Some(false))
        .filter_map(|a| a["animation_id"].as_str().map(str::to_owned))
        .collect();
    let kinetic_library: Value = serde_json::from_str(KINETIC_LIBRARY).map_err(|e| invalid(e.to_string()))?;
    let tools = runtime_words_tools(&kinetic_library);
    let graphics = graphic_cues(&doc, &kinetic_library, &studio_times(s));
    let images = project_backdrop_images(s, &dir);
    let scene_clips = project_scene_clips(&dir, &doc);
    let ecproj = inspect_lettering_ecproj(&dir.join("lettering/lyrics.ecproj"));
    Ok(json!({
        "images": images,
        "scene_clips": scene_clips,
        "graphics": graphics,
        "ecproj": ecproj,
        "ok": true,
        "saved": saved,
        "reference_changed": stale,
        "path": document_path(&dir),
        "document": doc,
        "in_video": in_video,
        "fonts": [],
        "kinetic_library": kinetic_library,
        "tools": tools
    }))
}

pub(crate) fn one_of(value: &Value, allowed: &[&str], what: &str) -> Result<Value> {
    value.as_str().filter(|v| allowed.contains(v)).map(|v| json!(v)).ok_or_else(|| invalid(format!("{what} must be one of {}", allowed.join(", "))))
}

fn apply_style_edits(anim: &mut Value, style: &Value) -> Result<()> {
    let map = style.as_object().ok_or_else(|| invalid("style must be an object"))?;
    for (key, value) in map {
        let valid = match key.as_str() {
            "font" | "color" | "emphasis_color" | "outline_color" | "background" => value.is_string(),
            "size" => value.as_f64().is_some_and(|v| (40.0..=320.0).contains(&v)),
            _ => false,
        };
        if !valid {
            return Err(invalid(format!("style.{key} is not an editable style field or has an invalid value")));
        }
        anim["style"][key] = value.clone();
    }
    Ok(())
}

fn validate_single_word_field(key: &str, value: &Value) -> Result<Value> {
    match key {
        "display_text" => value
            .as_str()
            .filter(|t| t.len() <= 4096 && !t.chars().any(|c| c.is_control()))
            .map(|t| json!(t))
            .ok_or_else(|| invalid("display_text must be at most 4096 bytes of plain text")),
        "emphasis" => value.as_bool().map(|b| json!(b)).ok_or_else(|| invalid("emphasis must be true or false")),
        "action" => one_of(value, ACTIONS, "action"),
        "entrance" => one_of(value, ENTRANCES, "entrance"),
        "exit" => one_of(value, EXITS, "exit"),
        other => Err(invalid(format!("{other} is not editable here; text and timing come from the artist's sheet and the reference"))),
    }
}

fn apply_word_edits(anim: &mut Value, id: &str, edits: &[Value]) -> Result<()> {
    for edit in edits {
        let word_id = edit["word_id"].as_str().ok_or_else(|| invalid("each word edit needs word_id"))?;
        let word = anim["words"]
            .as_array_mut()
            .and_then(|w| w.iter_mut().find(|w| w["word_id"] == word_id))
            .ok_or_else(|| invalid(format!("{word_id} is not in {id}")))?;
        for (key, value) in edit.as_object().into_iter().flatten() {
            if key == "word_id" {
                continue;
            }
            word[key] = validate_single_word_field(key, value)?;
        }
    }
    Ok(())
}

fn apply_line_designs(anim: &mut Value, id: &str, designs: &Value) -> Result<()> {
    for (line_id, design) in designs.as_object().ok_or_else(|| invalid("design must be an object of line_id: design"))? {
        let words: Vec<Value> = anim["words"].as_array().into_iter().flatten().filter(|w| w["line_id"] == json!(line_id)).cloned().collect();
        if words.is_empty() {
            return Err(invalid(format!("{line_id} is not a line of {id}")));
        }
        anim["design"][line_id] = if design.is_null() { Value::Null } else { line_design(&words, design)? };
    }
    Ok(())
}

fn apply_design_and_effects(dir: &Path, anim: &mut Value, id: &str, p: &Value) -> Result<()> {
    if let Some(designs) = p.get("design").filter(|v| !v.is_null()) {
        apply_line_designs(anim, id, designs)?;
    }
    if let Some(effects) = p.get("effects").filter(|v| !v.is_null()) {
        for (track, choice) in effects.as_object().ok_or_else(|| invalid("effects must be an object of track: effect"))? {
            if track != "text" {
                return Err(invalid(format!("effects.{track}: only the text track takes registry effects so far")));
            }
            anim["effects"][track] = if choice.is_null() { Value::Null } else { text_effect(dir, choice)? };
        }
    }
    Ok(())
}

fn apply_background_edit(s: &Session, dir: &Path, anim: &mut Value, background: &Value) -> Result<()> {
    if background.is_null() {
        anim["background_asset"] = Value::Null;
        anim["background_custom"] = Value::Null;
        return Ok(());
    }
    let asset_id = background.as_str().ok_or_else(|| invalid("background_asset must be a project image id"))?;
    let valid = project_backdrop_images(s, dir).iter().any(|a| a["id"].as_str() == Some(asset_id));
    if !valid {
        return Err(invalid("Choose an existing project image for this verse"));
    }
    anim["background_asset"] = json!(asset_id);
    anim["background_custom"] = json!(true);
    Ok(())
}

fn apply_save_payload(s: &Session, dir: &Path, anim: &mut Value, id: &str, p: &Value) -> Result<()> {
    if let Some(background) = p.get("background_asset") {
        apply_background_edit(s, dir, anim, background)?;
    }
    if let Some(style) = p.get("style").filter(|v| !v.is_null()) {
        apply_style_edits(anim, style)?;
    }
    if let Some(edits) = p["words"].as_array() {
        apply_word_edits(anim, id, edits)?;
    }
    apply_design_and_effects(dir, anim, id, p)?;
    if let Some(k) = p.get("kinetic") {
        anim["kinetic"] = if k.is_null() { Value::Null } else { kinetic_design(&anim["words"], k)? };
    }
    Ok(())
}

pub fn cmd_words_save(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::cmd::words::workspace_dir(s)?;
    let id = p["animation_id"].as_str().ok_or_else(|| invalid("animation_id is required"))?;
    let expected = p["expected_source_revision"].as_u64().ok_or_else(|| invalid("expected_source_revision is required"))?;
    let (mut doc, _) = load_or_default_doc(s, &dir)?;
    let anim = doc["animations"]
        .as_array_mut()
        .and_then(|a| a.iter_mut().find(|x| x["animation_id"] == id))
        .ok_or_else(|| invalid(format!("No Words animation {id}")))?;
    let current = anim["source_revision"].as_u64().unwrap_or(0);
    if current != expected {
        return Ok(json!({
            "ok": false,
            "conflict": true,
            "animation_id": id,
            "source_revision": current,
            "expected_source_revision": expected,
            "reason": "The animation changed since you read it; reload and reapply your edit"
        }));
    }
    apply_save_payload(s, &dir, anim, id, p)?;
    anim["source_revision"] = json!(current + 1);
    anim["origin"] = p.get("origin").filter(|v| v.is_string()).cloned().unwrap_or(json!("magic"));
    let saved = anim.clone();
    save(s, &dir, &doc)?;
    Ok(json!({"ok":true,"conflict":false,"path":document_path(&dir),"animation":saved}))
}

fn find_section_indices(doc: &Value, source: &str, target: &str) -> Result<(usize, usize)> {
    let list = doc["animations"].as_array().ok_or_else(|| invalid("No lyric sections"))?;
    let si = list.iter().position(|a| a["animation_id"] == source).ok_or_else(|| invalid("No source section"))?;
    let ti = list.iter().position(|a| a["animation_id"] == target).ok_or_else(|| invalid("No target section"))?;
    Ok((si, ti))
}

fn move_drawing_between_sections(doc: &mut Value, si: usize, ti: usize, index: usize, drawing: Value) -> Result<usize> {
    let target_index = if si == ti {
        doc["animations"][si]["kinetic"]["draws"][index] = drawing;
        index
    } else {
        doc["animations"][si]["kinetic"]["draws"].as_array_mut().ok_or_else(|| invalid("Source drawings are not an array"))?.remove(index);
        if !doc["animations"][ti]["kinetic"]["draws"].is_array() {
            doc["animations"][ti]["kinetic"]["draws"] = json!([]);
        }
        let draws = doc["animations"][ti]["kinetic"]["draws"].as_array_mut().ok_or_else(|| invalid("Target drawings are not an array"))?;
        let target_index = draws.len();
        draws.push(drawing);
        target_index
    };
    for i in if si == ti { vec![si] } else { vec![si, ti] } {
        let a = &mut doc["animations"][i];
        a["kinetic"] = kinetic_design(&a["words"], &a["kinetic"])?;
        a["source_revision"] = json!(a["source_revision"].as_u64().unwrap_or(0) + 1);
    }
    Ok(target_index)
}

fn resolve_target_word_time(doc: &Value, ti: usize, word: &str, times: &std::collections::HashMap<String, (Tick, Tick)>) -> Result<Tick> {
    let w = doc["animations"][ti]["words"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|w| w["word_id"] == word)
        .ok_or_else(|| invalid("The target word is not in this section"))?;
    let at = times
        .get(&word_time_key(&doc["animations"][ti]["scope"], word))
        .map(|t| t.0)
        .or_else(|| word_tick(w, "start_tick", "start"))
        .ok_or_else(|| invalid("The target word has no known timing"))?;
    if !doc["animations"][ti]["kinetic"].is_object() {
        return Err(invalid("Choose a word in a kinetic lyric section"));
    }
    Ok(at)
}

fn check_link_revisions(doc: &Value, si: usize, ti: usize, p: &Value) -> Result<bool> {
    let expected = p["expected_source_revision"].as_u64().ok_or_else(|| invalid("expected_source_revision is required"))?;
    let target_expected =
        if si == ti { expected } else { p["expected_target_revision"].as_u64().ok_or_else(|| invalid("expected_target_revision is required"))? };
    Ok(doc["animations"][si]["source_revision"].as_u64() == Some(expected) && doc["animations"][ti]["source_revision"].as_u64() == Some(target_expected))
}

fn prepare_relinked_drawing(doc: &Value, si: usize, index: usize, word: &str, cue_start: Tick, at: Tick) -> Result<Value> {
    let mut drawing =
        doc["animations"][si]["kinetic"]["draws"].as_array().and_then(|d| d.get(index)).cloned().ok_or_else(|| invalid("Drawing index is stale"))?;
    let offset = cue_start - at;
    drawing["word_id"] = json!(word);
    drawing["offset"] = json!(offset.seconds());
    if drawing["color"].is_null() {
        drawing["color"] = doc["animations"][si]["kinetic"]["style"]["hero_color"].clone();
    }
    Ok(drawing)
}

fn parse_link_request(p: &Value) -> Result<(&str, &str, &str, usize)> {
    let source = p["animation_id"].as_str().ok_or_else(|| invalid("animation_id is required"))?;
    let target = p["target_animation_id"].as_str().unwrap_or(source);
    let word = p["word_id"].as_str().ok_or_else(|| invalid("word_id is required"))?;
    let index = p["index"].as_u64().ok_or_else(|| invalid("drawing index is required"))? as usize;
    Ok((source, target, word, index))
}

fn find_source_cue_start(s: &mut Session, doc: &Value, source: &str, index: usize, ti: usize, word: &str) -> Result<(Tick, Tick)> {
    let lib: Value = serde_json::from_str(KINETIC_LIBRARY).map_err(|e| invalid(e.to_string()))?;
    let times = studio_times(s);
    let cues = graphic_cues(doc, &lib, &times);
    let cue = cues
        .iter()
        .find(|c| c["animation_id"] == source && c["index"].as_u64() == Some(index as u64))
        .ok_or_else(|| invalid("Select a drawing clip to relink"))?;
    let at = resolve_target_word_time(doc, ti, word, &times)?;
    let cue_start = word_tick(cue, "start_tick", "start").unwrap_or(at);
    Ok((cue_start, at))
}

pub fn cmd_words_link(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::cmd::words::workspace_dir(s)?;
    let mut doc = load(s, &dir)?.ok_or_else(|| invalid("Save this lyric design first"))?;
    let (source, target, word, index) = parse_link_request(p)?;
    let (si, ti) = find_section_indices(&doc, source, target)?;
    if !check_link_revisions(&doc, si, ti, p)? {
        return Ok(json!({"ok":false,"conflict":true,"reason":"A section changed; nothing relinked"}));
    }
    let (cue_start, at) = find_source_cue_start(s, &doc, source, index, ti, word)?;
    let drawing = prepare_relinked_drawing(&doc, si, index, word, cue_start, at)?;
    let target_index = move_drawing_between_sections(&mut doc, si, ti, index, drawing)?;
    save(s, &dir, &doc)?;
    Ok(json!({
        "ok": true,
        "conflict": false,
        "animation_id": target,
        "index": target_index,
        "word_id": word,
        "start_tick": cue_start.0,
        "start": cue_start.seconds(),
        "changed_sections": if si == ti { vec![source] } else { vec![source, target] }
    }))
}
