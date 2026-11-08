//! Mayhem reads native analyzer reports and the logs the runtimes and gates already write.
//! Opening this view never runs an analyzer.
use crate::{Result, Session, doctor::analytics_source_root};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// One log file the system already produces: name, path, freshness and the last lines verbatim.
fn log_entry(name: &str, path: PathBuf) -> Value {
    match fs::metadata(&path) {
        Err(error) => json!({"name":name,"path":path,"error":format!("{error}")}),
        Ok(meta) => {
            let tail = fs::read_to_string(&path)
                .map(|text| text.lines().rev().take(200).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n"))
                .unwrap_or_default();
            let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs());
            json!({"name":name,"path":path,"bytes":meta.len(),"modified":modified,"tail":tail})
        }
    }
}

/// The runtime logs, activity journal and gate report — read on demand, never generated here.
fn logs(root: &Path) -> Value {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from).or_else(|| home.as_ref().map(|h| h.join(".local/state")));
    let data = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| home.as_ref().map(|h| h.join(".local/share")));
    let mut sources = vec![log_entry("gate report (cargo xtask ci)", root.join("target/ci-report.jsonl"))];
    if let Some(state) = &state {
        sources.push(log_entry("activity journal", state.join("magic/activity.jsonl")));
        sources.push(log_entry("activity history", state.join("magic/activity-history.jsonl")));
    }
    for dir in [data.as_ref().map(|d| d.join("ai.storyteller.magicstudio/logs")), state.as_ref().map(|s| s.join("ai.storyteller.magicstudio/logs"))]
        .into_iter()
        .flatten()
    {
        let mut files: Vec<PathBuf> =
            fs::read_dir(&dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "log")).collect();
        files.sort();
        for file in files {
            let name = format!("runtime log · {}", file.file_name().unwrap_or_default().to_string_lossy());
            sources.push(log_entry(&name, file));
        }
    }
    Value::Array(sources)
}

fn saved(root: &Path) -> Value {
    match fs::read(root.join("target/analytics/analyzers.json")) {
        Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(report) if report["schema"] == "magic.analyzers/1" && report["source_root"].as_str() == root.to_str() => report,
            Ok(_) => {
                json!({"error":"Saved report belongs to a different source directory or schema","tools":[]})
            }
            Err(error) => json!({"error":format!("Cannot parse saved report: {error}"),"tools":[]}),
        },
        Err(error) => json!({"error":format!("No readable analyzer report: {error}"),"tools":[]}),
    }
}

pub fn evidence(s: &mut Session, params: &Value) -> Result<Value> {
    if params["activity_only"] == true {
        return Ok(json!({"activity":crate::herdr::activity(s,params)}));
    }
    let root = params["path"].as_str().map(PathBuf::from).unwrap_or_else(|| analytics_source_root(s));
    let report = saved(&root);
    let commands: Vec<_> = crate::command_specs().iter().map(|c| json!({"id":c.id,"label":c.label,"params":c.params})).collect();
    Ok(json!({"root":root,"report":report,"activity":crate::herdr::activity(s, params),"commands":commands,
        "cli_events":crate::activity::events(s, &json!({"limit":192}))?,
        "logs":logs(&root)}))
}
