//! The command registry: every user-visible action is a command with a stable id
//! (`domain.verb`) and JSON params, dispatched through [`crate::Session::execute`].
//!
//! Command handlers live in focused modules by job (declared in `lib.rs`):
//!   - `workspace` — path/CLI resolution
//!   - `project` — .magic lifecycle + connect/load/remove/update assets
//!   - `workshop` — workshop/storyboard/video/assets/audio + search
//!   - `storyboard`, `storyboard_crud`, `storyboard_design` — storyboard authoring
//!   - `lyrics`, `words`, `lettering` — timed words and kinetic typography
//!   - `video_target`, `assembly` — HyperFrames preview/render and FilmCraft assembly
//!
//! This file keeps only the `CommandSpec` registry, `command_specs()`, `find()`,
//! `check_param_types()`, and the `build()` array wiring every id to its handler.

use std::sync::OnceLock;

use serde_json::Value;

use crate::{EngineError, Result, Session};

// Re-export the public engine surface that other crates address via `crate::commands::*`.
pub use crate::project::write_project_file;
pub use crate::workspace::{
    asset_id_for_import, resolve_tool, resolve_workspace_path, sanitize_asset_id, workspace_root,
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
        | "app.errors"
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
        | "animation.review"
        | "character.list"
        | "character.actions"
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
        "app.analytics" => {
            params.get("scan").and_then(Value::as_bool) != Some(true)
                && params.get("judge").and_then(Value::as_bool) != Some(true)
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn explicit_analytics_judgment_is_not_a_dry_run_safe_query() {
        assert!(is_read_only_command("app.analytics", &json!({})));
        assert!(is_read_only_command(
            "app.analytics",
            &json!({"judge":false,"scan":false})
        ));
        assert!(!is_read_only_command(
            "app.analytics",
            &json!({"judge":true})
        ));
        assert!(!is_read_only_command(
            "app.analytics",
            &json!({"scan":true})
        ));
    }
}
