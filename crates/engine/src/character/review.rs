//! Project-local, append-only animation revisions. Source GLBs are never overwritten.
use super::{bad, tripo};
use crate::{Result, Session};
use serde_json::{Value, json};
use std::path::PathBuf;

fn path(s: &Session) -> Result<PathBuf> {
    Ok(crate::workshop::project_dir(s)?.join("animation-review.json"))
}
fn read(s: &Session) -> Result<Value> {
    let p = path(s)?;
    if !p.exists() {
        return Ok(json!({"version":1,"revision":0,"revisions":[]}));
    }
    let v: Value = serde_json::from_slice(&std::fs::read(p)?).map_err(|e| bad(e.to_string()))?;
    if v["version"] != 1 || !v["revisions"].is_array() {
        return Err(bad("Invalid animation-review.json"));
    }
    Ok(v)
}
fn fingerprint(p: &std::path::Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    Ok(format!("{:x}", Sha256::digest(std::fs::read(p)?)))
}

pub fn cmd_review(s: &mut Session, _: &Value) -> Result<Value> {
    let dir = crate::workshop::project_dir(s)?;
    let chars = super::cmd_character_list(s, &json!({}))?;
    let models: Vec<Value> = chars["characters"].as_array().into_iter().flatten().filter_map(|c| {
        let m = &c["tripo_3d"];
        let file = m["primary_glb"].as_str()?;
        let name = s.project_assets.iter().find(|a| a.id == c["id"]).map(|a| a.name.clone()).unwrap_or_else(|| c["id"].as_str().unwrap_or("").replace('-', " "));
        Some(json!({"id":c["id"],"name":name,"path":file,"rigged":m["rigged"],"clips":m["model_clips"],"version":std::fs::metadata(file).ok().and_then(|m|m.modified().ok()).and_then(|t|t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d|d.as_millis().to_string()),"qa_status":"needs-visual-review"}))
    }).collect();
    Ok(
        json!({"ok":true,"project":s.project_path,"directory":dir,"models":models,"review":read(s)?,"file":path(s)?}),
    )
}

pub fn cmd_save_revision(s: &mut Session, p: &Value) -> Result<Value> {
    let project = s
        .project_path
        .as_ref()
        .ok_or_else(|| bad("Open a project first"))?;
    if p["expected_project"].as_str() != project.to_str() {
        return Err(bad("Project changed; reopen animation review"));
    }
    let mut doc = read(s)?;
    if p["expected_revision"].as_u64() != doc["revision"].as_u64() {
        return Err(bad("Review changed on disk; reload before saving"));
    }
    let id = p["character"]
        .as_str()
        .filter(|v| !v.is_empty() && crate::workspace::sanitize_asset_id(v) == *v)
        .ok_or_else(|| bad("Invalid character"))?;
    let model = tripo::inspect_character_3d(&crate::workshop::project_dir(s)?, id);
    let file = model["primary_glb"]
        .as_str()
        .ok_or_else(|| bad("Character has no GLB"))?;
    let clip = p["clip"].as_str().ok_or_else(|| bad("clip is required"))?;
    if !model["model_clips"]
        .as_array()
        .is_some_and(|a| a.contains(&json!(clip)))
    {
        return Err(bad("Clip is not in this character's GLB"));
    }
    let d = &p["draft"];
    let start = number(d, "trim_start", 0., 3600.)?;
    let end = number(d, "trim_end", 0., 3600.)?;
    if end <= start {
        return Err(bad("Trim end must be after trim start"));
    }
    number(d, "speed", 0.1, 4.)?;
    for k in ["loop", "in_place"] {
        if !d[k].is_boolean() {
            return Err(bad(format!("{k} must be boolean")));
        }
    }
    if !matches!(
        d["status"].as_str(),
        Some("needs-visual-review" | "issues-found" | "approved")
    ) {
        return Err(bad("Invalid review status"));
    }
    let keys = d["keys"]
        .as_array()
        .filter(|a| a.len() <= 10000)
        .ok_or_else(|| bad("keys must be an array of at most 10000 corrections"))?;
    for key in keys {
        if key["bone"]
            .as_str()
            .is_none_or(|b| b.is_empty() || b.len() > 200)
        {
            return Err(bad("Invalid bone name"));
        }
        number(key, "time", 0., 3600.)?;
        if !key["rotation"].as_array().is_some_and(|v| {
            v.len() == 3
                && v.iter()
                    .all(|n| n.as_f64().is_some_and(|n| n.is_finite() && n.abs() <= 180.))
        }) {
            return Err(bad(
                "Correction rotation must have three angles within ±180 degrees",
            ));
        }
    }
    if !d["issues"].as_array().is_some_and(|v| {
        v.len() <= 1000
            && v.iter().all(|i| {
                i["time"]
                    .as_f64()
                    .is_some_and(|n| n.is_finite() && n >= 0. && n <= 3600.)
                    && i["text"]
                        .as_str()
                        .is_some_and(|t| !t.trim().is_empty() && t.len() <= 4000)
            })
    }) {
        return Err(bad("Invalid issue notes"));
    }
    if d["notes"].as_str().is_none_or(|s| s.len() > 16000) {
        return Err(bad("Notes must be text under 16000 bytes"));
    }
    let revision = doc["revision"].as_u64().unwrap_or(0) + 1;
    let entry = json!({"id":revision,"character":id,"clip":clip,"created_unix_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64,"source_sha256":fingerprint(std::path::Path::new(file))?,"parent":p["parent"],"draft":d});
    doc["revisions"].as_array_mut().unwrap().push(entry.clone());
    doc["revision"] = json!(revision);
    let out = path(s)?;
    let tmp = out.with_extension("json.tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(&doc).map_err(|e| bad(e.to_string()))?,
    )?;
    std::fs::rename(tmp, &out)?;
    Ok(json!({"ok":true,"file":out,"revision":revision,"entry":entry}))
}
fn number(p: &Value, key: &str, min: f64, max: f64) -> Result<f64> {
    p[key]
        .as_f64()
        .filter(|v| v.is_finite() && *v >= min && *v <= max)
        .ok_or_else(|| bad(format!("{key} must be between {min} and {max}")))
}
