//! The film as FilmCraft holds it: `film/<song>.fcproj` (built by `video.assemble`), a fast
//! half-size preview to watch in Studio, single program frames, the final export, and opening the
//! timeline in FilmCraft for hand edits. All time is measured in [`Tick`] (`254_016_000_000` ticks/s).

pub mod assembly;

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::time::{Tick, parse_raw_tick, parse_tick};
use crate::{EngineError, Result, Session};

fn bad(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

struct Film {
    dir: PathBuf,
    name: String,
    project: PathBuf,
}

fn film(s: &Session) -> Result<Film> {
    let dir = crate::workshop::project_dir(s)?;
    let name = s.project_path.as_ref().and_then(|p| p.file_stem()).map_or("Film".into(), |n| n.to_string_lossy().into_owned());
    let project = s
        .video_project_path
        .clone()
        .filter(|p| p.extension().is_some_and(|e| e == "fcproj"))
        .unwrap_or_else(|| dir.join("film").join(format!("{name}.fcproj")));
    Ok(Film { dir, name, project })
}

/// Record `project` as the song's one film in the .magic, so every step uses the same one.
pub(crate) fn link(s: &mut Session, project: &Path) -> Result<()> {
    if s.video_project_path.as_deref() == Some(project) {
        return Ok(());
    }
    s.video_project_path = Some(project.to_path_buf());
    crate::project::write_project_file(s)
}

impl Film {
    fn require(&self) -> Result<&Path> {
        if self.project.is_file() { Ok(&self.project) } else { Err(bad("No film timeline yet; run video.assemble first")) }
    }
    fn preview(&self) -> PathBuf {
        self.dir.join("film").join(format!("{}-preview.mp4", self.name))
    }
    fn export(&self) -> PathBuf {
        self.dir.join("exports").join(format!("{}.mp4", self.name))
    }
}

fn file_info(path: &Path) -> Value {
    let modified =
        std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as u64);
    json!({"path": path, "exists": path.is_file(), "modified": modified})
}

pub(crate) fn film_path(s: &Session) -> Result<PathBuf> {
    Ok(film(s)?.project)
}

pub(crate) fn timeline_file(s: &Session) -> Result<Value> {
    Ok(file_info(&film(s)?.project))
}

/// Run a JSONL batch of commands on `project` (or a fresh session when `fresh` is true or
/// `project` does not exist yet), optionally saving back to `project`.
pub(crate) fn run_steps(project: Option<&Path>, fresh: bool, save: bool, steps: &[Value]) -> Result<Vec<Value>> {
    crate::CraftHub::run_film_steps(project, fresh, save, steps)
}

/// Refresh the dedicated lettering layer while leaving the picture and audio cuts in place.
pub(crate) fn publish_lyrics(s: &Session, movie: &Path) -> Result<Value> {
    const TRACK: &str = "Kinetic Lyrics";
    let project = film_path(s)?;
    if let Some(live_result) = publish_lyrics_live(&project, movie, TRACK) {
        return live_result;
    }
    let _lock = crate::project_lock::for_project(&project);
    let mut run = |id: &str, params: Value| -> Result<Value> {
        let step = json!({"id": id, "params": params});
        let save = id != "sequence.inspect" && id != "media.status" && id != "file.save";
        if id == "file.save" {
            return Ok(json!({"saved": project}));
        }
        run_steps(Some(&project), false, save, &[step])?
            .into_iter()
            .next()
            .ok_or_else(|| EngineError::ExternalTool("filmcraft-cli".into(), format!("{id}: no result")))
    };
    publish_lyrics_with(&project, movie, TRACK, &mut run)
}

fn existing_lyric_slot(sequence: &Value, track_name: &str) -> Option<(u64, u64, u64)> {
    let track = sequence["video"].as_array()?.iter().find(|t| t["name"] == track_name)?;
    let track_id = track["id"].as_u64()?;
    let first = track["items"].as_array()?.first()?;
    let clip_id = first.get("clip").or_else(|| first.get("id")).and_then(Value::as_u64)?;
    let item_id = first.get("item").and_then(Value::as_u64)?;
    Some((track_id, clip_id, item_id))
}

