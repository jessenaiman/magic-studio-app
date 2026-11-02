//! Fidget prepares inspectable drawing requests; Scribbles owns image generation.

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

const TEMPLATES: &str = include_str!("scribbles/templates.json");

fn bad(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

pub(crate) fn templates() -> Result<Value> {
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

/// Final drawing instructions and provenance, independent of the image backend.
/// Preparation does not launch a model, read reference bytes, or mutate a project.
#[derive(serde::Serialize)]
pub(crate) struct PreparedDrawing {
    pub request: Value,
    pub fidget: Value,
}

impl PreparedDrawing {
    pub fn prepare(p: &Value, name: &str) -> Result<Self> {
        let all = templates()?;
        let kind = all["kinds"]
            .get(name)
            .ok_or_else(|| bad(format!("unknown drawing kind {name}")))?;
        if p.get("reference").is_some() && p.get("references").is_some() {
            return Err(bad("use reference or references, not both"));
        }
        let references = match p.get("references").or_else(|| p.get("reference")) {
            None => Vec::new(),
            Some(Value::String(one)) => vec![json!(one)],
            Some(Value::Array(many)) => many.clone(),
            _ => {
                return Err(bad(
                    "references must be an image path or array of image paths",
                ));
            }
        };
        if references.len() > 5
            || references
                .iter()
                .any(|r| r.as_str().is_none_or(|s| s.trim().is_empty()))
        {
            return Err(bad(
                "references must contain at most five nonempty image paths",
            ));
        }
        if kind["needs_reference"] == true && references.is_empty() {
            return Err(bad(format!(
                "{name} needs reference=<image of the character>"
            )));
        }
        if name == "new" && p["prompt"].as_str().is_none_or(|t| t.trim().is_empty()) {
            return Err(bad("new needs prompt=<the character to draw>"));
        }
        let prompt = compose(&all, kind, p)?;
        let mut request = json!({
            "prompt": prompt,
            "transparent_background": true,
            "timeout_s": p["timeout_s"].as_u64().unwrap_or(900),
            "name": p["name"].as_str().map(str::to_owned).unwrap_or_else(|| format!("scribbles-{name}"))
        });
        if !references.is_empty() {
            request["references"] = json!(references);
        }
        for key in ["provider", "model", "character", "project"] {
            if let Some(value) = p.get(key) {
                request[key] = value.clone();
            }
        }
        Ok(Self {
            request,
            fidget: json!({"kind": name, "label": kind["label"],
                "reference": kind["reference"], "canvas": kind["canvas"], "prompt": prompt}),
        })
    }
}

/// Return a final prompt and references for `scribbles.generate --params JSON`.
pub fn cmd_fidget_prepare(_: &mut Session, p: &Value) -> Result<Value> {
    let name = p["kind"].as_str().unwrap_or("custom");
    let prepared = PreparedDrawing::prepare(p, name)?;
    let mut request = prepared.request;
    request["ok"] = json!(true);
    request["fidget"] = prepared.fidget;
    Ok(request)
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
