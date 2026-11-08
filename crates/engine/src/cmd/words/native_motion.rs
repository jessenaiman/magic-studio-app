//! Read and write word motion on the real lyrics EffectCraft project.
//!
//! `lettering/lyrics.ecproj` is authoritative for a word group's transform keys. The panel
//! discovers the comp's transform properties from that project, `set` writes the keyframes
//! there with `prop.addKey`, saves it, and then records only the binding (comp/layer ids plus
//! the source word ids) and a read-back of the keys in words.json. `words.letter` remains the
//! full-rebuild path.
use super::{invalid, motion_curve, stage};
use crate::cmd::character::effectcraft;
use crate::{Result, Session};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const COMP: &str = "Lyrics";
const FRAME_WIDTH: f64 = 1920.0;
const PROPERTIES: [&str; 4] = ["transform/position", "transform/rotation", "transform/scale", "transform/opacity"];

fn project_path(dir: &Path) -> PathBuf {
    dir.join("lettering/lyrics.ecproj")
}

fn step(command: &str, params: Value) -> Value {
    json!({"command": command, "params": params})
}

fn number(value: &Value) -> Option<f64> {
    value.as_f64().filter(|v| v.is_finite())
}

fn rounded(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

/// The comp's transform property group, limited to the properties a word curve can drive.
fn transform_properties(tree: &Value) -> Vec<Value> {
    tree["properties"]["children"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|group| group["path"] == "transform")
        .and_then(|group| group["children"].as_array())
        .into_iter()
        .flatten()
        .filter(|property| property["path"].as_str().is_some_and(|path| PROPERTIES.contains(&path)))
        .cloned()
        .collect()
}

fn transform_property<'a>(tree: &'a Value, path: &str) -> Option<&'a Value> {
    tree["properties"]["children"]
        .as_array()?
        .iter()
        .find(|group| group["path"] == "transform")?
        .get("children")?
        .as_array()?
        .iter()
        .find(|property| property["path"] == path)
}

fn has_keys(property: &Value) -> bool {
    property["keys"].as_array().is_some_and(|keys| !keys.is_empty())
}

/// One comp layer by id.
fn layer_by_id(layers: &[Value], id: u64) -> Option<&Value> {
    layers.iter().find(|layer| layer["id"].as_u64() == Some(id))
}

/// The first text layer, used when words.json records no binding yet.
fn first_text_layer(layers: &[Value]) -> Option<&Value> {
    layers.iter().find(|layer| layer["type"] == "Text")
}

/// Match `line` or `line-2`, `line-3`, … plus the item suffix the lettering build gives a group.
fn layer_matches_group(layer: &Value, line: &str, index: usize) -> bool {
    let Some(name) = layer["name"].as_str() else {
        return false;
    };
    let suffix = format!("_item{index}");
    let Some(stem) = name.strip_suffix(&suffix) else {
        return false;
    };
    if stem == line {
        return true;
    }
    stem.strip_prefix(line)
        .and_then(|rest| rest.strip_prefix('-'))
        .is_some_and(|occurrence| !occurrence.is_empty() && occurrence.bytes().all(|byte| byte.is_ascii_digit()))
}

/// Resolve the layer for one word group: the recorded binding first, else the naming the
/// lettering build uses, narrowed to the occurrence whose active range contains the word.
fn find_layer<'a>(layers: &'a [Value], line: &str, index: usize, start: f64, binding: Option<&Value>) -> Option<&'a Value> {
    if let Some(layer) = binding.and_then(|binding| binding["layer"]["id"].as_u64()).and_then(|id| layer_by_id(layers, id))
        && binding.and_then(|binding| binding["layer"]["name"].as_str()).is_none_or(|name| layer["name"] == name)
    {
        return Some(layer);
    }
    let candidates: Vec<&Value> = layers.iter().filter(|layer| layer_matches_group(layer, line, index)).collect();
    let contains = |layer: &&Value| {
        let begin = layer["in"].as_f64().unwrap_or(f64::NEG_INFINITY);
        let end = layer["out"].as_f64().unwrap_or(f64::INFINITY);
        start >= begin - 0.001 && start <= end + 0.001
    };
    candidates
        .iter()
        .copied()
        .filter(contains)
        .min_by(|a, b| {
            let left = (a["in"].as_f64().unwrap_or_default() - start).abs();
            let right = (b["in"].as_f64().unwrap_or_default() - start).abs();
            left.total_cmp(&right)
        })
        .or_else(|| {
            candidates.iter().copied().min_by(|a, b| {
                let left = (a["in"].as_f64().unwrap_or_default() - start).abs();
                let right = (b["in"].as_f64().unwrap_or_default() - start).abs();
                left.total_cmp(&right)
            })
        })
}

