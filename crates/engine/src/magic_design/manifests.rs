use crate::session::{
    Asset, LoadedCutoutRigPlan, LoadedManifest, MeasuredMotionRange, MeasuredTurnaroundRegistration,
};
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

struct ManifestPaths {
    id: String,
    pose: Option<PathBuf>,
    turnaround: Option<PathBuf>,
    motion: Option<PathBuf>,
    registration: Option<PathBuf>,
}

/// An asset being assembled from manifests and optional local evidence.
struct ManifestAsset<'a> {
    asset: Asset,
    root: &'a Path,
    id: &'a str,
}

pub fn cmd_asset_load_manifest(s: &mut Session, params: &Value) -> Result<Value> {
    let root = crate::workspace::workspace_root(s);
    let ManifestPaths {
        id: asset_id,
        pose: pose_path,
        turnaround: turnaround_path,
        motion: motion_path,
        registration: reg_path,
    } = resolve_paths(s, params, &root)?;
    let (loaded_pose, direct_cutout_rig) = load_pose_document(pose_path.as_deref())?;
    let asset = s.assets.remove(&asset_id).unwrap_or_else(|| Asset {
        id: asset_id.clone(),
        ..Asset::default()
    });
    let mut loaded = ManifestAsset {
        asset,
        root: &root,
        id: &asset_id,
    };
    loaded.attach_pose_manifest(loaded_pose, turnaround_path.is_none());
    loaded.attach_turnaround_manifest(turnaround_path);
    loaded.attach_cutout_plan(direct_cutout_rig);
    loaded.restore_evidence(motion_path, reg_path);
    Ok(loaded.commit(s))
}

fn resolve_paths(s: &Session, params: &Value, root: &Path) -> Result<ManifestPaths> {
    let manifest_raw = params
        .get("manifest")
        .or_else(|| params.get("pose_manifest"))
        .and_then(Value::as_str)
        .filter(|m| !m.trim().is_empty())
        .ok_or_else(|| EngineError::InvalidParams("missing 'manifest' parameter".into()))?;
    let resolved_pose = crate::workspace::resolve_workspace_path(s, manifest_raw);
    let id = params
        .get("asset")
        .or_else(|| params.get("asset_id"))
        .and_then(Value::as_str)
        .map(crate::workspace::sanitize_asset_id)
        .or_else(|| s.active_asset.clone())
        .unwrap_or_else(|| {
            let parent = resolved_pose
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("asset");
            crate::workspace::sanitize_asset_id(parent)
        });
    let optional_path = |key| {
        params
            .get(key)
            .and_then(Value::as_str)
            .map(|p| crate::workspace::resolve_workspace_path(s, p))
    };
    Ok(ManifestPaths {
        pose: Some(resolved_pose),
        turnaround: optional_path("turnaround_manifest"),
        motion: optional_path("motion_range").or_else(|| {
            Some(
                root.join("exports/pose_and_parts")
                    .join(&id)
                    .join("motion_range.json"),
            )
        }),
        registration: optional_path("turnaround_registration").or_else(|| {
            Some(
                root.join("exports/pose_and_parts")
                    .join(&id)
                    .join("turnaround_registration.json"),
            )
        }),
        id,
    })
}

fn load_pose_document(
    pose_path: Option<&Path>,
) -> Result<(Option<LoadedManifest>, Option<LoadedCutoutRigPlan>)> {
    let mut direct_cutout_rig: Option<LoadedCutoutRigPlan> = None;
    let loaded_pose = match pose_path.as_ref() {
        Some(path) => {
            if let Ok(text) = std::fs::read_to_string(path)
                && let Ok(raw) = serde_json::from_str::<Value>(&text)
                && (raw.get("rig_type").is_some()
                    || (raw.get("layers").is_some() && raw.get("canvas").is_some()))
            {
                let plan = load_resolved_cutout_plan(path)?;
                direct_cutout_rig = Some(plan);
                None
            } else {
                Some(LoadedManifest::load_from_path(path)?)
            }
        }
        None => None,
    };
    Ok((loaded_pose, direct_cutout_rig))
}

fn load_resolved_cutout_plan(path: &Path) -> Result<LoadedCutoutRigPlan> {
    let mut plan = LoadedCutoutRigPlan::load_from_path(path)?;
    plan.resolve_paths(path.parent().unwrap_or(Path::new(".")));
    Ok(plan)
}

fn restore_sheet_attachment(
    attached: &mut Option<PathBuf>,
    manifest: Option<&LoadedManifest>,
    root: &Path,
    asset_id: &str,
    kind: &str,
) {
    if attached.is_some() {
        return;
    }
    let transparent = root.join(format!(
        "exports/pose_and_parts/{asset_id}/{asset_id}_{kind}_sheet_transparent.png"
    ));
    *attached = if transparent.is_file() {
        Some(transparent)
    } else {
        manifest
            .and_then(|m| m.source_image.as_ref())
            .filter(|p| p.is_file())
            .cloned()
    };
}

