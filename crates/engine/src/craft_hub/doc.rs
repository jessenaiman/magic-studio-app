//! A Craft app's document opened by Magic: a private work copy edited by the app's own CLI,
//! written back over the original only through a `FileStamp` guard.
//!
//! Magic never parses or writes a Craft document's format itself. It runs batches of the app's
//! commands against the work copy ([`CraftDoc::batch`]) and, when the edit is done, replaces the
//! original atomically ([`CraftDoc::save`]), refusing if another app or agent changed it since
//! it was opened. Each Craft app's batch command line lives in [`batch_command`]; nothing else
//! in the engine knows it.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

use super::CraftTarget;
use crate::link_watch::FileStamp;
use crate::{EngineError, Result};

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

/// A private folder removed when dropped.
pub(crate) struct ScopedTempDir(PathBuf);

impl ScopedTempDir {
    pub(crate) fn new(prefix: &str) -> Result<Self> {
        let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("{prefix}-{}-{seq}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        Ok(Self(dir))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScopedTempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An open Craft document: the original's path and stamp, and the work copy batches edit.
pub(crate) struct CraftDoc {
    target: CraftTarget,
    path: PathBuf,
    work_path: PathBuf,
    stamp: Option<FileStamp>,
    _work_dir: ScopedTempDir,
}

/// Commands that only read the document, so a batch of them needs no save of the work copy.
const READ_ONLY: &[&str] = &["document.inspect", "object.findByLabel", "story.get", "preflight.run"];

impl CraftDoc {
    /// Copy `path` to a private work file. Refuses when the file changes during the copy.
    pub(crate) fn open(target: CraftTarget, path: &Path) -> Result<Self> {
        let stamp = FileStamp::of(path);
        let work_dir = ScopedTempDir::new(&format!("magic-{}-work", target.name()))?;
        let name = path.file_name().ok_or_else(|| EngineError::InvalidParams(format!("{} has no file name", path.display())))?;
        let work_path = work_dir.0.join(name);
        std::fs::copy(path, &work_path)?;
        crate::link_watch::save_guard(path, stamp)?;
        Ok(Self { target, path: path.to_path_buf(), work_path, stamp, _work_dir: work_dir })
    }

    /// The original document.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// The work copy batches edit and renders read.
    pub(crate) fn work_path(&self) -> &Path {
        &self.work_path
    }

    /// Run `steps` against the work copy in one app process, saving the copy when any step
    /// changes it, and return each step's result.
    pub(crate) fn batch(&mut self, steps: &[(&str, Value)]) -> Result<Vec<Value>> {
        let mutates = steps.iter().any(|(id, _)| !READ_ONLY.contains(id));
        let mut cmd = batch_command(self.target, &self.work_path, mutates)?;
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let tool = |e: std::io::Error| EngineError::ExternalTool(self.target.cli_name().into(), e.to_string());
        let mut child = cmd.spawn().map_err(tool)?;
        let payload: Vec<Value> = steps.iter().map(|(id, params)| json!({"command": id, "params": params})).collect();
        let bytes = serde_json::to_vec(&payload).map_err(|e| EngineError::InvalidParams(e.to_string()))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&bytes).map_err(tool)?;
        }
        let out = child.wait_with_output().map_err(tool)?;
        let report = serde_json::from_slice::<Value>(&out.stdout).ok();
        if let Some(err) = report.as_ref().and_then(|r| r["failed"]["error"].as_str()) {
            return Err(EngineError::CommandFailed(self.target.cli_name().into(), err.to_string()));
        }
        if !out.status.success() {
            return Err(EngineError::ExternalTool(self.target.cli_name().into(), String::from_utf8_lossy(&out.stderr).trim().to_string()));
        }
        let report = report.ok_or_else(|| EngineError::ExternalTool(self.target.cli_name().into(), "batch reply is not JSON".into()))?;
        Ok(report["results"].as_array().cloned().unwrap_or_default())
    }

    /// Replace the original with the work copy. Refuses when another writer changed the
    /// original since it was opened or last saved.
    pub(crate) fn save(&mut self) -> Result<()> {
        crate::link_watch::save_guard(&self.path, self.stamp)?;
        let bytes = std::fs::read(&self.work_path)?;
        crate::doc_state::write_file(&self.path, bytes)?;
        self.stamp = FileStamp::of(&self.path);
        Ok(())
    }
}

/// The app's command line that reads a JSON batch on stdin against `work`, saving it in place
/// when `save` is set.
fn batch_command(target: CraftTarget, work: &Path, save: bool) -> Result<Command> {
    let cli = crate::workspace::find_executable(target.cli_name()).map_err(|e| EngineError::ExternalTool(target.cli_name().into(), e.to_string()))?;
    let mut cmd = Command::new(cli);
    match target {
        CraftTarget::Design => {
            cmd.arg("script").arg("-").arg("--in").arg(work);
            if save {
                cmd.arg("--save").arg(work);
            }
        }
        other => {
            return Err(EngineError::ExternalTool(other.cli_name().into(), "document batches are routed through CraftHub for DesignCraft only so far".into()));
        }
    }
    Ok(cmd)
}
