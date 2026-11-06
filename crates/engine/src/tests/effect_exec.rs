use crate::Session;
use serde_json::json;

fn effect_test_dir(label: &str) -> std::path::PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("magic-effect-{label}-{unique}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `save: false` must not implicitly overwrite an existing EffectCraft document.
#[test]
fn effect_exec_without_save_preserves_existing_project() {
    let dir = effect_test_dir("no-save");
    let path = dir.join("test.ecproj");
    let mut session = Session::default();
    session
        .execute(
            "effect.exec",
            json!({"path": path, "id": "comp.new", "params": {"name": "Original", "width": 64, "height": 64, "duration": 1}}),
        )
        .unwrap();
    let original = std::fs::read(&path).unwrap();
    session
        .execute(
            "effect.exec",
            json!({"path": path, "id": "comp.settings", "params": {"name": "Changed"}, "save": false}),
        )
        .unwrap();
    let after = std::fs::read(&path).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(after == original, "save=false rewrote the existing project");
}

#[test]
fn effect_exec_without_save_does_not_create_a_new_project() {
    let dir = effect_test_dir("new-no-save");
    let path = dir.join("test.ecproj");
    Session::default()
        .execute(
            "effect.exec",
            json!({"path": path, "id": "comp.new", "params": {"name": "Draft", "width": 64, "height": 64, "duration": 1}, "save": false}),
        )
        .unwrap();
    let exists = path.exists();
    std::fs::remove_dir_all(dir).unwrap();
    assert!(!exists, "save=false created a project");
}

#[test]
fn effect_exec_saves_existing_project_by_default() {
    let dir = effect_test_dir("default-save");
    let path = dir.join("test.ecproj");
    let mut session = Session::default();
    session
        .execute(
            "effect.exec",
            json!({"path": path, "id": "comp.new", "params": {"name": "Original", "width": 64, "height": 64, "duration": 1}}),
        )
        .unwrap();
    session
        .execute(
            "effect.exec",
            json!({"path": path, "id": "comp.settings", "params": {"name": "Saved"}}),
        )
        .unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(doc["items"]["1"]["name"], "Saved");
}

#[test]
fn effect_exec_without_automatic_save_allows_explicit_save_step() {
    let dir = effect_test_dir("explicit-save");
    let path = dir.join("test.ecproj");
    Session::default()
        .execute(
            "effect.exec",
            json!({"path": path, "save": false, "steps": [
                {"id": "comp.new", "params": {"name": "Explicit", "width": 64, "height": 64, "duration": 1}},
                {"id": "file.saveAs", "params": {"path": path}}
            ]}),
        )
        .unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(doc["items"]["1"]["name"], "Explicit");
}
