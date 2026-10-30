//! Presentation acknowledgement and initialization health; never a claim of visual quality.
use crate::Engine;
use serde_json::{Value, json};
use std::sync::Mutex;
use tauri::Manager;
#[derive(Default)]
pub struct Feedback(pub Mutex<Value>);
#[tauri::command]
pub fn ui_report(app: tauri::AppHandle, report: Value) -> Result<(), String> {
    if let Some(event) = report.get("event").filter(|v| v.is_object()) {
        let mut event = event.clone();
        event["project"] = report["project"].clone();
        event["origin"] = json!("UI interaction");
        event["client_label"] = json!("Studio user (unverified label)");
        magicstudio_engine::activity::append(event);
    }
    let state = app.state::<Feedback>();
    let mut v = state.0.lock().map_err(|e| e.to_string())?;
    let revision = v["ui_revision"].as_u64().unwrap_or(0) + 1;
    let engine_revision = v["engine_revision"].clone();
    let mut last = v["last_command"].clone();
    let history = v["command_revisions"].clone();
    let session_project = v["session_project"].clone();
    let requested = report["ack_engine_revision"].as_u64().unwrap_or(0);
    let expected = history
        .as_array()
        .and_then(|items| items.iter().find(|item| item["revision"] == requested));
    let valid = report["phase"] == "ready"
        && report["rendered"] == true
        && report["errors"].as_array().is_some_and(|a| a.is_empty())
        && expected.is_some_and(|e| e["project"] == report["project"]);
    let previous = v["ack_engine_revision"].as_u64().unwrap_or(0);
    let acknowledged = if valid {
        requested.max(previous)
    } else {
        previous
    };
    if last["revision"] == acknowledged {
        last["ui_ack"] = json!("DOM_applied; visual review separate");
    }
    if valid && requested > previous {
        magicstudio_engine::activity::append(
            json!({"stage":"frontend_ack","project":report["project"],"command":expected.map(|e|e["id"].clone()),"engine_revision":requested,"request_id":expected.map(|e|e["request_id"].clone()),"origin":"UI render report","ui_ack":"DOM applied; visual review separate"}),
        );
    }
    *v = report;
    v["ui_revision"] = json!(revision);
    v["engine_revision"] = engine_revision;
    v["last_command"] = last;
    v["command_revisions"] = history;
    v["session_project"] = session_project;
    v["ack_engine_revision"] = json!(acknowledged);
    v["updated_unix_ms"] = json!(now());
    Ok(())
}
fn now() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
pub fn command_finished(
    app: &tauri::AppHandle,
    id: &str,
    result: &Value,
    request_id: Option<&str>,
) -> u64 {
    let project = app
        .state::<Engine>()
        .with(None, |s| Ok(s.project_path.clone()))
        .ok()
        .flatten();
    let state = app.state::<Feedback>();
    let Ok(mut v) = state.0.lock() else {
        return 0;
    };
    let revision = v["engine_revision"].as_u64().unwrap_or(0) + 1;
    v["session_project"] = json!(project);
    v["engine_revision"] = json!(revision);
    v["last_command"] = json!({"request_id":request_id,"revision":revision,"id":id,"ok":result["ok"]!=false,"engine_finished_unix_ms":now(),"ui_ack":"pending"});
    let mut history = v["command_revisions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    history.push(json!({"request_id":request_id,"revision":revision,"id":id,"project":project}));
    if history.len() > 16 {
        history.remove(0);
    }
    v["command_revisions"] = json!(history);
    revision
}
/// What the engine lock could tell us; every field is None while the engine is busy.
struct EngineView {
    project: Option<std::path::PathBuf>,
    dirty: Option<bool>,
    chat_selection: Option<Value>,
    edits: Option<u64>,
    writer_owner: Option<bool>,
}

fn engine_view(app: &tauri::AppHandle, last_project: Option<std::path::PathBuf>) -> EngineView {
    let engine = app.state::<Engine>();
    let Ok(s) = engine.0.try_lock() else {
        return EngineView {
            project: last_project,
            dirty: None,
            chat_selection: None,
            edits: None,
            writer_owner: None,
        };
    };
    let dirty = if let Some(path) = &s.project_path {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .map(|doc| magicstudio_engine::project::project_is_dirty(&s, &doc))
    } else {
        Some(false)
    };
    EngineView {
        project: s.project_path.clone(),
        dirty,
        chat_selection: s.agent_selection.clone(),
        edits: Some(s.edits_since_save),
        writer_owner: Some(s.project_owner.is_some()),
    }
}

fn dirty_reasons(v: &Value, dirty: Option<bool>) -> Vec<Value> {
    let mut reasons = Vec::new();
    if dirty == Some(true) {
        reasons.push(json!("engine state differs from saved project"));
    }
    if dirty.is_none() {
        reasons.push(json!(
            "engine state unavailable; busy or unreadable saved project"
        ));
    }
    for field in v["draft_fields"].as_array().into_iter().flatten() {
        reasons.push(json!({"draft_field":field}));
    }
    if v["pending_invokes"].as_u64().unwrap_or(0) > 0 {
        reasons.push(json!({"pending_invokes":v["pending_invokes"]}));
    }
    reasons
}

/// Engine edits since the last project write, plus each changed form field. A clean diff
/// means 0 even after commands (e.g. exports) that never touch project state.
fn unsaved_edits(v: &Value, dirty: Option<bool>, edits: Option<u64>) -> Value {
    let drafts = v["draft_fields"].as_array().map_or(0, Vec::len) as u64;
    match (dirty, edits) {
        (Some(false), _) => json!(drafts),
        (Some(true), Some(n)) => json!(n.max(1) + drafts),
        _ => Value::Null,
    }
}

fn unsaved_state(v: &Value, dirty: Option<bool>) -> &'static str {
    if dirty == Some(true)
        || v["draft_fields"].as_array().is_some_and(|a| !a.is_empty())
        || v["pending_invokes"].as_u64().unwrap_or(0) > 0
    {
        return "unsaved_or_pending";
    }
    if dirty == Some(false) && v["phase"] == "ready" {
        return "no_detected_unsaved_changes";
    }
    "unknown"
}