fn same_lettering_source(source: &Path, movie: &Path) -> bool {
    if source == movie || matches!((source.canonicalize(), movie.canonicalize()), (Ok(a), Ok(b)) if a == b) {
        return true;
    }
    let parts: Vec<_> = movie.components().collect();
    let Some(media) = parts.iter().rposition(|part| part.as_os_str() == "media") else { return false };
    let suffix: PathBuf = parts[media..].iter().collect();
    parts.len() - media >= 4 && source.ends_with(suffix)
}

fn legacy_lyric_slot(sequence: &Value, movie: &Path, media: &Value) -> Result<Option<(u64, u64, u64)>> {
    let mut slots = Vec::new();
    for track in sequence["video"].as_array().into_iter().flatten() {
        for clip in track["items"].as_array().into_iter().flatten() {
            let Some((track_id, clip_id, item_id)) = track["id"].as_u64().zip(clip["clip"].as_u64()).zip(clip["item"].as_u64()).map(|((t, c), i)| (t, c, i))
            else {
                continue;
            };
            if clip["startFrame"].as_u64() != Some(0) || clip["endFrame"] != sequence["durationFrames"] {
                continue;
            }
            let owned =
                media.as_array().into_iter().flatten().any(|item| {
                    item["item"].as_u64() == Some(item_id) && item["path"].as_str().is_some_and(|path| same_lettering_source(Path::new(path), movie))
                });
            if owned {
                slots.push((track_id, clip_id, item_id));
            }
        }
    }
    match slots.as_slice() {
        [] => Ok(None),
        [slot] => Ok(Some(*slot)),
        _ => Err(bad("Multiple existing full-song lettering clips match this source; choose the dedicated Kinetic Lyrics track before publishing")),
    }
}

fn publish_lyrics_live(project: &Path, movie: &Path, track_name: &str) -> Option<Result<Value>> {
    let sequence = match crate::CraftHub::try_live(crate::CraftTarget::Film, project, "sequence.inspect", &json!({}), false)? {
        Ok(seq) => seq,
        Err(e) => return Some(Err(e)),
    };
    let mut run = |id: &str, params: Value| {
        crate::CraftHub::try_live(crate::CraftTarget::Film, project, id, &params, false)
            .unwrap_or_else(|| Err(EngineError::ExternalTool("filmcraft".into(), format!("lost live control connection during {id}"))))
    };
    Some(publish_lyrics_sequence(project, movie, track_name, &sequence, &mut run))
}

fn publish_lyrics_with(project: &Path, movie: &Path, track_name: &str, run: &mut impl FnMut(&str, Value) -> Result<Value>) -> Result<Value> {
    let sequence = run("sequence.inspect", json!({}))?;
    publish_lyrics_sequence(project, movie, track_name, &sequence, run)
}

fn publish_lyrics_sequence(
    project: &Path,
    movie: &Path,
    track_name: &str,
    sequence: &Value,
    run: &mut impl FnMut(&str, Value) -> Result<Value>,
) -> Result<Value> {
    let slot = match existing_lyric_slot(sequence, track_name) {
        Some(slot) => Some(slot),
        None => legacy_lyric_slot(sequence, movie, &run("media.status", json!({}))?)?,
    };
    if let Some((track, clip, item)) = slot {
        run("media.replaceFootage", json!({"item": item, "path": movie}))?;
        run("timeline.setTrack", json!({"track": track, "name": track_name, "enabled": true}))?;
        run("file.save", json!({}))?;
        return Ok(json!({"timeline": project, "track": track, "clip": clip, "item": item, "relinked": true}));
    }
    let duration =
        sequence["durationFrames"].as_u64().filter(|frames| *frames > 0).ok_or_else(|| bad("Open a FilmCraft sequence with picture or audio first"))?;
    let imported = run("file.import", json!({"paths":[movie]}))?;
    let item = imported["items"][0].as_u64().ok_or_else(|| bad("FilmCraft did not import the lyric render"))?;
    let old_track = sequence["video"].as_array().into_iter().flatten().find(|track| track["name"] == track_name);
    let track = old_track.and_then(|track| track["id"].as_u64()).map_or(json!("new"), |id| json!(id));
    let placed = run(
        "timeline.place",
        json!({"item":item,"track":track,"frame":0,
        "duration":{"frame":duration},"insert":false,"video":true,"audio":false}),
    )?;
    let clip = placed["clips"][0].as_u64().ok_or_else(|| bad("FilmCraft did not place the lyric render"))?;
    run("clip.fitToFrame", json!({"clips":[clip]}))?;
    let after = run("sequence.inspect", json!({}))?;
    let track = after["video"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|track| track["items"].as_array().into_iter().flatten().any(|row| row["clip"] == clip))
        .and_then(|track| track["id"].as_u64())
        .ok_or_else(|| bad("FilmCraft did not return the lyric track"))?;
    run("timeline.setTrack", json!({"track":track,"name":track_name,"enabled":true}))?;
    run("file.save", json!({}))?;
    Ok(json!({"timeline": project, "track":track, "clip":clip, "item": item, "relinked": false}))
}

