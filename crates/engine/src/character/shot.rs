//! `character.shot`: one character's sprite placed in a film-sized EffectCraft comp for one scene:
//! the sprite comp nested, scaled, keyed to travel from one point to another (or bounce in place),
//! optionally flipped, then rendered as a ProRes 4444 clip with alpha for FilmCraft's character
//! track. Only EffectCraft commands.

use serde_json::{Value, json};

use super::{bad, effectcraft, point, sprite::sprite_dir, step};
use crate::{Result, Session};

/// Film frame and rate; matches `video.assemble`.
const FRAME: [u32; 2] = [1920, 1080];
const FPS: f64 = 30.0;

struct Placement {
    from: [f64; 2],
    to: [f64; 2],
    scale: f64,
    mirror: bool,
    duration: f64,
    /// Jumps: lift the sprite off the ground once per sprite cycle (seconds), this many pixels.
    hop: Option<(f64, f64)>,
}

fn placement(p: &Value) -> Placement {
    let from = point(&p["from"]).unwrap_or([960.0, 780.0]);
    Placement {
        from,
        to: point(&p["to"]).unwrap_or(from),
        scale: p["scale"].as_f64().unwrap_or(100.0),
        mirror: p["mirror"] == true,
        duration: p["duration"].as_f64().unwrap_or(4.0).clamp(0.2, 600.0),
        hop: p["hop_period"]
            .as_f64()
            .filter(|t| *t > 0.1)
            .map(|t| (t, p["height"].as_f64().unwrap_or(140.0))),
    }
}

fn shot_steps(
    sprite_project: &std::path::Path,
    sprite_comp: &str,
    name: &str,
    at: &Placement,
) -> Vec<Value> {
    let sx = if at.mirror { -at.scale } else { at.scale };
    let key = move |t: f64, v: [f64; 2]| {
        step(
            "prop.addKey",
            json!({"layer": 1, "path": "transform/position", "time": t, "timeBase": "comp", "value": v}),
        )
    };
    vec![
        step("file.importProject", json!({"path": sprite_project})),
        step(
            "comp.new",
            json!({"name": name, "width": FRAME[0], "height": FRAME[1], "frameRate": FPS,
            "duration": at.duration, "open": true}),
        ),
        step(
            "layer.addItem",
            json!({"item": sprite_comp, "position": at.from, "duration": at.duration}),
        ),
        step(
            "layer.setTransform",
            json!({"layers": [1], "prop": "scale", "value": [sx, at.scale]}),
        ),
    ]
    .into_iter()
    .chain(position_keys(at).into_iter().map(|(t, v)| key(t, v)))
    .collect()
}

/// Travel from `from` to `to`; for a jump, ground, apex and ground again in each sprite cycle,
/// timed to the drawn take-off (15%), apex (50%) and landing (75%) of the cycle.
fn position_keys(at: &Placement) -> Vec<(f64, [f64; 2])> {
    let along = |t: f64| {
        let k = t / at.duration;
        [
            at.from[0] + (at.to[0] - at.from[0]) * k,
            at.from[1] + (at.to[1] - at.from[1]) * k,
        ]
    };
    let Some((period, height)) = at.hop else {
        return vec![(0.0, at.from), (at.duration, at.to)];
    };
    let mut keys = vec![(0.0, at.from)];
    let mut start = 0.0;
    while start + period * 0.75 <= at.duration {
        for (phase, lift) in [(0.15, 0.0), (0.5, height), (0.75, 0.0)] {
            let t = start + period * phase;
            let p = along(t);
            keys.push((t, [p[0], p[1] - lift]));
        }
        start += period;
    }
    keys.push((at.duration, at.to));
    keys
}

pub fn cmd_character_shot(s: &mut Session, p: &Value) -> Result<Value> {
    let id = p["character"]
        .as_str()
        .map(crate::workspace::sanitize_asset_id)
        .ok_or_else(|| bad("character=<id> is required"))?;
    let sprite = p["sprite"]
        .as_str()
        .ok_or_else(|| bad("sprite=<name> is required"))?;
    let name = p["name"]
        .as_str()
        .map_or_else(|| format!("{id}-{sprite}-shot"), str::to_string);
    let sprite_comp = format!("{id}-{sprite}");
    let sprite_project = sprite_dir(s, &id, sprite)?.join(format!("{sprite_comp}.ecproj"));
    if !sprite_project.is_file() {
        return Err(bad(format!(
            "{} is missing; run character.sprite first",
            sprite_project.display()
        )));
    }
    let at = placement(p);
    let dir = crate::workshop::project_dir(s)?
        .join("assets/characters")
        .join(&id)
        .join("shots");
    std::fs::create_dir_all(&dir)?;
    let project = dir.join(format!("{name}.ecproj"));
    let mut steps = shot_steps(&sprite_project, &sprite_comp, &name, &at);
    steps.push(step("file.saveAs", json!({"path": project})));
    effectcraft(&dir, None, &steps)?;
    let clip = dir.join(format!("{name}.mov"));
    crate::character::effectcraft_render(
        &project,
        json!({
            "comp": name, "output": clip, "format": "prores", "proresProfile": "4444", "channels": "rgba"
        }),
    )?;
    Ok(
        json!({"ok": true, "clip": clip, "ecproj": project, "comp": name, "duration": at.duration,
        "from": at.from, "to": at.to, "scale": at.scale, "mirror": at.mirror, "hops": at.hop.is_some()}),
    )
}
