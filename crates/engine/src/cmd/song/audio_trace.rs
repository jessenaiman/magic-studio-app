//! Receipt for the audio.analyze chain.
use crate::{EngineError, Result};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[allow(dead_code)]
pub(crate) struct AudioTrace {
    path: PathBuf,
    pub data: Value,
}

#[allow(dead_code)]
fn digest(path: &Path) -> Option<String> {
    fs::read(path).ok().map(|b| format!("{:x}", Sha256::digest(b)))
}

#[allow(dead_code)]
impl AudioTrace {
    pub fn new(project: &Path, audio: &Path, p: &Value) -> Result<Self> {
        let mut trace = Self {
            path: project.join("transcript-provenance.json"),
            data: json!({
                "status": "running",
                "operation": "audio.analyze",
                "audio_source": audio,
                "request_id": p["_request_id"],
                "client_label": p["_client"],
                "steps": [],
                "producer_status": "unknown until an actual transcription result is recorded"
            }),
        };
        trace.data["transcript_path"] = json!(project.join("transcript.json"));
        trace.persist()?;
        Ok(trace)
    }

    fn persist(&mut self) -> Result<()> {
        let mut v = fs::read(&self.path).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).filter(Value::is_object).unwrap_or(json!({}));
        let transcript = self.data["transcript_path"].as_str().map(Path::new);
        let hash = transcript.and_then(digest);
        self.data["transcript_sha256"] = json!(hash);
        v["analysis_attempt"] = self.data.clone();
        crate::doc_state::write_json(&self.path, &v)
            .map_err(|e| EngineError::InvalidParams(format!("Cannot save audio receipt {}: {e}", self.path.display())))?;
        Ok(())
    }
}