/// The word group's source item in words.json, with the animation and line ids.
fn find_item<'a>(document: &'a Value, animation_id: &str, line: &str, word_ids: &[Value]) -> Result<(&'a Value, &'a Value, &'a Value)> {
    let animation = document["animations"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|animation| animation["animation_id"] == animation_id)
        .ok_or_else(|| invalid("No matching lyric section"))?;
    let design = animation["design"].get(line).ok_or_else(|| invalid("No design for this lyric line"))?;
    let item = design["items"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|item| item["word_ids"].as_array().is_some_and(|ids| ids.as_slice() == word_ids))
        .ok_or_else(|| invalid("Choose an existing word/phrase group; split or group it in Lyrics first"))?;
    Ok((animation, design, item))
}

/// The first binding words.json records, optionally narrowed to a line and word group.
fn find_binding<'a>(document: &'a Value, line: Option<&str>, word_ids: Option<&[Value]>) -> Option<(&'a str, &'a str, &'a Value)> {
    for animation in document["animations"].as_array().into_iter().flatten() {
        let animation_id = animation["animation_id"].as_str().unwrap_or_default();
        let Some(design) = animation["design"].as_object() else {
            continue;
        };
        for (line_id, line_design) in design {
            if line.is_some_and(|wanted| wanted != line_id) {
                continue;
            }
            for item in line_design["items"].as_array().into_iter().flatten() {
                let binding = &item["native_motion_binding"];
                if !binding.is_object() {
                    continue;
                }
                if word_ids.is_some_and(|wanted| item["word_ids"].as_array().map(Vec::as_slice) != Some(wanted)) {
                    continue;
                }
                return Some((animation_id, line_id, binding));
            }
        }
    }
    None
}

/// `words.motion.properties`: the transform tree of the real lyrics comp, plus the binding
/// words.json records for the requested (or first) word group.
pub fn properties(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::cmd::words::workspace_dir(s)?;
    let project = project_path(&dir);
    if !project.is_file() {
        return Err(invalid("No lettering/lyrics.ecproj in this project; build the lettering first"));
    }
    let read = stage::cmd_words_read(s, &json!({}))?;
    let document = read["document"].clone();
    let line = p["line_id"].as_str();
    let word_ids: Option<Vec<Value>> = p["word_ids"].as_array().cloned();
    let binding = find_binding(&document, line, word_ids.as_deref());

    let info = effectcraft(&dir, Some(&project), &[step("comp.info", json!({"comp": COMP}))], false)?;
    let comp = info.first().cloned().unwrap_or(Value::Null);
    let layers = comp["layers"].as_array().cloned().unwrap_or_default();
    let binding = binding.filter(|(_, _, binding)| binding["layer"]["id"].as_u64().is_some_and(|id| layer_by_id(&layers, id).is_some()));
    let layer = binding
        .and_then(|(_, _, binding)| binding["layer"]["id"].as_u64())
        .and_then(|id| layer_by_id(&layers, id))
        .or_else(|| first_text_layer(&layers).or_else(|| layers.first()))
        .cloned()
        .ok_or_else(|| invalid("The lyrics EffectCraft project has no text layers"))?;
    let tree =
        effectcraft(&dir, Some(&project), &[step("layer.tree", json!({"layer": layer["id"], "depth": 2}))], false)?.first().cloned().unwrap_or(Value::Null);
    let properties = transform_properties(&tree);
    if properties.is_empty() {
        return Err(invalid("The lyrics EffectCraft layer exposes no transform properties"));
    }
    let binding = binding.map(|(animation_id, line_id, binding)| {
        json!({"animation_id": animation_id, "line_id": line_id, "comp": binding["comp"], "layer": binding["layer"], "word_ids": binding["word_ids"], "box": binding["box"]})
    });
    Ok(json!({
        "engine": "EffectCraft",
        "project": project,
        "comp": {"id": comp["id"], "name": comp["name"], "width": comp["width"], "height": comp["height"], "duration": comp["duration"]},
        "layer": {"id": layer["id"], "name": layer["name"], "in": layer["in"], "out": layer["out"]},
        "binding": binding,
        "properties": properties,
        "coordinate_mode": "offset from authored placement",
        "duration_range": [0.05, 8.0],
        "source_text_editable": false
    }))
}

