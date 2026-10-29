//! Exact latest-main Magic CLI observer calls. No persisted caller binding or bridge.
use serde_json::Value;
use std::{
    collections::HashSet,
    io::Read,
    process::{Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
const LIMIT: u64 = 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(10);
#[derive(Default)]
struct State {
    closed: bool,
    groups: HashSet<i32>,
}
#[derive(Clone, Default)]
pub struct Control(Arc<Mutex<State>>);
pub struct Owner(pub Control);
impl Drop for Owner {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
impl Control {
    pub fn cancel(&self) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.closed = true;
        for group in state.groups.drain() {
            stop(group);
        }
    }
}
struct Group {
    control: Control,
    id: i32,
}
impl Group {
    fn stop(&self) {
        let mut state = self.control.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.groups.remove(&self.id) {
            stop(self.id);
        }
    }
}
impl Drop for Group {
    fn drop(&mut self) {
        self.stop();
    }
}
#[cfg(target_os = "linux")]
fn stop(group: i32) {
    if group > 1 {
        let _ = nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(group),
            nix::sys::signal::Signal::SIGKILL,
        );
    }
}
#[cfg(not(target_os = "linux"))]
fn stop(_: i32) {}
#[derive(Clone, Copy)]
pub enum Query {
    Template,
    Root,
    Crew,
}
impl Query {
    fn id(self) -> &'static str {
        match self {
            Self::Template => "storyboard.template",
            Self::Root => "project.root",
            Self::Crew => "crew.list",
        }
    }
}
pub struct Results {
    pub template: Result<Value, String>,
    pub root: Result<Value, String>,
    pub crew: Result<Value, String>,
}
pub fn read(control: &Control) -> Results {
    Results {
        template: query(control, Query::Template),
        root: query(control, Query::Root),
        crew: query(control, Query::Crew),
    }
}
#[cfg(not(target_os = "linux"))]
fn query(_: &Control, _: Query) -> Result<Value, String> {
    Err("Provider unavailable: isolated observer currently supports Linux only".into())
}
#[cfg(target_os = "linux")]
fn query(control: &Control, query: Query) -> Result<Value, String> {
    use std::os::unix::{fs::FileTypeExt, process::CommandExt};
    if !std::fs::symlink_metadata("/dev/null").is_ok_and(|m| m.file_type().is_char_device()) {
        return Err("Provider isolation unavailable: /dev/null is not a character device".into());
    }
    // Hold the lifecycle lock through spawn+registration: quit cannot miss a child.
    let mut state = control.0.lock().unwrap_or_else(|e| e.into_inner());
    if state.closed {
        return Err("Provider cancelled".into());
    }
    let mut child = Command::new("magic-cli")
        .args([
            "--session-file",
            "/dev/null",
            "exec",
            query.id(),
            "--params",
            "{}",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "Command absent: magic-cli is not installed".into()
            } else {
                format!("Provider launch error: {e}")
            }
        })?;
    let group = i32::try_from(child.id()).map_err(|_| "Provider process id out of range")?;
    state.groups.insert(group);
    drop(state);
    let group_guard = Group {
        control: control.clone(),
        id: group,
    };
    let stdout = child.stdout.take().ok_or("Provider stdout unavailable")?;
    let stderr = child.stderr.take().ok_or("Provider stderr unavailable")?;
    let out = read_pipe(stdout);
    let err = read_pipe(stderr);
    let (tx, rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (reaped_tx, reaped_rx) = mpsc::channel();
    std::thread::spawn(move || {
        use nix::sys::wait::{Id, WaitPidFlag, waitid};
        // Observe completion without reaping: the zombie leader reserves its PID/PGID
        // until group cleanup+unregistration, so a reused unrelated group cannot be hit.
        let observed = waitid(
            Id::Pid(nix::unistd::Pid::from_raw(group)),
            WaitPidFlag::WEXITED | WaitPidFlag::WNOWAIT,
        );
        let _ = tx.send(observed);
        let _ = release_rx.recv();
        let _ = reaped_tx.send(child.wait());
    });
    let observed = rx.recv_timeout(DEADLINE);
    group_guard.stop();
    let _ = release_tx.send(());
    let reaped = reaped_rx.recv_timeout(Duration::from_secs(2));
    let stdout = out
        .recv_timeout(Duration::from_secs(2))
        .map_err(|_| "Provider stdout cleanup deadline exceeded")??;
    let stderr = err
        .recv_timeout(Duration::from_secs(2))
        .map_err(|_| "Provider stderr cleanup deadline exceeded")??;
    observed
        .map_err(|_| "Provider timeout: isolated CLI/native process group stopped")?
        .map_err(|e| format!("Provider completion observation failed: {e}"))?;
    let status = reaped
        .map_err(|_| "Provider reap deadline exceeded")?
        .map_err(|e| format!("Provider wait error: {e}"))?;
    if stdout.len() > LIMIT as usize || stderr.len() > LIMIT as usize {
        return Err("Provider output exceeded 1 MiB bound".into());
    }
    if !status.success() {
        let detail = crate::data::clean(&String::from_utf8_lossy(&stderr));
        let category = if detail.contains("magicstudio-shell")
            && (detail.contains("not found")
                || detail.contains("Could not find")
                || detail.contains("not installed"))
        {
            "Native backend absent"
        } else {
            "Provider error"
        };
        return Err(format!(
            "{category}: {status}: {}",
            detail.chars().take(1000).collect::<String>()
        ));
    }
    let value: Value =
        serde_json::from_slice(&stdout).map_err(|e| format!("Provider invalid JSON: {e}"))?;
    if !value.is_object() || value.get("ok") == Some(&Value::Bool(false)) {
        return Err(format!(
            "Provider rejected response: {}",
            crate::data::clean(&value.to_string())
                .chars()
                .take(1000)
                .collect::<String>()
        ));
    }
    Ok(value)
}
fn bounded(input: impl Read) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    input
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Provider pipe error: {e}"))?;
    Ok(bytes)
}

fn read_pipe(input: impl Read + Send + 'static) -> mpsc::Receiver<Result<Vec<u8>, String>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(bounded(input));
    });
    receiver
}
