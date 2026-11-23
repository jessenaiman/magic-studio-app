//! System notification helper using tauri-plugin-notification.
//! Wires product-flow completions (video assemble/export, image generate, scribbles operations) to desktop notifications.

use std::sync::OnceLock;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

static NOTIFIER: OnceLock<Notifier> = OnceLock::new();

struct Notifier {
    app: AppHandle,
}

impl Notifier {
    fn new(app: AppHandle) -> Self {
        Self { app }
    }

    fn show(&self, title: &str, body: &str) {
        #[cfg(target_os = "macos")]
        {
            let _ = self.app.notification().builder().title(title).body(body).show();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = self.app.notification().builder().title(title).body(body).show();
        }
        #[cfg(target_os = "windows")]
        {
            let _ = self.app.notification().builder().title(title).body(body).show();
        }
    }
}

/// Initialize the notifier with the app handle. Call once during setup.
pub fn init(app: AppHandle) {
    let _ = NOTIFIER.set(Notifier::new(app));
}

/// Send a notification for video assembly completion.
pub fn notify_video_assemble(export_path: Option<&str>, success: bool) {
    if let Some(notifier) = NOTIFIER.get() {
        if success {
            let body = if let Some(path) = export_path { format!("Exported to {}", path) } else { "Film assembled (no export)".into() };
            notifier.show("Video Assembled", &body);
        } else {
            notifier.show("Video Assembly Failed", "Check the console for details");
        }
    }
}

/// Send a notification for image generation completion.
pub fn notify_image_generated(asset_name: &str, provider: &str, success: bool) {
    if let Some(notifier) = NOTIFIER.get() {
        if success {
            notifier.show("Image Generated", &format!("{} via {}", asset_name, provider));
        } else {
            notifier.show("Image Generation Failed", &format!("Provider: {}", provider));
        }
    }
}

/// Send a notification for Scribbles operation completion.
pub fn notify_scribbles(kind: &str, character: Option<&str>, asset_count: usize, success: bool) {
    if let Some(notifier) = NOTIFIER.get() {
        let title = if success { "Scribbles Complete" } else { "Scribbles Failed" };
        let body = if let Some(char) = character {
            format!("{} — {} ({} asset{})", kind, char, asset_count, if asset_count == 1 { "" } else { "s" })
        } else {
            format!("{} ({} asset{})", kind, asset_count, if asset_count == 1 { "" } else { "s" })
        };
        notifier.show(title, &body);
    }
}

/// Send a notification for video export completion.
pub fn notify_video_export(path: &str, success: bool) {
    if let Some(notifier) = NOTIFIER.get() {
        if success {
            notifier.show("Video Exported", path);
        } else {
            notifier.show("Video Export Failed", path);
        }
    }
}
