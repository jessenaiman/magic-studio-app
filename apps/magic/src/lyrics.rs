//! Temporary Tauri host for the Lyrics design, using the existing `magic --view lyrics` route.
//! The reference owns the sample interactions; no project or audio setup is required.

pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .setup(|app| {
            tauri::WebviewWindowBuilder::new(
                app,
                "lyrics",
                tauri::WebviewUrl::App("lyrics.html".into()),
            )
            .title("Magic Studio — Lyrics")
            .inner_size(1440.0, 960.0)
            .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
}
