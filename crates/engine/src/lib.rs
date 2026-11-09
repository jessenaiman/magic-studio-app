//! Shared project state and command dispatch for Magic Studio's creative workspace.
//!
//! The desktop Tauri shell and CLI clients share the same command registry
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
//! use magic::{Session, command_specs, find_command};
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
//! | Lyrics, timing, lettering | [`lyrics`], [`words`], [`lettering`] |
//! | Video assembly, generation, agents | [`assembly`], [`video_target`], [`image_generation`], [`agent`] |
//!
//! Commands may write project files, launch installed production tools, or call
//! configured providers. A default session is not a sandbox for arbitrary command
//! execution. Discover the command contract and use explicit project selection.
//! Backstage is a separate observer binary, not an engine dependency or controller.
//!
//! For the non-Rust layers and existing diagrams, see the repository's
//! [documentation index](https://github.com/jessenaiman/magic-studio-app/blob/HEAD/docs/README.md)
//! and [rustdoc guide](https://github.com/jessenaiman/magic-studio-app/blob/HEAD/docs/RUSTDOC.md).
//! Repository links track the default branch; new guides appear after this
//! documentation change merges.

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

pub mod activity;
pub mod agent;
mod agent_run;
pub mod change_marker;
pub mod cmd;
pub mod command_sets;
pub mod commands;
pub mod craft_ai_codex;
pub mod craft_hub;
mod creation_loop;
pub mod crew;
pub mod doc_state;
pub mod doctor;
pub mod error;
pub mod feedback_state;
pub mod fidget;
pub mod guard;
pub mod headless;
pub mod herdr;
pub mod history;
pub mod image_generation;
pub mod link_watch;
mod mayhem;
pub mod panels;
pub mod project_lock;
pub(crate) mod read_only_file;
pub mod scribbles;
pub mod session;
pub mod settings;
pub mod skills_registry;
pub mod time;
pub mod update;
pub mod workspace;

pub use commands::{CommandSpec, command_specs, find as find_command};
pub use craft_hub::{CraftHub, CraftTarget};
pub use doc_state::write_file;
pub use error::{EngineError, Result};
pub use session::{Asset, ProjectAsset, Session};
// Backwards-compatible re-exports for the project domain now under `cmd::project`.
pub use cmd::project;
pub use cmd::project::catalog as project_catalog;
pub use cmd::project::intake as workshop_intake;
pub use cmd::project::settings as project_settings;
pub use cmd::project::transcript as workshop_transcript;
pub use cmd::project::workshop;

use serde_json::Value;

/// Rejects a parameter whose JSON type differs from the example value in the command's `params`
/// description (e.g. `"auto_cut": "true"` where the description says `false`). Every caller (CLI,
/// app) goes through `Session::execute`, so a mistyped value fails the same way everywhere
/// instead of a handler silently falling back to its default.
pub fn check_param_types(description: &str, params: &Value) -> Result<()> {
    let (Ok(Value::Object(expected)), Some(given)) = (serde_json::from_str::<Value>(description), params.as_object()) else {
        return Ok(());
    };
    for (key, value) in given {
        let wanted = match expected.get(key) {
            Some(Value::Bool(_)) => "boolean",
            Some(Value::Number(_)) => "number",
            _ => continue,
        };
        let ok = if wanted == "boolean" { value.is_boolean() } else { value.is_number() };
        if !ok {
            return Err(EngineError::InvalidParams(format!("`{key}` must be a {wanted}, got {value}")));
        }
    }
    Ok(())
}
