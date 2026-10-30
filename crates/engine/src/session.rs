//! Session state for Magic Studio.
//!
//! Holds the current project/assets, loaded manifests, and verified measurements,
//! and provides [`Session::execute`] for command dispatch.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{EngineError, Result};

pub use crate::session_manifests::{
    LoadedManifest, ManifestFrame, ManifestPart, MeasuredMotionRange, MeasuredTurnaroundRegistration,
    PartState, PoseMotionMetrics,
};
pub(crate) use crate::session_layers::compute_alpha_bounds;

/// The engine session. All project changes go through [`Session::execute`].
#[derive(Debug)]
pub struct Session {
    /// Current working directory for relative asset paths.
    pub cwd: PathBuf,
    /// Loaded assets keyed by their logical id.
    pub assets: HashMap<String, Asset>,
    /// Ordered connections in this project, separate from decoded/cached assets.
    pub project_assets: Vec<ProjectAsset>,
    /// The open `.magic` file; `None` until `project.new` or `project.open`.
    pub project_path: Option<PathBuf>,
    /// Where the final video project lives (HyperFrames/Remotion/FilmCraft). Single source of truth.
    pub video_project_path: Option<PathBuf>,
    /// Opaque native Theatre save-file state; absent on projects that have never saved Theatre edits.
    pub theatre_state: Option<Value>,
    /// Editable native sources and explicitly accepted immutable render receipts.
    pub animation_sources: BTreeMap<String, Value>,
    /// `Some("animation")` for a character study (Animation only, no song or
    /// storyboard); `None` for a song project.
    pub project_mode: Option<String>,
    /// User-facing sessions publish Home history and recent receipts; passive library/tests do not.
    pub record_user_state: bool,
    /// Active chat selection belongs to this engine/window, never to project serialization.
    pub agent_selection: Option<Value>,
    /// Exclusive writer lease on the canonical project file for the live Studio session.
    pub project_owner: Option<std::sync::Arc<std::fs::File>>,
    /// The selected recording; uploads use the copy inside the contained HyperFrames project.
    pub song_audio: Option<PathBuf>,
    /// Whether the user approved the workshop draft PDF.
    pub draft_approved: bool,
    /// The contained HyperFrames video project.
    pub video_target: String,
    /// Currently active asset id in the session.
    pub active_asset: Option<String>,
    /// Active 2D preview mode (`idle`, `walk`, `run`, `jump`, `wave`, `static`).
    pub preview_mode: String,
    /// Active 2D preview speed multiplier.
    pub preview_speed: f32,
    /// Active 2D preview frame range `(start_frame, end_frame)` at 24fps.
    pub preview_range: (u32, u32),
    /// Last command result, echoed back for inspection.
    pub last_result: Option<Value>,
    /// Whether Blender 5.3 is reachable at 127.0.0.1:9876.
    pub blender_available: bool,
    /// Project-file writes; `write_project_file` takes `&Session`, hence the cell.
    pub project_saves: std::sync::atomic::AtomicU64,
    /// Successful journaled commands since the project file was last written.
    pub edits_since_save: u64,
}

impl Default for Session {
    fn default() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            cwd,
            assets: HashMap::new(),
            project_assets: Vec::new(),
            project_path: None,
            video_project_path: None,
            theatre_state: None,
            animation_sources: BTreeMap::new(),
            project_mode: None,
            record_user_state: false,
            agent_selection: None,
            project_owner: None,
            song_audio: None,
            draft_approved: false,
            video_target: "hyperframes".into(),
            active_asset: None,
            preview_mode: "walk".to_string(),
            preview_speed: 1.0,
            preview_range: (0, 72),
            last_result: None,
            blender_available: false,
            project_saves: std::sync::atomic::AtomicU64::new(0),
            edits_since_save: 0,
        }
    }
}

