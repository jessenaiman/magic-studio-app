//! `character.action`: key one action from the shared pose table onto the rig in EffectCraft
//! (`prop.addKey` per pose), then Easy Ease the keys, as an animator blocks and smooths a cycle.

use serde_json::{Value, json};

use super::{POSES, View, bad, effectcraft, point, step};
use crate::{Result, Session};

/// The action's own keys, following `same_as` to the action it reuses.
fn action_keys<'a>(table: &'a Value, name: &str) -> Result<(&'a Value, &'a Value)> {
    let action = table["actions"]
        .get(name)
        .ok_or_else(|| bad(format!("unknown action {name}; see character.actions")))?;
    let keys = match action["same_as"].as_str() {
        Some(base) => &table["actions"][base]["keys"],
        None => &action["keys"],
    };
    Ok((action, keys))
}

/// The rest value each channel is offset from: joint position, no rotation.
fn rest_value(channel: &str, joint: [f64; 2], offset: &Value) -> Value {
    match channel {
        "position" => {
            let d = point(offset).unwrap_or([0.0, 0.0]);
            json!([joint[0] + d[0], joint[1] + d[1]])
        }
        "rotation" => json!(offset.as_f64().unwrap_or(0.0)),
        _ => offset.clone(),
    }
}

fn mirrored(channel: &str, layer: &str, value: Value, mirror: bool) -> Value {
    if !(mirror && layer == "root" && channel == "scale") {
        return value;
    }
    let s = point(&value).unwrap_or([100.0, 100.0]);
    json!([-s[0], s[1]])
}

struct Placement {
    at: f64,
    duration: f64,
    times: u64,
    mirror: bool,
}

fn channel_keys(layer: &str, channels: &Value, joint: [f64; 2], when: &Placement) -> Vec<Value> {
    let mut out = Vec::new();
    for (channel, keys) in channels.as_object().into_iter().flatten() {
        for cycle in 0..when.times {
            for key in keys.as_array().into_iter().flatten() {
                let t = when.at + cycle as f64 * when.duration + key[0].as_f64().unwrap_or(0.0);
                let value = mirrored(
                    channel,
                    layer,
                    rest_value(channel, joint, &key[1]),
                    when.mirror,
                );
                out.push(step(
                    "prop.addKey",
                    json!({"layer": layer, "path": format!("transform/{channel}"),
                        "time": t, "timeBase": "comp", "value": value}),
                ));
            }
        }
    }
    out
}

/// Each layer's rest position as EffectCraft reported it after rigging (parent space).
fn rest_positions(parts: &Value) -> Result<std::collections::HashMap<String, [f64; 2]>> {
    let rest = parts["rest_position"]
        .as_object()
        .ok_or_else(|| bad("parts.json has no rest positions; run character.rig again"))?;
    Ok(rest
        .iter()
        .filter_map(|(id, v)| point(v).map(|p| (id.clone(), p)))
        .collect())
}

pub fn cmd_character_action(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    let table: Value = serde_json::from_str(POSES).map_err(|e| bad(e.to_string()))?;
    let name = p["action"]
        .as_str()
        .ok_or_else(|| bad("action=<name> is required"))?;
    let (action, keys) = action_keys(&table, name)?;
    let joints = rest_positions(&view.parts()?)?;
    let when = Placement {
        at: p["at"].as_f64().unwrap_or(0.0).max(0.0),
        duration: action["duration"].as_f64().unwrap_or(1.0),
        times: p["times"].as_u64().unwrap_or(1).clamp(1, 64),
        mirror: action["mirror"] == true,
    };
    let mut steps = vec![step("comp.open", json!({"comp": view.comp()}))];
    let mut keyed = Vec::new();
    for (layer, channels) in keys.as_object().into_iter().flatten() {
        let Some(joint) = joints.get(layer) else {
            continue;
        };
        steps.extend(channel_keys(layer, channels, *joint, &when));
        keyed.push(layer.clone());
    }
    if when.mirror && !keys["root"]["scale"].is_array() {
        steps.push(step(
            "prop.addKey",
            json!({"layer": "root", "path": "transform/scale",
            "time": when.at, "timeBase": "comp", "value": [-100, 100]}),
        ));
    }
    steps.push(step("keys.selectAll", json!({})));
    steps.push(step("keys.easyEase", json!({})));
    effectcraft(&view.dir, Some(&project), &steps)?;
    let end = when.at + when.duration * when.times as f64;
    Ok(
        json!({"ok": true, "action": name, "layers": keyed, "start": when.at, "end": end,
        "loop": action["loop"], "ecproj": project}),
    )
}

/// The actions the pose table offers, and the views each one is drawn for.
pub fn cmd_character_actions(_: &mut Session, _: &Value) -> Result<Value> {
    let table: Value = serde_json::from_str(POSES).map_err(|e| bad(e.to_string()))?;
    let actions: Vec<Value> = table["actions"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(id, a)| json!({"id": id, "views": a["views"], "duration": a["duration"], "loop": a["loop"]}))
        .collect();
    Ok(json!({"ok": true, "skeleton": table["skeleton"], "actions": actions}))
}
