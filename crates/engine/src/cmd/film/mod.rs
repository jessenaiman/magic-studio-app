//! The film as FilmCraft holds it: `film/<song>.fcproj` (built by `video.assemble`), a fast
//! half-size preview to watch in Studio, single program frames, the final export, and opening the
//! timeline in FilmCraft for hand edits. Document operations call the linked FilmCraft engine.

pub mod assembly;
pub mod studio_cues;
pub mod video_target;

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

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
    let name = s
        .project_path
        .as_ref()
        .and_then(|p| p.file_stem())
        .map_or("Film".into(), |n| n.to_string_lossy().into_owned());
    // The song's one film: the .magic's linked video project when it is a FilmCraft project.
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
        if self.project.is_file() {
            Ok(&self.project)
        } else {
            Err(bad("No film timeline yet; run video.assemble first"))
        }
    }
    fn preview(&self) -> PathBuf {
        self.dir
            .join("film")
            .join(format!("{}-preview.mp4", self.name))
    }
    fn export(&self) -> PathBuf {
        self.dir.join("exports").join(format!("{}.mp4", self.name))
    }
}

fn file_info(path: &Path) -> Value {
    let modified = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64);
    json!({"path": path, "exists": path.is_file(), "modified": modified})
}

/// The same timeline Studio opens and exports. Checking the file starts no external process.
/// The song's one film (see `film`).
pub(crate) fn film_path(s: &Session) -> Result<PathBuf> {
    Ok(film(s)?.project)
}

pub(crate) fn timeline_file(s: &Session) -> Result<Value> {
    Ok(file_info(&film(s)?.project))
}

pub(crate) fn open_filmcraft(project: &Path) -> Result<filmcraft_engine::Session> {
    let mut session = filmcraft_engine::Session::default();
    let path = std::fs::canonicalize(project).unwrap_or_else(|_| project.to_path_buf());
    session
        .execute("file.open", json!({"path": path}))
        .map_err(|e| EngineError::ExternalTool("filmcraft-engine".into(), e.to_string()))?;
    Ok(session)
}

/// Refresh the dedicated lettering layer while leaving the picture and audio cuts in place.
pub(crate) fn publish_lyrics(s: &Session, movie: &Path) -> Result<Value> {
    const TRACK: &str = "Kinetic Lyrics";
    let project = film_path(s)?;
    let mut session = open_filmcraft(&project)?;
    let mut run = |id: &str, params: Value| {
        session.execute(id, params).map_err(|error| {
            EngineError::ExternalTool("filmcraft-engine".into(), format!("{id}: {error}"))
        })
    };
    let sequence = run("sequence.inspect", json!({}))?;
    let duration = sequence["durationFrames"]
        .as_u64()
        .filter(|frames| *frames > 0)
        .ok_or_else(|| bad("Open a FilmCraft sequence with picture or audio first"))?;
    let imported = run("file.import", json!({"paths":[movie]}))?;
    let item = imported["items"][0]
        .as_u64()
        .ok_or_else(|| bad("FilmCraft did not import the lyric render"))?;
    let old_track = sequence["video"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|track| track["name"] == TRACK);
    let track = old_track
        .and_then(|track| track["id"].as_u64())
        .map_or(json!("new"), |id| json!(id));
    let placed = run(
        "timeline.place",
        json!({"item":item,"track":track,"frame":0,
        "duration":{"frame":duration},"insert":false,"video":true,"audio":false}),
    )?;
    let clip = placed["clips"][0]
        .as_u64()
        .ok_or_else(|| bad("FilmCraft did not place the lyric render"))?;
    run("clip.fitToFrame", json!({"clips":[clip]}))?;
    let after = run("sequence.inspect", json!({}))?;
    let track = after["video"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|track| {
            track["items"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|row| row["clip"] == clip)
        })
        .and_then(|track| track["id"].as_u64())
        .ok_or_else(|| bad("FilmCraft did not return the lyric track"))?;
    run(
        "timeline.setTrack",
        json!({"track":track,"name":TRACK,"enabled":true}),
    )?;
    run("file.save", json!({}))?;
    Ok(json!({"timeline": project, "track":track, "clip":clip}))
}

