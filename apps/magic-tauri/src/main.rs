//! Thin desktop shell: Tauri IPC and the control bridge share one engine session.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod control;

use clap::Parser;
use magic::Session;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc,
};
use std::time::Duration;
use tauri::{Emitter, Manager};

#[derive(Parser, Debug)]
#[command(name = "magic-tauri")]
struct Cli {
    #[arg(long)]
    project: Option<String>,
    #[arg(long, default_value = "projects", value_parser = ["home", "projects", "project", "storyboard", "lyrics", "design", "animation", "studio"])]
    view: String,
    #[arg(long, env = "MAGIC_CONTROL_PORT")]
    control: Option<String>,
}

type Reply = mpsc::Sender<Result<Value, String>>;
struct Desktop {
    session: Mutex<Session>,
    initial_view: String,
    ready: AtomicBool,
    next_request: AtomicU64,
    pending: Mutex<HashMap<u64, Reply>>,
}

fn state(app: &tauri::AppHandle) -> Arc<Desktop> {
    app.state::<Arc<Desktop>>().inner().clone()
}

fn ui_request(app: &tauri::AppHandle, method: &str, params: Value) -> Result<Value, String> {
    let desktop = state(app);
    let id = desktop.next_request.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = mpsc::channel();
    desktop
        .pending
        .lock()
        .map_err(|e| e.to_string())?
        .insert(id, tx);
    let result = app
        .emit(
            "magic-ui-request",
            json!({"id":id,"method":method,"params":params}),
        )
        .map_err(|e| e.to_string())
        .and_then(|_| {
            rx.recv_timeout(Duration::from_secs(10))
                .map_err(|_| "UI did not respond within 10 seconds".to_string())
        })
        .and_then(|v| v);
    desktop
        .pending
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&id);
    result
}

fn dispatch(
    app: &tauri::AppHandle,
    id: &str,
    params: Value,
    notify: bool,
) -> Result<Value, String> {
    let desktop = state(app);
    match id {
        "engine.execute" | "ui.menu.invoke" => {
            let command = params["command"]
                .as_str()
                .or_else(|| params["id"].as_str())
                .ok_or("missing command")?;
            if matches!(command, "engine.execute" | "ui.menu.invoke") {
                return Err("nested dispatch is not supported".into());
            }
            return dispatch(
                app,
                command,
                params.get("params").cloned().unwrap_or_else(|| json!({})),
                notify,
            );
        }
        "engine.commands" | "ui.menu.list" => {
            return dispatch(app, "command.list", params, false);
        }
        "app.quit" => {
            // Let the control reply flush before the process exits.
            let app = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(150));
                app.exit(0);
            });
            return Ok(json!({"ok":true,"quitting":true}));
        }
        "ui.focus" if params.get("id").is_none() => {
            let window = app
                .get_webview_window("main")
                .ok_or("main window missing")?;
            window
                .show()
                .and_then(|_| window.set_focus())
                .map_err(|e| e.to_string())?;
            return Ok(json!({"ok":true}));
        }
        id if id.starts_with("ui.") => return ui_request(app, id, params),
        _ => {}
    }
    let changed = !magic::commands::is_read_only_command(id, &params);
    let result = desktop
        .session
        .lock()
        .map_err(|e| e.to_string())?
        .execute(id, params)
        .map_err(|e| e.to_string())?;
    if notify && changed {
        app.emit("magic-session-changed", ())
            .map_err(|e| e.to_string())?;
    }
    Ok(result)
}

#[tauri::command]
async fn execute_command(
    app: tauri::AppHandle,
    id: String,
    params: Value,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || dispatch(&app, &id, params, false))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn desktop_ready(app: tauri::AppHandle) {
    state(&app).ready.store(true, Ordering::Release);
}

#[tauri::command]
fn ui_reply(
    app: tauri::AppHandle,
    id: u64,
    result: Option<Value>,
    error: Option<String>,
) -> Result<(), String> {
    if let Some(tx) = state(&app)
        .pending
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&id)
    {
        let _ = tx.send(match error {
            Some(e) => Err(e),
            None => Ok(result.unwrap_or(Value::Null)),
        });
    }
    Ok(())
}

#[tauri::command]
async fn session_snapshot(app: tauri::AppHandle) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let desktop = state(&app);
        let mut session = desktop.session.lock().map_err(|e| e.to_string())?;
        let project = session.project_path.clone();
        let data = if project.is_some() {
            let status = session.execute("workshop.status", json!({})).map_err(|e| e.to_string())?;
            let assets = session.execute("project.list_assets", json!({})).map_err(|e| e.to_string())?;
            json!({"project": project, "status": status, "storyboard": status.get("storyboard"), "assets": assets})
        } else {
            Value::Null
        };
        Ok(json!({"project":project,"data":data,"initialView":desktop.initial_view}))
    })
    .await
    .map_err(|e| e.to_string())?
}

fn main() {
    let cli = Cli::parse();
    // Validate before creating windows; never silently start without the requested listener.
    let token = std::env::var("MAGIC_CONTROL_TOKEN").unwrap_or_default();
    if cli.control.is_some() && (token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        eprintln!("MAGIC_CONTROL_TOKEN must contain 64 hexadecimal characters");
        std::process::exit(2);
    }
    let initial_view = match cli.view.as_str() {
        "projects" => "project",
        "lyrics" => "design",
        other => other,
    }
    .to_string();
    let desktop = Arc::new(Desktop {
        session: Mutex::new(Session::user_session()),
        initial_view,
        ready: AtomicBool::new(false),
        next_request: AtomicU64::new(1),
        pending: Mutex::new(HashMap::new()),
    });
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // Duplicate launches must not open a second session or silently lose arguments.
            if args.len() > 1 { log::warn!("Magic is already running; use magic-cli --bridge to open a project or switch views"); }
            if let Some(window) = app.get_webview_window("main") { let _ = window.show().and_then(|_| window.set_focus()); }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_log::Builder::default().level(log::LevelFilter::Info).build())
        .plugin(tauri_plugin_websocket::init())
        .manage(desktop)
        .invoke_handler(tauri::generate_handler![execute_command, desktop_ready, ui_reply, session_snapshot])
        .setup(move |app| {
            if let Some(path) = &cli.project {
                state(app.handle()).session.lock().map_err(|e| e.to_string())?.execute("project.open", json!({"path":path}))?;
            }
            if let Some(spec) = &cli.control {
                let handle = app.handle().clone();
                let address = control::start(spec, &token, Arc::new(move |id, params| {
                    if !state(&handle).ready.load(Ordering::Acquire) { return Err("desktop not ready; retry after frontend initialization".into()); }
                    dispatch(&handle, id, params, true)
                }))?;
                eprintln!("magic-tauri: control listening on {address}");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Magic Studio");
}
