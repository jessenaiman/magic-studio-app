//! `doctor` subcommand (`magic-cli app doctor` / `app.doctor`):
//! lightweight local check of external CLI tool availability and stale `.video-server.json` state.
//! Never scans `/proc` to police or kill external processes.

use std::path::Path;

use serde_json::{Value, json};

use crate::workspace::resolve_tool;
use crate::{Result, Session};

pub fn cmd_app_doctor(s: &mut Session, params: &Value) -> Result<Value> {
    let clean = params
        .get("clean")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let cleaned_files: Vec<String> = Vec::new();
    let project_dir = s
        .project_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| s.cwd.clone());
    let server_file = project_dir.join(".video-server.json");
    let mut video_server_state = json!("none");
    if server_file.is_file() {
        if clean {
            video_server_state = json!({"file": server_file, "clean_requested": true, "removed": false, "note": "ownership/staleness not verified; not deleted"});
        } else {
            video_server_state = json!({"file": server_file});
        }
    }

    let tools: Vec<Value> = [
        "magic-cli",
        "agent-desktop",
        "codex",
        "hyperframes",
        "ffmpeg",
    ]
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
    let is_checkout =
        |r: &Path| r.join("Cargo.toml").is_file() && r.join("apps/magic-gui/Cargo.toml").is_file();
    if is_checkout(&s.cwd) {
        return s.cwd.clone();
    }
    if let Ok(canon) = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        && is_checkout(&canon)
    {
        return canon;
    }
    crate::workspace::workspace_root(s)
}

fn which_in_path(bin: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(bin).is_file()))
}
