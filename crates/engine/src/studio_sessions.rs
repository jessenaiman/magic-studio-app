//! Existing Studio endpoints, selected by canonical project path. No daemon or separate registry.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    time::Duration,
};
pub fn canonical(path: &Path) -> Result<PathBuf, String> {
    std::fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()))
}
/// A busy Studio can refuse a connection with EAGAIN while its accept queue is full.
/// Retry briefly: a dropped command is worse than a short wait.
#[cfg(unix)]
fn connect_with_retry(path: &Path) -> Result<std::os::unix::net::UnixStream, String> {
    let mut last = String::new();
    for attempt in 0..20 {
        match std::os::unix::net::UnixStream::connect(path) {
            Ok(stream) => return Ok(stream),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::ConnectionRefused {
                    return Err(format!("stale endpoint: {e}"));
                }
                last = e.to_string();
                if attempt < 19 {
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }
    Err(last)
}
#[cfg(unix)]
pub fn request(path: &Path, id: &str, params: Value) -> Result<Value, String> {
    request_within(path, id, params, 3)
}

/// A command can take a while (a model turn, an import). Give it room, and say plainly
/// that the Studio was busy instead of reporting a bare socket timeout.
pub fn request_slow(path: &Path, id: &str, params: Value) -> Result<Value, String> {
    request_within(
        path,
        id,
        params,
        if crate::find_command(id).is_some_and(|c| c.long_running) {
            3600
        } else {
            30
        },
    )
}
#[cfg(unix)]
fn check_endpoint(path: &Path) -> Result<(), String> {
    crate::local_ipc::private_path(path.parent().ok_or("Endpoint directory missing")?, true)?;
    crate::local_ipc::private_path(path, false)
}

/// Connect, prove the peer is the Studio, and bound both directions by the same deadline.
#[cfg(unix)]
fn connected(
    path: &Path,
    timeout_seconds: u64,
) -> Result<BufReader<std::os::unix::net::UnixStream>, String> {
    let stream = connect_with_retry(path)?;
    crate::local_ipc::peer(&stream).map_err(|e| format!("Studio peer check failed: {e}"))?;
    let timeout = Some(Duration::from_secs(timeout_seconds));
    stream.set_read_timeout(timeout).map_err(|e| e.to_string())?;
    stream.set_write_timeout(timeout).map_err(|e| e.to_string())?;
    Ok(BufReader::new(stream))
}

#[cfg(unix)]
fn retryable(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock
            | std::io::ErrorKind::TimedOut
            | std::io::ErrorKind::Interrupted
    )
}

/// One answer line, retried briefly; a silent Studio is reported as busy, not as a raw timeout.
#[cfg(unix)]
fn read_reply(
    reader: &mut BufReader<std::os::unix::net::UnixStream>,
    timeout_seconds: u64,
) -> Result<String, String> {
    let mut line = String::new();
    for attempt in 0..4 {
        match reader.read_line(&mut line) {
            Ok(_) => break,
            Err(error) if retryable(&error) && attempt < 3 => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => {
                return Err(format!(
                    "The Studio did not answer within {timeout_seconds}s ({error}); it is busy or the request is still running. Inspect the Studio before retrying — no duplicate request was sent."
                ));
            }
        }
    }
    Ok(line)
}

#[cfg(unix)]
fn reply_result(line: &str) -> Result<Value, String> {
    let reply: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    if reply["ok"] == true {
        Ok(reply["result"].clone())
    } else {
        Err(reply["error"]
            .as_str()
            .unwrap_or("Studio request failed")
            .into())
    }
}

#[cfg(unix)]
pub fn request_within(
    path: &Path,
    id: &str,
    params: Value,
    timeout_seconds: u64,
) -> Result<Value, String> {
    check_endpoint(path)?;
    let mut reader = connected(path, timeout_seconds)?;
    writeln!(reader.get_mut(), "{}", json!({"id":id,"params":params}))
        .map_err(|e| e.to_string())?;
    let line = read_reply(&mut reader, timeout_seconds)?;
    reply_result(&line)
}
/// The private directory Studio endpoints live in: Err is an error to report, Ok(None) none exists.
#[cfg(unix)]
fn sessions_dir() -> Result<Option<PathBuf>, String> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR unavailable")?;
    let dir = PathBuf::from(runtime).join("magicstudio-studio");
    if !dir.exists() {
        return Ok(None);
    }
    crate::local_ipc::private_path(&dir, true)?;
    Ok(Some(dir))
}

