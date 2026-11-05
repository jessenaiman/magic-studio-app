//! `song.timing`: the song's word timing and beats, in its FilmCraft film. HyperFrames runs
//! headless (no project, no preview server); FilmCraft holds the result: the full mix on A1, its
//! words as the clip's transcript, a lyric caption track to place animations against (captions
//! are left out of exports unless burned in), and downbeat and section markers.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

// ---- The CLIs and files this file uses -------------------------------------------------------

/// `hyperframes transcribe AUDIO --model medium.en --language en --engine whisper --json -d DIR`
/// writes DIR/transcript.json, one entry per word. Only for a song with no transcript yet; the
/// result is checked once against the artist's lyrics (`words.align`).
const HYPERFRAMES: &str = "hyperframes";
const TRANSCRIBE: [&str; 7] = [
    "--model",
    "medium.en",
    "--language",
    "en",
    "--engine",
    "whisper",
    "--json",
];
/// `hyperframes audio AUDIO --only beats,downbeats,bpm,sections --quality local --json`.
/// `local` never uploads (the `best` quality uses HeyGen's paid services).
const BEATS: [&str; 5] = [
    "--only",
    "beats,downbeats,bpm,sections",
    "--quality",
    "local",
    "--json",
];
/// FilmCraft document commands execute in-process through its Rust engine.
const FILMCRAFT_CLI: &str = "FilmCraft Rust API";
const TICKS: f64 = 254_016_000_000.0;
/// In the project folder.
const BEATS_FILE: &str = "beats.json";

