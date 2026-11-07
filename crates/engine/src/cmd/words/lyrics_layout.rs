//! Native Rust lyric line layout from a project's `lyrics-design.json` recipe.
//!
//! Replaces `pipeline/lyrics_layout.py` so `words.design` runs in-process without Python or PIL.

use serde_json::{Map, Value, json};

const GAP: f64 = 0.24;

struct StrokeSpec {
    view_box: &'static str,
    d: &'static str,
    length: f64,
    w: f64,
    h: f64,
}

fn stroke_spec(name: &str) -> Option<StrokeSpec> {
    match name {
        "underline" => Some(StrokeSpec { view_box: "0 0 280 30", d: "M 5 14 Q 61 5 125 13 T 269 10", length: 0.55, w: 280.0, h: 30.0 }),
        "swoop" | "rising-swoop" => Some(StrokeSpec {
            view_box: "0 0 790 160",
            d: "M 10 145 Q 126 170 217 93 T 438 58 Q 578 29 756 7 M 730 5 L 761 5 L 746 29",
            length: 1.6,
            w: 790.0,
            h: 160.0,
        }),
        "loop" => Some(StrokeSpec {
            view_box: "0 0 620 110",
            d: "M 15 20 C 110 95 425 95 455 40 C 485 -4 350 -3 352 37 C 355 95 515 85 602 50 M 579 33 L 608 49 L 586 73",
            length: 1.6,
            w: 620.0,
            h: 110.0,
        }),
        "hill" => Some(StrokeSpec { view_box: "0 0 410 95", d: "M 7 77 Q 73 72 118 32 Q 171 -8 228 31 Q 279 75 400 76", length: 1.4, w: 410.0, h: 95.0 }),
        "down" | "arrow-down" => Some(StrokeSpec {
            view_box: "0 0 160 330",
            d: "M 73 13 C 27 67 104 129 66 196 Q 47 231 61 289 M 30 264 L 61 294 L 87 260",
            length: 1.0,
            w: 160.0,
            h: 330.0,
        }),
        _ => None,
    }
}

fn lettering_tuple(name: &str) -> (&'static str, i64, bool) {
    match name {
        "song-bold" => ("song", 700, false),
        "serif-italic" => ("serif", 400, true),
        "serif-bold" => ("serif", 700, false),
        "sans-bold" => ("sans", 700, false),
        _ => ("sans", 400, false),
    }
}

fn band_baselines(band: &str, two_rows: bool) -> &'static [f64] {
    match (band, two_rows) {
        ("bottom", false) => &[985.0],
        ("bottom", true) => &[865.0, 990.0],
        (_, false) => &[150.0],
        (_, true) => &[135.0, 270.0],
    }
}

fn word_key(text: &str) -> String {
    text.chars().filter(|c| c.is_ascii_alphabetic()).map(|c| c.to_ascii_lowercase()).collect()
}

fn resolve_colour(palette: &Value, raw: Option<&str>) -> Value {
    let Some(c) = raw.filter(|s| !s.is_empty()) else {
        return Value::Null;
    };
    palette.get(c).and_then(Value::as_str).map(|s| json!(s)).unwrap_or_else(|| json!(c))
}

fn build_look(recipe: &Value, spec: &Value) -> Value {
    let pal = &recipe["palette"];
    let (font, weight, italic) = lettering_tuple(spec["lettering"].as_str().unwrap_or("sans"));
    let size = spec["size"].as_f64().unwrap_or(56.0);
    let color = resolve_colour(pal, Some(spec["color"].as_str().unwrap_or("#000000")));
    let outline = resolve_colour(pal, spec["outline"].as_str());
    let motion = spec["motion"].as_str().unwrap_or("quiet");
    json!({
        "font": font,
        "weight": weight,
        "italic": italic,
        "size": size,
        "color": color,
        "outline": outline,
        "motion": motion,
    })
}

fn glyph_factor(ch: char) -> f64 {
    match ch {
        ' ' => 0.28,
        'i' | 'l' | 'I' | '!' | '.' | ',' | '\'' | ':' | ';' => 0.28,
        'm' | 'w' | 'M' | 'W' => 0.78,
        c if c.is_ascii_uppercase() => 0.64,
        _ => 0.54,
    }
}

fn measure_width(text: &str, look: &Value, scale: f64) -> f64 {
    let size = look["size"].as_f64().unwrap_or(56.0) * scale;
    let bold_mul = if look["weight"].as_i64().unwrap_or(400) >= 700 { 1.06 } else { 1.0 };
    let glyph_w: f64 = text.chars().map(|c| glyph_factor(c) * size * bold_mul).sum();
    let outline_extra = if look["outline"].is_string() { 0.07 } else { 0.0 };
    let italic_extra = if look["italic"].as_bool().unwrap_or(false) { 0.12 } else { 0.0 };
    glyph_w + size * (outline_extra + italic_extra)
}