fn acknowledgement(v: &Value, ack: u64, revision: u64) -> &'static str {
    if v["phase"] == "error" {
        return "frontend_error";
    }
    if ack >= revision && v["phase"] == "ready" {
        return "DOM_applied; visual review separate";
    }
    "pending"
}

fn ui_ready(v: &Value, ack: u64, revision: u64) -> bool {
    v["phase"] == "ready"
        && v["rendered"] == true
        && v["errors"].as_array().is_some_and(|e| e.is_empty())
        && v["project"] == v["active_project"]
        && ack >= revision
}

pub fn snapshot(app: &tauri::AppHandle) -> Result<Value, String> {
    let mut v = app
        .state::<Feedback>()
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .clone();
    if !v.is_object() {
        v = json!({});
    }
    let last_project = v["session_project"].as_str().map(std::path::PathBuf::from);
    let engine = engine_view(app, last_project);
    v["project_guard_version"] = json!(1);
    v["ok"] = json!(true);
    v["connection"] = json!("connected");
    v["identity"] = crate::studio_identity();
    #[cfg(unix)]
    if let Some(socket) = app.try_state::<crate::local_ipc::LocalSocket>() {
        v["instance"] = json!(socket.0.file_name().and_then(|p| p.to_str()));
        v["endpoint"] = json!(format!("unix:{}", socket.0.display()));
    }

    v["writer_owner"] = json!(engine.writer_owner);
    v["active_project"] = json!(engine.project);
    v["project_id"] = json!(engine.project);
    v["project_identity_kind"] = json!("canonical project path");
    v["observed_unix_ms"] = json!(now());
    v["frontend_report_age_ms"] = v["updated_unix_ms"]
        .as_u64()
        .map(|updated| json!(now().saturating_sub(updated as u128)))
        .unwrap_or(Value::Null);
    v["chat_selection"] = match engine.chat_selection {
        Some(selected) => {
            json!({"agent":selected["agent"],"provider":selected["provider"],"model":selected["model"],"source":"active engine selection","runtime_model_verified":false})
        }
        None => {
            json!({"status":"unknown","reason":"active engine selection unavailable","runtime_model_verified":false})
        }
    };
    v["dirty_reasons"] = json!(dirty_reasons(&v, engine.dirty));
    v["unsaved_edits"] = unsaved_edits(&v, engine.dirty, engine.edits);
    v["engine_unsaved"] = json!(engine.dirty);
    v["unsaved_state"] = json!(unsaved_state(&v, engine.dirty));
    let ack = v["ack_engine_revision"].as_u64().unwrap_or(0);
    let rev = v["engine_revision"].as_u64().unwrap_or(0);
    v["ui_acknowledgement"] = json!(acknowledgement(&v, ack, rev));
    v["ui_ready"] = json!(ui_ready(&v, ack, rev));
    Ok(v)
}

