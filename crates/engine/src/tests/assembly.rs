use super::*;

#[test]
fn each_background_runs_until_the_next_and_the_last_to_the_end() {
    let got = spans(&[0.0, 7.74, 14.91], 33.0);
    let want = [(0.0, 7.74), (7.74, 7.17), (14.91, 18.09)];
    assert_eq!(got.len(), want.len());
    for ((a, b), (c, d)) in got.into_iter().zip(want) {
        assert!(
            (a - c).abs() < 1e-9 && (b - d).abs() < 1e-9,
            "{a},{b} vs {c},{d}"
        );
    }
}

#[test]
fn storyboard_backgrounds_resolve_scene_start_times_and_collapse_adjacent_duplicates() {
    let dir = std::env::temp_dir().join(format!("magic_assembly_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let img1 = dir.join("map.png");
    let img2 = dir.join("kitchen.png");
    std::fs::write(&img1, b"png1").unwrap();
    std::fs::write(&img2, b"png2").unwrap();
    let assets = vec![
        crate::ProjectAsset {
            id: "map".into(),
            name: "Map".into(),
            kind: "image".into(),
            path: Some("map.png".into()),
            preset: None,
            extra: Default::default(),
        },
        crate::ProjectAsset {
            id: "kitchen".into(),
            name: "Kitchen".into(),
            kind: "image".into(),
            path: Some("kitchen.png".into()),
            preset: None,
            extra: Default::default(),
        },
    ];
    let board = json!({
        "scenes": [
            {"start_seconds": 0.0, "image_asset_id": "map"},
            {"start_seconds": 4.2, "image_asset_id": "map"},
            {"start_seconds": 8.5, "image_asset_id": "kitchen"},
            {"start_seconds": 14.0, "image_path": "map.png"}
        ]
    });
    let bgs = backgrounds_from_storyboard(&board, &dir, &assets);
    let words_doc = json!({
        "animations": [
            {"song_anchor": 0.12, "background_asset": "map"},
            {"words": [{"start": 9.4}], "background_asset": "kitchen"}
        ]
    });
    let anim_bgs = backgrounds_from_storyboard(&words_doc, &dir, &assets);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        bgs,
        vec![
            (0.0, img1.clone()),
            (8.5, img2.clone()),
            (14.0, img1.clone()),
        ]
    );
    assert_eq!(anim_bgs, vec![(0.12, img1), (9.4, img2)]);
}
