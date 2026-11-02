//! `words.letter`: the song's lyrics lettered and animated by the installed effects app,
//! driven only through its CLI. Magic owns the words, their timing and the approved recipe
//! (`lyrics-design.json`); the effects app owns layout, type, motion and rendering. The result
//! lives in the project folder: `lettering/lyrics.ecproj`, the commands that built it, and a
//! transparent render for the film to place over the backgrounds.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const EFFECTS_CLI: &str = "effectcraft-cli";
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
        // Drops in from above.
        "fall" => (
            "position",
            vec![(start - 0.05, json!([0, -60, 0])), (start + arrive, still)],
        ),
        // Lifts into place from below.
        "rise" => (
            "position",
            vec![(start - 0.05, json!([0, 50, 0])), (start + arrive, still)],
        ),
        // Slides in from the left.
        "east" => (
            "position",
            vec![(start - 0.05, json!([-80, 0, 0])), (start + arrive, still)],
        ),
        // One full turn, ending upright.
        "loop" => (
            "rotation",
            vec![(start, json!(-360)), (start + 0.8, json!(0))],
        ),
        _ => return None,
    })
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

fn step(command: &str, params: Value) -> Value {
    json!({"command": command, "params": params})
}

/// The hero word's style and motion, limited to its characters by a range selector.
fn hero_steps(
    recipe: &Value,
    line: &Line,
    k: usize,
    spec: &Value,
    steps: &mut Vec<Value>,
) -> Value {
    let id = line.id.as_str();
    let (a, b) = line.range(k);
    let mut style = text_params(&look(recipe, spec));
    style["layer"] = json!(id);
    style["range"] = json!([a, b]);
    steps.push(step("layer.setText", style));
    let (start, end) = line.words[k].1.unwrap_or((line.start, line.end));
    let name = spec["motion"].as_str().unwrap_or("quiet");
    if let Some((property, keys)) = motion(name, start, end) {
        steps.push(step(
            "layer.addTextAnimator",
            json!({"layer": id, "properties": [property]}),
        ));
        let selector = "text/animators/animator/selectors/rangeSelector";
        for (path, value) in [
            ("advanced/units", json!(1)),
            ("start", json!(a)),
            ("end", json!(b)),
        ] {
            steps.push(step(
                "prop.set",
                json!({"layer": id, "path": format!("{selector}/{path}"), "value": value}),
            ));
        }
        for (time, value) in keys {
            let path = format!("text/animators/animator/properties/{property}");
            steps.push(step("prop.set", json!({"layer": id, "path": path, "value": value, "time": time.max(0.0), "timeBase": "comp"})));
        }
    }
    json!({"word": line.words[k].0, "motion": name, "at": start})
}

/// The effects-app commands for the whole song, and a summary of each line.
fn build_steps(recipe: &Value, all: &[Line], duration: f64) -> (Vec<Value>, Vec<Value>) {
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
        let mut text = text_params(&base);
        text["text"] = json!(line.text());
        text["name"] = json!(id);
        text["box"] = json!([120.0, box_y, w - 240.0, 300.0]);
        text["justify"] = json!("center");
        steps.push(step("layer.newText", text));
        let hero =
            hero(recipe, line).map(|(k, spec)| hero_steps(recipe, line, k, spec, &mut steps));
        steps.push(step(
            "layer.timing",
            json!({"layers": [id], "in": shown, "out": gone}),
        ));
        for (time, value) in [(shown, 0), (shown + 0.2, 100)] {
            steps.push(step("prop.set", json!({"layer": id, "path": "transform/opacity", "value": value, "time": time, "timeBase": "comp"})));
        }
        summary
            .push(json!({"id": id, "text": line.text(), "in": shown, "out": gone, "hero": hero}));
    }
    (steps, summary)
}

fn run_effects(dir: &Path, args: &[String]) -> Result<String> {
    crate::workspace::run_tool_in(dir, EFFECTS_CLI, args)
}

