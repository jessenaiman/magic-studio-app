//! Words composition and HyperFrames video placement (`words.compose`, `words.apply`, `words.design`, `words.track`).
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

use super::words::{
    HOLD, KINETIC_LIBRARY, aligned, captions_to_video, cmd_words_read, cmd_words_save,
    cmd_words_studio_track, fill_untimed, invalid, printed_caption_words, studio_times, tag_attr,
    timing, word_time_key,
};
pub use crate::words_validate::cmd_words_effect_install;
pub(crate) use crate::words_validate::{kinetic_design, line_design, text_effect};

fn kinetic_composition(
    anim: &Value,
    composition_id: &str,
    width: u64,
    height: u64,
    duration: f64,
    words: &[Value],
    layer: &str,
) -> Result<String> {
    let lib: Value = serde_json::from_str(KINETIC_LIBRARY).map_err(|e| invalid(e.to_string()))?;
    let data = json!({"width":width,"height":height,"duration":duration,"layer":layer,"words":words,"kinetic":anim["kinetic"],"style":anim["kinetic"]["style"],"library":lib});
    Ok(
        include_str!("../../../pipeline/templates/words-kinetic.html")
            .replace("__COMPOSITION_ID__", composition_id)
            .replace("__WIDTH__", &width.to_string())
            .replace("__HEIGHT__", &height.to_string())
            .replace("__DURATION__", &format!("{duration:.6}"))
            .replace(
                "__DATA__",
                &serde_json::to_string(&data)
                    .map_err(|e| invalid(e.to_string()))?
                    .replace("</", "<\\/"),
            ),
    )
}

fn group_words_by_line(anim: &Value) -> Vec<(String, Vec<&Value>)> {
    let mut lines: Vec<(String, Vec<&Value>)> = Vec::new();
    for w in anim["words"].as_array().into_iter().flatten() {
        let line = w["line_id"].as_str().unwrap_or_default().to_owned();
        match lines.last_mut() {
            Some((id, group)) if *id == line => group.push(w),
            _ => lines.push((line, vec![w])),
        }
    }
    lines
}

