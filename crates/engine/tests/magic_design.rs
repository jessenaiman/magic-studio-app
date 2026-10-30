use magicstudio_engine::Session;
use magicstudio_engine::magic_design as design;
use magicstudio_engine::session::{Asset, LoadedCutoutRigPlan};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn profile_sheet_labels_supply_mesh_depth() {
    use magicstudio_engine::mesh_geometry::{ShapeSettings, build};
    let silhouette = |half_width: u32| {
        image::RgbaImage::from_fn(32, 32, |x, y| {
            image::Rgba([180, 120, 60, if x.abs_diff(16) <= half_width && (2..30).contains(&y) { 255 } else { 0 }])
        })
    };
    let settings = ShapeSettings { inflation_depth_pct: 58, relief_detail_pct: 0,
        mesh_res: 16, proj_blend_pct: 0, bump_intensity: 0 };
    let front = silhouette(12);
    let side = silhouette(4);
    let expected = build(&[("front".into(), front.clone()), ("side".into(), side.clone())], settings);
    for label in ["Right profile", "Left profile"] {
        let actual = build(&[("front".into(), front.clone()), (label.into(), side.clone())], settings);
        assert_eq!(actual.positions, expected.positions);
    }
    let fallback = build(&[("front".into(), front)], settings);
    assert_ne!(fallback.positions, expected.positions);
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "magic-design-tests-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn json(&self, name: &str, value: Value) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        path
    }

    fn session(&self) -> Session {
        Session {
            cwd: self.0.clone(),
            ..Session::default()
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn missing_manifest_preserves_existing_asset_and_selection() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    let asset = Asset {
        id: "refactor_probe".into(),
        width: 42,
        ..Asset::default()
    };
    session.assets.insert(asset.id.clone(), asset);
    session.active_asset = Some("refactor_probe".into());
    let before = json!(session.assets);
    let result = design::cmd_asset_load_manifest(
        &mut session,
        &json!({
            "manifest": fixture.0.join("missing.json")
        }),
    );
    assert!(result.is_err());
    assert_eq!(json!(session.assets), before);
    assert_eq!(session.active_asset.as_deref(), Some("refactor_probe"));
}

#[test]
fn explicit_manifests_keep_pose_image_and_existing_attachment_precedence() {
    let fixture = Fixture::new();
    let pose = fixture.json(
        "pose.json",
        json!({"sheetType":"action", "frames":[
            {"label":"idle", "file":"pose.png", "size":[20,30]}
        ]}),
    );
    let turn = fixture.json(
        "turn.json",
        json!({"sheetType":"turnaround", "frames":[
            {"label":"front", "file":"front.png", "size":[40,50]}
        ]}),
    );
    let mut session = fixture.session();
    let attached = fixture.0.join("existing-sheet.png");
    session.assets.insert(
        "refactor_probe".into(),
        Asset {
            id: "refactor_probe".into(),
            attached_asset_sheet: Some(attached.clone()),
            ..Asset::default()
        },
    );
    let result = design::cmd_asset_load_manifest(
        &mut session,
        &json!({
            "asset":"refactor_probe", "manifest":pose, "turnaround_manifest":turn,
            "motion_range":fixture.0.join("missing-motion.json")
        }),
    )
    .unwrap();
    let asset = session.current_asset().unwrap();
    assert_eq!(asset.path, fixture.0.join("pose.png"));
    assert_eq!((asset.width, asset.height), (20, 30));
    assert_eq!(asset.attached_asset_sheet.as_ref(), Some(&attached));
    assert_eq!(result["pose_labels"], json!(["idle"]));
    assert_eq!(result["turnaround_labels"], json!(["front"]));
    assert_eq!(result["has_measured_motion"], false);
    assert!(result["foot_lift_max_px"].is_null());
}

#[test]
fn primary_turnaround_routes_to_turnaround_slot() {
    let fixture = Fixture::new();
    let manifest = fixture.json(
        "turn.json",
        json!({"sheetType":"turnaround", "frames":[
            {"label":"side", "file":"side.png", "size":[12,24]}
        ]}),
    );
    let mut session = fixture.session();
    let result = design::cmd_asset_load_manifest(
        &mut session,
        &json!({
            "asset":"refactor_probe", "manifest":manifest
        }),
    )
    .unwrap();
    let asset = session.current_asset().unwrap();
    assert!(asset.pose_manifest.is_none());
    assert!(asset.turnaround_manifest.is_some());
    assert_eq!(result["pose_sheet_missing"], true);
    assert_eq!(result["turnaround_labels"], json!(["side"]));
}

#[test]
fn direct_rig_resolves_relative_paths_and_preserves_absolute_paths() {
    let fixture = Fixture::new();
    let image = image::RgbaImage::new(12, 24);
    image.save(fixture.0.join("source.png")).unwrap();
    image.save(fixture.0.join("sheet.png")).unwrap();
    let absolute_psd = fixture.0.join("original.psd");
    let plan = LoadedCutoutRigPlan {
        rig_type: "cutout_rig".into(),
        source_image: "source.png".into(),
        cutout_asset_sheet: Some("sheet.png".into()),
        photocraft_psd: Some(absolute_psd.clone()),
        ..LoadedCutoutRigPlan::default()
    };
    let path = fixture.json("rig.json", json!(plan));
    let mut session = fixture.session();
    design::cmd_asset_load_manifest(
        &mut session,
        &json!({
            "asset":"refactor_probe", "manifest":path
        }),
    )
    .unwrap();
    let asset = session.current_asset().unwrap();
    assert_eq!(asset.path, fixture.0.join("source.png"));
    assert_eq!((asset.width, asset.height), (12, 24));
    let plan = asset.cutout_rig.as_ref().unwrap();
    assert_eq!(plan.source_image, fixture.0.join("source.png"));
    assert_eq!(plan.cutout_asset_sheet, Some(fixture.0.join("sheet.png")));
    assert_eq!(plan.photocraft_psd, Some(absolute_psd));
}

#[test]
fn comparison_errors_keep_slot_aliases_and_do_not_mutate_inspection() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    session.assets.insert(
        "refactor_probe".into(),
        Asset {
            id: "refactor_probe".into(),
            path: fixture.0.join("missing.png"),
            original_inspection: Some(json!({"previous":true})),
            ..Asset::default()
        },
    );
    session.active_asset = Some("refactor_probe".into());
    for (slot, expected) in [
        ("original", "original reference/import files not found"),
        ("poses", "asset_sheet reference/import files not attached"),
        (
            "turnaround",
            "turnaround reference/import files not attached",
        ),
        ("all", "no valid reference/import image pairs"),
    ] {
        let error =
            design::cmd_intake_compare_reference(&mut session, &json!({"slot":slot})).unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
    assert_eq!(
        session.current_asset().unwrap().original_inspection,
        Some(json!({"previous":true}))
    );
}

#[test]
fn sprite_cleanup_preserves_body_and_removes_only_faint_pixels_and_floor_shadow() {
    let mut source = image::RgbaImage::new(5, 10);
    source.put_pixel(2, 1, image::Rgba([200, 100, 50, 255]));
    source.put_pixel(2, 7, image::Rgba([200, 100, 50, 255]));
    source.put_pixel(1, 9, image::Rgba([50, 50, 50, 100]));
    source.put_pixel(0, 0, image::Rgba([200, 100, 50, 27]));
    source.put_pixel(1, 2, image::Rgba([50, 50, 50, 100]));
    let (cleaned, bounds) = design::decontaminate_sprite_rgba(&source, None);
    assert_eq!(bounds, [1, 1, 2, 7]);
    assert_eq!(cleaned.get_pixel(1, 9).0, [0, 0, 0, 0]);
    assert_eq!(cleaned.get_pixel(0, 0).0, [0, 0, 0, 0]);
    assert_eq!(cleaned.get_pixel(1, 2).0, [50, 50, 50, 100]);
    assert_eq!(source.get_pixel(1, 9).0, [50, 50, 50, 100]);
    let (resized, bounds) = design::decontaminate_sprite_rgba(&source, Some(30));
    assert_eq!(resized.dimensions(), (15, 30));
    assert_eq!(bounds, [3, 3, 6, 21]);
}

#[test]
fn transparent_sprite_keeps_canvas_without_resizing() {
    let source = image::RgbaImage::new(4, 8);
    let (image, bounds) = design::decontaminate_sprite_rgba(&source, Some(40));
    assert_eq!(image.dimensions(), (4, 8));
    assert_eq!(bounds, [0, 0, 4, 8]);
}

#[test]
fn unreviewed_sheet_blocks_extraction_and_rig_build_before_outputs() {
    use magicstudio_engine::session::ProjectAsset;
    let fixture = Fixture::new();
    let source = fixture.0.join("sheet.png");
    image::RgbaImage::new(12, 24).save(&source).unwrap();
    let mut session = fixture.session();
    session.project_path = Some(fixture.json("project.magic", json!({})));
    session.assets.insert(
        "refactor_probe".into(),
        Asset {
            id: "refactor_probe".into(),
            path: source.clone(),
            ..Asset::default()
        },
    );
    session.active_asset = Some("refactor_probe".into());
    session.project_assets.push(ProjectAsset {
        id: "refactor_probe".into(),
        name: "Probe".into(),
        preset: None,
        path: Some(source),
        kind: "image".into(),
        parts: Vec::new(),
    });
    let before = json!(session.assets);
    let output = fixture.0.join("cut");
    let extraction =
        design::cmd_asset_cut_sheet(&mut session, &json!({"out_dir":output})).unwrap_err();
    let rig = design::cmd_asset_build_cutout_rig(&mut session, &json!({})).unwrap_err();
    assert!(
        extraction
            .to_string()
            .contains("Review the complete source sheet")
    );
    assert!(rig.to_string().contains("Review the complete source sheet"));
    assert!(!output.exists());
    assert_eq!(json!(session.assets), before);
}

#[test]
fn scene_render_requires_a_registered_source() {
    let mut session = Session::default();
    let missing =
        magicstudio_engine::exports::cmd_render_scene_source(&mut session, &json!({"id":"missing"}))
            .unwrap_err();
    assert!(missing.to_string().contains("registered Blender source"));
    session
        .animation_sources
        .insert("probe".into(), json!({"backend":"other"}));
    let wrong_backend =
        magicstudio_engine::exports::cmd_render_scene_source(&mut session, &json!({"id":"probe"}))
            .unwrap_err();
    assert!(wrong_backend.to_string().contains("not a Blender source"));
}

#[test]
fn scene_render_preserves_revision_and_option_guards() {
    let fixture = Fixture::new();
    let project = fixture.json("probe.magic", json!({}));
    let mut session = fixture.session();
    session.project_path = Some(project.clone());
    session.animation_sources.insert(
        "probe".into(),
        json!({"id":"probe","backend":"blender","revision":1,"source":"unused.blend"}),
    );
    let before = session.animation_sources.clone();
    let stale = magicstudio_engine::exports::cmd_render_scene_source(
        &mut session,
        &json!({
            "expected_project":project,"id":"probe","expected_revision":0
        }),
    )
    .unwrap_err();
    assert!(stale.to_string().contains("Source revision changed"));
    let invalid = magicstudio_engine::exports::cmd_render_scene_source(&mut session, &json!({
        "expected_project":project,"id":"probe","expected_revision":1,"options":{"unsupported":true}
    })).unwrap_err();
    assert!(invalid.to_string().contains("Unsupported render option"));
    assert_eq!(session.animation_sources, before);
}
