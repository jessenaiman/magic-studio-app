//! Small adapter from native transform properties to the existing authored word curves.
use super::invalid;
use crate::Result;
use serde_json::{Value, json};

pub(crate) fn curve(spec: &Value) -> Result<(Value, Value)> {
    let property = spec["property"].as_str().ok_or_else(|| invalid("Choose a native transform property"))?;
    let columns: &[usize] = match property {
        "transform/position" => &[1, 2],
        "transform/scale" => &[3, 4],
        "transform/rotation" => &[5],
        "transform/opacity" => &[6],
        _ => return Err(invalid("This property is not supported by word curves")),
    };
    let duration =
        spec["duration"].as_f64().filter(|v| v.is_finite() && (0.05..=8.0).contains(v)).ok_or_else(|| invalid("Motion duration must be 0.05–8 seconds"))?;
    let delay = spec
        .get("delay")
        .map_or(Some(0.0), Value::as_f64)
        .filter(|v| v.is_finite() && *v >= 0.0 && *v + duration <= 8.0)
        .ok_or_else(|| invalid("Delay plus duration must fit within eight seconds"))?;
    let initial = if matches!(property, "transform/scale" | "transform/opacity") { 100.0 } else { 0.0 };
    let values = |value: &Value, default: Option<f64>| -> Result<Vec<f64>> {
        let data = if value.is_null() {
            vec![default.ok_or_else(|| invalid("A target value is required"))?; columns.len()]
        } else if columns.len() == 1 {
            vec![value.as_f64().ok_or_else(|| invalid("Expected a scalar value"))?]
        } else {
            value
                .as_array()
                .filter(|v| v.len() == columns.len())
                .ok_or_else(|| invalid("Expected two coordinates"))?
                .iter()
                .map(|v| v.as_f64().ok_or_else(|| invalid("Coordinates must be numbers")))
                .collect::<Result<Vec<_>>>()?
        };
        for (&number, &column) in data.iter().zip(columns) {
            let (low, high) = match column {
                1 => (-1920.0, 1920.0),
                2 => (-1080.0, 1080.0),
                3 | 4 => (1.0, 400.0),
                5 => (-14400.0, 14400.0),
                _ => (0.0, 100.0),
            };
            if !number.is_finite() || !(low..=high).contains(&number) {
                return Err(invalid("Transform value is outside the supported range"));
            }
        }
        Ok(data)
    };
    let from = values(&spec["from"], Some(initial))?;
    let to_value = if let Some(speed) = spec.get("speed") {
        if property != "transform/rotation" {
            return Err(invalid("Speed is expressed in degrees/second for rotation; use from/to for other channels"));
        }
        let speed = speed
            .as_f64()
            .filter(|v| v.is_finite() && v.abs() <= 1800.0)
            .ok_or_else(|| invalid("Rotation speed must be between -1800 and 1800 degrees/second"))?;
        json!(from[0] + speed * duration)
    } else {
        spec["to"].clone()
    };
    let to = values(&to_value, None)?;
    let mut first = vec![0.0, 0.0, 0.0, 100.0, 100.0, 0.0, 100.0];
    let mut last = first.clone();
    last[0] = delay + duration;
    for ((&column, &start), &end) in columns.iter().zip(&from).zip(&to) {
        first[column] = start;
        last[column] = end;
    }
    let value = |numbers: &[f64]| if numbers.len() == 1 { json!(numbers[0]) } else { json!(numbers) };
    let easing = spec.get("easing").map(crate::animation_model::parse_easing).unwrap_or(crate::animation_model::Easing::Linear);
    let (bx1, by1, bx2, by2) = easing.as_bezier();
    let loop_mode = crate::animation_model::parse_loop_mode(spec.get("loop_mode").and_then(Value::as_str));
    let first_frame = spec.get("first_frame").and_then(Value::as_u64).unwrap_or(0) as u32;
    let resolved_frame = crate::animation_model::evaluate_symbol_frame(loop_mode, first_frame, (duration * 30.0).round().max(1.0) as u32, 0);
    let mut config = json!({
        "property": property,
        "from": value(&from),
        "to": value(&to),
        "duration": duration,
        "delay": delay,
        "easing": easing.label(),
        "bezier": [bx1, by1, bx2, by2],
        "symbol": {
            "kind": "graphic",
            "loop_mode": match loop_mode {
                crate::animation_model::LoopMode::Loop => "loop",
                crate::animation_model::LoopMode::PlayOnce => "play_once",
                crate::animation_model::LoopMode::SingleFrame => "single_frame",
            },
            "first_frame": resolved_frame,
        }
    });
    if property == "transform/rotation" {
        config["speed"] = json!((to[0] - from[0]) / duration);
    }
    let sampled = crate::animation_model::sample_eased_curve(&from, &to, delay, duration, easing, 6);
    let keys: Vec<Value> = sampled
        .into_iter()
        .map(|(time, vals)| {
            let mut row = first.clone();
            row[0] = time;
            for (&column, &val) in columns.iter().zip(&vals) {
                row[column] = val;
            }
            json!(row)
        })
        .collect();
    Ok((config, json!(keys)))
}
