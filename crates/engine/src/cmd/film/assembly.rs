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

struct ClipSpec {
    from: Tick,
    to: Option<Tick>,
    source_in: Option<Tick>,
    path: PathBuf,
    name: Option<String>,
}

struct Inputs {
    name: String,
    audio: PathBuf,
    lettering: Option<PathBuf>,
    backgrounds: Vec<ClipSpec>,
    characters: Vec<ClipSpec>,
    lyrics_clips: Vec<ClipSpec>,
    duration: Tick,
}

fn parse_clip_list(dir: &Path, entries: Option<&Vec<Value>>, path_key: &str) -> Result<Vec<ClipSpec>> {
    let mut out = Vec::new();
    for entry in entries.into_iter().flatten() {
        let raw_path = entry[path_key].as_str().or_else(|| entry["path"].as_str());
        let clip = raw_path
            .map(|c| {
                let p = PathBuf::from(c);
                if p.is_absolute() { p } else { dir.join(p) }
            })
            .filter(|c| c.is_file());
        let clip = clip.ok_or_else(|| invalid(format!("{path_key} clip missing: {}", entry[path_key])))?;
        let from = entry.get("from_tick").or_else(|| entry.get("from")).or_else(|| entry.get("start")).and_then(parse_tick).unwrap_or(Tick::ZERO);
        let to = entry.get("to_tick").or_else(|| entry.get("to")).or_else(|| entry.get("end")).and_then(parse_tick);
        let source_in = entry.get("source_in_tick").or_else(|| entry.get("source_in")).and_then(parse_tick);
        let name = entry["name"].as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned);
        out.push(ClipSpec { from, to, source_in, path: clip, name });
    }
    out.sort_by_key(|a| a.from);
    Ok(out)
}

fn inputs(s: &Session, dir: &Path, p: &Value) -> Result<Inputs> {
    let name = s.project_path.as_ref().and_then(|p| p.file_stem()).map_or("Film".into(), |n| n.to_string_lossy().into_owned());
    let audio = s.song_audio.clone().map(|a| dir.join(a)).filter(|a| a.is_file()).ok_or_else(|| invalid("No song audio in this project"))?;
    let lettering_candidate = dir.join(p["lettering"].as_str().unwrap_or("lettering/lyrics.mov"));
    // No silent skip: missing lettering is an error unless the caller
    // explicitly passes `"lettering": null`.
    let explicit_skip = p.get("lettering").is_some_and(|v| v.is_null());
    let duration = crate::workshop::song_ticks(dir)
        .or_else(|| crate::cmd::film::media_duration_ticks(&audio, false).ok())
        .ok_or_else(|| invalid("Song length unknown; analyze the song first"))?;
    let backgrounds = if let Some(entries) = p["backgrounds"].as_array().filter(|b| !b.is_empty()) {
        let mut bgs = entries
            .iter()
            .map(|e| {
                let from = e.get("from_tick").or_else(|| e.get("from")).or_else(|| e.get("start")).and_then(parse_tick).unwrap_or(Tick::ZERO);
                let to = e.get("to_tick").or_else(|| e.get("to")).or_else(|| e.get("end")).and_then(parse_tick);
                let name = e["name"].as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned);
                Ok(ClipSpec { from, to, source_in: None, path: background(s, dir, e)?, name })
            })
            .collect::<Result<Vec<_>>>()?;
        bgs.sort_by_key(|a| a.from);
        bgs
    } else {
        auto_backgrounds(s, dir, duration)
            .ok_or_else(|| {
                let known: Vec<&str> = s.project_assets.iter().map(|a| a.id.as_str()).collect();
                invalid(format!("backgrounds: [{{\"asset\":id,\"from_tick\":ticks}}, ...] is required; assets: {}", known.join(", ")))
            })?
            .into_iter()
            .map(|(from, path)| ClipSpec { from, to: None, source_in: None, path, name: None })
            .collect()
    };
    let characters = parse_clip_list(dir, p["characters"].as_array(), "clip")?;
    let lyrics_clips = parse_clip_list(dir, p["lyrics"].as_array(), "clip")?;
    let lettering = if lettering_candidate.is_file() {
        Some(lettering_candidate.clone())
    } else if explicit_skip {
        None
    } else if !lyrics_clips.is_empty() {
        // `lyrics` clips replace the V3 lettering render. Not an error.
        None
    } else {
        return Err(invalid(format!("No lettering render at {}; run words.letter first", lettering_candidate.display())));
    };
    Ok(Inputs { name, audio, lettering, backgrounds, characters, lyrics_clips, duration })
}

