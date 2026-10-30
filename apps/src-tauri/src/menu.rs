//! Shared native menus. Stable action IDs are independent of their display names.
use serde_json::{Value, json};
use tauri::{
    Manager,
    menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder},
};

const FILE_LABEL: &str = "File";
// Labels and accelerators have one owner for the app bar and all views.
const FILE_ACTIONS: &[(&str, &str, &str, bool)] = &[
    ("file.new", "New Project…", "CmdOrCtrl+N", false),
    ("file.open", "Open Project…", "CmdOrCtrl+O", false),
    ("file.save", "Save Project", "CmdOrCtrl+S", true),
    (
        "file.save_as",
        "Save Project As…",
        "CmdOrCtrl+Shift+S",
        true,
    ),
    ("file.close", "Close Project", "CmdOrCtrl+W", true),
    ("app.settings", "Settings…", "CmdOrCtrl+,", false),
    ("app.quit", "Quit Magic Studio", "CmdOrCtrl+Q", false),
];

// WebKit handles edits directly on Wayland; Muda's predefined GTK edits use X11.
const TEXT_ACTIONS: &[(&str, &str, &str)] = &[
    ("edit.undo", "Undo", "Undo"),
    ("edit.redo", "Redo", "Redo"),
    ("edit.cut", "Cut", "Cut"),
    ("edit.copy", "Copy", "Copy"),
    ("edit.paste", "Paste", "Paste"),
    ("edit.select_all", "Select All", "SelectAll"),
];

pub fn labels() -> Value {
    json!({
        "file":{"label":FILE_LABEL,"items":FILE_ACTIONS.iter().map(|(id,label,key,project)|
            json!({"id":id,"label":label,"shortcut":key,"project":project,
                "separator":matches!(*id,"app.settings"|"app.quit")})).collect::<Vec<_>>()},
        "text":{"label":"Text editing","supported":cfg!(target_os = "linux"),"items":TEXT_ACTIONS.iter().map(|(id,label,_)|
            json!({"id":id,"label":label,"separator":*id=="edit.cut"})).collect::<Vec<_>>()}
    })
}

pub fn install(window: &tauri::Window) -> tauri::Result<()> {
    let mut file = SubmenuBuilder::new(window, FILE_LABEL);
    for &(id, label, accelerator, _) in FILE_ACTIONS {
        if matches!(id, "app.settings" | "app.quit") {
            file = file.separator();
        }
        let item = MenuItemBuilder::with_id(id, label)
            .accelerator(accelerator)
            .build(window)?;
        file = file.item(&item);
    }
    let file = file.build()?;
    window.set_menu(MenuBuilder::new(window).item(&file).build()?)?;
    // Keep native accelerators without a second menu strip above the app bar.
    #[cfg(not(target_os = "macos"))]
    window.hide_menu()?;
    window.on_menu_event(|window, event| {
        if event.id().as_ref().starts_with("edit.") {
            if let Err(error) = edit_text(window.app_handle(), event.id().as_ref()) {
                eprintln!("text menu: {error}");
            }
            return;
        }
        if event.id().as_ref() == "app.quit" {
            // close() goes through the existing CloseRequested guard.
            if let Err(error) = window.close() {
                eprintln!("menu quit: {error}");
            }
            return;
        }
        if let Some(view) = window.get_webview("project") {
            let id = json!(event.id().as_ref());
            if let Err(error) = view.eval(format!("window.studioMenu({id});")) {
                eprintln!("native menu: {error}");
            }
        }
    });
    Ok(())
}

/// The same focused-text actions are available to the DOM menu and CLI/MCP.
#[tauri::command]
pub fn menu_action(app: tauri::AppHandle, action: String) -> Result<Value, String> {
    if action == "app.quit" {
        app.get_window("main")
            .ok_or("Studio window unavailable")?
            .close()
            .map_err(|e| e.to_string())?;
        return Ok(json!({"ok":true,"status":"close_requested"}));
    }
    edit_text(&app, &action)
}

/// Execute the focused text field's own undo/clipboard operation, never project undo.
pub fn edit_text(app: &tauri::AppHandle, action: &str) -> Result<Value, String> {
    let command = TEXT_ACTIONS
        .iter()
        .find(|(id, _, _)| *id == action)
        .map(|(_, _, command)| *command)
        .ok_or("Unknown text editing action")?;
    #[cfg(target_os = "linux")]
    {
        use webkit2gtk::WebViewExt;
        app.get_webview("project")
            .ok_or("Project view unavailable")?
            .with_webview(move |platform| platform.inner().execute_editing_command(command))
            .map_err(|e| e.to_string())?;
        Ok(json!({"ok":true,"action":action,"status":"dispatched to focused text field"}))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = command;
        Err("Use the native text context menu on this platform".into())
    }
}
