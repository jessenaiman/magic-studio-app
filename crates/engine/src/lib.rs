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
pub mod animation_jobs;
pub mod change_marker;
pub mod character;
pub mod command_center;
pub mod commands;
pub mod command_catalog;
pub mod complexity;
pub mod control_bindings;
pub mod crew;
pub mod doctor;
pub mod exports;
pub mod image_cmds;
pub mod image_generation;
pub mod lyrics;
pub mod mesh_geometry;
pub mod motion;
pub mod motion_authoring;
pub mod panels;
pub mod part_replacement;
pub mod parts;
pub mod magic_design;
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
pub mod theatre;
pub mod turnaround;
pub mod video_target;
pub mod vision;
pub mod words;
mod words_compose;
mod words_stage;
mod words_validate;
pub mod workshop;
pub mod workshop_fixlist;
pub mod workshop_intake;
pub mod workshop_transcript;
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

/// The writer lease a journaled command needs, except for the project lifecycle calls.
fn writer_lease(
    s: &Session,
    id: &str,
    project_edit: bool,
) -> Result<Option<std::sync::Arc<std::fs::File>>> {
    let lifecycle = matches!(
        id,
        "project.open" | "project.close" | "project.new" | "project.intake"
    );
    if !project_edit || lifecycle {
        return Ok(None);
    }
    match s.project_path.as_deref() {
        Some(path) => Ok(Some(project::claim(s, path)?)),
        None => Ok(None),
    }
}

fn announce_dispatch(s: &Session, id: &str, params: &Value) {
    activity::command(
        s.project_path.as_deref(),
        id,
        params,
        "dispatch_running",
        None,
    );
}

/// The stage a finished command is recorded under.
fn outcome_stage(value: &Value) -> &'static str {
    if value["ok"] == false {
        "engine_error"
    } else if value["intake_ready"] == false {
        "engine_needs_work"
    } else {
        "engine_success"
    }
}

fn announce_result(s: &Session, id: &str, params: &Value, result: &Result<Value>) {
    match result {
        Ok(v) => activity::command(s.project_path.as_deref(), id, params, outcome_stage(v), None),
        Err(e) => activity::command(
            s.project_path.as_deref(),
            id,
            params,
            "engine_error",
            Some(&e.to_string()),
        ),
    }
}

/// An edit is a successful journaled command that left the project file unwritten.
fn note_edit(
    s: &mut Session,
    project_edit: bool,
    result: &Result<Value>,
    saves_before: u64,
    project_before: Option<&std::path::PathBuf>,
) {
    if !(project_edit && result.as_ref().is_ok_and(|v| v["ok"] != false)) {
        return;
    }
    if s.project_saves
        .load(std::sync::atomic::Ordering::Relaxed)
        != saves_before
        || s.project_path.as_ref() != project_before
    {
        s.edits_since_save = 0;
    } else {
        s.edits_since_save += 1;
    }
}

/// Save As transfers ownership to a new path; never restore the source lease over it.
fn restore_lease(
    s: &mut Session,
    prior: Option<std::sync::Arc<std::fs::File>>,
    had_writer: bool,
    project_before: Option<&std::path::PathBuf>,
) {
    if had_writer && s.project_path.as_ref() == project_before {
        s.project_owner = prior;
    }
}

