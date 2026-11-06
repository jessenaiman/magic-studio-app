//! FilmCraft layer and asset gates, preserving its required configuration and diagnostics.
//! https://github.com/storytold/filmcraft/blob/2508f5420eec2a6b7873f216beadbce2930b530b/xtask/src/main.rs#L32-L251
//! Magic's native orchestration/frontend mapping is documented in docs/architecture.md.
use crate::Result;
use serde_json::Value;
use std::process::Command;

/// (crate name without the `filmcraft-` prefix, layer).
const LAYERS: &[(&str, u8)] = &[
    ("testkit", 0),
    ("time", 0),
    ("geom", 0),
    ("color", 0),
    ("bitstream", 0),
    ("isobmff", 0),
    ("avi", 0),
    ("matroska", 0),
    ("mxf", 0),
    ("cfb", 0),
    ("mpegts", 0),
    ("mpeg2v", 0),
    ("ac3", 0),
    ("mpa", 0),
    ("ogg", 0),
    ("riff", 0),
    ("h264", 0),
    ("h264enc", 0),
    ("hevc", 0),
    ("vp8", 0),
    ("vp9", 0),
    ("prores", 0),
    ("mjpeg", 0),
    ("dnx", 0),
    ("apv", 0),
    ("dng", 0),
    ("av1", 0),
    ("aac", 0),
    ("flac", 0),
    ("opus", 0),
    ("frame", 1),
    ("media", 1),
    ("project", 1),
    ("audio-dsp", 1),
    ("text", 1),
    ("edit", 2),
    ("codecs", 2),
    ("keyframe", 2),
    ("effects", 2),
    ("audio", 2),
    ("captions", 2),
    ("speech", 2),
    ("tts", 2),
    ("tts-text", 2),
    ("interchange", 2),
    ("render", 3),
    ("gpu", 3),
    ("golden", 3),
    ("scopes", 3),
    ("playback", 3),
    ("export", 3),
    ("format", 3),
    ("engine", 4),
    ("platform", 5),
    ("ui-egui", 5),
    ("automation", 5),
    ("filmcraft", 6),
    ("cli", 6),
    ("web", 6),
    // Magic composes the L4 document engines through native services; its frontend is L6.
    ("magic", 5),
    ("magic-ui-egui", 5),
    ("magic-mcp", 5),
    ("magic-gui", 6),
    ("magic-cli", 6),
    ("magic-tui", 6),
];

/// L0 crates that are not codecs/containers (no `bitstream`-only restriction).
const L0_FOUNDATION: &[&str] = &["time", "geom", "color", "bitstream", "testkit"];

/// Allowed same-layer edges (from, to).
const SAME_LAYER: &[(&str, &str)] = &[
    ("media", "frame"),
    ("project", "media"),
    ("project", "frame"),
    ("effects", "keyframe"),
    ("audio", "keyframe"),
    ("playback", "render"),
    ("playback", "gpu"),
    ("export", "render"),
    ("scopes", "gpu"),
    ("gpu", "render"),
    ("cli", "filmcraft"),
    ("tts", "tts-text"),
    ("magic-ui-egui", "magic"),
    ("magic-mcp", "magic"),
];

/// Crates that must not appear below L5 (UI toolkits, windowing, OS audio/menus).
const UI_ONLY: &[&str] = &[
    "craft-ui",
    "egui",
    "eframe",
    "egui-wgpu",
    "winit",
    "rfd",
    "cpal",
    "muda",
    "tauri",
    "tauri-plugin-dialog",
];

fn short(name: &str) -> &str {
    name.strip_prefix("filmcraft-").unwrap_or(name)
}

fn layer_of(name: &str) -> Option<u8> {
    LAYERS
        .iter()
        .find(|(n, _)| *n == short(name))
        .map(|(_, l)| *l)
}

fn metadata() -> Result<Value> {
    let out = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into());
    }
    serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
}

