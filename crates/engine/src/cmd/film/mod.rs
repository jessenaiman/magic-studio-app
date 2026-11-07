//! The film as FilmCraft holds it: `film/<song>.fcproj` (built by `video.assemble`), a fast
//! half-size preview to watch in Studio, single program frames, the final export, and opening the
//! timeline in FilmCraft for hand edits. Document operations call the linked FilmCraft engine.

pub mod assembly;
mod export_validation;
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
    let name = s.project_path.as_ref().and_then(|p| p.file_stem()).map_or("Film".into(), |n| n.to_string_lossy().into_owned());
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
    session.execute("file.open", json!({"path": path})).map_err(|e| EngineError::ExternalTool("filmcraft-engine".into(), e.to_string()))?;
    Ok(session)
}

/// Refresh the dedicated lettering layer while leaving the picture and audio cuts in place.
///
/// Uses `media.replaceFootage` (`storytold/filmcraft/crates/engine/src/relink.rs`) when the
/// `"Kinetic Lyrics"` track already holds a clip item so `ItemId`, `ClipId`, track placement,
/// trims, markers, and groups stay intact and FilmCraft bumps `Session.revision` to invalidate
/// decoded frame caches. Falls back to `file.import` + `timeline.place` only on initial creation.
pub(crate) fn publish_lyrics(s: &Session, movie: &Path) -> Result<Value> {
    const TRACK: &str = "Kinetic Lyrics";
    let project = film_path(s)?;
    if let Some(live_result) = publish_lyrics_live(&project, movie, TRACK) {
        return live_result;
    }
    let _lock = crate::project_lock::for_project(&project);
    let mut session = open_filmcraft(&project)?;
    let mut run =
        |id: &str, params: Value| session.execute(id, params).map_err(|error| EngineError::ExternalTool("filmcraft-engine".into(), format!("{id}: {error}")));
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

/// Match the native source path, including a project relocation under the same
/// media tree. A bare lyrics.mov filename never establishes source ownership.
fn same_lettering_source(source: &Path, movie: &Path) -> bool {
    if source == movie || matches!((source.canonicalize(), movie.canonicalize()), (Ok(a), Ok(b)) if a == b) {
        return true;
    }
    let parts: Vec<_> = movie.components().collect();
    let Some(media) = parts.iter().rposition(|part| part.as_os_str() == "media") else { return false };
    let suffix: PathBuf = parts[media..].iter().collect();
    // media/<project>/lettering/lyrics.mov is the shortest owned source suffix.
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
    let cli = crate::workspace::find_executable("filmcraft-cli").map_err(|e| EngineError::ExternalTool("filmcraft-cli".into(), e.to_string()))?;
    let out = std::process::Command::new(&cli)
        .arg("--project")
        .arg(project)
        .arg("exec")
        .arg(id)
        .arg(params.to_string())
        .output()
        .map_err(|e| EngineError::ExternalTool("filmcraft-cli".into(), e.to_string()))?;
    if !out.status.success() {
        return Err(EngineError::ExternalTool("filmcraft-cli".into(), format!("exit {:?}: {}", out.status.code(), String::from_utf8_lossy(&out.stderr))));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| EngineError::ExternalTool("filmcraft-cli".into(), e.to_string()))
}

pub(crate) fn run(project: &Path, fresh: bool, steps: &[Value]) -> Result<Vec<Value>> {
    let mut session = if !fresh && project.is_file() { open_filmcraft(project)? } else { filmcraft_engine::Session::default() };
    let mut out = Vec::with_capacity(steps.len());
    for step in steps {
        let id = step["id"].as_str().or_else(|| step["command"].as_str()).ok_or_else(|| bad(format!("missing command id in {step}")))?;
        let params = step.get("params").cloned().unwrap_or_else(|| json!({}));
        let value = session.execute(id, params).map_err(|e| EngineError::ExternalTool("filmcraft-engine".into(), format!("{id}: {e}")))?;
        out.push(value);
    }
    session.execute("file.saveAs", json!({"path": project})).map_err(|e| EngineError::ExternalTool("filmcraft-engine".into(), e.to_string()))?;
    Ok(out)
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
            if let Some(val) = src.get(key).filter(|v| !v.is_null() || matches!(key, "captionSidecar" | "preset")) {
                // Native FilmCraft uses the explicit string "none" to clear a
                // sidecar; JSON null alone leaves its saved setting unchanged.
                dst.insert(key.into(), if key == "captionSidecar" && val.is_null() { json!("none") } else { val.clone() });
            }
        }
    }
    params
}

pub(crate) fn export_with_options(project: &Path, output: &Path, draft: bool, options: &Value) -> Result<()> {
    let mut session = open_filmcraft(project)?;
    let params = export_params(output, draft, options);
    let result = session.execute("file.exportMedia", params).map_err(|e| EngineError::ExternalTool("filmcraft-engine".into(), e.to_string()))?;
    if let Some(offline) = result["offlineMedia"].as_array().filter(|items| !items.is_empty()) {
        return Err(EngineError::ExternalTool("filmcraft-engine".into(), format!("export blocked by offline media: {offline:?}")));
    }
    Ok(())
}

