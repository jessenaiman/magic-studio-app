//! Workshop asset quality diagnostics (`assets.assess`) and asset fix-list
//! management (`assets.fixlist`, `assets.fixlist_add`, `assets.request_sheet`).

use std::path::PathBuf;

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};
use crate::workshop::{FIXLIST, project_dir, studio};
use crate::workspace::resolve_workspace_path;

fn fixlist_path(s: &Session) -> Result<PathBuf> {
    Ok(project_dir(s)?.join(FIXLIST))
}

fn read_fixlist(s: &Session) -> Result<Vec<Value>> {
    let path = fixlist_path(s)?;
    Ok(std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("assets").and_then(Value::as_array).cloned())
        .unwrap_or_default())
}

fn fixlist_next(items: &[Value]) -> Vec<String> {
    let clean = items
        .iter()
        .filter(|i| i["decision"] == json!("clean") && i["status"] != json!("done"))
        .count();
    let recreate = items
        .iter()
        .filter(|i| i["decision"] == json!("recreate") && i["status"] != json!("done"))
        .count();
    let mut next = Vec::new();
    if recreate > 0 {
        next.push(format!(
            "{recreate} asset(s) to recreate: run assets.request_sheet, then give the brief it writes to the image agent (one asset sheet, one pass)"
        ));
    }
    if clean > 0 {
        next.push(format!(
            "{clean} asset(s) to clean: run each item's recorded `tool` command, check the result at full size, then assets.fixlist_add asset=<id> status=done"
        ));
    }
    if next.is_empty() {
        next.push(
            "Nothing open. Assess new or changed assets with assets.assess path=<png>".into(),
        );
    }
    next
}

const CUT_SIDES: [&str; 4] = ["left", "right", "top", "bottom"];

/// Everything one image layer measures, before any joint is excused.
struct ImageMeasures {
    size: (u32, u32),
    opaque: f64,
    partial: f64,
    touching: f64,
    raggedness: f64,
    smear: f64,
    smear_aspect: f64,
    cuts: [u32; 4],
    full_bleed: bool,
}

fn border_touch_ratio(img: &image::RgbaImage, (w, h): (u32, u32)) -> f64 {
    let border: Vec<(u32, u32)> = (0..w)
        .flat_map(|x| [(x, 0), (x, h - 1)])
        .chain((0..h).flat_map(|y| [(0, y), (w - 1, y)]))
        .collect();
    let touching = border
        .iter()
        .filter(|(x, y)| img.get_pixel(*x, *y)[3] > 128)
        .count() as f64;
    touching / border.len().max(1) as f64
}

/// Opaque pixels with a transparent neighbour: the outline length, as a pixel count.
fn edge_pixels(img: &image::RgbaImage, (w, h): (u32, u32)) -> usize {
    let alpha = |x: u32, y: u32| img.get_pixel(x, y)[3];
    let mut edge = 0usize;
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            if alpha(x, y) > 128
                && [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
                    .iter()
                    .any(|(a, b)| alpha(*a, *b) <= 128)
            {
                edge += 1;
            }
        }
    }
    edge
}

fn opaque_median_luminance(img: &image::RgbaImage) -> f64 {
    let mut lum: Vec<f64> = img
        .pixels()
        .filter(|p| p[3] > 128)
        .map(|p| 0.299 * f64::from(p[0]) + 0.587 * f64::from(p[1]) + 0.114 * f64::from(p[2]))
        .collect();
    lum.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    lum.get(lum.len() / 2).copied().unwrap_or(0.0)
}

fn measure_image(img: &image::RgbaImage) -> ImageMeasures {
    let size = img.dimensions();
    let opaque = img.pixels().filter(|p| p[3] > 128).count() as f64;
    let partial = img.pixels().filter(|p| p[3] > 8 && p[3] < 247).count() as f64;
    let (smear, smear_aspect) = smear_band(img, opaque_median_luminance(img));
    let (cuts, full_bleed) = straight_cuts(img);
    ImageMeasures {
        size,
        opaque,
        partial,
        touching: border_touch_ratio(img, size),
        raggedness: edge_pixels(img, size) as f64 / opaque.max(1.0).sqrt(),
        smear,
        smear_aspect,
        cuts,
        full_bleed,
    }
}

