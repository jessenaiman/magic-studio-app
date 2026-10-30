//! Turn reviewed action-sheet frames into tracks for the existing motion engine and exporter.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};

fn invalid(message: &str) -> EngineError {
    EngineError::InvalidParams(message.into())
}

/// The manifest this call names, proven to live inside the project.
fn playback_source(s: &Session, p: &Value) -> Result<(std::path::PathBuf, std::path::PathBuf, Value)> {
    let manifest = crate::workspace::resolve_workspace_path(
        s,
        p["manifest"]
            .as_str()
            .ok_or_else(|| invalid("manifest is required"))?,
    );
    let root = crate::workshop::project_dir(s)?.canonicalize()?;
    let manifest = manifest.canonicalize()?;
    if !manifest.starts_with(&root) {
        return Err(invalid("The frame manifest must belong to this project"));
    }
    let data: Value =
        serde_json::from_slice(&std::fs::read(&manifest)?).map_err(|e| invalid(&e.to_string()))?;
    Ok((root, manifest, data))
}

/// One layer per frame, every frame inside the project and sharing one canvas.
fn frame_layers(
    root: &std::path::Path,
    directory: &std::path::Path,
    frames: &[Value],
) -> Result<(Vec<Value>, (u32, u32))> {
    let mut layers = Vec::new();
    let mut canvas = None;
    for (index, frame) in frames.iter().enumerate() {
        let path = directory
            .join(
                frame["file"]
                    .as_str()
                    .ok_or_else(|| invalid("Frame file missing"))?,
            )
            .canonicalize()?;
        if !path.starts_with(root) {
            return Err(invalid("Frame file is outside the project"));
        }
        let size = image::image_dimensions(&path)?;
        if canvas.is_some_and(|expected| expected != size) {
            return Err(invalid("All playback frames need the same canvas"));
        }
        canvas = Some(size);
        layers.push(json!({"id":format!("frame_{index:02}"),"file":path,"pivot":[size.0 as f64/2.0,size.1 as f64/2.0]}));
    }
    let canvas = canvas.ok_or_else(|| invalid("No frame canvas"))?;
    Ok((layers, canvas))
}

/// The rows the manifest names, else one row over every frame using the template settings.
fn playback_rows(data: &Value, frame_count: usize) -> Vec<Value> {
    data["animation"]["rows"]
        .as_array()
        .cloned()
        .filter(|r| !r.is_empty())
        .unwrap_or_else(|| {
            vec![json!({
                "id":data["animation"]["template"].as_str().unwrap_or("action"),
                "frames":(0..frame_count).collect::<Vec<_>>(),
                "fps":data["animation"]["fps"].as_u64().unwrap_or(12),
                "loop":data["animation"]["loop"].as_bool().unwrap_or(false)})]
        })
}

/// A track per row, plus its mirror variant when the row names one.
fn build_tracks(rows: &[Value], frames: &[Value]) -> Result<Vec<Value>> {
    let mut tracks = Vec::new();
    for row in rows {
        tracks.push(action_track(row, frames, false)?);
        if row["mirror_variant"].as_str().is_some() {
            tracks.push(action_track(row, frames, true)?);
        }
    }
    Ok(tracks)
}

/// The demonstration motion starts hidden on every layer.
fn demo_motion(layers: &[Value]) -> Value {
    let mut demo_motion = json!({});
    for layer in layers {
        demo_motion[layer["id"].as_str().unwrap_or_default()] =
            json!({"opacity":[{"kind":"keys","ease":"hold","keys":[[0,-100]]}]});
    }
    demo_motion
}

/// Lay every track's keys end to end, repeating looping tracks twice; returns the total cursor.
fn append_demo_tracks(demo_motion: &mut Value, tracks: &[Value]) -> Result<f64> {
    let mut cursor = 0.0;
    for track in tracks {
        let duration = track["duration"].as_f64().unwrap_or(1.0);
        let repetitions = if track["loop"] == true { 2 } else { 1 };
        for _ in 0..repetitions {
            for (id, props) in track["motion"]
                .as_object()
                .ok_or_else(|| invalid("Missing track motion"))?
            {
                append_demo_keys(demo_motion, id, props, cursor, duration)?;
            }
            cursor += duration;
        }
    }
    Ok(cursor)
}

