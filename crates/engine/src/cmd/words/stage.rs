//! Words stage views, reading, saving, and drawing link management.
use super::compose::{kinetic_design, line_design, text_effect};
use crate::cmd::words::{
    ACTIONS, ENTRANCES, EXITS, KINETIC_LIBRARY, aligned, default_document, document_path,
    fill_untimed, invalid, load, reference, save, studio_times, tag_attr, timing,
    with_sung_sections, word_time_key,
};
use crate::{Result, Session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

const LYRICS_ONLY_STYLE: &str = r#"<style data-magic="lyrics-only">
[data-composition-id] * { visibility: hidden !important; }
.lyric-design-clip, .lyric-design-clip *, .lyric-drawing-clip, .lyric-drawing-clip * { visibility: visible !important; }
#title, .caption, .lyric-word-clip, .lyric-word-clip *, [data-composition-src*="captions.html"] { visibility: hidden !important; }
html, body, [data-composition-id] { background: #1b2430 !important; }
</style>"#;

fn stage_only_style(html: &str) -> String {
    let mut visuals: Vec<(String, u64, f64, f64)> = Vec::new();
    let mut from = 0;
    while let Some(open) = html[from..].find('<').map(|i| i + from) {
        let close = html[open..].find('>').map_or(html.len(), |i| i + open + 1);
        let tag = &html[open..close];
        from = close;
        let is_media = (tag.starts_with("<video") || tag.starts_with("<img"))
            && tag_attr(tag, "class").is_some_and(|c| c.split_whitespace().any(|c| c == "clip"));
        if !is_media {
            continue;
        }
        let num = |k: &str| tag_attr(tag, k).and_then(|v| v.parse::<f64>().ok());
        if let (Some(id), Some(start)) = (tag_attr(tag, "id"), num("data-start")) {
            visuals.push((
                id.to_owned(),
                num("data-track-index").unwrap_or(0.0) as u64,
                start,
                start + num("data-duration").unwrap_or(f64::MAX),
            ));
        }
    }
    let cast: Vec<String> = visuals
        .iter()
        .filter(|(_, track, a, b)| {
            visuals
                .iter()
                .any(|(_, t, x, y)| t < track && x < b && a < y)
        })
        .map(|(id, ..)| format!("#{id}"))
        .collect();
    let hide = if cast.is_empty() {
        String::new()
    } else {
        format!("{} {{ visibility: hidden !important; }}\n", cast.join(", "))
    };
    format!(
        "<style data-magic=\"stage\">\n{hide}#stage-fill, #title, .lyric-word-clip, .lyric-word-clip *, .caption, [data-composition-src*=\"captions.html\"] {{ visibility: hidden !important; }}\n</style>"
    )
}

const SCENE_ONLY_STYLE: &str = r#"<style data-magic="scene-only">
#stage-fill, #title, .lyric-word-clip, .lyric-word-clip *, .lyric-design-clip, .lyric-design-clip *, .lyric-drawing-clip, .lyric-drawing-clip *, .caption, [data-composition-src*="captions.html"] { visibility: hidden !important; }
</style>"#;

fn without_video_media(html: &str) -> String {
    let mut out = html.to_owned();
    while let Some(start) = out.find("<video") {
        let end = out[start..]
            .find("</video>")
            .map(|i| start + i + 8)
            .or_else(|| out[start..].find('>').map(|i| start + i + 1))
            .unwrap_or(out.len());
        out.replace_range(start..end, "");
    }
    let mut from = 0;
    while let Some(rel) = out[from..].find("<img") {
        let start = from + rel;
        let end = out[start..]
            .find('>')
            .map(|i| start + i + 1)
            .unwrap_or(out.len());
        let tag = &out[start..end];
        if tag_attr(tag, "id").is_some_and(|id| id.starts_with("scene-")) {
            from = end;
        } else {
            out.replace_range(start..end, "");
        }
    }
    out
}

fn is_image_file(p: &Path) -> bool {
    p.is_file()
        && p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp"
            )
        })
}