/// The outliner name a call asked for, else the asset id read as words.
fn outliner_name(params: &Value, asset_id: &str) -> String {
    params
        .get("name")
        .or_else(|| params.get("asset"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| asset_id.replace('_', " "))
}

/// A validated intake keeps the asset's own path; a loaded manifest takes the one it names.
fn outliner_path(
    s: &Session,
    id: &str,
    params: &Value,
    asset_path: std::path::PathBuf,
) -> Option<std::path::PathBuf> {
    if id == "intake.validate" && !asset_path.as_os_str().is_empty() {
        return Some(asset_path);
    }
    params
        .get("manifest")
        .or_else(|| params.get("image"))
        .and_then(Value::as_str)
        .map(|path| commands::resolve_workspace_path(s, path))
}

/// Update the entry the outliner already has, else add one for this call.
fn connect_asset_entry(
    s: &mut Session,
    id: &str,
    asset_id: String,
    name: String,
    path: Option<std::path::PathBuf>,
    params: &Value,
) {
    if let Some(existing) = s
        .project_assets
        .iter_mut()
        .find(|entry| entry.id == asset_id)
    {
        if id == "intake.validate" && path.is_some() {
            existing.path = path;
        }
        return;
    }
    s.project_assets.push(ProjectAsset {
        id: asset_id,
        name,
        preset: params
            .get("preset")
            .and_then(Value::as_str)
            .map(str::to_owned),
        path,
        kind: "auto".into(),
        parts: Vec::new(),
    });
}

/// Older import/load callers also appear in the shared project outliner.
fn connect_outliner_asset(s: &mut Session, id: &str, params: &Value) {
    if !matches!(id, "intake.validate" | "asset.load_manifest") {
        return;
    }
    let Some(asset) = s.current_asset() else {
        return;
    };
    let asset_id = asset.id.clone();
    let asset_path = asset.path.clone();
    let name = outliner_name(params, &asset_id);
    let path = outliner_path(s, id, params, asset_path);
    connect_asset_entry(s, id, asset_id, name, path, params);
}

/// Keep the open project file in step with its asset connections.
fn sync_project_file(
    s: &mut Session,
    spec: &CommandSpec,
    id: &str,
    assets_before: Option<&Value>,
) -> Result<()> {
    if !spec.journal
        || matches!(id, "project.open" | "project.new" | "project.close")
        || s.project_path.is_none()
    {
        return Ok(());
    }
    if serde_json::to_value(&s.project_assets).ok().as_ref() != assets_before {
        commands::write_project_file(s)?;
    }
    Ok(())
}

impl Session {
    /// Execute one command by id with JSON parameters, returning JSON.
    pub fn execute(&mut self, id: &str, params: Value) -> Result<Value> {
        let _context = activity::enter(id, &params);
        // Cast writes are journaled workshop files, independent of the open song's writer lease.
        let project_edit = commands::find(id).is_some_and(|c| c.journal) && id != "crew.save";
        let _writer = writer_lease(self, id, project_edit)?;
        let prior_owner = self.project_owner.clone();
        if let Some(owner) = &_writer {
            self.project_owner = Some(owner.clone());
        }
        let record = self.record_user_state && commands::find(id).is_none_or(|c| c.journal);
        if record {
            announce_dispatch(self, id, &params);
        }
        let started = std::time::Instant::now();
        let (saves_before, project_before) = (
            self.project_saves
                .load(std::sync::atomic::Ordering::Relaxed),
            self.project_path.clone(),
        );
        let result = self.execute_inner(id, params.clone());
        note_edit(
            self,
            project_edit,
            &result,
            saves_before,
            project_before.as_ref(),
        );
        if record && let Ok(value) = &result {
            activity::result(
                self.project_path.as_deref(),
                id,
                value,
                started.elapsed().as_millis(),
            );
        }
        if record {
            announce_result(self, id, &params, &result);
        }
        restore_lease(self, prior_owner, _writer.is_some(), project_before.as_ref());
        result
    }
    fn execute_inner(&mut self, id: &str, params: Value) -> Result<Value> {
        let spec = commands::find(id).ok_or_else(|| EngineError::UnknownCommand(id.to_string()))?;
        check_param_types(spec.params, &params)?;
        (spec.enabled)(self).map_err(|e| EngineError::CommandFailed(id.to_string(), e))?;
        let assets_before = serde_json::to_value(&self.project_assets).ok();
        let out = (spec.run)(self, &params)?;
        connect_outliner_asset(self, id, &params);
        sync_project_file(self, spec, id, assets_before.as_ref())?;
        if spec.journal {
            self.record(out.clone());
        }
        Ok(out)
    }
}

pub mod local_ipc;

pub mod project_catalog;
pub mod studio_sessions;
