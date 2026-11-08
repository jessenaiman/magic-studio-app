//! Workshop Stage 2 — Transcript validation, SRT caption parsing/export,
//! provenance receipts, and FilmCraft audio/speech analysis in [`Tick`]s.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::cmd::project::workshop::{project_dir, transcript_files, workshop_status};
use crate::time::{TICKS_PER_SECOND, Tick, parse_tick};
use crate::workspace::resolve_workspace_path;
use crate::{EngineError, Result, Session};

pub(crate) fn transcript_document(path: &Path) -> Result<Value> {
    let mut doc: Value =
        serde_json::from_slice(&std::fs::read(path)?).map_err(|e| EngineError::InvalidParams(format!("Invalid transcript {}: {e}", path.display())))?;
    if doc.is_array() {
        doc = json!({"words":doc});
    }
    if !doc.is_object() {
        return Err(EngineError::InvalidParams("Transcript must be a word array or object with words".into()));
    }
    if !doc["paragraphs"].is_array() {
        let captions = path.with_file_name("transcript.srt");
        doc["paragraphs"] = if captions.is_file() { json!(import_caption_paragraphs(&std::fs::read_to_string(captions)?)?) } else { doc["words"].clone() };
    }
    Ok(doc)
}

fn timed_span(v: &Value) -> Option<(Tick, Tick)> {
    let start = v.get("start_tick").or_else(|| v.get("start")).and_then(parse_tick)?;
    let end = v.get("end_tick").or_else(|| v.get("end")).and_then(parse_tick)?;
    Some((start, end))
}

fn word_timing_ok(a: Tick, b: Tick, last: Tick) -> bool {
    let slop = Tick::from_units(1, 1000);
    a >= Tick::ZERO && b >= a && a >= last - slop
}

fn word_problems(words: &[Value]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut last = Tick::MIN;
    for (i, w) in words.iter().enumerate() {
        let has_text = w.get("text").and_then(Value::as_str).is_some_and(|t| !t.trim().is_empty());
        match (has_text, timed_span(w)) {
            (true, Some((a, b))) if word_timing_ok(a, b, last) => last = a,
            (true, Some(_)) => problems.push(format!("word {} is out of order or ends before it starts", i + 1)),
            _ => problems.push(format!("word {} needs text, start and end", i + 1)),
        }
        if problems.len() > 5 {
            break;
        }
    }
    problems
}

fn paragraphs_ordered(lines: &[Value]) -> bool {
    let mut previous = Tick::ZERO;
    for p in lines {
        match timed_span(p) {
            Some((a, b)) if a >= previous && a >= Tick::ZERO && b >= a && p["text"].as_str().is_some_and(|t| !t.trim().is_empty()) => {
                previous = a;
            }
            _ => return false,
        }
    }
    true
}

fn paragraph_problems(lines: &[Value]) -> Option<String> {
    if lines.is_empty() || !paragraphs_ordered(lines) {
        return Some("paragraphs need ordered nonnegative start/end and nonempty text".into());
    }
    None
}

fn duration_problems(doc: &Value, duration: Tick) -> Vec<String> {
    let mut problems = Vec::new();
    let slop = Tick::from_units(1, 1000);
    for key in ["words", "paragraphs"] {
        for (i, v) in doc[key].as_array().into_iter().flatten().enumerate() {
            if v.get("end_tick").or_else(|| v.get("end")).and_then(parse_tick).is_some_and(|end| end > duration + slop) {
                problems.push(format!(
                    "{key} item {} ends after the recording ({} ticks). Check matching audio/transcript; original received bytes retained.",
                    i + 1,
                    duration.0
                ));
            }
        }
    }
    problems
}

pub(crate) fn transcript_problems(doc: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    match doc.get("words").and_then(Value::as_array) {
        None => problems.push("missing \"words\": [{text, start_tick, end_tick}]".into()),
        Some(words) if words.is_empty() => problems.push("\"words\" is empty".into()),
        Some(words) => problems.extend(word_problems(words)),
    }
    match doc.get("paragraphs").and_then(Value::as_array) {
        None => problems.push("missing \"paragraphs\": [{text, start_tick, end_tick}] (the sung lines or stanzas)".into()),
        Some(lines) => problems.extend(paragraph_problems(lines)),
    }
    if let Some(duration) = doc.get("recording_duration_tick").or_else(|| doc.get("recording_duration_sec")).and_then(parse_tick).filter(|t| t.0 >= 0) {
        problems.extend(duration_problems(doc, duration));
    }
    problems
}

