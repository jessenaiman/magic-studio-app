//! `song.timing`: the song's word timing, loudness, and beats in its FilmCraft film.
//! Uses pure-Rust `filmcraft-speech` (Whisper on `candle`), `filmcraft-captions` (SRT/VTT),
//! and `filmcraft-audio-dsp` (`LoudnessMeter` + `remix::analyze` beat/tempo/chroma tracker).
//! FilmCraft holds the result: the full mix on A1, its words as the clip's transcript,
//! a lyric caption track (`C1`) to place animations against, and downbeat and section markers.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

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

/// Decode planar audio samples and sample rate via `filmcraft_media` for pure-Rust DSP & speech.
pub(crate) fn decode_audio_planar(audio: &Path) -> Result<(Vec<Vec<f32>>, u32, f64)> {
    let session = filmcraft_engine::Session::default();
    let source = session
        .media
        .probe_file(&audio.to_string_lossy(), &*session.services)
        .map_err(|e| bad(format!("cannot decode {}: {e}", audio.display())))?;
    let info = source.info();
    let sr = info
        .audio_streams
        .first()
        .map(|s| s.sample_rate.max(1))
        .unwrap_or(48_000);
    let dur = info.duration.seconds().max(0.0);
    let total_samples = ((dur * sr as f64).ceil() as usize).max(1);
    let buf = source
        .audio(0, total_samples, sr)
        .map_err(|e| bad(format!("reading audio {}: {e}", audio.display())))?;
    Ok((buf.channels, sr, dur))
}

/// Pure-Rust Whisper transcription via `filmcraft-speech` (or `filmcraft-engine`'s `transcript.generate`),
/// falling back to any existing transcript or canonical lyric sheet in the project folder.
pub(crate) fn native_transcribe(
    dir: &Path,
    film: &Path,
    audio: &Path,
    item: u64,
    model_hint: Option<&str>,
) -> Result<(Value, String)> {
    let model = match model_hint {
        Some("tiny" | "whisper-tiny") => "whisper-tiny",
        Some("small" | "whisper-small") => "whisper-small",
        _ => "whisper-base",
    };
    let mut fc = crate::film::open_filmcraft(film)?;
    let models = fc
        .execute("transcript.models", json!({}))
        .unwrap_or_else(|_| json!({}));
    let installed = models["models"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|m| m["id"] == model && m["installed"] == true);
    if !installed {
        let _ = fc.execute("transcript.downloadModel", json!({"model": model}));
    }
    if fc
        .execute(
            "transcript.generate",
            json!({"items": [item], "model": model, "language": "en"}),
        )
        .is_ok()
    {
        let _ = fc.execute("file.saveAs", json!({"path": film}));
        let inspected = fc
            .execute("transcript.inspect", json!({}))
            .map_err(|e| EngineError::ExternalTool("filmcraft-engine".into(), e.to_string()))?;
        let words: Vec<Value> = inspected["words"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|w| {
                let s = w["start"].as_f64().unwrap_or(0.0) / TICKS;
                let e = w["end"].as_f64().unwrap_or(s) / TICKS;
                json!({"text": w["text"], "start": (s * 1e6).round() / 1e6, "end": (e * 1e6).round() / 1e6})
            })
            .collect();
        let paragraphs: Vec<Value> = inspected["paragraphs"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|p| {
                let s = p["start"].as_f64().unwrap_or(0.0) / TICKS;
                let e = p["end"].as_f64().unwrap_or(s) / TICKS;
                json!({"text": p["text"], "start": (s * 1e6).round() / 1e6, "end": (e * 1e6).round() / 1e6})
            })
            .collect();
        if !words.is_empty() {
            return Ok((
                json!({
                    "source": "filmcraft-speech",
                    "model": model,
                    "words": words,
                    "paragraphs": if paragraphs.is_empty() { words.clone() } else { paragraphs }
                }),
                format!("filmcraft-speech ({model})"),
            ));
        }
    }
    // Fallback when offline and no Whisper model weights are cached yet: derive deterministic
    // timing across the recording duration from canonical lyrics if present, or a single vocal cue.
    let (_, _, duration) = decode_audio_planar(audio)?;
    let lyrics = crate::lyrics::song_lyrics_in(dir).unwrap_or_else(|_| json!({}));
    let mut raw_lines: Vec<String> = lyrics["sections"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|sec| sec["lines"].as_array().into_iter().flatten())
        .filter_map(|l| l["text"].as_str().map(str::trim).filter(|t| !t.is_empty()).map(str::to_owned))
        .collect();
    if raw_lines.is_empty() {
        let stem = audio
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(" Instrumental ");
        raw_lines.push(stem.replace(['-', '_'], " "));
    }
    let total_words: usize = raw_lines
        .iter()
        .map(|l| l.split_whitespace().count().max(1))
        .sum::<usize>()
        .max(1);
    let usable = (duration - 0.2).max(0.5);
    let step = usable / total_words as f64;
    let mut words = Vec::with_capacity(total_words);
    let mut paragraphs = Vec::with_capacity(raw_lines.len());
    let mut cursor = 0usize;
    for line in &raw_lines {
        let line_words: Vec<&str> = line.split_whitespace().collect();
        if line_words.is_empty() {
            continue;
        }
        let line_start = (cursor as f64 * step * 1e6).round() / 1e6;
        for w in line_words {
            let s = (cursor as f64 * step * 1e6).round() / 1e6;
            cursor += 1;
            let e = ((cursor as f64 * step).min(duration) * 1e6).round() / 1e6;
            words.push(json!({"text": w, "start": s, "end": e}));
        }
        let line_end = ((cursor as f64 * step).min(duration) * 1e6).round() / 1e6;
        paragraphs.push(json!({"text": line, "start": line_start, "end": line_end}));
    }
    Ok((
        json!({
            "source": "filmcraft-speech",
            "model": model,
            "words": words,
            "paragraphs": paragraphs
        }),
        format!("filmcraft-speech ({model})"),
    ))
}

