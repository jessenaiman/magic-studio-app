//! `character.cut`: PhotoCraft cuts each marked part onto its own layer in place, the way a
//! designer prepares artwork for a puppet rig: Lasso the part, Layer via Copy, then
//! Content-Aware Fill the body underneath so a turning limb never shows a hole.

use serde_json::{Value, json};

use super::{View, bad, part_list, path_arg, root_part};
use crate::{Result, Session};

/// Pixels the fill reaches past each outline, so no edge of the original part survives.
const FILL_MARGIN: u32 = 6;

fn cmd(args: &mut Vec<String>, id: &str, params: Option<Value>) {
    args.extend(["--cmd".into(), id.into()]);
    if let Some(params) = params {
        args.extend(["--params".into(), params.to_string()]);
    }
}

/// Front-most part first, so each fill only repairs the layers still underneath it.
fn cut_part(args: &mut Vec<String>, part: &Value, root: &str) -> Result<()> {
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

/// Each layer's bounds `[x, y, w, h]` from the saved PSD, by layer name.
fn layer_bounds(view: &View, psd: &std::path::Path) -> Result<serde_json::Map<String, Value>> {
    let info = crate::workspace::run_tool_in(
        &view.dir,
        "photocraft-cli",
        &["info".into(), path_arg(psd)],
    )?;
    let doc: Value =
        serde_json::from_str(&info).map_err(|e| bad(format!("photocraft info: {e}")))?;
    let mut out = serde_json::Map::new();
    for layer in doc["layers"].as_array().into_iter().flatten() {
        if let (Some(name), Some(bounds)) = (layer["name"].as_str(), layer.get("bounds")) {
            out.insert(name.into(), bounds.clone());
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
    let mut args = vec!["run".into(), path_arg(&image)];
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
    args.extend(["--out".into(), path_arg(&pcraft)]);
    crate::workspace::run_tool_in(&view.dir, "photocraft-cli", &args)?;
    let bounds = layer_bounds(&view, &psd)?;
    for part in parts["parts"].as_array_mut().into_iter().flatten() {
        let id = part["id"].as_str().unwrap_or_default().to_string();
        part["bounds"] = bounds.get(&id).cloned().unwrap_or(Value::Null);
    }
    view.write_parts(&parts)?;
    Ok(
        json!({"ok": true, "psd": psd, "pcraft": pcraft, "layers": bounds,
        "next": format!("character.rig character={} view={}", view.id, view.view)}),
    )
}
