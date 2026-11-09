//! `character.keys`: a rig's real layers, keyframes and sampled curves, read from EffectCraft.
use super::{View, bad, effectcraft, step};
use crate::{Result, Session};
use serde_json::{Value, json};
use std::collections::HashMap;

/// The most samples one channel's curve is read at.
const MAX_SAMPLES: usize = 240;
/// Keys this close outside the window still belong to it (a tenth of a 240 fps frame).
const WINDOW_TOLERANCE: f64 = 1.0 / 2400.0;

/// The animated channel a sample batch reads.
struct Channel {
    layer: usize,
    meta: Value,
    keys: Vec<Value>,
}

/// The `transform/*` properties of a `layer.tree` reply.
fn transform_paths(tree: &Value) -> Vec<String> {
    let groups = tree["properties"]["children"].as_array().into_iter().flatten();
    let leaves = groups.filter(|group| group["path"] == "transform").flat_map(|group| group["children"].as_array().into_iter().flatten());
    leaves.filter_map(|leaf| leaf["path"].as_str().map(str::to_string)).collect()
}

/// The comp-time window of `action`: `None` when the rig has no keys for it.
fn action_window(view: &View, action: Option<&str>, duration: f64) -> Result<Option<(f64, f64)>> {
    let Some(name) = action else {
        return Ok(Some((0.0, duration)));
    };
    let parts = view.parts()?;
    let range = &parts["actions"][name];
    Ok(range["start"].as_f64().zip(range["end"].as_f64()).filter(|(start, end)| end > start))
}

/// Run `steps` after opening `comp`, and return their results without the open's.
pub(super) fn read(view: &View, project: &std::path::Path, comp: &str, steps: Vec<Value>) -> Result<Vec<Value>> {
    let mut batch = vec![step("comp.open", json!({"comp": comp}))];
    batch.extend(steps);
    let mut results = effectcraft(&view.dir, Some(project), &batch, false)?;
    if results.len() != batch.len() {
        return Err(bad(format!("EffectCraft answered {} of {} reads", results.len(), batch.len())));
    }
    results.remove(0);
    Ok(results)
}

/// `character.keys`: layers from `comp.info`/`layer.tree`, `keys[]` and sampled values from
/// `prop.get`, limited to the action's window (`parts.actions[name]`, or the whole comp).
pub fn cmd_character_keys(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    let comp = view.comp();
    let action = p["action"].as_str().filter(|name| !name.is_empty());

    let info = read(&view, &project, &comp, vec![step("comp.info", json!({"comp": comp}))])?.remove(0);
    let duration = info["duration"].as_f64().ok_or_else(|| bad("comp.info reported no duration"))?;
    let fps = info["frameRate"].as_f64().filter(|fps| *fps > 0.0).ok_or_else(|| bad("comp.info reported no frame rate"))?;
    let listed = info["layers"].as_array().cloned().unwrap_or_default();
    let names: HashMap<u64, String> = listed.iter().filter_map(|l| Some((l["id"].as_u64()?, l["name"].as_str()?.to_string()))).collect();

    let tree_steps = listed.iter().map(|l| step("layer.tree", json!({"layer": l["id"], "comp": comp, "depth": 2}))).collect();
    let trees = read(&view, &project, &comp, tree_steps)?;

    let window = action_window(&view, action, duration)?;
    let mut prop_steps = Vec::new();
    let mut prop_targets = Vec::new();
    for (layer, tree) in trees.iter().enumerate() {
        for path in transform_paths(tree) {
            prop_steps.push(step("prop.get", json!({"layer": listed[layer]["id"], "path": path, "comp": comp})));
            prop_targets.push((layer, path));
        }
    }
    let props = if window.is_some() { read(&view, &project, &comp, prop_steps)? } else { Vec::new() };

    let mut channels = Vec::new();
    if let Some((start, end)) = window {
        for (prop, (layer, path)) in props.iter().zip(prop_targets) {
            let inside = |key: &&Value| key["compTime"].as_f64().is_some_and(|t| t >= start - WINDOW_TOLERANCE && t <= end + WINDOW_TOLERANCE);
            let keys: Vec<Value> = prop["keys"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(inside)
                .map(|key| json!({"time": key["compTime"], "value": key["value"], "in": key["in"], "out": key["out"]}))
                .collect();
            if !keys.is_empty() {
                channels.push(Channel { layer, meta: json!({"path": path, "name": prop["name"], "type": prop["type"]}), keys });
            }
        }
    }

    let mut sample_steps = Vec::new();
    let mut count = 0;
    if let Some((start, end)) = window {
        count = (((end - start) * fps).ceil() as usize + 1).clamp(2, MAX_SAMPLES);
        for channel in &channels {
            for index in 0..count {
                let time = start + (end - start) * index as f64 / (count - 1) as f64;
                sample_steps.push(step("prop.get", json!({"layer": listed[channel.layer]["id"], "path": channel.meta["path"], "comp": comp, "time": time})));
            }
        }
    }
    let samples = if sample_steps.is_empty() { Vec::new() } else { read(&view, &project, &comp, sample_steps)? };

    let mut layers: Vec<Value> = listed
        .iter()
        .zip(&trees)
        .map(|(layer, tree)| {
            let parent = tree["parent"].as_u64().and_then(|id| names.get(&id)).cloned();
            json!({"id": layer["id"], "name": layer["name"], "parent": parent, "channels": []})
        })
        .collect();
    for (channel, curve) in channels.into_iter().zip(samples.chunks(count.max(1))) {
        let curve: Vec<Value> = curve.iter().map(|sample| json!({"time": sample["time"], "value": sample["value"]})).collect();
        let mut entry = channel.meta;
        entry["keys"] = json!(channel.keys);
        entry["samples"] = json!(curve);
        if let Some(list) = layers[channel.layer]["channels"].as_array_mut() {
            list.push(entry);
        }
    }

    let (start, end) = window.map_or((Value::Null, Value::Null), |(start, end)| (json!(start), json!(end)));
    Ok(json!({"ok": true, "comp": comp, "fps": fps, "action": action, "start": start, "end": end, "layers": layers}))
}
