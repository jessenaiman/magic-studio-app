//! Undo and redo of document edits (`edit.undo`, `edit.redo`).

use serde_json::{Value, json};

use crate::doc_state::DocState;
use crate::history::Step;
use crate::{EngineError, Result, Session};

impl Session {
    /// Turn the documents the command edited (`step_base`) into one undo step.
    pub(crate) fn record_step(&mut self, label: &str) {
        let changed = std::mem::take(&mut self.step_base);
        if !changed.is_empty() {
            self.history.push(Step {
                label: label.to_string(),
                docs: changed,
            });
        }
    }

    pub fn undo(&mut self) -> Result<Value> {
        let step = self
            .history
            .undo
            .pop()
            .ok_or_else(|| EngineError::InvalidParams("Nothing to undo".into()))?;
        let inverse = self.swap_in(&step)?;
        let label = step.label.clone();
        self.history.redo.push(inverse);
        Ok(
            json!({"ok": true, "undone": label, "can_undo": !self.history.undo.is_empty(), "can_redo": true}),
        )
    }

    pub fn redo(&mut self) -> Result<Value> {
        let step = self
            .history
            .redo
            .pop()
            .ok_or_else(|| EngineError::InvalidParams("Nothing to redo".into()))?;
        let inverse = self.swap_in(&step)?;
        let label = step.label.clone();
        self.history.undo.push(inverse);
        Ok(
            json!({"ok": true, "redone": label, "can_undo": true, "can_redo": !self.history.redo.is_empty()}),
        )
    }

    /// Put the step's documents in place and write them; returns the step that reverses it.
    fn swap_in(&mut self, step: &Step) -> Result<Step> {
        let mut inverse = Step {
            label: step.label.clone(),
            docs: Vec::new(),
        };
        for saved in &step.docs {
            let current = self
                .docs
                .iter()
                .position(|d| d.kind == saved.kind && d.path == saved.path);
            let live = current.map(|i| self.docs[i].clone());
            inverse.docs.push(
                live.clone()
                    .unwrap_or_else(|| DocState::new(saved.kind, &saved.path, Value::Null)),
            );
            let mut restored = saved.clone();
            // The file on disk is the one the live copy last wrote; saving checks against that.
            restored.stamp = live.as_ref().and_then(|d| d.stamp);
            restored.revision = live.as_ref().map_or(0, |d| d.revision) + 1;
            restored.dirty = true;
            if restored.value.is_null() {
                // The document did not exist before: remove its file.
                if restored.changed_on_disk() {
                    return Err(EngineError::InvalidParams(format!(
                        "{} changed outside this session",
                        restored.path.display()
                    )));
                }
                let _ = std::fs::remove_file(&restored.path);
                self.docs
                    .retain(|d| !(d.kind == saved.kind && d.path == saved.path));
                continue;
            }
            restored.save()?;
            match current {
                Some(i) => self.docs[i] = restored,
                None => self.docs.push(restored),
            }
        }
        Ok(inverse)
    }
}

pub fn cmd_edit_undo(s: &mut Session, _: &Value) -> Result<Value> {
    s.undo()
}

pub fn cmd_edit_redo(s: &mut Session, _: &Value) -> Result<Value> {
    s.redo()
}
