//! File and message dialogs through craft-dialogs, visible to and answerable over `--control`.
//!
//! `Dialogs` stays on the main thread. Other threads queue through `run_on_main_thread` and wait
//! on a channel, so [`ask`] must not be called on the main thread. There is no frame loop here:
//! craft-dialogs' wake-up runs [`pump`] whenever a request is queued or an answer comes in.
//!
//! A command given a path (control channel, CLI) never asks. `ui.dialog.override` answers the
//! pickers the UI itself opens, through `set_override`, for automation.
use craft_dialogs::{Answer, Dialogs, FileDialog, Kind, Message, MessageButtons, MessageLevel, MessageResult, Override, Request};
use serde::Deserialize;
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use tauri::Manager;

struct Native {
    dialogs: Dialogs<mpsc::Sender<Answer>>,
    /// What was asked and not yet answered, oldest (the one on screen) first.
    pending: VecDeque<Value>,
    last: Value,
    overridden: bool,
}

thread_local! {
    static NATIVE: RefCell<Option<Native>> = const { RefCell::new(None) };
    /// A wake-up that arrived while [`pump`] held the dialogs.
    static AGAIN: Cell<bool> = const { Cell::new(false) };
}

impl Native {
    fn new(app: &tauri::AppHandle) -> Self {
        let handle = app.clone();
        let dialogs = Dialogs::new(Arc::new(move || {
            let target = handle.clone();
            // Fails only once the event loop has ended, when no answer has anyone to go to.
            let _ = handle.run_on_main_thread(move || pump(&target));
        }));
        Self { dialogs, pending: VecDeque::new(), last: Value::Null, overridden: false }
    }
}

/// Shows what is queued and delivers what was answered. Main thread only. Tauri can run a wake-up
/// inline when it is sent from the main thread, so a nested call only asks for another pass.
fn pump(app: &tauri::AppHandle) {
    let window = super::main_window(app).ok();
    NATIVE.with(|cell| {
        let Ok(mut slot) = cell.try_borrow_mut() else {
            AGAIN.set(true);
            return;
        };
        let Some(native) = slot.as_mut() else { return };
        loop {
            AGAIN.set(false);
            native.dialogs.show_pending(window.as_ref());
            for (reply, answer) in native.dialogs.take_answers() {
                let request = native.pending.pop_front().unwrap_or(Value::Null);
                native.last = json!({"request": request, "answer": describe_answer(&answer)});
                // The asker gone meanwhile has no use for the answer.
                let _ = reply.send(answer);
            }
            if !AGAIN.get() {
                break;
            }
        }
    });
}

/// Runs `work` on the main thread with the dialogs and waits for its result.
fn on_main<R: Send + 'static>(app: &tauri::AppHandle, work: impl FnOnce(&mut Native) -> R + Send + 'static) -> Result<R, String> {
    let (tx, rx) = mpsc::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let result = NATIVE.with_borrow_mut(|slot| work(slot.get_or_insert_with(|| Native::new(&handle))));
        pump(&handle);
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;
    rx.recv().map_err(|_| "the event loop ended before the dialog request ran".to_string())
}

/// Queues `request` and waits for its answer, however long the user takes.
pub(super) fn ask(app: &tauri::AppHandle, request: Request) -> Result<Answer, String> {
    let (tx, rx) = mpsc::channel();
    on_main(app, move |native| {
        native.pending.push_back(describe_request(&request));
        match request {
            Request::File(dialog) => native.dialogs.ask(dialog, tx),
            Request::Message(message) => native.dialogs.ask_message(message, tx),
        }
    })?;
    rx.recv().map_err(|_| "the dialog stopped without an answer".to_string())
}