fn effect_composition(
    anim: &Value,
    composition_id: &str,
    width: u64,
    height: u64,
    duration: f64,
    local: &dyn Fn(f64) -> f64,
) -> Result<String> {
    let effect = &anim["effects"]["text"];
    let name = effect["name"].as_str().unwrap_or_default();
    let lines = group_words_by_line(anim);
    let attr = |v: &str| {
        v.replace('&', "&amp;")
            .replace('\'', "&#39;")
            .replace('<', "&lt;")
    };
    let mut mounts = String::new();
    for (i, (_, words)) in lines.iter().enumerate() {
        let first_word = words.first().ok_or_else(|| invalid("Empty lyric line"))?;
        let start = local(first_word["start"].as_f64().unwrap_or(0.0));
        let last_word = words.last().ok_or_else(|| invalid("Empty lyric line"))?;
        let last = local(last_word["end"].as_f64().unwrap_or(0.0)) + HOLD;
        let end = lines.get(i + 1).map_or(last, |(_, next)| {
            last.min(local(next[0]["start"].as_f64().unwrap_or(last)))
        });
        let text = words
            .iter()
            .filter_map(|w| w["text"].as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let mut values = effect["variables"].as_object().cloned().unwrap_or_default();
        values.insert(
            effect["text_variable"].as_str().unwrap_or("text").into(),
            json!(text),
        );
        if effect["cues"] == true {
            let cues: Vec<String> = words
                .iter()
                .filter_map(|w| w["start"].as_f64())
                .map(|t| format!("{:.3}", (local(t) - start).max(0.0)))
                .collect();
            values.insert("cues".into(), json!(cues.join(",")));
        }
        mounts.push_str(&format!(
            "  <div class=\"line\" data-composition-id=\"{name}\" data-composition-src=\"../components/{name}.html\" data-start=\"{start:.6}\" data-duration=\"{:.6}\" data-variable-values='{}'></div>\n",
            (end - start).max(0.1),
            attr(&Value::Object(values).to_string())
        ));
    }
    Ok(format!(
        "<!doctype html>\n<html lang=\"en\">\n<head><meta charset=\"UTF-8\" /></head>\n<body>\n<!-- Derived by `magic-cli exec words.compose` from design/words.json with the {name} effect; regenerate rather than hand-edit. -->\n\
<div data-composition-id=\"{composition_id}\" data-width=\"{width}\" data-height=\"{height}\" data-duration=\"{duration:.6}\" class=\"words-track\">\n\
  <style>[data-composition-id=\"{composition_id}\"] {{ position: relative; width: {width}px; height: {height}px; overflow: hidden; }}\n\
  [data-composition-id=\"{composition_id}\"] > .line {{ position: absolute; left: 6%; right: 6%; top: 62%; height: 26%; }}</style>\n{mounts}</div>\n</body>\n</html>\n"
    ))
}

fn design_composition(
    anim: &Value,
    composition_id: &str,
    width: u64,
    height: u64,
    duration: f64,
    local: &dyn Fn(f64) -> f64,
) -> Result<String> {
    let lines = group_words_by_line(anim);
    let start_of = |w: &Value| local(w["start"].as_f64().unwrap_or(0.0));
    let mut out = Vec::new();
    for (i, (line_id, words)) in lines.iter().enumerate() {
        let start = start_of(words[0]);
        let last_word = words.last().ok_or_else(|| invalid("Empty lyric line"))?;
        let end = lines.get(i + 1).map_or(
            local(last_word["end"].as_f64().unwrap_or(0.0)) + HOLD,
            |(_, next)| start_of(next[0]),
        );
        let design = &anim["design"][line_id];
        if !design.is_object() {
            let text = words
                .iter()
                .filter_map(|w| w["text"].as_str())
                .collect::<Vec<_>>()
                .join(" ");
            out.push(json!({"start":start,"end":end,"plain":true,"text":text}));
            continue;
        }
        let items: Vec<Value> = design["items"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|it| {
                let accent = it["accent"]
                    .as_str()
                    .and_then(|id| words.iter().find(|w| w["word_id"] == json!(id)))
                    .map_or(start, |w| start_of(w));
                let mut it = it.clone();
                it["accent"] = json!(accent);
                it["text"] = json!(
                    it["word_ids"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|id| words
                            .iter()
                            .find(|w| w["word_id"] == *id)
                            .and_then(|w| w["text"].as_str()))
                        .collect::<Vec<_>>()
                        .join(" ")
                );
                it
            })
            .collect();
        out.push(json!({"start":start,"end":end,"items":items,"draws":design["draws"]}));
    }
    let style = &anim["style"];
    let data = json!({"composition_id":composition_id,"duration":duration,"font":style["font"],"size":style["size"].as_f64().unwrap_or(96.0),
        "color":style["color"].as_str().unwrap_or("#fff6e0"),"lines":out});
    Ok(
        include_str!("../../../pipeline/templates/words-design.html")
            .replace("__COMPOSITION_ID__", composition_id)
            .replace("__WIDTH__", &width.to_string())
            .replace("__HEIGHT__", &height.to_string())
            .replace("__DURATION__", &format!("{duration:.6}"))
            .replace(
                "__DATA__",
                &serde_json::to_string(&data)
                    .map_err(|e| invalid(e.to_string()))?
                    .replace("</", "<\\/"),
            ),
    )
}

fn without_placement(html: &str, id: &str) -> String {
    let mark = format!("data-magicstudio-words=\"{id}\"");
    let mut out = html.to_owned();
    for (open, close) in [("<style ", "</style>\n"), ("<div ", "</div>\n"), ("<img ", ">\n")] {
        let mut scan = 0;
        while let Some(at) = out[scan..].find(&mark).map(|i| scan + i) {
            let Some(begin) = out[..at].rfind(open).filter(|&b| !out[b..at].contains('>')) else {
                scan = at + mark.len();
                continue;
            };
            let Some(end) = out[at..].find(close).map(|e| at + e + close.len()) else {
                scan = at + mark.len();
                continue;
            };
            out.replace_range(begin..end, "");
            scan = begin;
        }
    }
    out
}

fn build_kinetic_for_design(a: &Value, recipe_doc: &Value, p: &Value, k: usize) -> Value {
    let kin = &recipe_doc["kinetic"];
    let pick = |list: &Value, key: &str| {
        p[key].as_str().map(|v| json!(v)).or_else(|| {
            list.as_array()
                .filter(|l| !l.is_empty())
                .map(|l| l[(p["seed"].as_u64().unwrap_or(0) as usize + k * 7) % l.len()].clone())
        })
    };
    let acts: serde_json::Map<String, Value> = a["words"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|w| {
            let key: String = w["text"]
                .as_str()?
                .chars()
                .filter(|c| c.is_alphabetic())
                .flat_map(char::to_lowercase)
                .collect();
            kin["acts"][&key]
                .as_str()
                .map(|act| (w["word_id"].as_str().unwrap_or_default().to_owned(), json!(act)))
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

fn run_lyrics_layout(s: &Session, dir: &Path, recipe: &Path, doc: &Value, times: &std::collections::HashMap<String, (f64, f64)>) -> Result<Value> {
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
                        w["start"] = json!(t.0);
                    }
                    w
                })
                .collect();
            json!({"animation_id":a["animation_id"],"section":a["scope"]["section"],"words":words})
        })
        .collect();
    let input = dir.join(".agent/words-design-input.json");
    if let Some(parent) = input.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &input,
        serde_json::to_vec(&json!({"animations":animations})).map_err(|e| invalid(e.to_string()))?,
    )?;
    let root = crate::workspace::workspace_root(s);
    let out = crate::workspace::run_tool_in(
        &root,
        "python3",
        &[
            root.join("pipeline/lyrics_layout.py").to_string_lossy().into_owned(),
            "--recipe".into(),
            recipe.to_string_lossy().into_owned(),
            "--input".into(),
            input.to_string_lossy().into_owned(),
        ],
    )?;
    out.lines()
        .rev()
        .find_map(|l| serde_json::from_str::<Value>(l).ok())
        .map(|v| v["designs"].clone())
        .ok_or_else(|| EngineError::CommandFailed("words.design".into(), "lyrics_layout.py returned no designs".into()))
}