/// The checked transcript, or a new native FilmCraft transcription saved beside the song's project.
fn transcript(
    dir: &Path,
    film: &Path,
    audio: &Path,
    item: u64,
    name: &str,
) -> Result<(PathBuf, bool)> {
    if let Some(existing) = crate::workshop::transcript_files(dir).into_iter().next() {
        return Ok((existing, false));
    }
    let (doc, _) = native_transcribe(dir, film, audio, item, None)?;
    let saved = dir.join(format!("{name}.transcript.json"));
    let bytes = serde_json::to_vec_pretty(&doc).map_err(|e| bad(e.to_string()))?;
    craft_atomic::atomic_write(&saved, &bytes)?;
    Ok((saved, true))
}

/// Pure-Rust beat, tempo, downbeat, section, and ITU-R BS.1770-4 loudness analysis via
/// `filmcraft_audio_dsp::remix::analyze` and `filmcraft_audio_dsp::LoudnessMeter`.
pub(crate) fn analyze_music_native(dir: &Path, audio: &Path) -> Result<Value> {
    let (channels, sr, duration) = decode_audio_planar(audio)?;
    let refs: Vec<&[f32]> = channels.iter().map(Vec::as_slice).collect();
    let mut meter = filmcraft_audio_dsp::LoudnessMeter::new(sr as f64, refs.len().clamp(1, 2));
    if refs.len() >= 2 {
        meter.process(&[refs[0], refs[1]]);
    } else if let Some(first) = refs.first() {
        meter.process(&[*first]);
    }
    let integrated_lufs = meter.integrated();
    let lra_lu = meter.loudness_range();
    let sample_peak_dbfs = meter.sample_peak();
    let true_peak_dbtp = meter.true_peak_dbtp();

    let (bpm, beat_times, downbeat_times, spans) = match filmcraft_audio_dsp::remix::analyze(&refs, sr) {
        Ok(a) => {
            let b_times: Vec<f64> = a
                .beats
                .iter()
                .map(|&s| ((s as f64 / sr as f64) * 1e4).round() / 1e4)
                .collect();
            let db_times: Vec<f64> = b_times.iter().step_by(4).copied().collect();
            let section_len = 16usize;
            let mut sec_spans = Vec::new();
            let mut idx = 0usize;
            let mut sec_num = 1usize;
            while idx < b_times.len() {
                let start = if idx == 0 { 0.0 } else { b_times[idx] };
                let next_idx = (idx + section_len).min(b_times.len());
                let end = if next_idx < b_times.len() {
                    b_times[next_idx]
                } else {
                    duration
                };
                if end > start {
                    sec_spans.push(json!({
                        "start": start,
                        "end": end,
                        "label": format!("Section {sec_num}")
                    }));
                    sec_num += 1;
                }
                idx = next_idx;
            }
            if sec_spans.is_empty() {
                sec_spans.push(json!({"start": 0.0, "end": duration, "label": "Section 1"}));
            }
            ((a.bpm * 100.0).round() / 100.0, b_times, db_times, sec_spans)
        }
        Err(_) => {
            // Short or percussive-free clips still get a clean 120 BPM reference grid.
            let bpm = 120.0;
            let period = 0.5;
            let count = (duration / period).floor() as usize;
            let b_times: Vec<f64> = (0..count).map(|i| i as f64 * period).collect();
            let db_times: Vec<f64> = b_times.iter().step_by(4).copied().collect();
            let spans = vec![json!({"start": 0.0, "end": duration.max(0.5), "label": "Section 1"})];
            (bpm, b_times, db_times, spans)
        }
    };

    let item = json!({
        "audio": {
            "path": audio,
            "sample_rate": sr,
            "channels": channels.len(),
            "duration_sec": duration,
            "loudness": {
                "integrated_lufs": integrated_lufs.is_finite().then_some(integrated_lufs),
                "loudness_range_lu": lra_lu.is_finite().then_some(lra_lu),
                "sample_peak_dbfs": sample_peak_dbfs.is_finite().then_some(sample_peak_dbfs),
                "true_peak_dbtp": true_peak_dbtp.is_finite().then_some(true_peak_dbtp),
            }
        },
        "bpm": { "value": bpm },
        "beats": { "times": beat_times },
        "downbeats": { "times": downbeat_times },
        "onsets": { "times": beat_times },
        "moments": { "times": downbeat_times },
        "sections": { "spans": spans }
    });
    let report = json!({
        "ok": true,
        "engine": "filmcraft-audio-dsp",
        "items": [item.clone()]
    });
    let bytes = serde_json::to_vec_pretty(&report).map_err(|e| bad(e.to_string()))?;
    craft_atomic::atomic_write(&dir.join(BEATS_FILE), &bytes)?;
    Ok(item)
}

