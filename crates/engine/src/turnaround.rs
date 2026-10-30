//! Turnaround registration accuracy and 2D preview mode.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use crate::session::{LoadedManifest, MeasuredMotionRange, MeasuredTurnaroundRegistration};
use crate::{EngineError, Result, Session};

/// Telemetry the command reports back after it has been stored on the asset.
struct AccuracySummary {
    foot_lift_max_px: f32,
    width_max_delta_px: f32,
    max_vertical_drift_px: f32,
    registration_passed: bool,
}

/// The asset being measured: an explicit param, else the active asset, else the default.
fn requested_asset(s: &Session, params: &Value) -> String {
    params
        .get("asset")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| s.active_asset.clone())
        .unwrap_or_else(|| "hopper".to_string())
}

/// A directory param resolved against the workspace, else the manifest's own directory.
fn dir_param(s: &Session, params: &Value, key: &str, fallback: PathBuf) -> PathBuf {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(|p| super::workspace::resolve_workspace_path(s, p))
        .unwrap_or(fallback)
}

/// Where the optimizer writes: the param, else the asset dir, never one of its inputs.
fn accuracy_out_dir(
    s: &Session,
    root: &Path,
    params: &Value,
    asset_id: &str,
    inputs: (&Path, &Path),
) -> PathBuf {
    let out = dir_param(
        s,
        params,
        "out_dir",
        root.join("exports/pose_and_parts").join(asset_id),
    );
    if out == inputs.0 || out == inputs.1 {
        return root
            .join("exports/pose_and_parts")
            .join(format!("{asset_id}_accuracy"));
    }
    out
}

fn optimizer_args(turnaround_dir: &Path, poses_dir: &Path, out_dir: &Path) -> Vec<String> {
    vec![
        "--turnaround-dir".to_string(),
        turnaround_dir.to_string_lossy().into_owned(),
        "--poses-dir".to_string(),
        poses_dir.to_string_lossy().into_owned(),
        "--out-dir".to_string(),
        out_dir.to_string_lossy().into_owned(),
    ]
}

/// A file the optimizer reported, else the name it writes inside the out dir.
fn reported_file(report: &Value, key: &str, fallback: PathBuf) -> PathBuf {
    report
        .get(key)
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .unwrap_or(fallback)
}

/// Record the measurement on the asset, filling manifests it does not have yet.
fn store_measurements(
    s: &mut Session,
    asset_id: &str,
    motion: MeasuredMotionRange,
    registration: MeasuredTurnaroundRegistration,
    reg_manifest: Option<LoadedManifest>,
    pose_manifest: Option<LoadedManifest>,
) -> AccuracySummary {
    let summary = AccuracySummary {
        foot_lift_max_px: motion.foot_lift_max_px,
        width_max_delta_px: motion.width_max_delta_px,
        max_vertical_drift_px: registration.max_vertical_drift_px,
        registration_passed: registration.registration_passed,
    };
    let asset = s
        .assets
        .entry(asset_id.to_string())
        .or_insert_with(|| crate::session::Asset {
            id: asset_id.to_string(),
            ..Default::default()
        });
    if asset.pose_manifest.is_none() {
        asset.pose_manifest = pose_manifest;
    }
    if let Some(rm) = reg_manifest {
        asset.turnaround_manifest = Some(rm);
    }
    asset.motion_range = Some(motion);
    asset.turnaround_registration = Some(registration);
    asset.sync_parts_from_manifests();
    s.active_asset = Some(asset_id.to_string());
    summary
}

