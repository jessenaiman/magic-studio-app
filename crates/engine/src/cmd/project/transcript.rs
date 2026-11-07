//! Workshop Stage 2 — Transcript validation, SRT caption parsing/export,
//! provenance receipts, and HyperFrames audio/beat analysis.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::cmd::film::video_target::ensure_video_target_project;
use crate::cmd::project::workshop::{project_dir, transcript_files, workshop_status};
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

fn timed_span(v: &Value) -> Option<(f64, f64)> {
    v.get("start").and_then(Value::as_f64).zip(v.get("end").and_then(Value::as_f64))
}

/// A word is usable when its text is nonempty and its timing is finite, nonnegative and ordered.
fn word_timing_ok(a: f64, b: f64, last: f64) -> bool {
    a >= 0.0 && a.is_finite() && b.is_finite() && b >= a && a >= last - 0.001
}

fn word_problems(words: &[Value]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut last = f64::MIN;
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

fn paragraph_timing_ok(a: f64, b: f64, previous: f64) -> bool {
    a.is_finite() && b.is_finite() && a >= previous && a >= 0.0 && b >= a
}

/// Paragraphs are ordered, nonnegative, timed and nonempty — or they are not paragraphs.
fn paragraphs_ordered(lines: &[Value]) -> bool {
    let mut previous = 0.0;
    for p in lines {
        match timed_span(p) {
            Some((a, b)) if paragraph_timing_ok(a, b, previous) && p["text"].as_str().is_some_and(|t| !t.trim().is_empty()) => {
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

/// Nothing may end after the recording it claims to describe.
fn duration_problems(doc: &Value, duration: f64) -> Vec<String> {
    let mut problems = Vec::new();
    for key in ["words", "paragraphs"] {
        for (i, v) in doc[key].as_array().into_iter().flatten().enumerate() {
            if v["end"].as_f64().is_some_and(|end| end > duration + 0.001) {
                problems.push(format!(
                    "{key} item {} ends after the recording ({duration:.6}s). Check matching audio/transcript; original received bytes retained.",
                    i + 1
                ));
            }
        }
    }
    problems
}

/// Checks a transcript has the shape audio.transcribe writes.
pub(crate) fn transcript_problems(doc: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    match doc.get("words").and_then(Value::as_array) {
        None => problems.push("missing \"words\": [{text, start, end}]".into()),
        Some(words) if words.is_empty() => problems.push("\"words\" is empty".into()),
        Some(words) => problems.extend(word_problems(words)),
    }
    match doc.get("paragraphs").and_then(Value::as_array) {
        None => problems.push("missing \"paragraphs\": [{text, start, end}] (the sung lines or stanzas)".into()),
        Some(lines) => problems.extend(paragraph_problems(lines)),
    }
    if let Some(duration) = doc["recording_duration_sec"].as_f64().filter(|v| v.is_finite() && *v >= 0.0) {
        problems.extend(duration_problems(doc, duration));
    }
    problems
}

/// Preserve externally authored cue boundaries using `filmcraft_captions::parse`; never invent word timing from subtitle lines.
pub(crate) fn import_caption_paragraphs(text: &str) -> Result<Vec<Value>> {
    let fmt = filmcraft_captions::detect(text.as_bytes(), Some("srt")).unwrap_or(filmcraft_captions::Format::Srt);
    let doc = filmcraft_captions::parse(text.as_bytes(), fmt).map_err(|e| EngineError::InvalidParams(format!("invalid caption file: {e}")))?;
    if doc.cues.is_empty() {
        return Err(EngineError::InvalidParams("caption file is empty".into()));
    }
    let mut cues = Vec::with_capacity(doc.cues.len());
    let mut previous = 0.0;
    for cue in doc.cues {
        let start = cue.start.seconds();
        let end = cue.end.seconds();
        let clean = cue.text.replace('\n', " ").trim().to_string();
        if start < previous - 1e-6 || end < start || clean.is_empty() {
            return Err(EngineError::InvalidParams("SRT cues must be ordered, timed and nonempty".into()));
        }
        previous = start;
        cues.push(json!({"text": clean, "start": start, "end": end}));
    }
    Ok(cues)
}

/// Supplies a transcript made earlier instead of transcribing again.
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

/// Captions, when given, define the paragraphs outright.
fn prepare_document(s: &Session, params: &Value, doc: &mut Value) -> Result<(Option<String>, &'static str)> {
    let caption_text = params["captions"].as_str().map(|p| resolve_workspace_path(s, p)).map(std::fs::read_to_string).transpose()?;
    if doc.is_array() {
        *doc = json!({"words": doc.clone()});
    }
    if let Some(text) = &caption_text {
        doc["paragraphs"] = json!(import_caption_paragraphs(text)?);
    }
    let mut grouping = "provided paragraphs";
    if doc.get("paragraphs").and_then(Value::as_array).is_none()
        && let Some(words) = doc["words"].as_array()
    {
        doc["paragraphs"] = json!(words.iter().map(|w| json!({"text":w["text"],"start":w["start"],"end":w["end"]})).collect::<Vec<_>>());
        grouping = "provisional word fallback; original word timestamps";
    }
    Ok((caption_text, grouping))
}

/// The captions to write when none were supplied: the document's own paragraph timing via `filmcraft_captions::write`.
pub(crate) fn srt_from_paragraphs(doc: &Value) -> Option<String> {
    let paragraphs = doc["paragraphs"].as_array()?;
    let cues: Vec<filmcraft_captions::Cue> = paragraphs
        .iter()
        .map(|p| filmcraft_captions::Cue {
            start: filmcraft_time::Tick::from_seconds_f64(p["start"].as_f64().unwrap_or(0.0)),
            end: filmcraft_time::Tick::from_seconds_f64(p["end"].as_f64().unwrap_or(0.0)),
            text: p["text"].as_str().unwrap_or_default().to_string(),
            ..Default::default()
        })
        .collect();
    let cap_doc = filmcraft_captions::Document { language: Some("en".into()), cues, ..Default::default() };
    let bytes = filmcraft_captions::write(&cap_doc, filmcraft_captions::Format::Srt, filmcraft_captions::WriteOptions::default());
    String::from_utf8(bytes).ok()
}

fn audio_mismatch(doc: &Value, song: Option<&PathBuf>) -> Option<String> {
    let recorded = doc.get("audio").and_then(Value::as_str)?;
    let song = song?;
    (Path::new(recorded) != song.as_path()).then(|| format!("transcript is for {recorded}, but this project's song is {}", song.display()))
}

/// Unknown duration is retained as a warning by the transcript importer.
fn probed_duration(s: &Session) -> Option<f64> {
    crate::cmd::film::media_duration(s.song_audio.as_deref()?, false).ok()
}

/// What the recording's duration is, where it came from, and anything that contradicts it.
struct RecordingFacts {
    probed: Option<f64>,
    duration: Option<f64>,
    warning: Option<&'static str>,
}

fn check_recording(s: &Session, doc: &mut Value, problems: &mut Vec<String>) -> RecordingFacts {
    let probed = probed_duration(s);
    doc["recording_duration_sec"] = json!(probed);
    problems.extend(transcript_problems(doc).into_iter().filter(|e| e.contains("recording")));
    RecordingFacts {
        probed,
        duration: probed,
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
        if old != dir.join(format!("{stem}.transcript.json")) && old != dir.join("hyperframes/transcript.json") {
            return Err(EngineError::InvalidParams(format!("project already has a transcript: {} (remove it first)", old.display())));
        }
    }
    Ok(())
}

/// Write the words, refusing to replace differing authored data unless `replace` is set.
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

fn write_captions(dir: &Path, target: &str, text: String) -> Result<PathBuf> {
    let folder = dir.join(target);
    std::fs::create_dir_all(&folder)?;
    let path = folder.join("transcript.srt");
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
    let to = dir.join(format!("{stem}.fcproj"));
    std::fs::copy(&project, &to)?;
    Ok(Some(to))
}

/// Every file the import wrote, in the order the caller reports them.
fn write_imports(s: &Session, dir: &Path, params: &Value, doc: &Value, caption_text: Option<String>, stem: &str) -> Result<Vec<PathBuf>> {
    let dest = dir.join("hyperframes/transcript.json");
    write_transcript(&dest, doc, params)?;
    let mut copied = vec![dest];
    if let Some(text) = caption_text {
        copied.push(write_captions(dir, &s.video_target, text)?);
    }
    if let Some(project) = copy_filmcraft(s, dir, params, stem)? {
        copied.push(project);
    }
    Ok(copied)
}

/// Keep an upstream producer's receipt when the transcript bytes are unchanged.
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
    let mut provenance = json!({"mode":"reused existing transcript; no ASR or transcoding","source":params["original_source"],"captions_source":params["original_captions"],"duration_sec":facts.duration,"duration_source":if facts.probed.is_some(){"authoritative recording probe"}else{"unknown"},"duration_warning":facts.warning, "recording_duration_sec":facts.probed,"provider":"unknown; not recorded by source","model":"unknown; not recorded by source","manual":"~/.agents/skills/media-use/audio/references/transcribe.md","executable":std::env::current_exe().ok(),"version":env!("CARGO_PKG_VERSION"),"tool":"workshop.import_transcript (native Rust; ASR not called)","grouping":grouping});
    merge_prior_producer(dir, source, &mut provenance);
    craft_atomic::atomic_write(
        &dir.join("transcript-provenance.json"),
        &serde_json::to_vec_pretty(&provenance).map_err(|e| EngineError::InvalidParams(e.to_string()))?,
    )?;
    Ok(())
}

fn captions_status(s: &mut Session, dir: &Path) -> Result<Value> {
    if ensure_video_target_project(s, dir, &s.video_target).is_ok() {
        return crate::cmd::film::video_target::ensure_native_captions(s, dir);
    }
    Ok(json!({"ready":false,"reason":"Create the contained HyperFrames project to display these timed lyrics"}))
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
    // Native captions are generated before the status is read, so the summary reflects them.
    let captions = captions_status(s, &dir)?;
    let mut status = workshop_status(s)?;
    status["captions"] = captions;
    status["imported"] = json!(copied);
    status["recording_duration_sec"] = json!(facts.probed);
    status["duration_warning"] = json!(facts.warning);
    Ok(status)
}

// ━━━ Audio transcription ━━━

fn analysis_audio(s: &Session, params: &Value) -> Result<PathBuf> {
    if s.video_target != "hyperframes" {
        return Err(EngineError::InvalidParams("Magic Studio uses HyperFrames for song analysis".into()));
    }
    params["audio"]
        .as_str()
        .map(|p| resolve_workspace_path(s, p))
        .or_else(|| s.song_audio.clone())
        .filter(|p| p.is_file())
        .ok_or_else(|| EngineError::InvalidParams("no song audio: pass audio=<file> or workshop.set_audio".into()))
}

fn hyperframes_runtime(params: &Value, hf: &Path) -> Result<String> {
    let runtime = crate::cmd::film::video_target::resolve_video_cli(params["resolve_video_cli"].as_str(), Some(hf))?;
    runtime.to_str().map(str::to_string).ok_or_else(|| EngineError::InvalidParams("HyperFrames executable path must be UTF-8".into()))
}

fn transcribe_recording(
    trace: &mut crate::cmd::song::audio_trace::AudioTrace,
    runtime: &str,
    hf: &Path,
    audio: &Path,
    transcript: &Path,
    model: &str,
    needed: bool,
) -> Result<bool> {
    let arg = |p: &Path| p.to_string_lossy().into_owned();
    Ok(trace
        .run(
            "transcription",
            needed,
            hf,
            runtime,
            &["transcribe".into(), arg(audio), "-d".into(), arg(hf), "--json".into(), "--model".into(), model.into()],
            std::slice::from_ref(&transcript.to_path_buf()),
        )?
        .is_some())
}

fn export_captions(
    trace: &mut crate::cmd::song::audio_trace::AudioTrace,
    runtime: &str,
    hf: &Path,
    transcript: &Path,
    srt: &Path,
    needed: bool,
) -> Result<bool> {
    let arg = |p: &Path| p.to_string_lossy().into_owned();
    Ok(trace
        .run(
            "caption_export",
            needed,
            hf,
            runtime,
            &["transcribe".into(), arg(transcript), "--to".into(), "srt".into(), "-o".into(), arg(srt)],
            std::slice::from_ref(&srt.to_path_buf()),
        )?
        .is_some())
}

/// Transcribe the recording and export captions, reusing whichever artifacts already exist.
fn transcribe_steps(
    trace: &mut crate::cmd::song::audio_trace::AudioTrace,
    runtime: &str,
    hf: &Path,
    audio: &Path,
    outputs: (&Path, &Path),
    model: &str,
    overwrite: bool,
) -> Result<Vec<&'static str>> {
    let (transcript, srt) = outputs;
    let mut ran = Vec::new();
    if transcribe_recording(trace, runtime, hf, audio, transcript, model, overwrite || !transcript.is_file())? {
        ran.push("hyperframes transcribe original recording → transcript.json");
    }
    if export_captions(trace, runtime, hf, transcript, srt, overwrite || !srt.is_file())? {
        ran.push("hyperframes transcribe --to srt → transcript.srt");
    }
    Ok(ran)
}

/// Sections, onsets and moments for the recording; Null when the caller opts out.
fn beat_analysis(
    params: &Value,
    trace: &mut crate::cmd::song::audio_trace::AudioTrace,
    runtime: &str,
    hf: &Path,
    audio: &Path,
    ran: &mut Vec<&'static str>,
) -> Result<Value> {
    if params["analyze_beats"] == false {
        return Ok(Value::Null);
    }
    let arg = |p: &Path| p.to_string_lossy().into_owned();
    let output = trace.run(
        "music_analysis",
        true,
        hf,
        runtime,
        &["audio".into(), arg(audio), "--only".into(), "sections,onsets,moments".into(), "--json".into()],
        &[],
    )?;
    let analysis: Value = serde_json::from_str(output.as_deref().unwrap_or_default())
        .map_err(|e| EngineError::InvalidParams(format!("HyperFrames audio returned invalid JSON: {e}")))?;
    if analysis["items"].as_array().is_none_or(Vec::is_empty) {
        return Err(EngineError::InvalidParams(format!("HyperFrames audio returned no recording analysis: {analysis}")));
    }
    std::fs::write(hf.join("audio-analysis.json"), serde_json::to_vec_pretty(&analysis).map_err(|e| EngineError::InvalidParams(e.to_string()))?)?;
    ran.push("hyperframes audio → audio-analysis.json");
    if let Some(step) = crate::cmd::film::video_target::detect_audio_beats(hf, runtime) {
        ran.push(step);
    }
    Ok(analysis)
}

fn transcript_word_count(trace: &mut crate::cmd::song::audio_trace::AudioTrace, transcript: &Path) -> Result<usize> {
    let words = trace.read_json("read_transcript", transcript)?;
    Ok(words.as_array().or_else(|| words["words"].as_array()).map_or(0, Vec::len))
}

/// Transcribe and analyze the selected original recording. Uses an explicit `resolve_video_cli`
/// when given, or pure-Rust `filmcraft-speech` + `filmcraft-captions` + `filmcraft-audio-dsp`.
pub fn cmd_audio_analyze(s: &mut Session, params: &Value) -> Result<Value> {
    let audio = analysis_audio(s, params)?;
    let dir = project_dir(s)?;
    let hf = dir.join("hyperframes");
    let overwrite = params["overwrite"].as_bool().unwrap_or(false);
    let model = params["model"].as_str().unwrap_or("whisper-base").to_string();
    if params["resolve_video_cli"].as_str().is_none() && params["engine"].as_str() != Some("hyperframes") {
        return crate::cmd::film::cmd_video_exec(
            s,
            &json!({
                "id": "transcript.generate",
                "params": {"model": model, "language": "auto"},
                "save": true
            }),
        );
    }
    let runtime = hyperframes_runtime(params, &hf)?;
    std::fs::create_dir_all(&hf)?;
    let transcript = hf.join("transcript.json");
    let srt = hf.join("transcript.srt");
    let mut trace = crate::cmd::song::audio_trace::AudioTrace::new(&dir, &audio, params)?;
    let mut ran = transcribe_steps(&mut trace, &runtime, &hf, &audio, (&transcript, &srt), &model, overwrite)?;
    let word_count = transcript_word_count(&mut trace, &transcript)?;
    let analysis = beat_analysis(params, &mut trace, &runtime, &hf, &audio, &mut ran)?;
    trace.import(s, json!({"path":transcript,"captions":srt,"replace":overwrite}))?;
    Ok(json!({"ok":true,"target":"hyperframes","ran":ran,"analysis_attempt":trace.data,
        "provenance_receipt":dir.join("transcript-provenance.json"),"words":word_count,
        "files":{"audio":audio,"transcript":transcript,"captions":srt,"audio_analysis":hf.join("audio-analysis.json"),"beats_dir":hf.join("beats")},"music_analysis":analysis,
        "next":["Review the words against the recording (`words.plan`), then add reference art (`project.add_asset`)"]}))
}