pub(crate) fn execute(project: &Path, id: &str, params: Value) -> Result<Value> {
    crate::CraftHub::execute(crate::CraftTarget::Film, Some(project), id, &params)
}

pub(crate) fn run(project: &Path, fresh: bool, steps: &[Value]) -> Result<Vec<Value>> {
    run_steps(Some(project), fresh, true, steps)
}

pub(crate) fn export(project: &Path, output: &Path, draft: bool) -> Result<()> {
    export_with_options(project, output, draft, &json!({}))
}

fn export_params(output: &Path, draft: bool, options: &Value) -> Value {
    let mut params = json!({"path": output, "format": "h264", "wait": true});
    if draft {
        params["scale"] = json!(0.5);
        params["quality"] = json!(60);
    }
    if let (Some(dst), Some(src)) = (params.as_object_mut(), options.as_object()) {
        for key in [
            "preset",
            "settings",
            "format",
            "audioCodec",
            "flacLevel",
            "width",
            "height",
            "fps",
            "scale",
            "quality",
            "audio",
            "burnCaptions",
            "captionSidecar",
            "loudnessLufs",
            "range",
            "startSeconds",
            "endSeconds",
            "hardwareEncoding",
            "gpuRendering",
            "proresProfile",
            "dnxProfile",
            "apvProfile",
            "h264Profile",
            "h264IntraOnly",
            "mxfVideoCodec",
            "bitrateKbps",
            "bitrateMode",
            "crf",
            "alpha",
            "exrCompression",
            "sequence",
        ] {
            if let Some(val) = src.get(key).filter(|v| !v.is_null() || matches!(key, "captionSidecar" | "preset")) {
                dst.insert(key.into(), if key == "captionSidecar" && val.is_null() { json!("none") } else { val.clone() });
            }
        }
    }
    params
}

pub(crate) fn export_with_options(project: &Path, output: &Path, draft: bool, options: &Value) -> Result<()> {
    let params = export_params(output, draft, options);
    let queue = options.get("queue").and_then(Value::as_bool) == Some(true);
    let cmd_id = if queue { "export.queue.add" } else { "file.exportMedia" };
    let result = crate::CraftHub::execute(crate::CraftTarget::Film, Some(project), cmd_id, &params)?;
    if !draft
        && options.get("allowOffline").and_then(Value::as_bool) != Some(true)
        && let Some(offline) = result["offlineMedia"].as_array().filter(|items| !items.is_empty())
    {
        return Err(EngineError::ExternalTool("filmcraft".into(), format!("export blocked by offline media: {offline:?}")));
    }
    Ok(())
}

pub(crate) fn frame_at_tick(project: &Path, tick: Tick, output: &Path) -> Result<()> {
    crate::CraftHub::render_film_frame(project, tick, 1.0, output)
}

pub(crate) fn probe_media(filename: &Path) -> Result<Value> {
    crate::CraftHub::probe_film_media(filename)
}

pub(crate) fn media_duration_ticks(filename: &Path, require_video: bool) -> Result<Tick> {
    let info = probe_media(filename)?;
    let is_movie = info["kind"].as_str() == Some("Movie") || info["video"].is_object();
    let has_audio = info["audio_streams"].as_array().is_some_and(|a| !a.is_empty());
    if require_video && !is_movie {
        return Err(EngineError::InvalidParams(format!("{} has no video stream", filename.display())));
    }
    if !require_video && !has_audio && !is_movie {
        return Err(EngineError::InvalidParams(format!("{} has no audio stream", filename.display())));
    }
    let duration = parse_raw_tick(&info["duration"]).unwrap_or(Tick::ZERO);
    if duration.0 <= 0 {
        return Err(EngineError::InvalidParams(format!("{} has invalid duration {} ticks", filename.display(), duration.0)));
    }
    Ok(duration)
}

