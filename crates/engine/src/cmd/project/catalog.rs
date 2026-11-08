//! The existing Home navigation history, shared by UI, terminal and native app. Not a media scan.
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
pub fn file() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|p| p.join("magic/recent_projects.json"))
}
pub fn list() -> Vec<String> {
    file().and_then(|p| fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}
fn canonical(path: &str) -> String {
    fs::canonicalize(path).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| path.to_string())
}

/// Never persist test/fixture or temp paths as the user's recent project.
fn is_transient(path: &str) -> bool {
    path.contains("/.agent/") || path.contains("/tmp/") || path.contains("/target/") || Path::new(path).starts_with(std::env::temp_dir())
}

fn update(path: &str, forget: bool) -> Result<(), String> {
    let path = canonical(path);
    if is_transient(&path) && !forget {
        return Ok(());
    }
    let target = file().ok_or("Config directory unavailable")?;
    fs::create_dir_all(target.parent().ok_or("Config directory unavailable")?).map_err(|e| e.to_string())?;
    let _lock = crate::project_lock::lock(&target, || {});
    let mut items: Vec<String> = fs::read(&target).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    items.retain(|p| canonical(p) != path);
    if !forget {
        items.insert(0, path);
        items.truncate(10);
    }
    let bytes = serde_json::to_vec(&items).map_err(|e| e.to_string())?;
    let result = craft_atomic::atomic_write(&target, &bytes).map_err(|e| e.to_string());
    crate::change_marker::invalidate();
    result
}
pub fn record(path: &str) -> Result<(), String> {
    update(path, false)
}
pub fn forget(path: &str) -> Result<(), String> {
    update(path, true)
}
fn is_image_file(path: &Path) -> bool {
    path.is_file() && crate::workspace::has_extension(path, &["png", "webp", "jpg", "jpeg"])
}

fn sorted_dir_entries(dir: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).collect();
    entries.sort();
    entries
}

fn first_image_in_tree(dir: &Path, max_depth: usize) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    let entries = sorted_dir_entries(dir);
    for entry in &entries {
        if is_image_file(entry) {
            return Some(entry.clone());
        }
    }
    if max_depth == 0 {
        return None;
    }
    for entry in entries {
        if entry.is_dir()
            && let Some(found) = first_image_in_tree(&entry, max_depth - 1)
        {
            return Some(found);
        }
    }
    None
}

fn find_character_preview(project_dir: &Path, char_dir: &Path, raw_name: &str, canonical_id: &str) -> Option<PathBuf> {
    let direct_candidates = [
        project_dir.join("assets/characters/approved-crops").join(format!("{raw_name}_from-lineup-v2.png")),
        project_dir.join("assets/characters/approved-crops").join(format!("{canonical_id}_from-lineup-v2.png")),
        char_dir.join("front").join(format!("{canonical_id}-front-drawing.png")),
        char_dir.join("front").join(format!("{raw_name}-front-drawing.png")),
        char_dir.join("front/frames").join(format!("{canonical_id}-front-000.000.png")),
        char_dir.join("sprites/sing/frames/frame_0001.png"),
        char_dir.join("sprites/jump/frames/frame_0001.png"),
        char_dir.join("sprites/walk/frames/frame_0001.png"),
        char_dir.join("sprites/run/frames/frame_0001.png"),
        char_dir.join(format!("{raw_name}-assembled.png")),
        char_dir.join(format!("{canonical_id}-assembled.png")),
        char_dir.join("rig/assembled.png"),
        char_dir.join("cutout.png"),
        char_dir.join("flow-original.png"),
        char_dir.join("3d/breakouts").join(format!("{canonical_id}-cutout.png")),
        char_dir.join("angles/front.png"),
        char_dir.join("parts/source/body-plate_v1.png"),
    ];
    for candidate in direct_candidates {
        if is_image_file(&candidate) {
            return Some(candidate);
        }
    }
    for sub in ["front/frames", "sprites", "drafts", "parts", "angles", "rig", "canvas_layers"] {
        let dir = char_dir.join(sub);
        if dir.is_dir() {
            let entries = sorted_dir_entries(&dir);
            if let Some(cutout) = entries.iter().find(|p| {
                is_image_file(p)
                    && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                        let low = n.to_ascii_lowercase();
                        low.contains("cutout")
                            || low.contains("assembled")
                            || low.contains("preview")
                            || low.contains("reference")
                            || low.contains("walk")
                            || low == "body.png"
                    })
            }) {
                return Some(cutout.clone());
            }
            if let Some(img) = first_image_in_tree(&dir, 2) {
                return Some(img);
            }
        }
    }
    first_image_in_tree(char_dir, 2)
}

