//! Mayhem reads native analyzer reports. Opening this view never runs an analyzer.
use crate::{Result, Session, doctor::analytics_source_root};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

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
        "cli_events":crate::activity::events(s, &json!({"limit":192}))?}))
}
