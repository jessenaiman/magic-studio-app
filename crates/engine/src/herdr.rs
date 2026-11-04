//! Read-only adapter for Herdr's published SessionSnapshot, never inferred worktree identities.
//! Schema: herdrdev/herdr ee5bfb5, src/api/schema/{session,panes,tabs,agents,workspaces}.rs.
use crate::Session;
use serde_json::{Value, json};
use std::{path::Path, process::Command};

pub fn activity(s: &Session, params: &Value) -> Value {
    let project = s
        .project_path
        .as_deref()
        .and_then(Path::parent)
        .unwrap_or(&s.cwd);
    let workspace = params["workspace_id"]
        .as_str()
        .map(str::to_string)
        .or_else(|| std::env::var("HERDR_WORKSPACE_ID").ok())
        .filter(|s| !s.is_empty());
    let read = || -> Result<Value, String> {
        if std::env::var("HERDR_ENV").as_deref() != Ok("1") {
            return Err(
                "Herdr caller context unavailable (HERDR_ENV != 1); no session queried".into(),
            );
        }
        let workspace = workspace
            .as_deref()
            .ok_or("Workspace unknown; supply workspace_id from Herdr")?;
        let project = project
            .canonicalize()
            .map_err(|e| format!("Project directory unavailable: {e}"))?;
        let output = Command::new("herdr")
            .args(["api", "snapshot"])
            .output()
            .map_err(|e| format!("herdr api snapshot: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "herdr api snapshot: {} · {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let response: Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| format!("Invalid Herdr JSON: {e}"))?;
        if !response["error"].is_null() {
            return Err(format!("Herdr returned {}", response["error"]));
        }
        let snapshot = &response["result"]["snapshot"];
        for key in ["workspaces", "tabs", "panes", "agents"] {
            if !snapshot[key].is_array() {
                return Err(format!("Unsupported Herdr snapshot: missing {key}"));
            }
        }
        let workspaces = snapshot["workspaces"]
            .as_array()
            .ok_or("Missing workspaces")?;
        let ws = workspaces
            .iter()
            .find(|w| w["workspace_id"] == workspace)
            .ok_or("Requested workspace not present in snapshot")?;
        let mut unknown_directories = 0;
        let rows: Vec<_> = snapshot["panes"].as_array().into_iter().flatten().filter(|p| p["workspace_id"] == workspace).filter_map(|pane| {
            let directory = pane["foreground_cwd"].as_str().or_else(|| pane["cwd"].as_str());
            let Some(path) = directory.filter(|p| Path::new(p).is_absolute()).and_then(|p| Path::new(p).canonicalize().ok()) else { unknown_directories += 1; return None; };
            if !path.starts_with(&project) { return None; }
            let agent = snapshot["agents"].as_array().into_iter().flatten().find(|a| a["workspace_id"] == workspace && a["pane_id"] == pane["pane_id"]);
            let tab = snapshot["tabs"].as_array().into_iter().flatten().find(|t| t["workspace_id"] == workspace && t["tab_id"] == pane["tab_id"]);
            Some(json!({"workspace_id":ws["workspace_id"],"workspace_name":ws["label"],
                "tab_id":pane["tab_id"],"tab_name":tab.map(|t| &t["label"]),
                "pane_id":pane["pane_id"],"pane_name":pane["label"],"terminal_id":pane["terminal_id"],
                "agent_name":agent.map(|a| &a["name"]),"agent_kind":agent.map(|a| &a["agent"]),
                "agent_status":agent.map(|a| &a["agent_status"]),"cwd":path}))
        }).collect();
        Ok(
            json!({"available":true,"version":snapshot["version"],"project_directory":project,"workspace_id":workspace,
            "scope":"Verified project directory and descendants, within the requested workspace", "rows":rows,"unknown_directories":unknown_directories}),
        )
    };
    read().unwrap_or_else(|error| json!({"available":false,"error":error,"rows":[],"project_directory":project,"workspace_id":workspace}))
}
