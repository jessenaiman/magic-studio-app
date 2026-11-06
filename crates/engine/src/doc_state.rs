//! One document of the open project, held by the session (`docs/architecture.md` › Engine shape).
//!
//! Views, the CLI, MCP and the control channel read and edit this copy through commands; the file is
//! written only by [`DocState::save`]. The file's stamp at load time lets the session notice a change
//! made by another app or agent ([`DocState::changed_on_disk`]): an unedited document reloads, an edited
//! one refuses to save over the newer file.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;

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
        Some(Self {
            len: meta.len(),
            modified: meta.modified().ok(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct DocState {
    /// What the document is, e.g. `words` for the lyric design (`design/words.json`).
    pub kind: &'static str,
    pub path: PathBuf,
    pub value: Value,
    /// Bumped by every edit, so readers can tell whether they hold the latest copy.
    pub revision: u64,
    /// Edited since it was loaded or saved.
    pub dirty: bool,
    /// The file as it was when the document last matched it (`None`: no file yet).
    pub stamp: Option<FileStamp>,
}

impl DocState {
    /// Read `path` as JSON. A missing file is `Ok(None)`.
    pub fn load(kind: &'static str, path: &Path) -> Result<Option<Self>> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let value = serde_json::from_slice(&bytes).map_err(|e| {
            EngineError::InvalidParams(format!("{} is not valid JSON: {e}", path.display()))
        })?;
        Ok(Some(Self {
            kind,
            path: path.to_path_buf(),
            value,
            revision: 0,
            dirty: false,
            stamp: FileStamp::of(path),
        }))
    }

    /// A document whose file does not exist yet.
    pub fn new(kind: &'static str, path: &Path, value: Value) -> Self {
        Self {
            kind,
            path: path.to_path_buf(),
            value,
            revision: 0,
            dirty: true,
            stamp: None,
        }
    }

    /// Whether another writer changed the file since the document last matched it.
    pub fn changed_on_disk(&self) -> bool {
        FileStamp::of(&self.path) != self.stamp
    }

    /// Replace the document's content as one edit.
    pub fn set(&mut self, value: Value) {
        self.value = value;
        self.revision += 1;
        self.dirty = true;
    }

    /// Write the document atomically. Refuses when another writer changed the file since the
    /// document last matched it, so their work is never silently overwritten.
    pub fn save(&mut self) -> Result<()> {
        if self.changed_on_disk() {
            return Err(EngineError::InvalidParams(format!(
                "{} changed outside this session; reload it before saving",
                self.path.display()
            )));
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let payload = serde_json::to_vec_pretty(&self.value)
            .map_err(|e| EngineError::InvalidParams(e.to_string()))?;
        craft_atomic::atomic_write(&self.path, &payload)?;
        self.stamp = FileStamp::of(&self.path);
        self.dirty = false;
        Ok(())
    }
}