impl Clone for Session {
    fn clone(&self) -> Self {
        Self {
            cwd: self.cwd.clone(),
            assets: self.assets.clone(),
            project_assets: self.project_assets.clone(),
            project_path: self.project_path.clone(),
            video_project_path: self.video_project_path.clone(),
            theatre_state: self.theatre_state.clone(),
            animation_sources: self.animation_sources.clone(),
            project_mode: self.project_mode.clone(),
            record_user_state: self.record_user_state,
            agent_selection: self.agent_selection.clone(),
            project_owner: self.project_owner.clone(),
            song_audio: self.song_audio.clone(),
            draft_approved: self.draft_approved,
            video_target: self.video_target.clone(),
            active_asset: self.active_asset.clone(),
            preview_mode: self.preview_mode.clone(),
            preview_speed: self.preview_speed,
            preview_range: self.preview_range,
            last_result: self.last_result.clone(),
            blender_available: self.blender_available,
            project_saves: std::sync::atomic::AtomicU64::new(
                self.project_saves
                    .load(std::sync::atomic::Ordering::Relaxed),
            ),
            edits_since_save: self.edits_since_save,
        }
    }
}

impl Session {
    pub fn user_session() -> Self {
        Self {
            record_user_state: true,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAsset {
    pub id: String,
    pub name: String,
    pub preset: Option<String>,
    pub path: Option<PathBuf>,
    /// `auto`, `image`, `poses`, `turnaround`, or `cutout_rig`.
    pub kind: String,
    /// Part visibility / solo / review state as last saved; applied when the asset loads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<PartState>,
}

/// A reference image attached to a character once the user likes the original design.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttachedReference {
    pub label: String,
    /// Role/kind: `"asset_sheet"`, `"turnaround_sheet"`, or `"reference"`.
    pub kind: String,
    /// Transparent-background (Step 1) image path.
    pub path: PathBuf,
    /// Raw uploaded image path before background removal.
    pub raw_path: Option<PathBuf>,
    pub width: u32,
    pub height: u32,
    /// Automated telemetry reported by the tool on import.
    pub report: Option<Value>,
}

/// A loaded image/character asset with its decoded metadata, cut manifests, and measured telemetry.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: String,
    /// Primary Original Design image path (transparent Step 1 cutout or primary hero frame).
    pub path: PathBuf,
    /// Raw Original Design image path before Step 1 background removal (if available).
    #[serde(default)]
    pub raw_original_path: Option<PathBuf>,
    /// Automated tool inspection report for the Original Design image.
    #[serde(default)]
    pub original_inspection: Option<Value>,
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    pub sheet_kind: Option<String>,
    pub validation: Option<Value>,
    /// Actual recommendation returned by the three-question intake command.
    #[serde(default)]
    pub intake_recommendation: Option<Value>,
    /// Attached full Pose / Asset Sheet image path (transparent background).
    #[serde(default)]
    pub attached_asset_sheet: Option<PathBuf>,
    /// Attached full Turnaround Sheet image path (transparent background).
    #[serde(default)]
    pub attached_turnaround_sheet: Option<PathBuf>,
    /// Reference images uploaded and attached to this character once the user likes the design.
    #[serde(default)]
    pub attached_references: Vec<AttachedReference>,
    /// Guided 2D Cutout Rig Plan (`character-rigging` + `pose-library-design` + PhotoCraft + EffectCraft 0.10.0).
    #[serde(default)]
    pub cutout_rig: Option<LoadedCutoutRigPlan>,
    pub pose_manifest: Option<LoadedManifest>,
    pub turnaround_manifest: Option<LoadedManifest>,
    pub turnaround_registration: Option<MeasuredTurnaroundRegistration>,
    pub motion_range: Option<MeasuredMotionRange>,
    pub parts: Vec<PartState>,
}

/// A single movable cutout layer part discovered and masked via `photocraft-cli`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CutoutRigPart {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub layer: i32,
    pub parent: Option<String>,
    pub pivot: [f32; 2],
    pub pivot_uv: [f32; 2],
    pub joint_radius_px: u32,
    pub completed_overlap_pixels: u32,
    pub rotation: [f32; 2],
    pub crop_box: [u32; 4],
    pub crop_size: [u32; 2],
    pub cropped_path: PathBuf,
    pub full_layer_path: PathBuf,
}

/// A parsed `{character_id}_cutout_rig_plan.json` produced by `guided_cutout_rig_builder.py`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoadedCutoutRigPlan {
    #[serde(default)]
    pub control_bindings: crate::control_bindings::ControlBindings,
    pub plan_path: PathBuf,
    pub character_id: String,
    pub rig_type: String,
    pub source_image: PathBuf,
    pub canvas_size: [u32; 2],
    pub character_bbox: [u32; 4],
    pub photocraft_psd: Option<PathBuf>,
    pub cutout_asset_sheet: Option<PathBuf>,
    pub parts: Vec<CutoutRigPart>,
    pub pose_library: Value,
    pub effectcraft: Value,
}

