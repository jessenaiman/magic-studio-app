//! Layer and asset gates: Magic's crate layers from docs/architecture.md › Layers, and asset
//! attribution sidecars.
use crate::Result;
use serde_json::Value;
use std::process::Command;

/// Magic's own crates and their layers (docs/architecture.md › Layers).
const LAYERS: &[(&str, u8)] = &[
    ("magic", 5),
    ("magic-ui-egui", 6),
    ("magic-mcp", 6),
    ("magic-gui", 7),
    ("magic-cli", 7),
    ("magic-tui", 7),
    ("xtask", 7),
];

/// Craft backends (L0–L4): maintained dependencies Magic drives, never edited here.
const CRAFT_PREFIXES: &[&str] = &[
    "designcraft-",
    "effectcraft-",
    "filmcraft-",
    "photocraft-",
    "vectorcraft-",
    "texturecraft-",
    "craft-",
];

/// Crates that must not appear below L6 (UI toolkits, windowing, OS dialogs).
const UI_ONLY: &[&str] = &[
    "craft-ui",
    "craft-ai-egui",
    "egui",
    "eframe",
    "egui-wgpu",
    "winit",
    "rfd",
    "tauri",
    "tauri-plugin-dialog",
];

fn layer_of(name: &str) -> Option<u8> {
    if let Some((_, l)) = LAYERS.iter().find(|(n, _)| *n == name) {
        return Some(*l);
    }
    CRAFT_PREFIXES
        .iter()
        .any(|p| name.starts_with(p))
        .then_some(4)
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
                "{name}: not assigned a layer (add it to xtask LAYERS and docs/architecture.md › Layers)"
            ));
            continue;
        };
        for d in deps {
            if l < 6 && UI_ONLY.contains(&d.as_str()) {
                errors.push(format!("{name} (L{l}) depends on UI crate `{d}`"));
            }
            let Some(dl) = layer_of(d) else { continue };
            if dl >= l {
                errors.push(format!(
                    "{name} (L{l}) depends on {d} (L{dl}): edges must go down"
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