fn design_one_section(
    s: &mut Session,
    a: &Value,
    k: usize,
    style: &Value,
    designs: &Value,
    recipe_doc: &Value,
    p: &Value,
) -> Result<Value> {
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
    let applied = if p["apply"] == false {
        Value::Null
    } else {
        cmd_words_apply(s, &json!({"animation_id":id}))?["in_video"].clone()
    };
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
    let dir = crate::workshop::project_dir(s)?;
    let recipe = p["recipe"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| dir.join("lyrics-design.json"));
    if !recipe.is_file() {
        return Err(invalid(format!(
            "No lyric recipe at {}; write one (see pipeline/lyrics_layout.py)",
            recipe.display()
        )));
    }
    let doc = cmd_words_read(s, &json!({}))?["document"].clone();
    let times = studio_times(s);
    let designs = run_lyrics_layout(s, &dir, &recipe, &doc, &times)?;
    let recipe_doc: Value = serde_json::from_slice(&fs::read(&recipe)?)
        .map_err(|e| invalid(format!("lyric recipe: {e}")))?;
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
    let captions = if p["apply"] != false && dir.join("hyperframes/compositions/captions.html").is_file() {
        let aligned = aligned(s)?;
        captions_to_video(s, &dir, &aligned)?
    } else {
        Value::Null
    };
    Ok(json!({
        "ok": true,
        "recipe": recipe,
        "captions": captions,
        "sections": sections,
        "lines": sections.iter().filter_map(|x| x["lines"].as_u64()).sum::<u64>(),
        "next": "open Design to adjust any line, or hyperframes snapshot <project>/hyperframes --at <t> to check the video"
    }))
}

/// `words.track`: the song's lyric line in the video, one clip per printed word at its sung time.
pub fn cmd_words_track(s: &mut Session, _: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let index = dir.join("hyperframes/index.html");
    let html = fs::read_to_string(&index)
        .map_err(|_| invalid("This project has no HyperFrames video yet; run video.init first"))?;
    let (words, _) = printed_caption_words(&aligned(s)?);
    if words.is_empty() {
        return Err(invalid("No printed words with timing; check words.align"));
    }
    let mut line_end: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for w in &words {
        let e = line_end
            .entry(word_time_key(w, w["line"].as_str().unwrap_or_default()))
            .or_insert(0.0);
        *e = e.max(w["end"].as_f64().unwrap_or(0.0));
    }
    let esc = |t: &str| t.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;");
    let mut clips = String::new();
    for (i, w) in words.iter().enumerate() {
        let (id, text) = (
            w["word_id"].as_str().unwrap_or_default(),
            w["text"].as_str().unwrap_or_default(),
        );
        let start = w["start"].as_f64().unwrap_or(0.0);
        let end = line_end[&word_time_key(w, w["line"].as_str().unwrap_or_default())] + HOLD;
        clips.push_str(&format!(
            "<div id=\"kinetic-r{}-{}\" class=\"clip lyric-word-clip\" data-section=\"{}\" data-occurrence=\"{}\" data-start=\"{start:.6}\" data-duration=\"{:.6}\" data-track-index=\"{}\" data-word-id=\"{}\" aria-label=\"Sung word: {}\" style=\"opacity:0\">{}</div>\n",
            w["occurrence"].as_u64().unwrap_or(1),
            esc(&id.replace("/w", "-")),
            esc(w["section"].as_str().unwrap_or("")),
            w["occurrence"].as_u64().unwrap_or(1),
            (end - start).max(0.1),
            200 + i,
            esc(id),
            esc(text),
            esc(text)
        ));
    }
    let mut out = strip_lyric_word_clips(&html)?;
    let at = out
        .find("<audio")
        .or_else(|| out.rfind("</div>"))
        .ok_or_else(|| invalid("The video has no root composition to hold the lyric line"))?;
    let at = out[..at].rfind('\n').map_or(at, |n| n + 1);
    out.insert_str(at, &clips);
    fs::write(&index, out)?;
    Ok(json!({
        "ok": true,
        "clips": words.len(),
        "first": words.first(),
        "last": words.last(),
        "path": index,
        "next": "words.design (or the Design page) builds the lyric visuals on these anchors; retime words in the Studio"
    }))
}

