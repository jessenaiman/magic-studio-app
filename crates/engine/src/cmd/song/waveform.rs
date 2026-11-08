//! Read-only waveform envelopes from craft-audio's media decoder.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};

pub fn read(s: &mut Session, p: &Value) -> Result<Value> {
    let bad = |message: &str| EngineError::InvalidParams(message.into());
    let path = s.song_audio.as_ref().ok_or_else(|| bad("Attach the song audio first"))?;
    let ext = path.extension().and_then(|e| e.to_str());
    let file = std::fs::File::open(path).map_err(|error| EngineError::InvalidParams(format!("Cannot read song audio: {error}")))?;
    let mut dec = craft_audio::Decoder::open(file, ext, &craft_audio::DecodeOptions::default())
        .map_err(|error| EngineError::InvalidParams(format!("Cannot read song audio: {error}")))?;
    if dec.info().channels == 0 {
        return Err(bad("The song source has no audio"));
    }
    let sample_rate = dec.info().sample_rate.max(1);
    let duration = if dec.length_known() {
        dec.info().duration_secs()
    } else {
        let probe_file = std::fs::File::open(path).map_err(|error| EngineError::InvalidParams(format!("Cannot read song audio: {error}")))?;
        craft_audio::Decoder::open(probe_file, ext, &craft_audio::DecodeOptions::default())
            .and_then(craft_audio::Decoder::probe)
            .map_err(|error| EngineError::InvalidParams(format!("Cannot read song audio: {error}")))?
            .duration_secs()
    };
    let number = |key: &str, default: f64| -> Result<f64> {
        p.get(key)
            .map_or(Ok(default), |value| value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| EngineError::InvalidParams(format!("{key} must be finite"))))
    };
    let start = number("start", 0.0)?;
    let requested_end = number("end", duration)?;
    if start < 0.0 || start >= duration || requested_end <= start || requested_end - start > 600.0 {
        return Err(bad("Choose an audio range of at most 600 seconds within the song"));
    }
    let end = requested_end.min(duration);
    let bins = match p.get("bins") {
        Some(value) => value.as_u64().filter(|value| (8..=4096).contains(value)).ok_or_else(|| bad("bins must be an integer from 8 to 4096"))? as usize,
        None => 800,
    };
    let first = (start * f64::from(sample_rate)).floor().max(0.0) as u64;
    let frames = ((end - start) * f64::from(sample_rate)).ceil().max(1.0) as usize;
    let _ = dec.seek(first);
    let mut peaks = vec![[0.0_f32, 0.0_f32]; bins];
    let mut offset = 0usize;
    while offset < frames {
        let Some(block) = dec.next_block().map_err(|error| EngineError::InvalidParams(format!("Cannot decode song audio: {error}")))? else {
            break;
        };
        let count = block.frames().min(frames - offset);
        for channel in &block.channels {
            for (index, &sample) in channel.iter().take(count).enumerate() {
                if sample.is_finite() {
                    let bin = ((offset + index) * bins / frames).min(bins - 1);
                    peaks[bin][0] = peaks[bin][0].min(sample);
                    peaks[bin][1] = peaks[bin][1].max(sample);
                }
            }
        }
        offset += count;
    }
    let metadata = std::fs::metadata(path)?;
    Ok(json!({"source":path,"start":start,"end":end,"duration":duration,"bins":bins,
        "peaks":peaks,"decoder":"FilmCraft","source_bytes":metadata.len(),
        "modified_ms":metadata.modified().ok().and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok()).map(|time| time.as_millis())}))
}
