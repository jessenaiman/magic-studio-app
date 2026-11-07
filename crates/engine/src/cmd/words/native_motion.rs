//! Discover native transform metadata and save source-word-bound curves through Words.
use super::{invalid, motion_curve, stage};
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};

pub fn properties(_s: &mut Session, _p: &Value) -> Result<Value> {
    let mut native = effectcraft_host::session();
    let mut run =
        |id: &str, params: Value| native.execute_checked(id, params).map_err(|error| EngineError::ExternalTool("effectcraft-engine".into(), error.to_string()));
    run("comp.new", json!({"name":"Property discovery","width":640,"height":360,"duration":2}))?;
    run("layer.newText", json!({"name":"Word","text":"Word"}))?;
    let tree = run("layer.tree", json!({"layer":"Word","depth":2}))?;
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

#[cfg(test)]
mod tests {
    //! Reference: <https://github.com/storytold/effectcraft/tree/e1de87e8c3efaf02293728716d296342e0383a81/crates/engine/src/cmd>
    use super::*;
    #[test]
    fn supported_controls_are_discovered_from_native_effectcraft_properties() {
        let result = properties(&mut Session::default(), &json!({})).unwrap();
        let properties = result["properties"].as_array().unwrap();
        assert!(properties.iter().any(|p| p["path"] == "transform/rotation" && p["type"] == "scalar"));
        assert!(properties.iter().any(|p| p["path"] == "transform/position" && p["type"] == "vec3"));
        assert!(!properties.iter().any(|p| p["path"] == "text/sourceText"));
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    #[test]
    fn generic_motion_and_drawing_links_survive_validation_without_rewriting_words() {
        let words = vec![json!({"word_id":"line/w0","text":"round"}), json!({"word_id":"line/w1","text":"again"})];
        let item = json!({"word_ids":["line/w0","line/w1"],"text":"WRONG","x":100,"y":200,"size":80,"weight":700,"italic":false,"color":"#173f49","font":"song","outline":null,"rotate":0,"motion":"quiet","accent":"line/w0","native_motion":{"property":"transform/rotation","speed":180,"duration":3}});
        let design = json!({"items":[item],"draws":[{"name":"Circle","view_box":"0 0 200 200","x":100,"y":200,"w":200,"h":200,"word_ids":["line/w0"],"paths":[{"d":"M0 0L10 10","stroke":"#173f49","width":3,"from":0.3,"to":1.4}]}]});
        let normalized = super::super::validate::line_design(&words, &design).unwrap();
        assert_eq!(normalized["items"][0]["text"], "round again");
        assert_eq!(normalized["items"][0]["motion_keys"][1][5], 540.0);
        assert_eq!(normalized["items"][0]["native_motion"]["speed"], 180.0);
        assert_eq!(normalized["draws"][0]["word_ids"], json!(["line/w0"]));
        let mut invalid = design;
        invalid["draws"][0]["word_ids"] = json!(["other/w0"]);
        assert!(super::super::validate::line_design(&words, &invalid).is_err());
    }
}
