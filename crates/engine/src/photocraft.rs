//! PhotoCraft document operations through its published Rust session.
use crate::{EngineError, Result};
use photocraft_engine::Session;
use serde_json::{Value, json};
use std::path::Path;

fn error(id: &str, e: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidParams(format!("PhotoCraft {id}: {e}"))
}

fn open(input: &Path) -> Result<Session> {
    let input = std::fs::canonicalize(input)?;
    let mut session = Session::new();
    let result = session
        .execute("file.openAs", json!({"path": input}))
        .map_err(|e| error("file.openAs", e))?;
    if session.active().is_none() {
        return Err(error(
            "file.openAs",
            format!("no document opened: {result}"),
        ));
    }
    Ok(session)
}

pub(crate) fn inspect(input: &Path) -> Result<Value> {
    open(input)?
        .execute("document.inspect", json!({}))
        .map_err(|e| error("document.inspect", e))
}

pub(crate) fn run(
    input: &Path,
    steps: &[(&str, Value)],
    output: Option<&Path>,
) -> Result<Vec<Value>> {
    if output.is_some_and(|path| photocraft_engine::file_cmds::same_file(input, path)) {
        return Err(error("save", "output would replace the input document"));
    }
    let mut session = open(input)?;
    let mut results = Vec::with_capacity(steps.len());
    for (id, params) in steps {
        if photocraft_engine::actions_cmds::noop_step(&session, id, params) {
            results.push(Value::Null);
            continue;
        }
        let value = session
            .execute(id, params.clone())
            .map_err(|e| error(id, e))?;
        if let Some(message) = photocraft_engine::actions_cmds::nested_failure(id, &value) {
            return Err(error(id, message));
        }
        results.push(value);
    }
    if let Some(path) = output {
        session
            .execute("file.saveACopy", json!({"path": path}))
            .map_err(|e| error("file.saveACopy", e))?;
    }
    Ok(results)
}
