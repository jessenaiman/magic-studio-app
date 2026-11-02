//! The command registry: every user-visible action is a command with a stable id
//! (`domain.verb`) and JSON params, dispatched through [`crate::Session::execute`].
//!
//! Command handlers live in focused modules by job (declared in `lib.rs`):
//!   - `workspace` — path/CLI resolution
//!   - `parts` — part visibility/solo/review
//!   - `turnaround` — turnaround accuracy + 2D preview
//!   - `magic_design` — asset intake, sheet preparation, manifest loading, agent prompt
//!   - `rig` — 4-dir mesh build + GLB/OBJ writers
//!   - `exports` — Blender/FilmCraft/cutout MP4/layered motion/validate
//!   - `image_cmds` — break apart / puppet warp / color to alpha / content-aware fill
//!   - `vision` — in-memory pixel analysis
//!   - `project` — .magic lifecycle + connect/load/remove/update assets
//!   - `workshop` — workshop/storyboard/video/assets/audio + search
//!
//! This file keeps only the `CommandSpec` registry, `command_specs()`, `find()`,
//! `check_param_types()`, and the `build()` array wiring every id to its handler.

use std::sync::OnceLock;

use serde_json::Value;

use crate::{EngineError, Result, Session};

// Re-export the public engine surface that other crates address via `crate::commands::*`.
pub use crate::magic_design::decontaminate_sprite_rgba;
pub use crate::project::write_project_file;
pub use crate::vision::{analyze_rgba_image, analyze_viewport_region};
pub use crate::workspace::{
    asset_id_for_import, image_batch_args,
    resolve_craft_cli, resolve_workspace_path, sanitize_asset_id, workspace_root,
};

pub struct CommandSpec {
    pub id: &'static str,
    pub label: &'static str,
    /// Menu placement, e.g. `["File", "New"]`. Empty = not in menus.
    pub menu: &'static [&'static str],
    /// Default shortcut (egui-style names, `Cmd` = ⌘ on macOS / Ctrl elsewhere).
    pub shortcut: Option<&'static str>,
    /// Parameter description (JSON-ish, for agents and the MCP schema).
    pub params: &'static str,
    /// One sentence describing behavior and the next useful operation.
    pub help: &'static str,
    /// A runnable CLI invocation using explicit example paths and IDs.
    pub example: &'static str,
    pub enabled: fn(&Session) -> std::result::Result<(), String>,
    pub run: fn(&mut Session, &Value) -> Result<Value>,
    /// Record in the journal (false for pure queries).
    pub journal: bool,
    /// Runs external CLI tools or heavy image processing; UI dispatches on a background thread.
    pub long_running: bool,
}

impl CommandSpec {
    pub(crate) fn help(mut self, help: &'static str, example: &'static str) -> Self {
        self.help = help;
        self.example = example;
        self
    }

    pub fn describe(&self) -> Value {
        serde_json::json!({"id":self.id,"label":self.label,"menu":self.menu,
            "shortcut":self.shortcut,"params":self.params,"journal":self.journal,
            "long_running":self.long_running,"help":self.help,"example":self.example})
    }
}

pub fn is_read_only_command(id: &str, params: &Value) -> bool {
    match id {
        "project.catalog" => params.get("forget").is_none_or(Value::is_null),
        "project.attach"
        | "backstage.observe"
        | "backstage.changes"
        | "project.identity"
        | "project.list_assets"
        | "project.search"
        | "workshop.status"
        | "workshop.files"
        | "workshop.storyboard"
        | "storyboard.read"
        | "storyboard.design"
        | "storyboard.backgrounds"
        | "storyboard.toc"
        | "storyboard.template"
        | "storyboard.doctor"
        | "character.assets"
        | "assets.sheet_plan"
        | "assets.sheet_info"
        | "assets.assess"
        | "assets.fixlist"
        | "motion.tracks"
        | "motion.clips"
        | "motion.sample"
        | "motion.theatre.read"
        | "validate.motion"
        | "animation.sources"
        | "animation.status"
        | "animation.list"
        | "animation.scene.actions"
        | "song.lyrics"
        | "words.align"
        | "words.read"
        | "words.studio_track"
        | "words.effects"
        | "words.track"
        | "skills.list"
        | "skills.info"
        | "crew.list"
        | "agent.session"
        | "agent.models"
        | "command.list"
        | "studio.cues"
        | "vision.analyze"
        | "ui.state"
        | "ui.feedback"
        | "ui.elements"
        | "ui.components"
        | "ui.controls"
        | "ui.unconfigured" => true,
        "project.root" => params.get("root").is_none_or(Value::is_null),
        "skills.limits" => {
            params.get("max_words").is_none_or(Value::is_null)
                && params.get("max_steps").is_none_or(Value::is_null)
        }
        "skills.diagram" => params.get("render").and_then(Value::as_bool) != Some(true),
        "agent.log" => params.get("refresh").and_then(Value::as_bool) != Some(true),
        "app.analytics" => params.get("scan").and_then(Value::as_bool) != Some(true),
        "app.doctor" => params.get("clean").and_then(Value::as_bool) != Some(true),
        _ => false,
    }
}

pub fn command_specs() -> &'static [CommandSpec] {
    static SPECS: OnceLock<Vec<CommandSpec>> = OnceLock::new();
    SPECS.get_or_init(build)
}

pub fn find(id: &str) -> Option<&'static CommandSpec> {
    command_specs().iter().find(|c| c.id == id)
}

// ---------- enablement ----------

pub(crate) fn always(_: &Session) -> std::result::Result<(), String> {
    Ok(())
}

pub(crate) fn has_asset(s: &Session) -> std::result::Result<(), String> {
    if s.assets.is_empty() {
        Err("no asset loaded".into())
    } else {
        Ok(())
    }
}

/// Rejects a parameter whose JSON type differs from the example value in the command's `params`
/// description. Every caller (CLI, MCP, app) goes through `Session::execute`, so a
/// mistyped value fails the same way everywhere.
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

// ---------- command registry (every id maps to one handler above) ----------

pub use crate::command_catalog::build;
