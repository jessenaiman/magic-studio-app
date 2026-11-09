//! EffectCraft-native character commands: puppet pins, expressions, keyframes,
//! time remapping, masks, 3D layers — each one routes a single EffectCraft
//! engine command ID through `effectcraft`, exactly as `effect.exec` does.
//! The full command list is discoverable with `effectcraft-cli commands --json`.

use serde_json::{Value, json};

use super::{View, effectcraft, step};
use crate::{EngineError, Result, Session};

fn bad(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

/// Open the character's comp and run one EffectCraft command step on it.
fn effect_one(view: &View, project: &std::path::Path, id: &str, params: Value) -> Result<Value> {
    let steps = vec![step("comp.open", json!({"comp": view.comp()})), step(id, params)];
    let out = effectcraft(&view.dir, Some(project), &steps, true)?;
    Ok(out.into_iter().last().unwrap_or(Value::Null))
}

/// `character.puppet.addPin` — add a Position/Advanced/Bend/Starch/Overlap pin to a layer mesh.
pub fn cmd_puppet_add_pin(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    let layer = p["layer"].as_str().ok_or_else(|| bad("layer=<layer name or id> is required"))?;
    let kind = p["kind"].as_str().unwrap_or("position");
    if !matches!(kind, "position" | "advanced" | "bend" | "starch" | "overlap") {
        return Err(bad("kind must be position, advanced, bend, starch or overlap"));
    }
    let position = p["position"].as_array().filter(|a| a.len() == 2).ok_or_else(|| bad("position=[x, y] in layer pixels is required"))?;
    let x = position[0].as_f64().ok_or_else(|| bad("position[0] must be a number"))?;
    let y = position[1].as_f64().ok_or_else(|| bad("position[1] must be a number"))?;
    let name = p["name"].as_str();
    let mut params = json!({"layer": layer, "kind": kind, "position": [x, y]});
    if let Some(n) = name {
        params["name"] = json!(n);
    }
    let result = effect_one(&view, &project, "puppet.addPin", params)?;
    Ok(json!({"ok": true, "layer": layer, "kind": kind, "position": [x, y], "result": result}))
}

/// `character.puppet.movePin` — move an existing puppet pin.
pub fn cmd_puppet_move_pin(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    let layer = p["layer"].as_str().ok_or_else(|| bad("layer=<layer name or id> is required"))?;
    let pin = p["pin"].as_str().ok_or_else(|| bad("pin=<pin name or index> is required"))?;
    let position = p["position"].as_array().filter(|a| a.len() == 2).ok_or_else(|| bad("position=[x, y] in layer pixels is required"))?;
    let x = position[0].as_f64().ok_or_else(|| bad("position[0] must be a number"))?;
    let y = position[1].as_f64().ok_or_else(|| bad("position[1] must be a number"))?;
    let result = effect_one(&view, &project, "puppet.movePin", json!({"layer": layer, "pin": pin, "position": [x, y]}))?;
    Ok(json!({"ok": true, "layer": layer, "pin": pin, "position": [x, y], "result": result}))
}

/// `character.puppet.removePin` — delete a puppet pin from a layer.
pub fn cmd_puppet_remove_pin(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    let layer = p["layer"].as_str().ok_or_else(|| bad("layer=<layer name or id> is required"))?;
    let pin = p["pin"].as_str().ok_or_else(|| bad("pin=<pin name or index> is required"))?;
    let result = effect_one(&view, &project, "puppet.removePin", json!({"layer": layer, "pin": pin}))?;
    Ok(json!({"ok": true, "layer": layer, "pin": pin, "result": result}))
}

/// `character.expression.set` — add a JavaScript expression to a layer property.
pub fn cmd_expression_set(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    let layer = p["layer"].as_str().ok_or_else(|| bad("layer=<layer name or id> is required"))?;
    let path = p["path"].as_str().ok_or_else(|| bad("path=<property path, e.g. transform/position> is required"))?;
    let expression = p["expression"].as_str().ok_or_else(|| bad("expression=<JavaScript expression string> is required"))?;
    let result = effect_one(&view, &project, "prop.setExpression", json!({"layer": layer, "path": path, "expression": expression}))?;
    Ok(json!({"ok": true, "layer": layer, "path": path, "expression": expression, "result": result}))
}

/// `character.keyframe.easyEase` — apply Easy Ease to selected/all keyframes on a layer property.
pub fn cmd_keyframe_easy_ease(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    let layer = p["layer"].as_str().ok_or_else(|| bad("layer=<layer name or id> is required"))?;
    let prop = p["prop"].as_str().unwrap_or("rotation");
    if !matches!(prop, "position" | "rotation" | "scale" | "anchor" | "opacity") {
        return Err(bad("prop must be position, rotation, scale, anchor or opacity"));
    }
    let result = effect_one(&view, &project, "keys.easyEase", json!({"layer": layer, "path": format!("transform/{prop}")}))?;
    Ok(json!({"ok": true, "layer": layer, "prop": prop, "result": result}))
}

/// `character.keyframe.velocity` — set keyframe influence and speed for a layer property.
pub fn cmd_keyframe_velocity(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    let layer = p["layer"].as_str().ok_or_else(|| bad("layer=<layer name or id> is required"))?;
    let prop = p["prop"].as_str().unwrap_or("position");
    if !matches!(prop, "position" | "rotation" | "scale" | "anchor" | "opacity") {
        return Err(bad("prop must be position, rotation, scale, anchor or opacity"));
    }
    let influence_in = p["influenceIn"].as_f64().unwrap_or(33.33);
    let influence_out = p["influenceOut"].as_f64().unwrap_or(33.33);
    let speed_in = p["speedIn"].as_f64().unwrap_or(0.0);
    let speed_out = p["speedOut"].as_f64().unwrap_or(0.0);
    let result = effect_one(
        &view,
        &project,
        "keys.velocity",
        json!({"layer": layer, "path": format!("transform/{prop}"),
            "influenceIn": influence_in, "influenceOut": influence_out,
            "speedIn": speed_in, "speedOut": speed_out}),
    )?;
    Ok(json!({"ok": true, "layer": layer, "prop": prop, "result": result}))
}

/// `character.time.remap` — enable time remapping on a layer and set the remap curve.
pub fn cmd_time_remap(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    let layer = p["layer"].as_str().ok_or_else(|| bad("layer=<layer name or id> is required"))?;
    let result = effect_one(&view, &project, "layer.timing", json!({"layer": layer, "enableTimeRemap": true}))?;
    Ok(json!({"ok": true, "layer": layer, "timeRemap": true, "result": result}))
}

/// `character.mask.add` — add a mask to a layer from an array of points.
pub fn cmd_mask_add(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    let layer = p["layer"].as_str().ok_or_else(|| bad("layer=<layer name or id> is required"))?;
    let points = p["points"].as_array().ok_or_else(|| bad("points=[[x, y], …] in layer pixels is required"))?;
    let mut pts = Vec::new();
    for pt in points {
        let arr = pt.as_array().filter(|a| a.len() == 2).ok_or_else(|| bad("each point must be [x, y]"))?;
        pts.push(json!([arr[0].as_f64().unwrap_or(0.0), arr[1].as_f64().unwrap_or(0.0)]));
    }
    let closed = p["closed"].as_bool().unwrap_or(true);
    let feather = p["feather"].as_f64().unwrap_or(0.0);
    let result = effect_one(&view, &project, "mask.new", json!({"layer": layer, "points": pts, "closed": closed, "feather": feather}))?;
    Ok(json!({"ok": true, "layer": layer, "points": pts, "closed": closed, "feather": feather, "result": result}))
}

/// `character.layer.new3dModel` — add a 3D model layer (.glb/.gltf/.fbx) to the comp.
pub fn cmd_layer_new_3d_model(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    let path = p["path"].as_str().ok_or_else(|| bad("path=<model file path> is required"))?;
    let name = p["name"].as_str().unwrap_or("3D Model");
    let result = effect_one(&view, &project, "layer.newModel", json!({"path": path, "name": name}))?;
    Ok(json!({"ok": true, "path": path, "name": name, "result": result}))
}

/// `character.layer.setBlendMode` — set a layer's blend mode (38 available).
pub fn cmd_layer_set_blend_mode(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    let layer = p["layer"].as_str().ok_or_else(|| bad("layer=<layer name or id> is required"))?;
    let mode = p["mode"].as_str().ok_or_else(|| bad("mode=<blend mode name> is required"))?;
    let result = effect_one(&view, &project, "layer.setBlendMode", json!({"layer": layer, "mode": mode}))?;
    Ok(json!({"ok": true, "layer": layer, "mode": mode, "result": result}))
}

/// `character.layer.setParent` — parent one layer to another with compensation.
pub fn cmd_layer_set_parent(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    let layer = p["layer"].as_str().ok_or_else(|| bad("layer=<layer name or id> is required"))?;
    let parent = p["parent"].as_str().ok_or_else(|| bad("parent=<parent layer name or id> is required"))?;
    let compensate = p["compensate"].as_bool().unwrap_or(true);
    let result = effect_one(&view, &project, "layer.setParent", json!({"layers": [layer], "parent": parent, "compensate": compensate}))?;
    Ok(json!({"ok": true, "layer": layer, "parent": parent, "compensate": compensate, "result": result}))
}