fn humanize_character_name(raw: &str) -> String {
    raw.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn count_ext_in_tree(dir: &Path, exts: &[&str], max_depth: usize) -> usize {
    if !dir.is_dir() {
        return 0;
    }
    let mut total = 0;
    for entry in sorted_dir_entries(dir) {
        if entry.is_file() && crate::workspace::has_extension(&entry, exts) {
            total += 1;
        } else if max_depth > 0 && entry.is_dir() {
            total += count_ext_in_tree(&entry, exts, max_depth - 1);
        }
    }
    total
}

fn inspect_project_characters(project_path: &Path) -> Vec<Value> {
    let Some(project_dir) = project_path.parent() else {
        return Vec::new();
    };
    let chars_root = project_dir.join("assets/characters");
    if !chars_root.is_dir() {
        return Vec::new();
    }
    let project_name = project_path.file_stem().and_then(|v| v.to_str()).unwrap_or("Untitled").to_string();
    let project_str = project_path.to_string_lossy().into_owned();
    let skip_names = ["approved-crops", "lineup", "expressions", "contact-parts-v1"];
    let mut out = Vec::new();
    for char_dir in sorted_dir_entries(&chars_root) {
        if !char_dir.is_dir() {
            continue;
        }
        let Some(raw_name) = char_dir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if skip_names.iter().any(|skip| raw_name.eq_ignore_ascii_case(skip)) {
            continue;
        }
        let canonical_id = crate::workspace::sanitize_asset_id(raw_name);
        if canonical_id.is_empty() {
            continue;
        }
        let rigged = ["front", "side", "back"].iter().any(|v| {
            let vdir = char_dir.join(v);
            vdir.join(format!("{canonical_id}-{v}.ecproj")).is_file() || vdir.join(format!("{raw_name}-{v}.ecproj")).is_file()
        }) || count_ext_in_tree(&char_dir, &["ecproj"], 1) > 0;
        let sprites_count =
            sorted_dir_entries(&char_dir.join("sprites")).into_iter().filter(|d| d.is_dir() && count_ext_in_tree(d, &["ecproj", "png"], 2) > 0).count();
        let parts_count = ["parts", "parts/animation-layers", "canvas_layers", "rig"]
            .iter()
            .map(|sub| count_ext_in_tree(&char_dir.join(sub), &["png", "webp"], 0))
            .max()
            .unwrap_or(0)
            .max(count_ext_in_tree(&char_dir, &["png"], 0));
        let has_3d = count_ext_in_tree(&char_dir, &["glb", "gltf"], 2) > 0;
        let thumb = find_character_preview(project_dir, &char_dir, raw_name, &canonical_id);
        if thumb.is_none() && !rigged && sprites_count == 0 && !has_3d {
            continue;
        }
        let status = if rigged && sprites_count > 0 {
            format!("Rigged · {sprites_count} action cycles")
        } else if rigged {
            "EffectCraft rig".to_string()
        } else if sprites_count > 0 {
            format!("{sprites_count} sprite cycles")
        } else if has_3d {
            "3D GLB model".to_string()
        } else if parts_count > 1 {
            format!("Layered parts ({parts_count})")
        } else {
            "Ready cutout".to_string()
        };
        out.push(json!({
            "id": canonical_id,
            "folder_name": raw_name,
            "name": humanize_character_name(raw_name),
            "project_name": project_name,
            "project_path": project_str,
            "thumb": thumb.map(|p| p.to_string_lossy().into_owned()),
            "rigged": rigged,
            "sprites_count": sprites_count,
            "parts_count": parts_count,
            "has_3d": has_3d,
            "status": status,
        }));
    }
    out
}

fn find_project_thumb(dir: &Path, chars: &[Value]) -> Option<String> {
    let preview = dir.join("storyboard/.preview/page-01.png");
    if preview.is_file() {
        return Some(preview.to_string_lossy().into_owned());
    }
    for sub in ["storyboard/.preview", "storyboard/thumbnails", "assets/backgrounds"] {
        if let Some(img) = first_image_in_tree(&dir.join(sub), 2) {
            return Some(img.to_string_lossy().into_owned());
        }
    }
    chars.iter().find_map(|c| c["thumb"].as_str().map(str::to_owned))
}

fn discover_workspace_magic_files(root: &Path, max_depth: usize, out: &mut Vec<PathBuf>) {
    if !root.is_dir() {
        return;
    }
    let skip_dirs = ["history", "backups", "command-center-tests-20261021", "target", "node_modules", ".git", "tripo-demos"];
    for entry in sorted_dir_entries(root) {
        if entry.is_file() && entry.extension().is_some_and(|e| e.eq_ignore_ascii_case("magic")) {
            out.push(entry);
        } else if max_depth > 0 && entry.is_dir() && entry.file_name().and_then(|n| n.to_str()).is_none_or(|n| !skip_dirs.contains(&n)) {
            discover_workspace_magic_files(&entry, max_depth - 1, out);
        }
    }
}

pub fn catalog(s: &mut crate::Session, params: &Value) -> crate::Result<Value> {
    if let Some(forget_path) = params["forget"].as_str() {
        forget(forget_path).map_err(crate::EngineError::InvalidParams)?;
    }
    let active = s.project_path.as_ref().map(|p| p.to_string_lossy().into_owned());
    let mut seen = std::collections::HashSet::new();
    let mut projects: Vec<Value> = list()
        .into_iter()
        .map(|p| fs::canonicalize(&p).map(|v| v.to_string_lossy().into_owned()).unwrap_or(p))
        .filter(|p| !(is_transient(p) && !Path::new(p).is_file()))
        .filter(|p| seen.insert(p.clone()))
        .map(|p| {
            let path = Path::new(&p);
            let dir = path.parent().unwrap_or(path);
            let is_open = active.as_deref() == Some(p.as_str());
            let readiness = readiness(path);
            let chars = if path.is_file() { inspect_project_characters(path) } else { Vec::new() };
            let char_ids: Vec<String> = chars.iter().filter_map(|c| c["id"].as_str().map(str::to_owned)).collect();
            let thumb = if path.is_file() { find_project_thumb(dir, &chars) } else { None };
            let availability = if !path.is_file() {
                "missing"
            } else if readiness["openable"] == true {
                "available"
            } else {
                "unreadable"
            };
            json!({
                "path": p,
                "project_id": p,
                "identity_kind": "canonical path",
                "name": path.file_stem().and_then(|v| v.to_str()).unwrap_or(&p),
                "folder": dir,
                "exists": path.is_file(),
                "thumb": thumb,
                "character_count": char_ids.len(),
                "characters": char_ids,
                "availability": availability,
                "open_state": if is_open { "attached" } else { "not_observed_open" },
                "sessions": [],
                "readiness": readiness
            })
        })
        .collect();
    projects.sort_by_key(|v| v["exists"] != true);

    // Build character inventory across recent projects + local workspace roots (`projects/`, `media/`).
    let mut char_project_paths: Vec<PathBuf> = seen.iter().map(PathBuf::from).filter(|p| p.is_file()).collect();
    let mut discovered = Vec::new();
    for sub in ["projects", "media"] {
        discover_workspace_magic_files(&s.cwd.join(sub), 3, &mut discovered);
    }
    for found in discovered {
        if let Ok(canon) = fs::canonicalize(&found)
            && !char_project_paths.contains(&canon)
        {
            char_project_paths.push(canon);
        }
    }
    let mut seen_char_dirs = std::collections::HashSet::new();
    let mut characters = Vec::new();
    for proj_path in &char_project_paths {
        let Some(proj_dir) = proj_path.parent() else { continue };
        for ch in inspect_project_characters(proj_path) {
            let folder_key = proj_dir.join("assets/characters").join(ch["folder_name"].as_str().unwrap_or_default()).to_string_lossy().into_owned();
            if seen_char_dirs.insert(folder_key) {
                characters.push(ch);
            }
        }
    }

    Ok(json!({
        "projects": projects,
        "characters": characters,
        "source": file(),
        "scope": "existing Home navigation history; newest ten, not all filesystem projects",
        "discovery_errors": []
    }))
}

fn readiness(path: &Path) -> Value {
    let Ok(text) = fs::read_to_string(path) else {
        return json!({"state":"unreadable","openable":false,"next_step":"Locate the project file, or remove this entry from Recent.","editor_ready":false});
    };
    let Ok(doc) = serde_json::from_str::<Value>(&text) else {
        return json!({"state":"unreadable","openable":false,"next_step":"The project file contains invalid JSON.","editor_ready":false});
    };
    if doc["format"].as_str() != Some("magic.project") {
        return json!({"state":"unreadable","openable":false,"next_step":"This file is not a Magic Studio project.","editor_ready":false});
    }
    let dir = path.parent().unwrap_or(path);
    let audio = doc["workshop"]["audio"].as_str().is_some_and(|a| !a.is_empty() && (dir.join(a).is_file() || Path::new(a).is_file()));
    let transcript = crate::cmd::project::workshop::transcript_files(dir).iter().any(|path| {
        crate::cmd::project::workshop::transcript_document(path)
            .ok()
            .is_some_and(|document| document["words"].as_array().is_some_and(|words| !words.is_empty()))
    });
    let editor_ready = doc["workshop"]["video_project"].as_str().is_some_and(|v| !v.is_empty() && (dir.join(v).exists() || Path::new(v).exists()));
    let next = if !audio {
        "Choose existing song audio"
    } else if !transcript {
        "Import or review timestamped transcript"
    } else if !editor_ready {
        "Review project; editor setup or server is still required"
    } else {
        "Open project"
    };
    json!({"openable":true,"audio_available":audio,"transcript_valid":transcript,"editor_ready":editor_ready,"approval":"not inferred","next_step":next,"problems":[],"quick_actions":[{"label":"Project","view":"project","open_block":"song","enabled":true},{"label":"Lyrics / transcript","view":"project","open_block":"lyrics","enabled":transcript},{"label":"Studio","view":"studio","enabled":editor_ready}]})
}