#[allow(dead_code)]
pub(crate) fn media_duration(filename: &Path, require_video: bool) -> Result<f64> {
    media_duration_ticks(filename, require_video).map(|t| t.seconds())
}

fn export_with(f: &Film, out: &Path, draft: bool, options: &Value) -> Result<()> {
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    export_with_options(f.require()?, out, draft, options)?;
    Ok(())
}

fn export_destination(f: &Film, p: &Value) -> (PathBuf, Value) {
    let mut options = p.clone();
    let format = p["format"]
        .as_str()
        .or_else(|| p["settings"]["format"].as_str())
        .map(str::to_ascii_lowercase)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "h264".into());
    let ext = match format.as_str() {
        "prores" | "dnxhr" | "apv" | "mjpeg" => "mov",
        "mxf-op1a" | "mxf-opatom" => "mxf",
        "png" => "png",
        "tiff" => "tiff",
        "bmp" => "bmp",
        "gif" => "gif",
        "wav" => "wav",
        "aiff" => "aiff",
        "flac" => "flac",
        "aac" => "aac",
        "mp3" => "mp3",
        "webm" => "webm",
        _ => "mp4",
    };
    let out = p["path"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|raw| {
            let path = PathBuf::from(raw);
            if path.is_absolute() { path } else { f.dir.join(path) }
        })
        .unwrap_or_else(|| if ext == "mp4" { f.export() } else { f.dir.join("exports").join(format!("{}.{ext}", f.name)) });
    if let Some(obj) = options.as_object_mut() {
        obj.remove("path");
    }
    (out, options)
}

fn inspect_export_status(f: &Film, out: &Path) -> Value {
    if !out.is_file() {
        return json!({"valid": false, "reason": "No exported media file at destination yet"});
    }
    let Ok(meta) = std::fs::metadata(out) else {
        return json!({"valid": false, "reason": "Cannot read exported file metadata"});
    };
    if meta.len() == 0 {
        return json!({"valid": false, "reason": "Exported file is empty (0 bytes)"});
    }
    if let (Ok(out_time), Ok(proj_time)) = (meta.modified(), std::fs::metadata(&f.project).and_then(|m| m.modified()))
        && out_time < proj_time
    {
        return json!({"valid": false, "reason": "Timeline changed after this export was rendered"});
    }
    match probe_media(out) {
        Ok(probe) => json!({
            "valid": true,
            "path": out,
            "bytes": meta.len(),
            "probe": probe,
        }),
        Err(_) => json!({
            "valid": true,
            "path": out,
            "bytes": meta.len(),
        }),
    }
}

/// What exists for the film and whether the preview and export are older than the timeline.
pub fn cmd_video_film(s: &mut Session, p: &Value) -> Result<Value> {
    let f = film(s)?;
    let project = file_info(&f.project);
    let preview = file_info(&f.preview());
    let (out, options) = export_destination(&f, p);
    let export = file_info(&out);
    let validation = inspect_export_status(&f, &out);
    let stale = |v: &Value| v["exists"] == true && v["modified"].as_u64() < project["modified"].as_u64();
    let storyboard = file_info(&f.dir.join("storyboard.designcraft"));
    let storyboard_stale = project["exists"] == true && storyboard["exists"] == true && storyboard["modified"].as_u64() > project["modified"].as_u64();
    Ok(json!({"ok": true, "timeline": project, "preview": preview, "export": export, "storyboard": storyboard, "storyboard_stale": storyboard_stale,
        "preview_stale": stale(&preview), "export_stale": !validation["valid"].as_bool().unwrap_or(false),
        "export_ready": validation["valid"] == true, "validation": validation, "export_options": options}))
}

/// A half-size draft of the whole film with sound, to watch in Studio before exporting.
pub fn cmd_video_preview(s: &mut Session, p: &Value) -> Result<Value> {
    let f = film(s)?;
    let out = f.preview();
    export_with(&f, &out, true, p)?;
    Ok(json!({"ok": true, "preview": out, "timeline": f.project}))
}

