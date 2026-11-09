//! `video.assemble`: the film put together by the linked FilmCraft engine using exact [`Tick`]s
//! and rational [`FrameRate`]s. The song's audio on A1, the backgrounds on V1 from the ticks
//! the caller gives, character shots on V2 and the lettering render on V3; then an export.

use crate::time::{FrameRate, Tick, frame_rate_json, parse_frame_rate, parse_tick};
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub(crate) const BACKGROUND_TRACK: &str = "V1";
pub(crate) const CHARACTER_TRACK: &str = "V2";
pub(crate) const LYRICS_TRACK: &str = "V3";
pub(crate) const SONG_TRACK: &str = "A1";

pub(crate) fn track_role(label: &str) -> Option<&'static str> {
    match label {
        BACKGROUND_TRACK => Some("background"),
        CHARACTER_TRACK => Some("character"),
        LYRICS_TRACK => Some("lyrics"),
        SONG_TRACK => Some("song"),
        _ => None,
    }
}

fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

fn arg(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

pub(crate) fn run(_dir: &Path, project: &Path, fresh: bool, steps: &[Value]) -> Result<Vec<Value>> {
    let file = project.with_extension("steps.jsonl");
    let mut body: String = steps.iter().map(|s| format!("{s}\n")).collect();
    if !fresh && let Ok(earlier) = std::fs::read_to_string(&file) {
        body = earlier + &body;
    }
    crate::doc_state::write_file(&file, &body)?;
    crate::cmd::film::run(project, fresh, steps)
}

pub(crate) fn ids(result: &Value, field: &str) -> Vec<u64> {
    result[field].as_array().into_iter().flatten().filter_map(Value::as_u64).collect()
}

fn background(s: &Session, dir: &Path, entry: &Value) -> Result<PathBuf> {
    if let Some(path) = entry["path"].as_str() {
        let path = dir.join(path);
        return path.is_file().then_some(path.clone()).ok_or_else(|| invalid(format!("No image at {}", path.display())));
    }
    let id = entry["asset"].as_str().ok_or_else(|| invalid("each background needs asset or path"))?;
    s.project_assets
        .iter()
        .find(|a| a.id == id)
        .and_then(|a| a.path.clone())
        .map(|p| dir.join(p))
        .filter(|p| p.is_file())
        .ok_or_else(|| invalid(format!("No image for asset {id}")))
}

/// Backgrounds in tick order with the `(start_tick, duration_tick)` each covers, the last one to the end.
fn spans(times: &[Tick], duration: Tick) -> Vec<(Tick, Tick)> {
    times.iter().enumerate().map(|(i, from)| (*from, (times.get(i + 1).copied().unwrap_or(duration) - *from).max(Tick::ZERO))).collect()
}

fn resolve_scene_image(scene: &Value, dir: &Path, assets: &[crate::ProjectAsset]) -> Option<PathBuf> {
    let asset_id = scene["image_asset_id"].as_str().or_else(|| scene["background_asset"].as_str()).or_else(|| scene["asset"].as_str());
    if let Some(id) = asset_id
        && let Some(path) = assets.iter().find(|a| a.id == id).and_then(|a| a.path.as_ref()).map(|p| dir.join(p)).filter(|p| p.is_file())
    {
        return Some(path);
    }
    scene["image_path"].as_str().or_else(|| scene["illustration"].as_str()).or_else(|| scene["path"].as_str()).map(|p| dir.join(p)).filter(|p| p.is_file())
}

fn backgrounds_from_storyboard(board: &Value, dir: &Path, assets: &[crate::ProjectAsset]) -> Vec<(Tick, PathBuf)> {
    let list = board["scenes"].as_array().or_else(|| board["animations"].as_array());
    let mut out: Vec<(Tick, PathBuf)> = list
        .into_iter()
        .flatten()
        .filter_map(|sc| {
            let start = sc
                .get("start_tick")
                .or_else(|| sc.get("start_seconds"))
                .or_else(|| sc.get("start"))
                .or_else(|| sc.get("song_anchor_tick"))
                .or_else(|| sc.get("song_anchor"))
                .or_else(|| sc.get("lines").and_then(|l| l.get(0)).and_then(|l| l.get("start_tick").or_else(|| l.get("start"))))
                .or_else(|| sc.get("words").and_then(|w| w.get(0)).and_then(|w| w.get("start_tick").or_else(|| w.get("start"))))
                .and_then(parse_tick)
                .unwrap_or(Tick::ZERO);
            Some((start, resolve_scene_image(sc, dir, assets)?))
        })
        .collect();
    out.sort_by_key(|a| a.0);
    let min_gap = Tick::from_units(1, 1000);
    out.dedup_by(|a, b| a.1 == b.1 || (a.0 - b.0).abs() < min_gap);
    out
}

fn auto_backgrounds(s: &Session, dir: &Path, duration: Tick) -> Option<Vec<(Tick, PathBuf)>> {
    let imgs: Vec<PathBuf> =
        s.project_assets.iter().filter(|a| a.kind == "image").filter_map(|a| a.path.as_ref().map(|p| dir.join(p))).filter(|p| p.is_file()).collect();
    if let Ok(bytes) = std::fs::read(dir.join("storyboard/storyboard.json"))
        && let Ok(doc) = serde_json::from_slice::<Value>(&bytes)
    {
        let bgs = backgrounds_from_storyboard(&doc, dir, &s.project_assets);
        if !bgs.is_empty() {
            return Some(bgs);
        }
    }
    if let Ok(bytes) = std::fs::read(dir.join("design/words.json"))
        && let Ok(doc) = serde_json::from_slice::<Value>(&bytes)
    {
        let bgs = backgrounds_from_storyboard(&doc, dir, &s.project_assets);
        let custom = doc["animations"].as_array().is_some_and(|a| a.iter().any(|x| x["background_custom"] == true));
        if bgs.len() > 1 || (!bgs.is_empty() && (custom || imgs.len() <= 1)) {
            return Some(bgs);
        }
    }
    if imgs.is_empty() {
        return None;
    }
    let count = imgs.len() as i64;
    let step = duration.mul_ratio(1, count).max(Tick::from_units(1, 1));
    Some(imgs.into_iter().enumerate().map(|(i, p)| (step.mul_ratio(i as i64, 1), p)).collect())
}

struct Inputs {
    name: String,
    audio: PathBuf,
    lettering: PathBuf,
    backgrounds: Vec<(Tick, PathBuf)>,
    characters: Vec<(Tick, PathBuf)>,
    duration: Tick,
}

fn character_clips(dir: &Path, p: &Value) -> Result<Vec<(Tick, PathBuf)>> {
    let mut out = Vec::new();
    for entry in p["characters"].as_array().into_iter().flatten() {
        let clip = entry["clip"].as_str().map(|c| dir.join(c)).filter(|c| c.is_file());
        let clip = clip.ok_or_else(|| invalid(format!("character clip missing: {}", entry["clip"])))?;
        let from = entry.get("from_tick").or_else(|| entry.get("from")).and_then(parse_tick).unwrap_or(Tick::ZERO);
        out.push((from, clip));
    }
    out.sort_by_key(|a| a.0);
    Ok(out)
}

fn inputs(s: &Session, dir: &Path, p: &Value) -> Result<Inputs> {
    let name = s.project_path.as_ref().and_then(|p| p.file_stem()).map_or("Film".into(), |n| n.to_string_lossy().into_owned());
    let audio = s.song_audio.clone().map(|a| dir.join(a)).filter(|a| a.is_file()).ok_or_else(|| invalid("No song audio in this project"))?;
    let lettering = dir.join(p["lettering"].as_str().unwrap_or("lettering/lyrics.mov"));
    if !lettering.is_file() {
        return Err(invalid(format!("No lettering render at {}; run words.letter first", lettering.display())));
    }
    let duration = crate::workshop::song_ticks(dir).ok_or_else(|| invalid("Song length unknown; analyze the song first"))?;
    let backgrounds = if let Some(entries) = p["backgrounds"].as_array().filter(|b| !b.is_empty()) {
        let mut bgs = entries
            .iter()
            .map(|e| {
                let from = e.get("from_tick").or_else(|| e.get("from")).and_then(parse_tick).unwrap_or(Tick::ZERO);
                Ok((from, background(s, dir, e)?))
            })
            .collect::<Result<Vec<_>>>()?;
        bgs.sort_by_key(|a| a.0);
        bgs
    } else {
        auto_backgrounds(s, dir, duration).ok_or_else(|| {
            let known: Vec<&str> = s.project_assets.iter().map(|a| a.id.as_str()).collect();
            invalid(format!("backgrounds: [{{\"asset\":id,\"from_tick\":ticks}}, ...] is required; assets: {}", known.join(", ")))
        })?
    };
    Ok(Inputs { name, audio, lettering, backgrounds, characters: character_clips(dir, p)?, duration })
}

fn build_film(dir: &Path, project: &Path, film: &Inputs, fps: FrameRate) -> Result<()> {
    let fps = fps.sane();
    let mut media = vec![arg(&film.audio)];
    media.extend(film.backgrounds.iter().map(|(_, i)| arg(i)));
    media.push(arg(&film.lettering));
    media.extend(film.characters.iter().map(|(_, c)| arg(c)));
    let made = run(
        dir,
        project,
        true,
        &[
            json!({"id": "file.newProject", "params": {"name": film.name}}),
            json!({"id": "file.newSequence", "params": {"name": film.name, "width": 1920, "height": 1080, "fps": fps.as_f64()}}),
            json!({"id": "file.import", "params": {"paths": media}}),
        ],
    )?;
    let items = made.get(2).map(|r| ids(r, "items")).unwrap_or_default();
    if items.len() != media.len() {
        return Err(invalid(format!("FilmCraft imported {} of {} files", items.len(), media.len())));
    }
    let nb = film.backgrounds.len();
    let (audio, stills, lyrics, shots) = (items[0], &items[1..=nb], items[nb + 1], &items[nb + 2..]);
    let times: Vec<Tick> = film.backgrounds.iter().map(|(t, _)| *t).collect();
    let mut place = vec![json!({"id": "timeline.place", "params": {"item": audio, "track": SONG_TRACK, "frame": 0, "insert": false, "video": false}})];
    for ((from, length), item) in spans(&times, film.duration).into_iter().zip(stills) {
        let start_frame = fps.frame_at(from).max(0);
        let end_frame = fps.frame_at(from + length).max(start_frame + 1);
        let duration_frames = end_frame - start_frame;
        place.push(json!({"id": "timeline.place", "params": {"item": item, "track": BACKGROUND_TRACK, "frame": start_frame, "insert": false, "duration": {"frame": duration_frames}}}));
    }
    for ((from, _), item) in film.characters.iter().zip(shots) {
        let start_frame = fps.frame_at(*from).max(0);
        place.push(json!({"id": "timeline.place", "params": {"item": item, "track": CHARACTER_TRACK, "frame": start_frame, "insert": false, "audio": false}}));
    }
    place.push(json!({"id": "timeline.place", "params": {"item": lyrics, "track": LYRICS_TRACK, "frame": 0, "insert": false, "audio": false}}));
    let placed = run(dir, project, false, &place)?;
    let clips: Vec<u64> = placed[1..=nb].iter().flat_map(|r| ids(r, "clips")).collect();
    run(dir, project, false, &[json!({"id": "clip.fillFrame", "params": {"clips": clips}})])?;
    Ok(())
}

fn export(dir: &Path, project: &Path, name: &str) -> Result<PathBuf> {
    let exports = dir.join("exports");
    std::fs::create_dir_all(&exports)?;
    let out = exports.join(format!("{name}.mp4"));
    crate::cmd::film::export(project, &out, false)?;
    Ok(out)
}

/// Gives the project its one FilmCraft film, with the song on A1, when it has none yet. A film
/// that already exists is linked and left exactly as it is.
pub(crate) fn ensure_song_film(s: &mut Session, audio: &Path) -> Result<PathBuf> {
    let dir = crate::workshop::project_dir(s)?;
    let wanted = crate::cmd::film::film_path(s)?;
    if let Some(existing) = Some(wanted.clone()).filter(|p| p.is_file()).or_else(|| crate::cmd::project::workshop::video_project_path(&dir)) {
        crate::cmd::film::link(s, &existing)?;
        return Ok(existing);
    }
    if let Some(folder) = wanted.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let name = s.project_path.as_ref().and_then(|p| p.file_stem()).map_or("Film".into(), |n| n.to_string_lossy().into_owned());
    let song = dir.join(audio);
    let made = run(
        &dir,
        &wanted,
        true,
        &[
            json!({"id": "file.newProject", "params": {"name": name}}),
            json!({"id": "file.newSequence", "params": {"name": name, "width": 1920, "height": 1080, "fps": FrameRate::FPS_30.as_f64()}}),
            json!({"id": "file.import", "params": {"paths": [arg(&song)]}}),
        ],
    )?;
    let item = made.get(2).map(|r| ids(r, "items")).unwrap_or_default().first().copied().ok_or_else(|| invalid("FilmCraft did not import the song"))?;
    run(&dir, &wanted, false, &[json!({"id": "timeline.place", "params": {"item": item, "track": SONG_TRACK, "frame": 0, "insert": false, "video": false}})])?;
    crate::cmd::film::link(s, &wanted)?;
    Ok(wanted)
}

/// `video.assemble`: build `film/<song>.fcproj` from the song, backgrounds and lettering, then
/// export `exports/<song>.mp4` unless `export` is false.
pub fn cmd_video_assemble(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let film = inputs(s, &dir, p)?;
    let project = crate::cmd::film::film_path(s)?;
    if let Some(folder) = project.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let fps = p.get("fps").and_then(parse_frame_rate).unwrap_or(FrameRate::FPS_30);
    build_film(&dir, &project, &film, fps)?;
    crate::cmd::film::link(s, &project)?;
    let exported = if p["export"] == false { None } else { Some(export(&dir, &project, &film.name)?) };
    let backgrounds: Vec<Value> = film.backgrounds.iter().map(|(from, image)| json!({"from_tick": from.0, "image": image})).collect();
    Ok(json!({
        "ok": true,
        "project": project,
        "export": exported,
        "duration_tick": film.duration.0,
        "frame_rate": frame_rate_json(fps),
        "backgrounds": backgrounds,
        "characters": film.characters.len(),
        "lettering": film.lettering,
    }))
}
