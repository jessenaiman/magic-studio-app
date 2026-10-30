use crate::session::LoadedManifest;
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Slice a reviewed source sheet and attach its manifest.
pub fn cmd_asset_cut_sheet(s: &mut Session, params: &Value) -> Result<Value> {
    let asset_id = params
        .get("asset")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| s.active_asset.clone())
        .ok_or_else(|| EngineError::InvalidParams("no active asset to cut".into()))?;

    let source_path = reviewed_source(s, params, &asset_id)?;

    let root = crate::workspace::workspace_root(s);
    let out_dir = if let Some(dir_str) = params.get("out_dir").and_then(Value::as_str) {
        crate::workspace::resolve_workspace_path(s, dir_str)
    } else {
        root.join("exports/pose_and_parts")
            .join(format!("{asset_id}_cut"))
    };

    let tolerance = params
        .get("tolerance")
        .and_then(Value::as_u64)
        .map(|v| v.clamp(1, 100) as u32)
        .unwrap_or(25);
    let use_ocr = params
        .get("use_ocr")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let effective_source = preprocess_sheet(params, &source_path, &out_dir, &root)?;
    let mut cut_params = params.clone();
    if cut_params.get("layout").is_none() {
        if let Some(layout) = crate::sheets::layout::for_sheet(&source_path)? {
            let source_size = image::image_dimensions(&source_path)?;
            if image::image_dimensions(&effective_source)? != source_size {
                return Err(EngineError::InvalidParams("Preprocessing changed the sheet canvas; its saved panel layout no longer applies".into()));
            }
            std::fs::create_dir_all(&out_dir)?;
            let file = out_dir.join("template.layout.json");
            std::fs::write(&file, layout.to_string())?;
            cut_params["layout"] = json!(file);
            cut_params["kind"] = layout["sheetType"].clone();
        }
    }
    let cut_args = cut_arguments(
        s,
        &cut_params,
        &effective_source,
        &out_dir,
        cut_params
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("auto"),
        tolerance,
        use_ocr,
    )?;
    let pipeline_out =
        crate::workspace::run_python_json(s, "pipeline.pose_and_part_cutter", &cut_args)?;

    let manifest_path = pipeline_out
        .get("manifest")
        .and_then(Value::as_str)
        .map(|p| crate::workspace::resolve_workspace_path(s, p))
        .unwrap_or_else(|| out_dir.join("manifest.json"));

    let loaded_manifest = LoadedManifest::load_from_path(&manifest_path)?;
    let mut response = sheet_summary(&loaded_manifest, &manifest_path);
    response["asset_id"] = json!(asset_id);
    response["tolerance"] = json!(tolerance);
    response["use_ocr"] = json!(use_ocr);
    response["pipeline"] = pipeline_out;
    if let Some(asset_mut) = s.assets.get_mut(&asset_id) {
        if loaded_manifest.sheet_type == "turnaround" {
            asset_mut.turnaround_manifest = Some(loaded_manifest);
        } else {
            asset_mut.pose_manifest = Some(loaded_manifest);
        }
        asset_mut.sync_parts_from_manifests();
    }
    s.active_asset = Some(asset_id.clone());

    Ok(response)
}

fn sheet_summary(loaded_manifest: &LoadedManifest, manifest_path: &Path) -> Value {
    let manifest_json = std::fs::read_to_string(manifest_path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());
    json!({
        "ok": true,
        "command": "asset.cut_sheet",
        "manifest": manifest_path.to_string_lossy(),
        "sheet_type": loaded_manifest.sheet_type,
        "status": loaded_manifest.status,
        "needs_review": loaded_manifest.status != "ready",
        "warnings": manifest_json.as_ref().and_then(|value| value.get("warnings").cloned()).unwrap_or_else(|| json!([])),
        "review": manifest_json.as_ref().and_then(|value| value.get("review").cloned()).unwrap_or(Value::Null),
        "frame_count": loaded_manifest.frames.len(),
        "frame_labels": loaded_manifest.frames.iter().map(|f| f.label.clone()).collect::<Vec<_>>(),
        "part_names": loaded_manifest.part_names,
        "animation":manifest_json.as_ref().and_then(|value|value.get("animation").cloned()),
    })
}

fn preprocess_sheet(
    params: &Value,
    source_path: &Path,
    out_dir: &Path,
    root: &Path,
) -> Result<PathBuf> {
    // Optional explicit PhotoCraft pre-pass using verified `photocraft-cli run <in> --cmd <id> --out <out>` syntax.
    let mut effective_source = source_path.to_path_buf();
    if let Some(pc_cmd) = params.get("photocraft_cmd").and_then(Value::as_str) {
        std::fs::create_dir_all(out_dir)?;
        let prepass_out = out_dir.join("photocraft_prepass.png");
        let pc_params = params.get("photocraft_params").and_then(Value::as_str);
        let pc_args = crate::workspace::build_photocraft_run_args(
            &source_path.to_string_lossy(),
            pc_cmd,
            pc_params,
            &prepass_out.to_string_lossy(),
        );
        crate::workspace::run_tool_in(root, "photocraft-cli", &pc_args)?;
        if prepass_out.is_file() {
            effective_source = prepass_out;
        }
    }

    Ok(effective_source)
}

fn cut_arguments(
    s: &Session,
    params: &Value,
    effective_source: &Path,
    out_dir: &Path,
    kind: &str,
    tolerance: u32,
    use_ocr: bool,
) -> Result<Vec<String>> {
    let mut cut_args = vec![
        effective_source.to_string_lossy().into_owned(),
        "--out-dir".to_string(),
        out_dir.to_string_lossy().into_owned(),
        "--kind".to_string(),
        kind.to_string(),
        "--tolerance".to_string(),
        tolerance.to_string(),
    ];
    if !use_ocr {
        cut_args.push("--no-ocr".to_string());
    }
    // Bounded correction path: forward an explicit layout file to the existing
    // cutter when automatic detection is incomplete. A missing file is reported
    // before any cut runs, so a failed correction never replaces usable output.
    if let Some(layout) = params.get("layout").and_then(Value::as_str) {
        let layout_path = crate::workspace::resolve_workspace_path(s, layout);
        if !layout_path.is_file() {
            return Err(EngineError::InvalidParams(format!(
                "layout file does not exist: {}",
                layout_path.display()
            )));
        }
        cut_args.push("--layout".to_string());
        cut_args.push(layout_path.to_string_lossy().into_owned());
    }
    Ok(cut_args)
}

fn reviewed_source(s: &Session, params: &Value, asset_id: &str) -> Result<PathBuf> {
    let existing_asset = s.assets.get(asset_id).ok_or_else(|| {
        EngineError::InvalidParams(format!("asset `{asset_id}` is not loaded in session"))
    })?;

    let source_path = if let Some(src_override) = params.get("source").and_then(Value::as_str) {
        crate::workspace::resolve_workspace_path(s, src_override)
    } else {
        existing_asset.path.clone()
    };

    if !source_path.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "source image for asset `{asset_id}` does not exist: {}",
            source_path.display()
        )));
    }

    crate::sheets::require_sheet_review(s, asset_id, &source_path)?;

    Ok(source_path)
}
