//! `words.letter`: the song's lyrics lettered and animated by the linked EffectCraft engine. Magic owns the words, their timing and the approved recipe
//! (`lyrics-design.json`); EffectCraft owns layout, type, motion and rendering. The result
//! lives in the project folder: `lettering/lyrics.ecproj`, the commands that built it, and a
//! transparent render for the film to place over the backgrounds.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const COMP: &str = "Lyrics";
const FRAME: (f64, f64) = (1920.0, 1080.0);
/// Seconds a line appears before its first sung word, and lingers after its last.
const LEAD: f64 = 0.25;
const HOLD: f64 = 1.0;

fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

/// One sung line: its id, its printed words with their sung times, and its sung span.
struct Line {
    id: String,
    /// Layer-name stem, unique per sung occurrence: a repeated chorus line gets `-2`, `-3`, …
    /// EffectCraft renames duplicate layer names, so a shared name would miss later repeats.
    layer: String,
    words: Vec<(String, Option<(f64, f64)>)>,
    start: f64,
    end: f64,
}

impl Line {
    fn text(&self) -> String {
        self.words
            .iter()
            .map(|w| w.0.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Character range of word `k` within `text()`.
    fn range(&self, k: usize) -> (usize, usize) {
        let start: usize = self.words[..k]
            .iter()
            .map(|w| w.0.chars().count() + 1)
            .sum();
        (start, start + self.words[k].0.chars().count())
    }
}

/// The artist's lines in sung order, from the aligned words; lines with no timed word skip.
fn lines(aligned: &Value) -> Vec<Line> {
    let mut out: Vec<Line> = Vec::new();
    for w in aligned["words"].as_array().into_iter().flatten() {
        let id = w["sung_id"]
            .as_str()
            .or(w["line_id"].as_str())
            .unwrap_or_default();
        let time = w["start"].as_f64().zip(w["end"].as_f64());
        let word = (w["text"].as_str().unwrap_or_default().to_owned(), time);
        match out.last_mut() {
            Some(line) if line.id == id => line.words.push(word),
            _ => out.push(Line {
                id: id.to_owned(),
                layer: id.to_owned(),
                words: vec![word],
                start: f64::MAX,
                end: f64::MIN,
            }),
        }
    }
    for line in &mut out {
        for (s, e) in line.words.iter().filter_map(|w| w.1) {
            (line.start, line.end) = (line.start.min(s), line.end.max(e));
        }
    }
    out.retain(|l| l.start <= l.end);
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for line in &mut out {
        let n = seen.entry(line.id.clone()).or_insert(0);
        *n += 1;
        if *n > 1 {
            line.layer = format!("{}-{n}", line.id);
        }
    }
    out
}

fn key(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphabetic)
        .flat_map(char::to_lowercase)
        .collect()
}

/// The recipe's look for one group of words: font, style, size, fill and outline.
struct Look {
    font: String,
    style: &'static str,
    size: f64,
    fill: String,
    outline: Option<String>,
}

fn colour(recipe: &Value, name: &Value) -> Option<String> {
    let name = name.as_str()?;
    if name.starts_with('#') {
        return Some(name.to_owned());
    }
    recipe["palette"][name].as_str().map(str::to_owned)
}

fn look(recipe: &Value, spec: &Value) -> Look {
    let font =
        |kind: &str, default: &str| recipe["fonts"][kind].as_str().unwrap_or(default).to_owned();
    let (font, style) = match spec["lettering"].as_str().unwrap_or("sans-bold") {
        "serif-bold" => (font("serif", "C059"), "Bold"),
        "serif-italic" => (font("serif", "C059"), "Italic"),
        "serif" => (font("serif", "C059"), "Roman"),
        "sans" => (font("sans", "Nimbus Sans"), "Regular"),
        _ => (font("sans", "Nimbus Sans"), "Bold"),
    };
    Look {
        font,
        style,
        size: spec["size"].as_f64().unwrap_or(58.0),
        fill: colour(recipe, &spec["color"]).unwrap_or_else(|| "#000000".into()),
        outline: colour(recipe, &spec["outline"]),
    }
}

fn item_look(recipe: &Value, item: &Value) -> Look {
    let font_fn =
        |kind: &str, default: &str| recipe["fonts"][kind].as_str().unwrap_or(default).to_owned();
    let kind = item["font"].as_str().unwrap_or("sans");
    let bold = item["weight"].as_u64().unwrap_or(700) >= 700;
    let italic = item["italic"].as_bool().unwrap_or(false);
    let (font, style) = match (kind, bold, italic) {
        ("serif" | "song", _, true) => (font_fn("serif", "C059"), "Italic"),
        ("serif" | "song", true, false) => (font_fn("serif", "C059"), "Bold"),
        ("serif" | "song", false, false) => (font_fn("serif", "C059"), "Roman"),
        (_, true, _) => (font_fn("sans", "Nimbus Sans"), "Bold"),
        (_, false, _) => (font_fn("sans", "Nimbus Sans"), "Regular"),
    };
    Look {
        font,
        style,
        size: item["size"].as_f64().unwrap_or(58.0),
        fill: colour(recipe, &item["color"]).unwrap_or_else(|| "#173F49".into()),
        outline: colour(recipe, &item["outline"]),
    }
}

/// The first recipe hero in the line, if any: (word index, its spec).
fn hero<'a>(recipe: &'a Value, line: &Line) -> Option<(usize, &'a Value)> {
    if recipe["max_heroes"].as_u64() == Some(0) {
        return None;
    }
    line.words
        .iter()
        .enumerate()
        .find_map(|(k, w)| recipe["heroes"].get(key(&w.0)).map(|spec| (k, spec)))
}

