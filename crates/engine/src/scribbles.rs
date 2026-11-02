//! Scribbles executes final drawing requests through the configured image backend.
//! Legacy template commands delegate prompt preparation to Fidget.

use crate::{EngineError, Result, Session};
use serde_json::{Value, json};

/// Generate a final prompt without adding template text. Accept the flat request
/// returned by `fidget.prepare`, including its optional provenance.
pub fn cmd_scribbles_generate(s: &mut Session, p: &Value) -> Result<Value> {
    let mut request = p.clone();
    if !request.is_object()
        || request["prompt"]
            .as_str()
            .is_none_or(|v| v.trim().is_empty())
    {
        return Err(EngineError::InvalidParams(
            "Scribbles needs an object with a nonempty final prompt".into(),
        ));
    }
    if request.get("poses").is_some() || request.get("rig_parts").is_some() {
        return Err(EngineError::InvalidParams(
            "Prepare pose/rig instructions in the final prompt before calling Scribbles".into(),
        ));
    }
    let settings = crate::project_settings::stored_settings();
    for key in ["provider", "model"] {
        if request.get(key).is_none() {
            if let Some(value) = settings["scribbles"].get(key) {
                request[key] = value.clone();
            } else if key == "model" {
                request[key] = crate::fidget::templates()?["model"].clone();
            }
        }
        if let Some(value) = request.get(key)
            && value.as_str().is_none_or(|v| v.trim().is_empty())
        {
            return Err(EngineError::InvalidParams(format!(
                "scribbles {key} must be a nonempty string"
            )));
        }
    }
    let mut result = crate::image_generation::generate(s, &request)?;
    if let Some(metadata) = p.get("fidget").filter(|v| v.is_object()) {
        let mut metadata = metadata.clone();
        let canvas = &metadata["canvas"];
        metadata["canvas_matches"] =
            json!(result["width"] == canvas[0] && result["height"] == canvas[1]);
        result["scribbles"] = metadata;
    }
    Ok(result)
}

fn draw(s: &mut Session, p: &Value, name: &str) -> Result<Value> {
    let prepared = crate::fidget::PreparedDrawing::prepare(p, name)?;
    let mut request = prepared.request;
    request["fidget"] = prepared.fidget;
    cmd_scribbles_generate(s, &request)
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

/// Templates remain discoverable at the existing compatibility endpoint.
pub fn cmd_scribbles_templates(_: &mut Session, _: &Value) -> Result<Value> {
    let all = crate::fidget::templates()?;
    Ok(json!({"ok": true, "model": all["model"], "kinds": all["kinds"]}))
}
