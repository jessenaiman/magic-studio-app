use super::{paths::results_dir, sheet_extraction::cmd_asset_cut_sheet};
use crate::session::{Asset, AttachedReference};
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

struct ImportedImage {
    source: PathBuf,
    transparent: PathBuf,
    directory: PathBuf,
    width: u32,
    height: u32,
    matting: Value,
}

struct SheetClassification {
    valid: bool,
    status: String,
    slot: &'static str,
}

pub fn cmd_intake_validate(s: &mut Session, params: &Value) -> Result<Value> {
    let image_raw = params
        .get("image")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::InvalidParams("missing 'image' parameter".into()))?;
    let resolved_path = crate::workspace::resolve_workspace_path(s, image_raw);
    if !resolved_path.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "image file does not exist: {}",
            resolved_path.display()
        )));
    }
    let attach_to_active = params
        .get("attach_to_active")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let asset_id = intake_asset_id(s, params, attach_to_active, &resolved_path);
    let image = ImportedImage::prepare(s, &resolved_path, &asset_id)?;
    let validation = crate::workspace::run_python_json(
        s,
        "pipeline.import_validator",
        &[image.transparent.to_string_lossy().into_owned()],
    )?;
    let slot = params.get("slot").and_then(Value::as_str).unwrap_or("auto");
    let classification = SheetClassification::from_validation(&validation, slot, attach_to_active);
    let mut asset = s.assets.remove(&asset_id).unwrap_or_else(|| Asset {
        id: asset_id.clone(),
        ..Asset::default()
    });
    image.attach_to(&mut asset, &classification, &validation);
    s.assets.insert(asset_id.clone(), asset);
    s.active_asset = Some(asset_id.clone());
    let auto_cut = image.cut_sheet(s, params, &asset_id, classification.slot)?;
    Ok(json!({
        "ok": true, "command": "intake.validate", "asset_id": asset_id,
        "slot": classification.slot, "matting_tool": image.matting["tool"], "matting":image.matting,
        "image": image.transparent.to_string_lossy(), "source_image": image.source.to_string_lossy(),
        "valid_turnaround": classification.valid, "status": classification.status,
        "width": image.width, "height": image.height, "has_alpha": true,
        "sheet_kind": classification.status, "validation": validation,
        "jev_comparison": null, "auto_cut": auto_cut,
    }))
}

fn intake_asset_id(s: &Session, params: &Value, attach_to_active: bool, source: &Path) -> String {
    params
        .get("asset")
        .or_else(|| params.get("name"))
        .and_then(Value::as_str)
        .and_then(crate::workspace::try_sanitize_asset_id)
        .or_else(|| {
            if attach_to_active {
                s.active_asset.clone()
            } else {
                None
            }
        })
        .unwrap_or_else(|| crate::workspace::asset_id_for_import(None, source))
}

/// Collect intake choices using the owning pipeline module and store its recommendation.
pub fn cmd_intake_three_question(s: &mut Session, params: &Value) -> Result<Value> {
    let asset = s
        .current_asset()
        .ok_or_else(|| EngineError::InvalidParams("no active asset in session".into()))?;
    let asset_id = asset.id.clone();
    let image = match params.get("image") {
        Some(Value::String(path)) if !path.trim().is_empty() => {
            crate::workspace::resolve_workspace_path(s, path)
        }
        Some(_) => {
            return Err(EngineError::InvalidParams(
                "'image' must be a nonempty path string".into(),
            ));
        }
        None => asset.path.clone(),
    };
    if !image.is_file() {
        return Err(EngineError::InvalidParams(format!(
            "image file does not exist: {}",
            image.display()
        )));
    }
    let mut args = vec![image.to_string_lossy().into_owned()];
    append_intake_answers(&mut args, params)?;
    let pipeline = crate::workspace::run_python_json(s, "pipeline.intake_router", &args)?;
    let recommendation = pipeline
        .get("recommended")
        .filter(|value| value.is_object() && value.as_object().is_some_and(|o| !o.is_empty()))
        .cloned()
        .ok_or_else(|| {
            EngineError::InvalidParams("intake returned no recommendation object".into())
        })?;
    let asset = s
        .current_asset_mut()
        .ok_or_else(|| EngineError::InvalidParams("active asset no longer available".into()))?;
    asset.intake_recommendation = Some(recommendation.clone());
    Ok(
        json!({"ok":true, "command":"intake.three_question", "asset_id":asset_id,
        "image":image, "recommendation":recommendation, "pipeline":pipeline}),
    )
}

fn append_intake_answers(args: &mut Vec<String>, params: &Value) -> Result<()> {
    for key in ["q1", "q2", "q3", "archetype", "verb"] {
        if let Some(value) = params.get(key) {
            let text = value
                .as_str()
                .filter(|text| !text.trim().is_empty())
                .ok_or_else(|| {
                    EngineError::InvalidParams(format!("'{key}' must be a nonempty string"))
                })?;
            args.extend([format!("--{key}"), text.to_string()]);
        }
    }
    Ok(())
}

