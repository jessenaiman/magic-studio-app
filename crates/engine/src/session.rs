//! Session state for Magic Studio.
//!
//! Holds the current project and its registered asset files, and provides
//! [`Session::execute`] for command dispatch. Pixel and motion work belongs to the
//! installed apps; the session only records which files a project uses.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    /// Last command result, echoed back for inspection.
    pub last_result: Option<Value>,
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
            project_mode: None,
            record_user_state: false,
            agent_selection: None,
            project_owner: None,
            song_audio: None,
            draft_approved: false,
            video_target: "hyperframes".into(),
            active_asset: None,
            last_result: None,
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
            project_mode: self.project_mode.clone(),
            record_user_state: self.record_user_state,
            agent_selection: self.agent_selection.clone(),
            project_owner: self.project_owner.clone(),
            song_audio: self.song_audio.clone(),
            draft_approved: self.draft_approved,
            video_target: self.video_target.clone(),
            active_asset: self.active_asset.clone(),
            last_result: self.last_result.clone(),
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

/// A file registered with the project: an image, or an app document (`.pcraft`, `.ecproj`, ...).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAsset {
    pub id: String,
    pub name: String,
    pub preset: Option<String>,
    pub path: Option<PathBuf>,
    /// `image` or the owning app's document kind.
    pub kind: String,
}

/// A loaded image asset and its decoded size.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: String,
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
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
