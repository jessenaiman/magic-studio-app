//! Saved lyric/drawing thumbnails rendered by EffectCraft, with no project writes.
use crate::{EngineError, Result, Session};
use image::{GenericImageView, ImageBuffer, Rgba};
use serde_json::{Value, json};
use std::path::Path;

fn choose_sample_time(layers: &[Value], layer_type: &str, requested: f64) -> f64 {
    let mut best_span: Option<(f64, f64, f64)> = None;
    for l in layers.iter().filter(|l| l["type"] == layer_type) {
        let l_in = l["in"].as_f64().unwrap_or(0.0);
        let l_out = l["out"].as_f64().unwrap_or(l_in + 1.0).max(l_in + 0.05);
        let dist = if requested < l_in {
            l_in - requested
        } else if requested > l_out {
            requested - l_out
        } else {
            0.0
        };
        if best_span.is_none_or(|(_, _, best_dist)| dist < best_dist) {
            best_span = Some((l_in, l_out, dist));
        }
    }
    let Some((l_in, l_out, dist)) = best_span else {
        return requested;
    };
    let span = (l_out - l_in).max(0.1);
    if layer_type == "Shape" {
        // Sample after the trim-path draw-on completes so the stroke is clearly visible.
        let settled = (l_in + span * 0.72).min(l_out - 0.04).max(l_in + 0.04);
        if dist > 0.0 { settled } else { requested.max(settled).min(l_out - 0.04) }
    } else {
        let enter_done = (l_in + (span * 0.35).min(0.45)).min(l_out - 0.04);
        if dist > 0.0 { enter_done } else { requested.clamp(enter_done, (l_out - 0.04).max(enter_done)) }
    }
}

fn enhance_and_crop_preview(path: &Path) {
    let Ok(img) = image::open(path) else {
        return;
    };
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return;
    }
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut light_px = 0u64;
    let mut total_px = 0u64;

    for (x, y, p) in rgba.enumerate_pixels() {
        if p[3] > 16 {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            if p[3] > 80 {
                total_px += 1;
                let lum = 0.299 * (p[0] as f64) + 0.587 * (p[1] as f64) + 0.114 * (p[2] as f64);
                if lum > 185.0 {
                    light_px += 1;
                }
            }
        }
    }
    if min_x > max_x || min_y > max_y {
        return;
    }

    // If most opaque pixels are light-colored (e.g., cream/gold text designed for dark scenes),
    // add a crisp dark ink outline so the thumbnail is legible on Studio's warm paper cards.
    let mut work = rgba.clone();
    if total_px > 0 && light_px * 3 >= total_px * 2 {
        let ink = [23u8, 63u8, 73u8, 235u8];
        let x0 = min_x.saturating_sub(3);
        let y0 = min_y.saturating_sub(3);
        let x1 = (max_x + 3).min(w - 1);
        let y1 = (max_y + 3).min(h - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                if rgba.get_pixel(x, y)[3] <= 32 {
                    let mut near_opaque = false;
                    'nbr: for dy in -2i32..=2 {
                        for dx in -2i32..=2 {
                            if dx * dx + dy * dy > 5 {
                                continue;
                            }
                            let nx = x as i32 + dx;
                            let ny = y as i32 + dy;
                            if nx >= 0 && ny >= 0 && (nx as u32) < w && (ny as u32) < h && rgba.get_pixel(nx as u32, ny as u32)[3] > 110 {
                                near_opaque = true;
                                break 'nbr;
                            }
                        }
                    }
                    if near_opaque {
                        work.put_pixel(x, y, Rgba(ink));
                    }
                }
            }
        }
    }

    // Crop around the active bounding box padded to 16:9 so words & drawings fill the Studio filmstrip cell.
    let bw = (max_x - min_x + 1) as f64;
    let bh = (max_y - min_y + 1) as f64;
    let cx = min_x as f64 + bw * 0.5;
    let cy = min_y as f64 + bh * 0.5;
    let target_w = (bw * 1.28).max(bh * 1.28 * (16.0 / 9.0)).clamp((w as f64) * 0.42, w as f64);
    let target_h = (target_w * (9.0 / 16.0)).clamp((h as f64) * 0.42, h as f64);
    let crop_w = (target_w.round() as u32).clamp(1, w);
    let crop_h = (target_h.round() as u32).clamp(1, h);
    let crop_x = ((cx - (crop_w as f64) * 0.5).round() as i64).clamp(0, (w - crop_w) as i64) as u32;
    let crop_y = ((cy - (crop_h as f64) * 0.5).round() as i64).clamp(0, (h - crop_h) as i64) as u32;

    let sub = work.view(crop_x, crop_y, crop_w, crop_h).to_image();
    if let Some(buf) = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(crop_w, crop_h, sub.into_raw()) {
        let _ = buf.save(path);
    }
}

