use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use magic::{EngineError, Session};
use serde_json::{Value, json};

struct SongFixture(PathBuf);

impl SongFixture {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/fixtures")
            .join(format!("lyrics-project-{}-{unique}", std::process::id()));
        fs::create_dir_all(root.join("liner-notes")).unwrap();
        fs::write(
            root.join("liner-notes/lyrics.json"),
            include_bytes!("../fixtures/fictional-song/fictional-song.lyrics.json"),
        )
        .unwrap();
        fs::write(
            root.join("fixture.transcript.json"),
            include_bytes!("../fixtures/fictional-song/fictional-song.transcript.json"),
        )
        .unwrap();
        Self(root)
    }
}

impl Drop for SongFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn lyrics_reopen_from_disk_and_stale_edits_preserve_the_newer_save() {
    let fixture = SongFixture::new();
    let project = fixture.0.join("song.magic");
    let path = fixture.0.join("design/words.json");
    let mut editor = Session::default();
    editor
        .execute("project.new", json!({"path": project}))
        .unwrap();
    let draft = editor.execute("words.read", json!({})).unwrap();
    let animation = &draft["document"]["animations"][0];
    let first_edit = json!({
        "animation_id": animation["animation_id"],
        "expected_source_revision": animation["source_revision"],
        "style": {"size": 120}
    });
    editor.execute("words.save", first_edit).unwrap();
    let disk: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(disk["animations"][0]["style"]["size"], 120);
    drop(editor);

    let mut stale = Session::default();
    stale
        .execute("project.open", json!({"path": project}))
        .unwrap();
    let reopened = stale.execute("words.read", json!({})).unwrap();
    let animation = &reopened["document"]["animations"][0];
    assert_eq!(animation["style"]["size"], 120);

    let mut edit = json!({
        "animation_id": animation["animation_id"],
        "expected_source_revision": animation["source_revision"],
        "style": {"size": 145}
    });
    let mut writer = Session::default();
    writer
        .execute("project.open", json!({"path": project}))
        .unwrap();
    writer.execute("words.save", edit.clone()).unwrap();
    let newer_bytes = fs::read(&path).unwrap();
    let newer: Value = serde_json::from_slice(&newer_bytes).unwrap();
    assert_eq!(newer["animations"][0]["style"]["size"], 145);

    edit["style"]["size"] = json!(170);
    let rejected = stale.execute("words.save", edit).unwrap();
    assert_eq!(rejected["conflict"], true);
    assert_eq!(fs::read(path).unwrap(), newer_bytes);
}

#[test]
fn public_commands_reject_invalid_parameters_and_missing_prerequisites() {
    let mut session = Session::default();
    let invalid = session.execute(
        "words.save",
        json!({"expected_source_revision": "a string"}),
    );
    assert!(
        matches!(invalid, Err(EngineError::InvalidParams(message)) if message.contains("must be a number"))
    );

    let missing_window = session.execute("ui.state", json!({}));
    assert!(
        matches!(missing_window, Err(EngineError::InvalidParams(message)) if message.contains("requires a live Magic Studio window"))
    );
}
