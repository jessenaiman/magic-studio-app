//! ArtCraft project file trunk (`project.trunk`) and multi-format file import (`project.import_file`).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::{Value, json};

use crate::cmd::project::cmd_project_add_asset;
use crate::cmd::project::workshop::{IMAGE_EXTENSIONS, project_dir, song_seconds};
use crate::workspace::{AUDIO_EXTENSIONS, has_extension, resolve_workspace_path};
use crate::{EngineError, Result, Session};

const CRAFT_DOC_EXTS: &[&str] = &[
    "designcraft",
    "idml",
    "deckcraft",
    "pdfcraft",
    "ecproj",
    "fcproj",
    "pcraft",
    "psd",
    "vectorcraft",
    "vcraft",
    "drawcraft",
    "tcraft",
    "mcraft",
    "cadcraft",
    "solidcraft",
];

struct FileClass {
    group: &'static str,
    group_label: &'static str,
    group_order: u8,
    craft_priority: u8,
    badge: &'static str,
    open_label: &'static str,
    is_craft_project: bool,
}

fn classify_file(path: &Path) -> Option<FileClass> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_ascii_lowercase();
    match ext.as_str() {
        "designcraft" | "idml" => Some(FileClass {
            group: "design_pdf",
            group_label: "Design & PDF",
            group_order: 1,
            craft_priority: 1,
            badge: "DC",
            open_label: "Open in DesignCraft ↗",
            is_craft_project: true,
        }),
        "deckcraft" => Some(FileClass {
            group: "design_pdf",
            group_label: "Design & PDF",
            group_order: 1,
            craft_priority: 2,
            badge: "DK",
            open_label: "Open in DeckCraft ↗",
            is_craft_project: true,
        }),
        "pdf" | "pdfcraft" => Some(FileClass {
            group: "design_pdf",
            group_label: "Design & PDF",
            group_order: 1,
            craft_priority: 3,
            badge: "PDF",
            open_label: "Open",
            is_craft_project: ext == "pdfcraft",
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
        _ => None,
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

fn should_skip_dir(rel: &str, name: &str) -> bool {
    if name.starts_with('.') || matches!(name, "node_modules" | "target" | "beats") {
        return true;
    }
    matches!(rel, "film/frames" | "assets/drawings/storyboard-actions" | "artifacts/storyboard-images")
}

fn walk_project_files(root: &Path, current: &Path, depth: usize, connected: &BTreeSet<PathBuf>, out: &mut Vec<PathBuf>) {
    if depth == 0 {
        return;
    }
    let Ok(read_dir) = std::fs::read_dir(current) else {
        return;
    };
    let mut entries: Vec<PathBuf> = read_dir.flatten().map(|e| e.path()).collect();
    entries.sort();
    for entry in entries {
        let Some(name) = entry.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('.') || name.ends_with(".steps.jsonl") || name.ends_with(".attribution") {
            continue;
        }
        let rel = entry.strip_prefix(root).ok().map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default();
        if entry.is_dir() {
            if !should_skip_dir(&rel, name) {
                walk_project_files(root, &entry, depth - 1, connected, out);
            }
            continue;
        }
        if !entry.is_file() {
            continue;
        }
        // Keep primary character cutouts connected in `.magic`, skip sliced internal rig part PNGs.
        if rel.starts_with("assets/characters/") && rel.contains("/parts/") && has_extension(&entry, IMAGE_EXTENSIONS) {
            let canon = entry.canonicalize().unwrap_or_else(|_| entry.clone());
            if !connected.contains(&canon) && !connected.contains(&entry) {
                continue;
            }
        }
        out.push(entry);
    }
}

/// Scan the open project folder and return grouped ArtCraft project & media files with inspected metadata.
pub fn cmd_project_trunk(s: &mut Session, _params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let canon_dir = dir.canonicalize().unwrap_or_else(|_| dir.clone());
    let duration_sec = song_seconds(&dir);
    let mut connected = BTreeSet::new();
    for asset in &s.project_assets {
        let Some(p) = &asset.path else { continue };
        let abs = if p.is_absolute() { p.clone() } else { dir.join(p) };
        connected.insert(abs.canonicalize().unwrap_or(abs));
    }

    let mut discovered = Vec::new();
    walk_project_files(&canon_dir, &canon_dir, 5, &connected, &mut discovered);
    if let Some(audio) = &s.song_audio
        && audio.is_file()
    {
        let canon_audio = audio.canonicalize().unwrap_or_else(|_| audio.clone());
        if !discovered.iter().any(|p| p == &canon_audio) {
            discovered.push(canon_audio);
        }
    }

    let mut rows = Vec::new();
    for path in discovered {
        if s.project_path.as_ref().is_some_and(|mp| mp == &path) {
            continue;
        }
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
        let detail = inspect_detail(&path, &canon_dir, cls.badge, size, duration_sec);
        rows.push(json!({
            "name": name,
            "path": path,
            "rel_dir": rel_dir,
            "group": cls.group,
            "group_label": cls.group_label,
            "group_order": cls.group_order,
            "craft_priority": cls.craft_priority,
            "badge": cls.badge,
            "open_label": cls.open_label,
            "is_craft_project": cls.is_craft_project,
            "size_bytes": size,
            "modified_ms": modified_ms,
            "detail": detail,
        }));
    }

    rows.sort_by(|a, b| {
        let ga = a["group_order"].as_u64().unwrap_or(99);
        let gb = b["group_order"].as_u64().unwrap_or(99);
        let pa = a["craft_priority"].as_u64().unwrap_or(99);
        let pb = b["craft_priority"].as_u64().unwrap_or(99);
        let na = a["name"].as_str().unwrap_or_default();
        let nb = b["name"].as_str().unwrap_or_default();
        ga.cmp(&gb).then_with(|| pa.cmp(&pb)).then_with(|| na.to_ascii_lowercase().cmp(&nb.to_ascii_lowercase()))
    });

    Ok(json!({
        "ok": true,
        "project_dir": canon_dir,
        "count": rows.len(),
        "files": rows,
    }))
}

fn target_subfolder_for_ext(ext: &str) -> &'static str {
    match ext {
        "designcraft" | "idml" | "deckcraft" | "pdf" | "pdfcraft" => "storyboard",
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
