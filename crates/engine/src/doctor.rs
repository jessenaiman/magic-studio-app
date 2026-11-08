//! `doctor` subcommand (`magic-cli app doctor` / `app.doctor`):
//! lightweight local check of external CLI tool availability and stale `.video-server.json` state.
//! Never scans `/proc` to police or kill external processes.

use std::path::Path;

use serde_json::{Value, json};

use crate::workspace::resolve_tool;
use crate::{Result, Session};

/// `monitoring.prerequisites`: one state per footer stage, for the footer, Mayhem and any
/// consumer that reads `data.monitoring.prerequisites` from the session snapshot. Computed
/// from the same project state as `workshop_status` — no polling, no journal, no new scans.
pub(crate) fn prerequisites(s: &Session, status: &Value) -> Value {
    let stage = |id: &str| status["stages"].as_array().and_then(|stages| stages.iter().find(|s| s["id"] == id));
    let p = |id: &str| {
        stage(id)
            .map(|s| json!({"state": stage_state(s), "detail": s["why"].as_str().unwrap_or("").to_string()}))
            .unwrap_or_else(|| json!({"state":"not-started","detail":"not started"}))
    };
    let sb = &status["storyboard"];
    let sb_state = match (sb["error"].as_str(), sb["complete"].as_bool()) {
        (Some(error), _) => ("failed", error.to_string()),
        (None, Some(true)) => ("ready", "storyboard complete".to_string()),
        (None, _) => ("needs-work", format!("{} unfilled fields", sb["count"].as_u64().unwrap_or(0))),
    };
    let timeline = &status["film_timeline"];
    let studio = match timeline["exists"].as_bool() {
        Some(true) => ("ready", timeline["path"].as_str().unwrap_or("film timeline linked").to_string()),
        _ => ("needs-work", "no film timeline linked".to_string()),
    };
    json!({
        "song": p("song"),
        "lyrics": p("lyrics"),
        "transcript": p("transcript"),
        "storyboard": {"state": sb_state.0, "detail": sb_state.1},
        "animate": p("animate"),
        "studio": {"state": studio.0, "detail": studio.1},
    })
}

fn stage_state(s: &Value) -> &'static str {
    if s["done"] == json!(true) {
        return "ready";
    }
    if s["problems"].as_array().is_some_and(|problems| !problems.is_empty()) {
        return role_state::failed;
    }
    "needs-work"
}

mod role_state {
    pub(crate) const failed: &str = "failed";
}

pub fn cmd_app_doctor(s: &mut Session, params: &Value) -> Result<Value> {
    let clean = params.get("clean").and_then(Value::as_bool).unwrap_or(false);

    let cleaned_files: Vec<String> = Vec::new();
    let project_dir = s.project_path.as_ref().and_then(|p| p.parent()).map(Path::to_path_buf).unwrap_or_else(|| s.cwd.clone());
    let server_file = project_dir.join(".video-server.json");
    let mut video_server_state = json!("none");
    if server_file.is_file() {
        if clean {
            video_server_state =
                json!({"file": server_file, "clean_requested": true, "removed": false, "note": "ownership/staleness not verified; not deleted"});
        } else {
            video_server_state = json!({"file": server_file});
        }
    }

    let tools: Vec<Value> = ["magic-cli", "agent-desktop", "codex", "hyperframes", "ffmpeg"]
        .iter()
        .map(|name| {
            let resolved = resolve_tool(name);
            let found = Path::new(&resolved).is_file() || which_in_path(&resolved);
            json!({ "tool": name, "resolved": resolved, "available": found })
        })
        .collect();

    Ok(json!({
        "ok": true,
        "cleaned": {
            "stopped_pids": null,
            "removed_files": cleaned_files,
            "note": "processes are not scanned or killed; .video-server.json is reported but not verified or deleted"
        },
        "processes": null,
        "video_server": video_server_state,
        "tools": tools,
        "in_process_libraries": [
            "craft-ai-core",
            "craft-atomic",
            "craft-doc",
            "craft-docpath",
            "craft-settings",
            "designcraft-engine",
            "designcraft-render",
            "effectcraft-engine",
            "effectcraft-host",
            "filmcraft-engine",
            "photocraft-engine",
            "texturecraft-formats",
            "vectorcraft-engine"
        ],
        "ponytail_candidates": null,
        "recommendations": [],
    }))
}

pub(crate) fn analytics_source_root(s: &crate::Session) -> std::path::PathBuf {
    let is_checkout = |r: &Path| r.join("Cargo.toml").is_file() && r.join("crates/engine/Cargo.toml").is_file();
    if is_checkout(&s.cwd) {
        return s.cwd.clone();
    }
    if let Ok(canon) = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize()
        && is_checkout(&canon)
    {
        return canon;
    }
    crate::workspace::workspace_root(s)
}

fn which_in_path(bin: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(bin).is_file()))
}
