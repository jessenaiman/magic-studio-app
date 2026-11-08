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
    input_path: PathBuf,
    stamp: Option<FileStamp>,
    _work_dir: ScopedTempDir,
}

/// Commands that only read the document, so a batch of them needs no save of the work copy.
const READ_ONLY: &[&str] = &["document.inspect", "object.findByLabel", "story.get", "preflight.run", "comp.open", "prop.get"];

impl CraftDoc {
    /// Copy `path` to a private work file. Refuses when the file changes during the copy.
    pub(crate) fn open(target: CraftTarget, path: &Path) -> Result<Self> {
        Self::from_source(target, path, path)
    }

    /// Open `path` if it exists on disk, or prepare a fresh work path to create it.
    pub(crate) fn open_or_create(target: CraftTarget, path: &Path) -> Result<Self> {
        if path.is_file() {
            return Self::open(target, path);
        }
        let work_dir = ScopedTempDir::new(&format!("magic-{}-work", target.name()))?;
        let name = path.file_name().ok_or_else(|| EngineError::InvalidParams(format!("{} has no file name", path.display())))?;
        let work_path = work_dir.0.join(name);
        Ok(Self { target, path: path.to_path_buf(), input_path: work_path.clone(), work_path, stamp: None, _work_dir: work_dir })
    }

    /// Copy `source` to a private work file whose `save` writes atomically to `path`.
    pub(crate) fn from_source(target: CraftTarget, source: &Path, path: &Path) -> Result<Self> {
        let stamp = FileStamp::of(path);
        let work_dir = ScopedTempDir::new(&format!("magic-{}-work", target.name()))?;
        let name = path.file_name().ok_or_else(|| EngineError::InvalidParams(format!("{} has no file name", path.display())))?;
        let work_path = work_dir.0.join(name);
        let src_name = source.file_name().unwrap_or(name);
        let input_path = if src_name == name {
            std::fs::copy(source, &work_path)?;
            work_path.clone()
        } else {
            let src_copy = work_dir.0.join(src_name);
            std::fs::copy(source, &src_copy)?;
            src_copy
        };
        crate::link_watch::save_guard(path, stamp)?;
        Ok(Self { target, path: path.to_path_buf(), work_path, input_path, stamp, _work_dir: work_dir })
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
        let mut cmd = batch_command(self.target, &self.input_path, &self.work_path, steps, mutates)?;
        if mutates {
            self.input_path = self.work_path.clone();
        }
        let use_stdin = !matches!(self.target, CraftTarget::Photo);
        if use_stdin {
            cmd.stdin(Stdio::piped());
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let tool = |e: std::io::Error| EngineError::ExternalTool(self.target.cli_name().into(), e.to_string());
        let mut child = cmd.spawn().map_err(tool)?;
        if use_stdin && let Some(mut stdin) = child.stdin.take() {
            let payload: Vec<Value> = steps.iter().map(|(id, params)| json!({"command": id, "params": params})).collect();
            let bytes = serde_json::to_vec(&payload).map_err(|e| EngineError::InvalidParams(e.to_string()))?;
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
        if let Some(report) = report {
            if let Some(list) = report.as_array().or_else(|| report["result"].as_array()) {
                return Ok(list.iter().map(|e| e.get("result").cloned().unwrap_or_else(|| e.clone())).collect());
            }
            return Ok(report["results"].as_array().cloned().unwrap_or_default());
        }
        Ok(String::from_utf8_lossy(&out.stdout).lines().filter_map(|line| serde_json::from_str::<Value>(line).ok()).collect())
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

/// The app's command line that runs `steps` against `input`, saving to `work` when `save` is set.
fn batch_command(target: CraftTarget, input: &Path, work: &Path, steps: &[(&str, Value)], save: bool) -> Result<Command> {
    let cli = crate::workspace::find_executable(target.cli_name()).map_err(|e| EngineError::ExternalTool(target.cli_name().into(), e.to_string()))?;
    let mut cmd = Command::new(cli);
    match target {
        CraftTarget::Design => {
            cmd.arg("script").arg("-").arg("--in").arg(input);
            if save {
                cmd.arg("--save").arg(work);
            }
        }
        CraftTarget::Effect => {
            cmd.arg("--json");
            if input.is_file() {
                cmd.arg("--project").arg(input);
                if save {
                    cmd.arg("--save");
                }
            } else {
                cmd.arg("--empty");
                if save {
                    cmd.arg("--save-as").arg(work);
                }
            }
            cmd.arg("run").arg("--commands").arg("-");
        }
        CraftTarget::Photo => {
            cmd.arg("run").arg(input);
            for (id, params) in steps {
                cmd.arg("--cmd").arg(id).arg("--params").arg(params.to_string());
            }
            if save {
                cmd.arg("--out").arg(work);
                if input == work {
                    cmd.arg("--in-place");
                }
            }
        }
        CraftTarget::Vector => {
            cmd.arg("run");
            if input.is_file() {
                cmd.arg("--in").arg(input);
            }
            cmd.arg("--commands").arg("-");
            if save {
                cmd.arg("--export").arg(work);
            }
        }
        other => {
            return Err(EngineError::ExternalTool(other.cli_name().into(), "document batches are not supported for this target yet".into()));
        }
    }
    Ok(cmd)
}
