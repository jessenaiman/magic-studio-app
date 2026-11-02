//! Studio windows: find the app, open or reuse a window, and send it commands.
//! Discovery, focus and the socket protocol are the engine's (`studio_sessions`); this file
//! adds only what a caller needs: agent windows stay off the user's screen.
use magicstudio_engine::studio_sessions;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Herdr sets HERDR_PANE_ID in agent panes; their windows stay out of the way.
pub fn agent_caller() -> bool {
    std::env::var_os("HERDR_PANE_ID").is_some_and(|v| !v.is_empty())
}

fn executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// The native app binary: an explicit override, the one installed beside this CLI, the
/// user install, then PATH.
pub fn shell_binary() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("MAGIC_STUDIO_BIN").map(PathBuf::from)
        && path.is_file()
    {
        return Ok(path);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let beside = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("magic-studio")));
    let candidates = [
        beside,
        home.as_ref()
            .map(|h| h.join(".local/share/magic-studio/bin/magic-studio")),
        home.as_ref().map(|h| h.join(".local/bin/magic-studio")),
    ];
    let path_hit = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|d| d.join("magic-studio"))
            .find(|p| executable(p))
    });
    candidates
        .into_iter()
        .flatten()
        .find(|p| executable(p))
        .or(path_hit)
        .ok_or_else(|| {
            "Magic Studio (`magic-studio`) is not installed or built. Build it with:\n  CARGO_BUILD_JOBS=1 cargo build -p magic-studio".into()
        })
}

fn socket_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR").map(|r| PathBuf::from(r).join("magicstudio-studio"))
}

fn socket_path(endpoint: &str) -> Result<&Path, String> {
    endpoint
        .strip_prefix("unix:")
        .map(Path::new)
        .ok_or_else(|| "Invalid Studio session endpoint; use studio open to select a window".into())
}

/// Send one command to a window, stating which project and window the caller expects so a
/// restarted or reassigned window refuses rather than acting on the wrong song.
pub fn call(
    endpoint: &str,
    id: &str,
    mut params: Value,
    expected_project: Option<&str>,
    expected_instance: Option<&str>,
) -> Result<Value, String> {
    if let Some(project) = expected_project {
        let canonical = studio_sessions::canonical(Path::new(project))
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| project.to_owned());
        params["_expected_project"] = json!(canonical);
    }
    if let Some(instance) = expected_instance {
        params["_expected_instance"] = json!(instance);
    }
    if params.get("_client").is_none()
        && let Ok(pane) = std::env::var("HERDR_PANE_ID")
        && !pane.is_empty()
    {
        params["_client"] = json!(format!("herdr:{pane}"));
    }
    let socket = socket_path(endpoint)?;
    let reply = studio_sessions::request_slow(socket, id, params)?;
    // Every live reply names the window that answered it.
    let mut reply = if reply.is_object() {
        reply
    } else {
        json!({ "value": reply })
    };
    if reply.get("endpoint").is_none() {
        reply["endpoint"] = json!(format!("unix:{}", socket.display()));
    }
    if reply.get("instance").is_none() {
        reply["instance"] = json!(socket.file_name().and_then(|n| n.to_str()));
    }
    Ok(reply)
}

/// The one window a `ui.*` command without a pinned window means, or a clear refusal.
pub fn call_auto(id: &str, params: Value, project: Option<&str>) -> Result<Value, String> {
    if let Some(project) = project.filter(|p| Path::new(p).exists())
        && let Some(live) = studio_sessions::for_project(Path::new(project))?
    {
        let endpoint = live["endpoint"].as_str().unwrap_or_default().to_owned();
        return call(&endpoint, id, params, Some(project), None);
    }
    let found = studio_sessions::discover();
    let sessions: Vec<&Value> = if project.is_some() {
        Vec::new()
    } else {
        found["sessions"].as_array().into_iter().flatten().collect()
    };
    match sessions.as_slice() {
        [only] => call(
            only["endpoint"].as_str().unwrap_or_default(),
            id,
            params,
            project,
            None,
        ),
        [] if id == "ui.feedback" => Ok(json!({
            "ok": true, "connection": "not_observed_open", "project": project,
            "ui_ready": false, "unsaved_state": "unknown",
            "recovery": "No matching responsive Studio. No app launched or switched."
        })),
        [] => Err("No matching connected Studio; no command executed or app launched".into()),
        _ => {
            Err("Multiple Studios available; select a project or studio window explicitly.".into())
        }
    }
}