fn workspace_crates(md: &Value) -> Vec<(String, Vec<String>)> {
    md["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["name"] != "xtask")
        .map(|p| {
            let deps = p["dependencies"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|d| d["kind"].is_null() || d["kind"] == "build")
                .filter_map(|d| d["name"].as_str().map(str::to_string))
                .collect();
            (p["name"].as_str().unwrap_or_default().to_string(), deps)
        })
        .collect()
}

pub(crate) fn layers() -> Result<()> {
    let md = metadata()?;
    let crates = workspace_crates(&md);
    let mut errors = Vec::new();
    for (name, deps) in &crates {
        let Some(l) = layer_of(name) else {
            errors.push(format!(
                "{name}: not assigned a layer (add it to xtask LAYERS and docs/architecture.md §1)"
            ));
            continue;
        };
        for d in deps {
            if (l < 5 || name == "magic") && UI_ONLY.contains(&d.as_str()) {
                errors.push(format!("{name} (L{l}) depends on UI/OS crate `{d}`"));
            }
            // Keep FilmCraft's checks and include every edge within Magic's workspace.
            if !d.starts_with("filmcraft-") && !crates.iter().any(|(name, _)| name == d) {
                continue;
            }
            let Some(dl) = layer_of(d) else { continue };
            let (a, b) = (short(name), short(d));
            if l == 0 && !L0_FOUNDATION.contains(&a) && b != "bitstream" {
                errors.push(format!(
                    "{a} (L0 codec/container) may depend only on bitstream, found {b}"
                ));
            } else if dl > l {
                errors.push(format!("{a} (L{l}) depends upward on {b} (L{dl})"));
            } else if dl == l && l > 0 && !SAME_LAYER.contains(&(a, b)) {
                errors.push(format!(
                    "{a} → {b}: same-layer edge (L{l}) not in the allowed list"
                ));
            }
        }
    }
    if errors.is_empty() {
        println!("layers: ok");
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

/// File extensions that count as assets (AGENTS.md §1).
const ASSET_EXT: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "ico", "icns", "bmp", "tif", "tiff", "heic",
    "avif", "exr", "ttf", "otf", "ttc", "woff", "woff2", "cube", "3dl", "lut", "look", "wav",
    "mp3", "aac", "flac", "ogg", "opus", "m4a", "aif", "aiff", "mp4", "mov", "m4v", "mkv", "webm",
    "avi", "mxf", "psd", "ai", "eps", "pdf", "prproj", "ffx", "prfpset", "mogrt", "aep",
];

/// Sidecar fields that must be present and non-empty.
const REQUIRED_FIELDS: &[&str] = &["asset", "title", "author", "source", "license", "added"];

fn repo_files() -> Result<Vec<String>> {
    let out = Command::new("git")
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into());
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .filter(|f| std::path::Path::new(f).exists())
        .collect())
}

fn is_asset(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ASSET_EXT.contains(&e.to_ascii_lowercase().as_str()))
}

pub(crate) fn assets() -> Result<()> {
    let files = repo_files()?;
    let index =
        std::fs::read_to_string("ATTRIBUTION.md").map_err(|e| format!("ATTRIBUTION.md: {e}"))?;
    let mut errors = Vec::new();
    let mut n = 0;
    for f in files.iter().filter(|f| is_asset(f)) {
        n += 1;
        let lower = f.to_ascii_lowercase();
        if lower.contains("adobe") || lower.contains("premiere") {
            errors.push(format!(
                "{f}: asset paths must not reference Adobe/Premiere (AGENTS.md §1)"
            ));
        }
        let side = format!("{f}.attribution");
        match std::fs::read_to_string(&side) {
            Err(_) => errors.push(format!("{f}: missing attribution sidecar {side}")),
            Ok(text) => {
                for field in REQUIRED_FIELDS {
                    let ok = text.lines().any(|l| {
                        l.split_once(':')
                            .is_some_and(|(k, v)| k.trim() == *field && !v.trim().is_empty())
                    });
                    if !ok {
                        errors.push(format!("{side}: field `{field}` missing or empty"));
                    }
                }
                let lic = text.lines().find_map(|l| {
                    l.split_once(':')
                        .filter(|(k, _)| k.trim() == "license")
                        .map(|(_, v)| v.trim().to_ascii_lowercase())
                });
                if lic.is_some_and(|l| {
                    l.contains("-nc")
                        || l.contains("-nd")
                        || l.contains("adobe")
                        || l.contains("proprietary")
                }) {
                    errors.push(format!(
                        "{side}: licence not allowed (no NC/ND, Adobe or proprietary licences)"
                    ));
                }
            }
        }
        if !index.contains(&format!("`{f}`")) {
            errors.push(format!("{f}: not listed in ATTRIBUTION.md"));
        }
    }
    for f in files.iter().filter(|f| f.ends_with(".attribution")) {
        let asset = f.trim_end_matches(".attribution");
        if !std::path::Path::new(asset).exists() {
            errors.push(format!("{f}: sidecar for a file that does not exist"));
        }
    }
    if errors.is_empty() {
        println!("assets: ok ({n} assets attributed)");
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}