fn append_demo_keys(
    demo_motion: &mut Value,
    id: &str,
    props: &Value,
    cursor: f64,
    duration: f64,
) -> Result<()> {
    let target = demo_motion[id]["opacity"][0]["keys"]
        .as_array_mut()
        .ok_or_else(|| invalid("Missing demo keys"))?;
    for key in props["opacity"][0]["keys"]
        .as_array()
        .ok_or_else(|| invalid("Missing frame holds"))?
    {
        target.push(json!([cursor + key[0].as_f64().unwrap_or(0.0), key[1]]));
    }
    target.push(json!([cursor + duration, -100]));
    if demo_motion[id].get("scale").is_none() {
        demo_motion[id]["scale"] = json!([{"kind":"keys","ease":"hold","keys":[[0,[0,0]]]}]);
    }
    let scale = props["scale"][0]["keys"][0][1]
        .as_array()
        .map(|_| props["scale"][0]["keys"][0][1].clone())
        .unwrap_or(json!([0, 0]));
    if let Some(keys) = demo_motion[id]["scale"][0]["keys"].as_array_mut() {
        keys.push(json!([cursor, scale]));
    }
    Ok(())
}

/// Hide the last layer once the reel has played out.
fn close_demo(demo_motion: &mut Value, tracks: &[Value], cursor: f64) {
    if let Some(id) = tracks.last().and_then(|t| t["layers"][0].as_str())
        && let Some(keys) = demo_motion[id]["opacity"][0]["keys"].as_array_mut()
    {
        keys.push(json!([cursor, 0]));
    }
}

/// Two keys at the same time cannot both stand: keep the later one.
fn dedupe_keys(keys: &mut Vec<Value>) {
    let mut unique: Vec<Value> = Vec::new();
    for key in keys.drain(..) {
        if unique.last().is_some_and(|last| last[0].as_f64() == key[0].as_f64()) {
            unique.pop();
        }
        unique.push(key);
    }
    *keys = unique;
}

fn dedupe_demo_keys(demo_motion: &mut Value) -> Result<()> {
    for props in demo_motion
        .as_object_mut()
        .ok_or_else(|| invalid("No demonstration motion"))?
        .values_mut()
    {
        for terms in props
            .as_object_mut()
            .ok_or_else(|| invalid("Invalid demonstration channels"))?
            .values_mut()
        {
            if let Some(keys) = terms[0]["keys"].as_array_mut() {
                dedupe_keys(keys);
            }
        }
    }
    Ok(())
}

/// The playback id and the spec path it writes, which must not already exist.
fn playback_destination(
    directory: &std::path::Path,
    p: &Value,
) -> Result<(String, std::path::PathBuf)> {
    let id = p["id"].as_str().unwrap_or("action-frames");
    let id = crate::workspace::sanitize_asset_id(id);
    let destination = directory.join(format!("{id}.motion.json"));
    if destination.exists() {
        return Err(invalid("Playback already exists; choose another id"));
    }
    Ok((id, destination))
}

pub fn prepare(s: &mut Session, p: &Value) -> Result<Value> {
    let (root, manifest, data) = playback_source(s, p)?;
    let frames = data["frames"]
        .as_array()
        .filter(|f| !f.is_empty())
        .ok_or_else(|| invalid("No extracted frames"))?;
    let directory = manifest
        .parent()
        .ok_or_else(|| invalid("Manifest has no directory"))?;
    let (layers, (w, h)) = frame_layers(&root, directory, frames)?;
    let rows = playback_rows(&data, frames.len());
    let tracks = build_tracks(&rows, frames)?;
    let mut demo_motion = demo_motion(&layers);
    let cursor = append_demo_tracks(&mut demo_motion, &tracks)?;
    close_demo(&mut demo_motion, &tracks, cursor);
    dedupe_demo_keys(&mut demo_motion)?;
    let mut tracks = tracks;
    tracks.push(json!({"id":"demonstration","label":"Classic action reel","duration":cursor,"loop":false,"motion":demo_motion,"sprite_action":true}));
    let (id, destination) = playback_destination(directory, p)?;
    let spec = json!({"canvas":[w,h],"fps":24,"reference":layers[0]["file"],"source_image":layers[0]["file"],
        "frame_sequence":{"manifest":manifest,"rows":rows},"layers":layers,"tracks":tracks});
    std::fs::write(
        &destination,
        serde_json::to_vec_pretty(&spec).map_err(|e| invalid(&e.to_string()))?,
    )?;
    let attached = crate::project::cmd_project_add_asset(
        s,
        &json!({"id":id,"name":p["name"].as_str().unwrap_or("Action frames"),"kind":"cutout_rig","path":destination,"load":true}),
    )?;
    Ok(
        json!({"ok":true,"asset_id":attached["asset_id"],"spec":destination,"tracks":spec["tracks"].as_array().map(|ts|ts.iter().map(|t|json!({"id":t["id"],"label":t["label"],"duration":t["duration"]})).collect::<Vec<_>>())}),
    )
}