/// Another window's socket: not our own pid's endpoint, and a socket at that.
#[cfg(unix)]
fn other_session_socket(path: &Path) -> bool {
    let own = format!("{}-", std::process::id());
    !path
        .file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with(&own))
        && path.extension().is_some_and(|e| e == "sock")
}

#[cfg(unix)]
fn probe_session(path: &Path) -> Result<Value, String> {
    let mut value = request_within(path, "ui.feedback", json!({}), 1)?;
    value["endpoint"] = json!(format!("unix:{}", path.display()));
    value["instance"] = json!(path.file_name().and_then(|p| p.to_str()));
    if let Some(raw) = value["active_project"].as_str() {
        value["canonical_project"] = json!(canonical(Path::new(raw)).ok());
    }
    Ok(value)
}

#[cfg(unix)]
fn probe_error(path: &Path, error: String) -> Value {
    json!({"endpoint":path,"error":error,"state":if error.starts_with("stale endpoint:"){"stale endpoint; no listener"}else{"unresponsive or inaccessible; ownership unknown"}})
}

#[cfg(unix)]
fn collect_sessions(dir: &Path) -> (Vec<Value>, Vec<Value>) {
    let mut sessions = Vec::new();
    let mut errors = Vec::new();
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if !other_session_socket(&path) {
            continue;
        }
        match probe_session(&path) {
            Ok(value) => sessions.push(value),
            // A socket that vanished while we looked is gone, not broken.
            Err(_) if matches!(path.try_exists(), Ok(false)) => {}
            Err(error) => errors.push(probe_error(&path, error)),
        }
    }
    (sessions, errors)
}

