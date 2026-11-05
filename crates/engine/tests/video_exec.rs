#[cfg(unix)]
#[test]
fn video_exec_reports_failed_save() {
    use serde_json::json;
    use std::os::unix::fs::PermissionsExt;

    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("magic-video-save-{unique}"));
    std::fs::create_dir_all(&dir).unwrap();
    let film = dir.join("test.fcproj");
    let mut editor = filmcraft_engine::Session::default();
    editor
        .execute("file.saveAs", json!({"path": film}))
        .unwrap();
    let original = std::fs::read(&film).unwrap();
    let mut session = magic::Session::default();
    session.project_path = Some(dir.join("test.magic"));
    session.video_project_path = Some(film.clone());
    std::fs::set_permissions(&film, std::fs::Permissions::from_mode(0o444)).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let result = magic::film::cmd_video_exec(
        &mut session,
        &json!({
            "id": "file.newBin", "params": {"name": "Unsaved"}
        }),
    );
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let after = std::fs::read(&film).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(after, original, "fixture must prevent saving");
    assert!(
        result.is_err(),
        "failed save was reported as success: {result:?}"
    );
}
