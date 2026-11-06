//! Pure feedback rules shared by the Studio window, headless runs and the bridge. They read the
//! engine's JSON state and never touch a window, so every front end reports the same verdict.
use serde_json::{Value, json};

pub fn dirty_reasons(v: &Value, dirty: Option<bool>) -> Vec<Value> {
    let mut reasons = Vec::new();
    if dirty == Some(true) {
        reasons.push(json!("engine state differs from saved project"));
    }
    if dirty.is_none() {
        reasons.push(json!(
            "engine state unavailable; busy or unreadable saved project"
        ));
    }
    for field in v["draft_fields"].as_array().into_iter().flatten() {
        reasons.push(json!({"draft_field":field}));
    }
    if v["pending_invokes"].as_u64().unwrap_or(0) > 0 {
        reasons.push(json!({"pending_invokes":v["pending_invokes"]}));
    }
    reasons
}

/// Engine edits since the last project write, plus each changed form field. A clean diff
/// means 0 even after commands (e.g. exports) that never touch project state.
pub fn unsaved_edits(v: &Value, dirty: Option<bool>, edits: Option<u64>) -> Value {
    let drafts = v["draft_fields"].as_array().map_or(0, Vec::len) as u64;
    match (dirty, edits) {
        (Some(false), _) => json!(drafts),
        (Some(true), Some(n)) => json!(n.max(1) + drafts),
        _ => Value::Null,
    }
}

pub fn unsaved_state(v: &Value, dirty: Option<bool>) -> &'static str {
    if dirty == Some(true)
        || v["draft_fields"].as_array().is_some_and(|a| !a.is_empty())
        || v["pending_invokes"].as_u64().unwrap_or(0) > 0
    {
        return "unsaved_or_pending";
    }
    if dirty == Some(false) && v["phase"] == "ready" {
        return "no_detected_unsaved_changes";
    }
    "unknown"
}

pub fn acknowledgement(v: &Value, ack: u64, revision: u64) -> &'static str {
    if v["phase"] == "error" {
        return "frontend_error";
    }
    if ack >= revision && v["phase"] == "ready" {
        return "DOM_applied; visual review separate";
    }
    "pending"
}

pub fn ui_ready(v: &Value, ack: u64, revision: u64) -> bool {
    v["phase"] == "ready"
        && v["rendered"] == true
        && v["errors"].as_array().is_some_and(|e| e.is_empty())
        && v["project"] == v["active_project"]
        && ack >= revision
}

#[cfg(test)]
#[path = "tests/feedback_state.rs"]
mod tests;
