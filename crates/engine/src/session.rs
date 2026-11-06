//! Session state for Magic Studio.
//!
//! Holds the current project and its registered asset files, and provides
//! [`Session::execute`] for command dispatch. Pixel and motion work belongs to the
//! installed apps; the session only records which files a project uses.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

mod documents;
mod edit;
mod execute;
pub mod undo;

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
    /// Folder opened directly by the standalone Lyrics editor.
    pub lyrics_workspace: Option<PathBuf>,
    /// Where the final video project lives (HyperFrames/Remotion/FilmCraft). Single source of truth.
    pub video_project_path: Option<PathBuf>,
    /// `Some("animation")` for a character study (Animation only, no song or
    /// storyboard); `None` for a song project.
    pub project_mode: Option<String>,
    /// User-facing sessions publish Home history and recent receipts; passive library/tests do not.
    pub record_user_state: bool,
    /// Active chat selection belongs to this engine/window, never to project serialization.
    pub agent_selection: Option<Value>,
    /// Exclusive lease on the project file or standalone canonical lyric sheet.
    pub project_owner: Option<std::sync::Arc<std::fs::File>>,
    /// The selected recording; uploads use the copy inside the contained HyperFrames project.
    pub song_audio: Option<PathBuf>,
    /// Editable song details, stored only in this project's `.magic` document.
    pub song: crate::cmd::song::song_metadata::SongMetadata,
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
    /// The open project's documents (`session/documents.rs`).
    pub docs: Vec<crate::doc_state::DocState>,
    /// Undo and redo steps over the open documents (`session/undo.rs`).
    pub history: crate::history::History,
    /// The current command's documents as they were before its first edit to each (the undo step).
    pub step_base: Vec<crate::doc_state::DocState>,
}

impl Default for Session {
    fn default() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            cwd,
            assets: HashMap::new(),
            project_assets: Vec::new(),
            project_path: None,
            lyrics_workspace: None,
            video_project_path: None,
            project_mode: None,
            record_user_state: false,
            agent_selection: None,
            project_owner: None,
            song_audio: None,
            song: crate::cmd::song::song_metadata::SongMetadata::default(),
            draft_approved: false,
            video_target: "hyperframes".into(),
            active_asset: None,
            last_result: None,
            project_saves: std::sync::atomic::AtomicU64::new(0),
            edits_since_save: 0,
            docs: Vec::new(),
            history: crate::history::History::default(),
            step_base: Vec::new(),
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
            lyrics_workspace: self.lyrics_workspace.clone(),
            video_project_path: self.video_project_path.clone(),
            project_mode: self.project_mode.clone(),
            record_user_state: self.record_user_state,
            agent_selection: self.agent_selection.clone(),
            project_owner: self.project_owner.clone(),
            song_audio: self.song_audio.clone(),
            song: self.song.clone(),
            draft_approved: self.draft_approved,
            video_target: self.video_target.clone(),
            active_asset: self.active_asset.clone(),
            last_result: self.last_result.clone(),
            project_saves: std::sync::atomic::AtomicU64::new(
                self.project_saves
                    .load(std::sync::atomic::Ordering::Relaxed),
            ),
            edits_since_save: self.edits_since_save,
            docs: self.docs.clone(),
            history: self.history.clone(),
            step_base: Vec::new(),
            // A copy does not own the apps this session opened.
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
    /// Fields written by other versions (for example saved part states), kept as they are.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
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
}