fn background_entry_from_dir(entry: &Path) -> Option<(String, Value)> {
    if is_image_file(entry) {
        let stem = entry.file_stem()?.to_str()?.to_owned();
        let name = entry.file_name()?.to_str()?.to_owned();
        return Some((
            stem.clone(),
            json!({"id":stem,"asset_id":stem,"name":name,"path":entry}),
        ));
    }
    let id = entry.file_name()?.to_str()?.to_owned();
    let img = fs::read_dir(entry)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| is_image_file(p))?;
    let panel = fs::read_to_string(entry.join("provenance.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v["from_panel"].as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("z-{id}"));
    let name = img.file_name()?.to_str()?.to_owned();
    Some((
        format!("{panel}:{id}"),
        json!({"id":id,"asset_id":id,"name":name,"path":img}),
    ))
}

fn push_unique_image(
    out: &mut Vec<Value>,
    seen: &mut std::collections::HashSet<String>,
    item: Value,
) {
    let id = item["id"].as_str().unwrap_or_default().to_owned();
    let path = item["path"].as_str().unwrap_or_default().to_owned();
    if !id.is_empty() && !path.is_empty() && seen.insert(id) && seen.insert(path) {
        out.push(item);
    }
}

pub(crate) fn project_backdrop_images(s: &Session, dir: &Path) -> Vec<Value> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut bg_entries: Vec<(String, Value)> = fs::read_dir(dir.join("assets/backgrounds"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| background_entry_from_dir(&e.path()))
        .collect();
    bg_entries.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, item) in bg_entries {
        push_unique_image(&mut out, &mut seen, item);
    }
    for p in crate::workshop::panel_files(dir) {
        if let Some((_, item)) = background_entry_from_dir(&p) {
            push_unique_image(&mut out, &mut seen, item);
        }
    }
    for a in s
        .project_assets
        .iter()
        .filter(|a| a.path.as_deref().is_some_and(is_image_file))
    {
        push_unique_image(
            &mut out,
            &mut seen,
            json!({"id":a.id,"asset_id":a.id,"name":a.name,"path":a.path}),
        );
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

pub(crate) fn stage_backdrop_file(s: &Session, dir: &Path, asset_id: &str) -> Result<String> {
    let images = project_backdrop_images(s, dir);
    let asset = images
        .iter()
        .find(|a| a["id"].as_str() == Some(asset_id))
        .and_then(|a| a["path"].as_str())
        .map(std::path::PathBuf::from)
        .ok_or_else(|| invalid("The scene image is no longer in this project"))?;
    let bytes = fs::read(&asset)?;
    let name = format!(
        "{:x}.{}",
        Sha256::digest(&bytes),
        asset.extension().and_then(|e| e.to_str()).unwrap_or("png")
    );
    let path = dir
        .join("hyperframes/compositions/words/backdrops")
        .join(&name);
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| invalid("Output path has no parent"))?,
    )?;
    if !path.is_file() {
        fs::write(&path, bytes)?;
    }
    Ok(name)
}

fn resolve_requested_backdrop<'a>(
    animations: &'a [Value],
    full: &'a Value,
    id: &str,
) -> Option<&'a str> {
    animations
        .iter()
        .find(|a| a["animation_id"] == id)
        .and_then(|a| a["background_asset"].as_str())
        .or_else(|| {
            animations
                .iter()
                .find_map(|a| a["background_asset"].as_str())
        })
        .or_else(|| full["images"].as_array()?.first()?.get("id")?.as_str())
}

fn build_backdrop_tags(
    s: &Session,
    dir: &Path,
    animations: &[Value],
    base_name: &str,
    has_scene_plates: bool,
) -> String {
    let mut tags = if has_scene_plates {
        String::new()
    } else {
        format!(
            "<img data-lyrics-backdrop src=\"compositions/words/backdrops/{base_name}\" style=\"position:absolute;inset:0;width:100%;height:100%;object-fit:cover;z-index:0\">"
        )
    };
    for (idx, anim) in animations.iter().enumerate() {
        if has_scene_plates && anim["background_custom"].as_bool() != Some(true) {
            continue;
        }
        let Some(asset_id) = anim["background_asset"].as_str() else {
            continue;
        };
        let Ok(name) = stage_backdrop_file(s, dir, asset_id) else {
            continue;
        };
        let anchor = anim["song_anchor"].as_f64().unwrap_or(0.0);
        let start = if idx == 0 { 0.0 } else { anchor };
        let raw_dur = anim["authored_duration"].as_f64().unwrap_or(10.0);
        let end = animations
            .get(idx + 1)
            .and_then(|next| next["song_anchor"].as_f64())
            .unwrap_or(anchor + raw_dur + 5.0);
        let duration = (end - start).max(raw_dur);
        tags.push_str(&format!(
            "<img id=\"backdrop-{idx}\" class=\"clip\" data-lyrics-backdrop src=\"compositions/words/backdrops/{name}\" data-start=\"{start:.3}\" data-duration=\"{duration:.3}\" data-track-index=\"0\" style=\"position:absolute;inset:0;width:100%;height:100%;object-fit:cover;z-index:1\">"
        ));
    }
    tags
}

