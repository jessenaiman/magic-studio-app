//! `words.letter`: the song's lyrics lettered and animated by the linked EffectCraft engine.
//! Magic owns the words, their timing in [`Tick`]s and the approved recipe (`lyrics-design.json`);
//! EffectCraft owns layout, type, motion and rendering (`lettering/lyrics.ecproj`).
use crate::cmd::words::word_tick;
use crate::time::{TICKS_PER_SECOND, Tick, parse_tick};
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const COMP: &str = "Lyrics";
const FRAME: (f64, f64) = (1920.0, 1080.0);
/// 0.25s lead-in and 1.0s hold in FilmCraft ticks.
const LEAD_TICK: Tick = Tick(TICKS_PER_SECOND / 4);
const HOLD_TICK: Tick = Tick(TICKS_PER_SECOND);

fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

/// One sung line: its id, its printed words with their sung `Tick` times, and its sung span.
struct Line {
    id: String,
    layer: String,
    words: Vec<(String, Option<(Tick, Tick)>)>,
    start: Tick,
    end: Tick,
}

impl Line {
    fn text(&self) -> String {
        self.words.iter().map(|w| w.0.as_str()).collect::<Vec<_>>().join(" ")
    }

    /// Character range of word `k` within `text()`.
    fn range(&self, k: usize) -> (usize, usize) {
        let start: usize = self.words[..k].iter().map(|w| w.0.chars().count() + 1).sum();
        (start, start + self.words[k].0.chars().count())
    }
}

