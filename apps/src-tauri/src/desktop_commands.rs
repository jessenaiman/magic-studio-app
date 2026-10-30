//! Guarded desktop operations shared by Tauri actions and same-user requests.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

use crate::Engine;

/// Replies owed by the project webview, by token.
#[derive(Default)]
pub struct Pending(Mutex<(u64, HashMap<u64, Sender<Value>>)>);

/// The page's answer to one `eval`.
#[tauri::command]
pub fn desktop_reply(pending: tauri::State<Pending>, token: u64, value: Value) {
    if let Some(tx) = pending.0.lock().ok().and_then(|mut p| p.1.remove(&token)) {
        let _ = tx.send(value);
    }
}

/// Runs `body` (the inside of an async function) in the project view and returns what it returns.
pub fn eval(app: &AppHandle, body: &str) -> Result<Value, String> {
    let view = app.get_webview("project").ok_or("no project view")?;
    let (tx, rx) = channel();
    let token = {
        let pending = app.state::<Pending>();
        let mut p = pending.0.lock().map_err(|e| e.to_string())?;
        p.0 += 1;
        let token = p.0;
        p.1.insert(token, tx);
        token
    };
    let reply = |v: &str| {
        format!("window.__TAURI__.core.invoke(\"desktop_reply\", {{token: {token}, value: {v}}})")
    };
    view.eval(format!(
        "(async () => {{ {body} }})().then(v => {}, e => {})",
        reply("{ok: true, value: v ?? null}"),
        reply("{ok: false, error: String((e && e.stack) || e)}")
    ))
    .map_err(|e| e.to_string())?;
    let v = rx
        .recv_timeout(Duration::from_secs(60))
        .map_err(|_| "the page did not answer".to_string())?;
    if v["ok"].as_bool() == Some(true) {
        Ok(v["value"].clone())
    } else {
        Err(v["error"].as_str().unwrap_or("page error").to_string())
    }
}

/// `magicUI.<name>(args…)` in the page.
fn page(app: &AppHandle, name: &str, args: &[&Value]) -> Result<Value, String> {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    eval(
        app,
        &format!("return await magicUI.{name}({});", args.join(", ")),
    )
}

/// A png path for a picture: `output`, else `<project>/.agent/looks/` (projects stay self-contained).
fn output(app: &AppHandle, params: &Value, name: &str) -> Result<std::path::PathBuf, String> {
    if let Some(out) = params["output"].as_str() {
        return Ok(out.into());
    }
    let project = app
        .state::<Engine>()
        .with(None, |s| Ok(s.project_path.clone()))?;
    let dir = project
        .as_deref()
        .and_then(std::path::Path::parent)
        .ok_or("no project open: pass output=<png>")?
        .join(".agent")
        .join("looks");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    Ok(dir.join(format!("{name}-{stamp}.png")))
}

/// A pinned request names the project it expects; a different one is refused, not switched.
fn check_pinned_project(app: &AppHandle, id: &str, params: &Value) -> Result<(), String> {
    let Some(expected) = params["_expected_project"].as_str() else {
        return Ok(());
    };
    let expected = magicstudio_engine::studio_sessions::canonical(std::path::Path::new(expected))?;
    // Read-only observation must not wait for the operation it is observing.
    let actual = if id == "ui.feedback" {
        crate::feedback::snapshot(app)?["active_project"]
            .as_str()
            .map(std::path::PathBuf::from)
    } else {
        app.state::<Engine>()
            .with(None, |s| Ok(s.project_path.clone()))?
    };
    if actual.as_deref() == Some(expected.as_path()) {
        return Ok(());
    }
    Err("Pinned Studio changed project; request refused without switching".into())
}