/// Sides where a joint excuses a straight edge; anything else named is a request error.
fn parse_joints(params: &Value) -> Result<Vec<&str>> {
    let joints: Vec<&str> = match params.get("joints") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(list)) => list.iter().filter_map(Value::as_str).collect(),
        Some(_) => {
            return Err(EngineError::InvalidParams(
                "joints is a list of sides: left, right, top, bottom".into(),
            ));
        }
    };
    if let Some(bad) = joints.iter().find(|j| !CUT_SIDES.contains(j)) {
        return Err(EngineError::InvalidParams(format!(
            "unknown joint side '{bad}' (left, right, top, bottom)"
        )));
    }
    Ok(joints)
}

/// A joint part (an arm, a wing) ends flat on the side where it joins; only those are excused.
fn apply_joints(measured: [u32; 4], joints: &[&str]) -> [u32; 4] {
    let mut cuts = measured;
    for (i, side) in CUT_SIDES.iter().enumerate() {
        if joints.contains(side) {
            cuts[i] = 0;
        }
    }
    cuts
}

fn assess_reasons(measures: &ImageMeasures, touching: f64, cuts: [u32; 4]) -> Vec<String> {
    let mut reasons = Vec::new();
    for (side, run) in CUT_SIDES.iter().zip(cuts) {
        if run > 0 {
            reasons.push(format!(
                "artwork cut off in a straight line along the {side} edge ({run} px): it was cropped through, not drawn to end there"
            ));
        }
    }
    if touching > 0.15 {
        reasons.push(format!(
            "{:.0}% of the outline runs into the box edge (clipped)",
            touching * 100.0
        ));
    }
    if measures.raggedness > 9.0 {
        reasons.push(format!(
            "ragged edge ({:.1} edge px per sqrt(area))",
            measures.raggedness
        ));
    }
    if measures.smear >= 0.02 && measures.smear_aspect >= 3.0 {
        reasons.push(format!(
            "{:.0}% of the asset is a flat dark band (smeared hidden overlap?)",
            measures.smear * 100.0
        ));
    }
    if measures.opaque == 0.0 {
        reasons.push("no opaque pixels".into());
    }
    reasons
}

/// Missing artwork cannot be cleaned back: a cut means a new, complete drawing.
fn assess_suggestion(
    measures: &ImageMeasures,
    touching: f64,
    cuts: [u32; 4],
    reasons: &[String],
) -> &'static str {
    if measures.opaque == 0.0 || touching > 0.35 || cuts.iter().any(|&run| run > 0) {
        "recreate"
    } else if reasons.is_empty() {
        "ok"
    } else {
        "clean"
    }
}

fn assess_next(suggestion: &str) -> String {
    match suggestion {
        "recreate" => "Record it: assets.fixlist_add asset=<id> path=<png> decision=recreate problem=<what is wrong>".to_string(),
        "clean" => "Record it: assets.fixlist_add asset=<id> path=<png> decision=clean problem=<what> tool=<exact command that fixes it>".to_string(),
        _ => "Looks fine by measure; still check it at full size before using it.".to_string(),
    }
}

/// Measures one image layer and suggests clean vs recreate with the evidence.
pub fn cmd_assets_assess(s: &mut Session, params: &Value) -> Result<Value> {
    let path = params
        .get("path")
        .and_then(Value::as_str)
        .map(|p| resolve_workspace_path(s, p))
        .ok_or_else(|| EngineError::InvalidParams("missing 'path'".into()))?;
    let img = image::open(&path)?.to_rgba8();
    let measures = measure_image(&img);
    let joints = parse_joints(params)?;
    let cuts = apply_joints(measures.cuts, &joints);
    // A full-bleed layer (a page, a background) fills its frame on purpose.
    let touching = if measures.full_bleed {
        0.0
    } else {
        measures.touching
    };
    let reasons = assess_reasons(&measures, touching, cuts);
    let suggestion = assess_suggestion(&measures, touching, cuts, &reasons);
    let (w, h) = measures.size;
    Ok(
        json!({"ok": true, "path": path, "size": [w, h], "suggestion": suggestion, "reasons": reasons,
        "measures": {"clipped_border": touching, "full_bleed": measures.full_bleed, "straight_cut_px": {"left": measures.cuts[0], "right": measures.cuts[1], "top": measures.cuts[2], "bottom": measures.cuts[3]}, "raggedness": measures.raggedness, "smear_band": measures.smear, "smear_aspect": measures.smear_aspect, "soft_edge_pixels": measures.partial / measures.opaque.max(1.0)},
        "next": [assess_next(suggestion)]}),
    )
}