/// The row's id, frame indices, fps and loop flag.
fn row_timing(row: &Value) -> Result<(&str, &Vec<Value>, f64, bool)> {
    let id = row["id"]
        .as_str()
        .ok_or_else(|| invalid("Action row needs an id"))?;
    let indices = row["frames"]
        .as_array()
        .filter(|a| !a.is_empty())
        .ok_or_else(|| invalid("Action row needs frame indices"))?;
    let fps = row["fps"]
        .as_f64()
        .filter(|f| *f > 0.0 && *f <= 60.0)
        .ok_or_else(|| invalid("Action FPS must be 1..60"))?;
    Ok((id, indices, fps, row["loop"].as_bool().unwrap_or(false)))
}

/// Each frame's hold, as (index, start, end), and the total duration before the tail pause.
fn frame_spans(
    indices: &[Value],
    frames: &[Value],
    fps: f64,
    looping: bool,
) -> Result<(Vec<(usize, f64, f64)>, f64)> {
    let mut spans = Vec::new();
    let mut duration = 0.0;
    for index in indices {
        let index = index
            .as_u64()
            .filter(|i| (*i as usize) < frames.len())
            .ok_or_else(|| invalid("Frame index outside manifest"))? as usize;
        let hold = frames[index]["holdFrames"].as_u64().unwrap_or(1).max(1) as f64 / fps;
        spans.push((index, duration, duration + hold));
        duration += hold;
    }
    if !looping {
        duration += 0.75;
    }
    Ok((spans, duration))
}

/// Opacity keys that show this frame from `start` to `end` and nothing after it.
fn opacity_keys(start: f64, end: f64, duration: f64, position: usize, last: bool, looping: bool) -> Vec<Value> {
    let mut keys = vec![json!([0, if start == 0.0 { 0 } else { -100 }])];
    if start > 0.0 {
        keys.push(json!([start, 0]));
    }
    if !last || looping {
        keys.push(json!([end, -100]));
    }
    if looping && position == 0 {
        keys.push(json!([duration, 0]));
    }
    keys
}

fn action_track(row: &Value, frames: &[Value], mirrored: bool) -> Result<Value> {
    let (id, indices, fps, looping) = row_timing(row)?;
    let (spans, duration) = frame_spans(indices, frames, fps, looping)?;
    let mut motion = json!({});
    let mut layer_ids = Vec::new();
    let last = spans.len();
    for (position, (index, start, end)) in spans.iter().enumerate() {
        let layer = format!("frame_{index:02}");
        layer_ids.push(layer.clone());
        let keys = opacity_keys(*start, *end, duration, position, position + 1 == last, looping);
        motion[&layer] = json!({"opacity":[{"kind":"keys","ease":"hold","keys":keys}]});
        if mirrored {
            motion[&layer]["scale"] = json!([{"kind":"keys","ease":"hold","keys":[[0,[-200,0]]]}]);
        }
    }
    let track_id = if mirrored {
        row["mirror_variant"].as_str().unwrap_or("walk_left")
    } else {
        id
    };
    Ok(
        json!({"id":track_id,"label":track_id.replace('_'," "),"duration":duration,"loop":looping,"layers":layer_ids,"motion":motion,"sprite_action":true}),
    )
}
