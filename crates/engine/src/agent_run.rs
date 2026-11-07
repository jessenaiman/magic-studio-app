//! Run a local agent CLI (Codex) with a hard timeout, capturing both streams.

use std::path::Path;
use std::time::{Duration, Instant};

use crate::{EngineError, Result};

/// The last `chars` of a stream, with the tail marker that makes a truncation visible.
fn tail(text: &str, chars: usize) -> String {
    let trimmed = text.trim();
    let out: String = trimmed.chars().rev().take(chars).collect::<String>().chars().rev().collect();
    if trimmed.chars().count() > chars { format!("…{out}") } else { out }
}

/// What one agent invocation produced.
pub(crate) struct AgentRun {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) code: Option<i32>,
    pub(crate) timed_out: bool,
}

impl AgentRun {
    pub(crate) fn succeeded(&self) -> bool {
        !self.timed_out && self.code == Some(0)
    }

    /// The part of the agent's own report worth showing when it failed.
    pub(crate) fn failure_detail(&self) -> String {
        if !self.stderr.trim().is_empty() { tail(&self.stderr, 800) } else { tail(&self.stdout, 800) }
    }
}

/// Run the agent with a hard timeout, capturing both streams. A long run must not freeze the
/// app, so on timeout the agent is stopped and whatever it wrote is still checked.
pub(crate) fn run_agent(program: &str, args: &[String], cwd: &Path, timeout_s: u64) -> Result<AgentRun> {
    let mut command = std::process::Command::new(program);
    let mut child = command
        .current_dir(cwd)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| EngineError::ExternalTool(program.to_string(), e.to_string()))?;
    let rx_out = spawn_stdout_reader(child.stdout.take());
    let rx_err = spawn_stderr_reader(child.stderr.take());
    let deadline = Instant::now() + Duration::from_secs(timeout_s.max(1));
    let (code, timed_out) = wait_for_agent_child(&mut child, deadline);
    let stdout = rx_out.recv_timeout(Duration::from_secs(15)).unwrap_or_default();
    let stderr = rx_err.recv_timeout(Duration::from_secs(15)).unwrap_or_default();
    Ok(AgentRun { stdout: String::from_utf8_lossy(&stdout).trim().to_string(), stderr: String::from_utf8_lossy(&stderr).trim().to_string(), code, timed_out })
}

fn spawn_stdout_reader(pipe: Option<std::process::ChildStdout>) -> std::sync::mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
    if let Some(mut pipe) = pipe {
        std::thread::spawn(move || {
            use std::io::Read;
            let mut buf = Vec::new();
            let _ = pipe.read_to_end(&mut buf);
            let _ = tx.send(buf);
        });
    }
    rx
}

fn spawn_stderr_reader(pipe: Option<std::process::ChildStderr>) -> std::sync::mpsc::Receiver<Vec<u8>> {
    let (tx_err, rx_err) = std::sync::mpsc::channel::<Vec<u8>>();
    if let Some(mut pipe) = pipe {
        std::thread::spawn(move || {
            use std::io::Read;
            let mut buf = Vec::new();
            let _ = pipe.read_to_end(&mut buf);
            let _ = tx_err.send(buf);
        });
    }
    rx_err
}

fn wait_for_agent_child(child: &mut std::process::Child, deadline: Instant) -> (Option<i32>, bool) {
    let mut timed_out = false;
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) => {}
            Err(_) => break None,
        }
        if Instant::now() >= deadline {
            timed_out = true;
            let _ = child.kill();
            break child.wait().ok().and_then(|status| status.code());
        }
        std::thread::sleep(Duration::from_millis(150));
    };
    (code, timed_out)
}