#[derive(Clone)]
struct PlacedGroup {
    words: Vec<Value>,
    look: Value,
    hero: Option<Value>,
    key: Option<String>,
}

fn push_word_to_groups(raw: &mut Vec<(Vec<Value>, Option<Value>, Option<String>)>, w: &Value, k: String, hero: Option<Value>, budget: &mut u64) {
    let same_hero = hero.is_some() && raw.last().and_then(|(_, _, prev)| prev.as_deref()).is_some_and(|p| p == k);
    if same_hero {
        if let Some(last) = raw.last_mut() {
            last.0.push(w.clone());
        }
    } else if let Some(h) = hero.filter(|_| *budget > 0) {
        *budget -= 1;
        raw.push((vec![w.clone()], Some(h), Some(k)));
    } else if raw.last().is_some_and(|(_, _, prev)| prev.is_none()) {
        if let Some(last) = raw.last_mut() {
            last.0.push(w.clone());
        }
    } else {
        raw.push((vec![w.clone()], None, None));
    }
}

fn group_words(recipe: &Value, words: &[Value]) -> Vec<PlacedGroup> {
    let heroes = &recipe["heroes"];
    let mut budget = recipe["max_heroes"].as_u64().unwrap_or(2);
    let mut raw: Vec<(Vec<Value>, Option<Value>, Option<String>)> = Vec::new();
    for w in words {
        let k = word_key(w["text"].as_str().unwrap_or_default());
        let hero = (!k.is_empty()).then(|| heroes.get(&k).filter(|v| v.is_object()).cloned()).flatten();
        push_word_to_groups(&mut raw, w, k, hero, &mut budget);
    }
    raw.into_iter()
        .map(|(ws, hero, key)| {
            let look = build_look(recipe, hero.as_ref().unwrap_or(&recipe["base"]));
            PlacedGroup { words: ws, look, hero, key }
        })
        .collect()
}

fn group_text(words: &[Value]) -> String {
    words.iter().filter_map(|w| w["text"].as_str()).collect::<Vec<_>>().join(" ")
}

fn row_metrics(row: &[PlacedGroup], scale: f64) -> (Vec<f64>, Vec<f64>) {
    let ws: Vec<f64> = row.iter().map(|g| measure_width(&group_text(&g.words), &g.look, scale)).collect();
    let gaps: Vec<f64> = row
        .windows(2)
        .map(|pair| {
            let a = pair[0].look["size"].as_f64().unwrap_or(56.0);
            let b = pair[1].look["size"].as_f64().unwrap_or(56.0);
            GAP * a.max(b) * scale
        })
        .collect();
    (ws, gaps)
}

fn fit_row(row: &[PlacedGroup], frame_w: f64, margin: f64) -> (f64, Vec<f64>, Vec<f64>) {
    let mut scale = 1.0_f64;
    let max_w = (frame_w - 2.0 * margin).max(200.0);
    loop {
        let (ws, gaps) = row_metrics(row, scale);
        let total: f64 = ws.iter().sum::<f64>() + gaps.iter().sum::<f64>();
        if total <= max_w || scale <= 0.35 {
            return (scale, ws, gaps);
        }
        scale *= 0.96;
    }
}

fn split_two_rows(placed: &[PlacedGroup]) -> Vec<Vec<PlacedGroup>> {
    let full: Vec<f64> = placed.iter().map(|g| measure_width(&group_text(&g.words), &g.look, 1.0)).collect();
    let total: f64 = full.iter().sum();
    let mut best_cut = 1;
    let mut best_diff = f64::MAX;
    let mut prefix = 0.0;
    for (idx, w) in full.iter().enumerate().take(placed.len().saturating_sub(1)) {
        prefix += *w;
        let diff = (prefix - (total - prefix)).abs();
        if diff < best_diff {
            best_diff = diff;
            best_cut = idx + 1;
        }
    }
    vec![placed[..best_cut].to_vec(), placed[best_cut..].to_vec()]
}

struct PlacedBox {
    x: f64,
    top: f64,
    w: f64,
    size: f64,
    last_start: Option<f64>,
    hero: Option<Value>,
    color: Value,
}

