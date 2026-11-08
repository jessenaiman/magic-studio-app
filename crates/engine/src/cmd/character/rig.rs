//! `character.rig`: EffectCraft builds the puppet from the cut PSD. Each part keeps its place
//! (Import as Composition, layer sizes), its anchor goes on its joint, it is parented to the part
//! it hangs from, and a `root` null carries the whole body.

use serde_json::{Value, json};

use super::{View, bad, effectcraft, part_list, point, root_part, step};
use crate::{Result, Session};

/// Anchor (layer px) and position (comp px) that pin a part's joint without moving its pixels.
fn joint_steps(part: &Value) -> Result<Vec<Value>> {
    let id = part["id"].as_str().ok_or_else(|| bad("a part has no id"))?;
    let joint = point(&part["joint"]).ok_or_else(|| bad(format!("part {id} has no joint")))?;
    let bounds = part["bounds"].as_array().filter(|b| b.len() == 4).ok_or_else(|| bad(format!("part {id} has no layer bounds; run character.cut")))?;
    let origin = [bounds[0].as_f64().unwrap_or(0.0), bounds[1].as_f64().unwrap_or(0.0)];
    let anchor = json!([joint[0] - origin[0], joint[1] - origin[1]]);
    Ok(vec![
        step("layer.setTransform", json!({"layers": [id], "prop": "anchor", "value": anchor})),
        step("layer.setTransform", json!({"layers": [id], "prop": "position", "value": joint})),
    ])
}

/// The rig's frame rate; the pose table's quarter-second poses land on whole frames.
const FPS: u32 = 24;

/// Ask EffectCraft for every layer's rest position. A parented layer's position is in its
/// parent's space, so action keys offset these values, never comp coordinates.
fn record_rest(view: &View, project: &std::path::Path, mut parts: Value) -> Result<()> {
    let mut layers: Vec<String> = part_list(&parts)?.iter().filter_map(|p| p["id"].as_str().map(str::to_string)).collect();
    layers.push("root".into());
    let mut steps = vec![step("comp.open", json!({"comp": view.comp()}))];
    for id in &layers {
        steps.push(step("prop.get", json!({"layer": id, "path": "transform/position", "time": 0})));
    }
    let out = effectcraft(&view.dir, Some(project), &steps, true)?;
    let values: Vec<Value> = out.into_iter().skip(1).collect();
    if values.len() != layers.len() {
        return Err(bad(format!("EffectCraft returned {} rest positions for {} layers", values.len(), layers.len())));
    }
    let mut rest = serde_json::Map::new();
    for (id, v) in layers.iter().zip(values) {
        rest.insert(id.clone(), v.get("value").cloned().unwrap_or(v));
    }
    parts["rest_position"] = Value::Object(rest);
    // A fresh rig has no keys, so its action library starts empty.
    parts["actions"] = json!({});
    view.write_parts(&parts)
}

pub fn cmd_character_rig(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let psd = view.require("psd", "character.cut")?;
    let parts = view.parts()?;
    let list = &part_list(&parts)?.clone();
    let root = root_part(list)?;
    let root_joint = list.iter().find(|p| p["id"].as_str() == Some(root)).and_then(|p| point(&p["joint"])).ok_or_else(|| bad("the root part has no joint"))?;
    let project = view.file("ecproj");
    let mut steps = vec![
        step("file.import", json!({"paths": [psd], "importAs": "compositionLayerSizes"})),
        step("comp.open", json!({"comp": view.comp()})),
        step("comp.settings", json!({"frameRate": FPS})),
    ];
    for part in list {
        steps.extend(joint_steps(part)?);
    }
    steps.push(step("layer.newNull", json!({"name": "root"})));
    steps.push(step("layer.setTransform", json!({"layers": ["root"], "prop": "position", "value": root_joint})));
    for part in list {
        let id = part["id"].as_str().unwrap_or_default();
        let parent = part["parent"].as_str().unwrap_or("root");
        steps.push(step("layer.setParent", json!({"layers": [id], "parent": parent, "compensate": true})));
        if let (Some(pins), Some(bounds)) = (part["pins"].as_array(), part["bounds"].as_array().filter(|b| b.len() == 4)) {
            let ox = bounds[0].as_f64().unwrap_or(0.0);
            let oy = bounds[1].as_f64().unwrap_or(0.0);
            let bw = bounds[2].as_f64().unwrap_or(0.0);
            let bh = bounds[3].as_f64().unwrap_or(0.0);
            let mut pin_names = Vec::new();
            for pin_pt in pins {
                if let Some([px, py]) = point(pin_pt) {
                    let lx = px - ox;
                    let ly = py - oy;
                    if lx >= 0.0 && lx <= bw && ly >= 0.0 && ly <= bh {
                        steps.push(step("puppet.addPin", json!({"layer": id, "kind": "position", "position": [lx, ly]})));
                        pin_names.push(format!("Puppet Pin {}", pin_names.len() + 1));
                    }
                }
            }
            if pin_names.len() >= 2 {
                steps.push(step(
                    "puppet.follow",
                    json!({
                        "layer": id,
                        "leader": pin_names[0],
                        "pins": &pin_names[1..],
                        "delay": 0.08,
                        "amount": 100,
                        "cascade": true
                    }),
                ));
            }
        }
    }
    steps.push(step("file.saveAs", json!({"path": project})));
    effectcraft(&view.dir, None, &steps, true)?;
    record_rest(&view, &project, parts)?;
    Ok(json!({"ok": true, "ecproj": project, "comp": view.comp(), "layers": list.len() + 1,
        "next": format!("character.action character={} view={} action=idle", view.id, view.view)}))
}
