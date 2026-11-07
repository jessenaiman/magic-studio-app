//! Thin desktop shell: Tauri IPC and the control bridge share one engine session.

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod asset_scope;
mod capture;
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
    std::fs::write(&target, &png).map_err(|e| format!("could not write {}: {e}", target.display()))?;
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
    let desktop = state(app);
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
            let window = main_window(app)?;
            window.show().and_then(|_| window.set_focus()).map_err(|e| e.to_string())?;
            return Ok(json!({"ok":true}));
        }
        "ui.screenshot" => return screenshot(app, &params),
        "ui.resize" => return resize(app, &params),
        id if id.starts_with("ui.") => return ui_request(app, id, params),
        _ => {}
    }
    let changed = !magic::commands::is_read_only_command(id, &params);
    let mut session = desktop.session.lock().map_err(|e| e.to_string())?;
    if let Some(expected) = params.get("_expected_project").and_then(Value::as_str)
        && session.project_path.as_ref().and_then(|path| path.to_str()) != Some(expected)
    {
        return Err("The active project changed; the previous workspace command was not run".into());
    }
    let result = session.execute(id, params).map_err(|e| e.to_string())?;
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
        let project = session.project_path.clone();
        let data = if project.is_some() {
            let status = session.execute("workshop.status", json!({})).map_err(|e| e.to_string())?;
            let assets = session.execute("project.list_assets", json!({})).map_err(|e| e.to_string())?;
            json!({"project": project, "status": status, "storyboard": status.get("storyboard"), "assets": assets})
        } else {
            Value::Null
        };
        asset_scope::allow(&app, &session, &data)?;
        Ok(json!({"project":project,"data":data,"initialView":desktop.initial_view}))
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
    let token = std::env::var("MAGIC_CONTROL_TOKEN").unwrap_or_default();
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
    });
    tauri::Builder::default()
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
                diag(format_args!("magic-tauri: control listening on {address}"));
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| {
            diag(format_args!("magic-tauri: error while running Magic Studio: {error}"));
            std::process::exit(1)
        });
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    const DOC: &str = include_str!("../../../docs/control-protocol.md");
    const FRONTEND: &str = include_str!("../../magic/ui/desktop.ts");
    const SHELL: &str = include_str!("main.rs");

    /// Method ids in the first column of the control-protocol table (`engine.*`, `ui.*`, `app.*`).
    fn documented() -> BTreeSet<String> {
        let is_method = |id: &str| ["engine.", "ui.", "app."].iter().any(|prefix| id.starts_with(prefix));
        DOC.lines()
            .filter(|line| line.starts_with("| `"))
            .filter_map(|line| line.split('|').nth(1))
            .flat_map(|cell| cell.split('`').skip(1).step_by(2).map(str::to_owned).collect::<Vec<_>>())
            .filter(|id| is_method(id))
            .collect()
    }

    /// `case "ui.x":` labels in the frontend's request switch.
    fn frontend_cases() -> BTreeSet<String> {
        FRONTEND.split("case \"").skip(1).filter_map(|rest| rest.split('"').next()).filter(|id| id.starts_with("ui.")).map(str::to_owned).collect()
    }

    /// Methods the Rust shell answers itself: string literals in the non-test source.
    fn handled_by_shell(id: &str) -> bool {
        SHELL.split("#[cfg(test)]").next().is_some_and(|source| source.contains(&format!("\"{id}\"")))
    }

    /// The control protocol is three artifacts that must agree: the documented table, the shell's
    /// dispatch and the frontend's request switch. A method in one but not the others is a bug.
    #[test]
    fn control_methods_match_docs_and_frontend() {
        let docs = documented();
        assert!(docs.len() >= 15, "control-protocol.md table not found: {docs:?}");
        let frontend = frontend_cases();
        for id in &docs {
            assert!(frontend.contains(id) || handled_by_shell(id), "{id} is documented but nothing handles it");
        }
        for id in &frontend {
            assert!(docs.contains(id), "{id} is handled by the frontend but missing from docs/control-protocol.md");
        }
    }
}