/// A pinned request also names the window instance it expects.
#[cfg(unix)]
fn check_pinned_instance(app: &AppHandle, params: &Value) -> Result<(), String> {
    let Some(expected) = params["_expected_instance"].as_str() else {
        return Ok(());
    };
    let socket = app.state::<crate::local_ipc::LocalSocket>();
    if socket.0.file_name().and_then(|p| p.to_str()) != Some(expected) {
        return Err("Studio instance changed; explicit reattach required".into());
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_pinned_instance(_app: &AppHandle, _params: &Value) -> Result<(), String> {
    Ok(())
}

/// A dry run of a write command reports the route instead of running it.
fn dry_run_reply(id: &str, params: &Value) -> Option<Value> {
    let dry_run = params.get("_dry_run").and_then(Value::as_bool) == Some(true);
    if !dry_run || magicstudio_engine::commands::is_read_only_command(id, params) {
        return None;
    }
    Some(json!({
        "ok": true,
        "dry_run": true,
        "route": "desktop",
        "command": id,
        "params": params,
    }))
}

/// The page commands: each forwards its parameters to one UI page script.
fn page_command(
    app: &AppHandle,
    id: &str,
    params: &Value,
    none: &Value,
) -> Option<Result<Value, String>> {
    let result = match id {
        "ui.menu" => page(app, "menu", &[params]),
        "ui.controls" => page(app, "controls", &[params]),
        "ui.attention" => page(app, "attention", &[params]),
        "ui.components" => page(app, "components", &[params]),
        "ui.unconfigured" => page(app, "unconfigured", &[]),
        "ui.state" => page(app, "state", &[]),
        "ui.elements" => page(app, "elements", &[params.get("region").unwrap_or(none)]),
        "ui.click" => page(
            app,
            "click",
            &[
                params.get("id").or(params.get("target")).unwrap_or(none),
                params.get("instance").unwrap_or(none),
                params.get("files").unwrap_or(none),
            ],
        ),
        "ui.drag" => page(app, "drag", &[params]),
        "ui.set" => page(
            app,
            "set",
            &[
                params.get("id").or(params.get("target")).unwrap_or(none),
                params.get("value").unwrap_or(none),
                params.get("instance").unwrap_or(none),
            ],
        ),
        "ui.key" => page(
            app,
            "key",
            &[
                params.get("key").unwrap_or(none),
                params.get("id").unwrap_or(none),
                params.get("instance").unwrap_or(none),
            ],
        ),
        _ => return None,
    };
    Some(result)
}

/// One call: click (optional), wait for the page to settle, picture the region, report the
/// app state and the clickable ids inside it.
fn look_command(app: &AppHandle, params: &Value, none: &Value) -> Result<Value, String> {
    if let Some(target) = params.get("click").filter(|v| !v.is_null()) {
        page(app, "click", &[target])?;
        std::thread::sleep(Duration::from_millis(
            params["settle_ms"].as_u64().unwrap_or(450),
        ));
    }
    let region = params.get("region").unwrap_or(none);
    let pad = params.get("pad").cloned().unwrap_or(json!(0));
    let rect: Option<[f64; 4]> =
        serde_json::from_value(page(app, "rect", &[region, &pad])?).map_err(|e| e.to_string())?;
    let out = output(app, params, "look")?;
    crate::snapshot_to(app, rect, &out)?;
    Ok(
        json!({"output": out, "state": page(app, "state", &[])?, "elements": page(app, "elements", &[region])?}),
    )
}

/// Close the workspace after reporting the request, then exit this process.
fn quit_command(app: &AppHandle, id: &str, params: &Value) -> Result<Value, String> {
    crate::feedback::guard(app, id, false)?;
    magicstudio_engine::activity::append(
        json!({"stage":"close_requested","command":"app.quit","origin":params["_origin"],"client_label":params["_client"],"request_id":params["_request_id"],"project":crate::feedback::snapshot(app)?["active_project"]}),
    );
    let close_app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        #[cfg(unix)]
        crate::local_ipc::cleanup(&close_app);
        close_app.exit(0);
    });
    Ok(
        json!({"ok":true,"status":"close_requested","exited":false,"verification":"Check endpoint/process exit after this response; no force kill or silent discard."}),
    )
}

fn text_edit_command(app: &AppHandle, params: &Value) -> Result<Value, String> {
    let action = params["action"].as_str().ok_or("Choose an edit action")?;
    crate::menu::edit_text(app, action)
}

fn eval_command(app: &AppHandle, params: &Value) -> Result<Value, String> {
    eval(app, params["js"].as_str().ok_or("ui.eval needs js")?)
}