pub(crate) fn load_control_bindings(
    raw: &Value,
    plan: &Path,
    parts: &[CutoutRigPart],
) -> Result<crate::control_bindings::ControlBindings> {
    let mut bindings: crate::control_bindings::ControlBindings = serde_json::from_value(
        raw.get("control_bindings")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
    )
    .map_err(|e| EngineError::InvalidParams(format!("invalid control_bindings: {e}")))?;
    // Older plans explicitly declare a semantic kind. Preserve their face preview
    // only when there is one declared head; never infer it from the filename or position.
    if raw.get("control_bindings").is_none() {
        let mut heads = parts.iter().filter(|part| part.kind == "head");
        if let Some(head) = heads.next()
            && heads.next().is_none()
        {
            bindings.face = Some(crate::control_bindings::FaceBinding {
                part: head.id.clone(),
                landmarks: None,
            });
        }
    }
    bindings
        .validate(&parts.iter().map(|p| p.id.clone()).collect::<Vec<_>>())
        .map_err(EngineError::InvalidParams)?;
    if let Some(face) = &mut bindings.face
        && let Some(path) = &mut face.landmarks
    {
        if path.is_relative() {
            *path = plan.parent().unwrap_or(Path::new(".")).join(&*path);
        }
        if !path.is_file() {
            return Err(EngineError::InvalidParams(format!(
                "face landmarks file does not exist: {}",
                path.display()
            )));
        }
    }
    Ok(bindings)
}

impl LoadedCutoutRigPlan {
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let raw: Value = serde_json::from_str(&text).map_err(|e| {
            EngineError::InvalidParams(format!("invalid cutout rig plan JSON: {e}"))
        })?;
        if let Some(layers) = raw.get("layers").and_then(Value::as_object) {
            return crate::session_layers::load_keyed_layers(path, &raw, layers);
        }
        if let (Some(canvas_arr), Some(layers_arr)) = (
            raw.get("canvas").and_then(Value::as_array),
            raw.get("layers").and_then(Value::as_array),
        ) {
            return crate::session_layers::load_layered_motion_spec(path, &raw, canvas_arr, layers_arr);
        }
        let character_id = raw
            .get("character_id")
            .and_then(Value::as_str)
            .unwrap_or("hopper")
            .to_string();
        let rig_type = raw
            .get("rig_type")
            .and_then(Value::as_str)
            .unwrap_or("cutout_rig")
            .to_string();
        let source_image = raw
            .get("source_image")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .unwrap_or_default();
        let canvas_arr = raw.get("canvas_size").and_then(Value::as_array);
        let canvas_size = [
            canvas_arr
                .and_then(|a| a.first())
                .and_then(Value::as_u64)
                .unwrap_or(1086) as u32,
            canvas_arr
                .and_then(|a| a.get(1))
                .and_then(Value::as_u64)
                .unwrap_or(1448) as u32,
        ];
        let bbox_arr = raw.get("character_bbox").and_then(Value::as_array);
        let character_bbox = [
            bbox_arr
                .and_then(|a| a.first())
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
            bbox_arr
                .and_then(|a| a.get(1))
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
            bbox_arr
                .and_then(|a| a.get(2))
                .and_then(Value::as_u64)
                .unwrap_or(canvas_size[0] as u64) as u32,
            bbox_arr
                .and_then(|a| a.get(3))
                .and_then(Value::as_u64)
                .unwrap_or(canvas_size[1] as u64) as u32,
        ];
        let photocraft_psd = raw
            .get("photocraft_psd")
            .and_then(Value::as_str)
            .map(PathBuf::from);
        let cutout_asset_sheet = raw
            .get("cutout_asset_sheet")
            .and_then(Value::as_str)
            .map(PathBuf::from);
        let parts: Vec<CutoutRigPart> = raw
            .get("parts")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        let pose_library = raw.get("pose_library").cloned().unwrap_or(Value::Null);
        let effectcraft = raw.get("effectcraft").cloned().unwrap_or(Value::Null);
        let control_bindings = load_control_bindings(&raw, path, &parts)?;
        Ok(Self {
            control_bindings,
            plan_path: path.to_path_buf(),
            character_id,
            rig_type,
            source_image,
            canvas_size,
            character_bbox,
            photocraft_psd,
            cutout_asset_sheet,
            parts,
            pose_library,
            effectcraft,
        })
    }

    pub fn resolve_paths(&mut self, base: &Path) {
        let resolve = |p: &Path| {
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                base.join(p)
            }
        };
        if !self.source_image.as_os_str().is_empty() {
            self.source_image = resolve(&self.source_image);
        }
        self.cutout_asset_sheet = self.cutout_asset_sheet.as_deref().map(resolve);
        self.photocraft_psd = self.photocraft_psd.as_deref().map(resolve);
        for part in &mut self.parts {
            part.cropped_path = resolve(&part.cropped_path);
            part.full_layer_path = resolve(&part.full_layer_path);
        }
    }
}

