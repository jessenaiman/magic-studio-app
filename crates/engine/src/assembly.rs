//! `video.assemble`: the film put together by the installed editor, driven only through its
//! CLI. The song's audio on A1, the backgrounds on V1 from the times the caller gives, and the
//! lettering render on V2; then an export. The editor project lives in the project folder
//! (`film/<song>.fcproj`) so it can be opened and refined in the editor itself.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const EDITOR_CLI: &str = "filmcraft-cli";

fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

fn arg(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Run editor commands (one JSON object per line) and return each command's result in order.
fn run(dir: &Path, project: &Path, fresh: bool, steps: &[Value]) -> Result<Vec<Value>> {
    let file = project.with_extension("steps.jsonl");
    let mut body: String = steps.iter().map(|s| format!("{s}\n")).collect();
    // Keep every batch the film was built from, in order, beside the project.
    if !fresh && let Ok(earlier) = std::fs::read_to_string(&file) {
        body = earlier + &body;
    }
    std::fs::write(&file, &body)?;
    let batch = project.with_extension("batch.jsonl");
    std::fs::write(
        &batch,
        steps.iter().map(|s| format!("{s}\n")).collect::<String>(),
    )?;
    let mode = if fresh { "--save-as" } else { "--project" };
    let mut args = vec![mode.to_owned(), arg(project)];
    if !fresh {
        args.push("--save".into());
    }
    args.extend(["run".into(), arg(&batch)]);
    let out = crate::workspace::run_tool_in(dir, EDITOR_CLI, &args)?;
    let _ = std::fs::remove_file(&batch);
    out.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .map(|r| {
            if r["ok"] == true {
                Ok(r["result"].clone())
            } else {
                Err(invalid(format!("{EDITOR_CLI} {}: {}", r["id"], r["error"])))
            }
        })
        .collect()
}

fn ids(result: &Value, field: &str) -> Vec<u64> {
    result[field]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .collect()
}

/// The image for one background entry: a registered asset id or a path.
fn background(s: &Session, dir: &Path, entry: &Value) -> Result<PathBuf> {
    if let Some(path) = entry["path"].as_str() {
        let path = dir.join(path);
        return path
            .is_file()
            .then_some(path.clone())
            .ok_or_else(|| invalid(format!("No image at {}", path.display())));
    }
    let id = entry["asset"]
        .as_str()
        .ok_or_else(|| invalid("each background needs asset or path"))?;
    s.project_assets
        .iter()
        .find(|a| a.id == id)
        .and_then(|a| a.path.clone())
        .map(|p| dir.join(p))
        .filter(|p| p.is_file())
        .ok_or_else(|| invalid(format!("No image for asset {id}")))
}

/// Backgrounds in time order with the span each covers, the last one to the end.
fn spans(times: &[f64], duration: f64) -> Vec<(f64, f64)> {
    times
        .iter()
        .enumerate()
        .map(|(i, from)| (*from, times.get(i + 1).copied().unwrap_or(duration) - from))
        .collect()
}

fn resolve_scene_image(
    scene: &Value,
    dir: &Path,
    assets: &[crate::ProjectAsset],
) -> Option<PathBuf> {
    let asset_id = scene["image_asset_id"]
        .as_str()
        .or_else(|| scene["background_asset"].as_str())
        .or_else(|| scene["asset"].as_str());
    if let Some(id) = asset_id
        && let Some(path) = assets
            .iter()
            .find(|a| a.id == id)
            .and_then(|a| a.path.as_ref())
            .map(|p| dir.join(p))
            .filter(|p| p.is_file())
    {
        return Some(path);
    }
    scene["image_path"]
        .as_str()
        .or_else(|| scene["illustration"].as_str())
        .or_else(|| scene["path"].as_str())
        .map(|p| dir.join(p))
        .filter(|p| p.is_file())
}

fn backgrounds_from_storyboard(
    board: &Value,
    dir: &Path,
    assets: &[crate::ProjectAsset],
) -> Vec<(f64, PathBuf)> {
    let list = board["scenes"]
        .as_array()
        .or_else(|| board["animations"].as_array());
    let mut out: Vec<(f64, PathBuf)> = list
        .into_iter()
        .flatten()
        .filter_map(|sc| {
            let start = sc["start_seconds"]
                .as_f64()
                .or_else(|| sc["start"].as_f64())
                .or_else(|| sc["lines"][0]["start"].as_f64())
                .unwrap_or(0.0);
            Some((start, resolve_scene_image(sc, dir, assets)?))
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.dedup_by(|a, b| a.1 == b.1);
    out
}

fn auto_backgrounds(s: &Session, dir: &Path, duration: f64) -> Option<Vec<(f64, PathBuf)>> {
    for rel in ["storyboard/storyboard.json", "design/words.json"] {
        if let Ok(bytes) = std::fs::read(dir.join(rel))
            && let Ok(doc) = serde_json::from_slice::<Value>(&bytes)
        {
            let bgs = backgrounds_from_storyboard(&doc, dir, &s.project_assets);
            if !bgs.is_empty() {
                return Some(bgs);
            }
        }
    }
    let imgs: Vec<PathBuf> = s
        .project_assets
        .iter()
        .filter(|a| a.kind == "image")
        .filter_map(|a| a.path.as_ref().map(|p| dir.join(p)))
        .filter(|p| p.is_file())
        .collect();
    if imgs.is_empty() {
        return None;
    }
    let step = (duration / (imgs.len() as f64)).max(1.0);
    Some(
        imgs.into_iter()
            .enumerate()
            .map(|(i, p)| ((i as f64) * step, p))
            .collect(),
    )
}

/// What the film is made of: song, backgrounds in time order with their images, lettering.
struct Inputs {
    name: String,
    audio: PathBuf,
    lettering: PathBuf,
    backgrounds: Vec<(f64, PathBuf)>,
    duration: f64,
}

fn inputs(s: &Session, dir: &Path, p: &Value) -> Result<Inputs> {
    let name = s
        .project_path
        .as_ref()
        .and_then(|p| p.file_stem())
        .map_or("Film".into(), |n| n.to_string_lossy().into_owned());
    let audio = s
        .song_audio
        .clone()
        .map(|a| dir.join(a))
        .filter(|a| a.is_file())
        .ok_or_else(|| invalid("No song audio in this project"))?;
    let lettering = dir.join(p["lettering"].as_str().unwrap_or("lettering/lyrics.mov"));
    if !lettering.is_file() {
        return Err(invalid(format!(
            "No lettering render at {}; run words.letter first",
            lettering.display()
        )));
    }
    let duration = crate::workshop::song_seconds(dir)
        .ok_or_else(|| invalid("Song length unknown; analyze the song first"))?;
    let backgrounds = if let Some(entries) = p["backgrounds"].as_array().filter(|b| !b.is_empty()) {
        let mut bgs = entries
            .iter()
            .map(|e| Ok((e["from"].as_f64().unwrap_or(0.0), background(s, dir, e)?)))
            .collect::<Result<Vec<_>>>()?;
        bgs.sort_by(|a, b| a.0.total_cmp(&b.0));
        bgs
    } else {
        auto_backgrounds(s, dir, duration).ok_or_else(|| {
            let known: Vec<&str> = s.project_assets.iter().map(|a| a.id.as_str()).collect();
            invalid(format!(
                "backgrounds: [{{\"asset\":id,\"from\":seconds}}, ...] is required; assets: {}",
                known.join(", ")
            ))
        })?
    };
    Ok(Inputs {
        name,
        audio,
        lettering,
        backgrounds,
        duration,
    })
}

/// The editor project: a sequence with the audio on A1, backgrounds filling V1 by time, and
/// the lettering on V2.
fn build_film(dir: &Path, project: &Path, film: &Inputs, fps: f64) -> Result<()> {
    let mut media = vec![arg(&film.audio)];
    media.extend(film.backgrounds.iter().map(|(_, i)| arg(i)));
    media.push(arg(&film.lettering));
    let made = run(
        dir,
        project,
        true,
        &[
            json!({"id": "file.newProject", "params": {"name": film.name}}),
            json!({"id": "file.newSequence", "params": {"name": film.name, "width": 1920, "height": 1080, "fps": fps}}),
            json!({"id": "file.import", "params": {"paths": media}}),
        ],
    )?;
    let items = made.get(2).map(|r| ids(r, "items")).unwrap_or_default();
    if items.len() != media.len() {
        return Err(invalid(format!(
            "{EDITOR_CLI} imported {} of {} files",
            items.len(),
            media.len()
        )));
    }
    let (audio, lyrics, stills) = (items[0], items[items.len() - 1], &items[1..items.len() - 1]);
    let times: Vec<f64> = film.backgrounds.iter().map(|(t, _)| *t).collect();
    let mut place = vec![
        json!({"id": "timeline.place", "params": {"item": audio, "track": "A1", "seconds": 0, "insert": false, "video": false}}),
    ];
    for ((from, length), item) in spans(&times, film.duration).into_iter().zip(stills) {
        place.push(json!({"id": "timeline.place", "params": {"item": item, "track": "V1", "seconds": from, "insert": false, "duration": {"seconds": length}}}));
    }
    place.push(json!({"id": "timeline.place", "params": {"item": lyrics, "track": "V2", "seconds": 0, "insert": false, "audio": false}}));
    let placed = run(dir, project, false, &place)?;
    let clips: Vec<u64> = placed[1..placed.len() - 1]
        .iter()
        .flat_map(|r| ids(r, "clips"))
        .collect();
    run(
        dir,
        project,
        false,
        &[json!({"id": "clip.fillFrame", "params": {"clips": clips}})],
    )?;
    Ok(())
}

fn export(dir: &Path, project: &Path, name: &str) -> Result<PathBuf> {
    let exports = dir.join("exports");
    std::fs::create_dir_all(&exports)?;
    let out = exports.join(format!("{name}.mp4"));
    let args = [
        "--project".into(),
        arg(project),
        "export".into(),
        arg(&out),
        "--format".into(),
        "h264".into(),
    ];
    crate::workspace::run_tool_in(dir, EDITOR_CLI, &args)?;
    Ok(out)
}

/// `video.assemble`: build `film/<song>.fcproj` from the song, backgrounds and lettering, then
/// export `exports/<song>.mp4` unless `export` is false.
pub fn cmd_video_assemble(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let film = inputs(s, &dir, p)?;
    let folder = dir.join("film");
    std::fs::create_dir_all(&folder)?;
    let project = folder.join(format!("{}.fcproj", film.name));
    build_film(&dir, &project, &film, p["fps"].as_f64().unwrap_or(30.0))?;
    let exported = if p["export"] == false {
        None
    } else {
        Some(export(&dir, &project, &film.name)?)
    };
    let backgrounds: Vec<Value> = film
        .backgrounds
        .iter()
        .map(|(from, image)| json!({"from": from, "image": image}))
        .collect();
    Ok(
        json!({"ok": true, "project": project, "export": exported, "duration": film.duration,
        "backgrounds": backgrounds, "lettering": film.lettering}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_background_runs_until_the_next_and_the_last_to_the_end() {
        let got = spans(&[0.0, 7.74, 14.91], 33.0);
        let want = [(0.0, 7.74), (7.74, 7.17), (14.91, 18.09)];
        assert_eq!(got.len(), want.len());
        for ((a, b), (c, d)) in got.into_iter().zip(want) {
            assert!(
                (a - c).abs() < 1e-9 && (b - d).abs() < 1e-9,
                "{a},{b} vs {c},{d}"
            );
        }
    }

    #[test]
    fn storyboard_backgrounds_resolve_scene_start_times_and_collapse_adjacent_duplicates() {
        let dir = std::env::temp_dir().join(format!("magic_assembly_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let img1 = dir.join("map.png");
        let img2 = dir.join("kitchen.png");
        std::fs::write(&img1, b"png1").unwrap();
        std::fs::write(&img2, b"png2").unwrap();
        let assets = vec![
            crate::ProjectAsset {
                id: "map".into(),
                name: "Map".into(),
                kind: "image".into(),
                path: Some("map.png".into()),
                preset: None,
                parts: Vec::new(),
            },
            crate::ProjectAsset {
                id: "kitchen".into(),
                name: "Kitchen".into(),
                kind: "image".into(),
                path: Some("kitchen.png".into()),
                preset: None,
                parts: Vec::new(),
            },
        ];
        let board = json!({
            "scenes": [
                {"start_seconds": 0.0, "image_asset_id": "map"},
                {"start_seconds": 4.2, "image_asset_id": "map"},
                {"start_seconds": 8.5, "image_asset_id": "kitchen"},
                {"start_seconds": 14.0, "image_path": "map.png"}
            ]
        });
        let bgs = backgrounds_from_storyboard(&board, &dir, &assets);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            bgs,
            vec![
                (0.0, img1.clone()),
                (8.5, img2),
                (14.0, img1),
            ]
        );
    }
}
