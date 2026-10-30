//! Project-owned EffectCraft compositions; editing and evaluation stay in EffectCraft.
use crate::workspace::{
    resolve_craft_cli, resolve_workspace_path, run_python_json, run_tool_in, workspace_root,
};
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::path::PathBuf;

fn path(s: &Session, p: &Value, key: &str) -> Result<PathBuf> {
    let raw = p[key]
        .as_str()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| EngineError::InvalidParams(format!("{key} is required")))?;
    let path = resolve_workspace_path(s, raw).canonicalize()?;
    if !path.is_file() {
        return Err(EngineError::InvalidParams(format!("{key} must be a file")));
    }
    Ok(path)
}

pub fn discover(s: &mut Session, _: &Value) -> Result<Value> {
    let tool = resolve_craft_cli("effectcraft-cli");
    let output = run_tool_in(
        &workspace_root(s),
        &tool,
        &["--empty".into(), "info".into(), "--json".into()],
    )?;
    let info: Value = serde_json::from_str(&output)
        .map_err(|e| EngineError::ExternalTool(tool.clone(), e.to_string()))?;
    Ok(
        json!({"backend":"effectcraft","available":true,"executable":tool,
              "version":info["version"],"commands":info["commands"],"effects":info["effects"],
              "source_format":"ecproj","template_format":"ectemplate"}),
    )
}

pub fn build(s: &mut Session, p: &Value) -> Result<Value> {
    crate::motion_authoring::project_guard(s, p)?;
    let spec = path(s, p, "spec")?;
    let output = p["out_dir"]
        .as_str()
        .filter(|v| !v.is_empty())
        .map(|v| resolve_workspace_path(s, v))
        .ok_or_else(|| {
            EngineError::InvalidParams("out_dir is required; choose a new output directory".into())
        })?;
    run_python_json(
        s,
        "pipeline.effectcraft.scene",
        &[
            "build".into(),
            spec.to_string_lossy().into_owned(),
            "--output".into(),
            output.to_string_lossy().into_owned(),
            "--tool".into(),
            resolve_craft_cli("effectcraft-cli"),
        ],
    )
}

pub fn inspect(s: &mut Session, p: &Value) -> Result<Value> {
    let source = path(s, p, "source")?;
    run_python_json(
        s,
        "pipeline.effectcraft.scene",
        &[
            "inspect".into(),
            source.to_string_lossy().into_owned(),
            "--tool".into(),
            resolve_craft_cli("effectcraft-cli"),
        ],
    )
}

pub fn start(s: &mut Session, p: &Value) -> Result<Value> {
    let source = path(s, p, "source")?;
    if source.extension().and_then(|v| v.to_str()) != Some("ecproj") {
        return Err(EngineError::InvalidParams(
            "source must be a saved .ecproj".into(),
        ));
    }
    let mut request = p.clone();
    request["source"] = json!(source);
    request["executable"] = json!(resolve_craft_cli("effectcraft-cli"));
    request["ffmpeg"] = json!("/usr/bin/ffmpeg");
    crate::blender::submit(s, p, request, "effectcraft")
}
