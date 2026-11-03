//! UI project operations enter the same guarded dispatch as CLI and MCP.
//! The correlated dispatch reply is rendered by the initiating UI handler, without recursive Open.
//!
//! Reasoning for the Pending slot: multiple agents (CLI, MCP, herdr panes) can issue
//! project.open/new/intake simultaneously. The Mutex serializes them so one transition
//! completes before another starts, and the _ui_dispatch_token lets the initiating
//! caller match its own reply. This is coordination, not a claim the design is optimal.
use serde_json::{Value, json};
use std::sync::Mutex;
use tauri::Manager;

#[derive(Default)]
/// One in-flight project transition at a time. Serializes concurrent project.open/new/intake
/// from multiple agents so transitions don't interleave. The token lets the caller match
/// its own reply.
pub struct Pending(Mutex<Option<(String, String)>>);

struct Request<'a>(&'a Pending);
impl Drop for Request<'_> {
    fn drop(&mut self) {
        if let Ok(mut pending) = self.0.0.lock() {
            *pending = None;
        }
    }
}

/// Returns true when `id`/`params` match the in-flight project transition, meaning
/// this call is the UI's own follow-up to a transition it started (not a new agent request).
pub fn is_ui_request(app: &tauri::AppHandle, id: &str, params: &Value) -> bool {
    app.state::<Pending>().0.lock().ok().is_some_and(|pending| {
        pending.as_ref().is_some_and(|(command, token)| {
            command == id && params["_ui_dispatch_token"].as_str() == Some(token.as_str())
        })
    })
}

pub fn execute(app: &tauri::AppHandle, id: &str, mut params: Value) -> Result<Value, String> {
    let token = crate::feedback::new_request();
    let pending = app.state::<Pending>();
    {
        let mut slot = pending.0.lock().map_err(|e| e.to_string())?;
        if slot.is_some() {
            return Err("Another New/Open command is still running".into());
        }
        *slot = Some((id.to_owned(), token.clone()));
    }
    let _request = Request(&pending);
    params["_ui_dispatch_token"] = json!(token);
    let mut result = crate::desktop_commands::handle(app, id, &params)?;
    if result["reused_session"] == true {
        return Ok(result);
    }
    let state = app.state::<crate::Engine>().with(None, crate::state_of)?;
    for (key, value) in state
        .as_object()
        .ok_or("Studio state unavailable after CLI command")?
    {
        result[key] = value.clone();
    }
    Ok(result)
}
