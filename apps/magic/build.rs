fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "lyrics-webview")]
    {
        println!("cargo:rerun-if-changed=../../references/magic_studio_kinetic_lyric_editor.tsx");
        println!("cargo:rerun-if-changed=../magic-cli/lyrics-entry.jsx");
        println!("cargo:rerun-if-changed=../magic-cli/lyrics.css");
        let status = std::process::Command::new("npm")
            .args(["run", "build:lyrics"])
            .current_dir("../magic-cli")
            .status()?;
        if !status.success() {
            return Err("Lyrics assets failed to build; see npm output above".into());
        }
        tauri_build::build();
    }
    Ok(())
}
