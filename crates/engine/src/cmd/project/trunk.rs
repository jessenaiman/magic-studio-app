//! ArtCraft project file trunk (`project.trunk`) and multi-format file import (`project.import_file`).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;
use serde_json::{Value, json};

use crate::cmd::project::cmd_project_add_asset;
use crate::cmd::project::workshop::{IMAGE_EXTENSIONS, project_dir, song_seconds};
use crate::workspace::{AUDIO_EXTENSIONS, has_extension, resolve_workspace_path};
use crate::{EngineError, ProjectAsset, Result, Session};

const CRAFT_DOC_EXTS: &[&str] =
    &["designcraft", "idml", "ecproj", "fcproj", "pcraft", "psd", "vectorcraft", "vcraft", "drawcraft", "tcraft", "mcraft", "cadcraft", "solidcraft"];

struct FileClass {
    group: &'static str,
    group_label: &'static str,
    group_order: u8,
    craft_priority: u8,
    badge: &'static str,
    open_label: &'static str,
    is_craft_project: bool,
}

/// Engine-owned inventory of the files belonging to one Magic project.
/// Asset registration enriches a file record; it never creates a second UI catalogue.
#[derive(Serialize)]
struct ProjectFiles {
    ok: bool,
    project_dir: PathBuf,
    count: usize,
    registered_asset_count: usize,
    files: Vec<ProjectFile>,
    external_assets: Vec<ProjectAsset>,
    missing_assets: Vec<ProjectAsset>,
}

#[derive(Serialize)]
struct ProjectFile {
    name: String,
    path: PathBuf,
    rel_dir: String,
    group: &'static str,
    group_label: &'static str,
    group_order: u8,
    craft_priority: u8,
    badge: &'static str,
    open_label: &'static str,
    is_craft_project: bool,
    is_image: bool,
    visual_kind: &'static str,
    current_timeline: bool,
    can_animate: bool,
    assets: Vec<ProjectAsset>,
    size_bytes: u64,
    modified_ms: u64,
    detail: String,
}

fn visual_kind(class: &FileClass) -> &'static str {
    match class.badge {
        "FC" | "MP4" | "MOV" | "WEBM" => "film",
        "EC" => "motion",
        "DC" | "PDF" => "design",
        "VC" | "SVG" => "vector",
        "PC" | "PNG" | "JPG" | "WEBP" => "image",
        "MP3" | "WAV" | "FLAC" => "audio",
        "MAGIC" => "project",
        _ if class.group == "texture_3d" => "model",
        _ => "document",
    }
}