pub(crate) fn frame(project: &Path, seconds: f64, output: &Path) -> Result<()> {
    // Via installed filmcraft-cli render (system app pattern)
    let cli = crate::workspace::find_executable("filmcraft-cli").map_err(|e| EngineError::ExternalTool("filmcraft-cli".into(), e.to_string()))?;
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let out = std::process::Command::new(&cli)
        .arg("--project")
        .arg(project)
        .arg("render")
        .arg("--seconds")
        .arg(seconds.to_string())
        .arg("--out")
        .arg(output)
        .output()
        .map_err(|e| EngineError::ExternalTool("filmcraft-cli".into(), e.to_string()))?;
    if !out.status.success() {
        return Err(EngineError::ExternalTool("filmcraft-cli".into(), format!("exit {:?}: {}", out.status.code(), String::from_utf8_lossy(&out.stderr))));
    }
    Ok(())
}

pub(crate) fn media_duration(filename: &Path, require_video: bool) -> Result<f64> {
    let session = filmcraft_engine::Session::default();
    let source = session
        .media
        .probe_file(&filename.to_string_lossy(), &*session.services)
        .map_err(|e| EngineError::InvalidParams(format!("cannot inspect {}: {e}", filename.display())))?;
    let info = source.info();
    if require_video && info.kind != filmcraft_media::MediaKind::Movie {
        return Err(EngineError::InvalidParams(format!("{} has no video stream", filename.display())));
    }
    if !require_video && info.audio_streams.is_empty() && info.kind != filmcraft_media::MediaKind::Movie {
        return Err(EngineError::InvalidParams(format!("{} has no audio stream", filename.display())));
    }
    let duration = info.duration.seconds();
    if !duration.is_finite() || duration <= 0.0 {
        return Err(EngineError::InvalidParams(format!("{} has invalid duration {duration}", filename.display())));
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
pub fn cmd_video_film(s: &mut Session, p: &Value) -> Result<Value> {
    let f = film(s)?;
    let project = file_info(&f.project);
    let preview = file_info(&f.preview());
    let (out, options) = export_validation::destination(&f, p);
    let export = file_info(&out);
    let validation = export_validation::status(&f, &out, &options, false);
    let stale = |v: &Value| v["exists"] == true && v["modified"].as_u64() < project["modified"].as_u64();
    Ok(json!({"ok": true, "timeline": project, "preview": preview, "export": export,
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
    let (out, options) = export_validation::destination(&f, p);
    let before = export_validation::context(&f, &out, &options)?;
    export_with(&f, &out, false, &options)?;
    let validation = export_validation::record(&f, &out, &options, &before)?;
    Ok(json!({"ok":true,"export":out,"timeline":f.project,"export_ready":true,"validation":validation,"export_options":options}))
}

/// Recheck a recorded export without rendering or changing its receipt.
pub fn cmd_video_validate_export(s: &mut Session, p: &Value) -> Result<Value> {
    let f = film(s)?;
    let (out, options) = export_validation::destination(&f, p);
    let validation = export_validation::status(&f, &out, &options, true);
    Ok(json!({"ok":true,"export":out,"timeline":f.project,"export_ready":validation["valid"] == true,"validation":validation,"export_options":options}))
}

/// Execute any FilmCraft command (`file.exportInterchange`, `file.projectManager`,
/// `graphics.newFromFile`, `sequence.inspect`, `project.items`, etc.) on the song's `.fcproj`.
pub fn cmd_video_exec(s: &mut Session, p: &Value) -> Result<Value> {
    let f = film(s)?;
    let project = f.require()?.to_path_buf();
    let id = p["id"].as_str().or_else(|| p["command"].as_str()).ok_or_else(|| bad("id=<filmcraft command> is required"))?;
    let params = p.get("params").cloned().unwrap_or_else(|| json!({}));
    let save = p["save"].as_bool().unwrap_or(true);
    let mut session = open_filmcraft(&project)?;
    let result = session.execute(id, params).map_err(|e| EngineError::ExternalTool("filmcraft-engine".into(), format!("{id}: {e}")))?;
    if save {
        session
            .execute("file.saveAs", json!({"path": project}))
            .map_err(|error| EngineError::ExternalTool("filmcraft-engine".into(), format!("file.saveAs: {error}")))?;
    }
    Ok(json!({"ok": true, "timeline": project, "id": id, "result": result}))
}

/// Render one program frame of the film at `seconds` (`time` or `seconds`, default `0.0`).
pub fn cmd_video_frame(s: &mut Session, p: &Value) -> Result<Value> {
    let f = film(s)?;
    let project = f.require()?.to_path_buf();
    let seconds = p["seconds"].as_f64().or_else(|| p["time"].as_f64()).unwrap_or(0.0).max(0.0);
    let out = p["path"]
        .as_str()
        .map(|raw| crate::workspace::resolve_workspace_path(s, raw))
        .unwrap_or_else(|| f.dir.join("film/frames").join(format!("{}-{seconds:07.3}.png", f.name)));
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
    Ok(json!({"ok": true, "timeline": f.project, "name": seq["name"],
        "duration": seq["duration"].as_f64().map(|t| t / TICKS), "tracks": tracks}))
}

/// Put a rendered clip (such as a `character.shot`) on a track at `from` seconds and save, so a
/// saved animation is part of the film at once.
pub fn cmd_video_place_clip(s: &mut Session, p: &Value) -> Result<Value> {
    let f = film(s)?;
    let clip = p["clip"].as_str().map(PathBuf::from).filter(|c| c.is_file()).ok_or_else(|| bad("clip=<rendered .mov or .mp4> is required"))?;
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
    let imported = crate::cmd::film::assembly::run(&f.dir, &project, false, &[json!({"id": "file.import", "params": {"paths": [clip]}})])?;
    let item = imported
        .first()
        .and_then(|r| crate::cmd::film::assembly::ids(r, "items").first().copied())
        .ok_or_else(|| bad(format!("FilmCraft did not import {}", clip.display())))?;
    let place = json!({"id": "timeline.place", "params": {"item": item, "track": track, "seconds": from,
        "insert": false, "audio": false}});
    let placed = crate::cmd::film::assembly::run(&f.dir, &project, false, &[place])?;
    let clips = placed.first().map(|r| crate::cmd::film::assembly::ids(r, "clips")).unwrap_or_default();
    if clips.is_empty() {
        return Err(bad(format!("FilmCraft did not place {} on {track}", clip.display())));
    }
    Ok(json!({"ok": true, "timeline": project, "track": track, "from": from, "clips": clips,
        "next": "video.preview to watch it"}))
}

#[cfg(test)]
mod tests {
    //! Reference: <https://github.com/storytold/filmcraft/blob/7322aad72491a294a06a26a63fb65a3fc30ac242/crates/engine/src/relink.rs#L701-L715>
    use super::*;

    #[test]
    fn legacy_source_matching_rejects_filename_only_and_ambiguous_clips() {
        let movie = Path::new("/new/media/artist/song/lettering/lyrics.mov");
        assert!(!same_lettering_source(Path::new("/unrelated/lyrics.mov"), movie));
        assert!(!same_lettering_source(Path::new("/old/media/other/song/lettering/lyrics.mov"), movie));
        let sequence = json!({"durationFrames":300,"video":[{"id":3,"items":[
            {"clip":112,"item":24,"startFrame":0,"endFrame":300},
            {"clip":113,"item":24,"startFrame":0,"endFrame":300}
        ]}]});
        let media = json!([{"item":24,"path":"/old/media/artist/song/lettering/lyrics.mov"}]);
        assert!(legacy_lyric_slot(&sequence, movie, &media).is_err());
    }

    #[test]
    fn publish_lyrics_adopts_relocated_native_source_without_replacing_edits() {
        let sequence = json!({"durationFrames":300,"video":[{"id":3,"name":"Video 3","items":[{"clip":112,"item":24,"startFrame":0,"endFrame":300,"effects":[{"id":"authored"}]}]}]});
        let mut calls = Vec::new();
        let result = publish_lyrics_sequence(
            Path::new("/new/media/artist/song/film/song.fcproj"),
            Path::new("/new/media/artist/song/lettering/lyrics.mov"),
            "Kinetic Lyrics",
            &sequence,
            &mut |id, params| {
                calls.push((id.to_owned(), params));
                if id == "media.status" { Ok(json!([{"item":24,"path":"/old/media/artist/song/lettering/lyrics.mov"}])) } else { Ok(json!({})) }
            },
        )
        .unwrap();
        assert_eq!(result["clip"], 112);
        assert_eq!(result["item"], 24);
        assert!(!calls.iter().any(|(id, _)| id == "file.import" || id == "timeline.place" || id == "clip.fitToFrame"));
    }

    #[test]
    fn publish_lyrics_replaces_footage_in_place_when_kinetic_track_exists() {
        let existing_seq = json!({
            "durationFrames": 300,
            "video": [{
                "id": 2,
                "name": "Kinetic Lyrics",
                "items": [{"clip": 42, "item": 9, "start": 0, "end": 1000}]
            }]
        });
        let mut calls = Vec::new();
        let out = publish_lyrics_sequence(Path::new("/tmp/song.fcproj"), Path::new("/tmp/lyrics.mov"), "Kinetic Lyrics", &existing_seq, &mut |id, params| {
            calls.push((id.to_string(), params));
            Ok(json!({}))
        })
        .unwrap();

        assert_eq!(out["relinked"], true);
        assert_eq!(out["track"], 2);
        assert_eq!(out["clip"], 42);
        assert_eq!(out["item"], 9);
        assert_eq!(calls[0].0, "media.replaceFootage");
        assert_eq!(calls[0].1, json!({"item": 9, "path": "/tmp/lyrics.mov"}));
        assert_eq!(calls[1].0, "timeline.setTrack");
        assert_eq!(calls[2].0, "file.save");
    }
}
