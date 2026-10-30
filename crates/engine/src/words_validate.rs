//! Words design validation and effect installation (`words.effect_install`, `kinetic_design`, `line_design`, `text_effect`).
use crate::{Result, Session};
use serde_json::{Value, json};
use std::{fs, path::Path};

use super::words::{FONTS, KINETIC_LIBRARY, MOVES, invalid, one_of};

/// `words.effect_install`: install one HyperFrames registry effect into the project with the
/// pinned HyperFrames (`hyperframes add`). Needs the network; already-installed effects are left as they are.
pub fn cmd_words_effect_install(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let hf = dir.join("hyperframes");
    let name = p["name"]
        .as_str()
        .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
        .ok_or_else(|| invalid("name must be a registry effect name (letters, digits or '-')"))?;
    let found = || {
        [
            format!("compositions/components/{name}.html"),
            format!("compositions/{name}.html"),
        ]
        .into_iter()
        .find(|p| hf.join(p).is_file())
    };
    if let Some(src) = found() {
        return Ok(json!({"name":name,"src":src,"installed":false,"already":true}));
    }
    let cli = crate::video_target::hyperframes_cli(p["hyperframes_cli"].as_str(), Some(&hf))?;
    let out = crate::workspace::run_tool_in(
        &hf,
        &cli.to_string_lossy(),
        &[
            "add".into(),
            name.into(),
            "--json".into(),
            "--no-clipboard".into(),
        ],
    )?;
    let src = found().ok_or_else(|| {
        invalid(format!(
            "hyperframes add {name} wrote no {name}.html: {}",
            out.trim()
        ))
    })?;
    Ok(
        json!({"name":name,"src":src,"installed":true,"already":false,"receipt":serde_json::from_str::<Value>(&out).unwrap_or(json!(out.trim()))}),
    )
}

fn hex_color(v: &Value) -> Option<Value> {
    v.as_str()
        .filter(|c| {
            c.len() == 7 && c.starts_with('#') && c[1..].bytes().all(|b| b.is_ascii_hexdigit())
        })
        .map(|c| json!(c))
}

fn safe_font_name(v: &Value) -> Option<&str> {
    v.as_str().filter(|f| {
        f.len() <= 40
            && f.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b' ' || b == b'-')
    })
}

fn kinetic_style(st: &Value) -> Value {
    let weight = |k: &str| {
        st[k]
            .as_u64()
            .filter(|w| (100..=900).contains(w))
            .unwrap_or(700)
    };
    let size = |k: &str, hi: f64, def: f64| {
        st[k]
            .as_f64()
            .filter(|v| (20.0..=hi).contains(v))
            .unwrap_or(def)
    };
    json!({
        "font": safe_font_name(&st["font"]).unwrap_or("sans"),
        "hero_font": safe_font_name(&st["hero_font"]),
        "weight": weight("weight"),
        "hero_weight": weight("hero_weight"),
        "hero_italic": st["hero_italic"].as_bool().unwrap_or(false),
        "size": size("size", 300.0, 64.0),
        "hero_size": size("hero_size", 400.0, 110.0),
        "color": hex_color(&st["color"]).unwrap_or(json!("#173F49")),
        "hero_color": hex_color(&st["hero_color"]).unwrap_or(json!("#B8442A")),
        "outline": hex_color(&st["outline"])
    })
}

fn validate_custom_drawing(shape: &Value) -> Result<Value> {
    let path = shape["d"]
        .as_str()
        .filter(|v| {
            !v.is_empty()
                && v.len() <= 16000
                && v.chars()
                    .all(|c| c.is_ascii_digit() || " ,.-eMmLlHhVvCcSsQqTtAaZz".contains(c))
        })
        .ok_or_else(|| invalid("drawing.d must be SVG path data"))?;
    let vb = shape["view_box"].as_str().unwrap_or("");
    let nums: Vec<f64> = vb
        .split_whitespace()
        .filter_map(|n| n.parse::<f64>().ok())
        .collect();
    if nums.len() != 4 || !nums.iter().all(|n| n.is_finite()) || nums[2] <= 0.0 || nums[3] <= 0.0 {
        return Err(invalid(
            "drawing.view_box must contain four finite numbers and positive dimensions",
        ));
    }
    Ok(json!({"name":shape["name"].as_str().unwrap_or("Drawing"),"view_box":vb,"d":path}))
}

