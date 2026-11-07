//! Tauri desktop entry point for Magic Studio GUI (Tauri edition).
//! Frontend (apps/magic/ui, React Strict TS) talks to magic-engine via Tauri IPC.
//! magic-cli and engine remain pure Rust; only this GUI crate uses Tauri.
//! CLI via tauri-plugin-cli (tauri.conf.json plugins.cli) + clap fallback for --project/--view/--control/--lyrics.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;
use magic::{command_specs, Session};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tauri::generate_handler;
use tauri_plugin_cli::CliExt;

#[derive(Parser, Debug)]
#[command(name = "magic")]
struct Cli {
    #[arg(long)] project: Option<String>,
    #[arg(long)] view: Option<String>,
    #[arg(long)] control: Option<String>,
    #[arg(long)] lyrics: Option<String>,
}

type AppSession = Arc<Mutex<Session>>;

#[tauri::command]
fn execute_command(state: tauri::State<'_, AppSession>, id: String, params: Value) -> Result<Value, String> {
    let mut session = state.lock().map_err(|e| e.to_string())?;
    session.execute(&id, params).map_err(|e| e.to_string())
}

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
    if let Some(lyrics) = &cli.lyrics {
        let _ = session.execute("project.open", json!({ "path": lyrics }));
    }
    let session = Arc::new(Mutex::new(session));
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_cli::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_log::Builder::default().level(log::LevelFilter::Info).build())
        .plugin(tauri_plugin_websocket::init())
        .manage(session)
        .invoke_handler(generate_handler![execute_command, command_list])
        .setup(|app| {
            // Tauri CLI plugin matches (tauri.conf.json plugins.cli) available to JS via plugin-cli
            let matches = app.cli().matches().ok();
            eprintln!("magic-tauri cli matches: {:?}", matches);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
