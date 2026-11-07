//! The one execution path (`docs/architecture.md` › Engine shape): every command, from a view, the
//! CLI, MCP or the control channel, runs here. A command runs inside the panic guard; a failed or
//! panicking command leaves the open documents as they were, and a journaled success becomes one
//! undo step (`session/undo.rs`).

use serde_json::Value;

use crate::{CommandSpec, EngineError, Result, Session, activity, check_param_types, commands, guard, project};

/// The writer lease a journaled command needs, except for the project lifecycle calls.
fn writer_lease(s: &Session, id: &str, project_edit: bool) -> Result<Option<std::sync::Arc<std::fs::File>>> {
    let lifecycle = matches!(id, "project.open" | "project.close" | "project.new" | "project.intake");
    if !project_edit || lifecycle {
        return Ok(None);
    }
    match s.project_path.as_deref() {
        Some(path) => Ok(Some(project::claim(s, path)?)),
        None => Ok(None),
    }
}

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

/// An edit is a successful journaled command that left the project file unwritten.
fn note_edit(s: &mut Session, project_edit: bool, result: &Result<Value>, saves_before: u64, project_before: Option<&std::path::PathBuf>) {
    if !(project_edit && result.as_ref().is_ok_and(|v| v["ok"] != false)) {
        return;
    }
    if s.project_saves.load(std::sync::atomic::Ordering::Relaxed) != saves_before || s.project_path.as_ref() != project_before {
        s.edits_since_save = 0;
    } else {
        s.edits_since_save += 1;
    }
}

/// Save As transfers ownership to a new path; never restore the source lease over it.
fn restore_lease(s: &mut Session, prior: Option<std::sync::Arc<std::fs::File>>, had_writer: bool, project_before: Option<&std::path::PathBuf>) {
    if had_writer && s.project_path.as_ref() == project_before {
        s.project_owner = prior;
    }
}

/// Keep the open project file in step with its asset connections.
fn sync_project_file(s: &mut Session, spec: &CommandSpec, id: &str, assets_before: Option<&Value>) -> Result<()> {
    if !spec.journal || matches!(id, "project.open" | "project.new" | "project.close") || s.project_path.is_none() {
        return Ok(());
    }
    if serde_json::to_value(&s.project_assets).ok().as_ref() != assets_before {
        commands::write_project_file(s)?;
    }
    Ok(())
}

fn execute_command(s: &mut Session, id: &str, params: Value) -> Result<Value> {
    let _context = activity::enter(id, &params);
    // Cast writes are journaled workshop files, independent of the open song's writer lease.
    let project_edit = commands::find(id).is_some_and(|c| c.journal) && id != "crew.save";
    let _writer = writer_lease(s, id, project_edit).inspect_err(|error| {
        if s.record_user_state {
            activity::error(s.project_path.as_deref(), id, error);
        }
    })?;
    let prior_owner = s.project_owner.clone();
    if let Some(owner) = &_writer {
        s.project_owner = Some(owner.clone());
    }
    let record = s.record_user_state && commands::find(id).is_none_or(|c| c.journal);
    if record {
        announce_dispatch(s, id, &params);
    }
    let started = std::time::Instant::now();
    let (saves_before, project_before) = (s.project_saves.load(std::sync::atomic::Ordering::Relaxed), s.project_path.clone());
    let result = execute_inner(s, id, params.clone());
    note_edit(s, project_edit, &result, saves_before, project_before.as_ref());
    if record && let Ok(value) = &result {
        activity::result(s.project_path.as_deref(), id, value, started.elapsed().as_millis());
    }
    if record || (s.record_user_state && !result.as_ref().is_ok_and(|v| v["ok"] != false)) {
        announce_result(s, id, &params, &result);
    }
    restore_lease(s, prior_owner, _writer.is_some(), project_before.as_ref());
    result
}

fn execute_inner(s: &mut Session, id: &str, params: Value) -> Result<Value> {
    let spec = commands::find(id).ok_or_else(|| EngineError::UnknownCommand(id.to_string()))?;
    check_param_types(spec.params, &params)?;
    (spec.enabled)(s).map_err(|e| EngineError::CommandFailed(id.to_string(), e))?;
    let assets_before = serde_json::to_value(&s.project_assets).ok();
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
    sync_project_file(s, spec, id, assets_before.as_ref())?;
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
