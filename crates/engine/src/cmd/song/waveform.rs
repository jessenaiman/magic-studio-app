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

#[cfg(test)]
mod tests {
    //! Reference: <https://github.com/storytold/filmcraft/tree/7322aad72491a294a06a26a63fb65a3fc30ac242/crates/media>
    use super::*;
    #[test]
    fn waveform_reads_real_stereo_peaks_without_cancelling_channels_or_writing_audio() {
        let path = std::env::temp_dir().join(format!("magic-waveform-{}.wav", std::process::id()));
        let samples: Vec<i16> = (0..1600).flat_map(|i| if i == 0 { [16384, -16384] } else { [0, 0] }).collect();
        let size = (samples.len() * 2) as u32;
        let mut bytes = Vec::new();
        bytes.extend(b"RIFF");
        bytes.extend((size + 36).to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16_u32.to_le_bytes());
        bytes.extend(1_u16.to_le_bytes());
        bytes.extend(2_u16.to_le_bytes());
        bytes.extend(8000_u32.to_le_bytes());
        bytes.extend(32000_u32.to_le_bytes());
        bytes.extend(4_u16.to_le_bytes());
        bytes.extend(16_u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(size.to_le_bytes());
        for sample in samples {
            bytes.extend(sample.to_le_bytes());
        }
        std::fs::write(&path, &bytes).unwrap();
        let mut session = Session { song_audio: Some(path.clone()), ..Session::default() };
        let result = read(&mut session, &json!({"start":0,"end":0.2,"bins":8})).unwrap();
        let peaks = result["peaks"].as_array().unwrap();
        assert_eq!(peaks.len(), 8);
        assert!(peaks[0][0].as_f64().unwrap() < -0.49);
        assert!(peaks[0][1].as_f64().unwrap() > 0.49);
        assert!(peaks[1..].iter().all(|p| p == &json!([0.0, 0.0])));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert!(read(&mut session, &json!({"start":2,"end":1})).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
