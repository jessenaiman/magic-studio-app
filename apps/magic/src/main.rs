//! Tauri desktop entry point for Magic Studio GUI.
//!
//! The `magic` binary is a Tauri webview app whose frontend talks to
//! `magic-engine` through Tauri IPC commands. `magic-cli` and the engine
//! remain pure Rust; only this GUI crate uses Tauri.

#![cfg_attr(mobile, tauri::mobile_entry_point)]

use clap::Parser;
use magic_engine::{command_specs, Session};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tauri::generate_handler;

/// CLI arguments parsed with clap so `magic --project /abs/Project.magic` works.
#[derive(Parser)]
#[command(name = "magic")]
struct Cli {
    /// Select the intended .magic project
    #[arg(long)]
    project: Option<String>,
}

type AppSession = Arc<Mutex<Session>>;

/// Generic command dispatcher: the frontend sends `{domain}.{verb}` + params,
/// and this dispatches to `magic-engine` through the same registry `magic-cli` uses.
#[tauri::command]
fn execute_command(
    state: tauri::State<'_, AppSession>,
    id: String,
    params: Value,
) -> Result<Value, String> {
    let mut session = state.lock().map_err(|e| e.to_string())?;
    session.execute(&id, params).map_err(|e| e.to_string())
}

/// The project the session has open (from `--project` or a later `project.open`), if any.
#[tauri::command]
fn current_project(state: tauri::State<'_, AppSession>) -> Option<String> {
    let session = state.lock().ok()?;
    session.project_path.as_ref().map(|p| p.display().to_string())
}

/// List all registered command IDs from the engine registry.
#[tauri::command]
fn command_list() -> Vec<String> {
    command_specs().iter().map(|c| c.id.to_string()).collect()
}

fn main() {
    let cli = Cli::parse();

    let mut session = Session::user_session();
    if let Some(path) = &cli.project {
        let _ = session.execute("project.open", json!({ "path": path }));
    }

    let session = Arc::new(Mutex::new(session));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(session)
        .invoke_handler(generate_handler![execute_command, command_list, current_project])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
