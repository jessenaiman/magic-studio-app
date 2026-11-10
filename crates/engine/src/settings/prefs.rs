//! Typed application preferences persisted in `config.toml` via [`craft_settings::SettingsStore`].
//!
//! Reference:
//! - <https://github.com/storytold/craft-libs/blob/e653048a187036d30fd912f35f14b32ab4084e5c/crates/settings/src/lib.rs>
//! - <https://github.com/storytold/craft-libs/blob/e653048a187036d30fd912f35f14b32ab4084e5c/docs/settings-storage.md>
//! - <https://github.com/storytold/filmcraft/blob/7322aad72491a294a06a26a63fb65a3fc30ac242/crates/engine/src/settings/prefs.rs>

use std::collections::BTreeMap;

use craft_settings::{Location, Settings, SettingsStore};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::schema::{Kind, fields_with_cat};
use crate::{EngineError, Result};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectPrefs {
    pub root: Option<String>,
    pub characters_root: Option<String>,
    pub storyboard_template: String,
    pub storyboard_format: String,
    pub storyboard_locked: bool,
}

impl Default for ProjectPrefs {
    fn default() -> Self {
        Self { root: None, characters_root: None, storyboard_template: "magic".into(), storyboard_format: "designcraft".into(), storyboard_locked: true }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImagePrefs {
    pub provider: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScribblesPrefs {
    pub template: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MagicPrefs {
    // Per docs/settings-storage.md (verbatim from craft-libs):
    // config.toml = settings people might edit. Window/UI state (theme, scale,
    // startup, tooltips, recent projects) belongs in ui.ron/app.ron beside
    // config.toml, caches in cache_dir, recovery in data_dir, API keys in the
    // OS keychain — none of those fields are in this config.toml anymore.
    pub project: ProjectPrefs,
    pub image: ImagePrefs,
    pub scribbles: ScribblesPrefs,
    pub tools: BTreeMap<String, String>,
}

impl Settings for MagicPrefs {
    const APP_ID: &'static str = "magic";
    const SCHEMA_VERSION: u32 = 1;
}

fn opt_str_json(opt: &Option<String>) -> Value {
    match opt {
        Some(s) => json!(s),
        None => Value::Null,
    }
}

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(msg.into())
}

fn validate_schema_value(key: &str, value: &Value) -> Result<()> {
    let (_, field) = fields_with_cat().into_iter().find(|(_, f)| f.key == key).ok_or_else(|| bad(format!("unknown setting `{key}`; see prefs.schema")))?;
    match field.kind {
        Kind::Bool => {
            if !value.is_boolean() {
                return Err(bad(format!("`{key}` must be a boolean")));
            }
        }
        Kind::Int { min, max, .. } => {
            let n = value.as_i64().ok_or_else(|| bad(format!("`{key}` must be an integer")))?;
            if (n as f64) < min || (n as f64) > max {
                return Err(bad(format!("`{key}` must be between {min} and {max}")));
            }
        }
        Kind::Float { min, max, .. } => {
            let n = value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(format!("`{key}` must be a number")))?;
            if n < min || n > max {
                return Err(bad(format!("`{key}` must be between {min} and {max}")));
            }
        }
        Kind::Choice(choices) => {
            let s = value.as_str().ok_or_else(|| bad(format!("`{key}` must be a string")))?;
            if !choices.iter().any(|(c, _)| *c == s) {
                return Err(bad(format!("invalid value `{s}` for `{key}`")));
            }
        }
        Kind::Text | Kind::Path => {
            if !value.is_null() && !value.is_string() {
                return Err(bad(format!("`{key}` must be a string or null")));
            }
        }
    }
    Ok(())
}

fn opt_str_from_value(value: &Value) -> Option<String> {
    value.as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned)
}

impl MagicPrefs {
    pub fn get_value(&self, key: &str) -> Value {
        match key {
            "project.root" => opt_str_json(&self.project.root),
            "project.characters_root" => opt_str_json(&self.project.characters_root),
            "project.storyboard_template" => json!(self.project.storyboard_template),
            "project.storyboard_format" => json!(self.project.storyboard_format),
            "project.storyboard_locked" => json!(self.project.storyboard_locked),
            "image.provider" => opt_str_json(&self.image.provider),
            "scribbles.template" => opt_str_json(&self.scribbles.template),
            "scribbles.provider" => opt_str_json(&self.scribbles.provider),
            "scribbles.model" => opt_str_json(&self.scribbles.model),
            _ => Value::Null,
        }
    }

    pub fn set_value(&mut self, key: &str, value: &Value) -> Result<()> {
        validate_schema_value(key, value)?;
        match key {
            "project.root" => self.project.root = opt_str_from_value(value),
            "project.characters_root" => self.project.characters_root = opt_str_from_value(value),
            "project.storyboard_template" => self.project.storyboard_template = value.as_str().unwrap_or("magic").to_owned(),
            "project.storyboard_format" => self.project.storyboard_format = value.as_str().unwrap_or("designcraft").to_owned(),
            "project.storyboard_locked" => self.project.storyboard_locked = value.as_bool().unwrap_or(true),
            "image.provider" => self.image.provider = opt_str_from_value(value),
            "scribbles.template" => self.scribbles.template = opt_str_from_value(value),
            "scribbles.provider" => self.scribbles.provider = opt_str_from_value(value),
            "scribbles.model" => self.scribbles.model = opt_str_from_value(value),
            _ => return Err(bad(format!("unknown setting `{key}`; see prefs.schema"))),
        }
        Ok(())
    }
}

pub fn open_store() -> SettingsStore<MagicPrefs> {
    SettingsStore::<MagicPrefs>::open(Location::Auto).0
}

pub fn open_store_at(loc: Location) -> SettingsStore<MagicPrefs> {
    SettingsStore::<MagicPrefs>::open(loc).0
}

pub fn get(key: &str) -> Value {
    open_store().get().get_value(key)
}

pub fn set(key: &str, value: &Value) -> Result<Value> {
    let mut store = open_store();
    let mut err = None;
    store.update(|prefs| {
        if let Err(e) = prefs.set_value(key, value) {
            err = Some(e);
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    store.flush().map_err(|e| EngineError::InvalidParams(format!("failed to save config.toml: {e}")))?;
    Ok(store.get().get_value(key))
}
