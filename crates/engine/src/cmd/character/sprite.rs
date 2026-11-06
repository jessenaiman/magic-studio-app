//! `character.sprite`: a Scribbles action sheet becomes a 2D sprite cycle. PhotoCraft crops each
//! cell into a frame and removes its background; EffectCraft imports the frames as an image sequence, loops it with Time
//! Remap + `loopOut("cycle")` and (for a left-facing copy) flips it. Saved as an `.ecproj`.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{bad, effectcraft, step};
use crate::{Result, Session};

/// Where one sprite lives: `<project>/assets/characters/<id>/sprites/<name>/`.
pub(crate) fn sprite_dir(s: &Session, id: &str, name: &str) -> Result<PathBuf> {
    Ok(crate::workshop::project_dir(s)?
        .join("assets/characters")
        .join(id)
        .join("sprites")
        .join(name))
}

/// Crop the sheet's cells (left to right, top to bottom) into frame_0001.png, frame_0002.png, …
fn crop_frames(dir: &Path, sheet: &Path, grid: (u32, u32)) -> Result<Vec<PathBuf>> {
    let (w, h) = image::image_dimensions(sheet)?;
    let (cols, rows) = grid;
    let (cw, ch) = (w / cols, h / rows);
    let frames = dir.join("frames");
    if frames.is_dir() {
        std::fs::remove_dir_all(&frames)?;
    }
    std::fs::create_dir_all(&frames)?;
    let mut out = Vec::new();
    for i in 0..cols * rows {
        let file = frames.join(format!("frame_{:04}.png", i + 1));
        let crop = json!({"x": (i % cols) * cw, "y": (i / cols) * ch, "width": cw, "height": ch});
        crate::cmd::character::cut::photocraft_run(
            sheet,
            &[
                ("image.crop", crop),
                ("layer.removeBackground", json!({"refine": true})),
            ],
            Some(&file),
        )?;
        out.push(file);
    }
    Ok(out)
}

pub fn cmd_character_sprite(s: &mut Session, p: &Value) -> Result<Value> {
    let id = p["character"]
        .as_str()
        .map(crate::workspace::sanitize_asset_id)
        .ok_or_else(|| bad("character=<id> is required"))?;
    let action = p["action"]
        .as_str()
        .ok_or_else(|| bad("action=<walk|jump|…> is required"))?;
    let name = p["name"].as_str().unwrap_or(action).to_string();
    let sheet = p["sheet"]
        .as_str()
        .map(|f| crate::workspace::resolve_workspace_path(s, f))
        .filter(|f| f.is_file())
        .ok_or_else(|| bad("sheet=<Scribbles action sheet PNG> is required"))?;
    let grid = (
        p["columns"].as_u64().unwrap_or(4).clamp(1, 16) as u32,
        p["rows"].as_u64().unwrap_or(2).clamp(1, 16) as u32,
    );
    let fps = p["fps"].as_u64().unwrap_or(12).clamp(1, 60);
    let duration = p["duration"].as_f64().unwrap_or(4.0).clamp(0.5, 600.0);
    let dir = sprite_dir(s, &id, &name)?;
    std::fs::create_dir_all(&dir)?;
    std::fs::copy(&sheet, dir.join("sheet.png"))?;
    let frames = crop_frames(&dir, &dir.join("sheet.png"), grid)?;
    let comp = format!("{id}-{name}");
    let project = dir.join(format!("{comp}.ecproj"));
    let mut steps = vec![
        step(
            "file.import",
            json!({"paths": [frames[0]], "sequence": true, "frameRate": fps, "newComp": true}),
        ),
        step(
            "comp.settings",
            json!({"name": comp, "duration": duration, "frameRate": fps}),
        ),
        step(
            "layer.enableTimeRemap",
            json!({"layers": [1], "value": true}),
        ),
        step("layer.timing", json!({"layers": [1], "out": duration})),
        step(
            "prop.setExpression",
            json!({"layer": 1, "path": "timeRemap", "expression": "loopOut(\"cycle\")"}),
        ),
    ];
    if p["mirror"] == true {
        steps.push(step(
            "layer.setTransform",
            json!({"layers": [1], "prop": "scale", "value": [-100, 100]}),
        ));
    }
    steps.push(step("file.saveAs", json!({"path": project})));
    effectcraft(&dir, None, &steps, true)?;
    let meta = json!({"character": id, "sprite": name, "action": action, "sheet": dir.join("sheet.png"),
        "source_sheet": sheet, "frames": frames.len(), "fps": fps, "cycle_seconds": frames.len() as f64 / fps as f64,
        "duration": duration, "mirror": p["mirror"] == true, "comp": comp, "ecproj": project});
    std::fs::write(
        dir.join("sprite.json"),
        serde_json::to_string_pretty(&meta).map_err(|e| bad(e.to_string()))?,
    )?;
    Ok(json!({"ok": true, "sprite": meta,
        "next": format!("character.render character={id} sprite={name} start=0 end={}", frames.len() as f64 / fps as f64)}))
}