fn bad(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

fn ticks(seconds: &Value) -> Value {
    json!((seconds.as_f64().unwrap_or(0.0) * TICKS).round() as i64)
}

fn arg(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The one way this file runs HyperFrames: on failure, say exactly what to rerun and check.
fn hyperframes(dir: &Path, args: &[String]) -> Result<String> {
    crate::workspace::run_tool_in(dir, HYPERFRAMES, args).map_err(|error| {
        let line = std::iter::once(HYPERFRAMES.to_owned())
            .chain(
                args.iter()
                    .map(|a| format!("'{}'", a.replace('\'', "'\\''"))),
            )
            .collect::<Vec<_>>()
            .join(" ");
        let detail = match error {
            EngineError::ExternalTool(_, detail) => detail,
            other => other.to_string(),
        };
        EngineError::ExternalTool(
            HYPERFRAMES.into(),
            format!("{detail}\nRerun it yourself: {line}\nCheck the install: {HYPERFRAMES} doctor"),
        )
    })
}

// ---- Steps ------------------------------------------------------------------------------------

/// The checked transcript, or a new HyperFrames transcription saved beside the song's project.
fn transcript(dir: &Path, audio: &Path, name: &str) -> Result<(PathBuf, bool)> {
    if let Some(existing) = crate::workshop::transcript_files(dir).into_iter().next() {
        return Ok((existing, false));
    }
    let work = dir.join(".agent/transcribe");
    std::fs::create_dir_all(&work)?;
    let mut args = vec!["transcribe".to_owned(), arg(audio)];
    args.extend(TRANSCRIBE.map(String::from));
    args.extend(["-d".into(), arg(&work)]);
    hyperframes(dir, &args)?;
    let saved = dir.join(format!("{name}.transcript.json"));
    std::fs::rename(work.join("transcript.json"), &saved)?;
    Ok((saved, true))
}

fn beats(dir: &Path, audio: &Path) -> Result<Value> {
    let mut args = vec!["audio".to_owned(), arg(audio)];
    args.extend(BEATS.map(String::from));
    let out = hyperframes(dir, &args)?;
    std::fs::write(dir.join(BEATS_FILE), &out)?;
    let report: Value =
        serde_json::from_str(&out).map_err(|e| bad(format!("{HYPERFRAMES} audio: {e}")))?;
    Ok(report["items"][0].clone())
}

/// The song's audio item in the film, imported and placed on A1 when it is not there yet.
fn song_item(dir: &Path, film: &Path, audio: &Path) -> Result<u64> {
    if !film.is_file() {
        if let Some(folder) = film.parent() {
            std::fs::create_dir_all(folder)?;
        }
        let sequence = json!({"id": "file.newSequence", "params": {"name": "Film", "fps": 30}});
        crate::assembly::run(dir, film, true, &[sequence])?;
    }
    let project = crate::filmcraft::execute(film, "project.inspect", json!({}))?;
    let name = audio
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let found = project["root"]["children"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|i| i["type"] == "Audio" && i["name"] == name)
        .and_then(|i| i["item"].as_u64());
    if let Some(item) = found {
        return Ok(item);
    }
    let imported = crate::assembly::run(
        dir,
        film,
        false,
        &[json!({"id": "file.import", "params": {"paths": [audio]}})],
    )?;
    let item = imported
        .first()
        .and_then(|r| crate::assembly::ids(r, "items").first().copied())
        .ok_or_else(|| {
            bad(format!(
                "{FILMCRAFT_CLI} did not import {}",
                audio.display()
            ))
        })?;
    let place = json!({"id": "timeline.place", "params": {"item": item, "track": "A1", "seconds": 0, "insert": false, "video": false}});
    crate::assembly::run(dir, film, false, &[place])?;
    Ok(item)
}

/// `song.timing`: transcript (checked, or new), beats, and both in the film. Run once per film:
/// a song whose clip already has a transcript is left as it is.
pub fn cmd_song_timing(s: &mut Session, _: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let audio = s
        .song_audio
        .clone()
        .filter(|a| a.is_file())
        .ok_or_else(|| bad("No song audio in this project"))?;
    let film = crate::film::film_path(s)?;
    let name = film
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("song")
        .to_owned();
    let (transcript_file, new) = transcript(&dir, &audio, &name)?;
    let beat = beats(&dir, &audio)?;
    let item = song_item(&dir, &film, &audio)?;
    crate::film::link(s, &film)?;
    let held = crate::filmcraft::execute(&film, "transcript.inspect", json!({}))?;
    let loaded = held["items"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|i| i["item"] == item);
    if !loaded {
        let words: Vec<Value> = crate::workshop::transcript_document(&transcript_file)?["words"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|w| json!({"text": w["text"], "start": ticks(&w["start"]), "end": ticks(&w["end"]), "speaker": 0}))
            .collect();
        let lyrics = crate::lyrics::song_lyrics_in(&dir)?;
        let artist = lyrics["artist"].as_str().unwrap_or("Singer");
        let mut steps = vec![
            json!({"id": "transcript.set", "params": {"item": item, "transcript": {"language": "en", "speakers": [{"name": artist}], "words": words}}}),
            json!({"id": "transcript.createCaptions", "params": {"lines": 1, "name": "Lyrics"}}),
        ];
        steps.extend(beat["downbeats"]["times"].as_array().into_iter().flatten().map(|t| {
            json!({"id": "markers.add", "params": {"target": "program", "time": ticks(t), "name": "Downbeat"}})
        }));
        steps.extend(beat["sections"]["spans"].as_array().into_iter().flatten().map(|span| {
            json!({"id": "markers.addChapter", "params": {"time": ticks(&span["start"]), "name": span["label"]}})
        }));
        crate::assembly::run(&dir, &film, false, &steps)?;
    }
    Ok(
        json!({"ok": true, "film": film, "transcript": transcript_file,
        "transcript_new": new, "check": new.then_some("Check it once against the artist's lyrics: words.align"),
        "loaded_into_film": !loaded, "bpm": beat["bpm"]["value"],
        "beats": beat["beats"]["times"].as_array().map_or(0, Vec::len),
        "downbeats": beat["downbeats"]["times"].as_array().map_or(0, Vec::len),
        "sections": beat["sections"]["spans"], "beats_file": dir.join(BEATS_FILE)}),
    )
}
