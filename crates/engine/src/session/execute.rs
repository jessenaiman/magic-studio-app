//! The one execution path (`docs/architecture.md` › Engine shape): every command, from a view, the
//! CLI, MCP or the control channel, runs here. A command runs inside the panic guard; a failed or
//! panicking command leaves the open documents as they were, and a journaled success becomes one
//! undo step (`session/undo.rs`).

use serde_json::Value;

use crate::{EngineError, Result, Session, activity, check_param_types, commands, guard};

fn announce_dispatch(s: &Session, id: &str, params: &Value) {
    activity::command(s.project_path.as_deref(), id, params, "dispatch_running", None);
}

/// The stage a finished command is recorded under.
fn outcome_stage(value: &Value) -> &'static str {
    if value["ok"] == false {
        "engine_error"
    } else if value["intake_ready"] == false {
        "engine_needs_work"
    } else {
        "engine_success"
    }
}

fn announce_result(s: &Session, id: &str, params: &Value, result: &Result<Value>) {
    match result {
        Ok(v) => activity::command(s.project_path.as_deref(), id, params, outcome_stage(v), v["error"].as_str().or_else(|| v["reason"].as_str())),
        Err(e) => activity::error(s.project_path.as_deref(), id, e),
    }
}

fn execute_command(s: &mut Session, id: &str, params: Value) -> Result<Value> {
    let _context = activity::enter(id, &params);
    let record = s.record_user_state && commands::find(id).is_none_or(|c| c.journal);
    if record {
        announce_dispatch(s, id, &params);
    }
    let started = std::time::Instant::now();
    let result = execute_inner(s, id, params.clone());
    if record && let Ok(value) = &result {
        activity::result(s.project_path.as_deref(), id, value, started.elapsed().as_millis());
    }
    if record || (s.record_user_state && !result.as_ref().is_ok_and(|v| v["ok"] != false)) {
        announce_result(s, id, &params, &result);
    }
    result
}

fn execute_inner(s: &mut Session, id: &str, params: Value) -> Result<Value> {
    let spec = commands::find(id).ok_or_else(|| EngineError::UnknownCommand(id.to_string()))?;
    check_param_types(spec.params, &params)?;
    (spec.enabled)(s).map_err(|e| EngineError::CommandFailed(id.to_string(), e))?;
    let (docs_before, project_before) = (s.docs.clone(), s.project_path.clone());
    s.step_base.clear();
    let run = guard::catch_panic(|| (spec.run)(s, &params))
        .map_err(|panic| EngineError::CommandFailed(id.to_string(), format!("panicked: {panic}")))
        .and_then(|result| result);
    let out = match run {
        Ok(out) => out,
        Err(error) => {
            // A failed or panicking command leaves the open documents as they were.
            s.docs = docs_before;
            s.step_base.clear();
            return Err(error);
        }
    };
    if s.project_path != project_before {
        s.close_documents();
    } else if spec.journal && !matches!(id, "edit.undo" | "edit.redo") {
        s.record_step(id);
    }
    if spec.journal {
        s.record(out.clone());
    }
    Ok(out)
}

impl Session {
    /// Execute one command by id with JSON parameters, returning JSON.
    pub fn execute(&mut self, id: &str, params: Value) -> Result<Value> {
        execute_command(self, id, params)
    }
}
