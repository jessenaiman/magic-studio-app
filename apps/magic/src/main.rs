//! Thin desktop shell: Tauri IPC and the control bridge share one engine session.

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod asset_scope;
mod capture;
mod clipboard_image;
mod control;
mod dialogs;
mod keymap;
mod lifecycle;
mod studio;
mod studio_audio;

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
#[command(name = "magic")]
struct Cli {
    #[arg(long)]
    project: Option<String>,
    #[arg(long, default_value = "projects", value_parser = ["home", "projects", "project", "storyboard", "lyrics", "design", "motion", "gallery", "animation", "studio"])]
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
    lifecycle: lifecycle::Lifecycle,
}

fn state(app: &tauri::AppHandle) -> Arc<Desktop> {
    app.state::<Arc<Desktop>>().inner().clone()
}

fn ui_request(app: &tauri::AppHandle, method: &str, params: Value) -> Result<Value, String> {
    let desktop = state(app);
    let id = desktop.next_request.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = mpsc::channel();
    desktop.pending.lock().map_err(|e| e.to_string())?.insert(id, tx);
    let result = app
        .emit("magic-ui-request", json!({"id":id,"method":method,"params":params}))
        .map_err(|e| e.to_string())
        .and_then(|_| rx.recv_timeout(Duration::from_secs(10)).map_err(|_| "UI did not respond within 10 seconds".to_string()))
        .and_then(|v| v);
    desktop.pending.lock().map_err(|e| e.to_string())?.remove(&id);
    result
}

/// Control parameters are hostile input: a key the method does not name is an error.
fn only_keys(params: &Value, method: &str, allowed: &[&str]) -> Result<(), String> {
    let object = params.as_object().ok_or("params must be an object")?;
    match object.keys().find(|key| !allowed.contains(&key.as_str())) {
        Some(key) => Err(format!("{method}: unknown parameter `{key}`; accepted: {}", allowed.join(", "))),
        None => Ok(()),
    }
}

fn main_window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    app.get_webview_window("main").ok_or_else(|| "main window missing".to_string())
}

/// `ui.screenshot {path?}`: PNG of the window, written to `path` (absolute, `.png`) or a temp file.
fn screenshot(app: &tauri::AppHandle, params: &Value) -> Result<Value, String> {
    only_keys(params, "ui.screenshot", &["path"])?;
    let path = match params.get("path") {
        None | Some(Value::Null) => None,
        Some(Value::String(path)) => Some(path.as_str()),
        Some(_) => return Err("path must be a string".into()),
    };
    let target = capture::target_path(path)?;
    let png = capture::snapshot(&main_window(app)?)?;
    magic::write_file(&target, &png).map_err(|e| format!("could not write {}: {e}", target.display()))?;
    let (width, height) = capture::png_size(&png).unwrap_or((0, 0));
    Ok(json!({"ok": true, "path": target, "width": width, "height": height, "bytes": png.len()}))
}

