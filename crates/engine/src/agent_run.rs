//! Run a local agent CLI (Codex) with a hard timeout, capturing both streams.

use std::path::Path;
use std::time::{Duration, Instant};

use crate::{EngineError, Result};

/// The last `chars` of a stream, with the tail marker that makes a truncation visible.
fn tail(text: &str, chars: usize) -> String {
    let trimmed = text.trim();
    let out: String = trimmed
        .chars()
        .rev()
        .take(chars)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if trimmed.chars().count() > chars {
        format!("…{out}")
    } else {
        out
    }
}

/// What one agent invocation produced.
pub(crate) struct AgentRun {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) code: Option<i32>,
    pub(crate) timed_out: bool,
    pub(crate) stopped_after_image: bool,
}

impl AgentRun {
    pub(crate) fn succeeded(&self) -> bool {
        !self.timed_out && self.code == Some(0)
    }

    /// The part of the agent's own report worth showing when it failed.
    pub(crate) fn failure_detail(&self) -> String {
        if !self.stderr.trim().is_empty() {
            tail(&self.stderr, 800)
        } else {
            tail(&self.stdout, 800)
        }
    }
}

/// Run the agent with a hard timeout, capturing both streams. A long run must not freeze the
/// app, so on timeout the agent is stopped and whatever it wrote is still checked.
pub(crate) fn run_agent(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout_s: u64,
) -> Result<AgentRun> {
    run_agent_inner(program, args, cwd, timeout_s, false)
}

/// Image bytes are already staged when the native success receipt arrives. Stop this one-shot
/// agent before it can retry generation while trying to reconstruct/copy binary tool output.
pub(crate) fn run_image_agent(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout_s: u64,
) -> Result<AgentRun> {
    run_agent_inner(program, args, cwd, timeout_s, true)
}

fn spawn_stdout_reader(
    pipe: Option<std::process::ChildStdout>,
) -> std::sync::mpsc::Receiver<Vec<u8>> {
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

fn spawn_stderr_reader(
    pipe: Option<std::process::ChildStderr>,
    stop_on_image: bool,
) -> (
    std::sync::mpsc::Receiver<Vec<u8>>,
    std::sync::mpsc::Receiver<()>,
) {
    let (tx_err, rx_err) = std::sync::mpsc::channel::<Vec<u8>>();
    let (image_tx, image_rx) = std::sync::mpsc::channel::<()>();
    if let Some(mut pipe) = pipe {
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            let mut reader = BufReader::new(&mut pipe);
            let mut buf = Vec::new();
            let mut line = Vec::new();
            let mut signalled = false;
            loop {
                line.clear();
                match reader.read_until(b'\n', &mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                buf.extend_from_slice(&line);
                if stop_on_image
                    && !signalled
                    && crate::image_generation::native_image_success(&String::from_utf8_lossy(
                        &line,
                    ))
                {
                    signalled = true;
                    let _ = image_tx.send(());
                }
            }
            let _ = tx_err.send(buf);
        });
    }
    (rx_err, image_rx)
}

fn wait_for_agent_child(
    child: &mut std::process::Child,
    deadline: Instant,
    stop_on_image: bool,
    image_rx: &std::sync::mpsc::Receiver<()>,
) -> (Option<i32>, bool, bool) {
    let mut timed_out = false;
    let mut stopped_after_image = false;
    let code = loop {
        if stop_on_image && image_rx.try_recv().is_ok() {
            stopped_after_image = true;
            let _ = child.kill();
            break child.wait().ok().and_then(|status| status.code());
        }
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
    (code, timed_out, stopped_after_image)
}

fn run_agent_inner(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout_s: u64,
    stop_on_image: bool,
) -> Result<AgentRun> {
    let mut command = std::process::Command::new(program);
    // `codex exec` defaults to error-only stderr. The native image importer needs
    // its INFO tool receipt to correlate staged bytes; never rely on the parent's log level.
    if stop_on_image {
        command.env("RUST_LOG", "error,codex_otel=info");
    }
    let mut child = command
        .current_dir(cwd)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| EngineError::ExternalTool(program.to_string(), e.to_string()))?;
    let rx_out = spawn_stdout_reader(child.stdout.take());
    let (rx_err, image_rx) = spawn_stderr_reader(child.stderr.take(), stop_on_image);
    let deadline = Instant::now() + Duration::from_secs(timeout_s.max(1));
    let (code, timed_out, stopped_after_image) =
        wait_for_agent_child(&mut child, deadline, stop_on_image, &image_rx);
    let stdout = rx_out
        .recv_timeout(Duration::from_secs(15))
        .unwrap_or_default();
    let stderr = rx_err
        .recv_timeout(Duration::from_secs(15))
        .unwrap_or_default();
    Ok(AgentRun {
        stdout: String::from_utf8_lossy(&stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&stderr).trim().to_string(),
        code,
        timed_out,
        stopped_after_image,
    })
}
