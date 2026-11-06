//! One production project root, independent of navigation history and per-intake overrides.
use crate::{EngineError, Result, Session};
use craft_settings::{Location, Settings, SettingsStore};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
struct MagicPrefs {
    paths: PathsSection,
    tools: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
struct PathsSection {
    projects_root: Option<String>,
    characters_root: Option<String>,
}

impl Settings for MagicPrefs {
    const APP_ID: &'static str = "magic";
    const SCHEMA_VERSION: u32 = 1;
}

fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

fn file() -> Result<PathBuf> {
    crate::project_catalog::file()
        .and_then(|p| p.parent().map(|p| p.join("settings.json")))
        .ok_or_else(|| {
            invalid("Magic Studio config directory unavailable; no recent-project fallback used")
        })
}

fn parse(bytes: &[u8]) -> Result<Value> {
    serde_json::from_slice::<Value>(bytes).ok().filter(Value::is_object)
        .ok_or_else(||invalid("Invalid Magic Studio settings.json; repair it explicitly. No history fallback used"))
}

fn stored() -> Result<Value> {
    let mut input = match fs::File::open(file()?) {
        Ok(input) => input,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(json!({})),
        Err(e) => return Err(e.into()),
    };
    input.lock_shared()?;
    let mut bytes = Vec::new();
    input.read_to_end(&mut bytes)?;
    parse(&bytes)
}

fn absolute(value: &str) -> Result<PathBuf> {
    let path = Path::new(value.trim());
    if value.trim().is_empty() || !path.is_absolute() || value.chars().any(char::is_control) {
        return Err(invalid(
            "Project root must be a nonempty absolute directory path",
        ));
    }
    Ok(path.to_path_buf())
}

fn configured_root(settings: &Value) -> Result<PathBuf> {
    if let Some(v) = settings.get("projects_root") {
        return absolute(
            v.as_str()
                .ok_or_else(|| invalid("Saved projects_root must be a string"))?,
        );
    }
    let (store, _) = SettingsStore::<MagicPrefs>::open(Location::Auto);
    if let Some(ref root) = store.get().paths.projects_root {
        return absolute(root);
    }
    crate::workspace::user_data_dir()
        .map(|root| root.join("magic/projects"))
        .ok_or_else(|| invalid("Project root unavailable; configure XDG_DATA_HOME or HOME"))
}

pub fn root() -> Result<PathBuf> {
    configured_root(&stored()?)
}

/// Read the saved Magic Studio settings object (`~/.config/magic/settings.json`), or `{}`.
pub(crate) fn stored_settings() -> Value {
    let mut val = stored().unwrap_or_else(|_| json!({}));
    let (store, _) = SettingsStore::<MagicPrefs>::open(Location::Auto);
    let prefs = store.get();
    if val.get("projects_root").is_none()
        && let Some(ref root) = prefs.paths.projects_root
    {
        val["projects_root"] = json!(root);
    }
    if val.get("characters_root").is_none()
        && let Some(ref chars) = prefs.paths.characters_root
    {
        val["characters_root"] = json!(chars);
    }
    if !prefs.tools.is_empty() && val.get("tools").is_none() {
        val["tools"] = json!(prefs.tools);
    }
    val
}

/// A folder path saved under `key` in `settings.json` or `config.toml`; GUI windows launched by
/// the desktop do not inherit a terminal's environment, so this is where they look.
pub fn saved_path(key: &str) -> Option<PathBuf> {
    stored_settings().get(key)?.as_str().map(PathBuf::from)
}

/// Optional external character library. Its location belongs to the user's configuration.
pub fn characters_root() -> Option<PathBuf> {
    std::env::var_os("magic_CHARACTERS_ROOT")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| saved_path("characters_root"))
        .filter(|path| path.is_absolute())
}

fn read_settings_file(output: &mut fs::File) -> Result<Value> {
    let mut bytes = Vec::new();
    output.read_to_end(&mut bytes)?;
    if bytes.is_empty() {
        Ok(json!({}))
    } else {
        parse(&bytes)
    }
}

fn write_settings_file(output: &mut fs::File, settings: &Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(settings).map_err(|e| invalid(e.to_string()))?;
    output.seek(SeekFrom::Start(0))?;
    output.write_all(&bytes)?;
    output.set_len(bytes.len() as u64)?;
    output.sync_all()?;
    Ok(())
}

fn persist_root(value: &Value) -> Result<()> {
    let root = absolute(
        value
            .as_str()
            .ok_or_else(|| invalid("Project root must be a string"))?,
    )?;
    if !root.is_dir() {
        return Err(invalid(
            "Project root directory is unavailable; mount or choose the intended directory first",
        ));
    }
    let root = fs::canonicalize(root)?;
    let path = file()?;
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| invalid("Config directory unavailable"))?,
    )?;
    let mut output = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;
    output.lock()?;
    let mut settings = read_settings_file(&mut output)?;
    settings["projects_root"] = json!(root);
    write_settings_file(&mut output, &settings)?;
    let (mut store, _) = SettingsStore::<MagicPrefs>::open(Location::Auto);
    store.update(|prefs| {
        prefs.paths.projects_root = Some(root.to_string_lossy().into_owned());
    });
    let _ = store.flush();
    Ok(())
}

/// Without `root`, inspect the setting. Only this explicit command changes it.
pub fn command(_: &mut Session, params: &Value) -> Result<Value> {
    if let Some(value) = params.get("root") {
        persist_root(value)?;
    }
    let settings = stored()?;
    Ok(
        json!({"root":configured_root(&settings)?,"config":file()?,"persisted":settings.get("projects_root").is_some(),"source":if settings.get("projects_root").is_some(){"saved production setting"}else{"production default"},"overrides":"project.intake root= or path= applies only to that request; recent history never changes this setting"}),
    )
}