fn parse_srt_timestamp(raw: &str) -> Option<Tick> {
    let token = raw.split_whitespace().next()?;
    let (hms, ms_str) = token.split_once([',', '.'])?;
    let mut parts = hms.split(':');
    let h: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let s: i64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || m >= 60 || s >= 60 {
        return None;
    }
    let ms: i64 = format!("{:0<3}", &ms_str[..ms_str.len().min(3)]).parse().ok()?;
    let total_ms = ((h * 3600 + m * 60 + s) * 1000) + ms;
    Some(Tick::from_units(total_ms, 1000))
}

fn format_srt_timestamp(tick: Tick) -> String {
    let total_ms = tick.max(Tick::ZERO).to_units_floor(1000);
    let ms = total_ms % 1000;
    let total_s = total_ms / 1000;
    let s = total_s % 60;
    let total_m = total_s / 60;
    let m = total_m % 60;
    let h = total_m / 60;
    format!("{h:02}:{m:02}:{s:02},{ms:03}")
}

pub(crate) fn import_caption_paragraphs(text: &str) -> Result<Vec<Value>> {
    let normalized = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let mut cues = Vec::new();
    let mut previous = Tick::ZERO;
    let slop = Tick::from_units(1, 1_000_000);
    for block in normalized.split("\n\n") {
        let mut lines = block.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("WEBVTT") && !l.starts_with("NOTE")).peekable();
        let Some(first) = lines.peek().copied() else { continue };
        if !first.contains("-->") {
            lines.next();
        }
        let Some(timing_line) = lines.next() else { continue };
        let Some((start_raw, end_raw)) = timing_line.split_once("-->") else {
            return Err(EngineError::InvalidParams("invalid caption file: missing '-->' timing arrow".into()));
        };
        let (Some(start), Some(end)) = (parse_srt_timestamp(start_raw), parse_srt_timestamp(end_raw)) else {
            return Err(EngineError::InvalidParams("invalid caption file: bad timestamp".into()));
        };
        let clean = lines.collect::<Vec<_>>().join(" ").trim().to_string();
        if start < previous - slop || end < start || clean.is_empty() {
            return Err(EngineError::InvalidParams("SRT cues must be ordered, timed and nonempty".into()));
        }
        previous = start;
        cues.push(json!({"text": clean, "start_tick": start.0, "end_tick": end.0, "start": start.0, "end": end.0}));
    }
    if cues.is_empty() {
        return Err(EngineError::InvalidParams("caption file is empty".into()));
    }
    Ok(cues)
}

pub fn cmd_workshop_import_transcript(s: &mut Session, params: &Value) -> Result<Value> {
    import_transcript_inner(s, params)
}

fn read_transcript_source(s: &Session, params: &Value) -> Result<(PathBuf, Value)> {
    let source = params
        .get("path")
        .and_then(Value::as_str)
        .map(|p| resolve_workspace_path(s, p))
        .ok_or_else(|| EngineError::InvalidParams("missing transcript 'path'".into()))?;
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&source)?).map_err(|e| EngineError::InvalidParams(format!("not a JSON transcript: {e}")))?;
    Ok((source, doc))
}

fn prepare_document(s: &Session, params: &Value, doc: &mut Value) -> Result<(Option<String>, &'static str)> {
    let caption_text = params["captions"].as_str().map(|p| resolve_workspace_path(s, p)).map(std::fs::read_to_string).transpose()?;
    if doc.is_array() {
        *doc = json!({"words": doc.clone()});
    }
    if let Some(words) = doc["words"].as_array_mut() {
        for w in words {
            if let Some((start, end)) = timed_span(w) {
                w["start_tick"] = json!(start.0);
                w["end_tick"] = json!(end.0);
                w["start"] = json!(start.0);
                w["end"] = json!(end.0);
            }
        }
    }
    if let Some(text) = &caption_text {
        doc["paragraphs"] = json!(import_caption_paragraphs(text)?);
    }
    let mut grouping = "provided paragraphs";
    if doc.get("paragraphs").and_then(Value::as_array).is_none()
        && let Some(words) = doc["words"].as_array()
    {
        doc["paragraphs"] = json!(
            words
                .iter()
                .map(|w| json!({"text":w["text"],"start_tick":w["start_tick"],"end_tick":w["end_tick"],"start":w["start"],"end":w["end"]}))
                .collect::<Vec<_>>()
        );
        grouping = "provisional word fallback; original word timestamps";
    }
    Ok((caption_text, grouping))
}

