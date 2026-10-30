//! Same-user Studio discovery and request transport.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// Same-user window IPC. Files are private; discovery does not require visibility into host PIDs.
#[cfg(unix)]
pub fn serve(app: AppHandle) -> Result<(), String> {
    use std::os::unix::{
        fs::{DirBuilderExt, PermissionsExt},
        net::UnixListener,
    };
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .ok_or("XDG_RUNTIME_DIR is missing; private UI feedback unavailable")?;
    let dir = std::path::PathBuf::from(&runtime).join("magicstudio-studio");
    magicstudio_engine::local_ipc::private_path(std::path::Path::new(&runtime), true)?;
    if !dir.exists() {
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&dir)
            .map_err(|e| e.to_string())?;
    }
    magicstudio_engine::local_ipc::private_path(&dir, true)?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let socket = dir.join(format!("{}-{nonce}.sock", std::process::id()));
    let listener = UnixListener::bind(&socket).map_err(|e| e.to_string())?;
    app.manage(LocalSocket(socket.clone()));
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            if magicstudio_engine::local_ipc::peer(&stream).is_err() {
                continue;
            }
            let app = app.clone();
            std::thread::spawn(move || {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                let Ok(mut writer) = stream.try_clone() else {
                    return;
                };
                for line in BufReader::new(stream).lines().map_while(Result::ok) {
                    let reply = match serde_json::from_str::<Value>(&line) {
                        Ok(v) => {
                            let id = v["id"].as_str().unwrap_or_default();
                            let p = v.get("params").cloned().unwrap_or(json!({}));
                            match crate::desktop_commands::handle(&app, id, &p) {
                                Ok(result) => json!({"ok":true,"result":result}),
                                Err(e) => json!({"ok":false,"error":e}),
                            }
                        }
                        Err(e) => json!({"ok":false,"error":e.to_string()}),
                    };
                    if writeln!(writer, "{reply}").is_err() {
                        break;
                    }
                }
            });
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
