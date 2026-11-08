//! Layer and asset gates: Magic's crate layers from docs/architecture.md › Layers, and asset
//! attribution sidecars.
use crate::Result;
use serde_json::Value;
use std::process::Command;

/// Magic's own crates and their layers (docs/architecture.md › Layers).
const LAYERS: &[(&str, u8)] = &[("magic-engine", 5), ("magic-mcp", 6), ("magic", 7), ("magic-cli", 7), ("magic-tui", 7), ("xtask", 7)];

/// Craft backends (L0–L4): maintained dependencies Magic drives, never edited here.
const CRAFT_PREFIXES: &[&str] = &["designcraft-", "effectcraft-", "filmcraft-", "photocraft-", "vectorcraft-", "texturecraft-", "craft-"];

/// Crates that must not appear below L6 (UI toolkits, windowing, OS dialogs).
const UI_ONLY: &[&str] = &["craft-ui", "craft-ai-egui", "egui", "eframe", "egui-wgpu", "winit", "rfd", "tauri", "tauri-plugin-dialog"];

fn layer_of(name: &str) -> Option<u8> {
    if let Some((_, l)) = LAYERS.iter().find(|(n, _)| *n == name) {
        return Some(*l);
    }
    CRAFT_PREFIXES.iter().any(|p| name.starts_with(p)).then_some(4)
}

pub(crate) fn metadata() -> Result<Value> {
    let out = Command::new(env!("CARGO")).args(["metadata", "--format-version", "1", "--no-deps"]).output().map_err(|e| e.to_string())?;
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
            errors.push(format!("{name}: not assigned a layer (add it to xtask LAYERS and docs/architecture.md › Layers)"));
            continue;
        };
        for d in deps {
            if l < 6 && UI_ONLY.contains(&d.as_str()) {
                errors.push(format!("{name} (L{l}) depends on UI crate `{d}`"));
            }
            let Some(dl) = layer_of(d) else { continue };
            if dl >= l {
                errors.push(format!("{name} (L{l}) depends on {d} (L{dl}): edges must go down"));
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
