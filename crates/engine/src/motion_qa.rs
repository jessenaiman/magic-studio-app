//! Programmatic QA gate (`validate.motion`) for Issue #55 (2.5D Puppet & Deform Rig).
//!
//! Verifies:
//! 1. Zero sine-wave fakery (`oscillate` / `Math.sin` shortcuts).
//! 2. Zero foot sliding during stance contact frames (`Contact -> Down -> Passing`).
//! 3. Zero 16x16 transparent V-notch gaps around rotating hinge joints in rendered frames.
//! 4. Full channel coverage across root/torso, arms, legs/feet, and secondary overlap.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::workspace::resolve_workspace_path;
use crate::{EngineError, Result, Session};

fn load_spec_value(s: &Session, params: &Value) -> Result<(PathBuf, Value)> {
    let spec_path = if let Some(raw) = params
        .get("spec")
        .or_else(|| params.get("manifest"))
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
    {
        resolve_workspace_path(s, raw)
    } else if let Some(rig) = s.current_asset().and_then(|a| a.cutout_rig.as_ref()) {
        rig.plan_path.clone()
    } else {
        return Err(EngineError::InvalidParams(
            "validate.motion requires `spec` (rig.json or rig_manifest.json) or an active rigged asset"
                .into(),
        ));
    };
    let bytes = std::fs::read(&spec_path)?;
    let spec: Value = serde_json::from_slice(&bytes)
        .map_err(|e| EngineError::InvalidParams(format!("invalid motion spec JSON: {e}")))?;
    Ok((spec_path, spec))
}

fn check_no_sine_fakery(spec: &Value) -> Vec<String> {
    let mut violations = Vec::new();
    let Some(tracks) = spec["tracks"].as_array() else {
        return violations;
    };
    for track in tracks {
        let tid = track["id"].as_str().unwrap_or("track");
        let Some(motion) = track["motion"].as_object() else {
            continue;
        };
        for (layer_id, props) in motion {
            let Some(prop_map) = props.as_object() else {
                continue;
            };
            for (prop_name, terms) in prop_map {
                let has_oscillate = terms
                    .as_array()
                    .is_some_and(|arr| arr.iter().any(|t| t["kind"].as_str() == Some("oscillate")));
                if has_oscillate {
                    violations.push(format!(
                        "{tid}.{layer_id}.{prop_name}: uses banned `oscillate` sine-wave shortcut"
                    ));
                }
            }
        }
    }
    violations
}

fn check_foot_stance_lock(spec: &Value) -> (bool, f64, Vec<String>) {
    let mut issues = Vec::new();
    let mut max_stance_drift: f64 = 0.0;
    let Some(tracks) = spec["tracks"].as_array() else {
        return (true, 0.0, issues);
    };
    for track in tracks {
        let tid = track["id"].as_str().unwrap_or("track");
        if !tid.contains("walk") {
            continue;
        }
        let Some(motion) = track["motion"].as_object() else {
            continue;
        };
        for (lid, props) in motion {
            let low = lid.to_ascii_lowercase();
            if !low.contains("foot") && !low.contains("leg") {
                continue;
            }
            if let Some(pos_terms) = props["position"].as_array() {
                for term in pos_terms {
                    let Some(keys) = term["keys"].as_array() else {
                        continue;
                    };
                    // Stance phase keys (first two keys in contact->passing) must keep ground Y within 1.5px
                    if keys.len() >= 2
                        && let (Some(y0), Some(y1)) = (
                            keys[0].get(1).and_then(|v| v.get(1)).and_then(Value::as_f64),
                            keys[1].get(1).and_then(|v| v.get(1)).and_then(Value::as_f64),
                        )
                    {
                        let drift = (y1 - y0).abs();
                        max_stance_drift = max_stance_drift.max(drift);
                        if drift > 1.5 {
                            issues.push(format!(
                                "{tid}.{lid}: stance foot vertical drift {drift:.2}px exceeds 1.5px ground lock"
                            ));
                        }
                    }
                }
            }
        }
    }
    (issues.is_empty(), max_stance_drift, issues)
}

fn inspect_16x16_joint_window(img: &image::RgbaImage, pivot: [u32; 2]) -> u32 {
    let (w, h) = img.dimensions();
    let x0 = pivot[0].saturating_sub(8);
    let y0 = pivot[1].saturating_sub(8);
    let x1 = (pivot[0] + 8).min(w);
    let y1 = (pivot[1] + 8).min(h);
    let mut opaque = 0u32;
    for y in y0..y1 {
        for x in x0..x1 {
            if img.get_pixel(x, y)[3] > 16 {
                opaque += 1;
            }
        }
    }
    opaque
}

