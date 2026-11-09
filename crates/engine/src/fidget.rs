//! Fidget prepares inspectable drawing requests and structured Markdown briefs; Scribbles owns image generation.
//!
//! Both Fidget (`RoleId("fidget")`, `text.complete`) and Scribbles (`RoleId("scribbles")`,
//! `image.generate`) are declared as [`craft_ai_core::Role`]s on the shared Craft AI client
//! ([`craft_ai_client::AiClient`] + [`craft_ai_client::ToolDesk`]).

use craft_ai_client::{AiClient, RoleStatus};
use craft_ai_core::{AppId, CapabilityId, InputRole, Role, RoleId};
use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

const TEMPLATES: &str = include_str!("scribbles/templates.json");

/// Mandatory single-line notice included in every Fidget Markdown template so downstream agents
/// treat templates as working structures and report areas or commands needing improvement.
pub const FIDGET_NOTE: &str = "NOTE: This template is a working structure, not a final specification — follow it to the best of your ability and report any areas or commands that need improvement after use.";

fn bad(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

pub(crate) fn templates() -> Result<Value> {
    serde_json::from_str(TEMPLATES).map_err(|e| bad(format!("scribbles templates: {e}")))
}

/// Declare Fidget (`text.complete`) as a Craft AI role.
pub fn fidget_role() -> Result<Role> {
    Ok(Role {
        id: RoleId::new("fidget").map_err(|e| bad(e.to_string()))?,
        label: "Fidget (Prompt & Markdown Template Architect)".into(),
        capability: CapabilityId::parse("text.complete").map_err(|e| bad(e.to_string()))?,
        inputs: vec![InputRole::Prompt, InputRole::System],
    })
}

/// Declare Scribbles (`image.generate`) as a Craft AI role.
pub fn scribbles_role() -> Result<Role> {
    Ok(Role {
        id: RoleId::new("scribbles").map_err(|e| bad(e.to_string()))?,
        label: "Scribbles (Template-Gated Character & Storyboard Artist)".into(),
        capability: CapabilityId::parse("image.generate").map_err(|e| bad(e.to_string()))?,
        inputs: vec![InputRole::Prompt, InputRole::Reference],
    })
}

/// Declare Scribbles Mark (`image.detect`) as a Craft AI role.
pub fn scribbles_mark_role() -> Result<Role> {
    Ok(Role {
        id: RoleId::new("scribbles-mark").map_err(|e| bad(e.to_string()))?,
        label: "Scribbles Mark (Part & Joint Detector)".into(),
        capability: CapabilityId::parse("image.detect").map_err(|e| bad(e.to_string()))?,
        inputs: vec![InputRole::Prompt, InputRole::Source],
    })
}

/// All Craft AI roles declared by Magic Studio.
pub fn magic_ai_roles() -> Result<Vec<Role>> {
    Ok(vec![fidget_role()?, scribbles_role()?, scribbles_mark_role()?])
}

/// Inspect the live Craft AI status (`AiClient::standard` + `ToolDesk`) for Magic Studio's roles.
pub fn craft_ai_status() -> Value {
    let roles = match magic_ai_roles() {
        Ok(r) => r,
        Err(e) => return json!({ "ok": false, "error": e.to_string() }),
    };
    let app_id = match AppId::new("magic") {
        Ok(id) => id,
        Err(e) => return json!({ "ok": false, "error": e.to_string() }),
    };
    let role_reports: Vec<Value> = match AiClient::standard(app_id).map(|client| client.with_factory(crate::craft_ai_codex::factory())) {
        Ok(client) => roles
            .iter()
            .map(|role| {
                let status = match client.status(role) {
                    RoleStatus::Ready(ready) => json!({
                        "state": "ready",
                        "connection_id": ready.connection.as_str(),
                        "connection_label": ready.connection_label,
                        "model": ready.model.as_str(),
                        "model_label": ready.model_label,
                        "locality": format!("{:?}", ready.locality),
                        "billing": format!("{:?}", ready.billing),
                    }),
                    RoleStatus::Unavailable { reason } => json!({
                        "state": "unavailable",
                        "reason": reason.to_string(),
                    }),
                };
                json!({
                    "id": role.id.as_str(),
                    "label": role.label,
                    "capability": role.capability.as_str(),
                    "status": status,
                })
            })
            .collect(),
        Err(e) => roles
            .iter()
            .map(|role| {
                json!({
                    "id": role.id.as_str(),
                    "label": role.label,
                    "capability": role.capability.as_str(),
                    "status": { "state": "unavailable", "reason": e.to_string() },
                })
            })
            .collect(),
    };
    json!({
        "ok": true,
        "app_id": "magic",
        "roles": role_reports,
    })
}

/// Format any Fidget prompt or response as Markdown with YAML frontmatter and the mandatory
/// single-line `NOTE:` sentence.
pub fn format_fidget_markdown(template: &str, save_command: &str, tags: &[String], canvas: Option<[u64; 2]>, body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.starts_with("---\n") && trimmed.contains(FIDGET_NOTE) {
        return trimmed.to_owned();
    }
    let tags_str = if tags.is_empty() { format!("[magic, fidget, {template}]") } else { format!("[{}]", tags.join(", ")) };
    let canvas_line = canvas.map(|[w, h]| format!("canvas: [{w}, {h}]\n")).unwrap_or_default();
    format!(
        "---\n\
         role: fidget\n\
         template: {template}\n\
         version: 1\n\
         status: working-draft\n\
         tags: {tags_str}\n\
         save_command: {save_command}\n\
         {canvas_line}---\n\
         > {FIDGET_NOTE}\n\n\
         {trimmed}\n\n\
         ## Save Command\n\
         Save and register the output using `magic-cli exec {save_command}`.\n"
    )
}

/// Extract the `template:` field from YAML frontmatter (`---` ... `---`), if present.
pub fn parse_frontmatter_template(text: &str) -> Option<String> {
    let trimmed = text.trim_start();
    let rest = trimmed.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    let frontmatter = &rest[..end];
    for line in frontmatter.lines() {
        if let Some((k, v)) = line.split_once(':')
            && k.trim() == "template"
        {
            let value = v.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_owned());
            }
        }
    }
    None
}

