//! Saved lyric/drawing thumbnails rendered by EffectCraft, with no project writes.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};

pub fn frame(s: &mut Session, p: &Value) -> Result<Value> {
    let time =
        p["time"].as_f64().filter(|v| v.is_finite() && *v >= 0.0).ok_or_else(|| EngineError::InvalidParams("time must be nonnegative seconds".into()))?;
    let kind = p["kind"].as_str().unwrap_or("lyrics");
    let layer_type = match kind {
        "lyrics" => "Text",
        "motion" => "Shape",
        _ => return Err(EngineError::InvalidParams("kind must be lyrics or motion".into())),
    };
    let dir = super::workspace_dir(s)?;
    let project = dir.join("lettering/lyrics.ecproj");
    if !project.is_file() {
        return Err(EngineError::InvalidParams("No saved EffectCraft lyrics composition; save the lyric design first".into()));
    }
    let cli = crate::workspace::find_executable("effectcraft-cli").map_err(|e| EngineError::ExternalTool("effectcraft-cli".into(), e.to_string()))?;
    let output = std::process::Command::new(cli).arg("info").arg("--project").arg(&project).arg("--json").output()?;
    if !output.status.success() {
        return Err(EngineError::ExternalTool("effectcraft-cli".into(), String::from_utf8_lossy(&output.stderr).into_owned()));
    }
    let info: Value = serde_json::from_slice(&output.stdout).map_err(|e| EngineError::ExternalTool("effectcraft-cli".into(), e.to_string()))?;
    let comp = &info["activeComp"];
    if comp["name"] != "Lyrics" {
        return Err(EngineError::InvalidParams("The saved lettering project must select its Lyrics composition for preview".into()));
    }
    let layers = comp["layers"].as_array().ok_or_else(|| EngineError::InvalidParams("EffectCraft returned no active composition layers".into()))?;
    let ids: Vec<Value> = layers.iter().filter(|l| l["type"] == layer_type).map(|l| l["id"].clone()).collect();
    if ids.is_empty() {
        return Ok(json!({"empty":true,"kind":kind,"time":time,"source":project}));
    }
    let folder = dir.join("lettering/.preview");
    std::fs::create_dir_all(&folder)?;
    let out = folder.join(format!("{kind}-{}.png", (time * 1000.0).round() as u64));
    crate::workspace::cmd_effect_exec(
        s,
        &json!({"path":project,"save":false,"steps":[
            {"id":"comp.open","params":{"comp":comp["id"]}},
            {"id":"layer.setSwitch","params":{"layers":ids,"switch":"video","value":false,"others":true}},
            {"id":"comp.saveFrameAs","params":{"path":out,"time":time,"scale":0.25,"transparent":true}}
        ]}),
    )?;
    if !out.is_file() {
        return Err(EngineError::InvalidParams("EffectCraft did not produce the preview frame".into()));
    }
    Ok(json!({"frame":out,"source":project,"kind":kind,"time":time,"native":"effectcraft"}))
}