pub(crate) fn srt_from_paragraphs(doc: &Value) -> Option<String> {
    let paragraphs = doc["paragraphs"].as_array()?;
    let mut out = String::new();
    for (idx, p) in paragraphs.iter().enumerate() {
        let start = format_srt_timestamp(p.get("start_tick").or_else(|| p.get("start")).and_then(parse_tick).unwrap_or(Tick::ZERO));
        let end = format_srt_timestamp(p.get("end_tick").or_else(|| p.get("end")).and_then(parse_tick).unwrap_or(Tick::ZERO));
        let text = p["text"].as_str().unwrap_or_default();
        out.push_str(&format!("{}\n{start} --> {end}\n{text}\n\n", idx + 1));
    }
    Some(out)
}

fn audio_mismatch(doc: &Value, song: Option<&PathBuf>) -> Option<String> {
    let recorded = doc.get("audio").and_then(Value::as_str)?;
    let song = song?;
    (Path::new(recorded) != song.as_path()).then(|| format!("transcript is for {recorded}, but this project's song is {}", song.display()))
}

fn probed_duration(s: &Session) -> Option<Tick> {
    crate::cmd::film::media_duration_ticks(s.song_audio.as_deref()?, false).ok()
}

struct RecordingFacts {
    probed: Option<Tick>,
    warning: Option<&'static str>,
}

fn check_recording(s: &Session, doc: &mut Value, problems: &mut Vec<String>) -> RecordingFacts {
    let probed = probed_duration(s);
    doc["recording_duration_tick"] = json!(probed.map(|t| t.0));
    problems.extend(transcript_problems(doc).into_iter().filter(|e| e.contains("recording")));
    RecordingFacts {
        probed,
        warning: probed
            .is_none()
            .then_some("Recording duration is unknown: provide matching audio readable by FilmCraft. Transcript endpoints are not recording-length evidence."),
    }
}

fn transcript_stem(s: &Session) -> String {
    crate::workspace::sanitize_asset_id(s.project_path.as_ref().and_then(|p| p.file_stem()).and_then(|n| n.to_str()).unwrap_or("song"))
}

fn ensure_no_existing_transcript(dir: &Path, stem: &str) -> Result<()> {
    for old in transcript_files(dir) {
        if old != dir.join(format!("{stem}.transcript.json")) && old != dir.join("transcript.json") && old != dir.join("hyperframes/transcript.json") {
            return Err(EngineError::InvalidParams(format!("project already has a transcript: {} (remove it first)", old.display())));
        }
    }
    Ok(())
}

fn write_transcript(dest: &Path, doc: &Value, params: &Value) -> Result<()> {
    let serialized = serde_json::to_vec_pretty(&doc["words"]).map_err(|e| EngineError::InvalidParams(e.to_string()))?;
    if dest.is_file() && !params["replace"].as_bool().unwrap_or(false) {
        let old = transcript_document(dest)?;
        if old["words"] != doc["words"] || old["paragraphs"] != doc["paragraphs"] {
            return Err(EngineError::InvalidParams(
                "Transcript differs from existing authored data. Inspect received files; pass replace=true only to replace explicitly.".into(),
            ));
        }
    }
    let parent = dest.parent().ok_or_else(|| EngineError::InvalidParams("Transcript destination has no parent directory".into()))?;
    std::fs::create_dir_all(parent)?;
    craft_atomic::atomic_write(dest, &serialized)?;
    Ok(())
}

fn write_captions(dir: &Path, text: String) -> Result<PathBuf> {
    let path = dir.join("transcript.srt");
    craft_atomic::atomic_write(&path, text.as_bytes())?;
    Ok(path)
}

fn copy_filmcraft(s: &Session, dir: &Path, params: &Value, stem: &str) -> Result<Option<PathBuf>> {
    let Some(project) = params.get("filmcraft_project").and_then(Value::as_str).map(|p| resolve_workspace_path(s, p)) else {
        return Ok(None);
    };
    if !project.is_file() {
        return Err(EngineError::InvalidParams(format!("FilmCraft project not found: {}", project.display())));
    }
    let to = dir.join("film").join(format!("{stem}.fcproj"));
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(&project, &to)?;
    Ok(Some(to))
}

