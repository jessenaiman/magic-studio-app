//! Global shortcut registration using tauri-plugin-global-shortcut.
//! Registers craft-keymap bindings as system-wide hotkeys.

use craft_keymap::{Bindable, Binding, Keymap, Platform, Scope, Scopes};
use serde_json::{Value, json};
use std::sync::OnceLock;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

static GLOBAL_SHORTCUT_MANAGER: OnceLock<GlobalShortcutManager> = OnceLock::new();

struct GlobalShortcutManager {
    app: AppHandle,
    keymap: Keymap,
}

impl GlobalShortcutManager {
    fn new(app: AppHandle) -> Self {
        let catalog = magic::command_specs()
            .iter()
            .filter_map(|spec| {
                let binding = Binding::parse(spec.shortcut?).ok()?;
                Some(Bindable::new(spec.id, spec.label, Scope::app()).with_default(binding))
            })
            .collect();
        let keymap = Keymap::new(catalog, Scopes::new(), Platform::current());
        Self { app, keymap }
    }

    /// Register all keymap bindings as global shortcuts.
    fn register_all(&self) -> Result<(), String> {
        let global_shortcut = self.app.global_shortcut();
        for bindable in self.keymap.catalog() {
            for binding in &bindable.defaults {
                let chord_str = binding.to_string();
                // Skip chords that are not suitable for global registration (e.g., simple letter keys)
                if self.is_suitable_for_global(&chord_str) {
                    let command_id = bindable.command.to_string();
                    let app_handle = self.app.clone();
                    global_shortcut
                        .on_shortcut(chord_str.as_str(), move |_app, _shortcut, _event| {
                            drop(tauri::async_runtime::spawn_blocking({
                                let app = app_handle.clone();
                                let cmd = command_id.clone();
                                move || {
                                    let _ = crate::keymap::press(&app, &cmd, false);
                                }
                            }));
                        })
                        .map_err(|e| format!("Failed to register global shortcut {}: {}", chord_str, e))?;
                    log::info!("Registered global shortcut: {} -> {}", chord_str, bindable.command);
                }
            }
        }
        Ok(())
    }

    /// Check if a chord is suitable for global registration.
    /// We skip single-character chords and chords that might conflict with system shortcuts.
    fn is_suitable_for_global(&self, chord: &str) -> bool {
        // Must have at least one modifier (Cmd/Ctrl/Alt/Shift)
        let has_modifier = chord.contains("Cmd")
            || chord.contains("Ctrl")
            || chord.contains("Alt")
            || chord.contains("Shift")
            || chord.contains("Super")
            || chord.contains("Meta");
        // Must not be just a single key
        let parts: Vec<&str> = chord.split('+').collect();
        has_modifier && parts.len() >= 2
    }

    /// Unregister all global shortcuts.
    fn unregister_all(&self) -> Result<(), String> {
        self.app.global_shortcut().unregister_all().map_err(|e| e.to_string())
    }
}

/// Initialize global shortcuts. Call once during app setup.
pub fn init(app: AppHandle) -> Result<(), String> {
    let manager = GlobalShortcutManager::new(app.clone());
    manager.register_all()?;
    let _ = GLOBAL_SHORTCUT_MANAGER.set(manager);
    Ok(())
}

/// Re-register global shortcuts (e.g., after keymap changes).
pub fn reregister(app: AppHandle) -> Result<(), String> {
    if let Some(manager) = GLOBAL_SHORTCUT_MANAGER.get() {
        manager.unregister_all()?;
    }
    let manager = GlobalShortcutManager::new(app.clone());
    manager.register_all()?;
    let _ = GLOBAL_SHORTCUT_MANAGER.set(manager);
    Ok(())
}

/// Tauri command: re-register all global shortcuts from the current keymap.
#[tauri::command]
pub async fn reregister_cmd(app: tauri::AppHandle) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || reregister(app)).await.map_err(|e| e.to_string())?.map(|_| json!({"ok": true}))
}

/// Tauri command: list currently registered global shortcuts.
#[tauri::command]
pub async fn list_registered_cmd(_app: tauri::AppHandle) -> Result<Value, String> {
    // Note: is_registered requires a specific shortcut to check.
    // We can't list all registered shortcuts with the current API.
    Ok(json!({"note": "Use is_registered with a specific shortcut to check"}))
}
