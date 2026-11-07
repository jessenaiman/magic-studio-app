//! Prefs access over settings.json. Copied pattern from FilmCraft prefs.rs.
use serde_json::Value;
pub fn get(key: &str) -> Value {
    crate::cmd::project::settings::stored_settings().get(key).cloned().unwrap_or(Value::Null)
}
