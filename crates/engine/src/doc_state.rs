//! One document of the open project, held by the session (`docs/architecture.md` › Engine shape).
//!
//! Views, the CLI, MCP and the control channel read and edit this copy through commands; the file is
//! written only by [`DocState::save`]. The file's stamp at load time lets the session notice a change
//! made by another app or agent (see [`crate::link_watch`]): an unedited document reloads, an edited
//! one refuses to save over the newer file.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::{EngineError, Result};

/// A file Magic owns in a project folder, read and written only through the session
/// ([`crate::Session::doc`], [`crate::Session::save_doc`]): atomic, guarded against outside
/// changes by its [`FileStamp`], and undoable. A new kind of project file is one type with this
/// impl; no view or command writes the file itself.
pub trait ProjectDoc: Serialize + DeserializeOwned {
    /// The session's name for this kind of document.
    const KIND: &'static str;
    /// Where the document lives in the project folder `dir`.
    fn path(dir: &Path) -> PathBuf;
}

/// Write `value` to `path` as pretty JSON through [`craft_atomic::atomic_write`], creating the
/// folder, and return the file's new stamp and size. The one JSON writer for Magic's files:
/// callers apply their own outside-change guard first.
pub(crate) fn write_json(path: &Path, value: &impl Serialize) -> Result<(Option<FileStamp>, usize)> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_vec_pretty(value).map_err(|e| EngineError::InvalidParams(format!("cannot encode {}: {e}", path.display())))?;
    craft_atomic::atomic_write(path, &payload)?;
    Ok((FileStamp::of(path), payload.len()))
}

/// Backwards-compatible alias: the stamp type lives in [`crate::link_watch`].
pub use crate::link_watch::FileStamp;

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
    pub stamp: Option<crate::link_watch::FileStamp>,
}

impl DocState {
    /// Read `path` as JSON. A missing file is `Ok(None)`.
    pub fn load(kind: &'static str, path: &Path) -> Result<Option<Self>> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let value = serde_json::from_slice(&bytes).map_err(|e| EngineError::InvalidParams(format!("{} is not valid JSON: {e}", path.display())))?;
        Ok(Some(Self { kind, path: path.to_path_buf(), value, revision: 0, dirty: false, stamp: crate::link_watch::FileStamp::of(path) }))
    }

    /// A document whose file does not exist yet.
    pub fn new(kind: &'static str, path: &Path, value: Value) -> Self {
        Self { kind, path: path.to_path_buf(), value, revision: 0, dirty: true, stamp: None }
    }

    /// Whether another writer changed the file since the document last matched it.
    pub fn changed_on_disk(&self) -> bool {
        crate::link_watch::changed_on_disk(&self.path, self.stamp)
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
        crate::link_watch::save_guard(&self.path, self.stamp)?;
        self.stamp = write_json(&self.path, &self.value)?.0;
        self.dirty = false;
        Ok(())
    }
}