/// Straight cuts along each side of the artwork (left, right, top, bottom), in px; 0 = none.
///
/// Artwork cropped through ends in a straight run of solid pixels with nothing solid beyond
/// them, and the outline pulls away from that line past both ends of the run. A complete
/// drawing can also have a long, nearly straight edge (a wing, a chin, a hull), but its outline
/// keeps running along the line, so a run counts as a cut only when, within 7 px past its ends,
/// the outline recedes at least 3 px at one end and 2 px at the other (measured on cropped
/// cast layers and complete parts). Runs are looked for on the four lines just inside the bounds,
/// because a soft fringe can sit beyond a cut. A side lying on the canvas edge and solid for 90%
/// of the frame is the edge of a full-bleed layer (a page, a background), not a cut; such a layer
/// also continues past the frame on its other sides that touch the canvas, so those are not cuts
/// either. A long edge inside the canvas is never full-bleed evidence.
fn point_at(vertical: bool, across: i64, along: i64) -> (i64, i64) {
    if vertical {
        (across, along)
    } else {
        (along, across)
    }
}

fn solid(img: &image::RgbaImage, (w, h): (i64, i64), x: i64, y: i64) -> bool {
    x >= 0 && y >= 0 && x < w && y < h && img.get_pixel(x as u32, y as u32)[3] > 128
}

