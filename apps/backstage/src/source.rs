//! Fixed read operations only. This adapter never accepts an arbitrary command from UI data.
use nix::{
    fcntl::{FcntlArg, OFlag, fcntl},
    sys::signal::{Signal, killpg},
    unistd::Pid,
};
use serde_json::Value;
use std::os::unix::process::CommandExt;
use std::{
    io::{Read, Write},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

struct Observation {
    child: Child,
    reaped: bool,
}
impl Drop for Observation {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = killpg(Pid::from_raw(self.child.id() as i32), Signal::SIGKILL);
            let _ = self.child.wait();
        }
    }
}
fn drain(pipe: &mut impl Read, bytes: &mut Vec<u8>) -> Result<bool, String> {
    let mut buffer = [0; 8192];
    loop {
        match pipe.read(&mut buffer) {
            Ok(0) => return Ok(true),
            Ok(n) => {
                bytes.extend_from_slice(&buffer[..n]);
                if bytes.len() > 8 * 1024 * 1024 {
                    return Err("Observation exceeds 8 MiB".into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(false),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
}
fn output(
    program: &str,
    args: &[&str],
    input: Option<String>,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, String> {
    let binding = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(program);
    cmd.args(args)
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("MAGIC_SESSION_FILE", binding.path().join("session.json"));
    let child = cmd.spawn().map_err(|e| format!("{program}: {e}"))?;
    let mut owned = Observation {
        child,
        reaped: false,
    };
    if let Some(input) = input {
        owned
            .child
            .stdin
            .take()
            .ok_or("No stdin")?
            .write_all(input.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    drop(owned.child.stdin.take());
    let mut out = owned.child.stdout.take().ok_or("No stdout")?;
    let mut err = owned.child.stderr.take().ok_or("No stderr")?;
    fcntl(&out, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).map_err(|e| e.to_string())?;
    fcntl(&err, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err("Observation cancelled on exit".into());
        }
        if Instant::now() >= deadline {
            return Err(format!("{program}: observation timed out"));
        }
        let out_done = drain(&mut out, &mut stdout)?;
        let err_done = drain(&mut err, &mut stderr)?;
        // Keep the child unreaped until both pipes close, preserving its process-group ID
        // for reliable timeout cleanup even if a descendant inherited a pipe.
        if out_done
            && err_done
            && let Some(status) = owned.child.try_wait().map_err(|e| e.to_string())?
        {
            owned.reaped = true;
            if !status.success() {
                return Err(String::from_utf8_lossy(&stderr)
                    .trim()
                    .chars()
                    .take(800)
                    .collect());
            }
            return Ok(stdout);
        }
        thread::sleep(Duration::from_millis(25));
    }
}

pub fn catalog(cancel: &AtomicBool) -> Result<Value, String> {
    run("magic-cli", &["exec", "project.catalog"], None, cancel)
}

pub fn changes(cancel: &AtomicBool) -> Result<Value, String> {
    run("magic-cli", &["backstage", "changes"], None, cancel)
}

pub fn audit(cancel: &AtomicBool) -> Result<Value, String> {
    // Do not rebrand app.analytics or reconstruct the provider's audit report here.
    run("magic-cli", &["audit"], None, cancel)
}

pub fn run(
    program: &str,
    args: &[&str],
    input: Option<String>,
    cancel: &AtomicBool,
) -> Result<Value, String> {
    serde_json::from_slice(&output(program, args, input, cancel)?)
        .map_err(|e| format!("{program}: invalid JSON: {e}"))
}

pub fn inspect(project: &str, cancel: &AtomicBool) -> Result<Value, String> {
    if !std::path::Path::new(project).is_absolute() {
        return Err("Saved-project observation requires an absolute path".into());
    }
    let expected =
        std::fs::canonicalize(project).map_err(|e| format!("Project identity unavailable: {e}"))?;
    // PR #12's isolated registry query. Never fall back to attach/status or --project.
    let param = format!("path={project}");
    let report =
        run("magic-cli", &["backstage", "observe", &param], None, cancel).map_err(|e| {
            format!("Saved-project observer unavailable (requires Magic backstage.observe): {e}")
        })?;
    if report["ok"] != true || report["observation"]["source"] != "saved_project_file" {
        return Err("Magic did not return the saved-project observation contract".into());
    }
    if report["project"].as_str() != expected.to_str() {
        return Err("Magic returned a different project identity; observation discarded".into());
    }
    Ok(report)
}