pub fn frame(s: &mut Session, p: &Value) -> Result<Value> {
    let time =
        p["time"].as_f64().filter(|v| v.is_finite() && *v >= 0.0).ok_or_else(|| EngineError::InvalidParams("time must be nonnegative seconds".into()))?;
    let kind = p["kind"].as_str().unwrap_or("lyrics");
    let dir = super::workspace_dir(s)?;
    let project = dir.join("lettering/lyrics.ecproj");
    if !project.is_file() {
        let _ = super::lettering::cmd_words_letter(s, &json!({"render": false}));
    }
    if !project.is_file() {
        return Err(EngineError::InvalidParams("No saved EffectCraft lyrics composition; save the lyric design first".into()));
    }
    let folder = dir.join("lettering/.preview");
    std::fs::create_dir_all(&folder)?;
    let out = folder.join(format!("{kind}-{}.png", (time * 1000.0).round() as u64));
    if out.is_file()
        && let (Ok(out_meta), Ok(proj_meta)) = (std::fs::metadata(&out), std::fs::metadata(&project))
        && out_meta.len() > 0
        && let (Ok(out_time), Ok(proj_time)) = (out_meta.modified(), proj_meta.modified())
        && out_time >= proj_time
    {
        return Ok(json!({"frame":out,"source":project,"kind":kind,"time":time,"sample_time":time,"native":"effectcraft"}));
    }
    let comp: Value = crate::CraftHub::execute(crate::CraftTarget::Effect, Some(&project), "comp.info", &json!({"comp": "Lyrics"}))
        .or_else(|_| crate::CraftHub::execute(crate::CraftTarget::Effect, Some(&project), "comp.info", &json!({})))?;
    if kind == "stage" {
        let scale = p["scale"].as_f64().unwrap_or(0.67).clamp(0.25, 1.0);
        crate::workspace::cmd_effect_exec(
            s,
            &json!({"path":project,"save":false,"steps":[
                {"id":"comp.open","params":{"comp":comp["id"]}},
                {"id":"comp.saveFrameAs","params":{"path":out,"time":time,"scale":scale,"transparent":true}}
            ]}),
        )?;
        if !out.is_file() {
            return Err(EngineError::InvalidParams("EffectCraft did not produce the stage frame".into()));
        }
        return Ok(json!({"frame":out,"source":project,"kind":"stage","time":time,"sample_time":time,"native":"effectcraft"}));
    }
    let layer_type = match kind {
        "lyrics" => "Text",
        "motion" => "Shape",
        _ => return Err(EngineError::InvalidParams("kind must be lyrics, motion, or stage".into())),
    };
    let layers = comp["layers"].as_array().ok_or_else(|| EngineError::InvalidParams("EffectCraft returned no active composition layers".into()))?;
    let ids: Vec<Value> = layers.iter().filter(|l| l["type"] == layer_type).map(|l| l["id"].clone()).collect();
    if ids.is_empty() {
        return Ok(json!({"empty":true,"kind":kind,"time":time,"source":project}));
    }
    let sample_time = choose_sample_time(layers, layer_type, time);
    crate::workspace::cmd_effect_exec(
        s,
        &json!({"path":project,"save":false,"steps":[
            {"id":"comp.open","params":{"comp":comp["id"]}},
            {"id":"layer.setSwitch","params":{"layers":ids,"switch":"video","value":false,"others":true}},
            {"id":"comp.saveFrameAs","params":{"path":out,"time":sample_time,"scale":0.35,"transparent":true}},
            {"id":"layer.setSwitch","params":{"layers":ids,"switch":"video","value":true,"others":false}}
        ]}),
    )?;
    if !out.is_file() {
        return Err(EngineError::InvalidParams("EffectCraft did not produce the preview frame".into()));
    }
    enhance_and_crop_preview(&out);
    Ok(json!({"frame":out,"source":project,"kind":kind,"time":time,"sample_time":sample_time,"native":"effectcraft"}))
}