fn apply_bounded_fields(
    target: &mut Value,
    src: &Value,
    prefix: &str,
    fields: &[(&str, f64, f64)],
) -> Result<()> {
    for (key, lo, hi) in fields {
        if let Some(v) = src.get(*key) {
            let label = if prefix.is_empty() {
                (*key).to_string()
            } else {
                format!("{prefix}.{key}")
            };
            target[*key] = bounded_num(v, &label, *lo, *hi)?;
        }
    }
    Ok(())
}

fn validate_kinetic_draw(d: &Value, ids: &[&Value], lib: &Value) -> Result<Value> {
    let place = d["place"].as_str().unwrap_or("under");
    if place != "beside" && place != "under" {
        return Err(invalid("kinetic.draws place must be under or beside"));
    }
    if !ids.contains(&&d["word_id"]) {
        return Err(invalid(format!(
            "kinetic.draws: {} is not a word of this section",
            d["word_id"]
        )));
    }
    let custom = match d.get("drawing").filter(|s| !s.is_null()) {
        Some(shape) => validate_custom_drawing(shape)?,
        None => Value::Null,
    };
    let stroke = if custom.is_null() {
        named_lib(lib, "strokes", &d["stroke"])?
    } else {
        Value::Null
    };
    let mut draw = json!({
        "word_id": d["word_id"],
        "stroke": stroke,
        "drawing": custom,
        "place": place
    });
    apply_bounded_fields(
        &mut draw,
        d,
        "drawing",
        &[
            ("offset", -600.0, 600.0),
            ("duration", 0.05, 30.0),
            ("hold", 0.0, 30.0),
            ("x", -2000.0, 6000.0),
            ("y", -2000.0, 6000.0),
            ("width", 1.0, 6000.0),
        ],
    )?;
    if let Some(color) = d.get("color") {
        draw["color"] =
            hex_color(color).ok_or_else(|| invalid("drawing.color must be a hex colour"))?;
    }
    if let Some(track) = d.get("track") {
        draw["track"] = one_of(track, &["lettering", "graphics"], "drawing.track")?;
    }
    Ok(draw)
}

fn named_lib(lib: &Value, kind: &str, v: &Value) -> Result<Value> {
    v.as_str()
        .filter(|n| lib[kind][*n].is_object())
        .map(|n| json!(n))
        .ok_or_else(|| invalid(format!("kinetic: {v} is not one of the library's {kind}")))
}

fn bounded_num(v: &Value, what: &str, lo: f64, hi: f64) -> Result<Value> {
    v.as_f64()
        .filter(|n| n.is_finite() && (*n >= lo && *n <= hi))
        .map(|n| json!(n))
        .ok_or_else(|| invalid(format!("{what} must be between {lo} and {hi}")))
}

fn validate_kinetic_acts(
    k: &Value,
    ids: &[&Value],
    lib: &Value,
) -> Result<serde_json::Map<String, Value>> {
    let mut acts = serde_json::Map::new();
    for (word, act) in k["acts"].as_object().into_iter().flatten() {
        if !ids.contains(&&json!(word)) {
            return Err(invalid(format!(
                "kinetic.acts: {word} is not a word of this section"
            )));
        }
        acts.insert(word.clone(), named_lib(lib, "acts", act)?);
    }
    Ok(acts)
}