fn strip_lyric_word_clips(html: &str) -> Result<String> {
    let mut out = html.to_owned();
    let mut scan = 0;
    while let Some(start) = out[scan..].find("<div").map(|i| scan + i) {
        let Some(tag_end) = out[start..].find('>').map(|i| start + i + 1) else {
            break;
        };
        let tag = &out[start..tag_end];
        if tag_attr(tag, "class").is_some_and(|v| v.split_whitespace().any(|c| c == "lyric-word-clip")) {
            let Some(end) = out[tag_end..].find("</div>").map(|i| tag_end + i + 6) else {
                return Err(invalid("Unclosed lyric anchor; nothing written"));
            };
            out.replace_range(start..end, "");
            scan = start;
        } else {
            scan = tag_end;
        }
    }
    Ok(out)
}

fn soften_webgpu_shader_guard(hf: &Path) {
    let path = hf.join("compositions/lib/shaders.iife.js");
    let Ok(js) = fs::read_to_string(&path) else {
        return;
    };
    let next = js
        .replace(
            "function Ws(e,t){if(e){o1.style.background=\"transparent\";if(o1.parentElement)o1.parentElement.style.background=\"transparent\";return;}",
            "function Ws(e,t){if(e)return;",
        )
        .replace(
            "let e=Ji.getFailureReason();if(e){o1.style.background=\"transparent\";if(o1.parentElement)o1.parentElement.style.background=\"transparent\";return;}",
            "let e=Ji.getFailureReason();if(e)return;",
        )
        .replace(
            "if(e){o1.style.background=\"transparent\";if(o1.parentElement)o1.parentElement.style.background=\"transparent\";return;}",
            "if(e)return;",
        )
        .replace(
            "Du=WE[o1.dataset.shader]",
            "Du=o1&&WE[o1.dataset.shader]",
        )
        .replace(
            "if(!Du)throw new Error(`shaders: no shader named \"${o1.dataset.shader}\" in this bundle`);",
            "if(!o1||!Du)return;",
        )
        .replace(
            "function OE(e){return T0=T0.then(()=>Ji.renderSyntheticFrame(e-Ji.__testing.getFrameDiagnostics().globalElapsedTime)),T0}",
            "function OE(e){return Ji.getFailureReason()?T0:(T0=T0.then(()=>Ji.renderSyntheticFrame(e-Ji.__testing.getFrameDiagnostics().globalElapsedTime)),T0)}",
        )
        .replace(
            "if(e)throw new Error(`shaders: WebGPU unavailable (${e})`);",
            "if(e)return;",
        );
    if next != js {
        let _ = fs::write(&path, next);
    }
    let _ = fs::remove_dir_all(hf.join(".hyperframes/preview"));
}

