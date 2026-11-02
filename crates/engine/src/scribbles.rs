//! `scribbles.*`: drawing requests to Scribbles (Codex image generation), one per kind of asset an
//! animator needs. Each kind is a template in `scribbles/templates.json`: a reference link to how
//! the step is done professionally, the canvas, the panel layout and the rules every drawing keeps.
//! The drawing itself is `image.generate`; this module only writes the structured prompt.

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

const TEMPLATES: &str = include_str!("scribbles/templates.json");

fn bad(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

fn templates() -> Result<Value> {
    serde_json::from_str(TEMPLATES).map_err(|e| bad(format!("scribbles templates: {e}")))
}

/// `{frames}`, `{action}`, `{view}`, `{keys}` and `{prompt}` filled from the request.
fn layout(kind: &Value, p: &Value) -> Result<String> {
    let mut text = kind["layout"].as_str().unwrap_or_default().to_string();
    if let Some(actions) = kind["actions"].as_object() {
        let name = p["action"].as_str().unwrap_or("walk");
        let action = actions.get(name).ok_or_else(|| {
            bad(format!(
                "action must be one of {}",
                actions.keys().cloned().collect::<Vec<_>>().join(", ")
            ))
        })?;
        text = text
            .replace("{frames}", "8")
            .replace("{action}", &format!("a {name} cycle"))
            .replace("{view}", action["view"].as_str().unwrap_or_default())
            .replace("{keys}", action["keys"].as_str().unwrap_or_default());
    }
    if text.contains("{prompt}") {
        let prompt = p["prompt"].as_str().filter(|t| !t.trim().is_empty());
        text = text.replace("{prompt}", prompt.ok_or_else(|| bad("prompt is required"))?);
    }
    Ok(text)
}

/// The full prompt: canvas, layout, the template's rules, the character and the shared rules.
fn compose(all: &Value, kind: &Value, p: &Value) -> Result<String> {
    let [w, h] = [
        kind["canvas"][0].as_u64().unwrap_or(1024),
        kind["canvas"][1].as_u64().unwrap_or(1536),
    ];
    let mut parts = vec![
        format!("Draw ONE image on a {w} x {h} px canvas."),
        layout(kind, p)?,
    ];
    for rule in kind["rules"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        parts.push(all[rule].as_str().unwrap_or_default().to_string());
    }
    if kind["layout"].as_str() != Some("{prompt}")
        && let Some(prompt) = p["prompt"].as_str().filter(|t| !t.trim().is_empty())
    {
        parts.push(format!("Character: {prompt}"));
    }
    if let Some(notes) = p["notes"].as_str().filter(|t| !t.trim().is_empty()) {
        parts.push(format!("Notes: {notes}"));
    }
    parts.push(all["shared"].as_str().unwrap_or_default().to_string());
    Ok(parts.join("\n"))
}

fn draw(s: &mut Session, p: &Value, name: &str) -> Result<Value> {
    let all = templates()?;
    let kind = all["kinds"]
        .get(name)
        .ok_or_else(|| bad(format!("unknown scribbles kind {name}")))?;
    let references: Vec<Value> = match p.get("reference") {
        Some(Value::String(one)) => vec![json!(one)],
        Some(Value::Array(many)) => many.clone(),
        _ => Vec::new(),
    };
    if kind["needs_reference"] == true && references.is_empty() {
        return Err(bad(format!(
            "scribbles {name} needs reference=<image of the character>"
        )));
    }
    if name == "new" && p["prompt"].as_str().is_none_or(|t| t.trim().is_empty()) {
        return Err(bad("scribbles new needs prompt=<the character to draw>"));
    }
    let prompt = compose(&all, kind, p)?;
    let model = crate::project_settings::stored_settings()
        .pointer("/scribbles/model")
        .and_then(Value::as_str)
        .or_else(|| all["model"].as_str())
        .map(str::to_string);
    let mut request = json!({"prompt": prompt, "transparent_background": true,
        "timeout_s": p["timeout_s"].as_u64().unwrap_or(900),
        "name": p["name"].as_str().unwrap_or(&format!("scribbles-{name}"))});
    if !references.is_empty() {
        request["references"] = json!(references);
    }
    for key in ["model", "character", "project"] {
        if let Some(value) = p
            .get(key)
            .cloned()
            .or_else(|| (key == "model").then(|| json!(model)))
        {
            request[key] = value;
        }
    }
    let mut result = crate::image_generation::generate(s, &request)?;
    let canvas = kind["canvas"].clone();
    let matches = result["width"] == canvas[0] && result["height"] == canvas[1];
    result["scribbles"] = json!({"kind": name, "label": kind["label"], "reference": kind["reference"],
        "canvas": canvas, "canvas_matches": matches, "prompt": prompt});
    Ok(result)
}

macro_rules! kind_command {
    ($fn:ident, $kind:literal) => {
        pub fn $fn(s: &mut Session, p: &Value) -> Result<Value> {
            draw(s, p, $kind)
        }
    };
}

kind_command!(cmd_scribbles_new, "new");
kind_command!(cmd_scribbles_image, "image");
kind_command!(cmd_scribbles_sheet, "sheet");
kind_command!(cmd_scribbles_poses, "poses");
kind_command!(cmd_scribbles_actions, "actions");
kind_command!(cmd_scribbles_custom, "custom");

/// The templates, so an agent or the UI can show the reference and layout before drawing.
pub fn cmd_scribbles_templates(_: &mut Session, _: &Value) -> Result<Value> {
    let all = templates()?;
    Ok(json!({"ok": true, "model": all["model"], "kinds": all["kinds"]}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walk_frames_prompt_names_every_key_pose_and_the_canvas() {
        let all = templates().unwrap();
        let prompt = compose(&all, &all["kinds"]["actions"], &json!({"action": "walk"})).unwrap();
        assert!(prompt.starts_with("Draw ONE image on a 1536 x 1024 px canvas."));
        assert!(prompt.contains("contact (right foot forward), down, passing, up"));
        assert!(prompt.contains("384 x 512 px"));
    }

    #[test]
    fn new_character_keeps_the_legs_apart_rule() {
        let all = templates().unwrap();
        let prompt = compose(
            &all,
            &all["kinds"]["new"],
            &json!({"prompt": "a felt bunny"}),
        )
        .unwrap();
        assert!(prompt.contains("clear gap of background between the legs"));
        assert!(prompt.contains("Character: a felt bunny"));
    }
}
