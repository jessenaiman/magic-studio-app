//! `character.action`: key one action from the shared pose table onto the rig in EffectCraft
//! (`prop.addKey` per pose), then Easy Ease the keys, as an animator blocks and smooths a cycle.

use std::collections::HashMap;

use serde_json::{Value, json};

use super::{POSES, View, bad, effectcraft, point, step};
use crate::{Result, Session};

/// The action's own keys, following `same_as` to the action it reuses.
fn action_keys<'a>(table: &'a Value, name: &str) -> Result<(&'a Value, &'a Value)> {
    let action = table["actions"].get(name).ok_or_else(|| bad(format!("unknown action {name}; see character.actions")))?;
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
                let value = mirrored(channel, layer, rest_value(channel, joint, &key[1]), when.mirror);
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
fn rest_positions(parts: &Value) -> Result<HashMap<String, [f64; 2]>> {
    let rest = parts["rest_position"].as_object().ok_or_else(|| bad("parts.json has no rest positions; run character.rig again"))?;
    Ok(rest.iter().filter_map(|(id, v)| point(v).map(|p| (id.clone(), p))).collect())
}

/// Gap between actions in the library, so each one starts from the rest pose.
const GAP: f64 = 0.5;
const CHANNELS: [&str; 3] = ["position", "rotation", "scale"];

/// Rest-pose keys for every layer at `t`, so no action leaks into the next one.
fn rest_keys(rest: &HashMap<String, [f64; 2]>, t: f64, mirror: bool) -> Vec<Value> {
    let mut out = Vec::new();
    for (layer, position) in rest {
        for channel in CHANNELS {
            let value = match channel {
                "position" => json!(position),
                "rotation" => json!(0),
                _ => mirrored(channel, layer, json!([100, 100]), mirror),
            };
            out.push(step(
                "prop.addKey",
                json!({"layer": layer, "path": format!("transform/{channel}"),
                "time": t, "timeBase": "comp", "value": value}),
            ));
        }
    }
    out
}

/// Where the action lives in the library: its saved range, or the next free slot.
fn library_slot(parts: &Value, name: &str, p: &Value) -> (Option<Value>, f64) {
    let library = parts["actions"].as_object();
    let saved = library.and_then(|l| l.get(name)).cloned();
    let next = library.into_iter().flatten().filter_map(|(_, r)| r["end"].as_f64()).fold(0.0_f64, |a, b| a.max(b + GAP));
    (saved, p["at"].as_f64().unwrap_or(next).max(0.0))
}

/// Evaluate an authored `parts/rig.json` track (with `oscillate` and `keys` entries) into
/// EffectCraft `prop.addKey` steps on `comp`.
fn rig_track_keys(
    view: &View,
    name: &str,
    rest: &HashMap<String, [f64; 2]>,
    when: &Placement,
) -> Option<(f64, bool, Vec<Value>, Vec<String>)> {
    let rig_path = view.dir.parent()?.join("parts/rig.json");
    let text = std::fs::read_to_string(rig_path).ok()?;
    let rig: Value = serde_json::from_str(&text).ok()?;
    let tracks = rig["tracks"].as_array()?;
    let track = tracks
        .iter()
        .find(|t| t["id"].as_str() == Some(name) || (name == "swim" && t["id"].as_str() == Some("swim-cruise")))
        .or_else(|| (name == "swim" || name == "swim-cruise").then(|| tracks.first()).flatten())?;
    let duration = track["duration"].as_f64().unwrap_or(when.duration).max(0.5);
    let is_loop = track["loop"].as_bool().unwrap_or(true);
    let motion = track["motion"].as_object()?;
    let sample_fps = 12_u64;
    let total_dur = duration * when.times as f64;
    let steps_count = ((total_dur * sample_fps as f64).round() as u64).max(2);
    let mut out = Vec::new();
    let mut keyed = Vec::new();
    for (layer, props) in motion {
        let Some(&base_pos) = rest.get(layer) else {
            continue;
        };
        let Some(props_obj) = props.as_object() else {
            continue;
        };
        keyed.push(layer.clone());
        for (channel, specs) in props_obj {
            if !matches!(channel.as_str(), "position" | "rotation" | "scale") {
                continue;
            }
            let Some(spec_arr) = specs.as_array() else {
                continue;
            };
            for step_i in 0..=steps_count {
                let local_t = (step_i as f64 / steps_count as f64) * total_dur;
                let comp_t = when.at + local_t;
                let mut dx = 0.0;
                let mut dy = 0.0;
                for spec in spec_arr {
                    match spec["kind"].as_str() {
                        Some("oscillate") => {
                            let amp = spec["amplitude"].as_f64().unwrap_or(0.0);
                            let hz = spec["hz"].as_f64().unwrap_or(1.0);
                            let phase = spec["phase"].as_f64().unwrap_or(0.0);
                            let axis = spec["axis"].as_u64().unwrap_or(0);
                            let val = amp * (std::f64::consts::TAU * local_t * hz + phase).sin();
                            if axis == 1 {
                                dy += val;
                            } else {
                                dx += val;
                                if channel == "scale" && spec.get("axis").is_none() {
                                    dy += val;
                                }
                            }
                        }
                        Some("keys") => {
                            if let Some(karr) = spec["keys"].as_array() {
                                let cyc_t = local_t % duration;
                                let mut prev = None;
                                let mut next = None;
                                for k in karr {
                                    if let Some(kt) = k[0].as_f64() {
                                        if kt <= cyc_t {
                                            prev = Some((kt, &k[1]));
                                        }
                                        if kt >= cyc_t && next.is_none() {
                                            next = Some((kt, &k[1]));
                                        }
                                    }
                                }
                                if let (Some((t0, v0)), Some((t1, v1))) = (prev, next) {
                                    let alpha = if (t1 - t0).abs() > 1e-6 { ((cyc_t - t0) / (t1 - t0)).clamp(0.0, 1.0) } else { 0.0 };
                                    if let (Some(p0), Some(p1)) = (point(v0), point(v1)) {
                                        dx += p0[0] + (p1[0] - p0[0]) * alpha;
                                        dy += p0[1] + (p1[1] - p0[1]) * alpha;
                                    } else if let (Some(f0), Some(f1)) = (v0.as_f64(), v1.as_f64()) {
                                        dx += f0 + (f1 - f0) * alpha;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                let value = match channel.as_str() {
                    "position" => json!([base_pos[0] + dx, base_pos[1] + dy]),
                    "rotation" => json!(dx),
                    "scale" => mirrored("scale", layer, json!([100.0 + dx, 100.0 + dy]), when.mirror),
                    _ => continue,
                };
                out.push(step(
                    "prop.addKey",
                    json!({"layer": layer, "path": format!("transform/{channel}"), "time": comp_t, "timeBase": "comp", "value": value}),
                ));
            }
        }
    }
    (!out.is_empty()).then_some((duration, is_loop, out, keyed))
}

pub fn cmd_character_action(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    let table: Value = serde_json::from_str(POSES).map_err(|e| bad(e.to_string()))?;
    let name = p["action"].as_str().ok_or_else(|| bad("action=<name> is required"))?;
    let pose_entry = action_keys(&table, name).ok();
    let mut parts = view.parts()?;
    let (saved, at) = library_slot(&parts, name, p);
    let mut action_loop = pose_entry.map(|(a, _)| a["loop"].clone()).unwrap_or(json!(true));
    let (start, end, keyed_layers, was_keyed) = if let Some(range) = saved.filter(|_| p["rekey"] != true) {
        (range["start"].as_f64().unwrap_or(0.0), range["end"].as_f64().unwrap_or(1.0), Vec::new(), false)
    } else {
        let rest = rest_positions(&parts)?;
        let mut when = Placement {
            at,
            duration: pose_entry.and_then(|(a, _)| a["duration"].as_f64()).unwrap_or(1.5),
            times: p["times"].as_u64().unwrap_or(1).clamp(1, 64),
            mirror: pose_entry.is_some_and(|(a, _)| a["mirror"] == true),
        };
        let mut steps = vec![step("comp.open", json!({"comp": view.comp()}))];
        let (end, keyed) = if let Some((track_dur, is_loop, track_steps, track_layers)) = rig_track_keys(&view, name, &rest, &when) {
            when.duration = p["duration"].as_f64().unwrap_or(track_dur);
            action_loop = json!(is_loop);
            let end = when.at + when.duration * when.times as f64;
            steps.extend(rest_keys(&rest, when.at, when.mirror));
            steps.extend(track_steps);
            steps.extend(rest_keys(&rest, end, when.mirror));
            (end, track_layers)
        } else {
            let (_, keys) = pose_entry.ok_or_else(|| bad(format!("unknown action {name}; see character.actions")))?;
            let end = when.at + when.duration * when.times as f64;
            steps.extend(rest_keys(&rest, when.at, when.mirror));
            steps.extend(rest_keys(&rest, end, when.mirror));
            let mut keyed = Vec::new();
            for (layer, channels) in keys.as_object().into_iter().flatten() {
                let Some(position) = rest.get(layer) else {
                    continue;
                };
                steps.extend(channel_keys(layer, channels, *position, &when));
                keyed.push(layer.clone());
            }
            (end, keyed)
        };
        steps.push(step("comp.settings", json!({"duration": end.max(10.0) + GAP})));
        steps.push(step("keys.selectAll", json!({})));
        steps.push(step("keys.easyEase", json!({})));
        effectcraft(&view.dir, Some(&project), &steps, true)?;
        parts["actions"][name] = json!({"start": when.at, "end": end, "times": when.times});
        view.write_parts(&parts)?;
        (when.at, end, keyed, true)
    };
    let mut reply = json!({
        "ok": true, "action": name, "layers": keyed_layers, "start": start, "end": end,
        "loop": action_loop, "ecproj": project, "keyed": was_keyed
    });
    if p["render"] == true {
        let fps = p["fps"].as_u64().unwrap_or(24);
        let rendered = super::cmd_character_render(s, &json!({"character": view.id, "view": view.view, "start": start, "end": end, "fps": fps}))?;
        reply["frames"] = rendered["frames"].clone();
        reply["fps"] = json!(fps);
    }
    Ok(reply)
}

/// Each action of the pose table as the panels list it: the drawn views it plays in, its
/// cycle, and the clips a rigged 3D model may play for it (`model`).
pub(crate) fn action_summaries(table: &Value) -> Vec<Value> {
    table["actions"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(id, a)| {
            json!({"id": id, "views": a["views"], "duration": a["duration"], "loop": a["loop"],
            "model": a["model"]})
        })
        .collect()
}

/// The actions the pose table offers, and the views each one is drawn for.
pub fn cmd_character_actions(_: &mut Session, _: &Value) -> Result<Value> {
    let table: Value = serde_json::from_str(POSES).map_err(|e| bad(e.to_string()))?;
    Ok(json!({"ok": true, "skeleton": table["skeleton"], "actions": action_summaries(&table)}))
}