pub fn discover() -> Value {
    #[cfg(unix)]
    {
        let dir = match sessions_dir() {
            Ok(Some(dir)) => dir,
            Ok(None) => return json!({"sessions":[],"errors":[]}),
            Err(error) => return json!({"sessions":[],"errors":[error]}),
        };
        let (sessions, errors) = collect_sessions(&dir);
        json!({"sessions":sessions,"errors":errors})
    }
    #[cfg(not(unix))]
    {
        json!({"sessions":[],"errors":["Private Studio discovery unsupported on this platform"]})
    }
}
pub fn for_project(path: &Path) -> Result<Option<Value>, String> {
    let project = canonical(path)?;
    let found = discover();
    let matches: Vec<Value> = found["sessions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["canonical_project"].as_str() == project.to_str())
        .cloned()
        .collect();
    if matches.is_empty()
        && found["errors"].as_array().is_some_and(|v| {
            v.iter().any(|e| {
                !e["error"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("stale endpoint:"))
            })
        })
    {
        return Err(format!("Studio ownership is unknown: {}", found["errors"]));
    }
    select(&matches)
}

fn select(matches: &[Value]) -> Result<Option<Value>, String> {
    if matches.len() < 2 {
        return Ok(matches.first().cloned());
    }
    let writers = matches
        .iter()
        .filter(|s| s["writer_owner"] == true)
        .collect::<Vec<_>>();
    if writers.len() == 1 {
        return Ok(Some(writers[0].clone()));
    }
    // Older builds lack the owner field. An explicit read-only attachment cannot be the writer.
    let possible = matches
        .iter()
        .filter(|s| s["writer_owner"] != false)
        .collect::<Vec<_>>();
    if writers.is_empty() && possible.len() == 1 {
        return Ok(Some(possible[0].clone()));
    }
    Err("Multiple Studio sessions report this project; no unique writer. Inspect magic-cli studio list and select the intended project; no command executed".into())
}

/// Shared app opening for desktop, CLI and MCP. Existing windows are never replaced.
pub fn open(executable: &Path, project: Option<&str>) -> Result<Value, String> {
    open_with_options(executable, project, None)
}

/// Focus the window already serving this project; None when there is none to reuse.
fn focus_existing(project: &Path) -> Result<Option<Value>, String> {
    let Some(existing) = for_project(project)? else {
        return Ok(None);
    };
    let endpoint = existing["endpoint"]
        .as_str()
        .ok_or("Studio endpoint missing")?;
    let socket = Path::new(endpoint.trim_start_matches("unix:"));
    match request(
        socket,
        "ui.focus",
        json!({"_expected_instance":existing["instance"],"_expected_project":project}),
    ) {
        Ok(_) => Ok(Some(existing)),
        // The endpoint went away between discovery and the request: open a window instead.
        Err(_) if matches!(socket.try_exists(), Ok(false)) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Linux reports a replaced running binary with a " (deleted)" suffix; launch the same path.
fn live_executable(executable: &Path) -> PathBuf {
    if executable.is_file() {
        return executable.to_path_buf();
    }
    executable
        .to_str()
        .and_then(|path| path.strip_suffix(" (deleted)"))
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .unwrap_or_else(|| executable.to_path_buf())
}

/// The window this launch asks for: character selection and an explicit project.
fn launch_command(
    executable: &Path,
    project: Option<&Path>,
    character: Option<&str>,
) -> std::process::Command {
    let mut command = std::process::Command::new(executable);
    if let Some(ch) = character {
        command.args(["--character", ch]);
    }
    if let Some(path) = project {
        command.arg(path);
    } else {
        command.arg("--new");
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    command
}

/// The endpoint this pid reported as ready for the requested project, if it has one yet.
fn ready_endpoint(dir: &Path, pid: u32, project: Option<&Path>) -> Option<Value> {
    let own = format!("{pid}-");
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if !path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with(&own))
        {
            continue;
        }
        let Ok(mut live) = request(&path, "ui.feedback", json!({})) else {
            continue;
        };
        if live["ui_ready"] != true || live["identity"]["pid"] != pid {
            continue;
        }
        if project.is_some_and(|p| live["active_project"].as_str() != p.to_str()) {
            continue;
        }
        live["endpoint"] = json!(format!("unix:{}", path.display()));
        live["instance"] = json!(path.file_name().and_then(|v| v.to_str()));
        return Some(live);
    }
    None
}

/// Wait for the window this pid started to report ready, or say how it ended.
fn await_ready(
    child: &mut std::process::Child,
    dir: &Path,
    project: Option<&Path>,
) -> Result<Value, String> {
    let pid = child.id();
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(30) {
        if let Some(live) = ready_endpoint(dir, pid, project) {
            return Ok(live);
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return Err(format!(
                "Studio process {pid} exited during opening: {status}"
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err(format!(
        "Studio process {pid} started but has not reported a ready window; it was left running. No other window was changed."
    ))
}

/// Open Magic Studio with an optional `--character` selection.
pub fn open_with_options(
    executable: &Path,
    project: Option<&str>,
    character: Option<&str>,
) -> Result<Value, String> {
    let project = project.map(Path::new).map(canonical).transpose()?;
    if let Some(path) = &project
        && let Some(existing) = focus_existing(path)?
    {
        return Ok(existing);
    }
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR missing")?;
    let dir = PathBuf::from(runtime).join("magicstudio-studio");
    let executable = live_executable(executable);
    let mut command = launch_command(&executable, project.as_deref(), character);
    let mut child = command
        .spawn()
        .map_err(|e| format!("Cannot launch {}: {e}", executable.display()))?;
    await_ready(&mut child, &dir, project.as_deref())
}