/// A section's kinetic design, checked against the kinetic library.
pub(crate) fn kinetic_design(words: &Value, k: &Value) -> Result<Value> {
    let lib: Value = serde_json::from_str(KINETIC_LIBRARY).map_err(|e| invalid(e.to_string()))?;
    let ids: Vec<&Value> = words
        .as_array()
        .into_iter()
        .flatten()
        .map(|w| &w["word_id"])
        .collect();
    let acts = validate_kinetic_acts(k, &ids, &lib)?;
    if !k["draws"].is_null() && !k["draws"].is_array() {
        return Err(invalid("kinetic.draws must be an array"));
    }
    let draws: Vec<Value> = k["draws"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|d| validate_kinetic_draw(d, &ids, &lib))
        .collect::<Result<_>>()?;
    let overlay = if k["overlay"].is_null() {
        json!("none")
    } else {
        named_lib(&lib, "overlays", &k["overlay"])?
    };
    let mut result = json!({
        "layout": named_lib(&lib, "layouts", &k["layout"])?,
        "entrance": named_lib(&lib, "entrances", &k["entrance"])?,
        "acts": acts,
        "draws": draws,
        "overlay": overlay,
        "band": if k["band"] == "bottom" { "bottom" } else { "top" },
        "seed": k["seed"].as_u64().unwrap_or(1),
        "style": kinetic_style(&k["style"])
    });
    apply_bounded_fields(
        &mut result,
        k,
        "",
        &[
            ("overlay_offset", -600.0, 600.0),
            ("overlay_duration", 0.05, 600.0),
        ],
    )?;
    Ok(result)
}

fn short_colour(v: &Value) -> Result<Value> {
    v.as_str()
        .filter(|c| c.len() <= 32 && !c.contains(['<', '>', '"', ';']))
        .map(|c| json!(c))
        .ok_or_else(|| invalid("colour must be a short CSS colour"))
}

fn collect_line_item_words<'a>(
    item: &'a Value,
    words: &[Value],
) -> Result<(Vec<&'a str>, String, Value)> {
    let ids: Vec<&'a str> = item["word_ids"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if ids.is_empty() {
        return Err(invalid("each item needs word_ids"));
    }
    let text: Vec<&str> = ids
        .iter()
        .map(|id| {
            words
                .iter()
                .find(|w| w["word_id"] == json!(id))
                .and_then(|w| w["text"].as_str())
                .ok_or_else(|| invalid(format!("{id} is not a word of this line")))
        })
        .collect::<Result<_>>()?;
    let accent = match &item["accent"] {
        Value::Null => Value::Null,
        a => a
            .as_str()
            .filter(|a| ids.contains(a))
            .map(|a| json!(a))
            .ok_or_else(|| invalid("accent must be one of the item's words"))?,
    };
    Ok((ids, text.join(" "), accent))
}

fn validate_line_item<'a>(
    item: &'a Value,
    words: &[Value],
    placed: &mut Vec<&'a str>,
) -> Result<Value> {
    let (ids, text, accent) = collect_line_item_words(item, words)?;
    placed.extend(&ids);
    let weight = item["weight"]
        .as_u64()
        .filter(|w| [400, 700].contains(w))
        .ok_or_else(|| invalid("weight must be 400 or 700"))?;
    let italic = item["italic"]
        .as_bool()
        .ok_or_else(|| invalid("italic must be true or false"))?;
    let outline = if item["outline"].is_null() {
        Value::Null
    } else {
        short_colour(&item["outline"])?
    };
    Ok(json!({
        "word_ids": ids,
        "text": text,
        "x": bounded_num(&item["x"], "x", -2000.0, 6000.0)?,
        "y": bounded_num(&item["y"], "y", -2000.0, 6000.0)?,
        "size": bounded_num(&item["size"], "size", 8.0, 600.0)?,
        "weight": weight,
        "italic": italic,
        "color": short_colour(&item["color"])?,
        "font": one_of(item.get("font").unwrap_or(&json!("song")), FONTS, "font")?,
        "outline": outline,
        "rotate": bounded_num(&item["rotate"], "rotate", -45.0, 45.0)?,
        "motion": one_of(&item["motion"], MOVES, "motion")?,
        "accent": accent
    }))
}

