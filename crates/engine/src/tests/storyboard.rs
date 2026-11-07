use super::*;

fn temp_board_path(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("magic-storyboard-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("storyboard.designcraft")
}

#[test]
fn storyboard_new_pages_set_add_and_thumbnails_roundtrip() {
    let path = temp_board_path("roundtrip");
    let path_str = path.to_str().unwrap();
    let mut s = Session::default();

    // 1. Create storyboard from embedded template
    let created = cmd_storyboard_new(&mut s, &json!({"path": path_str})).unwrap();
    assert_eq!(created["ok"], true);
    assert!(path.exists());

    // 2. Inspect pages & archetypes
    let pages = cmd_storyboard_pages(&mut s, &json!({"path": path_str})).unwrap();
    let page_list = pages["pages"].as_array().unwrap();
    assert_eq!(page_list.len(), 4);
    assert_eq!(page_list[0]["archetype"], "cover");
    assert_eq!(page_list[3]["archetype"], "scene");
    let cover = page_list[0]["slots"].as_array().unwrap();
    let key_art = cover.iter().find(|slot| slot["label"] == "cover_key_art").unwrap();
    assert_eq!(key_art["kind"], "image");
    assert!(key_art["image"].is_null());

    // 3. Edit Scene 1 text fields
    let updated = cmd_storyboard_set(
        &mut s,
        &json!({
            "path": path_str,
            "scene": 1,
            "fields": {"title": "Opening Sunrise", "cue": "Morning light"}
        }),
    )
    .unwrap();
    assert_eq!(updated["page"], 3);
    assert_eq!(updated["scene"], 1);

    // 4. Add Scene 2 from Scene 1 (using 1-based `scene` index)
    let added = cmd_storyboard_add(&mut s, &json!({"path": path_str, "scene": 1})).unwrap();
    assert_eq!(added["page"], 4);
    assert_eq!(added["scene"], 2);
    assert_eq!(added["archetype"], "scene");

    // Verify Scene 2 received incremented SCENE 02 and leaf numbers 09/10
    let slots = cmd_storyboard_slots(&mut s, &json!({"path": path_str, "scene": 2})).unwrap();
    let slot_list = slots["slots"].as_array().unwrap();
    let scene_id = slot_list.iter().find(|sl| sl["label"] == "scene_id").and_then(|sl| sl["text"].as_str()).unwrap();
    assert_eq!(scene_id, "SCENE 02");

    // 5. Batch render scene thumbnails in one pass
    let thumb_dir = path.parent().unwrap().join("cache/storyboard");
    let thumbs = cmd_storyboard_thumbnail(&mut s, &json!({"path": path_str, "out_dir": thumb_dir.to_str().unwrap(), "scale": 0.1})).unwrap();
    assert_eq!(thumbs["count"], 2);
    assert!(thumb_dir.join("thumb-1.png").exists());
    assert!(thumb_dir.join("thumb-2.png").exists());

    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
