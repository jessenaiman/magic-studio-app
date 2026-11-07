//! Craft persistence, `FileStamp` external-change guard, and undo-history safety tests.
//!
//! References:
//! - <https://github.com/storytold/craft-libs/blob/e653048a187036d30fd912f35f14b32ab4084e5c/crates/atomic/src/lib.rs>
//! - <https://github.com/storytold/vectorcraft/blob/a479c8baa1b783b5002f8ebd02391aa3395b5b49/crates/engine/src/doc_state.rs>
//! - <https://github.com/storytold/vectorcraft/blob/a479c8baa1b783b5002f8ebd02391aa3395b5b49/crates/engine/src/link_watch.rs>
//! - <https://github.com/storytold/photocraft/blob/a3ea9a4277e1e1067acbb30d592c56e9545b949d/crates/ui-egui/src/app/files.rs#L694-L706>

use serde_json::json;

use crate::Session;
use crate::doc_state::DocState;
use crate::history::Step;

#[test]
fn project_save_flushes_dirty_doc_states_atomically_and_guards_external_edits() {
    let dir = std::env::temp_dir().join(format!("magic-craft-save-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let proj_path = dir.join("song.magic");

    let mut s = Session::default();
    let created = s.execute("project.new", json!({"path": proj_path, "mode": "animation"})).unwrap();
    assert_eq!(created["ok"], true);
    assert!(s.project_stamp.get().is_some(), "atomic project write records FileStamp");

    // Stage a dirty DocState (mirrors VectorCraft DocState) and verify `project.save` flushes it.
    let words_path = dir.join("design/words.json");
    let mut words_doc = DocState::new("words", &words_path, json!({"version": 1}));
    words_doc.set(json!({"version": 1, "animations": []}));
    s.docs.push(words_doc);

    let saved = s.execute("project.save", json!({})).unwrap();
    assert_eq!(saved["ok"], true);
    assert_eq!(saved["documents"].as_array().unwrap().len(), 1);
    assert_eq!(saved["documents"][0]["kind"], "words");
    assert!(!s.docs[0].dirty);
    assert!(words_path.is_file());

    // Simulate an external process modifying `song.magic` on disk; saving must refuse to clobber it.
    std::fs::write(&proj_path, br#"{"format":"magic.project","version":1,"name":"external-edit","assets":[]}"#).unwrap();
    let err = s.execute("project.save", json!({})).unwrap_err();
    assert!(err.to_string().contains("changed outside this session"), "unexpected error: {err}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn failed_undo_preserves_history_step_until_swap_in_succeeds() {
    let dir = std::env::temp_dir().join(format!("magic-craft-undo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let doc_path = dir.join("words.json");

    let mut s = Session::default();
    let mut doc = DocState::new("words", &doc_path, json!({"step": 1}));
    doc.save().unwrap();
    let before = doc.clone();
    doc.set(json!({"step": 2}));
    doc.save().unwrap();
    s.docs.push(doc);
    s.history.push(Step { label: "words.save".into(), docs: vec![before] });

    // External modification causes `swap_in -> DocState::save` to refuse; undo step must stay on stack.
    std::fs::write(&doc_path, br#"{"step": 999, "external": true}"#).unwrap();
    assert!(s.undo().is_err());
    assert_eq!(s.history.undo.len(), 1, "failed undo must not pop the undo step");

    let _ = std::fs::remove_dir_all(&dir);
}
