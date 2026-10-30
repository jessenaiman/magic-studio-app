//! One reader for project-owned storyboard metadata, including moved projects.
//! The recorded project is the only relocation authority. No directory discovery
//! or writes occur here; references outside that project are left unchanged.
use crate::{EngineError, Result};
use serde_json::Value;
use std::path::{Component, Path};

fn json(path: &Path) -> Result<Value> {
    serde_json::from_slice(&std::fs::read(path)?)
        .map_err(|error| EngineError::InvalidParams(format!("{}: {error}", path.display())))
}

fn relocate(value: &mut Value, previous: &Path, current: &Path) {
    match value {
        Value::String(text) => {
            if let Ok(relative) = Path::new(text).strip_prefix(previous)
                && relative
                    .components()
                    .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
            {
                *text = current.join(relative).to_string_lossy().into_owned();
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| relocate(item, previous, current)),
        Value::Object(items) => items
            .values_mut()
            .for_each(|item| relocate(item, previous, current)),
        _ => {}
    }
}

fn resolve(document: &mut Value, owner: &Value, directory: &Path) {
    if let Some(previous) = owner["project"]
        .as_str()
        .map(Path::new)
        .filter(|path| path.is_absolute())
        .and_then(Path::parent)
        && previous != directory
    {
        relocate(document, previous, directory);
    }
}

pub(crate) fn load(directory: &Path) -> Result<Value> {
    let mut board = json(&directory.join("storyboard/movie-board.json"))?;
    let owner = serde_json::json!({"project":board["project"]});
    resolve(&mut board, &owner, directory);
    Ok(board)
}

/// Drafts may predate a move and do not carry their own project identity.
pub(crate) fn related(directory: &Path, path: &Path) -> Result<Value> {
    let owner = json(&directory.join("storyboard/movie-board.json"))?;
    let mut document = json(path)?;
    resolve(&mut document, &owner, directory);
    Ok(document)
}