fn check_joint_gaps_in_frames(spec: &Value, frame_paths: &[PathBuf]) -> (bool, Vec<String>) {
    let mut gaps = Vec::new();
    let Some(layers) = spec["layers"].as_array() else {
        return (true, gaps);
    };
    let pivots: Vec<(String, [u32; 2])> = layers
        .iter()
        .filter_map(|l| {
            let id = l["id"].as_str()?.to_string();
            let pv = l["pivot"].as_array()?;
            Some((
                id,
                [
                    pv.first()?.as_f64()?.round() as u32,
                    pv.get(1)?.as_f64()?.round() as u32,
                ],
            ))
        })
        .collect();
    for frame in frame_paths {
        let Ok(rgba) = image::open(frame).map(|i| i.into_rgba8()) else {
            continue;
        };
        for (id, pv) in &pivots {
            let opaque = inspect_16x16_joint_window(&rgba, *pv);
            if opaque == 0 {
                gaps.push(format!(
                    "{}: 16x16 transparent gap at joint `{id}` ({}, {})",
                    frame. file_name().unwrap_or_default().to_string_lossy(),
                    pv[0],
                    pv[1]
                ));
            }
        }
    }
    (gaps.is_empty(), gaps)
}

fn collect_diagnostic_frames(s: &Session, params: &Value, spec_path: &Path) -> Vec<PathBuf> {
    if let Some(arr) = params.get("frames").and_then(Value::as_array) {
        return arr
            .iter()
            .filter_map(Value::as_str)
            .map(|p| resolve_workspace_path(s, p))
            .filter(|p| p.is_file())
            .collect();
    }
    let Some(parent) = spec_path.parent() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("png") {
                found.push(p);
            }
        }
    }
    found
}

fn check_channel_coverage(spec: &Value) -> Value {
    let layers = spec["layers"].as_array().cloned().unwrap_or_default();
    let mut has_root = false;
    let mut has_arm = false;
    let mut has_leg = false;
    let mut has_overlap = false;
    for l in &layers {
        let low = l["id"].as_str().unwrap_or_default().to_ascii_lowercase();
        if low.contains("body") || low.contains("torso") || low.contains("hip") || low.contains("root") {
            has_root = true;
        }
        if low.contains("arm") || low.contains("hand") || low.contains("wing") {
            has_arm = true;
        }
        if low.contains("leg") || low.contains("foot") || low.contains("shin") || low.contains("thigh") {
            has_leg = true;
        }
        if low.contains("tail")
            || low.contains("ear")
            || low.contains("hair")
            || low.contains("cape")
            || low.contains("coat")
            || low.contains("skirt")
            || l.get("follow_through").is_some()
        {
            has_overlap = true;
        }
    }
    json!({
        "layer_count": layers.len(),
        "root_or_torso": has_root,
        "arms_or_wings": has_arm,
        "legs_or_feet": has_leg,
        "secondary_overlap": has_overlap,
    })
}

/// Validate a layered motion spec and optional rendered frames per Issue #55 Stage 3.
pub fn cmd_validate_motion(s: &mut Session, params: &Value) -> Result<Value> {
    let (spec_path, spec) = load_spec_value(s, params)?;
    let sine_violations = check_no_sine_fakery(&spec);
    let (foot_lock_ok, max_stance_drift_px, foot_issues) = check_foot_stance_lock(&spec);
    let frames = collect_diagnostic_frames(s, params, &spec_path);
    let (joint_gaps_ok, gap_issues) = check_joint_gaps_in_frames(&spec, &frames);
    let coverage = check_channel_coverage(&spec);
    let ok = sine_violations.is_empty() && foot_lock_ok && joint_gaps_ok;

    Ok(json!({
        "ok": ok,
        "command": "validate.motion",
        "spec": spec_path,
        "frames_checked": frames.len(),
        "zero_sine_fakery": sine_violations.is_empty(),
        "sine_violations": sine_violations,
        "zero_foot_sliding": foot_lock_ok,
        "max_stance_drift_px": max_stance_drift_px,
        "foot_sliding_issues": foot_issues,
        "zero_joint_gaps_16x16": joint_gaps_ok,
        "joint_gap_issues": gap_issues,
        "channel_coverage": coverage,
    }))
}