fn focus_command(app: &AppHandle) -> Result<Value, String> {
    let window = app.get_window("main").ok_or("Studio window unavailable")?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(json!({"ok":true,"focused":true,"reused_session":true}))
}

fn screenshot_command(app: &AppHandle, params: &Value) -> Result<Value, String> {
    let out = output(app, params, "screenshot")?;
    crate::snapshot_to(app, None, &out)?;
    Ok(json!({"output": out}))
}

/// The window and application commands that do not go through the engine.
fn action_command(
    app: &AppHandle,
    id: &str,
    params: &Value,
    none: &Value,
) -> Option<Result<Value, String>> {
    let result = match id {
        "ui.text_edit" => text_edit_command(app, params),
        "ui.eval" => eval_command(app, params),
        "ui.feedback" => crate::feedback::snapshot(app),
        "ui.focus" => focus_command(app),
        "ui.screenshot" => screenshot_command(app, params),
        "ui.look" => look_command(app, params, none),
        "app.quit" => quit_command(app, id, params),
        _ => return None,
    };
    Some(result)
}

/// Everything else runs through the engine and then refreshes the page that shows it.
fn engine_command(
    app: &AppHandle,
    id: &str,
    params: &Value,
    ui_project_request: bool,
) -> Result<Value, String> {
    if id == "project.save" {
        eval(
            app,
            "if (typeof saveStoryboard === 'function' && (projectView.textDrafts.size || state.data?.design?.dirty)) await saveStoryboard(); return true;",
        )?;
    }
    crate::feedback::guard(app, id, ui_project_request)?;
    let mut supplied = params.clone();
    if supplied.is_object() && supplied.get("_origin").is_none() {
        supplied["_origin"] = json!("CLI/MCP");
    }
    if supplied["_request_id"].is_null() {
        supplied["_request_id"] = json!(crate::feedback::new_request());
    }
    if id == "project.open" {
        supplied["claim_owner"] = json!(true);
    }
    let request_id = supplied["_request_id"].as_str().map(str::to_owned);
    supplied
        .as_object_mut()
        .map(|p| p.remove("_ui_dispatch_token"));
    let mut result = tauri::async_runtime::block_on(crate::execute_engine_command(
        app.clone(),
        id.to_owned(),
        supplied,
    ))?;
    if magicstudio_engine::find_command(id).is_some_and(|c| !c.journal) {
        return Ok(result);
    }
    let revision = crate::feedback::command_finished(app, id, &result, request_id.as_deref());
    if result.is_object() {
        result["_ui_engine_revision"] = json!(revision);
    }
    // This command came directly from the UI. Its awaiting handler renders once;
    // refreshing here would call Open while that same transition is still in flight.
    if ui_project_request {
        return Ok(result);
    }
    // The page shows the change (and follows a project opened or created from outside).
    let project = app
        .state::<Engine>()
        .with(None, |s| Ok(s.project_path.clone()))?;
    page(
        app,
        "refresh",
        &[
            &json!(id),
            &json!(project),
            &json!(revision),
            &result,
            &json!(request_id),
            params,
        ],
    )?;
    Ok(result)
}

pub(crate) fn handle(app: &AppHandle, id: &str, params: &Value) -> Result<Value, String> {
    let none = Value::Null;
    let ui_project_request = crate::project_dispatch::is_ui_request(app, id, params);
    check_pinned_project(app, id, params)?;
    check_pinned_instance(app, params)?;
    if let Some(reply) = dry_run_reply(id, params) {
        return Ok(reply);
    }
    if id == "project.open"
        && let Some(result) = crate::feedback::reuse_open(app, params)?
    {
        return Ok(result);
    }
    if matches!(
        id,
        "project.open" | "project.new" | "project.close" | "project.intake" | "app.quit"
    ) {
        eval(app, "await acFeedback.report(); return true;")?;
    }
    if let Some(result) = page_command(app, id, params, &none) {
        return result;
    }
    if let Some(result) = action_command(app, id, params, &none) {
        return result;
    }
    engine_command(app, id, params, ui_project_request)
}