fn classify_file(path: &Path) -> Option<FileClass> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_ascii_lowercase();
    match ext.as_str() {
        "magic" => Some(FileClass {
            group: "project_files",
            group_label: "Project & supporting files",
            group_order: 6,
            craft_priority: 0,
            badge: "MAGIC",
            open_label: "Open",
            is_craft_project: true,
        }),
        "designcraft" | "idml" => Some(FileClass {
            group: "design_pdf",
            group_label: "Design & PDF",
            group_order: 1,
            craft_priority: 1,
            badge: "DC",
            open_label: "Open in DesignCraft ↗",
            is_craft_project: true,
        }),
        "pdf" => Some(FileClass {
            group: "design_pdf",
            group_label: "Design & PDF",
            group_order: 1,
            craft_priority: 3,
            badge: "PDF",
            open_label: "Open",
            is_craft_project: false,
        }),
        "ecproj" => Some(FileClass {
            group: "effect_film",
            group_label: "Effect & Film",
            group_order: 2,
            craft_priority: 1,
            badge: "EC",
            open_label: "Open in EffectCraft ↗",
            is_craft_project: true,
        }),
        "fcproj" => Some(FileClass {
            group: "effect_film",
            group_label: "Effect & Film",
            group_order: 2,
            craft_priority: 2,
            badge: "FC",
            open_label: "Open in FilmCraft ↗",
            is_craft_project: true,
        }),
        "mp4" | "mov" | "webm" | "mkv" => Some(FileClass {
            group: "effect_film",
            group_label: "Effect & Film",
            group_order: 2,
            craft_priority: 3,
            badge: if ext == "mov" {
                "MOV"
            } else if ext == "webm" {
                "WEBM"
            } else {
                "MP4"
            },
            open_label: "Play",
            is_craft_project: false,
        }),
        "pcraft" | "psd" => Some(FileClass {
            group: "photo_vector",
            group_label: "Photo & Vector",
            group_order: 3,
            craft_priority: 1,
            badge: "PC",
            open_label: "Open in PhotoCraft ↗",
            is_craft_project: true,
        }),
        "vectorcraft" | "vcraft" | "drawcraft" => Some(FileClass {
            group: "photo_vector",
            group_label: "Photo & Vector",
            group_order: 3,
            craft_priority: 2,
            badge: "VC",
            open_label: "Open in VectorCraft ↗",
            is_craft_project: true,
        }),
        "svg" => Some(FileClass {
            group: "photo_vector",
            group_label: "Photo & Vector",
            group_order: 3,
            craft_priority: 3,
            badge: "SVG",
            open_label: "Open in VectorCraft ↗",
            is_craft_project: false,
        }),
        "png" | "jpg" | "jpeg" | "webp" => Some(FileClass {
            group: "photo_vector",
            group_label: "Photo & Vector",
            group_order: 3,
            craft_priority: 4,
            badge: if ext == "png" {
                "PNG"
            } else if ext == "webp" {
                "WEBP"
            } else {
                "JPG"
            },
            open_label: "Open",
            is_craft_project: false,
        }),
        "tcraft" | "mcraft" => Some(FileClass {
            group: "texture_3d",
            group_label: "3D & Texture",
            group_order: 4,
            craft_priority: 1,
            badge: "TC",
            open_label: "Open in TextureCraft ↗",
            is_craft_project: true,
        }),
        "glb" | "gltf" | "blend" | "cadcraft" | "solidcraft" | "step" | "stp" => Some(FileClass {
            group: "texture_3d",
            group_label: "3D & Texture",
            group_order: 4,
            craft_priority: 2,
            badge: if ext == "glb" || ext == "gltf" {
                "GLB"
            } else if ext == "blend" {
                "3D"
            } else {
                "CAD"
            },
            open_label: "Open",
            is_craft_project: matches!(ext.as_str(), "cadcraft" | "solidcraft"),
        }),
        "mp3" | "wav" | "flac" | "m4a" | "aac" | "ogg" => Some(FileClass {
            group: "audio_words",
            group_label: "Audio & Lyrics",
            group_order: 5,
            craft_priority: 1,
            badge: if ext == "wav" {
                "WAV"
            } else if ext == "flac" {
                "FLAC"
            } else {
                "MP3"
            },
            open_label: "Play",
            is_craft_project: false,
        }),
        "srt" | "lrc" | "txt" | "md" => Some(FileClass {
            group: "audio_words",
            group_label: "Audio & Lyrics",
            group_order: 5,
            craft_priority: 2,
            badge: if ext == "srt" || ext == "lrc" { "SRT" } else { "TXT" },
            open_label: "Open",
            is_craft_project: false,
        }),
        "json" if matches!(name.as_str(), "words.json" | "transcript.json" | "official_lyrics.json") => Some(FileClass {
            group: "audio_words",
            group_label: "Audio & Lyrics",
            group_order: 5,
            craft_priority: 3,
            badge: "JSON",
            open_label: "Open",
            is_craft_project: false,
        }),
        _ => Some(FileClass {
            group: "project_files",
            group_label: "Project & supporting files",
            group_order: 6,
            craft_priority: 4,
            badge: match ext.as_str() {
                "json" | "jsonl" => "JSON",
                "py" | "js" | "ts" | "html" | "css" => "CODE",
                "lock" => "LOCK",
                _ => "FILE",
            },
            open_label: "Open",
            is_craft_project: false,
        }),
    }
}

fn human_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{} KB", (bytes + 512) / 1024)
    } else {
        format!("{bytes} B")
    }
}

fn format_clock(secs: f64) -> String {
    let total = secs.max(0.0).round() as u64;
    format!("{:02}:{:02}", total / 60, total % 60)
}

fn srt_cue_count(path: &Path) -> Option<usize> {
    let text = std::fs::read_to_string(path).ok()?;
    let count = text.lines().filter(|l| l.contains("-->")).count();
    (count > 0).then_some(count)
}

fn words_line_count(dir: &Path) -> Option<usize> {
    let text = std::fs::read_to_string(dir.join("design/words.json")).ok()?;
    let doc: Value = serde_json::from_str(&text).ok()?;
    let count: usize = doc["animations"].as_array()?.iter().map(|a| a["lines"].as_array().map_or(0, Vec::len)).sum();
    (count > 0).then_some(count)
}