fn write_imports(s: &Session, dir: &Path, params: &Value, doc: &Value, caption_text: Option<String>, stem: &str) -> Result<Vec<PathBuf>> {
    let dest = dir.join("transcript.json");
    write_transcript(&dest, doc, params)?;
    let mut copied = vec![dest];
    if let Some(text) = caption_text {
        copied.push(write_captions(dir, text)?);
    }
    if let Some(project) = copy_filmcraft(s, dir, params, stem)? {
        copied.push(project);
    }
    Ok(copied)
}

fn merge_prior_producer(dir: &Path, source: &Path, provenance: &mut Value) {
    use sha2::Digest;
    let Some(raw_hash) = std::fs::read(source).ok().map(|bytes| format!("{:x}", sha2::Sha256::digest(bytes))) else {
        return;
    };
    let prior = std::fs::read(dir.join("transcript-provenance.json")).ok().and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    if let Some(prior) = prior.filter(|v| v["producer"]["transcript_sha256"] == json!(raw_hash)) {
        provenance["producer"] = prior["producer"].clone();
    }
}

fn write_provenance(dir: &Path, source: &Path, params: &Value, facts: &RecordingFacts, grouping: &str) -> Result<()> {
    let tick_val = facts.probed.map(|t| t.0);
    let sec_val = facts.probed.map(|t| t.0 as f64 / TICKS_PER_SECOND as f64);
    let mut provenance = json!({
        "mode": "reused existing transcript; no ASR or transcoding",
        "source": params["original_source"],
        "captions_source": params["original_captions"],
        "duration_tick": tick_val,
        "recording_duration_tick": tick_val,
        "recording_duration_sec": sec_val,
        "duration_source": if facts.probed.is_some() { "authoritative recording probe" } else { "unknown" },
        "duration_warning": facts.warning,
        "provider": "unknown; not recorded by source",
        "model": "unknown; not recorded by source",
        "executable": std::env::current_exe().ok(),
        "version": env!("CARGO_PKG_VERSION"),
        "tool": "workshop.import_transcript (native Rust; ASR not called)",
        "grouping": grouping,
    });
    merge_prior_producer(dir, source, &mut provenance);
    craft_atomic::atomic_write(
        &dir.join("transcript-provenance.json"),
        &serde_json::to_vec_pretty(&provenance).map_err(|e| EngineError::InvalidParams(e.to_string()))?,
    )?;
    Ok(())
}

fn import_transcript_inner(s: &mut Session, params: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let (source, mut doc) = read_transcript_source(s, params)?;
    let (mut caption_text, grouping) = prepare_document(s, params, &mut doc)?;
    if caption_text.is_none() {
        caption_text = srt_from_paragraphs(&doc);
    }
    doc["grouping"] = json!(grouping);
    let mut problems = transcript_problems(&doc);
    if let Some(problem) = audio_mismatch(&doc, s.song_audio.as_ref()) {
        problems.push(problem);
    }
    let facts = check_recording(s, &mut doc, &mut problems);
    if !problems.is_empty() {
        return Err(EngineError::InvalidParams(format!("transcript does not match: {}", problems.join("; "))));
    }
    let stem = transcript_stem(s);
    ensure_no_existing_transcript(&dir, &stem)?;
    let copied = write_imports(s, &dir, params, &doc, caption_text, &stem)?;
    write_provenance(&dir, &source, params, &facts, grouping)?;
    let mut status = workshop_status(s)?;
    status["imported"] = json!(copied);
    status["recording_duration_tick"] = json!(facts.probed.map(|t| t.0));
    status["duration_warning"] = json!(facts.warning);
    Ok(status)
}

/// Transcribe and analyze the selected original recording via FilmCraft (`transcript.generate`).
pub fn cmd_audio_analyze(s: &mut Session, params: &Value) -> Result<Value> {
    let _audio = params["audio"]
        .as_str()
        .map(|p| resolve_workspace_path(s, p))
        .or_else(|| s.song_audio.clone())
        .filter(|p| p.is_file())
        .ok_or_else(|| EngineError::InvalidParams("no song audio: pass audio=<file> or workshop.set_audio".into()))?;
    let model = params["model"].as_str().unwrap_or("whisper-base").to_string();
    crate::cmd::film::cmd_video_exec(
        s,
        &json!({
            "id": "transcript.generate",
            "params": {"model": model, "language": "auto"},
            "save": true
        }),
    )
}
