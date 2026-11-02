//! Project-owned song details; plain lyrics never replace the recorded timing or artist sheet.
use crate::{EngineError, Result, Session};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SongMetadata {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub album: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_number: Option<u32>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub lyrics: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub notes: String,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

/// Update only supplied fields and persist through the ordinary project save/lease path.
pub fn save(s: &mut Session, p: &Value) -> Result<Value> {
    let project = s
        .project_path
        .clone()
        .ok_or_else(|| EngineError::InvalidParams("Open a song project first".into()))?;
    let mut next =
        serde_json::to_value(&s.song).map_err(|e| EngineError::InvalidParams(e.to_string()))?;
    for key in ["title", "album", "lyrics", "notes"] {
        if let Some(value) = p.get(key) {
            let text = value
                .as_str()
                .ok_or_else(|| EngineError::InvalidParams(format!("{key} must be text")))?;
            next[key] = json!(if matches!(key, "title" | "album") {
                text.trim()
            } else {
                text
            });
        }
    }
    if let Some(value) = p.get("track_number") {
        if !value.is_null() && value.as_u64().is_none_or(|n| n == 0 || n > u32::MAX as u64) {
            return Err(EngineError::InvalidParams(
                "track_number must be a positive whole number or null".into(),
            ));
        }
        next["track_number"] = value.clone();
    }
    let next: SongMetadata =
        serde_json::from_value(next).map_err(|e| EngineError::InvalidParams(e.to_string()))?;
    let previous = std::mem::replace(&mut s.song, next);
    if let Err(error) = crate::project::cmd_project_save(s, &json!({"expected_project":project})) {
        s.song = previous;
        return Err(error);
    }
    Ok(json!({"ok":true,"project":project,"metadata":s.song}))
}
