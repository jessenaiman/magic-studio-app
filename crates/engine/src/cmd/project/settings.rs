//! Production project root & user settings backed solely by `config.toml` (`craft-settings`).
//!
//! Reference: <https://github.com/storytold/craft-libs/blob/e653048a187036d30fd912f35f14b32ab4084e5c/docs/settings-storage.md>

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde_json::{Value, json};

use crate::settings::MagicPrefs;
use crate::settings::prefs::open_store;
use crate::{EngineError, Result, Session};

fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

fn absolute(value: &str) -> Result<PathBuf> {
    let path = Path::new(value.trim());
    if value.trim().is_empty() || !path.is_absolute() || value.chars().any(char::is_control) {
        return Err(invalid("Project root must be a nonempty absolute directory path"));
    }
    Ok(path.to_path_buf())
}

fn configured_root(prefs: &MagicPrefs) -> Result<PathBuf> {
    if let Some(ref root) = prefs.project.root {
        return absolute(root);
    }
    crate::workspace::user_data_dir()
        .map(|root| root.join("magic/projects"))
        .ok_or_else(|| invalid("Project root unavailable; configure XDG_DATA_HOME or HOME"))
}

pub fn root() -> Result<PathBuf> {
    let store = open_store();
    configured_root(store.get())
}

/// Read the saved Magic Studio settings object from `config.toml` (`craft-settings`).
pub(crate) fn stored_settings() -> Value {
    let store = open_store();
    let prefs = store.get();
    let mut val = serde_json::to_value(prefs).unwrap_or_else(|_| json!({}));
    if let Some(ref root) = prefs.project.root {
        val["projects_root"] = json!(root);
    }
    if let Some(ref chars) = prefs.project.characters_root {
        val["characters_root"] = json!(chars);
    }
    val
}

/// A folder path saved under `key` (`projects_root` or `characters_root`) in `config.toml`; GUI
/// windows launched by the desktop do not inherit a terminal's environment, so this is where they look.
pub fn saved_path(key: &str) -> Option<PathBuf> {
    let store = open_store();
    let prefs = store.get();
    match key {
        "projects_root" | "project.root" => prefs.project.root.as_deref().map(PathBuf::from),
        "characters_root" | "project.characters_root" => prefs.project.characters_root.as_deref().map(PathBuf::from),
        _ => stored_settings().get(key)?.as_str().map(PathBuf::from),
    }
}

/// Optional external character library. Its location belongs to the user's configuration (`config.toml`).
pub fn characters_root() -> Option<PathBuf> {
    std::env::var_os("magic_CHARACTERS_ROOT")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| saved_path("characters_root"))
        .filter(|path| path.is_absolute())
}

fn persist_root(value: &Value) -> Result<()> {
    let root = absolute(value.as_str().ok_or_else(|| invalid("Project root must be a string"))?)?;
    if !root.is_dir() {
        return Err(invalid("Project root directory is unavailable; mount or choose the intended directory first"));
    }
    let root = fs::canonicalize(root)?;
    let mut store = open_store();
    store.update(|prefs| {
        prefs.project.root = Some(root.to_string_lossy().into_owned());
    });
    store.flush().map_err(|e| invalid(format!("Failed to persist config.toml: {e}")))?;
    Ok(())
}

/// Without `root`, inspect the setting. Only this explicit command changes it.
pub fn command(_: &mut Session, params: &Value) -> Result<Value> {
    if let Some(value) = params.get("root") {
        persist_root(value)?;
    }
    let store = open_store();
    let prefs = store.get();
    let persisted = prefs.project.root.is_some();
    Ok(json!({
        "root": configured_root(prefs)?,
        "config": store.path(),
        "persisted": persisted,
        "source": if persisted { "saved production setting" } else { "production default" },
        "overrides": "project.new root= or path= applies only to that request; recent history never changes this setting",
    }))
}