fn still_stage(s: &mut Session, html: &str, id: &str) -> Result<String> {
    let dir = crate::cmd::words::workspace_dir(s)?;
    let full = cmd_words_read(s, &json!({}))?;
    let animations = full["document"]["animations"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let requested = resolve_requested_backdrop(&animations, &full, id)
        .ok_or_else(|| invalid("Choose this verse's scene image"))?;
    let base_name = stage_backdrop_file(s, &dir, requested)?;
    let tags = build_backdrop_tags(
        s,
        &dir,
        &animations,
        &base_name,
        html.contains("id=\"scene-"),
    );
    let mut out = without_video_media(html);
    let root = out
        .find("data-composition-id=\"main\"")
        .or_else(|| out.find("id=\"root\""))
        .or_else(|| out.find("data-composition-id=\""))
        .and_then(|i| out[i..].find('>').map(|j| i + j + 1))
        .ok_or_else(|| invalid("The video has no main composition root"))?;
    out.insert_str(root, &tags);
    Ok(out)
}

fn lyrics_view_style_and_name(show: &str, html: &str) -> Result<(String, &'static str)> {
    match show {
        "still" => Ok((
            "<style data-magic=\"still\">#stage-fill, #title, .caption, .lyric-word-clip, .lyric-word-clip *, [data-composition-src*=\"captions.html\"] { visibility: hidden !important; }</style>".to_owned(),
            "still-stage",
        )),
        "lyrics" => Ok((LYRICS_ONLY_STYLE.to_owned(), "lyrics-only")),
        "scene" => Ok((SCENE_ONLY_STYLE.to_owned(), "scene-only")),
        "stage" => Ok((stage_only_style(html), "stage")),
        other => Err(invalid(format!("show must be stage, lyrics or scene, not {other}"))),
    }
}

fn write_if_changed(file: &Path, content: &str) -> Result<bool> {
    let written = fs::read_to_string(file).ok().as_deref() != Some(content);
    if written {
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(file, content)?;
    }
    Ok(written)
}

pub fn cmd_words_lyrics_view(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::cmd::words::workspace_dir(s)?;
    let hf = dir.join("hyperframes");
    let html = fs::read_to_string(hf.join("index.html"))
        .map_err(|_| invalid("This project has no HyperFrames video yet; run video.init first"))?;
    let show = p["show"].as_str().unwrap_or("lyrics");
    let html = if show == "still" {
        let id = p["animation_id"]
            .as_str()
            .ok_or_else(|| invalid("still needs animation_id"))?;
        still_stage(s, &html, id)?
    } else {
        html
    };
    let (style, name) = lyrics_view_style_and_name(show, &html)?;
    if name != "stage" && name != "still-stage" && !html.contains("lyric-word-clip") {
        return Err(invalid(
            "The video has no lyric word clips (class lyric-word-clip) to show on their own",
        ));
    }
    let head = html
        .find("</head>")
        .ok_or_else(|| invalid("The video's index.html has no </head>; nothing written"))?;
    let derived = format!("{}{style}\n{}", &html[..head], &html[head..]);
    let src = format!("compositions/words/{name}.html");
    let file = hf.join(&src);
    let written = write_if_changed(&file, &derived)?;
    if written {
        let _ = fs::remove_dir_all(hf.join(".hyperframes/preview"));
    }
    Ok(json!({
        "src": src,
        "file": file,
        "written": written,
        "video_sha256": format!("{:x}", Sha256::digest(html.as_bytes()))
    }))
}

const EFFECT_KINDS: &[(&str, &str)] = &[("text", "word by word text reveal")];

fn runtime_effectcraft_tools() -> Value {
    let mut ec = effectcraft_host::session();
    let mut get_data = |cmd: &str| {
        ec.execute_checked(cmd, json!({}))
            .ok()
            .map(|v| v.get("data").cloned().unwrap_or(v))
            .unwrap_or(Value::Null)
    };
    let as_list = |v: Value, key: &str| {
        if v.is_array() {
            v
        } else {
            v.get(key).cloned().unwrap_or_else(|| json!([]))
        }
    };
    let text_presets = as_list(get_data("text.presets"), "presets");
    let animator_properties = as_list(get_data("text.animatorProperties"), "properties");
    let paint_presets = as_list(get_data("paint.presets"), "presets");
    let effects: Vec<Value> = get_data("effect.list")
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
    let mut fc = filmcraft_engine::Session::default();
    let effects: Vec<Value> = fc
        .execute("effects.list", json!({}))
        .ok()
        .and_then(|v| v.as_array().cloned())
        .into_iter()
        .flatten()
        .map(|fx| {
            json!({
                "id": fx["id"],
                "name": fx["name"],
                "category": fx.get("folder").or_else(|| fx.get("category")).unwrap_or(&Value::Null),
                "kind": fx["kind"]
            })
        })
        .collect();
    let presets = fc
        .execute("presets.list", json!({}))
        .ok()
        .unwrap_or(Value::Null);
    json!({
        "effects": effects,
        "presets": if presets.is_array() { presets } else { json!([]) }
    })
}

pub(crate) fn runtime_words_tools(lib: &Value) -> Value {
    let move_meta = |m: &'static str| -> (&'static str, &'static str, &'static str, &'static str) {
        match m {
            "quiet" => ("quiet", "Quiet", "Fades in and holds", "•"),
            "sleep" => ("sleep", "Sleep", "Letters sink one by one", "☾"),
            "settle" => (
                "settle",
                "Sparkle Settle",
                "Drops and squashes into place",
                "★",
            ),
            "fall" => ("fall", "Kinetic Tilt", "Drops down angled", "📐"),
            "pop" => ("pop", "Bounce / Pop", "A quick swell on the beat", "⚡"),
            "rise" => ("rise", "Fire / Impact", "Arcs up and away", "🔥"),
            "east" => ("east", "Neon Drift", "Slides right", "✨"),
            "west" => ("west", "Slide West", "Slides left", "←"),
            "loop" => ("loop", "Wave / Loop", "Makes one continuous loop", "〰"),
            _ => (m, m, "Motion act", "⚡"),
        }
    };
    let moves: Vec<Value> = [
        "quiet", "pop", "loop", "fall", "rise", "east", "west", "settle", "sleep",
    ]
    .into_iter()
    .map(|key| {
        let (id, name, does, icon) = move_meta(key);
        let desc = lib["acts"][id]["does"].as_str().unwrap_or(does);
        json!({"id": id, "name": name, "does": desc, "icon": icon})
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
    let easings = json!([
        {"id": "spring-out", "name": "Spring Out", "does": "Elastic overshoot settle"},
        {"id": "elastic-pop", "name": "Elastic Pop", "does": "Snappy beat bounce"},
        {"id": "cubic-bezier", "name": "Cubic Bezier", "does": "Smooth cinematic ease"},
        {"id": "step-hold", "name": "Step Hold", "does": "Posterized frame hold"}
    ]);
    json!({
        "moves": moves,
        "letterings": letterings,
        "strokes": strokes,
        "choreographies": choreographies,
        "easings": easings,
        "actions": ACTIONS,
        "entrances": ENTRANCES,
        "exits": EXITS,
        "effectcraft": runtime_effectcraft_tools(),
        "filmcraft": runtime_filmcraft_tools()
    })
}

pub fn cmd_words_effects(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::cmd::words::workspace_dir(s)?;
    let hf = dir.join("hyperframes");
    let cli = crate::cmd::film::video_target::resolve_video_cli(
        p["resolve_video_cli"].as_str(),
        Some(&hf),
    )?;
    let wanted = p["kind"].as_str();
    if let Some(kind) = wanted
        && !EFFECT_KINDS.iter().any(|(k, _)| *k == kind)
    {
        return Err(invalid(format!(
            "kind must be one of {}",
            EFFECT_KINDS
                .iter()
                .map(|(k, _)| *k)
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    let mut kinds = serde_json::Map::new();
    for (kind, query) in EFFECT_KINDS
        .iter()
        .filter(|(k, _)| wanted.is_none_or(|w| w == *k))
    {
        let out = crate::workspace::run_tool_in(
            &hf,
            &cli.to_string_lossy(),
            &[
                "catalog".into(),
                "--query".into(),
                (*query).into(),
                "--json".into(),
            ],
        )?;
        let found: Value = serde_json::from_str(&out).map_err(|e| {
            invalid(format!(
                "HyperFrames catalog answered with something other than JSON: {e}"
            ))
        })?;
        let effects: Vec<Value> = found["results"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|r| json!({"name":r["name"],"type":r["type"],"title":r["title"],"description":r["description"],"tags":r["tags"]}))
            .collect();
        kinds.insert(
            (*kind).into(),
            json!({"query":query,"tier":found["tier"],"effects":effects}),
        );
    }
    let kinetic_library: Value =
        serde_json::from_str(KINETIC_LIBRARY).map_err(|e| invalid(e.to_string()))?;
    Ok(json!({
        "source": "hyperframes registry + craft engines",
        "kinds": kinds,
        "tools": runtime_words_tools(&kinetic_library)
    }))
}

fn append_kinetic_cues(
    anim: &Value,
    words: &[Value],
    lib: &Value,
    anchor: f64,
    section_end: f64,
    cues: &mut Vec<Value>,
) {
    let k = &anim["kinetic"];
    for (i, d) in k["draws"].as_array().into_iter().flatten().enumerate() {
        let Some(word) = words.iter().find(|w| w["word_id"] == d["word_id"]) else {
            continue;
        };
        let Some(word_anchor) = word["start"].as_f64() else {
            continue;
        };
        let start = (word_anchor + d["offset"].as_f64().unwrap_or(0.0)).max(0.0);
        let Some(stroke) = d
            .get("drawing")
            .filter(|v| v.is_object())
            .or_else(|| d["stroke"].as_str().and_then(|key| lib["strokes"].get(key)))
        else {
            continue;
        };
        if d["track"] == "lettering" || (d["track"].is_null() && stroke["track"] == "lettering") {
            continue;
        }
        let duration = d["duration"]
            .as_f64()
            .or_else(|| stroke["draw"].as_f64())
            .unwrap_or(1.0);
        let end = start + duration + d["hold"].as_f64().unwrap_or(1.0);
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
            "anchor": word_anchor,
            "color": d.get("color").unwrap_or(&k["style"]["hero_color"]),
            "start": start,
            "end": end,
            "draw_end": start + duration
        }));
    }
    if let Some(key) = k["overlay"].as_str().filter(|key| *key != "none")
        && let Some(overlay) = lib["overlays"].get(key)
    {
        let start = (anchor + k["overlay_offset"].as_f64().unwrap_or(0.0)).max(0.0);
        let duration = k["overlay_duration"]
            .as_f64()
            .unwrap_or(section_end - anchor);
        cues.push(json!({
            "id": format!("{}-overlay", anim["animation_id"].as_str().unwrap_or("words")),
            "kind": "overlay",
            "animation_id": anim["animation_id"],
            "name": overlay["name"],
            "overlay": key,
            "anchor": anchor,
            "start": start,
            "end": start + duration,
            "word_ids": []
        }));
    }
}

fn append_static_design_cues(
    anim: &Value,
    words: &[Value],
    section_end: f64,
    cues: &mut Vec<Value>,
) {
    for (line_id, design) in anim["design"].as_object().into_iter().flatten() {
        let line: Vec<_> = words.iter().filter(|w| w["line_id"] == *line_id).collect();
        let Some(start) = line.first().and_then(|w| w["start"].as_f64()) else {
            continue;
        };
        let end = words
            .iter()
            .find(|w| w["line_id"] != *line_id && w["start"].as_f64().is_some_and(|t| t > start))
            .and_then(|w| w["start"].as_f64())
            .unwrap_or(section_end);
        for (i, d) in design["draws"].as_array().into_iter().flatten().enumerate() {
            let paths = d["paths"].as_array().into_iter().flatten();
            let from = paths
                .clone()
                .filter_map(|p| p["from"].as_f64())
                .reduce(f64::min)
                .unwrap_or(0.0);
            let to = paths
                .clone()
                .filter_map(|p| p["to"].as_f64())
                .reduce(f64::max)
                .unwrap_or(from);
            cues.push(json!({
                "id": format!("{}-{line_id}-draw-{i}", anim["animation_id"].as_str().unwrap_or("words")),
                "kind": "drawing",
                "animation_id": anim["animation_id"],
                "line_id": line_id,
                "word_ids": line.iter().map(|w| w["word_id"].clone()).collect::<Vec<_>>(),
                "name": d["name"],
                "stroke": {"view_box": d["view_box"], "d": paths.filter_map(|p| p["d"].as_str()).collect::<Vec<_>>().join(" ")},
                "start": start + from,
                "end": end,
                "draw_end": (start + to).min(end)
            }));
        }
    }
}

fn hydrate_animation_times(
    anim: &mut Value,
    times: &std::collections::HashMap<String, (f64, f64)>,
) {
    let scope = anim["scope"].clone();
    for w in anim["words"].as_array_mut().into_iter().flatten() {
        if let Some((a, b)) = w["word_id"]
            .as_str()
            .and_then(|id| times.get(&word_time_key(&scope, id)))
        {
            w["start"] = json!(a);
            w["end"] = json!(b);
        }
    }
    fill_untimed(anim);
    timing(anim);
}

fn graphic_cues(
    doc: &Value,
    lib: &Value,
    times: &std::collections::HashMap<String, (f64, f64)>,
) -> Vec<Value> {
    let mut cues = Vec::new();
    for source in doc["animations"].as_array().into_iter().flatten() {
        let mut anim = source.clone();
        hydrate_animation_times(&mut anim, times);
        let words = anim["words"].as_array().cloned().unwrap_or_default();
        let anchor = anim["song_anchor"].as_f64().unwrap_or(0.0);
        let section_end = anchor + anim["authored_duration"].as_f64().unwrap_or(0.0);
        if anim["kinetic"].is_object() {
            append_kinetic_cues(&anim, &words, lib, anchor, section_end, &mut cues);
        } else {
            append_static_design_cues(&anim, &words, section_end, &mut cues);
        }
    }
    cues.sort_by(|a, b| {
        a["start"]
            .as_f64()
            .partial_cmp(&b["start"].as_f64())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
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
    let video = fs::read_to_string(dir.join("hyperframes/index.html")).unwrap_or_default();
    let in_video: Vec<&str> = video
        .split("data-magic-words=\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let kinetic_library: Value =
        serde_json::from_str(KINETIC_LIBRARY).map_err(|e| invalid(e.to_string()))?;
    let tools = runtime_words_tools(&kinetic_library);
    let graphics = graphic_cues(&doc, &kinetic_library, &studio_times(s));
    let images = project_backdrop_images(s, &dir);
    Ok(json!({
        "images": images,
        "graphics": graphics,
        "ok": true,
        "saved": saved,
        "reference_changed": stale,
        "path": document_path(&dir),
        "document": doc,
        "in_video": in_video,
        "fonts": video_fonts(&dir.join("hyperframes"), &video),
        "kinetic_library": kinetic_library,
        "tools": tools
    }))
}

fn video_fonts(hf: &Path, video: &str) -> Vec<Value> {
    video
        .split("@font-face")
        .skip(1)
        .filter_map(|rule| {
            let rule = rule.split('}').next()?;
            let family = rule
                .split("font-family:")
                .nth(1)?
                .split([';', '}'])
                .next()?
                .trim()
                .trim_matches(['"', '\''])
                .to_owned();
            let url = rule
                .split("url(")
                .nth(1)?
                .split(')')
                .next()?
                .trim()
                .trim_matches(['"', '\'']);
            let weight = rule
                .split("font-weight:")
                .nth(1)
                .and_then(|w| w.split([';', '}']).next())
                .map(str::trim)
                .unwrap_or("400");
            Some(json!({"family":family,"path":hf.join(url),"weight":weight}))
        })
        .collect()
}

pub(crate) fn one_of(value: &Value, allowed: &[&str], what: &str) -> Result<Value> {
    value
        .as_str()
        .filter(|v| allowed.contains(v))
        .map(|v| json!(v))
        .ok_or_else(|| invalid(format!("{what} must be one of {}", allowed.join(", "))))
}

fn apply_style_edits(anim: &mut Value, style: &Value) -> Result<()> {
    let map = style
        .as_object()
        .ok_or_else(|| invalid("style must be an object"))?;
    for (key, value) in map {
        let valid = match key.as_str() {
            "font" | "color" | "emphasis_color" | "outline_color" | "background" => {
                value.is_string()
            }
            "size" => value.as_f64().is_some_and(|v| (40.0..=320.0).contains(&v)),
            _ => false,
        };
        if !valid {
            return Err(invalid(format!(
                "style.{key} is not an editable style field or has an invalid value"
            )));
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
        "emphasis" => value
            .as_bool()
            .map(|b| json!(b))
            .ok_or_else(|| invalid("emphasis must be true or false")),
        "action" => one_of(value, ACTIONS, "action"),
        "entrance" => one_of(value, ENTRANCES, "entrance"),
        "exit" => one_of(value, EXITS, "exit"),
        other => Err(invalid(format!(
            "{other} is not editable here; text and timing come from the artist's sheet and the reference"
        ))),
    }
}

fn apply_word_edits(anim: &mut Value, id: &str, edits: &[Value]) -> Result<()> {
    for edit in edits {
        let word_id = edit["word_id"]
            .as_str()
            .ok_or_else(|| invalid("each word edit needs word_id"))?;
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
    for (line_id, design) in designs
        .as_object()
        .ok_or_else(|| invalid("design must be an object of line_id: design"))?
    {
        let words: Vec<Value> = anim["words"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|w| w["line_id"] == json!(line_id))
            .cloned()
            .collect();
        if words.is_empty() {
            return Err(invalid(format!("{line_id} is not a line of {id}")));
        }
        anim["design"][line_id] = if design.is_null() {
            Value::Null
        } else {
            line_design(&words, design)?
        };
    }
    Ok(())
}

fn apply_design_and_effects(dir: &Path, anim: &mut Value, id: &str, p: &Value) -> Result<()> {
    if let Some(designs) = p.get("design").filter(|v| !v.is_null()) {
        apply_line_designs(anim, id, designs)?;
    }
    if let Some(effects) = p.get("effects").filter(|v| !v.is_null()) {
        for (track, choice) in effects
            .as_object()
            .ok_or_else(|| invalid("effects must be an object of track: effect"))?
        {
            if track != "text" {
                return Err(invalid(format!(
                    "effects.{track}: only the text track takes registry effects so far"
                )));
            }
            anim["effects"][track] = if choice.is_null() {
                Value::Null
            } else {
                text_effect(&dir.join("hyperframes"), choice)?
            };
        }
    }
    Ok(())
}

fn apply_background_edit(
    s: &Session,
    dir: &Path,
    anim: &mut Value,
    background: &Value,
) -> Result<()> {
    if background.is_null() {
        anim["background_asset"] = Value::Null;
        anim["background_custom"] = Value::Null;
        return Ok(());
    }
    let asset_id = background
        .as_str()
        .ok_or_else(|| invalid("background_asset must be a project image id"))?;
    let valid = project_backdrop_images(s, dir)
        .iter()
        .any(|a| a["id"].as_str() == Some(asset_id));
    if !valid {
        return Err(invalid("Choose an existing project image for this verse"));
    }
    anim["background_asset"] = json!(asset_id);
    anim["background_custom"] = json!(true);
    Ok(())
}

fn apply_save_payload(
    s: &Session,
    dir: &Path,
    anim: &mut Value,
    id: &str,
    p: &Value,
) -> Result<()> {
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
        anim["kinetic"] = if k.is_null() {
            Value::Null
        } else {
            kinetic_design(&anim["words"], k)?
        };
    }
    Ok(())
}

pub fn cmd_words_save(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::cmd::words::workspace_dir(s)?;
    let id = p["animation_id"]
        .as_str()
        .ok_or_else(|| invalid("animation_id is required"))?;
    let expected = p["expected_source_revision"]
        .as_u64()
        .ok_or_else(|| invalid("expected_source_revision is required"))?;
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
    anim["origin"] = p
        .get("origin")
        .filter(|v| v.is_string())
        .cloned()
        .unwrap_or(json!("magic"));
    let saved = anim.clone();
    save(s, &dir, &doc)?;
    Ok(json!({"ok":true,"conflict":false,"path":document_path(&dir),"animation":saved}))
}

fn find_section_indices(doc: &Value, source: &str, target: &str) -> Result<(usize, usize)> {
    let list = doc["animations"]
        .as_array()
        .ok_or_else(|| invalid("No lyric sections"))?;
    let si = list
        .iter()
        .position(|a| a["animation_id"] == source)
        .ok_or_else(|| invalid("No source section"))?;
    let ti = list
        .iter()
        .position(|a| a["animation_id"] == target)
        .ok_or_else(|| invalid("No target section"))?;
    Ok((si, ti))
}

fn move_drawing_between_sections(
    doc: &mut Value,
    si: usize,
    ti: usize,
    index: usize,
    drawing: Value,
) -> Result<usize> {
    let target_index = if si == ti {
        doc["animations"][si]["kinetic"]["draws"][index] = drawing;
        index
    } else {
        doc["animations"][si]["kinetic"]["draws"]
            .as_array_mut()
            .ok_or_else(|| invalid("Source drawings are not an array"))?
            .remove(index);
        if !doc["animations"][ti]["kinetic"]["draws"].is_array() {
            doc["animations"][ti]["kinetic"]["draws"] = json!([]);
        }
        let draws = doc["animations"][ti]["kinetic"]["draws"]
            .as_array_mut()
            .ok_or_else(|| invalid("Target drawings are not an array"))?;
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

fn resolve_target_word_time(
    doc: &Value,
    ti: usize,
    word: &str,
    times: &std::collections::HashMap<String, (f64, f64)>,
) -> Result<f64> {
    let w = doc["animations"][ti]["words"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|w| w["word_id"] == word)
        .ok_or_else(|| invalid("The target word is not in this section"))?;
    let at = times
        .get(&word_time_key(&doc["animations"][ti]["scope"], word))
        .map(|t| t.0)
        .or_else(|| w["start"].as_f64())
        .ok_or_else(|| invalid("The target word has no known timing"))?;
    if !doc["animations"][ti]["kinetic"].is_object() {
        return Err(invalid("Choose a word in a kinetic lyric section"));
    }
    Ok(at)
}

fn check_link_revisions(doc: &Value, si: usize, ti: usize, p: &Value) -> Result<bool> {
    let expected = p["expected_source_revision"]
        .as_u64()
        .ok_or_else(|| invalid("expected_source_revision is required"))?;
    let target_expected = if si == ti {
        expected
    } else {
        p["expected_target_revision"]
            .as_u64()
            .ok_or_else(|| invalid("expected_target_revision is required"))?
    };
    Ok(
        doc["animations"][si]["source_revision"].as_u64() == Some(expected)
            && doc["animations"][ti]["source_revision"].as_u64() == Some(target_expected),
    )
}

fn prepare_relinked_drawing(
    doc: &Value,
    si: usize,
    index: usize,
    word: &str,
    cue_start: f64,
    at: f64,
) -> Result<Value> {
    let mut drawing = doc["animations"][si]["kinetic"]["draws"]
        .as_array()
        .and_then(|d| d.get(index))
        .cloned()
        .ok_or_else(|| invalid("Drawing index is stale"))?;
    drawing["word_id"] = json!(word);
    drawing["offset"] = json!(cue_start - at);
    if drawing["color"].is_null() {
        drawing["color"] = doc["animations"][si]["kinetic"]["style"]["hero_color"].clone();
    }
    Ok(drawing)
}

fn parse_link_request(p: &Value) -> Result<(&str, &str, &str, usize)> {
    let source = p["animation_id"]
        .as_str()
        .ok_or_else(|| invalid("animation_id is required"))?;
    let target = p["target_animation_id"].as_str().unwrap_or(source);
    let word = p["word_id"]
        .as_str()
        .ok_or_else(|| invalid("word_id is required"))?;
    let index = p["index"]
        .as_u64()
        .ok_or_else(|| invalid("drawing index is required"))? as usize;
    Ok((source, target, word, index))
}

fn find_source_cue_start(
    s: &mut Session,
    doc: &Value,
    source: &str,
    index: usize,
    ti: usize,
    word: &str,
) -> Result<(f64, f64)> {
    let lib: Value = serde_json::from_str(KINETIC_LIBRARY).map_err(|e| invalid(e.to_string()))?;
    let times = studio_times(s);
    let cues = graphic_cues(doc, &lib, &times);
    let cue = cues
        .iter()
        .find(|c| c["animation_id"] == source && c["index"].as_u64() == Some(index as u64))
        .ok_or_else(|| invalid("Select a drawing clip to relink"))?;
    let at = resolve_target_word_time(doc, ti, word, &times)?;
    let cue_start = cue["start"].as_f64().unwrap_or(at);
    Ok((cue_start, at))
}

pub fn cmd_words_link(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::cmd::words::workspace_dir(s)?;
    let mut doc = load(s, &dir)?.ok_or_else(|| invalid("Save this lyric design first"))?;
    let (source, target, word, index) = parse_link_request(p)?;
    let (si, ti) = find_section_indices(&doc, source, target)?;
    if !check_link_revisions(&doc, si, ti, p)? {
        return Ok(
            json!({"ok":false,"conflict":true,"reason":"A section changed; nothing relinked"}),
        );
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
        "start": cue_start,
        "changed_sections": if si == ti { vec![source] } else { vec![source, target] }
    }))
}
