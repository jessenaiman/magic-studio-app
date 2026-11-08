//! Keyboard shortcuts through craft-keymap. The command registry's shortcuts are the catalog.
use craft_keymap::{Active, Bindable, Binding, Chord, Keymap, Lookup, Platform, Scope, Scopes};
use serde_json::{Value, json};
use std::sync::OnceLock;

fn keymap() -> &'static Keymap {
    static KEYMAP: OnceLock<Keymap> = OnceLock::new();
    KEYMAP.get_or_init(|| {
        let catalog = magic::command_specs()
            .iter()
            .filter_map(|spec| {
                let binding = Binding::parse(spec.shortcut?).ok()?;
                Some(Bindable::new(spec.id, spec.label, Scope::app()).with_default(binding))
            })
            .collect();
        Keymap::new(catalog, Scopes::new(), Platform::current())
    })
}

fn run(app: &tauri::AppHandle, command: &str) -> Result<Value, String> {
    match command {
        // These two need input, which the app asks for the way its buttons do.
        "project.new" => super::dispatch(app, "ui.set_view", json!({"view": "home"}), true),
        "project.open" => {
            let title = "Open Magic Studio Project";
            let dialog = craft_dialogs::FileDialog::open().title(title).filter("Magic Studio Project", &["magic"]);
            match super::dialogs::ask(app, craft_dialogs::Request::File(dialog))? {
                craft_dialogs::Answer::Paths(paths) => match paths.first() {
                    Some(path) => super::dispatch(app, command, json!({"path": path}), true),
                    None => Ok(Value::Null),
                },
                craft_dialogs::Answer::Unavailable(why) => Err(why),
                _ => Ok(Value::Null),
            }
        }
        _ => super::dispatch(app, command, json!({}), true),
    }
}

/// Runs what `chord` is bound to. `typing`: a text field has the keyboard.
fn press(app: &tauri::AppHandle, chord: &str, typing: bool) -> Result<Value, String> {
    let chord = Chord::parse(chord).map_err(|e| format!("invalid chord: {e:?}"))?;
    match keymap().lookup(&chord, false, &Active { typing, ..Active::NONE }) {
        Lookup::Run(command) => {
            let result = run(app, &command.id)?;
            Ok(json!({"handled": true, "command": command.id, "result": result}))
        }
        _ => Ok(json!({"handled": false})),
    }
}

/// The key handler for the UI.
#[tauri::command]
pub async fn key_pressed(app: tauri::AppHandle, chord: String, typing: bool) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || press(&app, &chord, typing)).await.map_err(|e| e.to_string())?
}

/// `ui.keymap.press {chord, typing?}`: the same path a key press takes.
pub(super) fn control(app: &tauri::AppHandle, params: &Value) -> Result<Value, String> {
    super::only_keys(params, "ui.keymap.press", &["chord", "typing"])?;
    let chord = params["chord"].as_str().ok_or("ui.keymap.press needs a chord")?;
    press(app, chord, params["typing"] == true)
}
