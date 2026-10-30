//! Part visibility / solo / review state on the active asset.

use serde_json::{json, Value};

use crate::{EngineError, Result, Session};

// Explicit targets do not borrow another caller's active selection.
fn target<'a>(s: &'a mut Session, params: &Value) -> Result<&'a mut crate::Asset> {
    if let Some(character) = params.get("character") {
        crate::motion_authoring::project_guard(s, params)?;
        let id = character
            .as_str()
            .ok_or_else(|| EngineError::InvalidParams("character must be an asset ID".into()))?;
        let selected = s.active_asset.clone();
        let loaded = crate::project::cmd_project_load_asset(s, &json!({"id":id}));
        s.active_asset = selected;
        loaded?;
        return s
            .assets
            .get_mut(id)
            .ok_or_else(|| EngineError::InvalidParams("Target asset unavailable".into()));
    }
    if params.get("expected_project").is_some() {
        crate::motion_authoring::project_guard(s, params)?;
    }
    s.current_asset_mut()
        .ok_or_else(|| EngineError::InvalidParams("no active asset in session".into()))
}

/// Ensure every part of the active asset is visible and none are soloed.
pub fn cmd_part_show_all(s: &mut Session, params: &Value) -> Result<Value> {
    let asset = target(s, params)?;
    for part in &mut asset.parts {
        part.visible = true;
        part.solo = false;
    }
    Ok(json!({
        "ok": true,
        "command": "part.show_all",
        "asset_id": asset.id,
        "parts": asset.parts,
    }))
}

/// Optional visibility / solo / verification updates requested by `part.set_state`.
struct PartStatePatch {
    visible: Option<bool>,
    solo: Option<bool>,
    verified: Option<bool>,
}

impl PartStatePatch {
    fn from_params(params: &Value) -> Self {
        Self {
            visible: params.get("visible").and_then(Value::as_bool),
            solo: params.get("solo").and_then(Value::as_bool),
            verified: params.get("verified").and_then(Value::as_bool),
        }
    }

    fn apply(&self, part: &mut crate::PartState) {
        if let Some(v) = self.visible {
            part.visible = v;
        }
        if let Some(so) = self.solo {
            part.solo = so;
        }
        if let Some(ver) = self.verified {
            part.verified = ver;
        }
    }
}

fn set_state_all(asset: &mut crate::Asset, patch: &PartStatePatch) -> Value {
    for p in &mut asset.parts {
        patch.apply(p);
    }
    json!({
        "ok": true,
        "command": "part.set_state",
        "part": "*",
        "parts": asset.parts,
    })
}

fn set_state_one(
    asset: &mut crate::Asset,
    part_name: &str,
    patch: &PartStatePatch,
) -> Result<Value> {
    let entry = asset
        .parts
        .iter_mut()
        .find(|p| p.name == part_name)
        .ok_or_else(|| {
            EngineError::InvalidParams(format!("part `{part_name}` not found in active asset"))
        })?;
    patch.apply(entry);
    let updated = entry.clone();
    Ok(json!({
        "ok": true,
        "command": "part.set_state",
        "part": updated.name,
        "visible": updated.visible,
        "solo": updated.solo,
        "verified": updated.verified,
    }))
}

/// Update visibility/solo/review state of one part (or `"*"`/`"all"` for every part) on the active asset.
pub fn cmd_part_set_state(s: &mut Session, params: &Value) -> Result<Value> {
    let part_name = params
        .get("part")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::InvalidParams("missing 'part' parameter".into()))?;
    let patch = PartStatePatch::from_params(params);
    let asset = target(s, params)?;
    if part_name == "*" || part_name.eq_ignore_ascii_case("all") {
        return Ok(set_state_all(asset, &patch));
    }
    set_state_one(asset, part_name, &patch)
}