/// The artist's lines in sung order, from the aligned words; lines with no timed word skip.
fn lines(aligned: &Value) -> Vec<Line> {
    let mut out: Vec<Line> = Vec::new();
    for w in aligned["words"].as_array().into_iter().flatten() {
        let id = w["sung_id"].as_str().or(w["line_id"].as_str()).unwrap_or_default();
        let time = word_tick(w, "start_tick", "start").zip(word_tick(w, "end_tick", "end"));
        let word = (w["text"].as_str().unwrap_or_default().to_owned(), time);
        match out.last_mut() {
            Some(line) if line.id == id => line.words.push(word),
            _ => out.push(Line { id: id.to_owned(), layer: id.to_owned(), words: vec![word], start: Tick(i64::MAX), end: Tick(i64::MIN) }),
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
    text.chars().filter(char::is_ascii_alphabetic).flat_map(char::to_lowercase).collect()
}

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
    let font = |kind: &str, default: &str| recipe["fonts"][kind].as_str().unwrap_or(default).to_owned();
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
    let font_fn = |kind: &str, default: &str| recipe["fonts"][kind].as_str().unwrap_or(default).to_owned();
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

fn hero<'a>(recipe: &'a Value, line: &Line) -> Option<(usize, &'a Value)> {
    if recipe["max_heroes"].as_u64() == Some(0) {
        return None;
    }
    line.words.iter().enumerate().find_map(|(k, w)| recipe["heroes"].get(key(&w.0)).map(|spec| (k, spec)))
}

/// The hero's motion as `(animator property, keyframes of (Tick, value))`.
fn motion(name: &str, start: Tick, end: Tick) -> Option<(&'static str, Vec<(Tick, Value)>)> {
    let still = json!([0, 0, 0]);
    let min_arrive = Tick(TICKS_PER_SECOND * 3 / 10); // 0.3s
    let max_arrive = Tick(TICKS_PER_SECOND * 6 / 10); // 0.6s
    let arrive = (end - start).clamp(min_arrive, max_arrive);
    let t_50ms = Tick(TICKS_PER_SECOND / 20);
    let t_60ms = Tick(TICKS_PER_SECOND * 3 / 50);
    let t_100ms = Tick(TICKS_PER_SECOND / 10);
    let t_240ms = Tick(TICKS_PER_SECOND * 6 / 25);
    let t_800ms = Tick(TICKS_PER_SECOND * 4 / 5);
    Some(match name {
        "pop" => {
            ("scale", vec![(start - t_100ms, json!([100, 100, 100])), (start + t_60ms, json!([135, 135, 100])), (start + t_240ms, json!([100, 100, 100]))])
        }
        "fall" => ("position", vec![(start - t_50ms, json!([0, -60, 0])), (start + arrive, still)]),
        "rise" => ("position", vec![(start - t_50ms, json!([0, 50, 0])), (start + arrive, still)]),
        "east" => ("position", vec![(start - t_50ms, json!([-80, 0, 0])), (start + arrive, still)]),
        "west" => ("position", vec![(start - t_50ms, json!([80, 0, 0])), (start + arrive, still)]),
        "settle" => ("position", vec![(start - t_50ms, json!([0, -24, 0])), (start + arrive, still)]),
        "sleep" => ("rotation", vec![(start - t_50ms, json!(-12)), (start + arrive, json!(0))]),
        "loop" => ("rotation", vec![(start, json!(-360)), (start + t_800ms, json!(0))]),
        _ => return None,
    })
}

fn stroke_preset(name: &str) -> Option<(&'static str, &'static str, f64, f64, Tick)> {
    Some(match name {
        "underline" => ("0 0 280 30", "M 5 14 Q 61 5 125 13 T 269 10", 260.0, 28.0, Tick(TICKS_PER_SECOND * 11 / 20)),
        "swoop" => ("0 0 790 160", "M 10 145 Q 126 170 217 93 T 438 58 Q 578 29 756 7 M 730 5 L 761 5 L 746 29", 520.0, 110.0, HOLD_TICK),
        "loop" => {
            ("0 0 620 110", "M 15 20 C 110 95 425 95 455 40 C 485 -4 350 -3 352 37 C 355 95 515 85 602 50 M 579 33 L 608 49 L 586 73", 460.0, 90.0, HOLD_TICK)
        }
        "hill" => ("0 0 410 95", "M 7 77 Q 73 72 118 32 Q 171 -8 228 31 Q 279 75 400 76", 320.0, 74.0, Tick(TICKS_PER_SECOND * 9 / 10)),
        "down" => ("0 0 160 330", "M 73 13 C 27 67 104 129 66 196 Q 47 231 61 289 M 30 264 L 61 294 L 87 260", 120.0, 240.0, Tick(TICKS_PER_SECOND * 4 / 5)),
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
    from: Tick,
    to: Tick,
    shown: Tick,
    gone: Tick,
}

fn stroke_steps(s: &Stroke<'_>, steps: &mut Vec<Value>) {
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{}\" width=\"{}\" height=\"{}\"><path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\"/></svg>",
        s.view_box, s.w, s.h, s.d, s.color, s.width
    );
    steps.push(step("edit.pasteSvg", json!({"svg": svg, "name": s.id})));
    steps.push(step("prop.set", json!({"layer": s.id, "path": "transform/position", "value": [s.x + s.w / 2.0, s.y + s.h / 2.0, 0.0]})));
    steps.push(step("layer.addShapeItem", json!({"layer": s.id, "kind": "trim"})));
    let min_span = Tick(TICKS_PER_SECOND / 20);
    let from = s.from.max(Tick::ZERO);
    let to = s.to.max(from + min_span);
    for (t, v) in [(from, 0.0), (to, 100.0)] {
        steps.push(step("prop.addKey", json!({"layer": s.id, "path": "contents/trim/end", "value": v, "time": t.seconds(), "timeBase": "comp"})));
    }
    steps.extend(trim_steps(json!(s.id), s.shown, s.gone));
}

fn text_params(look: &Look) -> Value {
    let mut p = json!({"font": look.font, "style": look.style, "size": look.size, "fill": look.fill});
    if let Some(outline) = &look.outline {
        p["stroke"] = json!(outline);
        p["applyStroke"] = json!(true);
        p["strokeWidth"] = json!((look.size * 0.08).max(3.0));
        p["strokeOverFill"] = json!(false);
    }
    p
}

fn trim_steps(layer: Value, shown: Tick, gone: Tick) -> Vec<Value> {
    vec![
        step("layer.select", json!({"layers": [layer]})),
        step("time.set", json!({"time": shown.seconds()})),
        step("layer.timing", json!({"op": "trimInToTime"})),
        step("time.set", json!({"time": gone.seconds()})),
        step("layer.timing", json!({"op": "trimOutToTime"})),
    ]
}

fn step(command: &str, params: Value) -> Value {
    json!({"command": command, "params": params})
}

fn animate_range(id: &str, range: (usize, usize), name: &str, start: Tick, end: Tick, steps: &mut Vec<Value>) {
    let Some((property, keys)) = motion(name, start, end) else {
        return;
    };
    steps.push(step("layer.addTextAnimator", json!({"layer": id, "properties": [property]})));
    let selector = "text/animators/animator/selectors/rangeSelector";
    for (path, value) in [("advanced/units", json!(1)), ("start", json!(range.0)), ("end", json!(range.1))] {
        steps.push(step("prop.set", json!({"layer": id, "path": format!("{selector}/{path}"), "value": value})));
    }
    for (time, value) in keys {
        let path = format!("text/animators/animator/properties/{property}");
        steps.push(step("prop.addKey", json!({"layer": id, "path": path, "value": value, "time": time.max(Tick::ZERO).seconds(), "timeBase": "comp"})));
    }
}

fn hero_steps(recipe: &Value, line: &Line, k: usize, spec: &Value, box_y: f64, span: (Tick, Tick), steps: &mut Vec<Value>) -> Value {
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
    json!({"word": line.words[k].0, "motion": name, "at_tick": start.0, "at": start.seconds()})
}

fn item_word_time(line: &Line, item: &Value) -> (Tick, Tick) {
    let id_str = item["accent"].as_str().or_else(|| item["word_ids"].get(0).and_then(Value::as_str)).unwrap_or_default();
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

fn spatial_line_steps(recipe: &Value, line: &Line, design: &Value, shown: Tick, gone: Tick, steps: &mut Vec<Value>) -> Value {
    let mut hero_summary = Value::Null;
    let fade_tick = Tick(TICKS_PER_SECOND / 5); // 0.2s
    for (idx, item) in design["items"].as_array().into_iter().flatten().enumerate() {
        let layer_id = format!("{}_item{idx}", line.layer);
        let text_owned = item_text(line, item);
        let text_str = text_owned.as_str();
        let look = item_look(recipe, item);
        let x = item["x"].as_f64().unwrap_or(140.0);
        let y = item["y"].as_f64().unwrap_or(90.0);
        let bw = ((text_str.chars().count() as f64) * look.size * 0.72).clamp(220.0, FRAME.0 - 80.0);
        let bh = (look.size * 1.5).max(96.0);
        let mut text = text_params(&look);
        text["text"] = json!(text_str);
        text["name"] = json!(layer_id);
        if let Some(keys) = item["motion_keys"].as_array().filter(|keys| !keys.is_empty()) {
            let (start, _) = item_word_time(line, item);
            let (origin_x, origin_y) = if item["native_motion"].is_object() || item["native_motion_binding"]["box"].as_bool() == Some(true) {
                text["box"] = json!([x, y, bw, bh]);
                (x + bw / 2.0, y + bh / 2.0)
            } else {
                text["position"] = json!([x, y]);
                (x, y)
            };
            text["justify"] = json!("left");
            steps.push(step("layer.newText", text));
            steps.extend(trim_steps(json!(layer_id), start.max(shown), gone));
            let rotate = item["rotate"].as_f64().unwrap_or_default();
            for key in keys {
                let num = |i: usize| key[i].as_f64().unwrap_or_default();
                let key_tick = start + Tick::from_seconds_f64(num(0));
                for (property, value) in [
                    ("position", json!([origin_x + num(1), origin_y + num(2)])),
                    ("scale", json!([num(3), num(4)])),
                    ("rotation", json!(rotate + num(5))),
                    ("opacity", json!(num(6))),
                ] {
                    steps.push(step(
                        "prop.addKey",
                        json!({"layer": layer_id,
                        "path": format!("transform/{property}"), "time": key_tick.seconds(),
                        "timeBase": "comp", "value": value}),
                    ));
                }
            }
            hero_summary = json!({"word": text_str, "motion": "keyframes", "at_tick": start.0, "at": start.seconds()});
            continue;
        }
        text["box"] = json!([x, y, bw, bh]);
        text["justify"] = json!("left");
        steps.push(step("layer.newText", text));
        let rot = item["rotate"].as_f64().unwrap_or(0.0);
        if rot.abs() > 1e-4 {
            steps.push(step("prop.set", json!({"layer": layer_id, "path": "transform/rotation", "value": rot})));
        }
        let (ws, we) = item_word_time(line, item);
        let move_name = item["motion"].as_str().unwrap_or("quiet");
        animate_range(&layer_id, (0, text_str.chars().count()), move_name, ws, we, steps);
        if item["accent"].is_string() || move_name != "quiet" {
            hero_summary = json!({"word": text_str, "motion": move_name, "at_tick": ws.0, "at": ws.seconds()});
        }
        steps.extend(trim_steps(json!(layer_id), shown, gone));
        for (time, value) in [(shown, 0), (shown + fade_tick, 100)] {
            steps
                .push(step("prop.addKey", json!({"layer": layer_id, "path": "transform/opacity", "value": value, "time": time.seconds(), "timeBase": "comp"})));
        }
    }
    let default_draw = Tick(TICKS_PER_SECOND * 11 / 20);
    for (d_idx, draw) in design["draws"].as_array().into_iter().flatten().enumerate() {
        let vb = draw["view_box"].as_str().unwrap_or("0 0 280 30");
        let (x, y) = (draw["x"].as_f64().unwrap_or(0.0), draw["y"].as_f64().unwrap_or(0.0));
        let (w, h) = (draw["w"].as_f64().unwrap_or(240.0), draw["h"].as_f64().unwrap_or(30.0));
        for (p_idx, path) in draw["paths"].as_array().into_iter().flatten().enumerate() {
            let sid = format!("{}_draw{d_idx}_{p_idx}", line.layer);
            let from_rel = path.get("from").and_then(parse_tick).unwrap_or(Tick::ZERO);
            let to_rel = path.get("to").and_then(parse_tick).unwrap_or(default_draw);
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
                    from: line.start + from_rel,
                    to: line.start + to_rel,
                    shown,
                    gone,
                },
                steps,
            );
        }
    }
    hero_summary
}

fn build_steps_with_designs(recipe: &Value, designs: &Value, all: &[Line], duration: Tick) -> (Vec<Value>, Vec<Value>) {
    let (w, h) = FRAME;
    let top = recipe["band"].as_str() != Some("bottom");
    let box_y = if top { 50.0 } else { h - 350.0 };
    let base = look(recipe, &recipe["base"]);
    let fade_tick = Tick(TICKS_PER_SECOND / 5);
    let mut steps = vec![step("comp.new", json!({"name": COMP, "width": w, "height": h, "frameRate": 30, "duration": duration.seconds()}))];
    let mut summary = Vec::new();
    let mut previous_out = Tick::ZERO;
    for (i, line) in all.iter().enumerate() {
        let id = line.id.as_str();
        let next = all.get(i + 1).map_or(duration, |l| l.start);
        let shown = (line.start - LEAD_TICK).max(previous_out).max(Tick::ZERO);
        let gone = if next - line.end < HOLD_TICK { next } else { (line.end + HOLD_TICK).min(duration) };
        previous_out = gone;
        let line_design = &designs[&line.layer];
        let hero_info = if line_design["items"].as_array().is_some_and(|a| !a.is_empty()) {
            spatial_line_steps(recipe, line, line_design, shown, gone, &mut steps)
        } else {
            let mut text = text_params(&base);
            text["text"] = json!(line.text());
            text["name"] = json!(line.layer);
            text["box"] = json!([120.0, box_y, w - 240.0, 300.0]);
            text["justify"] = json!("center");
            steps.push(step("layer.newText", text));
            let h_val = hero(recipe, line).map(|(k, spec)| hero_steps(recipe, line, k, spec, box_y, (shown, gone), &mut steps)).unwrap_or(Value::Null);
            steps.extend(trim_steps(json!(line.layer), shown, gone));
            for (time, value) in [(shown, 0), (shown + fade_tick, 100)] {
                steps.push(step(
                    "prop.addKey",
                    json!({"layer": line.layer, "path": "transform/opacity", "value": value, "time": time.seconds(), "timeBase": "comp"}),
                ));
            }
            h_val
        };
        summary.push(json!({
            "id": id,
            "text": line.text(),
            "in_tick": shown.0,
            "out_tick": gone.0,
            "in": shown.seconds(),
            "out": gone.seconds(),
            "hero": hero_info
        }));
    }
    (steps, summary)
}

fn words_designs(dir: &Path, all: &[Line]) -> Value {
    let path = crate::cmd::words::document_path(dir);
    let Ok(bytes) = std::fs::read(path) else {
        return json!({});
    };
    let Ok(doc) = serde_json::from_slice::<Value>(&bytes) else {
        return json!({});
    };
    let mut map = serde_json::Map::new();
    let tolerance = TICKS_PER_SECOND / 10_000;
    for line in all {
        let design = doc["animations"]
            .as_array()
            .into_iter()
            .flatten()
            .find_map(|anim| {
                let belongs = anim["words"].as_array().into_iter().flatten().any(|word| {
                    let wid_line = word["word_id"].as_str().and_then(|wid| wid.split_once('/').map(|(l, _)| l));
                    (word["sung_id"].as_str().or(word["line_id"].as_str()).or(wid_line) == Some(line.id.as_str()))
                        && word_tick(word, "start_tick", "start").is_some_and(|start| (start.0 - line.start.0).abs() <= tolerance)
                });
                belongs.then(|| anim["design"].get(&line.id)).flatten()
            })
            .or_else(|| doc["animations"].as_array().into_iter().flatten().find_map(|anim| anim["design"].get(&line.id)));
        if let Some(design) = design {
            map.insert(line.layer.clone(), design.clone());
        }
    }
    Value::Object(map)
}

/// `words.letter`: build `lettering/lyrics.ecproj` from the approved recipe and the aligned
/// lyrics, then render it with transparency unless `render` is false.
pub fn cmd_words_letter(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::cmd::words::workspace_dir(s)?;
    let all = lines(&crate::cmd::words::aligned_in(&dir)?);
    let designs = words_designs(&dir, &all);
    let recipe_path = p["recipe"].as_str().map_or_else(|| dir.join("lyrics-design.json"), PathBuf::from);
    let recipe: Value = std::fs::read(&recipe_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .or_else(|| {
            designs
                .as_object()
                .filter(|m| !m.is_empty())
                .map(|_| json!({"band": "top", "base": {"lettering": "sans-bold", "size": 58, "color": "#173F49", "outline": "#FFF8E9"}}))
        })
        .ok_or_else(|| invalid(format!("No lyric design recipe at {}; propose one and get it approved first", recipe_path.display())))?;
    if all.is_empty() {
        return Err(invalid("No timed lyric lines: import the transcript and the artist's lyric sheet first"));
    }
    let last = all.last().map_or(Tick::ZERO, |l| l.end);
    let duration = crate::workshop::song_ticks(&dir).unwrap_or(last + Tick(2 * TICKS_PER_SECOND)).max(last);
    let (steps, summary) = build_steps_with_designs(&recipe, &designs, &all, duration);
    let out = dir.join("lettering");
    std::fs::create_dir_all(&out)?;
    let (project, commands) = (out.join("lyrics.ecproj"), out.join("lyrics.steps.jsonl"));
    let jsonl: String = steps.iter().map(|v| format!("{v}\n")).collect();
    crate::doc_state::write_file(&commands, jsonl)?;
    let mut native_steps = steps;
    native_steps.push(json!({"command":"file.saveAs","params":{"path":project}}));
    crate::cmd::character::effectcraft(&dir, None, &native_steps, true)?;
    let render = if p["render"] == false {
        None
    } else {
        let movie = out.join("lyrics.mov");
        crate::cmd::character::effectcraft_render(
            &project,
            json!({
                "comp": COMP, "output": movie, "format": "prores", "proresProfile": "4444", "channels": "rgba"
            }),
        )?;
        Some(movie)
    };
    let studio = if p["studio"] == true {
        let movie = render.as_deref().ok_or_else(|| invalid("Render the lettering before sending it to Studio"))?;
        Some(crate::cmd::film::publish_lyrics(s, movie)?)
    } else {
        None
    };
    Ok(json!({
        "ok": true,
        "project": project,
        "commands": commands,
        "render": render,
        "recipe": recipe_path,
        "duration_tick": duration.0,
        "duration": duration.seconds(),
        "lines": summary,
        "studio": studio
    }))
}

/// Publish the already-rendered alpha lettering after a background render.
pub fn cmd_words_publish(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::cmd::words::workspace_dir(s)?.canonicalize()?;
    let path = p["path"].as_str().map_or_else(|| dir.join("lettering/lyrics.mov"), |path| dir.join(path));
    let path = path.canonicalize()?;
    if !path.starts_with(&dir) || !path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("mov")) {
        return Err(invalid("Choose a rendered .mov inside the Magic project"));
    }
    let studio = crate::cmd::film::publish_lyrics(s, &path)?;
    Ok(json!({"ok":true,"render":path,"studio":studio}))
}