/// `Unavailable` means no dialog could be shown: an error, recorded, never a Cancel.
fn record_unavailable(app: &tauri::AppHandle, title: &str, why: &str) {
    let project = super::state(app).session.try_lock().ok().and_then(|s| s.project_path.clone());
    magic::activity::append(json!({
        "stage": "handler_error",
        "command": "ui.dialog",
        "project": project,
        "origin": "desktop",
        "control": title,
        "error": why.chars().take(350).collect::<String>(),
        "pid": std::process::id(),
        "severity": "error"
    }));
}

/// A warning with an OK button. Blocks until it is closed.
pub(super) fn warn(app: &tauri::AppHandle, title: &str, description: String) {
    let message = Message { level: MessageLevel::Warning, title: title.to_owned(), description, buttons: MessageButtons::Ok };
    match ask(app, Request::Message(message)) {
        Ok(Answer::Unavailable(why)) | Err(why) => record_unavailable(app, title, &why),
        Ok(_) => {}
    }
}

#[derive(Deserialize)]
pub struct PickFile {
    title: String,
    extensions: Option<Vec<String>>,
    #[serde(default)]
    directory: bool,
    #[serde(default)]
    multiple: bool,
}

fn file_dialog(options: &PickFile) -> FileDialog {
    let mut dialog = match (options.directory, options.multiple) {
        (true, _) => FileDialog::folder(),
        (false, true) => FileDialog::open_many(),
        (false, false) => FileDialog::open(),
    }
    .title(&options.title);
    if let Some(extensions) = &options.extensions {
        let extensions: Vec<&str> = extensions.iter().map(String::as_str).collect();
        dialog = dialog.filter(&options.title, &extensions);
    }
    dialog
}