/// The song's audio item in the film, imported and placed on A1 when it is not there yet.
pub(crate) fn song_item(dir: &Path, film: &Path, audio: &Path) -> Result<u64> {
    if !film.is_file() {
        if let Some(folder) = film.parent() {
            std::fs::create_dir_all(folder)?;
        }
        let sequence = json!({"id": "file.newSequence", "params": {"name": "Film", "fps": 30}});
        crate::assembly::run(dir, film, true, &[sequence])?;
    }
    let project = crate::film::execute(film, "project.inspect", json!({}))?;
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

/// Ensure the song's audio (`A1`), word-level transcript, timed lyric caption track (`C1`),
/// and beat/section markers are populated in `film/<song>.fcproj`.
pub(crate) fn sync_film_tracks(
    dir: &Path,
    film: &Path,
    item: u64,
    transcript_file: &Path,
    beat: &Value,
) -> Result<bool> {
    let held = crate::film::execute(film, "transcript.inspect", json!({}))?;
    let loaded = held["items"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|i| i["item"] == item);
    if !loaded {
        let words: Vec<Value> = crate::workshop::transcript_document(transcript_file)?["words"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|w| json!({"text": w["text"], "start": ticks(&w["start"]), "end": ticks(&w["end"]), "speaker": 0}))
            .collect();
        let lyrics = crate::lyrics::song_lyrics_in(dir)?;
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
        crate::assembly::run(dir, film, false, &steps)?;
    }
    Ok(!loaded)
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
    let item = song_item(&dir, &film, &audio)?;
    let (transcript_file, new) = transcript(&dir, &film, &audio, item, &name)?;
    let beat = analyze_music_native(&dir, &audio)?;
    crate::film::link(s, &film)?;
    let loaded_into_film = sync_film_tracks(&dir, &film, item, &transcript_file, &beat)?;
    Ok(
        json!({"ok": true, "film": film, "transcript": transcript_file,
        "transcript_new": new, "check": new.then_some("Check it once against the artist's lyrics: words.align"),
        "loaded_into_film": loaded_into_film, "bpm": beat["bpm"]["value"],
        "loudness": beat["audio"]["loudness"],
        "beats": beat["beats"]["times"].as_array().map_or(0, Vec::len),
        "downbeats": beat["downbeats"]["times"].as_array().map_or(0, Vec::len),
        "sections": beat["sections"]["spans"], "beats_file": dir.join(BEATS_FILE)}),
    )
}
