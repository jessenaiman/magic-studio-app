use super::*;

#[test]
fn scribbles_rejects_untemplated_raw_prompt() {
    let mut s = Session::default();
    let err = cmd_scribbles_generate(&mut s, &json!({"prompt": "draw a cat"})).unwrap_err();
    assert!(err.to_string().contains("Scribbles requires a template"));
}

#[test]
fn scribbles_resolves_template_from_kind_or_frontmatter() {
    let (req, meta) = resolve_template_request(&json!({"kind": "new", "prompt": "a felt fox"})).unwrap();
    assert_eq!(meta["kind"], json!("new"));
    assert_eq!(meta["save_command"], json!("project.add_asset"));
    assert!(req["markdown"].as_str().unwrap().contains(crate::fidget::FIDGET_NOTE));

    let md = crate::fidget::format_fidget_markdown(
        "storyboard_hero",
        "storyboard.set",
        &["storyboard".into()],
        Some([1536, 1024]),
        "Wide hero shot of the fox on a hill",
    );
    let (_, meta2) = resolve_template_request(&json!({"prompt": md})).unwrap();
    assert_eq!(meta2["kind"], json!("storyboard_hero"));
    assert_eq!(meta2["save_command"], json!("storyboard.set"));
    assert_eq!(meta2["save_slot"], json!("hero_image"));
}
