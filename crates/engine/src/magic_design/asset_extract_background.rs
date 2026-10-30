use super::paths::results_dir;
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};

/// Split an image into its subject and a filled background plate with two headless
/// `photocraft-cli run`s: select the subject (`rect`/`point` → Object Selection, else Select
/// Subject), then (1) clear everything else → `<stem>_subject.png`, (2) grow the selection by `expand`
/// px and Content-Aware Fill it → `<stem>_background.png`. Both land beside the source (see `results_dir`).
pub fn cmd_asset_extract_background(s: &mut Session, params: &Value) -> Result<Value> {
    let id = params
        .get("asset")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| s.active_asset.clone())
        .ok_or_else(|| EngineError::InvalidParams("no active asset; pass asset=<id>".into()))?;
    let source = s
        .project_assets
        .iter()
        .find(|a| a.id == id)
        .and_then(|a| a.path.clone())
        .or_else(|| s.assets.get(&id).map(|a| a.path.clone()))
        .ok_or_else(|| EngineError::InvalidParams(format!("asset '{id}' has no image path")))?;
    let select = match (params.get("rect"), params.get("point")) {
        (Some(rect), _) => json!({"rect": rect}),
        (None, Some(point)) => json!({"point": point}),
        (None, None) => Value::Null,
    };
    let (select_cmd, select_params) = if select.is_null() {
        ("select.subject", "{}".to_string())
    } else {
        ("select.object", select.to_string())
    };
    let expand = params
        .get("expand")
        .and_then(Value::as_u64)
        .unwrap_or(6)
        .clamp(1, 500);
    let root = crate::workspace::workspace_root(s);
    let out_dir = results_dir(s, &source, &root, &id);
    std::fs::create_dir_all(&out_dir)?;
    let stem = source
        .file_stem()
        .and_then(|st| st.to_str())
        .unwrap_or("image");
    let (subject, background) = (
        out_dir.join(format!("{stem}_subject.png")),
        out_dir.join(format!("{stem}_background.png")),
    );
    let bin = crate::workspace::resolve_craft_cli("photocraft-cli");
    let tool_version = crate::workspace::run_tool_in(&root, &bin, &["--version".into()])?;
    let cut = crate::workspace::run_tool_in(
        &root,
        &bin,
        &[
            "run".into(),
            source.to_string_lossy().into_owned(),
            "--cmd".into(),
            "layer.new.layerFromBackground".into(),
            "--cmd".into(),
            select_cmd.into(),
            "--params".into(),
            select_params.clone(),
            "--cmd".into(),
            "select.inverse".into(),
            "--cmd".into(),
            "edit.clear".into(),
            "--out".into(),
            subject.to_string_lossy().into_owned(),
        ],
    )?;
    let fill = crate::workspace::run_tool_in(
        &root,
        &bin,
        &[
            "run".into(),
            source.to_string_lossy().into_owned(),
            "--cmd".into(),
            select_cmd.into(),
            "--params".into(),
            select_params,
            "--cmd".into(),
            "select.modify.expand".into(),
            "--params".into(),
            json!({"radius": expand}).to_string(),
            "--cmd".into(),
            "edit.contentAwareFill".into(),
            "--params".into(),
            "{\"sampling\":\"auto\",\"output\":\"current\"}".into(),
            "--out".into(),
            background.to_string_lossy().into_owned(),
        ],
    )?;
    let selection = command_result(&cut, select_cmd);
    let filled =
        command_result(&fill, "edit.contentAwareFill").map(|value| value["filled"].clone());
    Ok(
        json!({"ok": true, "asset": id, "source": source, "subject": subject, "background": background,
        "tool": {"executable": bin, "version": tool_version},
        "operations": [select_cmd, "select.inverse", "edit.clear", "select.modify.expand", "edit.contentAwareFill"],
        "selection": {"command": select_cmd, "params": select, "result": selection}, "filled_pixels": filled,
        "next": [
            "Look at the subject on a solid colour and the background at full size before using them.",
            "Thin or see-through parts (wings, whiskers, antennae) may be missed: if they stay painted on the background, record it with assets.fixlist_add.",
            "Wrong object? Re-run with rect=[x,y,w,h] around the subject (or point=[x,y])."
        ]}),
    )
}

fn command_result(output: &str, command: &str) -> Option<Value> {
    output
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|value| value["command"] == command)
        .map(|value| value["result"].clone())
}
