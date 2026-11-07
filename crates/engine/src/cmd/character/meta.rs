//! What Studio's Animation tab shows for one character, and saving the marks a person adjusts on
//! its canvas. Reads and writes `parts.json` only; the app documents stay the source of truth.

use std::path::Path;

use serde_json::{Value, json};

use super::{POSES, View, bad, part_list, point, root_part};
use crate::{Result, Session};

/// A file's modification time, so Studio reloads images and frames after a change.
fn revision(path: &Path) -> u64 {
    std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64)
}

fn view_summary(view: &View) -> Value {
    let parts = view.parts().unwrap_or(Value::Null);
    let drawing = parts["image"].as_str().map(|i| view.dir.join(i));
    let ecproj = view.file("ecproj");
    json!({
        "view": view.view,
        "drawing": drawing,
        "size": parts["size"],
        "status": parts["status"],
        "parts": parts["parts"],
        "actions": parts["actions"],
        "marked": parts["parts"].is_array(),
        "cut": view.file("psd").is_file(),
        "rigged": ecproj.is_file(),
        "ecproj": ecproj.is_file().then_some(&ecproj),
        "revision": revision(&ecproj).max(revision(&view.dir.join("parts.json"))),
    })
}

/// One character: every drawn view, what step it has reached, and the actions it can play.
pub fn cmd_character_meta(s: &mut Session, p: &Value) -> Result<Value> {
    let mut views = Vec::new();
    for name in ["front", "side", "back"] {
        let mut params = p.clone();
        params["view"] = json!(name);
        let view = View::from_params(s, &params)?;
        if view.dir.is_dir() {
            views.push(view_summary(&view));
        }
    }
    let table: Value = serde_json::from_str(POSES).map_err(|e| bad(e.to_string()))?;
    let actions = super::action::action_summaries(&table);
    let project_dir = crate::workshop::project_dir(s)?;
    let char_id = p["character"].as_str().map(crate::workspace::sanitize_asset_id).unwrap_or_default();
    let tripo_3d = super::tripo::inspect_character_3d(s, &project_dir, &char_id);
    Ok(json!({"ok": true, "character": p["character"], "skeleton": table["skeleton"],
        "views": views, "actions": actions, "tripo_3d": tripo_3d}))
}

fn check_part(part: &Value, known: &[Value]) -> Result<()> {
    let id = part["id"].as_str().ok_or_else(|| bad("every part needs an id"))?;
    if !known.contains(&part["id"]) {
        return Err(bad(format!("{id} is not a part of the shared skeleton")));
    }
    if point(&part["joint"]).is_none() {
        return Err(bad(format!("{id} needs a joint [x, y]")));
    }
    let outline = part["outline"].as_array().map_or(0, Vec::len);
    if outline < 3 || part["outline"].as_array().into_iter().flatten().any(|p| point(p).is_none()) {
        return Err(bad(format!("{id} needs an outline of 3 or more [x, y] points")));
    }
    Ok(())
}

/// Save marks adjusted on the canvas. The cut and rig built from older marks are now stale,
/// so their recorded bounds and rest positions are cleared; run character.cut again.
pub fn cmd_character_save_marks(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let mut parts = view.parts()?;
    let edited = p["parts"].as_array().ok_or_else(|| bad("parts=[{id, parent, joint, outline}] is required"))?;
    let table: Value = serde_json::from_str(POSES).map_err(|e| bad(e.to_string()))?;
    let known = table["parts"].as_array().cloned().unwrap_or_default();
    for part in edited {
        check_part(part, &known)?;
    }
    let cleaned: Vec<Value> = edited
        .iter()
        .map(|part| {
            json!({"id": part["id"], "parent": part["parent"], "joint": part["joint"],
            "outline": part["outline"]})
        })
        .collect();
    parts["parts"] = json!(cleaned);
    root_part(part_list(&parts)?)?;
    parts["status"] = json!("reviewed; run character.cut");
    if let Some(map) = parts.as_object_mut() {
        map.remove("rest_position");
        map.remove("actions");
    }
    view.write_parts(&parts)?;
    Ok(json!({"ok": true, "parts": view.dir.join("parts.json"), "count": cleaned.len(),
        "next": format!("character.cut character={} view={}", view.id, view.view)}))
}
