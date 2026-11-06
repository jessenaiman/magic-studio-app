//! Scribbles executes template-gated drawing requests through the configured image backend.
//!
//! Access to Scribbles requires a template (either prepared by [`crate::fidget::cmd_fidget_prepare`],
//! selected via `kind`/`template`, or carried in YAML frontmatter), which also prescribes the
//! engine command used to save and register the generated image (`project.add_asset`,
//! `storyboard.set`, `character.sprite`).

use crate::{EngineError, Result, Session};
use serde_json::{Value, json};

fn resolve_template_request(p: &Value) -> Result<(Value, Value)> {
    let all = crate::fidget::templates()?;
    let kinds = &all["kinds"];

    // 1. Already prepared by `fidget.prepare`
    if let Some(fidget) = p.get("fidget").filter(|v| v.is_object()) {
        let kind_name = fidget["kind"]
            .as_str()
            .filter(|k| kinds.get(*k).is_some())
            .ok_or_else(|| {
                EngineError::InvalidParams(
                    "Scribbles requires a valid template in `fidget.kind`; inspect `scribbles.templates`".into(),
                )
            })?;
        let spec = &kinds[kind_name];
        let mut meta = fidget.clone();
        if meta.get("save_command").is_none() {
            meta["save_command"] = spec["save_command"].clone();
        }
        if meta.get("save_kind").is_none() {
            meta["save_kind"] = spec["save_kind"].clone();
        }
        if meta.get("save_slot").is_none() {
            meta["save_slot"] = spec["save_slot"].clone();
        }
        if meta.get("note").is_none() {
            meta["note"] = json!(crate::fidget::FIDGET_NOTE);
        }
        return Ok((p.clone(), meta));
    }

    // 2. Explicit `kind` or `template` parameter -> prepare via Fidget
    if let Some(kind_name) = p["kind"].as_str().or_else(|| p["template"].as_str()) {
        let prepared = crate::fidget::PreparedDrawing::prepare(p, kind_name)?;
        return Ok((prepared.request, prepared.fidget));
    }

    // 3. Prompt or markdown carries Fidget YAML frontmatter (`---\n...template: <kind>...`)
    let frontmatter_kind = p["markdown"]
        .as_str()
        .and_then(crate::fidget::parse_frontmatter_template)
        .or_else(|| {
            p["prompt"]
                .as_str()
                .and_then(crate::fidget::parse_frontmatter_template)
        });
    if let Some(kind_name) = frontmatter_kind {
        let spec = kinds.get(&kind_name).ok_or_else(|| {
            EngineError::InvalidParams(format!(
                "Unknown Scribbles template `{kind_name}` in Markdown frontmatter; inspect `scribbles.templates`"
            ))
        })?;
        let meta = json!({
            "role": "fidget",
            "kind": kind_name,
            "label": spec["label"],
            "reference": spec["reference"],
            "canvas": spec["canvas"],
            "tags": spec["tags"],
            "save_command": spec["save_command"],
            "save_kind": spec["save_kind"],
            "save_slot": spec["save_slot"],
            "note": crate::fidget::FIDGET_NOTE,
        });
        return Ok((p.clone(), meta));
    }

    Err(EngineError::InvalidParams(
        "Scribbles requires a template (`kind`, `template`, or `fidget` from `fidget.prepare`); inspect `scribbles.templates`".into(),
    ))
}

fn apply_template_save_command(
    s: &mut Session,
    request: &Value,
    metadata: &Value,
    generated: &Value,
) -> Result<Value> {
    let save_cmd = metadata["save_command"]
        .as_str()
        .unwrap_or("project.add_asset");
    let image_path = generated["image"].as_str().unwrap_or_default();
    if save_cmd == "storyboard.set"
        && !image_path.is_empty()
        && (request.get("scene").is_some() || request.get("page").is_some())
    {
        let slot = request["slot"]
            .as_str()
            .or_else(|| metadata["save_slot"].as_str())
            .unwrap_or("hero_image");
        let mut set_params = json!({
            "images": { slot: image_path }
        });
        for key in ["path", "page", "scene", "label"] {
            if let Some(v) = request.get(key) {
                set_params[key] = v.clone();
            }
        }
        let receipt = s.execute("storyboard.set", set_params)?;
        return Ok(json!({
            "command": "storyboard.set",
            "slot": slot,
            "receipt": receipt,
        }));
    }
    Ok(json!({
        "command": save_cmd,
        "asset_id": generated["asset_id"],
        "image": generated["image"],
    }))
}

/// Generate a final prompt gated by a Scribbles template. Supports single or looped (`count` 1..=4)
/// `image_gen` execution and runs the template's prescribed save command.
pub fn cmd_scribbles_generate(s: &mut Session, p: &Value) -> Result<Value> {
    if !p.is_object() {
        return Err(EngineError::InvalidParams(
            "Scribbles needs a JSON object with a template and prompt".into(),
        ));
    }
    if p.get("poses").is_some() || p.get("rig_parts").is_some() {
        return Err(EngineError::InvalidParams(
            "Prepare pose/rig instructions in the final prompt before calling Scribbles".into(),
        ));
    }
    let (mut request, mut metadata) = resolve_template_request(p)?;
    if request["prompt"]
        .as_str()
        .is_none_or(|v| v.trim().is_empty())
    {
        return Err(EngineError::InvalidParams(
            "Scribbles needs a nonempty final prompt".into(),
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
    let count = request.get("count").and_then(Value::as_u64).unwrap_or(1);
    if !(1..=4).contains(&count) {
        return Err(EngineError::InvalidParams(
            "scribbles count must be between 1 and 4".into(),
        ));
    }
    let mut runs = Vec::new();
    let mut last_result = Value::Null;
    for index in 0..count {
        let mut iter_req = request.clone();
        if count > 1 {
            let base_name = request["name"].as_str().unwrap_or("scribbles-item");
            iter_req["name"] = json!(format!("{base_name}-{}", index + 1));
        }
        let generated = crate::image_generation::generate(s, &iter_req)?;
        let save_receipt = apply_template_save_command(s, &iter_req, &metadata, &generated)?;
        let mut entry = generated.clone();
        entry["save_receipt"] = save_receipt;
        runs.push(entry);
        last_result = generated;
    }
    let canvas = &metadata["canvas"];
    metadata["canvas_matches"] =
        json!(last_result["width"] == canvas[0] && last_result["height"] == canvas[1]);
    metadata["loop_count"] = json!(count);
    if let Some(first) = runs.first() {
        metadata["save_receipt"] = first["save_receipt"].clone();
    }
    if count > 1 {
        metadata["items"] = json!(runs);
    }
    last_result["scribbles"] = metadata;
    Ok(last_result)
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
    Ok(json!({
        "ok": true,
        "model": all["model"],
        "note": all["note"],
        "kinds": all["kinds"],
        "craft_ai": crate::fidget::craft_ai_status(),
    }))
}

#[cfg(test)]
#[path = "tests/scribbles.rs"]
mod tests;