/// The final film: full size, `exports/<song>.mp4` (or custom format/preset/options).
pub fn cmd_video_export(s: &mut Session, p: &Value) -> Result<Value> {
    let f = film(s)?;
    let (out, options) = export_destination(&f, p);
    export_with(&f, &out, false, &options)?;
    let validation = inspect_export_status(&f, &out);
    Ok(json!({"ok":true,"export":out,"timeline":f.project,"export_ready":validation["valid"] == true,"validation":validation,"export_options":options}))
}

/// Execute any FilmCraft command on the song's `.fcproj`.
pub fn cmd_video_exec(s: &mut Session, p: &Value) -> Result<Value> {
    let f = film(s)?;
    let project = f.require()?.to_path_buf();
    let id = p["id"].as_str().or_else(|| p["command"].as_str()).ok_or_else(|| bad("id=<filmcraft command> is required"))?;
    let params = p.get("params").cloned().unwrap_or_else(|| json!({}));
    let save = p["save"].as_bool().unwrap_or(true);
    let result = if save {
        crate::CraftHub::execute_and_save(crate::CraftTarget::Film, &project, id, &params)?
    } else {
        crate::CraftHub::execute(crate::CraftTarget::Film, Some(&project), id, &params)?
    };
    Ok(json!({"ok": true, "timeline": project, "id": id, "result": result}))
}

/// Render one program frame of the film at `tick` (`tick`, `time`, or `seconds`, default `0`).
pub fn cmd_video_frame(s: &mut Session, p: &Value) -> Result<Value> {
    let f = film(s)?;
    let project = f.require()?.to_path_buf();
    let tick = p.get("tick").or_else(|| p.get("time")).or_else(|| p.get("seconds")).and_then(parse_tick).unwrap_or(Tick::ZERO).max(Tick::ZERO);
    let out = p["path"]
        .as_str()
        .map(|raw| crate::workspace::resolve_workspace_path(s, raw))
        .unwrap_or_else(|| f.dir.join("film/frames").join(format!("{}-t{}.png", f.name, tick.0)));
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    frame_at_tick(&project, tick, &out)?;
    Ok(json!({"ok": true, "frame": out, "tick": tick.0, "timeline": project}))
}

/// Open the timeline in FilmCraft for hand edits.
pub fn cmd_video_open(s: &mut Session, _: &Value) -> Result<Value> {
    let f = film(s)?;
    let project = f.require()?.to_path_buf();
    crate::workspace::open_editor("filmcraft", &project)?;
    Ok(json!({"ok": true, "timeline": project, "document": {"app":"filmcraft", "file":project}}))
}

/// Save the film `.fcproj` with FilmCraft `file.save`.
pub fn cmd_film_save(s: &mut Session, _: &Value) -> Result<Value> {
    let f = film(s)?;
    let project = f.require()?.to_path_buf();
    run_steps(Some(&project), false, false, &[json!({"id": "file.save", "params": {}})])?;
    Ok(json!({"ok": true, "project": project}))
}

fn track_rows(tracks: &Value, kind: &str, media: &Value) -> Vec<Value> {
    let raw_tick = |v: &Value| parse_raw_tick(v).map(|t| t.0);
    let prefix = match kind {
        "video" => "V",
        "caption" => "C",
        _ => "A",
    };
    tracks
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(i, t)| {
            let items_key = if kind == "caption" { "captions" } else { "items" };
            let clips: Vec<Value> = t[items_key]
                .as_array()
                .into_iter()
                .flatten()
                .map(|c| {
                    let source = media.as_array().into_iter().flatten().find(|source| source["item"].as_u64().is_some() && source["item"] == c["item"]);
                    json!({
                        "clip": c.get("clip").or_else(|| c.get("id")).cloned().unwrap_or(Value::Null),
                        "item": c.get("item").cloned().unwrap_or(Value::Null),
                        "name": c.get("name").or_else(|| c.get("text")).cloned().unwrap_or(Value::Null),
                        "start_tick": raw_tick(&c["start"]),
                        "end_tick": raw_tick(&c["end"]),
                        "start": raw_tick(&c["start"]),
                        "end": raw_tick(&c["end"]),
                        "source_in_tick": raw_tick(&c["sourceIn"]),
                        "source_out_tick": raw_tick(&c["sourceOut"]),
                        "source_path": source.and_then(|source| source["path"].as_str()),
                        "source_status": source.and_then(|source| source["status"].as_str()),
                        "enabled": c.get("enabled").cloned().unwrap_or(json!(true)),
                    })
                })
                .collect();
            let label = format!("{prefix}{}", i + 1);
            let name = t.get("name").and_then(Value::as_str).unwrap_or(&label);
            let role = if name.eq_ignore_ascii_case("Kinetic Lyrics") {
                Some("lyrics")
            } else if name.to_ascii_lowercase().contains("character") || name.to_ascii_lowercase().contains("animation") {
                Some("character")
            } else {
                crate::cmd::film::assembly::track_role(&label)
            };
            json!({"id": t["id"], "label": label, "name": name, "kind": kind,
                "role": role, "enabled": t["enabled"], "clips": clips})
        })
        .collect()
}

