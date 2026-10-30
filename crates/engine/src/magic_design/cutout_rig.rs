use crate::{EngineError, Result, Session};
use serde_json::{Value, json};

/// Build a guided cutout rig plan from a reviewed source sheet.
pub fn cmd_asset_build_cutout_rig(s: &mut Session, params: &Value) -> Result<Value> {
    activate_requested_asset(s, params)?;
    let asset = s
        .current_asset()
        .ok_or_else(|| EngineError::InvalidParams("no active asset".into()))?;
    let id = asset.id.clone();
    let src_path = asset.path.clone();
    crate::sheets::require_sheet_review(s, &id, &src_path)?;

    let rec_parts_csv = asset
        .intake_recommendation
        .as_ref()
        .and_then(|r| r.get("selectedParts"))
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(",")
        })
        .filter(|s| !s.is_empty());

    let parts_csv = rec_parts_csv;

    let out_dir = crate::workspace::workspace_root(s)
        .join("exports/pose_and_parts")
        .join(&id);
    let mut args = vec![
        "-m".into(),
        "pipeline.guided_cutout_rig_builder".into(),
        src_path.to_string_lossy().into_owned(),
        "--character".into(),
        id.clone(),
        "--out-dir".into(),
        out_dir.to_string_lossy().into_owned(),
    ];
    if let Some(p_csv) = &parts_csv {
        args.push("--parts".into());
        args.push(p_csv.clone());
    }
    crate::workspace::run_tool_in(&crate::workspace::workspace_root(s), "python3", &args)?;
    let plan_path = out_dir.join(format!("{id}_cutout_rig_plan.json"));
    let plan = crate::session::LoadedCutoutRigPlan::load_from_path(&plan_path)?;
    if let Some(asset) = s.current_asset_mut() {
        asset.attached_asset_sheet = plan.cutout_asset_sheet.clone();
        asset.cutout_rig = Some(plan);
        asset.sync_parts_from_manifests();
    }
    let synced_video_tracks = sync_placed_tracks(s, &id);
    Ok(json!({
        "ok": true,
        "asset_id": id,
        "selected_parts": parts_csv,
        "output": out_dir,
        "synced_video_tracks": synced_video_tracks,
    }))
}

fn activate_requested_asset(s: &mut Session, params: &Value) -> Result<()> {
    if s.project_path.is_none()
        && (params.get("isolated").and_then(Value::as_bool) == Some(true)
            || params.get("use_default_project").and_then(Value::as_bool) == Some(true))
    {
        let _ = crate::project::ensure_default_asset_project(s)?;
    }
    if let Some(asset_id) = params
        .get("asset")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        if !s.assets.contains_key(asset_id) {
            let _ = crate::project::cmd_project_load_asset(s, &json!({"id": asset_id}));
        }
        if s.assets.contains_key(asset_id) {
            s.active_asset = Some(asset_id.to_string());
        }
    }
    Ok(())
}

fn sync_placed_tracks(s: &mut Session, id: &str) -> Vec<Value> {
    let mut synced_video_tracks = Vec::new();
    if let Some(dir) = s.project_path.as_ref().and_then(|p| p.parent()) {
        let placed = crate::video_target::read_placed_tracks(dir);
        let mut tracks_to_sync: Vec<String> = Vec::new();
        for item in &placed {
            if item["character"].as_str() == Some(id)
                && let Some(tr) = item["track"].as_str().filter(|t| !t.is_empty())
                && !tracks_to_sync.iter().any(|existing| existing == tr)
            {
                tracks_to_sync.push(tr.to_string());
            }
        }
        for tr in tracks_to_sync {
            if let Ok(exported) = crate::motion::cmd_motion_export_track(
                s,
                &json!({"character": id, "track": tr, "include_theatre": true}),
            ) && let Some(arr) = exported["synced_placements"].as_array()
            {
                synced_video_tracks.extend(arr.iter().cloned());
            }
        }
    }
    synced_video_tracks
}
