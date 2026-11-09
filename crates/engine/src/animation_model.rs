//! Shared tween, easing, property, and nested symbol timeline model powered by [`anicraft_doc`].
//!
//! Unifies Words/Lyrics motion and 2D Character rig/sprite animation on AniCraft's pure L1
//! primitives:
//! - [`Easing`]: `PRESETS`, `Classic(f32)` (`-100..=100`), `CubicBezier(x1, y1, x2, y2)`,
//!   round-trip Bézier control points [`Easing::as_bezier`], and [`Easing::apply`].
//! - [`Property`] / [`Track`] / [`MotionTween`]: `X`, `Y`, `ScaleX`, `ScaleY`, `Rotation`, `Skew`,
//!   `Alpha`, `Tint` with [`Property::from_key`], [`Property::range`], [`Property::unit`], and
//!   [`Property::group`].
//! - [`Symbol`] / [`SymbolKind`] / [`LoopMode`] / [`Timeline`]: nested symbol timelines with
//!   `Loop`, `PlayOnce`, and `SingleFrame` evaluation onto the main song timeline.

pub use anicraft_doc::{
    Easing, Keyframe, Layer as AniLayer, LoopMode, MAX_SYMBOL_DEPTH, MotionTween, Property, PropertyKey, Props, Symbol, SymbolId, SymbolKind, Timeline, Track,
};
pub use anicraft_geom::{Affine, Decomposed, Vec2};
use serde_json::{Value, json};

/// Parse a JSON easing value (preset name, `{"classic": strength}`, or `[x1, y1, x2, y2]`)
/// into [`anicraft_doc::Easing`].
pub fn parse_easing(raw: &Value) -> Easing {
    if let Some(arr) = raw.as_array()
        && arr.len() == 4
        && let (Some(x1), Some(y1), Some(x2), Some(y2)) = (arr[0].as_f64(), arr[1].as_f64(), arr[2].as_f64(), arr[3].as_f64())
    {
        return Easing::CubicBezier(x1 as f32, y1 as f32, x2 as f32, y2 as f32);
    }
    if let Some(strength) = raw.get("classic").and_then(Value::as_f64) {
        return Easing::Classic(strength as f32);
    }
    if let Some(bez) = raw.get("bezier").and_then(Value::as_array)
        && bez.len() == 4
        && let (Some(x1), Some(y1), Some(x2), Some(y2)) = (bez[0].as_f64(), bez[1].as_f64(), bez[2].as_f64(), bez[3].as_f64())
    {
        return Easing::CubicBezier(x1 as f32, y1 as f32, x2 as f32, y2 as f32);
    }
    let key = raw.as_str().unwrap_or("cubic_out").trim().to_ascii_lowercase().replace([' ', '-'], "_");
    match key.as_str() {
        "linear" | "step_hold" => Easing::Linear,
        "quad_in" | "ease_in_quad" | "ease_in" => Easing::QuadIn,
        "quad_out" | "ease_out_quad" | "ease_out" => Easing::QuadOut,
        "quad_in_out" | "ease_in_out_quad" | "ease_in_out" => Easing::QuadInOut,
        "cubic_in" | "ease_in_cubic" => Easing::CubicIn,
        "cubic_out" | "ease_out_cubic" | "spring_out" => Easing::CubicOut,
        "cubic_in_out" | "ease_in_out_cubic" | "elastic_pop" => Easing::CubicInOut,
        "smooth" | "smooth_bezier" | "cubic_bezier" => Easing::CubicBezier(0.25, 0.1, 0.25, 1.0),
        _ => Easing::CubicOut,
    }
}

/// Parse a loop mode string (`"loop"`, `"play_once"`, `"once"`, `"single_frame"`, `"hold"`) into [`LoopMode`].
pub fn parse_loop_mode(raw: Option<&str>) -> LoopMode {
    match raw.unwrap_or("loop").trim().to_ascii_lowercase().replace([' ', '-'], "_").as_str() {
        "play_once" | "once" | "playonce" => LoopMode::PlayOnce,
        "single_frame" | "singleframe" | "hold" | "still" => LoopMode::SingleFrame,
        _ => LoopMode::Loop,
    }
}

/// Evaluate which 0-based frame inside a nested [`Symbol`] timeline is shown when `elapsed_frames`
/// have passed since the symbol instance started on the parent song timeline.
pub fn evaluate_symbol_frame(loop_mode: LoopMode, first_frame: u32, total_frames: u32, elapsed_frames: u32) -> u32 {
    let len = total_frames.max(1);
    let start = first_frame.min(len - 1);
    match loop_mode {
        LoopMode::SingleFrame => start,
        LoopMode::PlayOnce => (start + elapsed_frames).min(len - 1),
        LoopMode::Loop => (start + elapsed_frames) % len,
    }
}

/// Build the JSON catalog of [`Easing::PRESETS`] with roundtrip [`Easing::as_bezier`] control points.
pub fn easing_catalog() -> Value {
    let presets: Vec<Value> = Easing::PRESETS
        .iter()
        .map(|(label, ease)| {
            let id = match ease {
                Easing::Linear => "linear",
                Easing::QuadIn => "quad_in",
                Easing::QuadOut => "quad_out",
                Easing::QuadInOut => "quad_in_out",
                Easing::CubicIn => "cubic_in",
                Easing::CubicOut => "cubic_out",
                Easing::CubicInOut => "cubic_in_out",
                Easing::Classic(_) => "classic",
                Easing::CubicBezier(_, _, _, _) => "smooth",
            };
            let (x1, y1, x2, y2) = ease.as_bezier();
            let samples = [0.0_f32, 0.25, 0.5, 0.75, 1.0].map(|t| ease.apply(t));
            json!({
                "id": id,
                "name": label,
                "does": ease.label(),
                "bezier": [x1, y1, x2, y2],
                "samples": samples,
            })
        })
        .collect();
    json!(presets)
}

/// Build the JSON catalog of [`Property::ALL`] (`x`, `y`, `scale_x`, `scale_y`, `rotation`, `skew`, `alpha`, `tint`).
pub fn property_catalog() -> Value {
    let props: Vec<Value> = Property::ALL
        .iter()
        .map(|p| {
            let r = p.range();
            json!({
                "key": p.key(),
                "label": p.label(),
                "unit": p.unit(),
                "group": p.group(),
                "min": *r.start(),
                "max": *r.end(),
            })
        })
        .collect();
    json!(props)
}

/// Sample an eased curve between `from` and `to` over `[delay, delay + duration]` using [`Easing::apply`].
pub fn sample_eased_curve(from: &[f64], to: &[f64], delay: f64, duration: f64, easing: Easing, subdivisions: usize) -> Vec<(f64, Vec<f64>)> {
    let mut out = Vec::new();
    if delay > 0.0 {
        out.push((0.0, from.to_vec()));
    }
    let count = if matches!(easing, Easing::Linear) { 1 } else { subdivisions.clamp(2, 16) };
    for step in 0..=count {
        let t = (step as f32) / (count as f32);
        let k = f64::from(easing.apply(t));
        let time = delay + duration * f64::from(t);
        let vals: Vec<f64> = from.iter().zip(to.iter()).map(|(&a, &b)| a + (b - a) * k).collect();
        out.push((time, vals));
    }
    out
}