/// The timeline's tracks and clips (in [`Tick`]s) as FilmCraft holds them, for Studio's layer view.
pub fn cmd_video_timeline(s: &mut Session, _: &Value) -> Result<Value> {
    let f = film(s)?;
    let seq = execute(f.require()?, "sequence.inspect", json!({}))?;
    let media = execute(f.require()?, "media.status", json!({}))?;
    let mut tracks = track_rows(&seq["captions"], "caption", &media);
    let mut video = track_rows(&seq["video"], "video", &media);
    video.reverse();
    tracks.extend(video);
    tracks.extend(track_rows(&seq["audio"], "audio", &media));
    let duration_tick = parse_raw_tick(&seq["duration"]).map(|t| t.0);
    Ok(json!({
        "ok": true,
        "timeline": f.project,
        "name": seq["name"],
        "duration_tick": duration_tick,
        "duration": duration_tick,
        "tracks": tracks,
    }))
}

/// Put a rendered clip (such as a `character.shot`) on a track at `from` tick and save.
pub fn cmd_video_place_clip(s: &mut Session, p: &Value) -> Result<Value> {
    let f = film(s)?;
    let clip = p["clip"].as_str().map(PathBuf::from).filter(|c| c.is_file()).ok_or_else(|| bad("clip=<rendered .mov or .mp4> is required"))?;
    let from = p.get("from_tick").or_else(|| p.get("from")).and_then(parse_tick).unwrap_or(Tick::ZERO).max(Tick::ZERO);
    let fps = p.get("fps").and_then(crate::time::parse_frame_rate).unwrap_or(crate::time::FrameRate::FPS_30);
    let track = p["track"].as_str().unwrap_or(crate::cmd::film::assembly::CHARACTER_TRACK);
    if s.project_mode.is_some() && !f.project.is_file() {
        if let Some(folder) = f.project.parent() {
            std::fs::create_dir_all(folder)?;
        }
        let sequence = json!({"id": "file.newSequence", "params": {"name": f.name,
            "width": p["width"].as_u64().unwrap_or(1920), "height": p["height"].as_u64().unwrap_or(1080),
            "fps": fps.as_f64()}});
        crate::cmd::film::assembly::run(&f.dir, &f.project, true, &[sequence])?;
        link(s, &f.project)?;
    }
    let project = f.require()?.to_path_buf();
    let imported = crate::cmd::film::assembly::run(&f.dir, &project, false, &[json!({"id": "file.import", "params": {"paths": [clip]}})])?;
    let item = imported
        .first()
        .and_then(|r| crate::cmd::film::assembly::ids(r, "items").first().copied())
        .ok_or_else(|| bad(format!("FilmCraft did not import {}", clip.display())))?;
    let place = json!({"id": "timeline.place", "params": {"item": item, "track": track, "seconds": from.seconds(),
        "insert": false, "audio": false}});
    let placed = crate::cmd::film::assembly::run(&f.dir, &project, false, &[place])?;
    let clips = placed.first().map(|r| crate::cmd::film::assembly::ids(r, "clips")).unwrap_or_default();
    if clips.is_empty() {
        return Err(bad(format!("FilmCraft did not place {} on {track}", clip.display())));
    }
    Ok(json!({
        "ok": true,
        "timeline": project,
        "track": track,
        "from_tick": from.0,
        "clips": clips,
        "next": "video.preview to watch it"
    }))
}