impl ImportedImage {
    fn prepare(s: &Session, resolved_path: &Path, asset_id: &str) -> Result<Self> {
        // STEP 1 OF IMAGE IMPORT: photocraft-cli `layer.removeBackground` strips the background (including enclosed gaps) to a transparent RGBA PNG.
        let root = crate::workspace::workspace_root(s);
        let import_dir = results_dir(s, resolved_path, &root, asset_id);
        std::fs::create_dir_all(&import_dir)?;
        let stem_clean = resolved_path
            .file_stem()
            .and_then(|st| st.to_str())
            .map(crate::workspace::sanitize_asset_id)
            .unwrap_or_else(|| "sheet".to_string());
        let transparent_path = import_dir.join(format!("{stem_clean}_transparent.png"));

        // Background removal is PhotoCraft's job; if it cannot run, the import fails rather than
        // continuing with a hand-rolled or un-matted image.
        let (width, height) = image::image_dimensions(resolved_path).map_err(|e| {
            EngineError::InvalidParams(format!(
                "cannot read image {}: {e}",
                resolved_path.display()
            ))
        })?;
        let matting = super::asset_make_transparent::remove_background(
            s,
            resolved_path,
            &transparent_path,
            &json!({}),
        )?;

        Ok(Self {
            source: resolved_path.to_path_buf(),
            transparent: transparent_path,
            directory: import_dir,
            width,
            height,
            matting,
        })
    }

    fn attach_to(
        &self,
        asset: &mut Asset,
        classification: &SheetClassification,
        validation: &Value,
    ) {
        let image = self;
        asset.width = image.width;
        asset.height = image.height;
        asset.has_alpha = true;
        asset.sheet_kind = Some(classification.status.clone());
        asset.validation = Some(validation.clone());

        if classification.slot == "original" || asset.path.as_os_str().is_empty() {
            asset.path = image.transparent.clone();
            asset.raw_original_path = Some(image.source.clone());
            asset.original_inspection = None;
        } else {
            let ref_label = image
                .source
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Reference")
                .to_string();
            if classification.slot == "turnaround_sheet" {
                asset.attached_turnaround_sheet = Some(image.transparent.clone());
            } else if classification.slot == "asset_sheet" {
                asset.attached_asset_sheet = Some(image.transparent.clone());
            }
            asset
                .attached_references
                .retain(|r| r.kind != classification.slot || classification.slot == "reference");
            asset.attached_references.push(AttachedReference {
                label: ref_label,
                kind: classification.slot.to_string(),
                path: image.transparent.clone(),
                raw_path: Some(image.source.clone()),
                width: image.width,
                height: image.height,
                report: None,
            });
        }
    }

    fn cut_sheet(
        &self,
        s: &mut Session,
        params: &Value,
        asset_id: &str,
        resolved_slot: &str,
    ) -> Result<Option<Value>> {
        let image = self;
        let tolerance = params
            .get("tolerance")
            .and_then(Value::as_u64)
            .map(|v| v.clamp(1, 100) as u32)
            .unwrap_or(25);
        let import_dir = &image.directory;
        let resolved_path = &image.source;
        // Automatically slice the imported transparent sheet if requested
        let auto_cut = params
            .get("auto_cut")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let use_ocr = params
            .get("use_ocr")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let result = if auto_cut
            && (resolved_slot == "turnaround_sheet" || resolved_slot == "asset_sheet")
        {
            let cut_kind = if resolved_slot == "turnaround_sheet" {
                "turnaround"
            } else {
                "action"
            };
            let cut_out_dir = import_dir.join(format!("{resolved_slot}_cut"));
            Some(cmd_asset_cut_sheet(
                s,
                &json!({
                    "asset": asset_id,
                    "source": resolved_path.to_string_lossy(),
                    "out_dir": cut_out_dir.to_string_lossy(),
                    "kind": cut_kind,
                    "tolerance": tolerance,
                    "use_ocr": use_ocr,
                }),
            )?)
        } else {
            None
        };

        Ok(result)
    }
}

impl SheetClassification {
    fn from_validation(validation: &Value, slot_param: &str, attach_to_active: bool) -> Self {
        let valid = validation
            .get("valid")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let status = validation
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or(if valid {
                "ready"
            } else {
                "single_or_partial_sheet"
            });
        let rows_detected = validation
            .get("rowsDetected")
            .and_then(Value::as_u64)
            .unwrap_or(1);
        let views_in_row = validation
            .get("viewsInPrimaryRow")
            .and_then(Value::as_u64)
            .unwrap_or(1);

        Self {
            valid,
            status: status.to_string(),
            slot: Self::resolve_slot(
                slot_param,
                attach_to_active,
                valid,
                rows_detected,
                views_in_row,
            ),
        }
    }

    fn resolve_slot(
        slot_param: &str,
        attach_to_active: bool,
        valid: bool,
        rows_detected: u64,
        views_in_row: u64,
    ) -> &'static str {
        match slot_param {
            "original" | "original_design" => "original",
            "reference" => "reference",
            "turnaround" | "turnaround_sheet" => "turnaround_sheet",
            "pose" | "pose_sheet" | "asset" | "asset_sheet" => "asset_sheet",
            _ => {
                if !attach_to_active && rows_detected == 1 && views_in_row <= 1 {
                    "original"
                } else if valid && rows_detected == 1 && views_in_row >= 3 {
                    "turnaround_sheet"
                } else if rows_detected >= 2 || views_in_row >= 3 {
                    "asset_sheet"
                } else if attach_to_active {
                    "reference"
                } else {
                    "original"
                }
            }
        }
    }
}