/// Bounding box of the opaque pixels; None when nothing is opaque.
fn solid_bounds(img: &image::RgbaImage) -> Option<(i64, i64, i64, i64)> {
    let (mut x0, mut y0, mut x1, mut y1) = (i64::MAX, i64::MAX, -1, -1);
    for (x, y, p) in img.enumerate_pixels() {
        if p[3] > 128 {
            let (x, y) = (i64::from(x), i64::from(y));
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
    }
    (x1 >= 0).then_some((x0, y0, x1, y1))
}

/// One side of the artwork: the four lines just inside its bounds, the outward step, the span
/// along the side, and whether the side is vertical. Points are (across, along).
struct CutSide {
    lines: Vec<i64>,
    outward: i64,
    span: (i64, i64),
    vertical: bool,
}

fn cut_sides((x0, y0, x1, y1): (i64, i64, i64, i64)) -> [CutSide; 4] {
    [
        CutSide {
            lines: (x0..=(x0 + 3).min(x1)).collect(),
            outward: -1,
            span: (y0, y1),
            vertical: true,
        },
        CutSide {
            lines: ((x1 - 3).max(x0)..=x1).rev().collect(),
            outward: 1,
            span: (y0, y1),
            vertical: true,
        },
        CutSide {
            lines: (y0..=(y0 + 3).min(y1)).collect(),
            outward: -1,
            span: (x0, x1),
            vertical: false,
        },
        CutSide {
            lines: ((y1 - 3).max(y0)..=y1).rev().collect(),
            outward: 1,
            span: (x0, x1),
            vertical: false,
        },
    ]
}

/// How far the outline sits inward of `line` at `along` (None: nothing solid within 12 px).
fn recede(
    img: &image::RgbaImage,
    (w, h): (i64, i64),
    side: &CutSide,
    line: i64,
    along: i64,
) -> Option<i64> {
    (0..=12).find(|d| {
        let (x, y) = point_at(side.vertical, line - side.outward * d, along);
        solid(img, (w, h), x, y)
    })
}

/// Furthest the outline pulls away within 7 px past an end (missing evidence counts as 13).
fn pull_away(
    img: &image::RgbaImage,
    size: (i64, i64),
    side: &CutSide,
    line: i64,
    end: i64,
    step: i64,
) -> i64 {
    (1..=7)
        .map(|k| recede(img, size, side, line, end + step * k).unwrap_or(13))
        .max()
        .unwrap_or(0)
}

/// A completed run counts as a cut when it is long enough and the outline pulls away past both ends.
fn run_is_cut(
    img: &image::RgbaImage,
    size: (i64, i64),
    side: &CutSide,
    line: i64,
    start: i64,
    end: i64,
    length: f64,
) -> bool {
    let before = pull_away(img, size, side, line, start, -1);
    let after = pull_away(img, size, side, line, end - 1, 1);
    let pulls_away = (before >= 3 && after >= 2) || (before >= 2 && after >= 3);
    let run = (end - start) as u32;
    run > 16 && f64::from(run) > length * 0.02 && pulls_away
}

/// Longest qualifying run of solid edge along one line, and whether the whole frame ran solid.
fn line_runs(
    img: &image::RgbaImage,
    size: (i64, i64),
    side: &CutSide,
    line: i64,
    length: f64,
    frame: f64,
    on_edge: bool,
) -> (u32, bool) {
    let frame_line = on_edge && line == side.lines[0];
    let (mut best, mut bleed) = (0u32, false);
    let mut start: Option<i64> = None;
    for along in side.span.0..=side.span.1 + 1 {
        let (x, y) = point_at(side.vertical, line, along);
        let (ox, oy) = point_at(side.vertical, line + side.outward, along);
        let edge =
            along <= side.span.1 && solid(img, size, x, y) && !solid(img, size, ox, oy);
        match (edge, start) {
            (true, None) => start = Some(along),
            (false, Some(s)) => {
                let run = (along - s) as u32;
                if frame_line && f64::from(run) >= frame * 0.9 {
                    bleed = true;
                } else if run_is_cut(img, size, side, line, s, along, length) {
                    best = best.max(run);
                }
                start = None;
            }
            _ => {}
        }
    }
    (best, bleed)
}

fn straight_cuts(img: &image::RgbaImage) -> ([u32; 4], bool) {
    let size = (i64::from(img.width()), i64::from(img.height()));
    let (w, h) = size;
    let Some(bounds) = solid_bounds(img) else {
        return ([0; 4], false);
    };
    let (x0, y0, x1, y1) = bounds;
    let sides = cut_sides(bounds);
    // A full-bleed side has to be the frame itself: on the canvas edge, solid for 90% of it.
    let on_canvas_edge = [x0 == 0, x1 == w - 1, y0 == 0, y1 == h - 1];
    let mut runs = [0u32; 4];
    let mut bleed = [false; 4];
    for (i, side) in sides.iter().enumerate() {
        let length = (side.span.1 - side.span.0 + 1) as f64;
        let frame = if side.vertical { h } else { w } as f64;
        for &line in &side.lines {
            let (run, whole_frame) =
                line_runs(img, size, side, line, length, frame, on_canvas_edge[i]);
            runs[i] = runs[i].max(run);
            bleed[i] |= whole_frame;
        }
    }
    let full_bleed = bleed.iter().any(|&b| b);
    if full_bleed {
        for i in 0..4 {
            if bleed[i] || on_canvas_edge[i] {
                runs[i] = 0;
            }
        }
    }
    (runs, full_bleed)
}

fn band_luminance(img: &image::RgbaImage) -> Vec<f64> {
    img.pixels()
        .map(|p| 0.299 * f64::from(p[0]) + 0.587 * f64::from(p[1]) + 0.114 * f64::from(p[2]))
        .collect()
}

fn visible_pixels(img: &image::RgbaImage) -> Vec<bool> {
    img.pixels().map(|p| p[3] > 16).collect()
}

/// Standard deviation of brightness in a 5x5 window: blurred paint is flat, texture is not.
fn local_spread(lum: &[f64], wu: usize, hu: usize, x: usize, y: usize) -> f64 {
    let (mut n, mut sum, mut sq) = (0.0, 0.0, 0.0);
    for yy in y.saturating_sub(2)..(y + 3).min(hu) {
        for xx in x.saturating_sub(2)..(x + 3).min(wu) {
            let l = lum[yy * wu + xx];
            n += 1.0;
            sum += l;
            sq += l * l;
        }
    }
    (sq / n - (sum / n) * (sum / n)).max(0.0).sqrt()
}

/// Dark, blurred and flat pixels: candidates for a painted strip rather than material texture.
fn flat_mask(lum: &[f64], visible: &[bool], wu: usize, hu: usize, median: f64) -> Vec<bool> {
    (0..lum.len())
        .map(|i| {
            visible[i] && lum[i] < median * 0.45 && local_spread(lum, wu, hu, i % wu, i / wu) < 4.0
        })
        .collect()
}

/// Queue a pixel's unvisited up/down/left/right neighbours.
fn push_neighbors(flat: &mut [bool], stack: &mut Vec<usize>, i: usize, wu: usize, hu: usize) {
    let (x, y) = (i % wu, i / wu);
    let mut push = |j: usize| {
        if flat[j] {
            flat[j] = false;
            stack.push(j);
        }
    };
    if x > 0 {
        push(i - 1);
    }
    if x + 1 < wu {
        push(i + 1);
    }
    if y > 0 {
        push(i - wu);
    }
    if y + 1 < hu {
        push(i + wu);
    }
}

/// Flood one connected region: its size and the bounding box [x0, y0, x1, y1].
fn flood_region(flat: &mut [bool], start: usize, wu: usize, hu: usize) -> (usize, [usize; 4]) {
    let mut stack = vec![start];
    let mut size = 0usize;
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
    while let Some(i) = stack.pop() {
        let (x, y) = (i % wu, i / wu);
        size += 1;
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        push_neighbors(flat, &mut stack, i, wu, hu);
    }
    (size, [x0, y0, x1, y1])
}

/// Four-way flood fill over the mask; returns the largest region's size and box aspect.
fn largest_region(mut flat: Vec<bool>, wu: usize, hu: usize) -> (usize, f64) {
    let (mut best, mut best_aspect) = (0usize, 0.0);
    for start in 0..flat.len() {
        if !flat[start] {
            continue;
        }
        flat[start] = false;
        let (size, [x0, y0, x1, y1]) = flood_region(&mut flat, start, wu, hu);
        if size > best {
            let (bw, bh) = ((x1 - x0 + 1) as f64, (y1 - y0 + 1) as f64);
            (best, best_aspect) = (size, bw.max(bh) / bw.min(bh));
        }
    }
    (best, best_aspect)
}

/// Largest flat dark band: (share of visible pixels, long/short side of its box).
/// A painted hidden-overlap strip is dark, blurred (5x5 brightness spread < 4) and elongated;
/// dark stripes keep the material's texture and pupils are round, so neither counts.
fn smear_band(img: &image::RgbaImage, median: f64) -> (f64, f64) {
    let (w, h) = img.dimensions();
    let (wu, hu) = (w as usize, h as usize);
    let lum = band_luminance(img);
    let visible = visible_pixels(img);
    let flat = flat_mask(&lum, &visible, wu, hu, median);
    let (best, best_aspect) = largest_region(flat, wu, hu);
    let total = visible.iter().filter(|v| **v).count().max(1);
    (best as f64 / total as f64, best_aspect)
}

pub fn cmd_assets_fixlist(s: &mut Session, _: &Value) -> Result<Value> {
    let items = read_fixlist(s)?;
    Ok(json!({"ok": true, "file": fixlist_path(s)?, "assets": items, "next": fixlist_next(&items)}))
}

fn required_asset(params: &Value) -> Result<&str> {
    params
        .get("asset")
        .and_then(Value::as_str)
        .filter(|a| !a.trim().is_empty())
        .ok_or_else(|| EngineError::InvalidParams("missing 'asset' (a short id)".into()))
}

/// Only the listed fields are copied from the request; anything else stays as stored.
fn merge_item_fields(item: &mut Value, params: &Value) {
    for key in ["path", "problem", "tool", "evidence", "reference", "status"] {
        if let Some(v) = params.get(key) {
            item[key] = v.clone();
        }
    }
}

fn apply_decision(item: &mut Value, params: &Value) -> Result<()> {
    let Some(d) = params.get("decision").and_then(Value::as_str) else {
        return Ok(());
    };
    if !["clean", "recreate"].contains(&d) {
        return Err(EngineError::InvalidParams(
            "decision must be clean or recreate".into(),
        ));
    }
    item["decision"] = json!(d);
    Ok(())
}

/// An entry is only actionable with a decision, and "clean" needs the command that does it.
fn validate_item(item: &Value) -> Result<()> {
    if item.get("decision").is_none() {
        return Err(EngineError::InvalidParams(
            "new entries need decision=clean|recreate".into(),
        ));
    }
    if item["decision"] == json!("clean") && item.get("tool").is_none() {
        return Err(EngineError::InvalidParams(
            "decision=clean needs tool=<the exact command that fixes it>".into(),
        ));
    }
    Ok(())
}

fn write_fixlist(s: &Session, items: &[Value]) -> Result<()> {
    let text = serde_json::to_string_pretty(&json!({"assets": items}))
        .map_err(|e| EngineError::InvalidParams(e.to_string()))?;
    std::fs::write(fixlist_path(s)?, text)?;
    Ok(())
}

/// Adds or updates one asset on the fix list (keyed by `asset`).
pub fn cmd_assets_fixlist_add(s: &mut Session, params: &Value) -> Result<Value> {
    let asset = required_asset(params)?;
    let mut items = read_fixlist(s)?;
    let idx = items.iter().position(|i| i["asset"] == json!(asset));
    let mut item = idx
        .map(|i| items[i].clone())
        .unwrap_or_else(|| json!({"asset": asset, "status": "open"}));
    merge_item_fields(&mut item, params);
    apply_decision(&mut item, params)?;
    validate_item(&item)?;
    match idx {
        Some(i) => items[i] = item,
        None => items.push(item),
    }
    write_fixlist(s, &items)?;
    cmd_assets_fixlist(s, &json!({}))
}

/// Writes ONE asset-sheet brief for the image agent covering every open "recreate" item.
pub fn cmd_assets_request_sheet(s: &mut Session, _: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let items: Vec<Value> = read_fixlist(s)?
        .into_iter()
        .filter(|i| i["decision"] == json!("recreate") && i["status"] != json!("done"))
        .collect();
    if items.is_empty() {
        return Err(EngineError::InvalidParams(
            "no open 'recreate' items on the fix list".into(),
        ));
    }
    let studio = studio(&dir);
    let mut brief = format!(
        "# Asset sheet request — {} item(s), ONE pass\n\n",
        items.len()
    );
    if let Some(intent) = studio.intent {
        brief.push_str(&format!("Studio intent:\n{intent}\n\n"));
    }
    brief.push_str("Make ONE asset sheet image containing every asset below, each separated on a plain background with space around it, so each can be cut out cleanly (Magic Studio cuts them with PhotoCraft). Match each asset's reference exactly in character, colour and material. Keep every part whole: nothing touching the sheet edge or another asset. No text on the sheet.\n\n## Assets\n");
    for (n, i) in items.iter().enumerate() {
        brief.push_str(&format!(
            "{}. **{}** — problem: {}. Current file: {}. Reference: {}\n",
            n + 1,
            i["asset"].as_str().unwrap_or_default(),
            i["problem"].as_str().unwrap_or("(not given)"),
            i["path"].as_str().unwrap_or("(none)"),
            i["reference"].as_str().unwrap_or("match the current file")
        ));
    }
    brief.push_str("\nSave the sheet as `assets/asset-sheet-<date>.png` with its exact prompt as `.prompt.md` beside it, then stop and report.\n");
    let out = dir.join("briefs");
    std::fs::create_dir_all(&out)?;
    let path = out.join("ASSET-SHEET-request.md");
    std::fs::write(&path, &brief)?;
    Ok(
        json!({"ok": true, "brief": path, "items": items.len(), "next": [
        format!("Send the image agent: follow {} exactly", path.display()),
        "When the sheet exists: cut each asset out (PhotoCraft via magic-cli), check at full size, assets.fixlist_add asset=<id> status=done".to_string()]}),
    )
}
