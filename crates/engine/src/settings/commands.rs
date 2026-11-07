//! Settings commands (`prefs.schema`, `prefs.get`, `prefs.set`) backed by `config.toml` (`craft-settings`).
//!
//! Reference: <https://github.com/storytold/filmcraft/blob/7322aad72491a294a06a26a63fb65a3fc30ac242/crates/engine/src/settings/commands.rs>
use crate::{CommandSpec, Result, Session};
use serde_json::{Value, json};

fn spec(id: &'static str, label: &'static str, params: &'static str, run: fn(&mut Session, &Value) -> Result<Value>, journal: bool) -> CommandSpec {
    CommandSpec { id, label, menu: &[], shortcut: None, params, help: "", example: "", long_running: false, enabled: crate::commands::always, run, journal }
}
fn schema(s: &mut Session, p: &Value) -> Result<Value> {
    let want = p.get("category").and_then(Value::as_str);
    if let Some(w) = want
        && !super::CATEGORIES.iter().any(|c| c.id == w)
    {
        return Err(crate::EngineError::InvalidParams(format!("unknown category `{w}`")));
    }
    Ok(super::schema_json(s, want))
}
fn get(_s: &mut Session, p: &Value) -> Result<Value> {
    let key = p.get("key").and_then(Value::as_str).ok_or_else(|| crate::EngineError::InvalidParams("need `key`".into()))?;
    let known = super::fields_with_cat().iter().any(|(_, f)| f.key == key);
    if !known {
        return Err(crate::EngineError::InvalidParams(format!("unknown setting `{key}`; see prefs.schema")));
    }
    Ok(json!({"key":key,"value":super::prefs::get(key)}))
}
fn set(_s: &mut Session, p: &Value) -> Result<Value> {
    let key = p.get("key").and_then(Value::as_str).ok_or_else(|| crate::EngineError::InvalidParams("need `key`".into()))?;
    let value = p.get("value").cloned().ok_or_else(|| crate::EngineError::InvalidParams("need `value`".into()))?;
    let saved = super::prefs::set(key, &value)?;
    Ok(json!({"key":key,"value":saved}))
}
pub fn commands() -> Vec<CommandSpec> {
    vec![
        spec("prefs.schema", "Settings Schema", r#"{"category":str?}"#, schema, false),
        spec("prefs.get", "Get Setting", r#"{"key":str}"#, get, false),
        spec("prefs.set", "Set Setting", r#"{"key":str,"value":any}"#, set, true),
    ]
}
