fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "lyrics-webview")]
    {
        tauri_build::build();
    }
    Ok(())
}