/// The webview previews a chosen file through the asset protocol before any command runs.
fn grant_preview(app: &tauri::AppHandle, paths: &[PathBuf], directory: bool) -> Result<(), String> {
    let scope = app.asset_protocol_scope();
    for path in paths {
        if directory { scope.allow_directory(path, true) } else { scope.allow_file(path) }.map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// The UI's picker: null when cancelled, a path, or an array of paths for `multiple`.
#[tauri::command]
pub async fn pick_file(app: tauri::AppHandle, options: PickFile) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || match ask(&app, Request::File(file_dialog(&options))) {
        Ok(Answer::Paths(paths)) => {
            grant_preview(&app, &paths, options.directory)?;
            let mut names = paths.iter().map(|path| path.to_string_lossy().into_owned());
            Ok(if options.multiple { json!(names.collect::<Vec<_>>()) } else { names.next().map_or(Value::Null, Value::String) })
        }
        Ok(Answer::Cancelled) => Ok(Value::Null),
        Ok(Answer::Message(_)) => Err("a file dialog was answered with a message result".to_string()),
        Ok(Answer::Unavailable(why)) | Err(why) => {
            record_unavailable(&app, &options.title, &why);
            Err(format!("The file dialog couldn't be shown: {why}"))
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

fn describe_request(request: &Request) -> Value {
    match request {
        Request::File(dialog) => json!({
            "kind": match dialog.kind {
                Kind::Open { multiple: false } => "open",
                Kind::Open { multiple: true } => "openMany",
                Kind::Folder => "folder",
                Kind::Save => "save",
            },
            "title": dialog.title,
            "filters": dialog.filters.iter().map(|f| json!({"name": f.name, "extensions": f.extensions})).collect::<Vec<_>>(),
            "directory": dialog.directory,
            "fileName": dialog.file_name,
        }),
        Request::Message(message) => json!({
            "kind": "message",
            "level": format!("{:?}", message.level).to_lowercase(),
            "title": message.title,
            "description": message.description,
            "buttons": format!("{:?}", message.buttons),
        }),
    }
}

fn describe_answer(answer: &Answer) -> Value {
    match answer {
        Answer::Cancelled => json!({"cancelled": true}),
        Answer::Paths(paths) => json!({"paths": paths}),
        Answer::Message(result) => json!({"button": match result {
            MessageResult::Ok => "ok",
            MessageResult::Cancel => "cancel",
            MessageResult::Yes => "yes",
            MessageResult::No => "no",
            MessageResult::Custom(label) => label,
        }}),
        Answer::Unavailable(why) => json!({"unavailable": why}),
    }
}

/// What `ui.dialog.override` asked for; the `Override` itself is built on the main thread.
enum Scripted {
    Native,
    Answers { paths: Option<Vec<PathBuf>>, button: Option<MessageResult> },
    Cancelled,
    Unavailable(String),
}

const OVERRIDE_FORMS: &str = "ui.dialog.override takes exactly one of: paths and/or button, cancelled: true, unavailable: reason, clear: true";

fn scripted(params: &Value) -> Result<Scripted, String> {
    super::only_keys(params, "ui.dialog.override", &["paths", "button", "cancelled", "unavailable", "clear"])?;
    let paths = params.get("paths");
    let button = params.get("button");
    let forms = ["cancelled", "unavailable", "clear"].iter().filter(|key| params.get(**key).is_some()).count();
    if forms + usize::from(paths.is_some() || button.is_some()) != 1 {
        return Err(OVERRIDE_FORMS.into());
    }
    if let Some(flag) = params.get("cancelled").or_else(|| params.get("clear")) {
        if flag != true {
            return Err(OVERRIDE_FORMS.into());
        }
        return Ok(if params.get("clear").is_some() { Scripted::Native } else { Scripted::Cancelled });
    }
    if let Some(why) = params.get("unavailable") {
        return why.as_str().map(|why| Scripted::Unavailable(why.to_owned())).ok_or_else(|| "unavailable must be a string".to_string());
    }
    let paths = paths
        .map(|paths| {
            let list = paths.as_array().filter(|list| !list.is_empty()).ok_or("paths must be a non-empty array of absolute paths")?;
            list.iter()
                .map(|path| path.as_str().map(PathBuf::from).filter(|path| path.is_absolute()).ok_or("paths must be a non-empty array of absolute paths"))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    let button = button
        .map(|button| match button.as_str() {
            Some("ok") => Ok(MessageResult::Ok),
            Some("cancel") => Ok(MessageResult::Cancel),
            Some("yes") => Ok(MessageResult::Yes),
            Some("no") => Ok(MessageResult::No),
            Some(label) => Ok(MessageResult::Custom(label.to_owned())),
            None => Err("button must be a string"),
        })
        .transpose()?;
    Ok(Scripted::Answers { paths, button })
}

fn override_for(scripted: Scripted) -> Option<Override> {
    match scripted {
        Scripted::Native => None,
        Scripted::Cancelled => Some(Box::new(|request| match request {
            Request::File(_) => Answer::Cancelled,
            Request::Message(_) => Answer::Message(MessageResult::Cancel),
        })),
        Scripted::Unavailable(why) => Some(Box::new(move |_| Answer::Unavailable(why.clone()))),
        Scripted::Answers { paths, button } => Some(Box::new(move |request| match request {
            Request::File(_) => paths.clone().map_or_else(|| Answer::Unavailable("the override has no paths for a file dialog".into()), Answer::Paths),
            Request::Message(_) => button.clone().map_or_else(|| Answer::Unavailable("the override has no button for a message box".into()), Answer::Message),
        })),
    }
}

/// `ui.dialog.state` and `ui.dialog.override`.
pub(super) fn control(app: &tauri::AppHandle, method: &str, params: &Value) -> Result<Value, String> {
    if method == "ui.dialog.state" {
        super::only_keys(params, method, &[])?;
        return on_main(app, |native| {
            json!({
                "open": native.dialogs.is_open(),
                "queued": native.dialogs.queued(),
                "override": native.overridden,
                "pending": native.pending,
                "last": native.last,
            })
        });
    }
    let scripted = scripted(params)?;
    on_main(app, move |native| {
        let answers = override_for(scripted);
        native.overridden = answers.is_some();
        native.dialogs.set_override(answers);
        json!({"ok": true, "override": native.overridden})
    })
}
