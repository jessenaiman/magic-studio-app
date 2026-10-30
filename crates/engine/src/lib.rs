//! Shared project state and command dispatch for Magic Studio's creative workspace.
//!
//! The desktop Tauri shell, CLI, and MCP clients share the same command registry
//! and [`Session::execute`] entry point. This crate owns project state and domain
//! validation; frontends should not implement a second persistence model.
//!
//! # Start here
//!
//! - [`Session`] holds the selected project, connected assets, and measurements.
//! - [`command_specs`] and [`CommandSpec`] expose stable command IDs, parameter
//!   descriptions, enablement, journaling, and long-running metadata.
//! - [`find_command`] locates a command; [`Session::execute`] checks parameters
//!   and enablement before dispatching its handler and recording applicable edits.
//! - [`EngineError`] describes dispatch, validation, I/O, and external-tool failures.
//!
//! Inspect the registry without opening a project or invoking a production tool:
//!
//! ```no_run
//! use magicstudio_engine::{Session, command_specs, find_command};
//!
//! let session = Session::default();
//! for command in command_specs() {
//!     println!("{}: {}", command.id, command.params);
//! }
//! if let Some(command) = find_command("project.open") {
//!     println!("{} enabled: {:?}", command.label, (command.enabled)(&session));
//! }
//! ```
//!
//! # Responsibility map
//!
//! | Area | Modules |
//! |---|---|
//! | Project lifecycle, paths, settings | [`project`], [`workspace`], [`project_settings`] |
//! | Storyboard and song workspace | [`storyboard`], [`storyboard_crud`], [`storyboard_design`], [`workshop`] |
//! | Character, rig, motion, sequence | [`character`], [`rig`], [`motion`], [`motion_authoring`], [`sequence`] |
//! | Lyrics, timing, native animation sources | [`lyrics`], [`words`], [`theatre`], [`animation_links`] |
//! | Production adapters and exports | [`exports`], [`blender`], [`image_generation`], [`agent`] |
//!
//! Commands may write project files, launch installed production tools, or call
//! configured providers. A default session is not a sandbox for arbitrary command
//! execution. Discover the command contract and use explicit project selection.
//! Backstage is a separate observer binary, not an engine dependency or controller.
//!
//! For the non-Rust layers and existing diagrams, see the repository's
//! [documentation index](https://github.com/jessenaiman/magic-studio-app/blob/HEAD/docs/README.md)
//! and [architecture guide](https://github.com/jessenaiman/magic-studio-app/blob/HEAD/docs/architecture/README.md).
//! Repository links track the default branch; new guides appear after this
//! documentation change merges.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unimplemented,
        clippy::todo,
        clippy::unreachable
    )
)]

pub mod activity;
pub mod agent;
pub mod animation_links;
pub(crate) mod audio_trace;
pub mod blender;
pub mod change_marker;
pub mod character;
pub mod command_center;
pub mod commands;
pub mod complexity;
pub mod control_bindings;
pub mod crew;
pub mod doctor;
pub mod exports;
pub mod image_cmds;
pub mod image_generation;
pub mod lyrics;
pub mod mesh_geometry;
pub mod mlt;
pub mod motion;
pub mod motion_authoring;
pub mod opentoonz;
pub mod panels;
pub mod part_replacement;
pub mod parts;
pub mod photocraft;
pub mod project;
pub mod project_settings;
pub(crate) mod read_only_file;
pub mod rig;
pub mod sequence;
pub mod session;
pub mod sheets;
pub mod skills_registry;
pub mod storyboard;
pub mod storyboard_crud;
pub mod storyboard_design;
mod storyboard_document;
pub mod studio_cues;
pub mod synfig;
pub mod theatre;
pub mod turnaround;
pub mod video_target;
pub mod vision;
pub mod words;
mod words_compose;
mod words_stage;
pub mod workshop;
pub mod workspace;

pub use commands::{CommandSpec, command_specs, find as find_command};
pub use session::{
    Asset, AttachedReference, LoadedManifest, ManifestFrame, MeasuredMotionRange,
    MeasuredTurnaroundRegistration, PartState, PoseMotionMetrics, ProjectAsset, Session,
};

use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("unknown command `{0}`")]
    UnknownCommand(String),
    #[error("command `{0}` failed: {1}")]
    CommandFailed(String, String),
    #[error("invalid parameters: {0}")]
    InvalidParams(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("external tool `{0}` failed: {1}")]
    ExternalTool(String, String),
    #[error("not yet implemented")]
    NotImplemented,
}

pub type Result<T> = std::result::Result<T, EngineError>;

