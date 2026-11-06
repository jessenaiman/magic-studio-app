use super::launcher_app;

/// The launcher's desktop entry names the installed build; a stale link elsewhere is ignored.
#[test]
fn desktop_apps_resolve_through_the_launcher_entry() {
    let home = std::env::temp_dir().join(format!("magic-launcher-test-{}", std::process::id()));
    let apps = home.join(".local/share/applications");
    std::fs::create_dir_all(&apps).unwrap();
    let build = home.join("photocraft-9.9.9.AppImage");
    std::fs::write(&build, b"").unwrap();
    std::fs::write(
        apps.join("ai.storyteller.photocraft.desktop"),
        format!("[Desktop Entry]\nExec=\"{}\" %F\n", build.display()),
    )
    .unwrap();
    let found = launcher_app(&home, "photocraft");
    let missing = launcher_app(&home, "vectorcraft");
    std::fs::remove_dir_all(&home).unwrap();
    assert_eq!(found.as_deref(), Some(build.to_str().unwrap()));
    assert_eq!(missing, None);
}
