use super::*;

#[test]
fn walk_frames_prompt_names_every_key_pose_and_the_canvas() {
    let all = templates().unwrap();
    let prompt = compose(&all, &all["kinds"]["actions"], &json!({"action": "walk"})).unwrap();
    assert!(prompt.starts_with("Draw ONE image on a 1536 x 1024 px canvas."));
    assert!(prompt.contains("contact (right foot forward), down, passing, up"));
    assert!(prompt.contains("384 x 512 px"));
}

#[test]
fn new_character_keeps_the_legs_apart_rule() {
    let all = templates().unwrap();
    let prompt = compose(
        &all,
        &all["kinds"]["new"],
        &json!({"prompt": "a felt bunny"}),
    )
    .unwrap();
    assert!(prompt.contains("clear gap of background between the legs"));
    assert!(prompt.contains("Character: a felt bunny"));
}

#[test]
fn fidget_prepare_emits_yaml_frontmatter_note_and_save_command() {
    let prepared = PreparedDrawing::prepare(&json!({"prompt": "a clockwork owl"}), "new").unwrap();
    let md = prepared.fidget["markdown"].as_str().unwrap();
    assert!(md.starts_with("---\nrole: fidget\ntemplate: new\n"));
    assert!(md.contains("save_command: project.add_asset"));
    assert!(md.contains(&format!("> {FIDGET_NOTE}")));
    assert_eq!(parse_frontmatter_template(md).as_deref(), Some("new"));
}

#[test]
fn craft_ai_roles_declare_fidget_and_scribbles() {
    let roles = magic_ai_roles().unwrap();
    assert_eq!(roles.len(), 3);
    assert_eq!(roles[0].id.as_str(), "fidget");
    assert_eq!(roles[0].capability.as_str(), "text.complete");
    assert_eq!(roles[1].id.as_str(), "scribbles");
    assert_eq!(roles[1].capability.as_str(), "image.generate");
    let status = craft_ai_status();
    assert_eq!(status["ok"], json!(true));
}