/// The hero's motion as (animator property, keyframes of (comp seconds, value)). Every motion
/// arrives at the word's place and ends there, so a moving word never covers its neighbours.
fn motion(name: &str, start: f64, end: f64) -> Option<(&'static str, Vec<(f64, Value)>)> {
    let still = json!([0, 0, 0]);
    let arrive = (end - start).clamp(0.3, 0.6);
    Some(match name {
        "pop" => (
            "scale",
            vec![
                (start - 0.1, json!([100, 100, 100])),
                (start + 0.06, json!([135, 135, 100])),
                (start + 0.24, json!([100, 100, 100])),
            ],
        ),
        "fall" => (
            "position",
            vec![(start - 0.05, json!([0, -60, 0])), (start + arrive, still)],
        ),
        "rise" => (
            "position",
            vec![(start - 0.05, json!([0, 50, 0])), (start + arrive, still)],
        ),
        "east" => (
            "position",
            vec![(start - 0.05, json!([-80, 0, 0])), (start + arrive, still)],
        ),
        "west" => (
            "position",
            vec![(start - 0.05, json!([80, 0, 0])), (start + arrive, still)],
        ),
        "settle" => (
            "position",
            vec![(start - 0.05, json!([0, -24, 0])), (start + arrive, still)],
        ),
        "sleep" => (
            "rotation",
            vec![(start - 0.05, json!(-12)), (start + arrive, json!(0))],
        ),
        "loop" => (
            "rotation",
            vec![(start, json!(-360)), (start + 0.8, json!(0))],
        ),
        _ => return None,
    })
}

fn stroke_preset(name: &str) -> Option<(&'static str, &'static str, f64, f64, f64)> {
    Some(match name {
        "underline" => (
            "0 0 280 30",
            "M 5 14 Q 61 5 125 13 T 269 10",
            260.0,
            28.0,
            0.55,
        ),
        "swoop" => (
            "0 0 790 160",
            "M 10 145 Q 126 170 217 93 T 438 58 Q 578 29 756 7 M 730 5 L 761 5 L 746 29",
            520.0,
            110.0,
            1.0,
        ),
        "loop" => (
            "0 0 620 110",
            "M 15 20 C 110 95 425 95 455 40 C 485 -4 350 -3 352 37 C 355 95 515 85 602 50 M 579 33 L 608 49 L 586 73",
            460.0,
            90.0,
            1.0,
        ),
        "hill" => (
            "0 0 410 95",
            "M 7 77 Q 73 72 118 32 Q 171 -8 228 31 Q 279 75 400 76",
            320.0,
            74.0,
            0.9,
        ),
        "down" => (
            "0 0 160 330",
            "M 73 13 C 27 67 104 129 66 196 Q 47 231 61 289 M 30 264 L 61 294 L 87 260",
            120.0,
            240.0,
            0.8,
        ),
        _ => return None,
    })
}

struct Stroke<'a> {
    id: &'a str,
    view_box: &'a str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    d: &'a str,
    color: &'a str,
    width: f64,
    from: f64,
    to: f64,
    shown: f64,
    gone: f64,
}