/// The window behind a socket once its UI is ready for this project. An unfocused agent
/// window is not painted, so it is ready once its DOM applied without errors.
fn ready_window(socket: &Path, project: Option<&str>) -> Option<Value> {
    let mut live = studio_sessions::request_within(socket, "ui.feedback", json!({}), 2).ok()?;
    let applied = live["phase"] == "ready" && live["rendered"] == true;
    let clean = live["errors"].as_array().is_none_or(Vec::is_empty);
    if live["ui_ready"] != true && !(applied && clean) {
        return None;
    }
    if project.is_some_and(|p| live["active_project"].as_str() != Some(p)) {
        return None;
    }
    live["painted"] = json!(live["ui_ready"] == true);
    live["endpoint"] = json!(format!("unix:{}", socket.display()));
    live["instance"] = json!(socket.file_name().and_then(|n| n.to_str()));
    Some(live)
}

fn sockets() -> Vec<PathBuf> {
    socket_dir()
        .and_then(|d| std::fs::read_dir(d).ok())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "sock"))
        .collect()
}

fn hyprctl() -> Option<PathBuf> {
    std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|d| d.join("hyprctl"))
            .find(|p| executable(p))
    })
}

/// MAGIC_AGENT_WORKSPACE, else the emptiest of workspaces 1-4 (ones the user can see).
fn empty_workspace(hyprctl: &Path) -> String {
    if let Ok(chosen) = std::env::var("MAGIC_AGENT_WORKSPACE") {
        return chosen;
    }
    let listing = std::process::Command::new(hyprctl)
        .args(["workspaces", "-j"])
        .output()
        .ok()
        .and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok())
        .unwrap_or(Value::Null);
    let windows = |n: u64| {
        listing
            .as_array()
            .into_iter()
            .flatten()
            .find(|w| w["id"] == n)
            .and_then(|w| w["windows"].as_u64())
            .unwrap_or(0)
    };
    (1..=4)
        .min_by_key(|&n| (windows(n), n))
        .unwrap_or(1)
        .to_string()
}

fn shell_quote(arg: &str) -> String {
    if !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./=:@".contains(c))
    {
        arg.to_owned()
    } else {
        format!("'{}'", arg.replace('\'', r"'\''"))
    }
}

/// Start the app unfocused on a quiet workspace, then wait for its new window.
fn open_for_agent(hyprctl: &Path, project: Option<&str>) -> Result<Value, String> {
    let mut argv = vec![shell_binary()?.to_string_lossy().into_owned()];
    match project {
        Some(p) => argv.push(p.to_owned()),
        None => argv.push("--new".into()),
    }
    let workspace = empty_workspace(hyprctl);
    let line = argv
        .iter()
        .map(|a| shell_quote(a))
        .collect::<Vec<_>>()
        .join(" ");
    let rule = format!("[workspace {workspace} silent] {line}");
    let before = sockets();
    let out = std::process::Command::new(hyprctl)
        .args(["dispatch", &format!("hl.dsp.exec_cmd({})", json!(rule))])
        .output()
        .map_err(|e| format!("Agent window launch failed: {e}"))?;
    let said = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() || said.trim() != "ok" {
        return Err(format!(
            "Agent window launch failed: {}{}",
            said,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        for socket in sockets().iter().filter(|s| !before.contains(s)) {
            if let Some(mut live) = ready_window(socket, project) {
                live["workspace"] = json!(workspace);
                return Ok(live);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err("Studio window (agent workspace) not ready within 30s.".into())
}

/// Open or reuse the window for `project`. A person's call brings it to the front; an
/// agent's call reuses it silently, or opens a new one off the user's screen.
pub fn open(project: Option<&str>) -> Result<Value, String> {
    if !agent_caller() {
        return studio_sessions::open(&shell_binary()?, project);
    }
    if let Some(p) = project
        && let Some(existing) = studio_sessions::for_project(Path::new(p))?
    {
        return Ok(existing);
    }
    match hyprctl() {
        Some(hyprctl) => open_for_agent(&hyprctl, project),
        None => studio_sessions::open(&shell_binary()?, project),
    }
}

pub fn discover() -> Value {
    studio_sessions::discover()
}