fn mount_overlay(
    s: &mut Session,
    dir: &Path,
    id: &str,
    read: &Value,
    html: &mut String,
    root: usize,
    track_index: u64,
    w: u64,
    h: u64,
) -> Result<Option<Value>> {
    let overlay = &read["document"]["animations"][0]["kinetic"]["overlay"];
    let lib: Value = serde_json::from_str(KINETIC_LIBRARY).map_err(|e| invalid(e.to_string()))?;
    let Some(block) = overlay.as_str().and_then(|o| lib["overlays"][o]["block"].as_str()) else {
        return Ok(None);
    };
    let look = &lib["overlays"][overlay.as_str().unwrap_or_default()];
    let installed = cmd_words_effect_install(s, &json!({"name":block}))?;
    soften_webgpu_shader_guard(&dir.join("hyperframes"));
    let own = format!("{id}-overlay");
    let src = installed["src"].as_str().unwrap_or_default();
    let own_src = Path::new(src)
        .with_file_name(format!("{own}.html"))
        .to_string_lossy()
        .into_owned();
    let source = fs::read_to_string(dir.join("hyperframes").join(src))?
        .replace(" data-requires-webgpu", "")
        .replace("background: #000", "background: transparent");
    let _ = fs::write(dir.join("hyperframes").join(src), &source);
    fs::write(
        dir.join("hyperframes").join(&own_src),
        source
            .replace(&format!("\"{block}\""), &format!("\"{own}\""))
            .replace(&format!("'{block}'"), &format!("'{own}'")),
    )?;
    let cue = read["graphics"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| c["kind"] == "overlay")
        .ok_or_else(|| invalid("Overlay timing is unavailable"))?;
    let overlay_start = cue["start"]
        .as_f64()
        .ok_or_else(|| invalid("Overlay start is unavailable"))?;
    let overlay_duration = cue["end"]
        .as_f64()
        .ok_or_else(|| invalid("Overlay end is unavailable"))?
        - overlay_start;
    html.insert_str(
        root,
        &format!(
            "<div id=\"{own}\" class=\"clip lyric-overlay-clip\" data-magicstudio-words=\"{id}\" data-composition-id=\"{own}\" data-composition-src=\"{own_src}\" data-start=\"{:.6}\" data-duration=\"{:.6}\" data-track-index=\"{}\" data-width=\"{w}\" data-height=\"{h}\" style=\"position:absolute;left:0;top:0;width:{w}px;height:{h}px;z-index:75;mix-blend-mode:{};opacity:{}\"></div>\n",
            overlay_start,
            overlay_duration,
            track_index + 1,
            look["blend"].as_str().unwrap_or("normal"),
            look["opacity"].as_f64().unwrap_or(1.0)
        ),
    );
    Ok(Some(json!({"name":overlay,"block":block,"src":own_src,"start":overlay_start,"duration":overlay_duration})))
}

fn section_backdrop_span(track: &Value, scope: &Value, composed: &Value, html: &str) -> (f64, f64) {
    let anchor = composed["song_anchor"].as_f64().unwrap_or(0.0);
    let dur = composed["authored_duration"].as_f64().unwrap_or(0.0);
    let secs = track["sections"].as_array();
    let pos = secs.and_then(|s| {
        s.iter()
            .position(|sec| sec["section"] == scope["section"] && sec["occurrence"] == scope["occurrence"])
    });
    let start = if pos == Some(0) { 0.0 } else { anchor };
    let next_start = match (secs, pos) {
        (Some(s), Some(i)) => s.get(i + 1).and_then(|n| n["start"].as_f64()),
        _ => None,
    };
    let comp_dur = html
        .split("data-duration=\"")
        .nth(1)
        .and_then(|v| v.split('"').next())
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(anchor + dur);
    let end = next_start.unwrap_or_else(|| comp_dur.max(anchor + dur));
    (start, (end - start).max(dur))
}

fn mount_section_backdrop(
    s: &Session,
    dir: &Path,
    id: &str,
    read: &Value,
    composed: &Value,
    track: &Value,
    html: &mut String,
    root: usize,
    w: u64,
    h: u64,
) -> Option<Value> {
    let anim = &read["document"]["animations"][0];
    let asset_id = anim["background_asset"].as_str()?;
    let name = crate::words_stage::stage_backdrop_file(s, dir, asset_id).ok()?;
    let (start, duration) = section_backdrop_span(track, &anim["scope"], composed, html);
    html.insert_str(
        root,
        &format!(
            "<img id=\"{id}-backdrop\" class=\"clip lyric-backdrop-clip\" data-magicstudio-words=\"{id}\" data-lyrics-backdrop src=\"compositions/words/backdrops/{name}\" data-start=\"{start:.6}\" data-duration=\"{duration:.6}\" data-track-index=\"1\" style=\"position:absolute;inset:0;width:{w}px;height:{h}px;object-fit:cover;z-index:1\">\n"
        ),
    );
    Some(json!({"asset_id":asset_id,"src":format!("compositions/words/backdrops/{name}"),"start":start,"duration":duration}))
}

