//! Turn reviewed action-sheet frames into tracks for the existing motion engine and exporter.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};

fn invalid(message: &str) -> EngineError {
    EngineError::InvalidParams(message.into())
}

pub fn prepare(s: &mut Session, p: &Value) -> Result<Value> {
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
    let frames = data["frames"]
        .as_array()
        .filter(|f| !f.is_empty())
        .ok_or_else(|| invalid("No extracted frames"))?;
    let directory = manifest
        .parent()
        .ok_or_else(|| invalid("Manifest has no directory"))?;
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
        if !path.starts_with(&root) {
            return Err(invalid("Frame file is outside the project"));
        }
        let size = image::image_dimensions(&path)?;
        if canvas.is_some_and(|expected| expected != size) {
            return Err(invalid("All playback frames need the same canvas"));
        }
        canvas = Some(size);
        layers.push(json!({"id":format!("frame_{index:02}"),"file":path,"pivot":[size.0 as f64/2.0,size.1 as f64/2.0]}));
    }
    let rows=data["animation"]["rows"].as_array().cloned().filter(|r|!r.is_empty()).unwrap_or_else(||vec![json!({
        "id":data["animation"]["template"].as_str().unwrap_or("action"),"frames":(0..frames.len()).collect::<Vec<_>>(),
        "fps":data["animation"]["fps"].as_u64().unwrap_or(12),"loop":data["animation"]["loop"].as_bool().unwrap_or(false)})]);
    let mut tracks = Vec::new();
    for row in &rows {
        let track = action_track(row, frames, false)?;
        tracks.push(track);
        if row["mirror_variant"].as_str().is_some() {
            tracks.push(action_track(row, frames, true)?);
        }
    }
    let mut demo_motion = json!({});
    for layer in &layers {
        demo_motion[layer["id"].as_str().unwrap_or_default()] =
            json!({"opacity":[{"kind":"keys","ease":"hold","keys":[[0,-100]]}]});
    }
    let mut cursor = 0.0;
    for track in &tracks {
        let duration = track["duration"].as_f64().unwrap_or(1.0);
        let repetitions = if track["loop"] == true { 2 } else { 1 };
        for _ in 0..repetitions {
            for (id, props) in track["motion"]
                .as_object()
                .ok_or_else(|| invalid("Missing track motion"))?
            {
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
                    demo_motion[id]["scale"] =
                        json!([{"kind":"keys","ease":"hold","keys":[[0,[0,0]]]}]);
                }
                let scale = props["scale"][0]["keys"][0][1]
                    .as_array()
                    .map(|_| props["scale"][0]["keys"][0][1].clone())
                    .unwrap_or(json!([0, 0]));
                if let Some(keys) = demo_motion[id]["scale"][0]["keys"].as_array_mut() {
                    keys.push(json!([cursor, scale]));
                }
            }
            cursor += duration;
        }
    }
    if let Some(id) = tracks.last().and_then(|t| t["layers"][0].as_str())
        && let Some(keys) = demo_motion[id]["opacity"][0]["keys"].as_array_mut()
    {
        keys.push(json!([cursor, 0]));
    }
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
                let mut unique: Vec<Value> = Vec::new();
                for key in keys.drain(..) {
                    if unique.last().is_some_and(|last| last[0].as_f64() == key[0].as_f64()) {
                        unique.pop();
                    }
                    unique.push(key);
                }
                *keys = unique;
            }
        }
    }
    tracks.push(json!({"id":"demonstration","label":"Classic action reel","duration":cursor,"loop":false,"motion":demo_motion,"sprite_action":true}));
    let (w, h) = canvas.ok_or_else(|| invalid("No frame canvas"))?;
    let id = p["id"].as_str().unwrap_or("action-frames");
    let id = crate::workspace::sanitize_asset_id(id);
    let destination = directory.join(format!("{id}.motion.json"));
    if destination.exists() {
        return Err(invalid("Playback already exists; choose another id"));
    }
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

fn action_track(row: &Value, frames: &[Value], mirrored: bool) -> Result<Value> {
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
    let looping = row["loop"].as_bool().unwrap_or(false);
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
    let mut motion = json!({});
    let mut layer_ids = Vec::new();
    for (position, (index, start, end)) in spans.iter().enumerate() {
        let layer = format!("frame_{index:02}");
        layer_ids.push(layer.clone());
        let mut keys = vec![json!([0, if *start == 0.0 { 0 } else { -100 }])];
        if *start > 0.0 {
            keys.push(json!([start, 0]));
        }
        if position + 1 < spans.len() || looping {
            keys.push(json!([end, -100]));
        }
        if looping && position == 0 {
            keys.push(json!([duration, 0]));
        }
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
