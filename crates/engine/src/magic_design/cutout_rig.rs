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
    if params["stage"] == "skeleton" {
        return prepare_skeleton(s, &id, &src_path);
    }

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

    let out_dir = prepare_dir(s, &id);
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

/// Where preparation writes: the open character's own folder, else `exports/pose_and_parts/<id>`.
fn prepare_dir(s: &Session, id: &str) -> std::path::PathBuf {
    if s.character_file
        && let Some(dir) = s.project_path.as_deref().and_then(std::path::Path::parent)
    {
        return dir.join("rig");
    }
    crate::workspace::workspace_root(s).join("exports/pose_and_parts").join(id)
}

/// AssetFurnace's explicitly configured install: environment, then saved settings.
fn assetfurnace_home() -> Result<std::path::PathBuf> {
    [
        std::env::var_os("ASSETFURNACE_HOME").map(std::path::PathBuf::from),
        crate::project_settings::saved_path("assetfurnace_home"),
    ]
    .into_iter()
    .flatten()
    .find(|home| home.join(".venv/bin/python").is_file())
    .ok_or_else(|| EngineError::InvalidParams(
        "Configure AssetFurnace's install folder as assetfurnace_home in Magic Studio settings.json, or set ASSETFURNACE_HOME".into(),
    ))
}

/// Stage 1: AssetFurnace makes and rigs a 3D model of the artwork; the model and its
/// skeleton are saved beside the character for review before parts are cut.
fn prepare_skeleton(s: &Session, id: &str, src_path: &std::path::Path) -> Result<Value> {
    let home = assetfurnace_home()?;
    let root = crate::workspace::workspace_root(s);
    let out = prepare_dir(s, id).join("model");
    let args: Vec<String> = [
        root.join("pipeline/rigged_model_from_picture.py"),
        src_path.to_path_buf(),
        "--out".into(),
        out.clone(),
        "--assetfurnace".into(),
        home.clone(),
    ]
    .iter()
    .map(|p| p.to_string_lossy().into_owned())
    .collect();
    let python = home.join(".venv/bin/python");
    let stdout = crate::workspace::run_tool_in(&root, &python.to_string_lossy(), &args)?;
    let receipt: Value = stdout
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str(line).ok())
        .ok_or_else(|| EngineError::ExternalTool("AssetFurnace".into(), "no result".into()))?;
    Ok(json!({
        "ok": true,
        "asset_id": id,
        "stage": "skeleton",
        "model": receipt["model"],
        "skeleton": receipt["skeleton"],
        "joints": receipt["joints"],
        "next": "Review the skeleton; the 2D parts are cut from it next.",
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