fn build_film(dir: &Path, project: &Path, film: &Inputs, fps: FrameRate) -> Result<()> {
    let fps = fps.sane();
    let mut unique_paths: Vec<PathBuf> = Vec::new();
    let mut push_unique = |p: &Path| -> usize {
        if let Some(idx) = unique_paths.iter().position(|existing| existing == p) {
            idx
        } else {
            let idx = unique_paths.len();
            unique_paths.push(p.to_path_buf());
            idx
        }
    };
    let audio_idx = push_unique(&film.audio);
    let bg_indices: Vec<usize> = film.backgrounds.iter().map(|b| push_unique(&b.path)).collect();
    let lettering_idx: Option<usize> = film.lettering.as_deref().map(&mut push_unique);
    let char_indices: Vec<usize> = film.characters.iter().map(|c| push_unique(&c.path)).collect();
    let lyr_indices: Vec<usize> = film.lyrics_clips.iter().map(|l| push_unique(&l.path)).collect();

    let media: Vec<String> = unique_paths.iter().map(|p| arg(p)).collect();
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

    let times: Vec<Tick> = film.backgrounds.iter().map(|b| b.from).collect();
    let default_spans = spans(&times, film.duration);
    let mut place =
        vec![json!({"id": "timeline.place", "params": {"item": items[audio_idx], "track": SONG_TRACK, "frame": 0, "insert": false, "video": false}})];
    let mut names_for_placements: Vec<Option<String>> = vec![None];

    for ((bg, (from, default_len)), &item_idx) in film.backgrounds.iter().zip(default_spans).zip(&bg_indices) {
        let length = bg.to.map(|to| (to - from).max(Tick::ZERO)).unwrap_or(default_len);
        let start_frame = fps.frame_at(from).max(0);
        let end_frame = fps.frame_at(from + length).max(start_frame + 1);
        let duration_frames = end_frame - start_frame;
        place.push(json!({
            "id": "timeline.place",
            "params": {
                "item": items[item_idx],
                "track": BACKGROUND_TRACK,
                "frame": start_frame,
                "insert": false,
                "duration": {"frame": duration_frames}
            }
        }));
        names_for_placements.push(bg.name.clone());
    }

    for (ch, &item_idx) in film.characters.iter().zip(&char_indices) {
        let start_frame = fps.frame_at(ch.from).max(0);
        let mut params = json!({"item": items[item_idx], "track": CHARACTER_TRACK, "frame": start_frame, "insert": false, "audio": false});
        if let Some(to) = ch.to {
            let end_frame = fps.frame_at(to).max(start_frame + 1);
            params["duration"] = json!({"frame": end_frame - start_frame});
            params["sourceIn"] = json!(ch.source_in.unwrap_or(ch.from).0);
        }
        place.push(json!({"id": "timeline.place", "params": params}));
        names_for_placements.push(ch.name.clone());
    }

    if !film.lyrics_clips.is_empty() {
        for (lyr, &item_idx) in film.lyrics_clips.iter().zip(&lyr_indices) {
            let start_frame = fps.frame_at(lyr.from).max(0);
            let mut params = json!({"item": items[item_idx], "track": LYRICS_TRACK, "frame": start_frame, "insert": false, "audio": false});
            if let Some(to) = lyr.to {
                let end_frame = fps.frame_at(to).max(start_frame + 1);
                params["duration"] = json!({"frame": end_frame - start_frame});
                params["sourceIn"] = json!(lyr.source_in.unwrap_or(lyr.from).0);
            }
            place.push(json!({"id": "timeline.place", "params": params}));
            names_for_placements.push(lyr.name.clone());
        }
    } else if let Some(l_idx) = lettering_idx {
        place.push(json!({"id": "timeline.place", "params": {"item": items[l_idx], "track": LYRICS_TRACK, "frame": 0, "insert": false, "audio": false}}));
        names_for_placements.push(Some("Kinetic Lyrics".into()));
    }

    let placed = run(dir, project, false, &place)?;
    let nb = film.backgrounds.len();
    let bg_clips: Vec<u64> = placed[1..=nb].iter().flat_map(|r| ids(r, "clips")).collect();
    let mut post_steps = vec![json!({"id": "clip.fillFrame", "params": {"clips": bg_clips}})];

    for (res, custom_name) in placed.iter().zip(names_for_placements.iter()) {
        if let Some(name) = custom_name
            && let Some(&clip_id) = ids(res, "clips").first()
        {
            post_steps.push(json!({"id": "clip.rename", "params": {"clip": clip_id, "name": name}}));
        }
    }
    if !film.characters.is_empty() {
        post_steps.push(json!({"id": "timeline.setTrack", "params": {"track": CHARACTER_TRACK, "name": "Character Animation", "enabled": true}}));
    }
    if !film.lyrics_clips.is_empty() || lettering_idx.is_some() {
        post_steps.push(json!({"id": "timeline.setTrack", "params": {"track": LYRICS_TRACK, "name": "Kinetic Lyrics", "enabled": true}}));
    }
    run(dir, project, false, &post_steps)?;
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
    if wanted.is_file() {
        crate::cmd::film::link(s, &wanted)?;
        return Ok(wanted);
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
    let backgrounds: Vec<Value> = film.backgrounds.iter().map(|b| json!({"from_tick": b.from.0, "image": b.path, "name": b.name})).collect();
    Ok(json!({
        "ok": true,
        "project": project,
        "export": exported,
        "duration_tick": film.duration.0,
        "frame_rate": frame_rate_json(fps),
        "backgrounds": backgrounds,
        "characters": film.characters.len(),
        "lyrics": film.lyrics_clips.len(),
        "lettering": film.lettering,
    }))
}