/// Run `optimize-accuracy` on the turnaround + pose dirs and store measured telemetry on the active asset.
pub fn cmd_turnaround_optimize_accuracy(s: &mut Session, params: &Value) -> Result<Value> {
    let root = super::workspace::workspace_root(s);
    let asset_id = requested_asset(s, params);
    let (turnaround_default, poses_default) = (
        s.assets
            .get(&asset_id)
            .and_then(|a| a.turnaround_manifest.as_ref())
            .map(|m| m.manifest_dir.clone())
            .unwrap_or_else(|| root.join("exports/pose_and_parts/hopper_turnaround")),
        s.assets
            .get(&asset_id)
            .and_then(|a| a.pose_manifest.as_ref())
            .map(|m| m.manifest_dir.clone())
            .unwrap_or_else(|| root.join("exports/pose_and_parts/hopper_poses")),
    );
    let turnaround_dir = dir_param(s, params, "turnaround_dir", turnaround_default);
    let poses_dir = dir_param(s, params, "poses_dir", poses_default);
    let out_dir = accuracy_out_dir(s, &root, params, &asset_id, (&turnaround_dir, &poses_dir));
    let args = optimizer_args(&turnaround_dir, &poses_dir, &out_dir);
    let report = super::workspace::run_python_json(s, "pipeline.codex_accuracy_optimizer", &args)?;

    let motion = MeasuredMotionRange::load_from_path(&reported_file(
        &report,
        "motionRangeFile",
        out_dir.join("motion_range.json"),
    ))?;
    let registration = MeasuredTurnaroundRegistration::load_from_path(&reported_file(
        &report,
        "registrationFile",
        out_dir.join("turnaround_registration.json"),
    ))?;
    let reg_manifest =
        LoadedManifest::load_from_path(&out_dir.join("registered_turnaround/manifest.json")).ok();
    let pose_manifest = LoadedManifest::load_from_path(&poses_dir).ok();
    let summary = store_measurements(
        s,
        &asset_id,
        motion,
        registration,
        reg_manifest,
        pose_manifest,
    );

    Ok(json!({
        "ok": true,
        "command": "turnaround.optimize_accuracy",
        "asset_id": asset_id,
        "registration_passed": summary.registration_passed,
        "max_vertical_drift_px": summary.max_vertical_drift_px,
        "foot_lift_max_px": summary.foot_lift_max_px,
        "width_max_delta_px": summary.width_max_delta_px,
        "report": report,
    }))
}

/// Set the active 2D preview mode (idle|walk|run|jump|wave|static), optional speed and frame range.
pub fn cmd_anim_preview_2d(s: &mut Session, params: &Value) -> Result<Value> {
    let mode = params
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("walk")
        .to_ascii_lowercase();
    if !matches!(
        mode.as_str(),
        "idle"
            | "walk"
            | "run"
            | "jump"
            | "wave"
            | "static"
            | "standing"
            | "walking"
            | "running"
            | "waving"
    ) {
        return Err(EngineError::InvalidParams(format!(
            "invalid preview mode '{mode}'; expected idle|walk|run|jump|wave|static"
        )));
    }
    s.preview_mode = mode.clone();
    if let Some(speed) = params.get("speed").and_then(Value::as_f64) {
        s.preview_speed = (speed as f32).clamp(0.1, 4.0);
    }
    if let (Some(start), Some(end)) = (
        params.get("range_start").and_then(Value::as_u64),
        params.get("range_end").and_then(Value::as_u64),
    ) && end > start
    {
        s.preview_range = (start as u32, end as u32);
    }

    let available_poses: Vec<String> = s
        .current_asset()
        .and_then(|a| a.pose_manifest.as_ref())
        .map(|m| m.frames.iter().map(|f| f.label.clone()).collect())
        .unwrap_or_default();

    // Only a rig moves. A reference sheet or plain image shows one still pose, so an action
    // other than static is reported as not playing rather than as a success.
    let rig_type = s
        .current_asset()
        .and_then(|a| a.cutout_rig.as_ref())
        .map(|r| r.rig_type.clone());
    let still = matches!(mode.as_str(), "static" | "standing");
    let (ok, plays) = match rig_type.as_deref() {
        Some("layered_motion") => (
            true,
            "the rig spec's own motion tracks (the action mode does not apply)".to_string(),
        ),
        Some(_) => (true, format!("the cutout rig's {mode} motion")),
        None if still => (true, "a still image".to_string()),
        None => (
            false,
            "a still image: this asset has no rig, so it cannot move".to_string(),
        ),
    };
    let mut out = json!({
        "ok": ok,
        "command": "anim.preview_2d",
        "mode": mode,
        "plays": plays,
        "speed": s.preview_speed,
        "range": [s.preview_range.0, s.preview_range.1],
        "active_asset": s.active_asset,
        "available_poses": available_poses,
    });
    if !ok {
        out["next"] = json!([
            "Build a rig for it: animation.prepare, or connect a rig spec with project.add_asset kind=cutout_rig."
        ]);
    }
    Ok(out)
}

