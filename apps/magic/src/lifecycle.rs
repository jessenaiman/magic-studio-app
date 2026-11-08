//! One non-destructive lifecycle gate for native windows, control and frontend IPC.
use magic::Session;
use serde_json::{Value, json};

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tauri::Emitter;
use tauri_plugin_dialog::DialogExt;

#[derive(Default)]
pub(super) struct Lifecycle {
    transition: AtomicBool,
    exit_approved: AtomicBool,
    native_request: AtomicBool,
}

impl Lifecycle {
    pub(super) fn exiting(&self) -> bool {
        self.exit_approved.load(Ordering::Acquire)
    }
    pub(super) fn transitioning(&self) -> bool {
        self.transition.load(Ordering::Acquire)
    }
}

fn check_engine(session: &Session) -> Result<Value, String> {
    if session.docs.iter().any(|doc| doc.dirty) {
        return Err("Save the edited project documents before closing, restarting or switching projects".into());
    }
    let saved = match &session.project_path {
        Some(path) => {
            let bytes = std::fs::read(path).map_err(|e| format!("Cannot verify saved project {}: {e}; save it before continuing", path.display()))?;
            let saved: Value = serde_json::from_slice(&bytes).map_err(|e| format!("Cannot verify saved project: {e}; save it before continuing"))?;
            if !saved.is_object() || saved["format"] != "magic.project" {
                return Err("Cannot verify saved project format; save the project before continuing".into());
            }
            if magic::project::project_is_dirty(session, &saved) {
                return Err("Save project changes before closing, restarting or switching projects".into());
            }
            saved
        }
        None => Value::Null,
    };
    Ok(
        json!({"project":session.project_path,"saved":saved,"documents":session.docs.iter().map(|doc| json!({"path":doc.path,"revision":doc.revision})).collect::<Vec<_>>()}),
    )
}

fn check_drafts(report: &Value) -> Result<(), String> {
    let (Some(dirty), Some(saving), Some(drafts)) = (report["dirty"].as_bool(), report["saving"].as_bool(), report["drafts"].as_array()) else {
        return Err("Cannot verify editor drafts; wait for the workspace to finish loading before continuing".into());
    };
    if saving {
        return Err("Wait for the editor save to finish before closing, restarting or switching projects".into());
    }
    if dirty || !drafts.is_empty() {
        return Err(format!(
            "Save the editor changes before closing, restarting or switching projects{}",
            if drafts.is_empty() { String::new() } else { format!(": {}", drafts.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")) }
        ));
    }
    Ok(())
}

pub(super) struct Transition {
    desktop: Arc<super::Desktop>,
    stamp: Value,
}

impl Drop for Transition {
    fn drop(&mut self) {
        self.desktop.lifecycle.transition.store(false, Ordering::Release);
    }
}

impl Transition {
    pub(super) fn verify(&self, session: &Session) -> Result<(), String> {
        if check_engine(session)? != self.stamp {
            return Err("Project state changed during the lifecycle check; retry the action".into());
        }
        Ok(())
    }
}

fn available_engine(session: &std::sync::Mutex<Session>) -> Result<Value, String> {
    let session = session.lock().map_err(|e| e.to_string())?;
    check_engine(&session)
}

pub(super) fn begin(app: &tauri::AppHandle) -> Result<Transition, String> {
    let desktop = super::state(app);
    desktop
        .lifecycle
        .transition
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "Another close or project switch is already being checked")?;
    let mut guard = Transition { desktop, stamp: Value::Null };
    guard.stamp = available_engine(&guard.desktop.session)?;
    if !guard.stamp["project"].is_null() {
        if !guard.desktop.ready.load(Ordering::Acquire) {
            return Err("Wait for the desktop to finish loading before continuing".into());
        }
        check_drafts(&super::ui_request(app, "ui.drafts", json!({}))?)?;
    }
    Ok(guard)
}

pub(super) fn exit(app: &tauri::AppHandle, restart: bool) -> Result<Value, String> {
    let guard = begin(app)?;
    {
        let session = guard.desktop.session.lock().map_err(|e| e.to_string())?;
        guard.verify(&session)?;
        guard.desktop.lifecycle.exit_approved.store(true, Ordering::Release);
    }
    let handle = app.clone();
    std::thread::spawn(move || {
        // Preserve the existing control reply flush before terminating the app.
        std::thread::sleep(std::time::Duration::from_millis(150));
        if restart {
            handle.request_restart();
        } else {
            handle.exit(0);
        }
    });
    Ok(json!({"ok":true,"quitting":!restart,"restarting":restart}))
}

pub(super) fn native_close(app: &tauri::AppHandle) {
    let desktop = super::state(app);
    if desktop.lifecycle.native_request.swap(true, Ordering::AcqRel) {
        return;
    }
    let handle = app.clone();
    std::thread::spawn(move || {
        if let Err(error) = exit(&handle, false) {
            let _ = handle.emit("magic-lifecycle-blocked", json!({"error":error}));
            handle.dialog().message(error).title("Unsaved work or active operation").kind(tauri_plugin_dialog::MessageDialogKind::Warning).show(|_| {});
        }
        desktop.lifecycle.native_request.store(false, Ordering::Release);
    });
}