pub(crate) fn execute(project: &Path, id: &str, params: Value) -> Result<Value> {
    open_filmcraft(project)?
        .execute(id, params)
        .map_err(|e| EngineError::ExternalTool("filmcraft-engine".into(), format!("{id}: {e}")))
}

pub(crate) fn run(project: &Path, fresh: bool, steps: &[Value]) -> Result<Vec<Value>> {
    let mut session = if !fresh && project.is_file() {
        open_filmcraft(project)?
    } else {
        filmcraft_engine::Session::default()
    };
    let mut out = Vec::with_capacity(steps.len());
    for step in steps {
        let id = step["id"]
            .as_str()
            .or_else(|| step["command"].as_str())
            .ok_or_else(|| bad(format!("missing command id in {step}")))?;
        let params = step.get("params").cloned().unwrap_or_else(|| json!({}));
        let value = session.execute(id, params).map_err(|e| {
            EngineError::ExternalTool("filmcraft-engine".into(), format!("{id}: {e}"))
        })?;
        out.push(value);
    }
    session
        .execute("file.saveAs", json!({"path": project}))
        .map_err(|e| EngineError::ExternalTool("filmcraft-engine".into(), e.to_string()))?;
    Ok(out)
}

pub(crate) fn export(project: &Path, output: &Path, draft: bool) -> Result<()> {
    export_with_options(project, output, draft, &json!({}))
}

pub(crate) fn export_with_options(
    project: &Path,
    output: &Path,
    draft: bool,
    options: &Value,
) -> Result<()> {
    let mut session = open_filmcraft(project)?;
    let mut params = json!({"path": output, "format": "h264", "wait": true});
    if draft {
        params["scale"] = json!(0.5);
        params["quality"] = json!(60);
    }
    if let (Some(dst), Some(src)) = (params.as_object_mut(), options.as_object()) {
        for key in [
            "preset",
            "format",
            "audioCodec",
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
            "proresProfile",
            "dnxProfile",
            "bitrateKbps",
            "crf",
            "alpha",
        ] {
            if let Some(val) = src.get(key).filter(|v| !v.is_null()) {
                dst.insert(key.into(), val.clone());
            }
        }
    }
    let result = session
        .execute("file.exportMedia", params)
        .map_err(|e| EngineError::ExternalTool("filmcraft-engine".into(), e.to_string()))?;
    if let Some(offline) = result["offlineMedia"]
        .as_array()
        .filter(|items| !items.is_empty())
    {
        return Err(EngineError::ExternalTool(
            "filmcraft-engine".into(),
            format!("export blocked by offline media: {offline:?}"),
        ));
    }
    Ok(())
}

pub(crate) fn frame(project: &Path, seconds: f64, output: &Path) -> Result<()> {
    let mut session = open_filmcraft(project)?;
    let ticks = filmcraft_engine::commands::secs(seconds).0;
    if session
        .execute(
            "file.exportFrame",
            json!({"path": output, "format": "png", "target": "program", "time": ticks}),
        )
        .is_ok()
        && output.is_file()
    {
        return Ok(());
    }
    let (frame, offline) = session
        .try_render_program_with_offline_at(0.5, filmcraft_engine::commands::secs(seconds))
        .map_err(|e| EngineError::ExternalTool("filmcraft-engine".into(), e.to_string()))?;
    if !offline.is_empty() {
        return Err(EngineError::ExternalTool(
            "filmcraft-engine".into(),
            format!("frame blocked by offline media: {offline:?}"),
        ));
    }
    let rgba = frame.over_black_rgba8();
    image::save_buffer(
        output,
        &rgba,
        frame.w as u32,
        frame.h as u32,
        image::ColorType::Rgba8,
    )?;
    Ok(())
}

pub(crate) fn media_duration(filename: &Path, require_video: bool) -> Result<f64> {
    let session = filmcraft_engine::Session::default();
    let source = session
        .media
        .probe_file(&filename.to_string_lossy(), &*session.services)
        .map_err(|e| {
            EngineError::InvalidParams(format!("cannot inspect {}: {e}", filename.display()))
        })?;
    let info = source.info();
    if require_video && info.kind != filmcraft_media::MediaKind::Movie {
        return Err(EngineError::InvalidParams(format!(
            "{} has no video stream",
            filename.display()
        )));
    }
    if !require_video
        && info.audio_streams.is_empty()
        && info.kind != filmcraft_media::MediaKind::Movie
    {
        return Err(EngineError::InvalidParams(format!(
            "{} has no audio stream",
            filename.display()
        )));
    }
    let duration = info.duration.seconds();
    if !duration.is_finite() || duration <= 0.0 {
        return Err(EngineError::InvalidParams(format!(
            "{} has invalid duration {duration}",
            filename.display()
        )));
    }
    Ok(duration)
}

