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
    let drawing = parts["image"]
        .as_str()
        .or_else(|| parts["source"].as_str())
        .map(|i| view.dir.join(i))
        .filter(|p| p.is_file())
        .or_else(|| super::find_initial_cutout(&view.dir));
    let ecproj = view.file("ecproj");
    let layers_dir = view.dir.join("layers");
    let enriched_parts = parts["parts"].as_array().map(|arr| {
        arr.iter()
            .map(|p| {
                let mut item = p.clone();
                if item["joint"].is_null() && item["pivot"].is_array() {
                    item["joint"] = item["pivot"].clone();
                }
                let explicit_file = item["file"].as_str().map(|f| view.dir.join(f)).filter(|f| f.is_file());
                if let Some(ef) = explicit_file {
                    item["file"] = json!(ef);
                } else if let Some(id) = item["id"].as_str() {
                    let layer_png = layers_dir.join(format!("{id}.png"));
                    let direct_png = view.dir.join(format!("{id}.png"));
                    if layer_png.is_file() {
                        item["file"] = json!(layer_png);
                    } else if direct_png.is_file() {
                        item["file"] = json!(direct_png);
                    } else if let Some(ref d) = drawing {
                        item["file"] = json!(d);
                    }
                }
                item
            })
            .collect::<Vec<_>>()
    });
    let size = if parts["size"].is_array() {
        parts["size"].clone()
    } else if parts["canvas"].is_array() {
        parts["canvas"].clone()
    } else if let Some(ref d) = drawing {
        super::cut::photocraft_dimensions(d).ok().or_else(|| image::image_dimensions(d).ok()).map(|(w, h)| json!([w, h])).unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    json!({
        "view": view.view,
        "animation_style": parts["animation_style"].as_str().unwrap_or(if ecproj.is_file() || view.file("psd").is_file() || parts["parts"].is_array() { "cutout_2_5d" } else { "cutout" }),
        "drawing": drawing,
        "size": size,
        "status": parts["status"],
        "parts": enriched_parts.map(Value::Array).unwrap_or_else(|| parts["parts"].clone()),
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
        if view.dir.is_dir() && (!view.is_flat || name == "front") {
            views.push(view_summary(&view));
        }
    }
    let project_dir = crate::workshop::project_dir(s)?;
    let raw_id = p["character"].as_str().map(crate::workspace::sanitize_asset_id).unwrap_or_default();
    let char_folder = super::character_folder(&project_dir, &raw_id);
    let char_id = char_folder.file_name().map(|n| crate::workspace::sanitize_asset_id(&n.to_string_lossy())).filter(|s| !s.is_empty()).unwrap_or(raw_id);
    if views.is_empty()
        && let Some(cutout) = super::find_initial_cutout(&char_folder)
    {
        let size = super::cut::photocraft_dimensions(&cutout)
            .ok()
            .or_else(|| image::image_dimensions(&cutout).ok())
            .map(|(w, h)| json!([w, h]))
            .unwrap_or(Value::Null);
        views.push(json!({
            "view": "front",
            "animation_style": "cutout",
            "drawing": cutout,
            "size": size,
            "status": "unmarked cutout; click Mark parts… to mark with Scribbles",
            "parts": Value::Null,
            "actions": Value::Null,
            "marked": false,
            "cut": false,
            "rigged": false,
            "ecproj": Value::Null,
            "revision": revision(&cutout),
        }));
    }
    let table: Value = serde_json::from_str(POSES).map_err(|e| bad(e.to_string()))?;
    let actions = super::action::action_summaries(&table);
    let tripo_3d = super::tripo::inspect_character_3d(s, &project_dir, &char_id);
    let sprites = super::sprite_summaries(&char_folder);
    let (animation_style, animation_styles) = super::classify_animation_style(&views, &sprites, &tripo_3d);
    Ok(json!({"ok": true, "character": char_id, "animation_style": animation_style, "animation_styles": animation_styles,
        "skeleton": table["skeleton"], "views": views, "actions": actions, "sprites": sprites, "tripo_3d": tripo_3d}))
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
