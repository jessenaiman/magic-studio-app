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

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectPrefs {
    pub root: Option<String>,
    pub characters_root: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImagePrefs {
    pub provider: String,
}

impl Default for ImagePrefs {
    fn default() -> Self {
        Self { provider: "codex".into() }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScribblesPrefs {
    pub template: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LyricsPrefs {
    pub default_size: i64,
    pub outline: bool,
}

impl Default for LyricsPrefs {
    fn default() -> Self {
        Self { default_size: 72, outline: true }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnimationPrefs {
    pub loop_preview: bool,
    pub duration: f64,
}

impl Default for AnimationPrefs {
    fn default() -> Self {
        Self { loop_preview: true, duration: 4.0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearancePrefs {
    pub theme: String,
    pub accessible_contrast: bool,
    pub ui_scale: f64,
}

impl Default for AppearancePrefs {
    fn default() -> Self {
        Self { theme: "dark".into(), accessible_contrast: false, ui_scale: 1.0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralPrefs {
    pub at_startup: String,
    pub when_opening_project: String,
    pub show_tooltips: bool,
    pub show_rich_tooltips: bool,
    pub recent_projects: String,
}

impl Default for GeneralPrefs {
    fn default() -> Self {
        Self {
            at_startup: "showHome".into(),
            when_opening_project: "showOpenDialog".into(),
            show_tooltips: true,
            show_rich_tooltips: true,
            recent_projects: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AutosavePrefs {
    pub enabled: bool,
    pub interval_minutes: i64,
}

impl Default for AutosavePrefs {
    fn default() -> Self {
        Self { enabled: true, interval_minutes: 5 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MediaCachePrefs {
    pub max_mb: i64,
    pub location: Option<String>,
}

impl Default for MediaCachePrefs {
    fn default() -> Self {
        Self { max_mb: 4096, location: None }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaybackPrefs {
    pub resolution: String,
    pub draft_decode: bool,
}

impl Default for PlaybackPrefs {
    fn default() -> Self {
        Self { resolution: "half".into(), draft_decode: true }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimelinePrefs {
    pub still_image_duration: f64,
    pub transition_duration: f64,
}

impl Default for TimelinePrefs {
    fn default() -> Self {
        Self { still_image_duration: 5.0, transition_duration: 1.0 }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioPrefs {
    pub edit_location: Option<String>,
    pub output_device: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LabelsPrefs {
    pub preset: String,
}

impl Default for LabelsPrefs {
    fn default() -> Self {
        Self { preset: "Default".into() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MediaPrefs {
    pub analyze_on_import: bool,
}

impl Default for MediaPrefs {
    fn default() -> Self {
        Self { analyze_on_import: true }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MagicPrefs {
    pub project: ProjectPrefs,
    pub image: ImagePrefs,
    pub scribbles: ScribblesPrefs,
    pub lyrics: LyricsPrefs,
    pub animation: AnimationPrefs,
    pub appearance: AppearancePrefs,
    pub general: GeneralPrefs,
    pub autosave: AutosavePrefs,
    #[serde(rename = "mediaCache")]
    pub media_cache: MediaCachePrefs,
    pub playback: PlaybackPrefs,
    pub timeline: TimelinePrefs,
    pub audio: AudioPrefs,
    pub labels: LabelsPrefs,
    pub media: MediaPrefs,
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
            "image.provider" => json!(self.image.provider),
            "scribbles.template" => opt_str_json(&self.scribbles.template),
            "scribbles.provider" => opt_str_json(&self.scribbles.provider),
            "lyrics.default_size" => json!(self.lyrics.default_size),
            "lyrics.outline" => json!(self.lyrics.outline),
            "animation.loop_preview" => json!(self.animation.loop_preview),
            "animation.duration" => json!(self.animation.duration),
            "appearance.theme" => json!(self.appearance.theme),
            "appearance.accessible_contrast" => json!(self.appearance.accessible_contrast),
            "appearance.ui_scale" => json!(self.appearance.ui_scale),
            "general.at_startup" => json!(self.general.at_startup),
            "general.when_opening_project" => json!(self.general.when_opening_project),
            "general.show_tooltips" => json!(self.general.show_tooltips),
            "general.show_rich_tooltips" => json!(self.general.show_rich_tooltips),
            "general.recent_projects" => json!(self.general.recent_projects),
            "autosave.enabled" => json!(self.autosave.enabled),
            "autosave.interval_minutes" => json!(self.autosave.interval_minutes),
            "mediaCache.max_mb" => json!(self.media_cache.max_mb),
            "mediaCache.location" => opt_str_json(&self.media_cache.location),
            "playback.resolution" => json!(self.playback.resolution),
            "playback.draft_decode" => json!(self.playback.draft_decode),
            "timeline.still_image_duration" => json!(self.timeline.still_image_duration),
            "timeline.transition_duration" => json!(self.timeline.transition_duration),
            "audio.edit_location" => opt_str_json(&self.audio.edit_location),
            "audio.output_device" => json!(self.audio.output_device),
            "labels.preset" => json!(self.labels.preset),
            "media.analyze_on_import" => json!(self.media.analyze_on_import),
            _ => Value::Null,
        }
    }

    pub fn set_value(&mut self, key: &str, value: &Value) -> Result<()> {
        validate_schema_value(key, value)?;
        match key {
            "project.root" => self.project.root = opt_str_from_value(value),
            "project.characters_root" => self.project.characters_root = opt_str_from_value(value),
            "image.provider" => self.image.provider = value.as_str().unwrap_or("codex").to_owned(),
            "scribbles.template" => self.scribbles.template = opt_str_from_value(value),
            "scribbles.provider" => self.scribbles.provider = opt_str_from_value(value),
            "lyrics.default_size" => self.lyrics.default_size = value.as_i64().unwrap_or(72),
            "lyrics.outline" => self.lyrics.outline = value.as_bool().unwrap_or(true),
            "animation.loop_preview" => self.animation.loop_preview = value.as_bool().unwrap_or(true),
            "animation.duration" => self.animation.duration = value.as_f64().unwrap_or(4.0),
            "appearance.theme" => self.appearance.theme = value.as_str().unwrap_or("dark").to_owned(),
            "appearance.accessible_contrast" => self.appearance.accessible_contrast = value.as_bool().unwrap_or(false),
            "appearance.ui_scale" => self.appearance.ui_scale = value.as_f64().unwrap_or(1.0),
            "general.at_startup" => self.general.at_startup = value.as_str().unwrap_or("showHome").to_owned(),
            "general.when_opening_project" => self.general.when_opening_project = value.as_str().unwrap_or("showOpenDialog").to_owned(),
            "general.show_tooltips" => self.general.show_tooltips = value.as_bool().unwrap_or(true),
            "general.show_rich_tooltips" => self.general.show_rich_tooltips = value.as_bool().unwrap_or(true),
            "general.recent_projects" => self.general.recent_projects = value.as_str().unwrap_or_default().to_owned(),
            "autosave.enabled" => self.autosave.enabled = value.as_bool().unwrap_or(true),
            "autosave.interval_minutes" => self.autosave.interval_minutes = value.as_i64().unwrap_or(5),
            "mediaCache.max_mb" => self.media_cache.max_mb = value.as_i64().unwrap_or(4096),
            "mediaCache.location" => self.media_cache.location = opt_str_from_value(value),
            "playback.resolution" => self.playback.resolution = value.as_str().unwrap_or("half").to_owned(),
            "playback.draft_decode" => self.playback.draft_decode = value.as_bool().unwrap_or(true),
            "timeline.still_image_duration" => self.timeline.still_image_duration = value.as_f64().unwrap_or(5.0),
            "timeline.transition_duration" => self.timeline.transition_duration = value.as_f64().unwrap_or(1.0),
            "audio.edit_location" => self.audio.edit_location = opt_str_from_value(value),
            "audio.output_device" => self.audio.output_device = value.as_str().unwrap_or_default().to_owned(),
            "labels.preset" => self.labels.preset = value.as_str().unwrap_or("Default").to_owned(),
            "media.analyze_on_import" => self.media.analyze_on_import = value.as_bool().unwrap_or(true),
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