/// Export the active sequence with FilmCraft; `scale` and `quality` make a quick draft.
fn export_with(f: &Film, out: &Path, draft: bool, options: &Value) -> Result<()> {
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    export_with_options(f.require()?, out, draft, options)?;
    Ok(())
}

/// What exists for the film and whether the preview and export are older than the timeline.
pub fn cmd_video_film(s: &mut Session, _: &Value) -> Result<Value> {
    let f = film(s)?;
    let project = file_info(&f.project);
    let preview = file_info(&f.preview());
    let export = file_info(&f.export());
    let stale =
        |v: &Value| v["exists"] == true && v["modified"].as_u64() < project["modified"].as_u64();
    Ok(
        json!({"ok": true, "timeline": project, "preview": preview, "export": export,
        "preview_stale": stale(&preview), "export_stale": stale(&export)}),
    )
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
    let ext = match p["format"].as_str() {
        Some("prores" | "dnxhr") => "mov",
        Some("gif") => "gif",
        Some("wav") => "wav",
        Some("flac") => "flac",
        Some("aac") => "aac",
        _ => "mp4",
    };
    let out = p["path"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| f.dir.join("exports").join(format!("{}.{ext}", f.name)));
    export_with(&f, &out, false, p)?;
    Ok(json!({"ok": true, "export": out, "timeline": f.project}))
}

/// Execute any FilmCraft command (`file.exportInterchange`, `file.projectManager`,
/// `graphics.newFromFile`, `sequence.inspect`, `project.items`, etc.) on the song's `.fcproj`.
pub fn cmd_video_exec(s: &mut Session, p: &Value) -> Result<Value> {
    let f = film(s)?;
    let project = f.require()?.to_path_buf();
    let id = p["id"]
        .as_str()
        .or_else(|| p["command"].as_str())
        .ok_or_else(|| bad("id=<filmcraft command> is required"))?;
    let params = p.get("params").cloned().unwrap_or_else(|| json!({}));
    let save = p["save"].as_bool().unwrap_or(true);
    let mut session = open_filmcraft(&project)?;
    let result = session
        .execute(id, params)
        .map_err(|e| EngineError::ExternalTool("filmcraft-engine".into(), format!("{id}: {e}")))?;
    if save {
        session
            .execute("file.saveAs", json!({"path": project}))
            .map_err(|error| {
                EngineError::ExternalTool(
                    "filmcraft-engine".into(),
                    format!("file.saveAs: {error}"),
                )
            })?;
    }
    Ok(json!({"ok": true, "timeline": project, "id": id, "result": result}))
}

/// Render one program frame of the film at `seconds` (`time` or `seconds`, default `0.0`).
pub fn cmd_video_frame(s: &mut Session, p: &Value) -> Result<Value> {
    let f = film(s)?;
    let project = f.require()?.to_path_buf();
    let seconds = p["seconds"]
        .as_f64()
        .or_else(|| p["time"].as_f64())
        .unwrap_or(0.0)
        .max(0.0);
    let out = p["path"]
        .as_str()
        .map(|raw| crate::workspace::resolve_workspace_path(s, raw))
        .unwrap_or_else(|| {
            f.dir
                .join("film/frames")
                .join(format!("{}-{seconds:07.3}.png", f.name))
        });
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    frame(&project, seconds, &out)?;
    Ok(json!({"ok": true, "frame": out, "seconds": seconds, "timeline": project}))
}

/// Open the timeline in FilmCraft (the installed app the launcher registered) for hand edits.
pub fn cmd_video_open(s: &mut Session, _: &Value) -> Result<Value> {
    let f = film(s)?;
    let project = f.require()?.to_path_buf();
    crate::workspace::open_editor("filmcraft", &project)?;
    Ok(json!({"ok": true, "timeline": project, "document": {"app":"filmcraft", "file":project}}))
}

