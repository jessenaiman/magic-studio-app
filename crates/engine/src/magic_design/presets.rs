use std::path::{Path, PathBuf};

/// Return the exports root, preferring the workspace root when `exports/pose_and_parts` exists,
/// else falling back to the build-manifest repository root.
pub(super) fn exports_root(root: &Path) -> PathBuf {
    let local = root.join("exports/pose_and_parts");
    if local.is_dir() {
        return root.to_path_buf();
    }
    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| root.to_path_buf());
    if manifest_root.join("exports/pose_and_parts").is_dir() {
        return manifest_root;
    }
    root.to_path_buf()
}

/// Vault hero image path for a character slug (hardcoded user library paths).
pub(super) fn vault_hero_image(slug: &str) -> Option<PathBuf> {
    let vault = Path::new("/home/jesse/main-vault/Creative/Design/Characters");
    let rel = match slug {
        "hopper" => "Hopper-no-background.png_202607180130.jpeg",
        "whiskers" => "Whiskers-no-back.png_202607180130.jpeg",
        "jenny" => "Penny.jpeg",
        "puddles" => "Puddles-tranparent.png_202607180130.jpeg",
        "scout" => "Scout-no-back.png_202607180130.jpeg",
        "rusty" => "Rusty-transparent.png_202607180130.jpeg",
        "sam" => "Sam.png_202607180130.jpeg",
        "maisy" => "Slide8.PNG_202607180130.jpeg",
        "mr_puddles" => "Mr Puddles.png",
        "old_mac" => "old_mac-tranparent.png_202607180130.jpeg",
        "miss_hayley" => "Miss Hayley.png",
        _ => return None,
    };
    let p = vault.join(rel);
    p.is_file().then_some(p)
}

pub(super) struct ManifestPaths {
    pub id: String,
    pub pose: Option<PathBuf>,
    pub turnaround: Option<PathBuf>,
    pub motion: Option<PathBuf>,
    pub registration: Option<PathBuf>,
}

struct Preset {
    aliases: &'static [&'static str],
    id: &'static str,
    pose_dirs: &'static [&'static str],
    turnaround_dir: &'static str,
}

const PRESETS: &[Preset] = &[
    Preset {
        aliases: &["hopper"],
        id: "hopper",
        pose_dirs: &["hopper_poses"],
        turnaround_dir: "hopper_turnaround",
    },
    Preset {
        aliases: &["whiskers"],
        id: "whiskers",
        pose_dirs: &[
            "whiskers_poses",
            "Create_pose_sheet_for_puppet_202607180130_20",
        ],
        turnaround_dir: "Character_turnaround_sheet_anima…_202607180130_17",
    },
    Preset {
        aliases: &["puddles"],
        id: "puddles",
        pose_dirs: &["Create_pose_sheet_for_puppet_202607180130_13"],
        turnaround_dir: "Character_turnaround_sheet_anima…_202607180130_12",
    },
    Preset {
        aliases: &["jenny", "sun"],
        id: "jenny",
        pose_dirs: &["Create_pose_sheet_for_puppet_202607180130_3"],
        turnaround_dir: "Character_turnaround_sheet_anima…_202607180130_9",
    },
    Preset {
        aliases: &["rusty", "coralfish", "coral_fish"],
        id: "rusty",
        pose_dirs: &["Create_pose_sheet_for_puppet_202607180130_24"],
        turnaround_dir: "Character_turnaround_sheet_anima…_202607180130_19",
    },
    Preset {
        aliases: &["miss_hayley", "misshayley", "hayley"],
        id: "miss_hayley",
        pose_dirs: &["miss_hayley_poses"],
        turnaround_dir: "miss_hayley_turnaround",
    },
    Preset {
        aliases: &["old_mac", "oldmac"],
        id: "old_mac",
        pose_dirs: &["Create_pose_sheet_for_puppet_202607180130_25"],
        turnaround_dir: "old_mac_turnaround",
    },
    Preset {
        aliases: &["scout"],
        id: "scout",
        pose_dirs: &["Create_pose_sheet_for_puppet_202607180130_5"],
        turnaround_dir: "scout_turnaround",
    },
    Preset {
        aliases: &["mr_puddles", "mrpuddles"],
        id: "mr_puddles",
        pose_dirs: &[],
        turnaround_dir: "mr_puddles_turnaround",
    },
    Preset {
        aliases: &["sam"],
        id: "sam",
        pose_dirs: &["Create_pose_sheet_for_puppet_202607180130_33"],
        turnaround_dir: "Character_turnaround_sheet_anima…_202607180130_21",
    },
    Preset {
        aliases: &["maisy"],
        id: "maisy",
        pose_dirs: &[],
        turnaround_dir: "Character_turnaround_sheet_anima…_202607180130_29",
    },
];

pub(super) fn preset_paths(root: &Path, preset: &str) -> ManifestPaths {
    let root = exports_root(root);
    let base = root.join("exports/pose_and_parts");
    let name = preset.to_ascii_lowercase();
    let preset = PRESETS.iter().find(|p| p.aliases.contains(&name.as_str()));
    let id = preset
        .map(|p| p.id.to_owned())
        .unwrap_or_else(|| crate::workspace::sanitize_asset_id(&name));
    let pose = match preset {
        Some(p) => p
            .pose_dirs
            .iter()
            .find_map(|dir| existing_file(base.join(dir).join("manifest.json"))),
        None => existing_file(base.join(&id).join("manifest.json")),
    };
    let turnaround = preset.and_then(|p| {
        existing_file(base.join(&id).join("registered_turnaround/manifest.json"))
            .or_else(|| existing_file(base.join(p.turnaround_dir).join("manifest.json")))
    });
    ManifestPaths {
        motion: Some(telemetry_path(&root, &id, "motion_range.json")),
        registration: Some(telemetry_path(&root, &id, "turnaround_registration.json")),
        id,
        pose,
        turnaround,
    }
}

fn existing_file(path: PathBuf) -> Option<PathBuf> {
    path.is_file().then_some(path)
}

fn telemetry_path(root: &Path, id: &str, filename: &str) -> PathBuf {
    let primary = root.join("exports/pose_and_parts").join(id).join(filename);
    if primary.is_file() {
        return primary;
    }
    existing_file(
        root.join("exports/design_share")
            .join(id)
            .join("accuracy")
            .join(filename),
    )
    .unwrap_or(primary)
}
