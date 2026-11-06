//! Undo and redo steps (`docs/architecture.md` › Engine shape). A step records the open documents as
//! they were before one journaled command changed them; undoing puts them back.

use crate::doc_state::DocState;

/// The most steps kept; the oldest is dropped beyond it.
const MAX_STEPS: usize = 200;

#[derive(Clone, Debug)]
pub struct Step {
    /// The command that made the change (shown as "Undo <label>").
    pub label: String,
    /// The documents the command changed, as they were on the other side of the step.
    pub docs: Vec<DocState>,
}

#[derive(Clone, Debug, Default)]
pub struct History {
    pub undo: Vec<Step>,
    pub redo: Vec<Step>,
}

impl History {
    /// Record a new change: it can be undone, and nothing undone before it can be redone any more.
    pub fn push(&mut self, step: Step) {
        self.undo.push(step);
        if self.undo.len() > MAX_STEPS {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}
