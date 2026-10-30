//! Same-user Studio discovery and request transport.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// The private directory the window's endpoint lives in, created with owner-only access.
#[cfg(unix)]
fn endpoint_dir(runtime: &std::ffi::OsStr) -> Result<std::path::PathBuf, String> {
    use std::os::unix::fs::DirBuilderExt;
    let dir = std::path::PathBuf::from(runtime).join("magicstudio-studio");
    magicstudio_engine::local_ipc::private_path(std::path::Path::new(runtime), true)?;
    if !dir.exists() {
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&dir)
            .map_err(|e| e.to_string())?;
    }
    magicstudio_engine::local_ipc::private_path(&dir, true)?;
    Ok(dir)
}

/// Bind this process's endpoint and make it reachable by this user only.
#[cfg(unix)]
fn bind_endpoint(
    dir: &std::path::Path,
) -> Result<(std::path::PathBuf, std::os::unix::net::UnixListener), String> {
    use std::os::unix::fs::PermissionsExt;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let socket = dir.join(format!("{}-{nonce}.sock", std::process::id()));
    let listener =
        std::os::unix::net::UnixListener::bind(&socket).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    Ok((socket, listener))
}

/// One request line answered with one reply line.
#[cfg(unix)]
fn reply_to(app: &AppHandle, line: &str) -> Value {
    match serde_json::from_str::<Value>(line) {
        Ok(v) => {
            let id = v["id"].as_str().unwrap_or_default();
            let p = v.get("params").cloned().unwrap_or(json!({}));
            match crate::desktop_commands::handle(app, id, &p) {
                Ok(result) => json!({"ok":true,"result":result}),
                Err(e) => json!({"ok":false,"error":e}),
            }
        }
        Err(e) => json!({"ok":false,"error":e.to_string()}),
    }
}

/// Serve one verified peer until it stops writing.
#[cfg(unix)]
fn serve_stream(app: AppHandle, stream: std::os::unix::net::UnixStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    for line in BufReader::new(stream).lines().map_while(Result::ok) {
        let reply = reply_to(&app, &line);
        if writeln!(writer, "{reply}").is_err() {
            break;
        }
    }
}

/// Same-user window IPC. Files are private; discovery does not require visibility into host PIDs.
#[cfg(unix)]
pub fn serve(app: AppHandle) -> Result<(), String> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .ok_or("XDG_RUNTIME_DIR is missing; private UI feedback unavailable")?;
    let dir = endpoint_dir(&runtime)?;
    let (socket, listener) = bind_endpoint(&dir)?;
    app.manage(LocalSocket(socket));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            if magicstudio_engine::local_ipc::peer(&stream).is_err() {
                continue;
            }
            let app = app.clone();
            std::thread::spawn(move || serve_stream(app, stream));
        }
    });
    Ok(())
}

#[cfg(unix)]
pub struct LocalSocket(pub std::path::PathBuf);
#[cfg(unix)]
pub fn cleanup(app: &AppHandle) {
    if let Some(socket) = app.try_state::<LocalSocket>()
        && magicstudio_engine::local_ipc::private_path(&socket.0, false).is_ok()
    {
        let _ = std::fs::remove_file(&socket.0);
    }
}