/// Rejects a parameter whose JSON type differs from the example value in the command's `params`
/// description (e.g. `"auto_cut": "true"` where the description says `false`). Every caller (CLI, MCP,
/// app) goes through `Session::execute`, so a mistyped value fails the same way everywhere
/// instead of a handler silently falling back to its default.
pub fn check_param_types(description: &str, params: &Value) -> Result<()> {
    let (Ok(Value::Object(expected)), Some(given)) = (
        serde_json::from_str::<Value>(description),
        params.as_object(),
    ) else {
        return Ok(());
    };
    for (key, value) in given {
        let wanted = match expected.get(key) {
            Some(Value::Bool(_)) => "boolean",
            Some(Value::Number(_)) => "number",
            _ => continue,
        };
        let ok = if wanted == "boolean" {
            value.is_boolean()
        } else {
            value.is_number()
        };
        if !ok {
            return Err(EngineError::InvalidParams(format!(
                "`{key}` must be a {wanted}, got {value}"
            )));
        }
    }
    Ok(())
}

impl Session {
    /// Execute one command by id with JSON parameters, returning JSON.
    pub fn execute(&mut self, id: &str, params: Value) -> Result<Value> {
        let _context = activity::enter(id, &params);
        // Cast writes are journaled workshop files, independent of the open song's writer lease.
        let project_edit = commands::find(id).is_some_and(|c| c.journal) && id != "crew.save";
        let _writer = if project_edit
            && !matches!(
                id,
                "project.open" | "project.close" | "project.new" | "project.intake"
            ) {
            self.project_path
                .as_deref()
                .map(|path| project::claim(self, path))
                .transpose()?
        } else {
            None
        };
        let prior_owner = self.project_owner.clone();
        if let Some(owner) = &_writer {
            self.project_owner = Some(owner.clone());
        }
        let record = self.record_user_state && commands::find(id).is_none_or(|c| c.journal);
        if record {
            activity::command(
                self.project_path.as_deref(),
                id,
                &params,
                "dispatch_running",
                None,
            );
        }
        let started = std::time::Instant::now();
        let (saves_before, project_before) = (
            self.project_saves
                .load(std::sync::atomic::Ordering::Relaxed),
            self.project_path.clone(),
        );
        let result = self.execute_inner(id, params.clone());
        // An edit is a successful journaled command that left the project file unwritten.
        if result.as_ref().is_ok_and(|v| v["ok"] != false) && project_edit {
            if self
                .project_saves
                .load(std::sync::atomic::Ordering::Relaxed)
                != saves_before
                || self.project_path != project_before
            {
                self.edits_since_save = 0;
            } else {
                self.edits_since_save += 1;
            }
        }
        if record && let Ok(value) = &result {
            activity::result(
                self.project_path.as_deref(),
                id,
                value,
                started.elapsed().as_millis(),
            );
        }
        if record {
            match &result {
                Ok(v) => activity::command(
                    self.project_path.as_deref(),
                    id,
                    &params,
                    if v["ok"] == false {
                        "engine_error"
                    } else if v["intake_ready"] == false {
                        "engine_needs_work"
                    } else {
                        "engine_success"
                    },
                    None,
                ),
                Err(e) => activity::command(
                    self.project_path.as_deref(),
                    id,
                    &params,
                    "engine_error",
                    Some(&e.to_string()),
                ),
            };
        }
        // Save As transfers ownership to a new path; never restore the source lease over it.
        if _writer.is_some() && self.project_path == project_before {
            self.project_owner = prior_owner;
        }
        result
    }
    fn execute_inner(&mut self, id: &str, params: Value) -> Result<Value> {
        let spec = commands::find(id).ok_or_else(|| EngineError::UnknownCommand(id.to_string()))?;
        check_param_types(spec.params, &params)?;
        (spec.enabled)(self).map_err(|e| EngineError::CommandFailed(id.to_string(), e))?;
        let assets_before = serde_json::to_value(&self.project_assets).ok();
        let out = (spec.run)(self, &params)?;
        // Older import/load callers also appear in the shared project outliner.
        if matches!(id, "intake.validate" | "asset.load_manifest")
            && let Some(asset) = self.current_asset()
        {
            let asset_id = asset.id.clone();
            let asset_path = asset.path.clone();
            let display_name = params
                .get("name")
                .or_else(|| params.get("asset"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| asset_id.replace('_', " "));
            let path = if id == "intake.validate" && !asset_path.as_os_str().is_empty() {
                Some(asset_path)
            } else {
                params
                    .get("manifest")
                    .or_else(|| params.get("image"))
                    .and_then(Value::as_str)
                    .map(|path| commands::resolve_workspace_path(self, path))
            };
            if let Some(existing) = self
                .project_assets
                .iter_mut()
                .find(|entry| entry.id == asset_id)
            {
                if id == "intake.validate" && path.is_some() {
                    existing.path = path;
                }
            } else {
                self.project_assets.push(ProjectAsset {
                    id: asset_id,
                    name: display_name,
                    preset: params
                        .get("preset")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    path,
                    kind: "auto".into(),
                    parts: Vec::new(),
                });
            }
        }
        // Keep the open project file in step with its asset connections.
        if spec.journal
            && !matches!(id, "project.open" | "project.new" | "project.close")
            && self.project_path.is_some()
            && serde_json::to_value(&self.project_assets).ok() != assets_before
        {
            commands::write_project_file(self)?;
        }
        if spec.journal {
            self.record(out.clone());
        }
        Ok(out)
    }
}

pub mod local_ipc;

pub mod project_catalog;
pub mod studio_sessions;