fn stroke_steps(s: &Stroke<'_>, steps: &mut Vec<Value>) {
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{}\" width=\"{}\" height=\"{}\"><path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\"/></svg>",
        s.view_box, s.w, s.h, s.d, s.color, s.width
    );
    steps.push(step("edit.pasteSvg", json!({"svg": svg, "name": s.id})));
    steps.push(step(
        "prop.set",
        json!({"layer": s.id, "path": "transform/position", "value": [s.x + s.w / 2.0, s.y + s.h / 2.0, 0.0]}),
    ));
    steps.push(step(
        "layer.addShapeItem",
        json!({"layer": s.id, "kind": "trim"}),
    ));
    for (t, v) in [(s.from.max(0.0), 0.0), (s.to.max(s.from + 0.05), 100.0)] {
        steps.push(step(
            "prop.set",
            json!({"layer": s.id, "path": "contents/trim/end", "value": v, "time": t, "timeBase": "comp"}),
        ));
    }
    steps.extend(trim_steps(json!(s.id), s.shown, s.gone));
}

fn text_params(look: &Look) -> Value {
    let mut p =
        json!({"font": look.font, "style": look.style, "size": look.size, "fill": look.fill});
    if let Some(outline) = &look.outline {
        p["stroke"] = json!(outline);
        p["applyStroke"] = json!(true);
        p["strokeWidth"] = json!((look.size * 0.08).max(3.0));
        // The outline sits outside the letters instead of eating into them.
        p["strokeOverFill"] = json!(false);
    }
    p
}

/// Show a layer only from `shown` to `gone`: select it, then Trim In and Trim Out at those times
/// (After Effects' Alt+[ / Alt+]). `layer.timing` with bare `in`/`out` is accepted by
/// EffectCraft but changes nothing, which left every lyric on screen for the whole song.
fn trim_steps(layer: Value, shown: f64, gone: f64) -> Vec<Value> {
    vec![
        step("layer.select", json!({"layers": [layer]})),
        step("time.set", json!({"time": shown})),
        step("layer.timing", json!({"op": "trimInToTime"})),
        step("time.set", json!({"time": gone})),
        step("layer.timing", json!({"op": "trimOutToTime"})),
    ]
}

fn step(command: &str, params: Value) -> Value {
    json!({"command": command, "params": params})
}

fn animate_range(
    id: &str,
    range: (usize, usize),
    name: &str,
    start: f64,
    end: f64,
    steps: &mut Vec<Value>,
) {
    let Some((property, keys)) = motion(name, start, end) else {
        return;
    };
    steps.push(step(
        "layer.addTextAnimator",
        json!({"layer": id, "properties": [property]}),
    ));
    let selector = "text/animators/animator/selectors/rangeSelector";
    for (path, value) in [
        ("advanced/units", json!(1)),
        ("start", json!(range.0)),
        ("end", json!(range.1)),
    ] {
        steps.push(step(
            "prop.set",
            json!({"layer": id, "path": format!("{selector}/{path}"), "value": value}),
        ));
    }
    for (time, value) in keys {
        let path = format!("text/animators/animator/properties/{property}");
        steps.push(step(
            "prop.set",
            json!({"layer": id, "path": path, "value": value, "time": time.max(0.0), "timeBase": "comp"}),
        ));
    }
}

/// The hero word's style and motion, limited to its characters by a range selector.
fn hero_steps(
    recipe: &Value,
    line: &Line,
    k: usize,
    spec: &Value,
    box_y: f64,
    span: (f64, f64),
    steps: &mut Vec<Value>,
) -> Value {
    let id = line.layer.as_str();
    let (a, b) = line.range(k);
    let hero_look = look(recipe, spec);
    let mut style = text_params(&hero_look);
    style["layer"] = json!(id);
    style["range"] = json!([a, b]);
    steps.push(step("layer.setText", style));
    let (start, end) = line.words[k].1.unwrap_or((line.start, line.end));
    let name = spec["motion"].as_str().unwrap_or("quiet");
    animate_range(id, (a, b), name, start, end, steps);
    if let Some(stroke_name) = spec["stroke"].as_str()
        && let Some((view_box, d, sw, sh, draw_s)) = stroke_preset(stroke_name)
    {
        let stroke_id = format!("{id}_stroke");
        stroke_steps(
            &Stroke {
                id: &stroke_id,
                view_box,
                x: (FRAME.0 - sw) / 2.0,
                y: box_y + 190.0,
                w: sw,
                h: sh,
                d,
                color: &hero_look.fill,
                width: 7.0,
                from: start,
                to: (start + draw_s).min(span.1),
                shown: span.0,
                gone: span.1,
            },
            steps,
        );
    }
    json!({"word": line.words[k].0, "motion": name, "at": start})
}

