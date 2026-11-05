//! Shared Resources guide. An edit changes one field and never opens a song project.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::{fs, path::Path};

fn invalid(reason: &str) -> EngineError {
    EngineError::InvalidParams(reason.into())
}

fn read(path: &Path) -> Result<Value> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            include_bytes!("../../../references/creator-loop.json").to_vec()
        }
        Err(e) => return Err(e.into()),
    };
    serde_json::from_slice(&bytes).map_err(|e| invalid(&e.to_string()))
}

pub fn workflow(_: &mut Session, p: &Value) -> Result<Value> {
    let dir = crate::workspace::user_data_dir()
        .ok_or_else(|| invalid("User data directory unavailable"))?
        .join("magic/resources");
    let path = dir.join("creator-loop.json");
    let doc = if p.get("field").is_some() {
        fs::create_dir_all(&dir)?;
        let lock = fs::File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join("creator-loop.lock"))?;
        lock.lock()?;
        let mut doc = read(&path)?;
        let field = p["field"]
            .as_str()
            .ok_or_else(|| invalid("Missing field"))?;
        let table = match p["section"].as_str().unwrap_or("production") {
            "revision" if field == "label" => "nodes",
            "production" if ["label", "view", "command", "pipeline", "verify"].contains(&field) => {
                "production"
            }
            _ => return Err(invalid("Unknown workflow field")),
        };
        let row = doc[table]
            .as_array_mut()
            .and_then(|rows| rows.iter_mut().find(|row| row["id"] == p["id"]))
            .ok_or_else(|| invalid("Unknown workflow step"))?;
        if p.get("previous") != row.get(field) {
            return Err(invalid(
                "This field changed elsewhere. Reload the guide before editing it.",
            ));
        }
        let text = p["value"]
            .as_str()
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| invalid("Workflow text cannot be empty"))?;
        row[field] = json!(text);
        let temporary = dir.join("creator-loop.pending");
        fs::write(
            &temporary,
            serde_json::to_vec_pretty(&doc).map_err(|e| invalid(&e.to_string()))?,
        )?;
        fs::rename(temporary, &path)?;
        doc
    } else {
        read(&path)?
    };
    let steps: Vec<_> = if p["section"] == "revision" {
        doc["mainPath"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|id| {
                doc["nodes"]
                    .as_array()?
                    .iter()
                    .find(|node| node["id"] == *id)
                    .map(|node| &node["label"])
            })
            .collect()
    } else {
        doc["production"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|row| &row["label"])
            .collect()
    };
    if let Some(n) = p["step"].as_u64() {
        return n
            .checked_sub(1)
            .and_then(|i| doc["production"].get(i as usize))
            .cloned()
            .ok_or_else(|| invalid("Unknown step number"));
    }
    if p["steps"] == true {
        return Ok(json!({"steps":steps}));
    }
    Ok(json!({"document":doc,"steps":steps,"path":path}))
}
