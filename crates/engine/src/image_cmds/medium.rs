//! Reference-medium previews through real PhotoCraft filters; originals stay untouched.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};

pub(crate) fn catalog() -> Value {
    json!([
        {"id":"cutout","label":"Cutout · flat colours","command":"filter.gallery.cutout","settings":{"numberOfLevels":5,"edgeSimplicity":6,"edgeFidelity":3},"controls":[
            {"key":"numberOfLevels","label":"Colour levels","min":2,"max":8,"value":5},
            {"key":"edgeSimplicity","label":"Edge simplicity","min":0,"max":10,"value":6},
            {"key":"edgeFidelity","label":"Edge fidelity","min":1,"max":3,"value":3}]},
        {"id":"poster","label":"Poster edges","command":"filter.gallery.posterEdges","settings":{"edgeThickness":2,"edgeIntensity":1,"posterization":2}},
        {"id":"watercolor","label":"Watercolour","command":"filter.gallery.watercolor","settings":{"brushDetail":9,"shadowIntensity":1,"texture":1}},
        {"id":"pencil","label":"Coloured pencil","command":"filter.gallery.coloredPencil","settings":{"pencilWidth":4,"strokePressure":8,"paperBrightness":25}},
        {"id":"dry_brush","label":"Dry brush","command":"filter.gallery.dryBrush","settings":{"brushSize":2,"brushDetail":8,"texture":1}},
        {"id":"oil","label":"Oil paint","command":"filter.stylize.oilPaint","settings":{"stylization":4,"cleanliness":5,"lighting":false}}
    ])
}

fn resolve_medium_settings(p: &Value, medium_id: &str) -> Result<(String, Value)> {
    let invalid = |message: &str| EngineError::InvalidParams(message.into());
    let mediums = catalog();
    let medium = mediums
        .as_array()
        .and_then(|items| items.iter().find(|m| m["id"] == medium_id))
        .ok_or_else(|| invalid("Choose a medium from assets.sheet_plan mediums"))?;
    let mut settings = medium["settings"].clone();
    if let Some(overrides) = p.get("settings") {
        for (key, value) in overrides
            .as_object()
            .ok_or_else(|| invalid("settings must be an object of PhotoCraft filter parameters"))?
        {
            settings[key] = value.clone();
        }
    }
    Ok((
        medium["command"].as_str().unwrap_or_default().to_owned(),
        settings,
    ))
}

pub fn cmd_image_apply_medium(s: &mut Session, p: &Value) -> Result<Value> {
    let invalid = |message: &str| EngineError::InvalidParams(message.into());
    let source = crate::workspace::resolve_workspace_path(
        s,
        p["source"]
            .as_str()
            .ok_or_else(|| invalid("source image is required"))?,
    )
    .canonicalize()?;
    image::image_dimensions(&source)?;
    let medium_id = p["medium"].as_str().unwrap_or("cutout");
    let (command, settings) = resolve_medium_settings(p, medium_id)?;
    let root = crate::workshop::project_dir(s)?.canonicalize()?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| invalid(&e.to_string()))?
        .as_nanos();
    let out_dir = root.join("assets/reference-previews");
    std::fs::create_dir_all(&out_dir)?;
    let out = out_dir.join(format!("{medium_id}-{stamp}.png"));
    let mut args = vec!["run".into(), source.to_string_lossy().into_owned()];
    args.extend(["--cmd".into(), "layer.layerMask.fromTransparency".into()]);
    if medium_id == "cutout" {
        args.extend([
            "--cmd".into(),
            "filter.blur.surfaceBlur".into(),
            "--params".into(),
            json!({"radius":4,"threshold":40}).to_string(),
        ]);
    }
    args.extend([
        "--cmd".into(),
        command,
        "--params".into(),
        settings.to_string(),
        "--out".into(),
        out.to_string_lossy().into_owned(),
    ]);
    let result = crate::workspace::run_tool_in(
        &crate::workspace::workspace_root(s),
        "photocraft-cli",
        &args,
    )?;
    let (width, height) = image::image_dimensions(&out)?;
    let receipt = json!({"ok":true,"source":source,"image":out,"medium":medium_id,"settings":settings,"width":width,"height":height,"tool":"photocraft-cli","tool_output":result});
    std::fs::write(out.with_extension("medium.json"), receipt.to_string())?;
    Ok(receipt)
}

pub(crate) fn project_reference(s: &Session, path: &str) -> Result<std::path::PathBuf> {
    let file = crate::workspace::resolve_workspace_path(s, path).canonicalize()?;
    let root = crate::workshop::project_dir(s)?.canonicalize()?;
    let registered = s
        .project_assets
        .iter()
        .filter_map(|a| a.path.as_ref())
        .filter_map(|p| p.canonicalize().ok())
        .any(|p| p == file);
    if !file.starts_with(&root) && !registered {
        return Err(EngineError::InvalidParams(
            "The chosen sheet reference must belong to the project or its registered artwork"
                .into(),
        ));
    }
    image::image_dimensions(&file)?;
    Ok(file)
}
