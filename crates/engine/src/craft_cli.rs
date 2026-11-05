//! Access installed Craft releases without depending on Magic's pinned engine versions.
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

pub fn execute(session: &mut Session, params: &Value) -> Result<Value> {
    let app = params["app"].as_str().unwrap_or_default();
    let tool = match app {
        "filmcraft" => "filmcraft-cli",
        "effectcraft" => "effectcraft-cli",
        "vectorcraft" => "vectorcraft-cli",
        "photocraft" => "photocraft-cli",
        "wordcraft" => "wordcraft-cli",
        _ => {
            return Err(EngineError::InvalidParams(
                "app must be filmcraft, effectcraft, vectorcraft, photocraft, or wordcraft".into(),
            ));
        }
    };
    let args = params["args"]
        .as_array()
        .filter(|args| !args.is_empty())
        .ok_or_else(|| EngineError::InvalidParams("args must be a nonempty string array".into()))?;
    let args = args
        .iter()
        .map(|arg| {
            arg.as_str()
                .ok_or_else(|| EngineError::InvalidParams("args must contain only strings".into()))
        })
        .collect::<Result<Vec<_>>>()?;
    let cwd = match params.get("cwd") {
        Some(value) => crate::workspace::resolve_workspace_path(
            session,
            value
                .as_str()
                .ok_or_else(|| EngineError::InvalidParams("cwd must be a directory path".into()))?,
        ),
        None => match session.project_path.as_ref().and_then(|path| path.parent()) {
            Some(path) => path.to_path_buf(),
            None => std::env::current_dir()?,
        },
    };
    let _slot = crate::workspace::acquire_tool_slot();
    // The version-independent CLI name on PATH is the Launcher-managed entry point.
    // Pass each argument literally: no shell, interpolation, or generated script.
    let output = Command::new(tool)
        .args(&args)
        .current_dir(&cwd)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| EngineError::ExternalTool(tool.into(), error.to_string()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(EngineError::ExternalTool(
            tool.into(),
            format!(
                "exit {:?}\nstdout: {}\nstderr: {}",
                output.status.code(),
                stdout.trim(),
                stderr.trim()
            ),
        ));
    }
    Ok(
        json!({"app": app, "cwd": cwd, "exit_code": output.status.code(),
        "stdout": stdout, "stderr": stderr,
        "result": serde_json::from_str::<Value>(&stdout).ok()}),
    )
}
