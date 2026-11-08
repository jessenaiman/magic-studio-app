//! Exact rational time base for Magic Studio, backed by [`filmcraft_time`].
//!
//! All engine timing (`words`, `motion`, `film`, `character`) operates in [`Tick`]s
//! (`254_016_000_000` ticks per second) and rational [`FrameRate`] (`num / den`),
//! eliminating floating-point drift across 23.976 / 24 / 29.97 / 30 / 60 fps timelines.

pub use filmcraft_time::{FrameRate, TICKS_PER_SECOND, Tick, TimeRange};
use serde_json::{Value, json};

/// Parse a [`Tick`] from a JSON value:
/// - `{"ticks": i64}` -> `Tick(i64)`
/// - integer `i64` -> `Tick(i64)` when already in tick scale (`|n| >= TICKS_PER_SECOND / 10_000` or `0`),
///   or `Tick::from_units(n, 1)` when a small whole-second literal is supplied.
/// - float `f64` (at external ingestion boundaries only) -> `Tick::from_seconds_f64(f)`
pub fn parse_tick(v: &Value) -> Option<Tick> {
    if let Some(t) = v.get("ticks").and_then(Value::as_i64) {
        return Some(Tick(t));
    }
    if let Some(n) = v.as_i64() {
        if n == 0 || n.unsigned_abs() >= (TICKS_PER_SECOND as u64 / 10_000) {
            return Some(Tick(n));
        }
        return Some(Tick::from_units(n, 1));
    }
    v.as_f64().filter(|f| f.is_finite()).map(Tick::from_seconds_f64)
}

/// Parse an exact [`Tick`] when the value is guaranteed to be raw ticks (or `{"ticks": i64}`).
pub fn parse_raw_tick(v: &Value) -> Option<Tick> {
    v.get("ticks").and_then(Value::as_i64).or_else(|| v.as_i64()).map(Tick)
}

/// Parse a rational [`FrameRate`] from a JSON value:
/// - `{"num": 30000, "den": 1001}` -> `FrameRate::new(30000, 1001)`
/// - `"29.97"` / `"23.976"` / `"24"` / `"30"` -> matching `FrameRate`
/// - integer `i64` (e.g. `24`, `30`, `60`) -> `FrameRate::new(n, 1)`
/// - float `f64` (e.g. `29.97`) -> `FrameRate::from_f64(fps)`
pub fn parse_frame_rate(v: &Value) -> Option<FrameRate> {
    if let (Some(num), Some(den)) = (v.get("num").and_then(Value::as_i64), v.get("den").and_then(Value::as_i64)) {
        return (num > 0 && den > 0).then(|| FrameRate::new(num, den));
    }
    if let Some(s) = v.as_str() {
        let trimmed = s.trim().trim_end_matches("fps").trim();
        if let Some((num_s, den_s)) = trimmed.split_once('/')
            && let (Ok(num), Ok(den)) = (num_s.trim().parse::<i64>(), den_s.trim().parse::<i64>())
        {
            return (num > 0 && den > 0).then(|| FrameRate::new(num, den));
        }
        return match trimmed {
            "23.976" | "23.98" => Some(FrameRate::FPS_23_976),
            "24" => Some(FrameRate::FPS_24),
            "25" => Some(FrameRate::FPS_25),
            "29.97" => Some(FrameRate::FPS_29_97),
            "30" => Some(FrameRate::FPS_30),
            "48" => Some(FrameRate::FPS_48),
            "50" => Some(FrameRate::FPS_50),
            "59.94" => Some(FrameRate::FPS_59_94),
            "60" => Some(FrameRate::FPS_60),
            "119.88" => Some(FrameRate::FPS_119_88),
            "120" => Some(FrameRate::FPS_120),
            _ => trimmed.parse::<i64>().ok().filter(|&n| n > 0).map(|n| FrameRate::new(n, 1)),
        };
    }
    if let Some(n) = v.as_i64() {
        return (n > 0).then(|| FrameRate::new(n, 1));
    }
    v.as_f64().filter(|f| f.is_finite() && *f > 0.0).map(FrameRate::from_f64)
}

/// Serialize a [`FrameRate`] as a JSON object `{"num": num, "den": den}`.
pub fn frame_rate_json(rate: FrameRate) -> Value {
    let r = rate.sane();
    json!({ "num": r.num, "den": r.den })
}