fn validate_line_stroke_path(path: &Value) -> Result<Value> {
    let d = path["d"]
        .as_str()
        .filter(|d| {
            !d.is_empty()
                && d.chars()
                    .all(|c| c.is_ascii_digit() || " ,.-eMmLlHhVvCcSsQqTtAaZz".contains(c))
        })
        .ok_or_else(|| invalid("a stroke's d must be SVG path data"))?;
    let from = bounded_num(&path["from"], "from", 0.0, 600.0)?;
    let to = bounded_num(&path["to"], "to", 0.0, 600.0)?;
    if from.as_f64() > to.as_f64() {
        return Err(invalid("a stroke finishes drawing after it starts"));
    }
    Ok(json!({
        "d": d,
        "stroke": short_colour(&path["stroke"])?,
        "width": bounded_num(&path["width"], "width", 0.5, 60.0)?,
        "from": from,
        "to": to
    }))
}

fn validate_line_draw(draw: &Value) -> Result<Value> {
    let paths: Vec<Value> = draw["paths"]
        .as_array()
        .ok_or_else(|| invalid("a drawing needs paths"))?
        .iter()
        .map(validate_line_stroke_path)
        .collect::<Result<_>>()?;
    let view_box = draw["view_box"]
        .as_str()
        .filter(|v| {
            v.split_whitespace().count() == 4
                && v.split_whitespace().all(|n| n.parse::<f64>().is_ok())
        })
        .ok_or_else(|| invalid("view_box must be four numbers"))?;
    Ok(json!({
        "name": draw["name"].as_str().unwrap_or("Drawing"),
        "view_box": view_box,
        "x": bounded_num(&draw["x"], "x", -2000.0, 6000.0)?,
        "y": bounded_num(&draw["y"], "y", -2000.0, 6000.0)?,
        "w": bounded_num(&draw["w"], "w", 1.0, 6000.0)?,
        "h": bounded_num(&draw["h"], "h", 1.0, 6000.0)?,
        "paths": paths
    }))
}

/// Check one line's design. Items group the line's own words, all of them, in sung order.
pub(crate) fn line_design(words: &[Value], design: &Value) -> Result<Value> {
    let mut placed: Vec<&str> = Vec::new();
    let items: Vec<Value> = design["items"]
        .as_array()
        .ok_or_else(|| invalid("a design needs items"))?
        .iter()
        .map(|item| validate_line_item(item, words, &mut placed))
        .collect::<Result<_>>()?;
    let line: Vec<&str> = words.iter().filter_map(|w| w["word_id"].as_str()).collect();
    if placed != line {
        return Err(invalid(
            "the items must hold every word of the line once, in sung order",
        ));
    }
    let draws: Vec<Value> = design["draws"]
        .as_array()
        .into_iter()
        .flatten()
        .map(validate_line_draw)
        .collect::<Result<_>>()?;
    Ok(json!({"items":items,"draws":draws}))
}

fn declared_variables(component: &str) -> Vec<Value> {
    let Some(at) = component.find("data-composition-variables='") else {
        return Vec::new();
    };
    let rest = &component[at + "data-composition-variables='".len()..];
    rest.split('\'')
        .next()
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_default()
}

pub(crate) fn text_effect(hf: &Path, choice: &Value) -> Result<Value> {
    let name = choice["name"]
        .as_str()
        .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
        .ok_or_else(|| invalid("effect name must be letters, digits or '-'"))?;
    let src = format!("compositions/components/{name}.html");
    let component = fs::read_to_string(hf.join(&src)).map_err(|_| {
        invalid(format!(
            "{name} is not installed in this project; run words.effect_install first"
        ))
    })?;
    let declared = declared_variables(&component);
    let text = declared
        .iter()
        .find(|v| v["type"] == "string" && v["role"] == "content")
        .and_then(|v| v["id"].as_str())
        .ok_or_else(|| {
            invalid(format!(
                "{name} takes no text, so it cannot show a lyric line"
            ))
        })?;
    let cues = declared.iter().any(|v| v["id"] == "cues");
    let mut variables = serde_json::Map::new();
    for (key, value) in choice["variables"].as_object().into_iter().flatten() {
        if key == text || key == "cues" || !declared.iter().any(|v| v["id"] == json!(key)) {
            return Err(invalid(format!("{name} has no settable variable {key}")));
        }
        variables.insert(key.clone(), value.clone());
    }
    Ok(json!({"name":name,"src":src,"text_variable":text,"cues":cues,"variables":variables}))
}