pub fn curve(_s: &mut Session, p: &Value) -> Result<Value> {
    let (config, keys) = motion_curve::curve(p)?;
    Ok(json!({"engine":"EffectCraft","config":config,"motion_keys":keys}))
}

/// The word's text, as the lettering build spells it, for the box the native controls use.
fn item_text(item: &Value, animation: &Value, word_ids: &[Value]) -> String {
    if let Some(text) = item["text"].as_str().filter(|text| !text.is_empty()) {
        return text.to_owned();
    }
    let words = animation["words"].as_array().cloned().unwrap_or_default();
    word_ids
        .iter()
        .filter_map(Value::as_str)
        .filter_map(|id| words.iter().find(|word| word["word_id"] == id).and_then(|word| word["text"].as_str()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The layer-space origin the lettering build used: the centre of the native text box, or the
/// point-text baseline for older hand-authored curves.
fn uses_box(item: &Value) -> bool {
    item["native_motion_binding"]["box"].as_bool().or_else(|| item["native_motion"].is_object().then_some(true)).unwrap_or(false)
}

fn item_origin(item: &Value, animation: &Value, word_ids: &[Value]) -> (f64, f64) {
    let x = number(&item["x"]).unwrap_or(0.0);
    let y = number(&item["y"]).unwrap_or(0.0);
    if !uses_box(item) {
        return (x, y);
    }
    let size = number(&item["size"]).unwrap_or(58.0);
    let text = item_text(item, animation, word_ids);
    let width = ((text.chars().count() as f64) * size * 0.72).clamp(220.0, FRAME_WIDTH - 80.0);
    let height = (size * 1.5).max(96.0);
    (x + width / 2.0, y + height / 2.0)
}

/// The first key time and the values a comp writes, in words.json's seven-column curve rows.
fn read_back(tree: &Value, path: &str, origin: (f64, f64), rotate: f64) -> Result<Vec<Value>> {
    let property = transform_property(tree, path).ok_or_else(|| invalid(format!("The lyrics layer has no {path} property")))?;
    let keys = property["keys"].as_array().filter(|keys| keys.len() >= 2).ok_or_else(|| invalid("The lyrics comp did not record the new keyframes"))?;
    let first_time = number(&keys[0]["time"]).unwrap_or(0.0);
    let mut rows = Vec::with_capacity(keys.len());
    for key in keys {
        let time = rounded(number(&key["time"]).unwrap_or(first_time) - first_time);
        let mut row = vec![time, 0.0, 0.0, 100.0, 100.0, 0.0, 100.0];
        match path {
            "transform/position" => {
                let value = key["value"].as_array().cloned().unwrap_or_default();
                row[1] = rounded(number(value.first().unwrap_or(&Value::Null)).unwrap_or(origin.0) - origin.0);
                row[2] = rounded(number(value.get(1).unwrap_or(&Value::Null)).unwrap_or(origin.1) - origin.1);
            }
            "transform/scale" => {
                let value = key["value"].as_array().cloned().unwrap_or_default();
                row[3] = rounded(number(value.first().unwrap_or(&Value::Null)).unwrap_or(100.0));
                row[4] = rounded(number(value.get(1).unwrap_or(&Value::Null)).unwrap_or(100.0));
            }
            "transform/rotation" => row[5] = rounded(number(&key["value"]).unwrap_or(rotate) - rotate),
            _ => row[6] = rounded(number(&key["value"]).unwrap_or(100.0)),
        }
        rows.push(json!(row));
    }
    if !(2..=24).contains(&rows.len()) {
        return Err(invalid("The lyrics comp recorded an unsupported number of keyframes"));
    }
    Ok(rows)
}

/// `words.motion.set`: write a word group's curve into the existing lyrics comp and layer,
/// save it, then store only the binding and a read-back of the keys in words.json.
pub fn set(s: &mut Session, p: &Value) -> Result<Value> {
    let section = p["animation_id"].as_str().ok_or_else(|| invalid("animation_id is required"))?;
    let line = p["line_id"].as_str().ok_or_else(|| invalid("line_id is required"))?;
    let word_ids = p["word_ids"].as_array().filter(|ids| !ids.is_empty()).ok_or_else(|| invalid("word_ids must identify an existing word or phrase group"))?;
    let revision = p["expected_source_revision"].as_u64().ok_or_else(|| invalid("expected_source_revision is required"))?;
    let dir = crate::cmd::words::workspace_dir(s)?;
    let project = project_path(&dir);
    if !project.is_file() {
        return Err(invalid("No lettering/lyrics.ecproj in this project; build the lettering first"));
    }
    let read = stage::cmd_words_read(s, &json!({}))?;
    let document = read["document"].clone();
    let (animation, design, item) = find_item(&document, section, line, word_ids)?;
    if animation["source_revision"].as_u64() != Some(revision) {
        let current = animation["source_revision"].as_u64().unwrap_or(0);
        return Ok(json!({
            "ok": false,
            "conflict": true,
            "animation_id": section,
            "source_revision": current,
            "expected_source_revision": revision,
            "reason": "The animation changed since you read it; reload and reapply your edit"
        }));
    }
    let (config, keys) = motion_curve::curve(p)?;
    let property = config["property"].as_str().unwrap_or("transform/rotation");
    let rotate = number(&item["rotate"]).unwrap_or(0.0);
    let (origin_x, origin_y) = item_origin(item, animation, word_ids);
    let start = item["accent"]
        .as_str()
        .filter(|accent| word_ids.iter().any(|id| id.as_str() == Some(accent)))
        .or_else(|| word_ids.first().and_then(Value::as_str))
        .and_then(|accent| animation["words"].as_array().into_iter().flatten().find(|word| word["word_id"] == accent).and_then(|word| number(&word["start"])))
        .unwrap_or(0.0);

    let info = effectcraft(&dir, Some(&project), &[step("comp.info", json!({"comp": COMP}))], false)?;
    let comp = info.first().cloned().unwrap_or(Value::Null);
    let comp_id = comp["id"].as_u64().ok_or_else(|| invalid("The lyrics EffectCraft comp is missing its id"))?;
    let comp_name = comp["name"].as_str().unwrap_or(COMP).to_owned();
    let layers = comp["layers"].as_array().cloned().unwrap_or_default();
    let index = design["items"]
        .as_array()
        .into_iter()
        .flatten()
        .position(|candidate| candidate["word_ids"].as_array().is_some_and(|ids| ids.as_slice() == word_ids))
        .unwrap_or(0);
    let layer = find_layer(&layers, line, index, start, item.get("native_motion_binding"))
        .cloned()
        .ok_or_else(|| invalid("The lyrics comp has no layer for this word group; build the lettering first"))?;
    let layer_id = layer["id"].as_u64().ok_or_else(|| invalid("The lyrics layer is missing its id"))?;
    let layer_name = layer["name"].as_str().unwrap_or_default().to_owned();
    let tree = effectcraft(&dir, Some(&project), &[step("layer.tree", json!({"layer": layer_id, "depth": 2}))], false)?.first().cloned().unwrap_or(Value::Null);

    let animated: Vec<&str> = PROPERTIES.iter().copied().filter(|path| transform_property(&tree, path).is_some_and(has_keys)).collect();
    let mut steps = vec![step("comp.open", json!({"comp": comp_id})), step("time.set", json!({"time": (start - 0.25).max(0.0)}))];
    for path in &animated {
        steps.push(step("prop.toggleAnimation", json!({"layer": layer_id, "path": path})));
    }
    // A cleared property keeps the value it happened to hold at the current time; put each
    // one back at the authored placement before the new curve takes over.
    for path in &animated {
        let value = match *path {
            "transform/position" => json!([origin_x, origin_y, 0.0]),
            "transform/rotation" => json!(rotate),
            "transform/scale" => json!([100.0, 100.0, 100.0]),
            _ => json!(100.0),
        };
        steps.push(step("prop.set", json!({"layer": layer_id, "path": path, "value": value})));
    }
    let scalar = |value: &Value, fallback: f64| number(value).unwrap_or(fallback);
    let pair = |values: &[Value], fallback: (f64, f64)| {
        (number(values.first().unwrap_or(&Value::Null)).unwrap_or(fallback.0), number(values.get(1).unwrap_or(&Value::Null)).unwrap_or(fallback.1))
    };
    for key in keys.as_array().into_iter().flatten() {
        let row = key.as_array().cloned().unwrap_or_default();
        if row.len() != 7 {
            continue;
        }
        let at = start + number(&row[0]).unwrap_or(0.0);
        let value = match property {
            "transform/position" => {
                let (dx, dy) = pair(&row[1..3], (0.0, 0.0));
                json!([origin_x + dx, origin_y + dy, 0.0])
            }
            "transform/scale" => {
                let (sx, sy) = pair(&row[3..5], (100.0, 100.0));
                json!([sx, sy])
            }
            "transform/rotation" => json!(rotate + scalar(&row[5], 0.0)),
            _ => json!(scalar(&row[6], 100.0)),
        };
        steps.push(step("prop.addKey", json!({"layer": layer_id, "path": property, "value": value, "time": at, "timeBase": "comp"})));
    }
    effectcraft(&dir, Some(&project), &steps, true)?;
    let tree = effectcraft(&dir, Some(&project), &[step("layer.tree", json!({"layer": layer_id, "depth": 2}))], false)?.first().cloned().unwrap_or(Value::Null);
    let read_keys = read_back(&tree, property, (origin_x, origin_y), rotate)?;
    let binding = json!({
        "comp": {"id": comp_id, "name": comp_name},
        "layer": {"id": layer_id, "name": layer_name},
        "word_ids": word_ids,
        "box": uses_box(item)
    });

    let mut designs = json!({});
    let mut saved_design = design.clone();
    if let Some(items) = saved_design["items"].as_array_mut() {
        for candidate in items {
            if candidate["word_ids"].as_array().is_some_and(|ids| ids.as_slice() == word_ids) {
                candidate["motion_keys"] = json!(read_keys.clone());
                candidate["native_motion_binding"] = binding.clone();
                candidate["accent"] = word_ids[0].clone();
                if let Some(object) = candidate.as_object_mut() {
                    object.remove("native_motion");
                }
            }
        }
    }
    designs[line] = saved_design;
    let saved = stage::cmd_words_save(
        s,
        &json!({"animation_id": section, "expected_source_revision": revision, "design": designs, "origin": "native-transform-controls"}),
    )?;
    let mut reply = saved.clone();
    if let Some(object) = reply.as_object_mut() {
        object.insert("engine".into(), json!("EffectCraft"));
        object.insert("project".into(), json!(project));
        object.insert("binding".into(), binding);
        object.insert("config".into(), config);
        object.insert("motion_keys".into(), json!(read_keys));
    }
    Ok(reply)
}

/// `effect.save`: persist `lettering/lyrics.ecproj` with EffectCraft `file.save`.
///
/// Runs the single `file.save` command through `effectcraft` with `save: true`, so a live
/// EffectCraft saves its open document and a headless run writes the work copy back to the
/// project. `shot.commit` calls this before `film.save` and `project.save`.
pub fn cmd_effect_save(s: &mut Session, _p: &Value) -> Result<Value> {
    let dir = crate::cmd::words::workspace_dir(s)?;
    let project = project_path(&dir);
    if !project.is_file() {
        return Err(invalid("No lettering/lyrics.ecproj in this project; build the lettering first"));
    }
    effectcraft(&dir, Some(&project), &[step("file.save", json!({}))], true)?;
    Ok(json!({"ok": true, "project": project}))
}
