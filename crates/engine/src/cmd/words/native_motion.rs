//! Discover native transform metadata and save source-word-bound curves through Words.
use super::{invalid, motion_curve, stage};
use crate::{Result, Session};
use serde_json::{Value, json};

pub fn properties(_s: &mut Session, _p: &Value) -> Result<Value> {
    let steps = [
        json!({"command":"comp.new","params":{"name":"Property discovery","width":640,"height":360,"duration":2}}),
        json!({"command":"layer.newText","params":{"name":"Word","text":"Word"}}),
        json!({"command":"layer.tree","params":{"layer":"Word","depth":2}}),
    ];
    let results = crate::cmd::character::effectcraft(std::path::Path::new("."), None, &steps, false)?;
    let tree = results.into_iter().nth(2).unwrap_or(Value::Null);
    let properties: Vec<Value> = tree["properties"]["children"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|group| group["path"] == "transform")
        .and_then(|group| group["children"].as_array())
        .into_iter()
        .flatten()
        .filter(|property| matches!(property["path"].as_str(), Some("transform/position" | "transform/rotation" | "transform/scale" | "transform/opacity")))
        .cloned()
        .collect();
    Ok(
        json!({"engine":"EffectCraft","properties":properties,"coordinate_mode":"offset from authored placement","duration_range":[0.05,8.0],"source_text_editable":false}),
    )
}

pub fn curve(_s: &mut Session, p: &Value) -> Result<Value> {
    let (config, keys) = motion_curve::curve(p)?;
    Ok(json!({"engine":"EffectCraft","config":config,"motion_keys":keys}))
}

pub fn set(s: &mut Session, p: &Value) -> Result<Value> {
    let section = p["animation_id"].as_str().ok_or_else(|| invalid("animation_id is required"))?;
    let line = p["line_id"].as_str().ok_or_else(|| invalid("line_id is required"))?;
    let word_ids = p["word_ids"].as_array().filter(|ids| !ids.is_empty()).ok_or_else(|| invalid("word_ids must identify an existing word or phrase group"))?;
    let revision = p["expected_source_revision"].as_u64().ok_or_else(|| invalid("expected_source_revision is required"))?;
    let read = stage::cmd_words_read(s, &json!({}))?;
    let animation = read["document"]["animations"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|animation| animation["animation_id"] == section)
        .ok_or_else(|| invalid("No matching lyric section"))?;
    let mut design = animation["design"][line].clone();
    let item = design["items"]
        .as_array_mut()
        .and_then(|items| items.iter_mut().find(|item| item["word_ids"].as_array() == Some(word_ids)))
        .ok_or_else(|| invalid("Choose an existing word/phrase group; split or group it in Lyrics first"))?;
    let (config, keys) = motion_curve::curve(p)?;
    item["native_motion"] = config;
    item["motion_keys"] = keys;
    item["accent"] = word_ids[0].clone();
    let mut designs = json!({});
    designs[line] = design;
    stage::cmd_words_save(s, &json!({"animation_id":section,"expected_source_revision":revision,"design":designs,"origin":"native-transform-controls"}))
}