fn item_word_time(line: &Line, item: &Value) -> (f64, f64) {
    let id_str = item["accent"]
        .as_str()
        .or_else(|| item["word_ids"].get(0).and_then(Value::as_str))
        .unwrap_or_default();
    if let Some((_, suffix)) = id_str.rsplit_once("/w")
        && let Ok(idx) = suffix.parse::<usize>()
        && let Some((_, Some(t))) = line.words.get(idx)
    {
        return *t;
    }
    (line.start, line.end)
}

fn item_text(line: &Line, item: &Value) -> String {
    if let Some(s) = item["text"].as_str().filter(|s| !s.is_empty()) {
        return s.to_owned();
    }
    item["word_ids"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str()?.rsplit_once("/w")?.1.parse::<usize>().ok())
        .filter_map(|idx| line.words.get(idx).map(|(w, _)| w.as_str()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn spatial_line_steps(
    recipe: &Value,
    line: &Line,
    design: &Value,
    shown: f64,
    gone: f64,
    steps: &mut Vec<Value>,
) -> Value {
    let mut hero_summary = Value::Null;
    for (idx, item) in design["items"].as_array().into_iter().flatten().enumerate() {
        let layer_id = format!("{}_item{idx}", line.layer);
        let text_owned = item_text(line, item);
        let text_str = text_owned.as_str();
        let look = item_look(recipe, item);
        let x = item["x"].as_f64().unwrap_or(140.0);
        let y = item["y"].as_f64().unwrap_or(90.0);
        let bw =
            ((text_str.chars().count() as f64) * look.size * 0.72).clamp(220.0, FRAME.0 - 80.0);
        let bh = (look.size * 1.5).max(96.0);
        let mut text = text_params(&look);
        text["text"] = json!(text_str);
        text["name"] = json!(layer_id);
        text["box"] = json!([x, y, bw, bh]);
        text["justify"] = json!("left");
        steps.push(step("layer.newText", text));
        let rot = item["rotate"].as_f64().unwrap_or(0.0);
        if rot.abs() > 1e-4 {
            steps.push(step(
                "prop.set",
                json!({"layer": layer_id, "path": "transform/rotation", "value": rot}),
            ));
        }
        let (ws, we) = item_word_time(line, item);
        let move_name = item["motion"].as_str().unwrap_or("quiet");
        animate_range(
            &layer_id,
            (0, text_str.chars().count()),
            move_name,
            ws,
            we,
            steps,
        );
        if item["accent"].is_string() || move_name != "quiet" {
            hero_summary = json!({"word": text_str, "motion": move_name, "at": ws});
        }
        steps.extend(trim_steps(json!(layer_id), shown, gone));
        for (time, value) in [(shown, 0), (shown + 0.2, 100)] {
            steps.push(step(
                "prop.set",
                json!({"layer": layer_id, "path": "transform/opacity", "value": value, "time": time, "timeBase": "comp"}),
            ));
        }
    }
    for (d_idx, draw) in design["draws"].as_array().into_iter().flatten().enumerate() {
        let vb = draw["view_box"].as_str().unwrap_or("0 0 280 30");
        let (x, y) = (
            draw["x"].as_f64().unwrap_or(0.0),
            draw["y"].as_f64().unwrap_or(0.0),
        );
        let (w, h) = (
            draw["w"].as_f64().unwrap_or(240.0),
            draw["h"].as_f64().unwrap_or(30.0),
        );
        for (p_idx, path) in draw["paths"].as_array().into_iter().flatten().enumerate() {
            let sid = format!("{}_draw{d_idx}_{p_idx}", line.layer);
            stroke_steps(
                &Stroke {
                    id: &sid,
                    view_box: vb,
                    x,
                    y,
                    w,
                    h,
                    d: path["d"].as_str().unwrap_or_default(),
                    color: path["stroke"].as_str().unwrap_or("#B8442A"),
                    width: path["width"].as_f64().unwrap_or(7.0),
                    from: line.start + path["from"].as_f64().unwrap_or(0.0),
                    to: line.start + path["to"].as_f64().unwrap_or(0.55),
                    shown,
                    gone,
                },
                steps,
            );
        }
    }
    hero_summary
}

fn build_steps_with_designs(
    recipe: &Value,
    designs: &Value,
    all: &[Line],
    duration: f64,
) -> (Vec<Value>, Vec<Value>) {
    let (w, h) = FRAME;
    let top = recipe["band"].as_str() != Some("bottom");
    let box_y = if top { 50.0 } else { h - 350.0 };
    let base = look(recipe, &recipe["base"]);
    let mut steps = vec![step(
        "comp.new",
        json!({"name": COMP, "width": w, "height": h, "frameRate": 30, "duration": duration}),
    )];
    let mut summary = Vec::new();
    let mut previous_out: f64 = 0.0;
    for (i, line) in all.iter().enumerate() {
        let id = line.id.as_str();
        let next = all.get(i + 1).map_or(duration, |l| l.start);
        let shown = (line.start - LEAD).max(previous_out).max(0.0);
        let gone = if next - line.end < HOLD {
            next
        } else {
            (line.end + HOLD).min(duration)
        };
        previous_out = gone;
        let line_design = &designs[id];
        let hero_info = if line_design["items"]
            .as_array()
            .is_some_and(|a| !a.is_empty())
        {
            spatial_line_steps(recipe, line, line_design, shown, gone, &mut steps)
        } else {
            let mut text = text_params(&base);
            text["text"] = json!(line.text());
            text["name"] = json!(line.layer);
            text["box"] = json!([120.0, box_y, w - 240.0, 300.0]);
            text["justify"] = json!("center");
            steps.push(step("layer.newText", text));
            let h_val = hero(recipe, line)
                .map(|(k, spec)| {
                    hero_steps(recipe, line, k, spec, box_y, (shown, gone), &mut steps)
                })
                .unwrap_or(Value::Null);
            steps.extend(trim_steps(json!(line.layer), shown, gone));
            for (time, value) in [(shown, 0), (shown + 0.2, 100)] {
                steps.push(step("prop.set", json!({"layer": line.layer, "path": "transform/opacity", "value": value, "time": time, "timeBase": "comp"})));
            }
            h_val
        };
        summary.push(
            json!({"id": id, "text": line.text(), "in": shown, "out": gone, "hero": hero_info}),
        );
    }
    (steps, summary)
}

fn words_designs(dir: &Path) -> Value {
    let path = crate::words::document_path(dir);
    let Ok(bytes) = std::fs::read(path) else {
        return json!({});
    };
    let Ok(doc) = serde_json::from_slice::<Value>(&bytes) else {
        return json!({});
    };
    let mut map = serde_json::Map::new();
    for anim in doc["animations"].as_array().into_iter().flatten() {
        if let Some(obj) = anim["design"].as_object() {
            for (k, v) in obj {
                map.insert(k.clone(), v.clone());
            }
        }
    }
    Value::Object(map)
}

/// `words.letter`: build `lettering/lyrics.ecproj` from the approved recipe and the aligned
/// lyrics, then render it with transparency unless `render` is false.
pub fn cmd_words_letter(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let designs = words_designs(&dir);
    let recipe_path = p["recipe"]
        .as_str()
        .map_or_else(|| dir.join("lyrics-design.json"), PathBuf::from);
    let recipe: Value = std::fs::read(&recipe_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .or_else(|| {
            designs.as_object().filter(|m| !m.is_empty()).map(|_| {
                json!({"band": "top", "base": {"lettering": "sans-bold", "size": 58, "color": "#173F49", "outline": "#FFF8E9"}})
            })
        })
        .ok_or_else(|| {
            invalid(format!(
                "No lyric design recipe at {}; propose one and get it approved first",
                recipe_path.display()
            ))
        })?;
    let all = lines(&crate::words::aligned_in(&dir)?);
    if all.is_empty() {
        return Err(invalid(
            "No timed lyric lines: import the transcript and the artist's lyric sheet first",
        ));
    }
    let last = all.last().map_or(0.0, |l| l.end);
    let duration = crate::workshop::song_seconds(&dir)
        .unwrap_or(last + 2.0)
        .max(last);
    let (steps, summary) = build_steps_with_designs(&recipe, &designs, &all, duration);
    let out = dir.join("lettering");
    std::fs::create_dir_all(&out)?;
    let (project, commands) = (out.join("lyrics.ecproj"), out.join("lyrics.steps.jsonl"));
    let jsonl: String = steps.iter().map(|v| format!("{v}\n")).collect();
    std::fs::write(&commands, jsonl)?;
    let mut native_steps = steps;
    native_steps.push(json!({"command":"file.saveAs","params":{"path":project}}));
    crate::character::effectcraft(&dir, None, &native_steps, true)?;
    let render = if p["render"] == false {
        None
    } else {
        let movie = out.join("lyrics.mov");
        crate::character::effectcraft_render(
            &project,
            json!({
                "comp": COMP, "output": movie, "format": "prores", "proresProfile": "4444", "channels": "rgba"
            }),
        )?;
        Some(movie)
    };
    Ok(
        json!({"ok": true, "project": project, "commands": commands, "render": render,
        "recipe": recipe_path, "duration": duration, "lines": summary}),
    )
}

#[cfg(test)]
#[path = "../tests/unit/lettering.rs"]
mod tests;
