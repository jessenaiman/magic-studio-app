//! The existing Home navigation history, shared by UI, terminal and MCP. Not a media scan.
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
pub fn file() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|p| p.join("magicstudio/recent_projects.json"))
}
pub fn list() -> Vec<String> {
    file()
        .and_then(|p| fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
fn canonical(path: &str) -> String {
    fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string())
}

/// Never persist test/fixture or temp paths as the user's recent project.
fn is_transient(path: &str) -> bool {
    path.contains("/.agent/")
        || path.contains("/tmp/")
        || Path::new(path).starts_with(std::env::temp_dir())
}

fn open_catalog(target: &Path) -> Result<fs::File, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(target)
        .map_err(|e| e.to_string())?;
    file.lock().map_err(|e| e.to_string())?;
    Ok(file)
}

fn read_items(file: &mut fs::File) -> Result<Vec<String>, String> {
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    // An unreadable or older catalog is replaced, not treated as a failure.
    Ok(serde_json::from_slice(&bytes).unwrap_or_default())
}

fn rewrite(file: &mut fs::File, items: &[String]) -> Result<(), String> {
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(items).map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    file.set_len(bytes.len() as u64).map_err(|e| e.to_string())
}

fn update(path: &str, forget: bool) -> Result<(), String> {
    let path = canonical(path);
    let target = file().ok_or("Config directory unavailable")?;
    fs::create_dir_all(target.parent().ok_or("Config directory unavailable")?)
        .map_err(|e| e.to_string())?;
    let mut file = open_catalog(&target)?;
    let mut items = read_items(&mut file)?;
    items.retain(|p| canonical(p) != path);
    if is_transient(&path) {
        return Ok(());
    }
    if !forget {
        items.insert(0, path);
        items.truncate(10);
    }
    let result = rewrite(&mut file, &items);
    // Even a failed write may have changed a prefix of the catalog.
    crate::change_marker::invalidate();
    result
}
pub fn record(path: &str) -> Result<(), String> {
    update(path, false)
}
pub fn forget(path: &str) -> Result<(), String> {
    update(path, true)
}
pub fn catalog(s: &mut crate::Session, params: &Value) -> crate::Result<Value> {
    if let Some(forget_path) = params["forget"].as_str() {
        forget(forget_path).map_err(crate::EngineError::InvalidParams)?;
    }
    let active = s
        .project_path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned());
    let mut seen = std::collections::HashSet::new();
    let mut projects: Vec<Value> = list()
        .into_iter()
        .map(|p| {
            fs::canonicalize(&p)
                .map(|v| v.to_string_lossy().into_owned())
                .unwrap_or(p)
        })
        .filter(|p| seen.insert(p.clone()))
        .map(|p| {
            let path = Path::new(&p);
            let dir = path.parent().unwrap_or(path);
            let thumb = dir.join("storyboard/.preview/page-01.png");
            let is_open = active.as_deref() == Some(p.as_str());
            let readiness = readiness(path);
            let availability = if !path.is_file() {
                "missing"
            } else if readiness["openable"] == true {
                "available"
            } else {
                "unreadable"
            };
            json!({
                "path": p,
                "project_id": p,
                "identity_kind": "canonical path",
                "name": path.file_stem().and_then(|v| v.to_str()).unwrap_or(&p),
                "folder": dir,
                "exists": path.is_file(),
                "thumb": thumb.is_file().then(|| thumb.to_string_lossy().into_owned()),
                "availability": availability,
                "open_state": if is_open { "attached" } else { "not_observed_open" },
                "sessions": [],
                "readiness": readiness
            })
        })
        .collect();
    projects.sort_by_key(|v| v["exists"] != true);
    Ok(json!({
        "projects": projects,
        "source": file(),
        "scope": "existing Home navigation history; newest ten, not all filesystem projects",
        "discovery_errors": []
    }))
}

fn readiness(path: &Path) -> Value {
    let Ok(text) = fs::read_to_string(path) else {
        return json!({"state":"unreadable","openable":false,"next_step":"Locate the project file, or remove this entry from Recent.","editor_ready":false});
    };
    let Ok(doc) = serde_json::from_str::<Value>(&text) else {
        return json!({"state":"unreadable","openable":false,"next_step":"The project file contains invalid JSON.","editor_ready":false});
    };
    if doc["format"].as_str() != Some("magicstudio.project") {
        return json!({"state":"unreadable","openable":false,"next_step":"This file is not a Magic Studio project.","editor_ready":false});
    }
    let dir = path.parent().unwrap_or(path);
    let audio = doc["workshop"]["audio"]
        .as_str()
        .is_some_and(|a| !a.is_empty() && (dir.join(a).is_file() || Path::new(a).is_file()));
    let transcript = crate::workshop::transcript_files(dir).iter().any(|path| {
        crate::workshop::transcript_document(path)
            .ok()
            .is_some_and(|document| {
                document["words"]
                    .as_array()
                    .is_some_and(|words| !words.is_empty())
            })
    });
    let editor_ready = dir.join("hyperframes/index.html").is_file()
        || doc["workshop"]["video_project"]
            .as_str()
            .is_some_and(|v| !v.is_empty() && (dir.join(v).exists() || Path::new(v).exists()));
    let next = if !audio {
        "Choose existing song audio"
    } else if !transcript {
        "Import or review timestamped transcript"
    } else if !editor_ready {
        "Review project; editor setup or server is still required"
    } else {
        "Open project"
    };
    json!({"openable":true,"audio_available":audio,"transcript_valid":transcript,"editor_ready":editor_ready,"approval":"not inferred","next_step":next,"problems":[],"quick_actions":[{"label":"Project","view":"project","open_block":"song","enabled":true},{"label":"Lyrics / transcript","view":"project","open_block":"lyrics","enabled":transcript},{"label":"Studio","view":"studio","enabled":editor_ready}]})
}
