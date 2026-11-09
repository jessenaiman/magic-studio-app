//! A character's motion path, stored in its rig as Position keys on the root layer.
use super::keys::read;
use super::{View, bad, effectcraft, step};
use crate::{Result, Session};
use serde_json::{Value, json};
use std::path::Path;

const POSITION: &str = "transform/position";
const SCALE: &str = "transform/scale";

/// The rig's one parentless layer, which everything else follows.
fn root_layer(view: &View, project: &Path, comp: &str) -> Result<Value> {
    let info = read(view, project, comp, vec![step("comp.info", json!({"comp": comp}))])?.remove(0);
    let listed = info["layers"].as_array().cloned().unwrap_or_default();
    let trees = read(view, project, comp, listed.iter().map(|l| step("layer.tree", json!({"layer": l["id"], "comp": comp, "depth": 1}))).collect())?;
    let mut roots = listed.iter().zip(&trees).filter(|(_, tree)| tree["parent"].is_null()).map(|(layer, _)| layer["id"].clone());
    match (roots.next(), roots.next()) {
        (Some(root), None) => Ok(root),
        _ => Err(bad("The rig needs exactly one root layer to carry a motion path")),
    }
}

fn number(p: &Value, key: &str, min: f64, max: f64) -> Result<f64> {
    p[key].as_f64().filter(|v| v.is_finite() && *v >= min && *v <= max).ok_or_else(|| bad(format!("{key} must be between {min} and {max}")))
}

fn point(value: &Value) -> Option<[f64; 2]> {
    Some([value.get(0)?.as_f64()?, value.get(1)?.as_f64()?])
}

/// The drawn points of a per-frame path: the first, the last and every change of direction.
fn corners(points: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    if points.len() < 3 {
        return points;
    }
    let bends = points.windows(3).filter(|w| {
        let (ax, ay) = (w[1][0] - w[0][0], w[1][1] - w[0][1]);
        let (bx, by) = (w[2][0] - w[1][0], w[2][1] - w[1][1]);
        (ax * by - ay * bx).abs() > 1e-3
    });
    let mut kept = vec![points[0]];
    kept.extend(bends.map(|w| w[1]));
    kept.extend(points.last());
    kept
}

/// `character.path`: the rig's root Position keys as path points; empty when there is no rig.
pub fn cmd_character_path(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let project = view.file("ecproj");
    if !project.is_file() {
        return Ok(json!({"ok":true,"saved":false,"points":[]}));
    }
    let comp = view.comp();
    let root = root_layer(&view, &project, &comp)?;
    let props = read(
        &view,
        &project,
        &comp,
        vec![step("prop.get", json!({"layer": root, "path": POSITION, "comp": comp})), step("prop.get", json!({"layer": root, "path": SCALE, "comp": comp}))],
    )?;
    let keys = props[0]["keys"].as_array().cloned().unwrap_or_default();
    let points = corners(keys.iter().filter_map(|key| point(&key["value"])).collect());
    if points.is_empty() {
        return Ok(json!({"ok":true,"saved":false,"points":[]}));
    }
    let scale_at = |index: usize| props[1]["keys"].as_array().and_then(|k| k.get(index)).and_then(|key| key["value"][0].as_f64());
    let scale_keys = props[1]["keys"].as_array().map_or(0, Vec::len);
    Ok(json!({
        "ok": true, "saved": true, "points": points, "duration": keys.last().map(|key| &key["compTime"]),
        "size_start": scale_at(0), "size_end": scale_keys.checked_sub(1).and_then(scale_at),
    }))
}

/// `character.save_path`: record the drawn points as motion on the rig's root, starting where
/// the root already is, and key its size when the path changes it.
pub fn cmd_character_save_path(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let project = view.require("ecproj", "character.rig")?;
    let points = p["points"].as_array().filter(|a| a.len() <= 1000).ok_or_else(|| bad("points must be an array of at most 1000 [x, y] pairs"))?;
    let points: Vec<[f64; 2]> = points
        .iter()
        .map(point)
        .collect::<Option<_>>()
        .filter(|v: &Vec<[f64; 2]>| v.iter().flatten().all(|n| n.is_finite()))
        .ok_or_else(|| bad("Each point must be [x, y]"))?;
    if points.len() < 2 {
        return Err(bad("Draw at least two points"));
    }
    let duration = number(p, "duration", 0.1, 3600.)?;
    let size_start = number(p, "size_start", 0., 1000.)?;
    let size_end = number(p, "size_end", 0., 1000.)?;
    let comp = view.comp();
    let root = root_layer(&view, &project, &comp)?;
    let rest = read(&view, &project, &comp, vec![step("prop.get", json!({"layer": root, "path": POSITION, "comp": comp, "time": 0}))])?.remove(0);
    let rest = point(&rest["value"]).ok_or_else(|| bad("The root layer has no position"))?;
    let (dx, dy) = (rest[0] - points[0][0], rest[1] - points[0][1]);
    let last = (points.len() - 1) as f64;
    let timed: Vec<Value> = points.iter().enumerate().map(|(i, pt)| json!([i as f64 / last * duration, pt[0] + dx, pt[1] + dy])).collect();
    let mut steps = vec![
        step("comp.open", json!({"comp": comp})),
        step("prop.toggleAnimation", json!({"layer": root, "path": POSITION, "value": false})),
        step("prop.toggleAnimation", json!({"layer": root, "path": SCALE, "value": false})),
        step("motion.sketch", json!({"layer": root, "points": timed, "start": 0, "captureSpeed": 100, "smoothing": 0})),
    ];
    for (time, size) in [(0.0, size_start), (duration, size_end)] {
        if size_start != 100.0 || size_end != 100.0 {
            steps.push(step("prop.addKey", json!({"layer": root, "path": SCALE, "time": time, "timeBase": "comp", "value": [size, size, 100]})));
        }
    }
    effectcraft(&view.dir, Some(&project), &steps, true)?;
    Ok(json!({"ok":true,"ecproj":project,"comp":comp,"root":root,"points":timed.len(),"duration":duration}))
}
