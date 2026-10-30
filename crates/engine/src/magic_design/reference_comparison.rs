use std::path::Path;

use serde_json::{Value, json};

use crate::session::Asset;
use crate::{EngineError, Result, Session};

struct ComparisonInput<'a> {
    slot: &'static str,
    report_key: &'static str,
    missing: &'static str,
    raw: Option<&'a Path>,
    imported: Option<&'a Path>,
    manifest: Option<&'a Path>,
}

fn comparison_inputs(asset: &Asset) -> [ComparisonInput<'_>; 3] {
    [
        ComparisonInput {
            slot: "original",
            report_key: "original_design",
            missing: "original reference/import files not found",
            raw: Some(asset.raw_original_path.as_deref().unwrap_or(&asset.path)),
            imported: Some(&asset.path),
            manifest: None,
        },
        ComparisonInput {
            slot: "asset_sheet",
            report_key: "attached_asset_sheet",
            missing: "asset_sheet reference/import files not attached",
            raw: asset
                .pose_manifest
                .as_ref()
                .and_then(|m| m.source_image.as_deref())
                .or(asset.attached_asset_sheet.as_deref()),
            imported: asset.attached_asset_sheet.as_deref(),
            manifest: asset
                .pose_manifest
                .as_ref()
                .map(|m| m.manifest_path.as_path()),
        },
        ComparisonInput {
            slot: "turnaround_sheet",
            report_key: "attached_turnaround_sheet",
            missing: "turnaround reference/import files not attached",
            raw: asset
                .turnaround_manifest
                .as_ref()
                .and_then(|m| m.source_image.as_deref())
                .or(asset.attached_turnaround_sheet.as_deref()),
            imported: asset.attached_turnaround_sheet.as_deref(),
            manifest: asset
                .turnaround_manifest
                .as_ref()
                .map(|m| m.manifest_path.as_path()),
        },
    ]
}

impl ComparisonInput<'_> {
    fn compare(&self, s: &Session, yaw: f64) -> Result<Option<Value>> {
        let input = self;
        let (Some(raw), Some(imported)) = (input.raw, input.imported) else {
            return Ok(None);
        };
        if !raw.is_file() || !imported.is_file() {
            return Ok(None);
        }
        let mut args = vec![
            raw.to_string_lossy().into_owned(),
            imported.to_string_lossy().into_owned(),
            "--yaw".to_string(),
            format!("{yaw:.1}"),
        ];
        if let Some(manifest) = input.manifest {
            args.extend(["--manifest".into(), manifest.to_string_lossy().into_owned()]);
        }
        crate::workspace::run_python_json(s, "pipeline.jev_qa_verifier", &args).map(Some)
    }
}

/// Compare raw vs imported images across original / asset_sheet / turnaround_sheet slots.
pub fn cmd_intake_compare_reference(s: &mut Session, params: &Value) -> Result<Value> {
    let slot = comparison_slot(params)?;
    let yaw = params.get("yaw").and_then(Value::as_f64).unwrap_or(0.0);
    let asset = s
        .current_asset()
        .ok_or_else(|| EngineError::InvalidParams("no active asset in session".into()))?;
    let asset_id = asset.id.clone();
    let mut report = serde_json::Map::new();
    let mut compared_slots = Vec::new();
    let mut skipped_slots = Vec::new();
    for input in comparison_inputs(asset) {
        if slot != "all" && slot != input.slot {
            continue;
        }
        match input.compare(s, yaw)? {
            Some(value) => {
                report.insert(input.report_key.into(), value);
                compared_slots.push(input.slot);
            }
            None if slot == input.slot => {
                return Err(EngineError::InvalidParams(format!(
                    "{} for asset '{asset_id}'",
                    input.missing
                )));
            }
            None => skipped_slots.push(input.slot),
        }
    }
    if compared_slots.is_empty() {
        return Err(EngineError::InvalidParams(format!(
            "no valid reference/import image pairs available to compare for asset '{asset_id}'"
        )));
    }
    report.insert("ok".into(), json!(true));
    report.insert("command".into(), json!("intake.compare_reference"));
    report.insert("asset_id".into(), json!(asset_id));
    report.insert("yaw_deg".into(), json!(yaw));
    report.insert("compared_slots".into(), json!(compared_slots));
    report.insert("skipped_slots".into(), json!(skipped_slots));
    let out = Value::Object(report);
    if let Some(asset) = s.current_asset_mut() {
        asset.original_inspection = Some(out.clone());
    }
    Ok(out)
}

fn comparison_slot(params: &Value) -> Result<&'static str> {
    Ok(
        match params.get("slot").and_then(Value::as_str).unwrap_or("all") {
            "all" => "all",
            "original" => "original",
            "asset_sheet" | "poses" => "asset_sheet",
            "turnaround" | "turnaround_sheet" => "turnaround_sheet",
            other => {
                return Err(EngineError::InvalidParams(format!(
                    "invalid slot '{other}'; expected all|original|asset_sheet|turnaround"
                )));
            }
        },
    )
}
