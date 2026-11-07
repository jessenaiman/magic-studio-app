//! Commands. Copied from FilmCraft settings/commands.rs, refactored for Magic.
use crate::{CommandSpec, Result, Session};
use serde_json::{Value, json};

fn spec(id: &'static str, label: &'static str, params: &'static str, run: fn(&mut Session, &Value) -> Result<Value>, journal: bool) -> CommandSpec {
    CommandSpec { id, label, menu: &[], shortcut: None, params, help: "", example: "", long_running: false, enabled: crate::commands::always, run, journal }
}
fn schema(s: &mut Session, p: &Value) -> Result<Value> {
    let want = p.get("category").and_then(Value::as_str);
    Ok(super::schema_json(s, want))
}
fn get(_s: &mut Session, p: &Value) -> Result<Value> {
    let key = p.get("key").and_then(Value::as_str).ok_or_else(|| crate::EngineError::InvalidParams("need `key`".into()))?;
    Ok(json!({"key":key,"value":super::prefs::get(key)}))
}
fn set(_s: &mut Session, p: &Value) -> Result<Value> {
    let key = p.get("key").and_then(Value::as_str).ok_or_else(|| crate::EngineError::InvalidParams("need `key`".into()))?;
    let value = p.get("value").cloned().ok_or_else(|| crate::EngineError::InvalidParams("need `value`".into()))?;
    // Validate against schema
    let known = super::fields_with_cat().iter().any(|(_, f)| f.key == key);
    if !known {
        return Err(crate::EngineError::InvalidParams(format!("unknown setting `{key}`; see prefs.schema")));
    }
    // Persist via project settings file (same file project.root uses)
    let settings = crate::cmd::project::settings::stored_settings();
    let mut obj = settings.as_object().cloned().unwrap_or_default();
    obj.insert(key.to_string(), value.clone());
    // Reuse persist via project.root path? Direct file write:
    let file = crate::cmd::project::settings::file_pub()?;
    let bytes = serde_json::to_vec_pretty(&Value::Object(obj)).map_err(|e| crate::EngineError::InvalidParams(e.to_string()))?;
    std::fs::write(&file, bytes)?;
    Ok(json!({"key":key,"value":value}))
}
pub fn commands() -> Vec<CommandSpec> {
    vec![
        spec("prefs.schema", "Settings Schema", r#"{"category":str?}"#, schema, false),
        spec("prefs.get", "Get Setting", r#"{"key":str}"#, get, false),
        spec("prefs.set", "Set Setting", r#"{"key":str,"value":any}"#, set, true),
    ]
}
