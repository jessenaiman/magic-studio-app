//! The film as FilmCraft holds it: `film/<song>.fcproj` (built by `video.assemble`), a fast
//! half-size preview to watch in Studio, single program frames, the final export, and opening the
//! timeline in FilmCraft for hand edits. Every step is a `filmcraft-cli` command.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

const EDITOR_CLI: &str = "filmcraft-cli";

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

fn path_arg(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// The same timeline Studio opens and exports. Checking the file starts no external process.
/// The song's one film (see `film`).
pub(crate) fn film_path(s: &Session) -> Result<PathBuf> {
    Ok(film(s)?.project)
}

pub(crate) fn timeline_file(s: &Session) -> Result<Value> {
    Ok(file_info(&film(s)?.project))
}

/// Export the active sequence with FilmCraft; `scale` and `quality` make a quick draft.
fn export_with(f: &Film, out: &Path, draft: bool) -> Result<()> {
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut args = vec![
        "--project".into(),
        path_arg(f.require()?),
        "export".into(),
        path_arg(out),
    ];
    args.extend(["--format".into(), "h264".into()]);
    if draft {
        args.extend([
            "--scale".into(),
            "0.5".into(),
            "--quality".into(),
            "60".into(),
        ]);
    }
    crate::workspace::run_tool_in(&f.dir, EDITOR_CLI, &args)?;
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
pub fn cmd_video_preview(s: &mut Session, _: &Value) -> Result<Value> {
    let f = film(s)?;
    let out = f.preview();
    export_with(&f, &out, true)?;
    Ok(json!({"ok": true, "preview": out, "timeline": f.project}))
}

/// The final film: full size, `exports/<song>.mp4`.
pub fn cmd_video_export(s: &mut Session, _: &Value) -> Result<Value> {
    let f = film(s)?;
    let out = f.export();
    export_with(&f, &out, false)?;
    Ok(json!({"ok": true, "export": out, "timeline": f.project}))
}

/// One program frame of the timeline at `seconds`, as FilmCraft renders it.
pub fn cmd_video_frame(s: &mut Session, p: &Value) -> Result<Value> {
    let f = film(s)?;
    let seconds = p["seconds"].as_f64().unwrap_or(0.0).max(0.0);
    let out = f
        .dir
        .join("film/frames")
        .join(format!("{}-{seconds:08.3}.png", f.name));
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let args = vec![
        "--project".into(),
        path_arg(f.require()?),
        "render".into(),
        "--seconds".into(),
        seconds.to_string(),
        "--out".into(),
        path_arg(&out),
        "--scale".into(),
        "0.5".into(),
    ];
    crate::workspace::run_tool_in(&f.dir, EDITOR_CLI, &args)?;
    Ok(json!({"ok": true, "frame": out, "seconds": seconds}))
}

/// Open the timeline in FilmCraft (the installed app the launcher registered) for hand edits.
pub fn cmd_video_open(s: &mut Session, _: &Value) -> Result<Value> {
    let f = film(s)?;
    let project = f.require()?.to_path_buf();
    let mut opened = crate::native_app::open(s, &crate::native_app::FILMCRAFT, &project)?;
    opened["timeline"] = json!(project);
    Ok(opened)
}

/// FilmCraft's time unit: ticks per second.
const TICKS: f64 = 254_016_000_000.0;

fn track_rows(tracks: &Value, kind: &str) -> Vec<Value> {
    let secs = |v: &Value| v.as_f64().map(|t| t / TICKS);
    tracks
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(i, t)| {
            let clips: Vec<Value> = t["items"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|c| json!({"clip": c["clip"], "item": c["item"], "name": c["name"],
                    "start": secs(&c["start"]), "end": secs(&c["end"]), "enabled": c["enabled"]}))
                .collect();
            let label = format!("{}{}", if kind == "video" { "V" } else { "A" }, i + 1);
            json!({"id": t["id"], "label": label, "kind": kind, "enabled": t["enabled"], "clips": clips})
        })
        .collect()
}

/// The timeline's tracks and clips (in seconds) as FilmCraft holds them, for Studio's layer view.
pub fn cmd_video_timeline(s: &mut Session, _: &Value) -> Result<Value> {
    let f = film(s)?;
    let args = vec![
        "--project".into(),
        path_arg(f.require()?),
        "inspect".into(),
        "sequence".into(),
    ];
    let out = crate::workspace::run_tool_in(&f.dir, EDITOR_CLI, &args)?;
    let seq: Value =
        serde_json::from_str(&out).map_err(|e| bad(format!("{EDITOR_CLI} inspect: {e}")))?;
    let mut tracks = track_rows(&seq["video"], "video");
    tracks.reverse();
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
        crate::assembly::run(&f.dir, &f.project, true, &[sequence])?;
        link(s, &f.project)?;
    }
    let project = f.require()?.to_path_buf();
    let imported = crate::assembly::run(
        &f.dir,
        &project,
        false,
        &[json!({"id": "file.import", "params": {"paths": [clip]}})],
    )?;
    let item = imported
        .first()
        .and_then(|r| crate::assembly::ids(r, "items").first().copied())
        .ok_or_else(|| bad(format!("{EDITOR_CLI} did not import {}", clip.display())))?;
    let place = json!({"id": "timeline.place", "params": {"item": item, "track": track, "seconds": from,
        "insert": false, "audio": false}});
    let placed = crate::assembly::run(&f.dir, &project, false, &[place])?;
    let clips = placed
        .first()
        .map(|r| crate::assembly::ids(r, "clips"))
        .unwrap_or_default();
    if clips.is_empty() {
        return Err(bad(format!(
            "{EDITOR_CLI} did not place {} on {track}",
            clip.display()
        )));
    }
    Ok(
        json!({"ok": true, "timeline": project, "track": track, "from": from, "clips": clips,
        "next": "video.preview to watch it"}),
    )
}
