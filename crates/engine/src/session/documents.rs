//! The open project's documents, held by the session (`docs/architecture.md` › Engine shape).

use std::path::Path;

use crate::doc_state::DocState;
use crate::link_watch::{ReloadDecision, decide};
use crate::{EngineError, Result, Session};

impl Session {
    pub fn documents(&self) -> &[DocState] {
        &self.docs
    }

    /// The document of `kind` at `path`, loaded on first use. A document another app or agent changed
    /// on disk reloads when it has no unsaved edits; with unsaved edits on both sides it is an error,
    /// never a silent overwrite. `Ok(None)`: the file does not exist and nothing is open for it.
    pub fn document(&mut self, kind: &'static str, path: &Path) -> Result<Option<&mut DocState>> {
        let open = self
            .docs
            .iter()
            .position(|d| d.kind == kind && d.path == path);
        let reload = match open {
            Some(i) => {
                let doc = &self.docs[i];
                match decide(doc.dirty, doc.changed_on_disk()) {
                    ReloadDecision::Refuse => {
                        return Err(EngineError::InvalidParams(format!(
                            "{} changed outside this session while it had unsaved edits",
                            path.display()
                        )));
                    }
                    ReloadDecision::Reload => true,
                    ReloadDecision::Keep => false,
                }
            }
            None => true,
        };
        if reload {
            self.docs.retain(|d| !(d.kind == kind && d.path == path));
            match DocState::load(kind, path)? {
                Some(doc) => self.docs.push(doc),
                None => return Ok(None),
            }
        }
        Ok(self
            .docs
            .iter_mut()
            .find(|d| d.kind == kind && d.path == path))
    }

    /// Forget the open documents (the project changed).
    pub fn close_documents(&mut self) {
        self.docs.clear();
        self.history.clear();
    }
}
