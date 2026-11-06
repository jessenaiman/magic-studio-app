use super::*;

fn ready() -> Value {
    json!({"phase":"ready","rendered":true,"errors":[],"project":"a","active_project":"a"})
}

#[test]
fn clean_saved_ready_state_reports_no_unsaved_changes() {
    let v = json!({"phase":"ready","draft_fields":[],"pending_invokes":0});
    assert_eq!(
        unsaved_state(&v, Some(false)),
        "no_detected_unsaved_changes"
    );
    assert!(dirty_reasons(&v, Some(false)).is_empty());
    assert_eq!(unsaved_edits(&v, Some(false), Some(0)), json!(0));
}

#[test]
fn dirty_engine_state_is_unsaved_and_counts_edits() {
    let v = json!({"phase":"ready","draft_fields":["title"],"pending_invokes":0});
    assert_eq!(unsaved_state(&v, Some(true)), "unsaved_or_pending");
    assert_eq!(unsaved_edits(&v, Some(true), Some(2)), json!(3));
    assert_eq!(dirty_reasons(&v, Some(true)).len(), 2);
}

#[test]
fn unreadable_saved_state_is_unknown_not_clean() {
    let v = json!({"phase":"ready","draft_fields":[],"pending_invokes":0});
    assert_eq!(unsaved_state(&v, None), "unknown");
    assert_eq!(unsaved_edits(&v, None, None), Value::Null);
    assert_eq!(dirty_reasons(&v, None).len(), 1);
}

#[test]
fn pending_invokes_block_a_clean_verdict() {
    let v = json!({"phase":"ready","draft_fields":[],"pending_invokes":1});
    assert_eq!(unsaved_state(&v, Some(false)), "unsaved_or_pending");
}

#[test]
fn acknowledgement_tracks_revision_and_errors() {
    assert_eq!(
        acknowledgement(&ready(), 3, 3),
        "DOM_applied; visual review separate"
    );
    assert_eq!(acknowledgement(&ready(), 2, 3), "pending");
    assert_eq!(
        acknowledgement(&json!({"phase":"error"}), 9, 3),
        "frontend_error"
    );
}

#[test]
fn ui_ready_requires_render_no_errors_and_matching_project() {
    assert!(ui_ready(&ready(), 4, 4));
    assert!(!ui_ready(&ready(), 3, 4));
    let mut errored = ready();
    errored["errors"] = json!(["boom"]);
    assert!(!ui_ready(&errored, 4, 4));
    let mut other = ready();
    other["active_project"] = json!("b");
    assert!(!ui_ready(&other, 4, 4));
}