impl Asset {
    /// Synchronize `self.parts` and attached sheet paths from the loaded pose or turnaround manifest
    /// while preserving any existing visibility/solo toggles for matching part names.
    pub fn sync_parts_from_manifests(&mut self) {
        let mut names = Vec::new();
        if let Some(rig) = &self.cutout_rig {
            append_unique_names(&mut names, rig.parts.iter().map(|p| &p.id));
        }
        if let Some(pm) = self.pose_manifest.clone() {
            if self.attached_asset_sheet.is_none() {
                self.attached_asset_sheet = pm.source_image.clone();
            }
            if let Some(sheet_path) = self.attached_asset_sheet.clone() {
                self.ensure_manifest_reference(
                    "asset_sheet",
                    format!("Pose Sheet ({} poses)", pm.frames.len()),
                    sheet_path,
                    &pm,
                );
            }
            append_unique_names(&mut names, pm.part_names.iter());
        }
        if let Some(tm) = self.turnaround_manifest.clone() {
            if self.attached_turnaround_sheet.is_none() {
                self.attached_turnaround_sheet = tm.source_image.clone();
            }
            if let Some(sheet_path) = self.attached_turnaround_sheet.clone() {
                self.ensure_manifest_reference(
                    "turnaround_sheet",
                    format!("Turnaround Sheet ({} views)", tm.frames.len()),
                    sheet_path,
                    &tm,
                );
            }
            append_unique_names(&mut names, tm.part_names.iter());
        }
        let existing: HashMap<String, (bool, bool, bool)> = self
            .parts
            .iter()
            .map(|p| (p.name.clone(), (p.visible, p.solo, p.verified)))
            .collect();
        self.parts = names
            .into_iter()
            .map(|name| {
                let (visible, solo, verified) =
                    existing.get(&name).copied().unwrap_or((true, false, false));
                PartState {
                    name,
                    visible,
                    solo,
                    verified,
                }
            })
            .collect();
    }

    fn ensure_manifest_reference(
        &mut self,
        kind: &str,
        label: String,
        path: PathBuf,
        manifest: &LoadedManifest,
    ) {
        if self.attached_references.iter().any(|r| r.kind == kind) {
            return;
        }
        let [width, height] = manifest.source_size.unwrap_or([1376, 768]);
        self.attached_references.push(AttachedReference {
            label,
            kind: kind.to_string(),
            path,
            raw_path: manifest.source_image.clone(),
            width,
            height,
            report: None,
        });
    }
}

fn append_unique_names<'a>(names: &mut Vec<String>, iter: impl Iterator<Item = &'a String>) {
    for name in iter {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
}

impl Session {
    /// Record the result of the last executed command.
    pub fn record(&mut self, value: Value) {
        self.last_result = Some(value);
    }

    /// Return a reference to the currently active [`Asset`], if any.
    pub fn current_asset(&self) -> Option<&Asset> {
        let id = self.active_asset.as_deref()?;
        self.assets.get(id)
    }

    /// Return a mutable reference to the currently active [`Asset`], if any.
    pub fn current_asset_mut(&mut self) -> Option<&mut Asset> {
        let id = self.active_asset.clone()?;
        self.assets.get_mut(&id)
    }
}