fn inspect_detail(path: &Path, dir: &Path, badge: &str, size: u64, duration_sec: Option<f64>) -> String {
    let mut parts: Vec<String> = Vec::new();
    match badge {
        "PC" | "VC" | "EC" => {
            if badge == "EC"
                && path.file_name().and_then(|n| n.to_str()) == Some("lyrics.ecproj")
                && let Some(lines) = words_line_count(dir).or_else(|| srt_cue_count(&dir.join("hyperframes/transcript.srt")))
            {
                parts.push(format!("{lines} timed lines"));
            }
            if size <= 64 * 1024 * 1024
                && let Ok(bytes) = std::fs::read(path)
                && let Ok(info) = craft_doc::info(&bytes, &craft_doc::Limits::default())
            {
                if !info.supported {
                    parts.push("newer schema".to_string());
                } else if parts.is_empty() {
                    parts.push(format!("v{}", info.version));
                }
            }
            parts.push(human_size(size));
        }
        "FC" => {
            parts.push("Master cut".to_string());
            if let Some(secs) = duration_sec {
                parts.push(format_clock(secs));
            } else {
                parts.push(human_size(size));
            }
        }
        "PNG" | "JPG" | "WEBP" => {
            if let Ok((w, h)) = image::image_dimensions(path) {
                parts.push(format!("{w}×{h}"));
            }
            parts.push(human_size(size));
        }
        "SRT" => {
            if let Some(cues) = srt_cue_count(path) {
                parts.push(format!("{cues} timed cues"));
            } else {
                parts.push(human_size(size));
            }
        }
        "MP3" | "WAV" | "FLAC" => {
            if let Some(secs) = duration_sec {
                parts.push(format_clock(secs));
            }
            parts.push(human_size(size));
        }
        _ => {
            parts.push(human_size(size));
        }
    }
    parts.join(" · ")
}

fn walk_project_files(root: &Path, current: &Path, visited: &mut BTreeSet<PathBuf>, out: &mut Vec<PathBuf>) {
    let Ok(canonical) = current.canonicalize() else {
        return;
    };
    // A project index never traverses an external link or a directory cycle.
    if !canonical.starts_with(root) || !visited.insert(canonical) {
        return;
    }
    let Ok(read_dir) = std::fs::read_dir(current) else {
        return;
    };
    let mut entries: Vec<PathBuf> = read_dir.flatten().map(|e| e.path()).collect();
    entries.sort();
    for entry in entries {
        let Ok(canonical) = entry.canonicalize() else {
            continue;
        };
        if !canonical.starts_with(root) {
            continue;
        }
        if entry.is_dir() {
            walk_project_files(root, &entry, visited, out);
            continue;
        }
        if !entry.is_file() {
            continue;
        }
        out.push(entry);
    }
}

/// Scan the open project folder and return grouped ArtCraft project & media files with inspected metadata.
pub fn cmd_project_trunk(s: &mut Session, _params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let canon_dir = dir.canonicalize().unwrap_or_else(|_| dir.clone());
    let duration_sec = song_seconds(&dir);
    let timeline = crate::cmd::film::film_path(s).ok().and_then(|p| p.canonicalize().ok());
    let mut assets_by_path = std::collections::BTreeMap::<PathBuf, Vec<ProjectAsset>>::new();
    let mut external_assets = Vec::new();
    let mut missing_assets = Vec::new();
    for asset in &s.project_assets {
        let Some(path) = asset.path.as_ref() else {
            missing_assets.push(asset.clone());
            continue;
        };
        let absolute = if path.is_absolute() { path.clone() } else { canon_dir.join(path) };
        match absolute.canonicalize() {
            Ok(canonical) if canonical.starts_with(&canon_dir) => assets_by_path.entry(canonical).or_default().push(asset.clone()),
            Ok(_) => external_assets.push(asset.clone()),
            Err(_) => missing_assets.push(asset.clone()),
        }
    }
    let mut discovered = Vec::new();
    walk_project_files(&canon_dir, &canon_dir, &mut BTreeSet::new(), &mut discovered);

    let mut rows = Vec::new();
    for path in discovered {
        let Some(cls) = classify_file(&path) else {
            continue;
        };
        let meta = std::fs::metadata(&path).ok();
        let size = meta.as_ref().map_or(0, std::fs::Metadata::len);
        let modified_ms = meta.and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64);
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        let rel_dir = path
            .parent()
            .and_then(|p| p.strip_prefix(&canon_dir).ok())
            .map(|r| {
                let s = r.to_string_lossy().replace('\\', "/");
                if s.is_empty() { "./".to_string() } else { format!("{s}/") }
            })
            .unwrap_or_else(|| "linked/".to_string());
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        let assets = assets_by_path.get(&canonical).cloned().unwrap_or_default();
        let current_timeline = timeline.as_ref() == Some(&canonical);
        let can_animate = !rel_dir.starts_with("motion/")
            && !rel_dir.starts_with("lettering/")
            && assets
                .first()
                .is_some_and(|asset| matches!(asset.kind.as_str(), "image" | "model_3d" | "character" | "cutout_rig" | "poses" | "turnaround" | "anim_3d"));
        let mut detail = inspect_detail(&path, &canon_dir, cls.badge, size, duration_sec);
        if current_timeline {
            detail = format!("Current timeline · {detail}");
        }
        rows.push(ProjectFile {
            name,
            is_image: has_extension(&path, IMAGE_EXTENSIONS),
            visual_kind: visual_kind(&cls),
            path,
            rel_dir,
            group: cls.group,
            group_label: cls.group_label,
            group_order: cls.group_order,
            craft_priority: cls.craft_priority,
            badge: cls.badge,
            open_label: cls.open_label,
            is_craft_project: cls.is_craft_project,
            current_timeline,
            can_animate,
            assets,
            size_bytes: size,
            modified_ms,
            detail,
        });
    }

    rows.sort_by(|a, b| {
        b.current_timeline
            .cmp(&a.current_timeline)
            .then_with(|| a.group_order.cmp(&b.group_order))
            .then_with(|| a.craft_priority.cmp(&b.craft_priority))
            .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
    });

    serde_json::to_value(ProjectFiles {
        ok: true,
        project_dir: canon_dir,
        count: rows.len(),
        registered_asset_count: s.project_assets.len(),
        files: rows,
        external_assets,
        missing_assets,
    })
    .map_err(|e| EngineError::InvalidParams(e.to_string()))
}