fn insert_section_hide_style(html: &mut String, id: &str, clips: &[String], has_backdrop: bool) -> Result<()> {
    if clips.is_empty() && !has_backdrop {
        return Ok(());
    }
    let head = html
        .find("</head>")
        .ok_or_else(|| invalid("The video's index.html has no </head>; nothing written"))?;
    let mut selectors = clips.to_vec();
    if has_backdrop {
        selectors.push("#animation".to_owned());
    }
    selectors.push(".caption".to_owned());
    html.insert_str(
        head,
        &format!(
            "<style data-magicstudio-words=\"{id}\">{} {{ visibility: hidden !important; }}</style>\n",
            selectors.join(", ")
        ),
    );
    Ok(())
}

pub fn cmd_words_apply(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let index = dir.join("hyperframes/index.html");
    let id = p["animation_id"]
        .as_str()
        .filter(|i| i.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
        .ok_or_else(|| invalid("animation_id is required (letters, digits, '-' or '_')"))?;
    let original = fs::read_to_string(&index)
        .map_err(|_| invalid("This project has no HyperFrames video yet; run video.init first"))?;
    let mut html = without_placement(&original, id);
    let use_it = p["use"].as_bool().unwrap_or(true);
    let mut receipt = json!({"ok":true,"animation_id":id,"in_video":use_it});
    if use_it {
        let composed = cmd_words_compose(s, &json!({"animation_id":id}))?;
        let read = cmd_words_read(s, &json!({"animation_id":id}))?;
        let scope = &read["document"]["animations"][0]["scope"];
        let track = cmd_words_studio_track(s, &json!({}))?;
        let clips: Vec<String> = track["sections"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|sec| sec["section"] == scope["section"] && sec["occurrence"] == scope["occurrence"])
            .flat_map(|sec| sec["clips"].as_array().cloned().unwrap_or_default())
            .filter_map(|c| c["clip_id"].as_str().map(|c| format!("#{c}")))
            .collect();
        let has_backdrop = read["document"]["animations"][0]["background_asset"].is_string();
        insert_section_hide_style(&mut html, id, &clips, has_backdrop)?;
        let root = html
            .find("data-composition-id=")
            .and_then(|at| html[at..].find('>').map(|e| at + e + 1))
            .ok_or_else(|| invalid("The video has no root composition"))?;
        let track_index = html
            .split("data-track-index=\"")
            .skip(1)
            .filter_map(|r| r.split('"').next()?.parse::<u64>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        let (w, h) = (
            composed["width"].as_u64().unwrap_or(1920),
            composed["height"].as_u64().unwrap_or(1080),
        );
        let composition_id = composed["composition_id"].as_str().unwrap_or(id);
        html.insert_str(
            root,
            &format!(
                "<div id=\"{composition_id}\" class=\"clip lyric-design-clip\" data-magicstudio-words=\"{id}\" data-composition-id=\"{composition_id}\" data-composition-src=\"{}\" data-start=\"{:.6}\" data-duration=\"{:.6}\" data-track-index=\"{track_index}\" data-width=\"{w}\" data-height=\"{h}\" style=\"position:absolute;left:0;top:0;width:{w}px;height:{h}px;z-index:76\"></div>\n",
                composed["src"].as_str().unwrap_or_default(),
                composed["song_anchor"].as_f64().unwrap_or(0.0),
                composed["authored_duration"].as_f64().unwrap_or(0.0)
            ),
        );
        for (idx, draw) in composed["drawings"].as_array().into_iter().flatten().enumerate() {
            html.insert_str(
                root,
                &format!(
                    "<div id=\"{}\" class=\"clip lyric-drawing-clip\" data-magicstudio-words=\"{id}\" data-track-kind=\"graphics\" data-composition-id=\"{}\" data-composition-src=\"{}\" data-start=\"{:.6}\" data-duration=\"{:.6}\" data-track-index=\"{}\" data-width=\"{w}\" data-height=\"{h}\" style=\"position:absolute;inset:0;z-index:77\"></div>\n",
                    draw["composition_id"].as_str().unwrap_or_default(),
                    draw["composition_id"].as_str().unwrap_or_default(),
                    draw["src"].as_str().unwrap_or_default(),
                    draw["start"].as_f64().unwrap_or(0.0),
                    draw["duration"].as_f64().unwrap_or(0.0),
                    track_index + 2 + idx as u64
                ),
            );
        }
        receipt["drawings"] = composed["drawings"].clone();
        if let Some(ov) = mount_overlay(s, &dir, id, &read, &mut html, root, track_index, w, h)? {
            receipt["overlay"] = ov;
        }
        if let Some(bg) = mount_section_backdrop(s, &dir, id, &read, &composed, &track, &mut html, root, w, h) {
            receipt["backdrop"] = bg;
        }
        receipt["src"] = composed["src"].clone();
        receipt["start"] = composed["song_anchor"].clone();
        receipt["hidden_clips"] = json!(clips.len());
    }
    let _ = fs::remove_dir_all(dir.join("hyperframes/.hyperframes/preview"));
    if html != original || use_it {
        let temporary = index.with_extension("html.tmp");
        fs::write(&temporary, &html)?;
        fs::rename(&temporary, &index)?;
    }
    receipt["changed"] = json!(html != original);
    Ok(receipt)
}

fn composition_size(hf: &Path) -> (u64, u64) {
    let index = fs::read_to_string(hf.join("index.html")).unwrap_or_default();
    let attr = |name: &str| {
        index
            .split(&format!("{name}=\""))
            .nth(1)
            .and_then(|v| v.split('"').next())
            .and_then(|v| v.parse().ok())
    };
    (
        attr("data-width").unwrap_or(1920),
        attr("data-height").unwrap_or(1080),
    )
}

fn compose_drawing_subcompositions(
    hf: &Path,
    anim: &Value,
    read: &Value,
    composition_id: &str,
    width: u64,
    height: u64,
    anchor: f64,
    words: &[Value],
) -> Result<Vec<Value>> {
    if !anim["kinetic"].is_object() {
        return Ok(Vec::new());
    }
    let mut drawings = Vec::new();
    for cue in read["graphics"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["kind"] == "drawing")
    {
        let start = cue["start"].as_f64().unwrap_or(anchor);
        let end = cue["end"].as_f64().unwrap_or(start + 1.0);
        let index = cue["index"].as_u64().unwrap_or(0) as usize;
        let mut own_anim = anim.clone();
        own_anim["kinetic"]["draws"] = json!([anim["kinetic"]["draws"][index]]);
        let mut shifted = words.to_vec();
        for w in &mut shifted {
            for key in ["start", "end"] {
                w[key] = json!(w[key].as_f64().unwrap_or(0.0) + anchor - start);
            }
        }
        let own = format!("{composition_id}-drawing-{index}");
        let src = format!("compositions/words/{own}.html");
        fs::write(
            hf.join(&src),
            kinetic_composition(&own_anim, &own, width, height, end - start, &shifted, "drawings")?,
        )?;
        drawings.push(json!({"composition_id":own,"src":src,"start":start,"duration":end-start}));
    }
    Ok(drawings)
}

fn render_section_html(
    anim: &Value,
    composition_id: &str,
    width: u64,
    height: u64,
    duration: f64,
    words: &[Value],
    data: &Value,
    local: &dyn Fn(f64) -> f64,
) -> Result<String> {
    let designed = anim["design"]
        .as_object()
        .is_some_and(|d| d.values().any(Value::is_object));
    if anim["kinetic"].is_object() {
        kinetic_composition(anim, composition_id, width, height, duration, words, "text")
    } else if designed {
        design_composition(anim, composition_id, width, height, duration, local)
    } else if anim["effects"]["text"].is_object() {
        effect_composition(anim, composition_id, width, height, duration, local)
    } else {
        Ok(include_str!("../../../pipeline/templates/words-composition.html")
            .replace("__COMPOSITION_ID__", composition_id)
            .replace("__WIDTH__", &width.to_string())
            .replace("__HEIGHT__", &height.to_string())
            .replace("__DURATION__", &format!("{duration:.6}"))
            .replace(
                "__DATA__",
                &serde_json::to_string(data)
                    .map_err(|e| invalid(e.to_string()))?
                    .replace("</", "<\\/"),
            ))
    }
}

fn sync_studio_word_times(s: &mut Session, synced: &mut Value) {
    let track = cmd_words_studio_track(s, &json!({})).unwrap_or(Value::Null);
    let times = studio_times(s);
    let scope = synced["scope"].clone();
    for w in synced["words"].as_array_mut().into_iter().flatten() {
        if let Some((a, b)) = w["word_id"]
            .as_str()
            .and_then(|wid| times.get(&word_time_key(&scope, wid)))
        {
            w["start"] = json!(a);
            w["end"] = json!(b);
            w["timing_source"] = json!("studio");
        }
    }
    fill_untimed(synced);
    timing(synced);
    let next_start = track["sections"]
        .as_array()
        .and_then(|secs| {
            let pos = secs
                .iter()
                .position(|sec| sec["section"] == scope["section"] && sec["occurrence"] == scope["occurrence"])?;
            secs.get(pos + 1)?["start"].as_f64()
        });
    if let (Some(anchor), Some(ns)) = (synced["song_anchor"].as_f64(), next_start) {
        let cur_dur = synced["authored_duration"].as_f64().unwrap_or(0.0);
        synced["authored_duration"] = json!(cur_dur.min((ns - 0.04 - anchor).max(0.2)));
    }
}

fn build_local_words(anim: &Value, local: &dyn Fn(f64) -> f64) -> Result<Vec<Value>> {
    anim["words"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|w| {
            let start = w["start"]
                .as_f64()
                .ok_or_else(|| invalid("Word has no numeric start"))?;
            let end = w["end"]
                .as_f64()
                .ok_or_else(|| invalid("Word has no numeric end"))?;
            Ok(json!({
                "word_id": w["word_id"],
                "line_id": w["line_id"],
                "text": w["text"],
                "start": local(start),
                "end": local(end),
                "emphasis": w["emphasis"],
                "action": w["action"],
                "entrance": w["entrance"],
                "exit": w["exit"]
            }))
        })
        .collect()
}

/// `words.compose`: write the derived, untrimmed HyperFrames sub-composition for one animation.
pub fn cmd_words_compose(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let hf = dir.join("hyperframes");
    if !hf.join("index.html").is_file() {
        return Err(invalid(
            "This project has no HyperFrames project yet; run video.init first",
        ));
    }
    let id = p["animation_id"]
        .as_str()
        .ok_or_else(|| invalid("animation_id is required"))?;
    let read = cmd_words_read(s, &json!({"animation_id":id}))?;
    let mut synced = read["document"]["animations"][0].clone();
    sync_studio_word_times(s, &mut synced);
    let anim = &synced;
    let instance = match p["instance_id"].as_str() {
        Some(i)
            if !i.is_empty()
                && i.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') =>
        {
            Some(i)
        }
        Some(_) => return Err(invalid("instance_id must be letters, digits, '-' or '_'")),
        None => None,
    };
    let composition_id = instance.map_or_else(|| id.to_owned(), |i| format!("{id}--{i}"));
    let untimed: Vec<&str> = anim["words"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|w| !w["start"].is_number() || !w["end"].is_number())
        .filter_map(|w| w["word_id"].as_str())
        .collect();
    if !untimed.is_empty() {
        return Err(invalid(format!(
            "Words without timing cannot be placed: {}; resolve them with words.align",
            untimed.join(", ")
        )));
    }
    let anchor = anim["song_anchor"]
        .as_f64()
        .ok_or_else(|| invalid("Animation has no song_anchor"))?;
    let duration = anim["authored_duration"]
        .as_f64()
        .ok_or_else(|| invalid("Animation has no authored_duration"))?;
    let local = |t: f64| ((t - anchor) * 1e6).round() / 1e6;
    let words = build_local_words(anim, &local)?;
    let (width, height) = composition_size(&hf);
    let data = json!({
        "composition_id": composition_id,
        "animation_id": id,
        "source_revision": anim["source_revision"],
        "duration": duration,
        "hold": anim["hold"],
        "style": anim["style"],
        "words": words
    });
    let html = render_section_html(anim, &composition_id, width, height, duration, &words, &data, &local)?;
    let file = hf
        .join("compositions/words")
        .join(format!("{composition_id}.html"));
    fs::create_dir_all(
        file.parent()
            .ok_or_else(|| invalid("Output path has no parent"))?,
    )?;
    fs::write(&file, &html)?;
    let drawings = compose_drawing_subcompositions(&hf, anim, &read, &composition_id, width, height, anchor, &words)?;
    Ok(json!({
        "ok": true,
        "animation_id": id,
        "instance_id": instance,
        "composition_id": composition_id,
        "file": file,
        "drawings": drawings,
        "src": format!("compositions/words/{composition_id}.html"),
        "sha256": format!("{:x}", Sha256::digest(html.as_bytes())),
        "source_revision": anim["source_revision"],
        "authored_duration": duration,
        "song_anchor": anchor,
        "lead_in": anim["lead_in"],
        "hold": anim["hold"],
        "width": width,
        "height": height,
        "saved_source": read["saved"]
    }))
}