/// FilmCraft's time unit: ticks per second.
const TICKS: f64 = 254_016_000_000.0;

fn track_rows(tracks: &Value, kind: &str) -> Vec<Value> {
    let secs = |v: &Value| v.as_f64().map(|t| t / TICKS);
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
                    json!({
                        "clip": c.get("clip").or_else(|| c.get("id")).cloned().unwrap_or(Value::Null),
                        "item": c.get("item").cloned().unwrap_or(Value::Null),
                        "name": c.get("name").or_else(|| c.get("text")).cloned().unwrap_or(Value::Null),
                        "start": secs(&c["start"]),
                        "end": secs(&c["end"]),
                        "enabled": c.get("enabled").cloned().unwrap_or(json!(true)),
                    })
                })
                .collect();
            let label = format!("{prefix}{}", i + 1);
            json!({"id": t["id"], "label": label, "kind": kind, "enabled": t["enabled"], "clips": clips})
        })
        .collect()
}

/// The timeline's tracks and clips (in seconds) as FilmCraft holds them, for Studio's layer view.
pub fn cmd_video_timeline(s: &mut Session, _: &Value) -> Result<Value> {
    let f = film(s)?;
    let seq = execute(f.require()?, "sequence.inspect", json!({}))?;
    let mut tracks = track_rows(&seq["captions"], "caption");
    let mut video = track_rows(&seq["video"], "video");
    video.reverse();
    tracks.extend(video);
    tracks.extend(track_rows(&seq["audio"], "audio"));
    Ok(
        json!({"ok": true, "timeline": f.project, "name": seq["name"],
        "duration": seq["duration"].as_f64().map(|t| t / TICKS), "tracks": tracks}),
    )
}

/// Put a rendered clip (such as a `character.shot`) on a track at `from` seconds and save, so a
/// saved animation is part of the film at once.
pub fn cmd_video_place_clip(s: &mut Session, p: &Value) -> Result<Value> {
    let f = film(s)?;
    let clip = p["clip"]
        .as_str()
        .map(PathBuf::from)
        .filter(|c| c.is_file())
        .ok_or_else(|| bad("clip=<rendered .mov or .mp4> is required"))?;
    let from = p["from"].as_f64().unwrap_or(0.0).max(0.0);
    let track = p["track"].as_str().unwrap_or("V2");
    // An animation study has no song to assemble from, so its first clip starts the film.
    if s.project_mode.is_some() && !f.project.is_file() {
        if let Some(folder) = f.project.parent() {
            std::fs::create_dir_all(folder)?;
        }
        let sequence = json!({"id": "file.newSequence", "params": {"name": f.name,
            "width": p["width"].as_u64().unwrap_or(1920), "height": p["height"].as_u64().unwrap_or(1080),
            "fps": p["fps"].as_f64().unwrap_or(30.0)}});
        crate::cmd::film::assembly::run(&f.dir, &f.project, true, &[sequence])?;
        link(s, &f.project)?;
    }
    let project = f.require()?.to_path_buf();
    let imported = crate::cmd::film::assembly::run(
        &f.dir,
        &project,
        false,
        &[json!({"id": "file.import", "params": {"paths": [clip]}})],
    )?;
    let item = imported
        .first()
        .and_then(|r| crate::cmd::film::assembly::ids(r, "items").first().copied())
        .ok_or_else(|| bad(format!("FilmCraft did not import {}", clip.display())))?;
    let place = json!({"id": "timeline.place", "params": {"item": item, "track": track, "seconds": from,
        "insert": false, "audio": false}});
    let placed = crate::cmd::film::assembly::run(&f.dir, &project, false, &[place])?;
    let clips = placed
        .first()
        .map(|r| crate::cmd::film::assembly::ids(r, "clips"))
        .unwrap_or_default();
    if clips.is_empty() {
        return Err(bad(format!(
            "FilmCraft did not place {} on {track}",
            clip.display()
        )));
    }
    Ok(
        json!({"ok": true, "timeline": project, "track": track, "from": from, "clips": clips,
        "next": "video.preview to watch it"}),
    )
}
