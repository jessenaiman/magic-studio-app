use crate::{EngineError, Result, Session};
use serde_json::{Value, json};

/// Matte one reference through PhotoCraft, independently of intake and sheet classification.
pub fn cmd_asset_make_transparent(s: &mut Session, p: &Value) -> Result<Value> {
    let id = p["asset"]
        .as_str()
        .or(s.active_asset.as_deref())
        .ok_or_else(|| {
            EngineError::InvalidParams("Choose an asset to remove its background".into())
        })?
        .to_owned();
    let source =
        match p["source"].as_str() {
            Some(path) => crate::image_cmds::medium::project_reference(s, path)?,
            None => s.assets.get(&id).map(|a| a.path.clone()).ok_or_else(|| {
                EngineError::InvalidParams("Load the selected asset first".into())
            })?,
        };
    let mut result = matte_image(s, &source, p)?;
    if p["attach"].as_bool().unwrap_or(true) {
        let asset = s.assets.get_mut(&id).ok_or_else(|| {
            EngineError::InvalidParams("Selected asset is no longer loaded".into())
        })?;
        asset.raw_original_path.get_or_insert_with(|| {
            result["source_image"]
                .as_str()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| source.clone())
        });
        asset.path = std::path::PathBuf::from(result["image"].as_str().unwrap_or_default());
        asset.width = result["width"].as_u64().unwrap_or(0) as u32;
        asset.height = result["height"].as_u64().unwrap_or(0) as u32;
        asset.has_alpha = true;
        asset.original_inspection = None;
        if let Some(registered) = s.project_assets.iter_mut().find(|a| a.id == id) {
            registered.path = Some(asset.path.clone());
        }
    }
    result["asset_id"] = json!(id);
    Ok(result)
}

pub(crate) fn matte_image(s: &Session, source: &std::path::Path, p: &Value) -> Result<Value> {
    if p["reuse_alpha"].as_bool() == Some(true) {
        let pixels = image::open(source)?.into_rgba8();
        if !pixels.pixels().any(|p| p[3] < 255) || !pixels.pixels().any(|p| p[3] > 0) {
            return Err(EngineError::InvalidParams(
                "The prepared reference needs visible pixels and transparency".into(),
            ));
        }
        let previous: Value = std::fs::read(source.with_extension("matte.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(Value::Null);
        return Ok(
            json!({"ok":true,"image":source,"source_image":previous["source_image"],
            "width":pixels.width(),"height":pixels.height(),"has_alpha":true,"reused_alpha":true}),
        );
    }
    let root = crate::workshop::project_dir(s)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| EngineError::InvalidParams(e.to_string()))?
        .as_nanos();
    let directory = root.join("assets/reference-previews");
    std::fs::create_dir_all(&directory)?;
    let out = directory.join(format!("matte-{stamp}.png"));
    let operation = remove_background(s, source, &out, p)?;
    let pixels = image::open(&out)?.into_rgba8();
    let visible = pixels.pixels().filter(|p| p[3] > 0).count();
    let transparent = pixels.pixels().filter(|p| p[3] < 255).count();
    if visible == 0 || transparent == 0 {
        return Err(EngineError::InvalidParams(
            "Matting returned no usable transparent subject; review the source and selection"
                .into(),
        ));
    }
    let (width, height) = pixels.dimensions();
    let result = json!({"ok":true,"command":"asset.make_transparent","source_image":source,
        "image":out,"width":width,"height":height,"has_alpha":true,"visible_pixels":visible,
        "transparent_pixels":transparent,"needs_review":true,"operation":operation,
        "selection":p,"next":"Review antennae, fine edges and feet before using this reference"});
    std::fs::write(out.with_extension("matte.json"), result.to_string())?;
    Ok(result)
}

fn photocraft_background_args(source: &std::path::Path, out: &std::path::Path, p: &Value) -> Vec<String> {
    let mut args = vec!["run".into(), source.to_string_lossy().into_owned()];
    let keep = p["keep_polygons"].as_array();
    if p.get("rect").is_some() || keep.is_some() {
        let has_rect = p.get("rect").is_some();
        args.extend([
            "--cmd".into(),
            "layer.new.layerFromBackground".into(),
            "--cmd".into(),
            if has_rect { "select.object" } else { "select.subject" }.into(),
            "--params".into(),
            if has_rect { json!({"rect":p["rect"]}).to_string() } else { "{}".into() },
        ]);
        for polygon in keep.into_iter().flatten() {
            args.extend([
                "--cmd".into(),
                "select.lasso".into(),
                "--params".into(),
                json!({"points":polygon,"mode":"add"}).to_string(),
            ]);
        }
        args.extend([
            "--cmd".into(),
            "select.inverse".into(),
            "--cmd".into(),
            "edit.clear".into(),
        ]);
    } else {
        args.extend([
            "--cmd".into(),
            "layer.removeBackground".into(),
            "--params".into(),
            "{\"refine\":true}".into(),
        ]);
    }
    args.extend(["--out".into(), out.to_string_lossy().into_owned()]);
    args
}

/// Shared import operation. Backend and model are selectable; tool versions are discovered.
pub(crate) fn remove_background(
    s: &Session,
    source: &std::path::Path,
    out: &std::path::Path,
    p: &Value,
) -> Result<Value> {
    let installed = std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join("rembg").is_file()));
    let backend = p["backend"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| std::env::var("MAGIC_MATTING_BACKEND").ok())
        .unwrap_or_else(|| "auto".into());
    let manual = p.get("rect").is_some() || p.get("keep_polygons").is_some();
    if backend == "rembg" || (backend == "auto" && installed && !manual) {
        let model = p["model"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| std::env::var("MAGIC_MATTING_MODEL").ok())
            .unwrap_or_else(|| "birefnet-general-lite".into());
        let args = vec![
            "i".into(),
            "-m".into(),
            model.clone(),
            "-dc".into(),
            source.to_string_lossy().into_owned(),
            out.to_string_lossy().into_owned(),
        ];
        let output =
            crate::workspace::run_tool_in(&crate::workspace::workspace_root(s), "rembg", &args)?;
        return Ok(json!({"tool":"rembg","model":model,"decontaminate":true,"tool_output":output}));
    }
    if backend != "auto" && backend != "photocraft" {
        return Err(EngineError::InvalidParams(
            "backend must be auto, rembg, or photocraft".into(),
        ));
    }
    let args = photocraft_background_args(source, out, p);
    let tool_output = crate::workspace::run_tool_in(
        &crate::workspace::workspace_root(s),
        "photocraft-cli",
        &args,
    )?;
    Ok(json!({"tool":"photocraft-cli","tool_output":tool_output}))
}