/// One boundary for replacing/closing the live workspace. Unknown state never permits silent loss.
pub fn guard(app: &tauri::AppHandle, id: &str, ui_origin: bool) -> Result<(), String> {
    if !matches!(
        id,
        "project.open" | "project.new" | "project.close" | "project.intake" | "app.quit"
    ) {
        return Ok(());
    }
    let v = snapshot(app)?;
    if v["active_project"].is_null()
        && id != "app.quit"
        && v["pending_invokes"].as_u64().unwrap_or(0) <= u64::from(ui_origin)
    {
        return Ok(());
    }
    let fields = v["draft_fields"].as_array().is_some_and(|a| !a.is_empty());
    let pending = v["pending_invokes"].as_u64().unwrap_or(u64::MAX) > u64::from(ui_origin);
    if v["engine_unsaved"] != false
        || fields
        || pending
        || v["phase"] != "ready"
        || v["errors"].as_array().is_none_or(|a| !a.is_empty())
    {
        return Err(format!(
            "{id} blocked: unsaved, pending or unknown Studio state. Review edits in Studio and save them; query `magic-cli exec ui.feedback` before retrying. No project was replaced or app closed."
        ));
    }
    Ok(())
}

pub fn new_request() -> String {
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!(
        "studio-{}-{}-{}",
        std::process::id(),
        now(),
        SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}
/// This window already owns the project: focus it and record why.
fn focus_same_session(app: &tauri::AppHandle, target: &std::path::Path, params: &Value) -> Result<Option<Value>, String> {
    let active = app
        .state::<Engine>()
        .with(None, |s| Ok(s.project_path.clone()))?;
    let reported = app
        .state::<Feedback>()
        .0
        .lock()
        .map_err(|e| e.to_string())?["project"]
        .as_str()
        .map(str::to_owned);
    if active.as_deref() != Some(target) || reported.as_deref() != target.to_str() {
        return Ok(None);
    }
    app.get_window("main")
        .ok_or("Studio window unavailable")?
        .set_focus()
        .map_err(|e| e.to_string())?;
    magicstudio_engine::activity::append(
        json!({"stage":"session_focused","command":"project.open","project":target,"request_id":params["_request_id"],"origin":params["_origin"]}),
    );
    Ok(Some(
        json!({"ok":true,"project":target,"reused_session":true,"focused":true}),
    ))
}

/// What asking the other live window achieved.
enum LiveFocus {
    /// Nobody else is serving this project.
    NoSession,
    /// This very process has it open; the frontend must apply the document itself.
    SameProcess,
    /// Another window was asked to focus.
    Focused(Value),
}

/// Another live window owns the project: ask it to focus, unless it is this process.
fn focus_live_session(target: &std::path::Path) -> Result<LiveFocus, String> {
    let Some(live) = magicstudio_engine::studio_sessions::for_project(target)? else {
        return Ok(LiveFocus::NoSession);
    };
    // This engine has opened the file but its frontend has not applied it yet. Returning
    // "focused existing session" here used to skip the actual document/name render.
    if live["identity"]["pid"].as_u64() == Some(std::process::id() as u64) {
        return Ok(LiveFocus::SameProcess);
    }
    #[cfg(unix)]
    {
        let endpoint = live["endpoint"]
            .as_str()
            .ok_or("Existing Studio endpoint unavailable")?
            .trim_start_matches("unix:");
        magicstudio_engine::studio_sessions::request(
            std::path::Path::new(endpoint),
            "ui.focus",
            json!({"_expected_project":target,"_expected_instance":live["instance"]}),
        )?;
        Ok(LiveFocus::Focused(
            json!({"ok":true,"project":target,"reused_session":true,"focused":true,"instance":live["instance"]}),
        ))
    }
    #[cfg(not(unix))]
    {
        let _ = live;
        Ok(LiveFocus::NoSession)
    }
}

/// A workspace is already open elsewhere: start a new instance for this project.
fn open_new_instance(target: &std::path::Path, active: bool) -> Result<Option<Value>, String> {
    if !active {
        return Ok(None);
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut opened = magicstudio_engine::studio_sessions::open(&exe, target.to_str())?;
    opened["reused_session"] = json!(true);
    opened["opened_new_instance"] = json!(true);
    Ok(Some(opened))
}

/// Reopening an already owned project focuses its existing window; it never reloads unsaved work.
pub fn reuse_open(app: &tauri::AppHandle, params: &Value) -> Result<Option<Value>, String> {
    let Some(raw) = params["path"].as_str() else {
        return Ok(None);
    };
    let target = magicstudio_engine::studio_sessions::canonical(std::path::Path::new(raw))?;
    if let Some(focused) = focus_same_session(app, &target, params)? {
        return Ok(Some(focused));
    }
    match focus_live_session(&target)? {
        LiveFocus::Focused(value) => return Ok(Some(value)),
        LiveFocus::SameProcess => return Ok(None),
        LiveFocus::NoSession => {}
    }
    let active = app
        .state::<Engine>()
        .with(None, |s| Ok(s.project_path.clone()))?
        .is_some();
    open_new_instance(&target, active)
}