fn build_hero_stroke_draw(boxes: &[PlacedBox], words: &[Value], frame_w: f64, frame_h: f64) -> Vec<Value> {
    let first_sung = words.iter().find_map(|w| w["start"].as_f64());
    for b in boxes {
        let Some(stroke_name) = b.hero.as_ref().and_then(|h| h["stroke"].as_str()).filter(|s| !s.is_empty()) else {
            continue;
        };
        let Some(spec) = stroke_spec(stroke_name) else {
            continue;
        };
        let (dw, dh, sx, sy) = if stroke_name == "down" || stroke_name == "arrow-down" {
            let dh = b.size * 1.2;
            let dw = dh * spec.w / spec.h;
            ((dw), (dh), (b.x + b.w + 24.0).min(frame_w - dw - 20.0), b.top + b.size * 0.1)
        } else {
            let dw = b.w.max(160.0);
            let dh = dw * spec.h / spec.w;
            (dw, dh, b.x + (b.w - dw) / 2.0, b.top + b.size * 1.08)
        };
        let start = match (first_sung, b.last_start) {
            (Some(s0), Some(last)) => ((last - s0).max(0.0) * 100.0).round() / 100.0,
            _ => 0.0,
        };
        let mut label = stroke_name.chars();
        let cap = match label.next() {
            Some(first) => first.to_uppercase().collect::<String>() + label.as_str(),
            None => String::new(),
        };
        return vec![json!({
            "name": cap,
            "view_box": spec.view_box,
            "x": sx.round() as i64,
            "y": sy.min(frame_h - dh - 10.0).round() as i64,
            "w": dw.round() as i64,
            "h": dh.round() as i64,
            "paths": [{
                "d": spec.d,
                "stroke": b.color,
                "width": 7,
                "from": start,
                "to": ((start + spec.length) * 100.0).round() / 100.0,
            }],
        })];
    }
    Vec::new()
}

fn design_line(recipe: &Value, words: &[Value], band: &str) -> Value {
    let frame_w = recipe["frame"][0].as_f64().unwrap_or(1920.0);
    let frame_h = recipe["frame"][1].as_f64().unwrap_or(1080.0);
    let margin = recipe["margin"].as_f64().unwrap_or(110.0);
    let placed = group_words(recipe, words);
    let (scale, _, _) = fit_row(&placed, frame_w, margin);
    let rows = if scale < 0.85 && placed.len() > 1 { split_two_rows(&placed) } else { vec![placed] };
    let baselines = band_baselines(band, rows.len() > 1);
    let mut items = Vec::new();
    let mut boxes = Vec::new();
    for (baseline, row) in baselines.iter().zip(rows.iter()) {
        let (row_scale, ws, gaps) = fit_row(row, frame_w, margin);
        let total_w: f64 = ws.iter().sum::<f64>() + gaps.iter().sum::<f64>();
        let mut x = (frame_w - total_w) / 2.0;
        for (k, (g, w)) in row.iter().zip(ws.iter()).enumerate() {
            let size = (g.look["size"].as_f64().unwrap_or(56.0) * row_scale).round();
            let top = baseline - size * 0.92;
            let word_ids: Vec<Value> = g.words.iter().map(|w_| w_["word_id"].clone()).collect();
            let accent = if g.look["motion"].as_str().unwrap_or("quiet") != "quiet" {
                g.words.last().map(|w_| w_["word_id"].clone()).unwrap_or(Value::Null)
            } else {
                Value::Null
            };
            let mut item = g.look.clone();
            item["size"] = json!(size as i64);
            item["word_ids"] = json!(word_ids);
            item["x"] = json!(x.round() as i64);
            item["y"] = json!(top.round() as i64);
            item["rotate"] = json!(0);
            item["accent"] = accent;
            boxes.push(PlacedBox {
                x,
                top,
                w: *w,
                size,
                last_start: g.words.last().and_then(|w_| w_["start"].as_f64()),
                hero: g.hero.clone().or_else(|| g.key.as_ref().map(|_| Value::Null)),
                color: item["color"].clone(),
            });
            items.push(item);
            x += *w + gaps.get(k).copied().unwrap_or(0.0);
        }
    }
    json!({
        "items": items,
        "draws": build_hero_stroke_draw(&boxes, words, frame_w, frame_h),
    })
}

/// Produce `{ animation_id: { line_id: design } }` for every sung line in `animations`.
pub(crate) fn designs(recipe: &Value, animations: &[Value]) -> Value {
    let mut out = Map::new();
    let default_band = recipe["band"].as_str().unwrap_or("top");
    for anim in animations {
        let section = anim["section"].as_str().unwrap_or_default();
        let anim_band = recipe["bands"][section].as_str().unwrap_or(default_band);
        let mut line_order: Vec<String> = Vec::new();
        let mut line_words: Map<String, Value> = Map::new();
        for w in anim["words"].as_array().into_iter().flatten() {
            let lid = w["line_id"].as_str().unwrap_or_default().to_owned();
            if !line_words.contains_key(&lid) {
                line_order.push(lid.clone());
                line_words.insert(lid.clone(), json!([]));
            }
            if let Some(arr) = line_words.get_mut(&lid).and_then(Value::as_array_mut) {
                arr.push(w.clone());
            }
        }
        let mut anim_designs = Map::new();
        for lid in line_order {
            let ws = line_words.get(&lid).and_then(Value::as_array).cloned().unwrap_or_default();
            let band = recipe["lines"][&lid]["band"].as_str().unwrap_or(anim_band);
            anim_designs.insert(lid, design_line(recipe, &ws, band));
        }
        if let Some(id) = anim["animation_id"].as_str() {
            out.insert(id.to_owned(), Value::Object(anim_designs));
        }
    }
    Value::Object(out)
}
