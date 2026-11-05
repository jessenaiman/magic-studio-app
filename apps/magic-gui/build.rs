fn main() {
    #[cfg(feature = "lyrics-webview")]
    tauri_build::build();
}
