//! `character.cut`: PhotoCraft cuts each marked part onto its own layer in place, the way a
//! designer prepares artwork for a puppet rig: Lasso the part, Layer via Copy, then
//! Content-Aware Fill the body underneath so a turning limb never shows a hole.

use serde_json::{Value, json};

use super::{View, bad, part_list, root_part};
use crate::{Result, Session};

/// Pixels the fill reaches past each outline, so no edge of the original part survives.
const FILL_MARGIN: u32 = 6;

fn cmd(args: &mut Vec<(&'static str, Value)>, id: &'static str, params: Option<Value>) {
    args.push((id, params.unwrap_or_else(|| json!({}))));
}

/// Front-most part first, so each fill only repairs the layers still underneath it.
fn cut_part(args: &mut Vec<(&'static str, Value)>, part: &Value, root: &str) -> Result<()> {
    let id = part["id"].as_str().ok_or_else(|| bad("a part has no id"))?;
    let outline = part["outline"]
        .as_array()
        .filter(|o| o.len() >= 3)
        .ok_or_else(|| bad(format!("part {id} needs an outline of 3 or more points")))?;
    let lasso = json!({"points": outline, "antiAlias": true});
    cmd(args, "layer.select", Some(json!({"name": root})));
    cmd(args, "select.lasso", Some(lasso.clone()));
    cmd(args, "layer.new.layerViaCopy", None);
    cmd(args, "layer.renameLayer", Some(json!({"name": id})));
    cmd(args, "layer.select", Some(json!({"name": root})));
    cmd(args, "select.lasso", Some(lasso));
    cmd(
        args,
        "select.modify.expand",
        Some(json!({"radius": FILL_MARGIN})),
    );
    cmd(
        args,
        "edit.contentAwareFill",
        Some(json!({"sampling": "auto"})),
    );
    cmd(args, "select.deselect", None);
    Ok(())
}

fn photocraft_cli() -> Result<std::path::PathBuf> {
    crate::workspace::find_executable("photocraft-cli").map_err(|e| crate::EngineError::ExternalTool("photocraft-cli".into(), e.to_string()))
}

pub(crate) fn photocraft_run(
    input: &std::path::Path,
    steps: &[(&str, Value)],
    output: Option<&std::path::Path>,
) -> Result<()> {
    // Via installed photocraft-cli (system app pattern). Steps are --cmd/--params pairs.
    let cli = photocraft_cli()?;
    let mut cmd = std::process::Command::new(&cli);
    cmd.arg("run").arg(input);
    for (id, params) in steps {
        cmd.arg("--cmd").arg(id).arg("--params").arg(params.to_string());
    }
    if let Some(out) = output {
        if out == input {
            return Err(crate::EngineError::InvalidParams(format!("refusing to overwrite input {}", input.display())));
        }
        if let Some(parent) = out.parent() { std::fs::create_dir_all(parent)?; }
        cmd.arg("--out").arg(out);
    }
    let out = cmd.output().map_err(|e| crate::EngineError::ExternalTool("photocraft-cli".into(), e.to_string()))?;
    if !out.status.success() {
        return Err(crate::EngineError::ExternalTool("photocraft-cli".into(), format!("exit {:?}: {}", out.status.code(), String::from_utf8_lossy(&out.stderr))));
    }
    Ok(())
}

/// Each layer's bounds `[x, y, w, h]` from the saved PSD, by layer name.
fn layer_bounds(psd: &std::path::Path) -> Result<serde_json::Map<String, Value>> {
    let cli = photocraft_cli()?;
    let out = std::process::Command::new(&cli).arg("info").arg(psd).output().map_err(|e| crate::EngineError::ExternalTool("photocraft-cli".into(), e.to_string()))?;
    let doc: Value = serde_json::from_slice(&out.stdout).map_err(|e| crate::EngineError::ExternalTool("photocraft-cli".into(), e.to_string()))?;
    let mut out = serde_json::Map::new();
    for layer in doc["layers"].as_array().into_iter().flatten() {
        if let (Some(name), Some(bounds)) = (layer["name"].as_str(), layer.get("bounds")) {
            out.insert(name.into(), bounds.clone());
        }
    }
    Ok(out)
}

/// Each layer as a trimmed PNG (PhotoCraft Layers to Files), for Studio's layer list.
fn export_layers(view: &View, psd: &std::path::Path) -> Result<serde_json::Map<String, Value>> {
    let dir = view.dir.join("layers");
    if dir.is_dir() {
        std::fs::remove_dir_all(&dir)?;
    }
    std::fs::create_dir_all(&dir)?;
    let mut args = Vec::new();
    let params = json!({"dir": dir, "format": "png", "prefix": "", "trim": true});
    cmd(&mut args, "file.export.layersToFiles", Some(params));
    photocraft_run(psd, &args, None)?;
    let mut out = serde_json::Map::new();
    for entry in std::fs::read_dir(&dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // PhotoCraft names each file layer_<index>_<layer name>.png.
        if let Some(id) = name
            .strip_suffix(".png")
            .and_then(|n| n.splitn(3, '_').nth(2))
        {
            out.insert(id.to_string(), json!(entry.path()));
        }
    }
    Ok(out)
}

pub fn cmd_character_cut(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let mut parts = view.parts()?;
    let list = part_list(&parts)?.clone();
    let root = root_part(&list)?.to_string();
    let image = parts["image"]
        .as_str()
        .map(|i| view.dir.join(i))
        .filter(|i| i.is_file())
        .ok_or_else(|| bad("parts.json image is missing"))?;
    let psd = view.file("psd");
    let pcraft = view.file("pcraft");
    let mut args = Vec::new();
    cmd(&mut args, "layer.renameLayer", Some(json!({"name": root})));
    for part in list
        .iter()
        .rev()
        .filter(|p| p["id"].as_str() != Some(&root))
    {
        cut_part(&mut args, part, &root)?;
    }
    cmd(
        &mut args,
        "file.saveACopy",
        Some(json!({"path": psd, "overwrite": true})),
    );
    photocraft_run(&image, &args, Some(&pcraft))?;
    let bounds = layer_bounds(&psd)?;
    let files = export_layers(&view, &psd)?;
    for part in parts["parts"].as_array_mut().into_iter().flatten() {
        let id = part["id"].as_str().unwrap_or_default().to_string();
        part["bounds"] = bounds.get(&id).cloned().unwrap_or(Value::Null);
        part["file"] = files.get(&id).cloned().unwrap_or(Value::Null);
    }
    view.write_parts(&parts)?;
    Ok(
        json!({"ok": true, "psd": psd, "pcraft": pcraft, "layers": bounds,
        "next": format!("character.rig character={} view={}", view.id, view.view)}),
    )
}
