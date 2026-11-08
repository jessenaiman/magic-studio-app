//! Editing the open documents: every change is one edit with a new revision (`session/documents.rs`
//! holds them, `session/undo.rs` steps back through them).

use std::path::Path;

use serde_json::Value;

use crate::doc_state::DocState;
use crate::{EngineError, Result, Session};

impl Session {
    /// Set the content of the document of `kind` at `path` (opening it if needed) as one edit.
    pub fn edit_document(&mut self, kind: &'static str, path: &Path, value: Value) -> Result<&mut DocState> {
        let existing = self.document(kind, path)?.map(|d| d.clone());
        // The undo step keeps the document as it was before this command first edited it.
        if !self.step_base.iter().any(|b| b.kind == kind && b.path == path) {
            self.step_base.push(existing.clone().unwrap_or_else(|| DocState::new(kind, path, Value::Null)));
        }
        if existing.is_none() {
            self.docs.push(DocState::new(kind, path, Value::Null));
        }
        let doc = self
            .docs
            .iter_mut()
            .find(|d| d.kind == kind && d.path == path)
            .ok_or_else(|| EngineError::InvalidParams(format!("{kind} document did not open")))?;
        doc.set(value);
        Ok(doc)
    }

    /// Set the document's content and write it.
    pub fn save_document(&mut self, kind: &'static str, path: &Path, value: Value) -> Result<()> {
        self.edit_document(kind, path, value)?.save()
    }

    /// The project document `D` from the open project's folder, loaded on first use and reloaded
    /// when another writer changed it. `Ok(None)`: the file does not exist yet.
    pub fn doc<D: crate::doc_state::ProjectDoc>(&mut self, dir: &Path) -> Result<Option<D>> {
        let path = D::path(dir);
        match self.document(D::KIND, &path)? {
            Some(doc) => serde_json::from_value(doc.value.clone())
                .map(Some)
                .map_err(|e| EngineError::InvalidParams(format!("{} is not a valid {} document: {e}", path.display(), D::KIND))),
            None => Ok(None),
        }
    }

    /// Store `doc` as one undoable edit of the project document `D` and write it.
    pub fn save_doc<D: crate::doc_state::ProjectDoc>(&mut self, dir: &Path, doc: &D) -> Result<()> {
        let value = serde_json::to_value(doc).map_err(|e| EngineError::InvalidParams(format!("cannot encode {}: {e}", D::KIND)))?;
        self.save_document(D::KIND, &D::path(dir), value)
    }
}