/// `ui.resize {width, height}`: logical window size, bounded to sizes a display can show.
fn resize(app: &tauri::AppHandle, params: &Value) -> Result<Value, String> {
    only_keys(params, "ui.resize", &["width", "height"])?;
    let side = |key: &str, range: std::ops::RangeInclusive<u64>| -> Result<f64, String> {
        let value = params.get(key).and_then(Value::as_u64).filter(|v| range.contains(v));
        value.map(|v| v as f64).ok_or_else(|| format!("{key} must be an integer from {} to {}", range.start(), range.end()))
    };
    let (width, height) = (side("width", 320..=7680)?, side("height", 240..=4320)?);
    let window = main_window(app)?;
    window.set_size(tauri::LogicalSize::new(width, height)).map_err(|e| e.to_string())?;
    // A tiling compositor or a maximized window may keep its own size: report what really happened.
    let deadline = std::time::Instant::now() + Duration::from_millis(1500);
    loop {
        let scale = window.scale_factor().map_err(|e| e.to_string())?;
        let actual = window.inner_size().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
        if (actual.width - width).abs() <= 2.0 && (actual.height - height).abs() <= 2.0 {
            return Ok(json!({"ok": true, "width": actual.width, "height": actual.height}));
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!(
                "the window manager kept the window at {}x{} (tiled and maximized windows cannot be resized)",
                actual.width.round(),
                actual.height.round()
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn dispatch(app: &tauri::AppHandle, id: &str, params: Value, notify: bool) -> Result<Value, String> {
    match id {
        "engine.execute" | "ui.menu.invoke" => {
            let command = params["command"].as_str().or_else(|| params["id"].as_str()).ok_or("missing command")?;
            if matches!(command, "engine.execute" | "ui.menu.invoke") {
                return Err("nested dispatch is not supported".into());
            }
            return dispatch(app, command, params.get("params").cloned().unwrap_or_else(|| json!({})), notify);
        }
        "engine.commands" | "ui.menu.list" => {
            return dispatch(app, "command.list", params, false);
        }
        "app.quit" | "app.restart" => {
            only_keys(&params, id, &[])?;
            return lifecycle::exit(app, id == "app.restart");
        }
        "app.hide" => {
            only_keys(&params, id, &[])?;
            main_window(app)?.hide().map_err(|e| e.to_string())?;
            return Ok(json!({"ok":true,"hidden":true}));
        }
        "ui.focus" if params.get("id").is_none() => {
            let window = main_window(app)?;
            window.show().and_then(|_| window.set_focus()).map_err(|e| e.to_string())?;
            return Ok(json!({"ok":true}));
        }
        "ui.screenshot" => return screenshot(app, &params),
        "ui.resize" => return resize(app, &params),
        "ui.dialog.state" | "ui.dialog.override" => return dialogs::control(app, id, &params),
        "ui.filmcraft" => return studio::control(&params),
        "ui.effectcraft" => return studio::control_effect(&params),
        "video.open" => return studio::open_project(app),
        "ui.keymap.press" => return keymap::control(app, &params),
        "ui.feedback" | "ui.look" => return ui_request(app, "ui.inspect", params),
        id if id.starts_with("ui.") => return ui_request(app, id, params),
        _ => {}
    }
    run_engine_command(app, id, params, notify)
}

fn run_engine_command(app: &tauri::AppHandle, id: &str, params: Value, notify: bool) -> Result<Value, String> {
    let desktop = state(app);
    let changed = !magic::commands::is_read_only_command(id, &params);
    if desktop.lifecycle.exiting() {
        return Err("Magic is closing or restarting; the command was not run".into());
    }
    let transition = if matches!(id, "project.open" | "project.new" | "project.close") {
        Some(lifecycle::begin(app)?)
    } else {
        if changed && desktop.lifecycle.transitioning() {
            return Err("A close or project switch is being checked; retry after it finishes".into());
        }
        None
    };
    let mut session = desktop.session.lock().map_err(|e| e.to_string())?;
    if let Some(transition) = &transition {
        transition.verify(&session)?;
    }
    if let Some(expected) = params.get("_expected_project").and_then(Value::as_str)
        && session.project_path.as_ref().and_then(|path| path.to_str()) != Some(expected)
    {
        return Err("The active project changed; the previous workspace command was not run".into());
    }
    let result = if id == "clipboard.import_image" {
        clipboard_image::import_image(app, &mut session, &params)?
    } else {
        session.execute(id, params).map_err(|e| e.to_string())?
    };
    asset_scope::allow(app, &session, &result)?;
    drop(session);
    if notify && changed {
        app.emit("magic-session-changed", ()).map_err(|e| e.to_string())?;
    }
    Ok(result)
}

#[tauri::command]
async fn execute_command(app: tauri::AppHandle, id: String, params: Value) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || dispatch(&app, &id, params, false)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn desktop_ready(app: tauri::AppHandle) {
    state(&app).ready.store(true, Ordering::Release);
}

#[tauri::command]
fn record_ui_error(app: tauri::AppHandle, message: String, source: Option<String>, control: Option<String>) {
    let project = state(&app).session.lock().ok().and_then(|s| s.project_path.clone());
    magic::activity::append(json!({
        "stage": "handler_error",
        "command": source.as_deref().unwrap_or("ui"),
        "project": project,
        "origin": "frontend",
        "control": control,
        "error": message.chars().take(350).collect::<String>(),
        "pid": std::process::id(),
        "severity": "error"
    }));
}

#[tauri::command]
fn ui_reply(app: tauri::AppHandle, id: u64, result: Option<Value>, error: Option<String>) -> Result<(), String> {
    if let Some(tx) = state(&app).pending.lock().map_err(|e| e.to_string())?.remove(&id) {
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
        let mut snapshot = session.execute("project.snapshot", json!({})).map_err(|e| e.to_string())?;
        asset_scope::allow(&app, &session, &snapshot["data"])?;
        snapshot["initialView"] = json!(desktop.initial_view);
        Ok(snapshot)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// One diagnostic line on stderr. `eprintln!` panics (exit 101) when stderr's reader is gone,
/// and a desktop app launched from a menu or a service has none.
fn diag(message: std::fmt::Arguments) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "{message}");
}

fn main() {
    let cli = Cli::parse();
    // Validate before creating windows; never silently start without the requested listener.
    let token = if cli.control.is_some() { magic::workspace::ensure_control_token().unwrap_or_default() } else { String::new() };
    if cli.control.is_some() && (token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit())) {
        diag(format_args!("MAGIC_CONTROL_TOKEN must contain 64 hexadecimal characters"));
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
        lifecycle: lifecycle::Lifecycle::default(),
    });
    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // Duplicate launches must not open a second session or silently lose arguments.
            if args.len() > 1 {
                log::warn!("Magic is already running; use magic-cli --bridge to open a project or switch views");
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show().and_then(|_| window.set_focus());
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_persisted_scope::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .target(tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir { file_name: None }))
                .build(),
        )
        .manage(desktop)
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event
                && window.label() == "main"
                && !state(window.app_handle()).lifecycle.exiting()
            {
                api.prevent_close();
                lifecycle::native_close(window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            execute_command,
            desktop_ready,
            record_ui_error,
            ui_reply,
            session_snapshot,
            dialogs::pick_file,
            keymap::key_pressed
        ])
        .setup(move |app| {
            studio::install_film_host();
            studio::install_effect_host();
            if let Some(path) = &cli.project {
                dispatch(app.handle(), "project.open", json!({"path":path}), false)?;
            }
            if let Some(spec) = &cli.control {
                let handle = app.handle().clone();
                let address = control::start(
                    spec,
                    &token,
                    Arc::new(move |id, params| {
                        if !state(&handle).ready.load(Ordering::Acquire) {
                            return Err("desktop not ready; retry after frontend initialization".into());
                        }
                        dispatch(&handle, id, params, true)
                    }),
                )?;
                state(app.handle()).session.lock().map_err(|_| "engine session lock poisoned")?.control_address = Some(address);
                diag(format_args!("magic: control listening on {address}"));
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .unwrap_or_else(|error| {
            diag(format_args!("magic: error while running Magic Studio: {error}"));
            std::process::exit(1)
        })
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event
                && !state(app).lifecycle.exiting()
            {
                api.prevent_exit();
                lifecycle::native_close(app);
            }
        });
}