/// `words.letter`: build `lettering/lyrics.ecproj` from the approved recipe and the aligned
/// lyrics, then render it with transparency unless `render` is false.
pub fn cmd_words_letter(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let recipe_path = p["recipe"]
        .as_str()
        .map_or_else(|| dir.join("lyrics-design.json"), PathBuf::from);
    let recipe: Value = std::fs::read(&recipe_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
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
    let (steps, summary) = build_steps(&recipe, &all, duration);
    let out = dir.join("lettering");
    std::fs::create_dir_all(&out)?;
    let (project, commands) = (out.join("lyrics.ecproj"), out.join("lyrics.steps.jsonl"));
    let jsonl: String = steps.iter().map(|v| format!("{v}\n")).collect();
    std::fs::write(&commands, jsonl)?;
    let arg = |p: &Path| p.to_string_lossy().into_owned();
    run_effects(
        &dir,
        &[
            "--empty".into(),
            "--save-as".into(),
            arg(&project),
            "--json".into(),
            "run".into(),
            "--commands".into(),
            arg(&commands),
        ],
    )?;
    let render = if p["render"] == false {
        None
    } else {
        let movie = out.join("lyrics.mov");
        run_effects(
            &dir,
            &[
                "--project".into(),
                arg(&project),
                "render".into(),
                "--comp".into(),
                COMP.into(),
                "--out".into(),
                arg(&movie),
                "--format".into(),
                "prores".into(),
                "--prores".into(),
                "4444".into(),
                "--channels".into(),
                "rgba".into(),
            ],
        )?;
        Some(movie)
    };
    Ok(
        json!({"ok": true, "project": project, "commands": commands, "render": render,
        "recipe": recipe_path, "duration": duration, "lines": summary}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aligned() -> Value {
        let w = |line: &str, text: &str, t: Option<(f64, f64)>| {
            let (s, e) = t.map_or((Value::Null, Value::Null), |(s, e)| (json!(s), json!(e)));
            json!({"sung_id": line, "text": text, "start": s, "end": e})
        };
        json!({"words": [
            w("l1", "X", None), w("l1", "marks", Some((0.0, 0.8))), w("l1", "the", Some((0.8, 1.2))), w("l1", "spot,", Some((1.26, 1.82))),
            w("l2", "And", Some((3.5, 3.9))), w("l2", "Dash", Some((4.1, 4.5))),
        ]})
    }

    fn recipe() -> Value {
        json!({"band": "top", "max_heroes": 1, "palette": {"ink": "#173F49", "rust": "#B8442A", "cream": "#FFF8E9"},
            "base": {"lettering": "sans-bold", "size": 58, "color": "ink", "outline": "cream"},
            "heroes": {"spot": {"lettering": "serif-bold", "size": 130, "color": "rust", "motion": "pop"}}})
    }

    #[test]
    fn a_hero_is_styled_and_animated_on_its_own_characters_at_its_sung_time() {
        let all = lines(&aligned());
        assert_eq!(all[0].text(), "X marks the spot,");
        assert_eq!(all[0].range(3), (12, 17));
        let (steps, summary) = build_steps(&recipe(), &all, 33.0);
        let styled = steps
            .iter()
            .find(|s| s["command"] == "layer.setText")
            .unwrap();
        assert_eq!(styled["params"]["range"], json!([12, 17]));
        assert_eq!(styled["params"]["font"], "C059");
        let pop: Vec<f64> = steps
            .iter()
            .filter(|s| s["params"]["path"] == "text/animators/animator/properties/scale")
            .filter_map(|s| s["params"]["time"].as_f64())
            .collect();
        assert_eq!(pop.len(), 3);
        assert!((pop[1] - 1.32).abs() < 1e-9);
        assert_eq!(summary[0]["hero"]["word"], "spot,");
    }

    #[test]
    fn lines_show_before_they_are_sung_and_never_overlap() {
        let (_, summary) = build_steps(&recipe(), &lines(&aligned()), 33.0);
        let (out0, in1) = (
            summary[0]["out"].as_f64().unwrap(),
            summary[1]["in"].as_f64().unwrap(),
        );
        assert!(summary[0]["in"].as_f64().unwrap() == 0.0);
        assert!(in1 >= out0, "{out0} {in1}");
        assert!((in1 - 3.25).abs() < 1e-9);
    }
}
