//! Tauri transport for the Backstage PTY owner; no socket or command dispatcher.
use crate::backstage_pty::{self, TerminalState};
use serde_json::Value;
use tauri::{Manager, ipc::Channel};
fn desktop_only(webview: &tauri::Webview) -> Result<(), String> {
    if webview.label() != "project" {
        return Err("Backstage is available only in the desktop workspace.".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn backstage_start(
    app: tauri::AppHandle,
    webview: tauri::Webview,
    cols: u16,
    rows: u16,
    output: Channel<Value>,
) -> Result<u64, String> {
    desktop_only(&webview)?;
    tauri::async_runtime::spawn_blocking(move || {
        backstage_pty::start(&app.state::<TerminalState>(), cols, rows, move |message| {
            output.send(message).map_err(|e| e.to_string())
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn backstage_write(
    app: tauri::AppHandle,
    webview: tauri::Webview,
    session: u64,
    bytes: Vec<u8>,
) -> Result<(), String> {
    desktop_only(&webview)?;
    tauri::async_runtime::spawn_blocking(move || {
        backstage_pty::write(&app.state::<TerminalState>(), session, bytes)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn backstage_resize(
    app: tauri::AppHandle,
    webview: tauri::Webview,
    session: u64,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    desktop_only(&webview)?;
    tauri::async_runtime::spawn_blocking(move || {
        backstage_pty::resize(&app.state::<TerminalState>(), session, cols, rows)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn backstage_stop(
    app: tauri::AppHandle,
    webview: tauri::Webview,
    session: u64,
) -> Result<(), String> {
    desktop_only(&webview)?;
    tauri::async_runtime::spawn_blocking(move || {
        backstage_pty::stop(&app.state::<TerminalState>(), Some(session))
    })
    .await
    .map_err(|e| e.to_string())?
}
pub fn cleanup(app: &tauri::AppHandle) {
    if let Err(error) = backstage_pty::stop(&app.state::<TerminalState>(), None) {
        eprintln!("{error}");
    }
}

#[tauri::command]
pub async fn backstage_ack(
    app: tauri::AppHandle,
    webview: tauri::Webview,
    session: u64,
    sequence: u64,
) -> Result<(), String> {
    desktop_only(&webview)?;
    tauri::async_runtime::spawn_blocking(move || {
        backstage_pty::acknowledge(&app.state::<TerminalState>(), session, sequence)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn backstage_current(
    app: tauri::AppHandle,
    webview: tauri::Webview,
) -> Result<Option<u64>, String> {
    desktop_only(&webview)?;
    tauri::async_runtime::spawn_blocking(move || {
        backstage_pty::current(&app.state::<TerminalState>())
    })
    .await
    .map_err(|e| e.to_string())?
}