fn target_subfolder_for_ext(ext: &str) -> &'static str {
    match ext {
        "designcraft" | "idml" | "pdf" => "storyboard",
        "ecproj" => "lettering",
        "fcproj" | "mp4" | "mov" | "webm" | "mkv" => "film",
        "pcraft" | "psd" => "assets/backgrounds",
        "vectorcraft" | "vcraft" | "drawcraft" | "svg" => "assets/vectors",
        "tcraft" | "mcraft" | "glb" | "gltf" | "blend" | "cadcraft" | "solidcraft" | "step" | "stp" => "assets/3d",
        _ if AUDIO_EXTENSIONS.contains(&ext) => "audio",
        "srt" | "lrc" | "txt" | "md" => "liner-notes",
        _ => "assets/files",
    }
}

/// Import one or more Craft, PDF, media, or artwork files into the open project's canonical folder structure.
pub fn cmd_project_import_file(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let mut raw_paths: Vec<String> = params.get("paths").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect();
    if let Some(single) = params.get("path").and_then(Value::as_str).filter(|p| !p.trim().is_empty()) {
        raw_paths.push(single.to_owned());
    }
    if raw_paths.is_empty() {
        return Err(EngineError::InvalidParams("missing 'paths' or 'path' to import".into()));
    }
    let overwrite = params.get("overwrite").and_then(Value::as_bool).unwrap_or(false);
    let mut imported = Vec::new();

    for raw in &raw_paths {
        let src = resolve_workspace_path(s, raw);
        if !src.is_file() {
            return Err(EngineError::InvalidParams(format!("file not found: {}", src.display())));
        }
        if has_extension(&src, IMAGE_EXTENSIONS) {
            let stem = src.file_stem().and_then(|st| st.to_str()).unwrap_or("artwork");
            let res = cmd_project_add_asset(
                s,
                &json!({
                    "path": src,
                    "kind": "image",
                    "name": stem,
                    "image": stem,
                    "if_not_exists": true,
                    "load": false,
                }),
            )?;
            imported.push(res.get("path").cloned().unwrap_or_else(|| json!(src)));
            continue;
        }

        let ext = src.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
        let subfolder = target_subfolder_for_ext(&ext);
        let dest_dir = dir.join(subfolder);
        std::fs::create_dir_all(&dest_dir)?;
        let file_name = src.file_name().ok_or_else(|| EngineError::InvalidParams("invalid file name".into()))?;
        let dest = dest_dir.join(file_name);

        if src.canonicalize().ok() != dest.canonicalize().ok() {
            let bytes = std::fs::read(&src)?;
            if CRAFT_DOC_EXTS.contains(&ext.as_str())
                && matches!(ext.as_str(), "pcraft" | "vectorcraft" | "ecproj")
                && let Ok(info) = craft_doc::info(&bytes, &craft_doc::Limits::default())
                && !info.supported
            {
                log::warn!("importing {} with newer craft-doc schema v{}", src.display(), info.version);
            }
            if dest.is_file() && !overwrite && std::fs::read(&dest).ok().as_ref() != Some(&bytes) {
                return Err(EngineError::InvalidParams(format!("destination already exists with different contents: {}", dest.display())));
            }
            if !dest.exists() || overwrite {
                craft_atomic::atomic_write(&dest, &bytes)?;
            }
        }

        if matches!(ext.as_str(), "glb" | "gltf") {
            let stem = dest.file_stem().and_then(|st| st.to_str()).unwrap_or("model");
            let _ = cmd_project_add_asset(
                s,
                &json!({
                    "path": dest,
                    "kind": "model_3d",
                    "name": stem,
                    "if_not_exists": true,
                    "load": false,
                }),
            );
        }
        imported.push(json!(dest));
    }

    let trunk = cmd_project_trunk(s, &json!({}))?;
    Ok(json!({
        "ok": true,
        "imported": imported,
        "trunk": trunk,
    }))
}
