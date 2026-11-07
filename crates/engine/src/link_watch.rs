//! Outside-change watch: whether another writer changed a document's file on disk.
//!
//! Format follows VectorCraft's `crates/engine/src/link_watch.rs` (same shape, Magic variants).
//! Same behaviour as the pre-move `DocState::changed_on_disk` / `session/documents.rs` logic:
//! an unedited document reloads, an edited one refuses to save over the newer file — never a
//! silent overwrite.

use std::path::Path;
use std::time::SystemTime;

use crate::{EngineError, Result};

/// The file's identity when the document last matched it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileStamp {
    pub len: u64,
    pub modified: Option<SystemTime>,
}

impl FileStamp {
    /// The stamp of `path` now, or `None` when the file does not exist.
    pub fn of(path: &Path) -> Option<Self> {
        let meta = std::fs::metadata(path).ok()?;
        Some(Self { len: meta.len(), modified: meta.modified().ok() })
    }
}

/// Whether the file at `path` differs from `stamp` (the stamp at last load/save).
pub fn changed_on_disk(path: &Path, stamp: Option<FileStamp>) -> bool {
    FileStamp::of(path) != stamp
}

/// The reload/refuse decision for `session/documents.rs::document`: an unedited document that
/// changed on disk reloads; an edited one errors.
pub enum ReloadDecision {
    /// Current copy is in step with disk: keep it, no reload.
    Keep,
    /// Changed on disk with no unsaved edits: drop and reload from disk.
    Reload,
    /// Changed on disk but this session has unsaved edits: refuse.
    Refuse,
}

pub fn decide(dirty: bool, changed: bool) -> ReloadDecision {
    match (dirty, changed) {
        (true, true) => ReloadDecision::Refuse,
        (_, true) => ReloadDecision::Reload,
        _ => ReloadDecision::Keep,
    }
}

/// Helper used by `DocState::save`'s refuse check.
pub fn save_guard(path: &Path, stamp: Option<FileStamp>) -> Result<()> {
    if changed_on_disk(path, stamp) {
        return Err(EngineError::InvalidParams(format!("{} changed outside this session; reload it before saving", path.display())));
    }
    Ok(())
}

/// Path helper for callers that hold a document path and stamp.
pub fn stamp_of(path: &Path) -> Option<FileStamp> {
    FileStamp::of(path)
}

/// Convenience for `DocState`-shaped callers: does this (path, stamp) pair have outside changes?
pub fn is_changed(path: &Path, stamp: Option<FileStamp>) -> bool {
    changed_on_disk(path, stamp)
}
