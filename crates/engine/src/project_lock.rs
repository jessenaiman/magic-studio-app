//! One saving headless CLI run at a time per project file.
//!
//! Reference: <https://github.com/storytold/filmcraft/blob/7322aad72491a294a06a26a63fb65a3fc30ac242/apps/filmcraft-cli/src/project_lock.rs>
//!
//! Open desktop sessions never hold an OS file lock on `.magic` (they use `FileStamp` +
//! `craft_atomic::atomic_write`). A headless CLI run reads `.magic`, edits it, and atomically
//! rewrites the file, so each mutating CLI run holds a short-lived `.{name}.lock` sidecar beside
//! `.magic` from before its edit until its save completes.

use std::fs::{File, TryLockError};
use std::path::{Path, PathBuf};

/// Most bytes of the project's file name kept in the lock's name; the rest of the 255-byte limit
/// is left for the leading dot and `.lock`.
const MAX_NAME_BYTES: usize = 200;

/// Sidecar lock file path next to `project` (`.{name}.lock`).
pub fn lock_path(project: &Path) -> PathBuf {
    let name = project.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    project.with_file_name(format!(".{}.lock", &name[..name.floor_char_boundary(MAX_NAME_BYTES)]))
}

/// Lock `project`, calling `waiting` first if another headless CLI run holds the lock.
pub fn lock(project: &Path, waiting: impl FnOnce()) -> std::io::Result<File> {
    let file = File::options().create(true).truncate(false).read(true).write(true).open(lock_path(project))?;
    match file.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            waiting();
            file.lock()?;
        }
        Err(TryLockError::Error(e)) => return Err(e),
    }
    Ok(file)
}

/// Acquire the short-lived CLI lock for `project` when saving in place. Read-only folders or
/// filesystems without advisory locks log a diagnostic warning and return `None` (matching
/// `filmcraft-cli`).
pub fn for_project(project: &Path) -> Option<File> {
    if !project.is_file() {
        return None;
    }
    let shown = project.display();
    let waiting = || {
        let _ = std::io::Write::write_fmt(&mut std::io::stderr(), format_args!("magic-cli: waiting for another run saving {shown}\n"));
    };
    match lock(project, waiting) {
        Ok(file) => Some(file),
        Err(e) => {
            let _ = std::io::Write::write_fmt(
                &mut std::io::stderr(),
                format_args!("magic-cli: warning: can't lock {shown} ({e}); another run saving it now could overwrite these edits\n"),
            );
            None
        }
    }
}