impl ManifestAsset<'_> {
    fn restore_evidence(&mut self, motion_path: Option<PathBuf>, reg_path: Option<PathBuf>) {
        self.restore_original_image();
        self.restore_cutout_plan();
        restore_sheet_attachment(
            &mut self.asset.attached_asset_sheet,
            self.asset.pose_manifest.as_ref(),
            self.root,
            self.id,
            "pose",
        );
        restore_sheet_attachment(
            &mut self.asset.attached_turnaround_sheet,
            self.asset.turnaround_manifest.as_ref(),
            self.root,
            self.id,
            "turnaround",
        );
        self.asset.motion_range = motion_path
            .filter(|p| p.is_file())
            .and_then(|p| MeasuredMotionRange::load_from_path(&p).ok());
        self.asset.turnaround_registration = reg_path
            .filter(|p| p.is_file())
            .and_then(|p| MeasuredTurnaroundRegistration::load_from_path(&p).ok());
        self.asset.sync_parts_from_manifests();
    }

    fn commit(self, session: &mut Session) -> Value {
        let response = self.summary();
        session.assets.insert(self.id.to_owned(), self.asset);
        session.active_asset = Some(self.id.to_owned());
        response
    }

    fn attach_pose_manifest(&mut self, loaded_pose: Option<LoadedManifest>, no_turnaround: bool) {
        let asset = &mut self.asset;
        if let Some(first_frame) = loaded_pose.as_ref().and_then(|m| m.frames.first()) {
            asset.path = first_frame.resolved_path.clone();
            asset.width = first_frame.size[0];
            asset.height = first_frame.size[1];
            asset.has_alpha = true;
        }
        if let Some(main) = &loaded_pose {
            asset.sheet_kind = Some(main.sheet_type.clone());
        }

        if let Some(main) = loaded_pose {
            if main.sheet_type == "turnaround" && no_turnaround {
                asset.turnaround_manifest = Some(main);
            } else {
                asset.pose_manifest = Some(main);
            }
        }
    }

    fn attach_turnaround_manifest(&mut self, turnaround_path: Option<PathBuf>) {
        let asset = &mut self.asset;
        if let Some(tp) = turnaround_path
            && tp.exists()
            && let Ok(tm) = LoadedManifest::load_from_path(&tp)
        {
            if asset.path.as_os_str().is_empty()
                && let Some(first_turn) = tm.frames.first()
            {
                asset.path = first_turn.resolved_path.clone();
                asset.width = first_turn.size[0];
                asset.height = first_turn.size[1];
                asset.has_alpha = true;
            }
            asset.turnaround_manifest = Some(tm);
        }
    }

    fn attach_cutout_plan(&mut self, direct_cutout_rig: Option<LoadedCutoutRigPlan>) {
        let asset = &mut self.asset;
        if let Some(plan) = direct_cutout_rig {
            if asset.path.as_os_str().is_empty() && plan.source_image.is_file() {
                asset.path = plan.source_image.clone();
                if let Ok((w, h)) = image::image_dimensions(&asset.path) {
                    asset.width = w;
                    asset.height = h;
                }
                asset.has_alpha = true;
            } else if asset.path.as_os_str().is_empty()
                && let Some(first_part) = plan.parts.first()
                && first_part.cropped_path.is_file()
            {
                asset.path = first_part.cropped_path.clone();
                asset.width = plan.canvas_size[0];
                asset.height = plan.canvas_size[1];
                asset.has_alpha = true;
            }
            if let Some(ref sheet) = plan.cutout_asset_sheet
                && sheet.is_file()
            {
                asset.attached_asset_sheet = Some(sheet.clone());
            }
            asset.cutout_rig = Some(plan);
        }
    }

    fn restore_original_image(&mut self) {
        let asset = &mut self.asset;
        let root = self.root;
        let asset_id = self.id;
        let orig_transparent = root.join(format!(
            "exports/pose_and_parts/{asset_id}/{asset_id}_original_transparent.png"
        ));
        if orig_transparent.is_file() {
            asset.path = orig_transparent;
            if let Ok((w, h)) = image::image_dimensions(&asset.path) {
                asset.width = w;
                asset.height = h;
            }
            asset.has_alpha = true;
        }
    }

    fn restore_cutout_plan(&mut self) {
        let asset = &mut self.asset;
        let root = self.root;
        let asset_id = self.id;
        let cutout_plan_path = root.join(format!(
            "exports/pose_and_parts/{asset_id}/{asset_id}_cutout_rig_plan.json"
        ));
        if asset.cutout_rig.is_none()
            && cutout_plan_path.is_file()
            && let Ok(plan) = LoadedCutoutRigPlan::load_from_path(&cutout_plan_path)
        {
            if let Some(ref sheet) = plan.cutout_asset_sheet
                && sheet.is_file()
            {
                asset.attached_asset_sheet = Some(sheet.clone());
            }
            asset.cutout_rig = Some(plan);
        }
    }

    fn summary(&self) -> Value {
        let asset = &self.asset;
        let asset_id = self.id;
        json!({
            "ok": true,
            "command": "asset.load_manifest",
            "asset_id": asset_id,
            "pose_labels": asset.pose_manifest.as_ref().map(|m| m.frames.iter().map(|f| f.label.clone()).collect::<Vec<_>>()).unwrap_or_default(),
            "turnaround_labels": asset.turnaround_manifest.as_ref().map(|m| m.frames.iter().map(|f| f.label.clone()).collect::<Vec<_>>()).unwrap_or_default(),
            "pose_sheet_missing": asset.pose_manifest.is_none(),
            "has_measured_motion": asset.motion_range.is_some(),
            "has_measured_registration": asset.turnaround_registration.is_some(),
            "foot_lift_max_px": asset.motion_range.as_ref().map(|m| m.foot_lift_max_px),
            "max_vertical_drift_px": asset.turnaround_registration.as_ref().map(|r| r.max_vertical_drift_px),
        })
    }
}