/// `{frames}`, `{action}`, `{view}`, `{keys}` and `{prompt}` filled from the request.
fn layout(kind: &Value, p: &Value) -> Result<String> {
    let mut text = kind["layout"].as_str().unwrap_or_default().to_string();
    if let Some(actions) = kind["actions"].as_object() {
        let name = p["action"].as_str().unwrap_or("walk");
        let action = actions.get(name).ok_or_else(|| bad(format!("action must be one of {}", actions.keys().cloned().collect::<Vec<_>>().join(", "))))?;
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
    let [w, h] = [kind["canvas"][0].as_u64().unwrap_or(1024), kind["canvas"][1].as_u64().unwrap_or(1536)];
    let mut parts = vec![format!("Draw ONE image on a {w} x {h} px canvas."), layout(kind, p)?];
    for rule in kind["rules"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        parts.push(all[rule].as_str().unwrap_or_default().to_string());
    }
    if kind["layout"].as_str() != Some("{prompt}")
        && !kind["layout"].as_str().unwrap_or_default().contains("{prompt}")
        && let Some(prompt) = p["prompt"].as_str().filter(|t| !t.trim().is_empty())
    {
        parts.push(format!("Character: {prompt}"));
    }
    if let Some(notes) = p["notes"].as_str().filter(|t| !t.trim().is_empty()) {
        parts.push(format!("Notes: {notes}"));
    }
    let shared = kind["shared_override"].as_str().or_else(|| all["shared"].as_str()).unwrap_or_default();
    if !shared.is_empty() {
        parts.push(shared.to_string());
    }
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
        let kind = all["kinds"].get(name).ok_or_else(|| bad(format!("unknown drawing kind {name}")))?;
        if p.get("reference").is_some() && p.get("references").is_some() {
            return Err(bad("use reference or references, not both"));
        }
        let references = match p.get("references").or_else(|| p.get("reference")) {
            None => Vec::new(),
            Some(Value::String(one)) => vec![json!(one)],
            Some(Value::Array(many)) => many.clone(),
            _ => {
                return Err(bad("references must be an image path or array of image paths"));
            }
        };
        if references.len() > 5 || references.iter().any(|r| r.as_str().is_none_or(|s| s.trim().is_empty())) {
            return Err(bad("references must contain at most five nonempty image paths"));
        }
        if kind["needs_reference"] == true && references.is_empty() {
            return Err(bad(format!("{name} needs reference=<image of the character>")));
        }
        if name == "new" && p["prompt"].as_str().is_none_or(|t| t.trim().is_empty()) {
            return Err(bad("new needs prompt=<the character to draw>"));
        }
        let prompt = compose(&all, kind, p)?;
        let save_command = kind["save_command"].as_str().unwrap_or("project.add_asset").to_owned();
        let save_kind = kind["save_kind"].as_str().unwrap_or("image").to_owned();
        let save_slot = kind["save_slot"].as_str().map(str::to_owned);
        let tags: Vec<String> = kind["tags"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect();
        let canvas = [kind["canvas"][0].as_u64().unwrap_or(1024), kind["canvas"][1].as_u64().unwrap_or(1536)];
        let markdown = format_fidget_markdown(name, &save_command, &tags, Some(canvas), &prompt);
        let mut request = json!({
            "kind": name,
            "prompt": prompt,
            "markdown": markdown,
            "transparent_background": !matches!(name, "storyboard_hero" | "storyboard_sketch"),
            "timeout_s": p["timeout_s"].as_u64().unwrap_or(900),
            "name": p["name"].as_str().map(str::to_owned).unwrap_or_else(|| format!("scribbles-{name}"))
        });
        if !references.is_empty() {
            request["references"] = json!(references);
        }
        for key in ["provider", "model", "character", "project", "scene", "action", "count"] {
            if let Some(value) = p.get(key) {
                request[key] = value.clone();
            }
        }
        Ok(Self {
            request,
            fidget: json!({
                "role": "fidget",
                "kind": name,
                "label": kind["label"],
                "reference": kind["reference"],
                "canvas": kind["canvas"],
                "tags": tags,
                "save_command": save_command,
                "save_kind": save_kind,
                "save_slot": save_slot,
                "note": FIDGET_NOTE,
                "prompt": prompt,
                "markdown": markdown,
            }),
        })
    }
}

/// Return a final prompt, Markdown brief with YAML frontmatter + `NOTE:`, and references for
/// `scribbles.generate --params JSON`.
pub fn cmd_fidget_prepare(_: &mut Session, p: &Value) -> Result<Value> {
    let name = p["kind"].as_str().or_else(|| p["template"].as_str()).unwrap_or("custom");
    let prepared = PreparedDrawing::prepare(p, name)?;
    let mut request = prepared.request;
    request["ok"] = json!(true);
    request["fidget"] = prepared.fidget;
    Ok(request)
}
