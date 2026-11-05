//! FilmCraft document operations through its published Rust engine.
use crate::{EngineError, Result};
use filmcraft_engine::Session;
use serde_json::{Value, json};
use std::path::Path;

fn error(id: &str, e: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidParams(format!("FilmCraft {id}: {e}"))
}

/// Inspect the media itself using the same bounded file reader as FilmCraft's media browser.
pub(crate) fn media_duration(path: &Path, movie: bool) -> Result<f64> {
    let session = Session::default();
    let filename = path
        .to_str()
        .ok_or_else(|| error("probe", "expected a UTF-8 media path"))?;
    let source = session
        .media
        .probe_file(filename, &*session.services)
        .map_err(|e| error("probe", format!("{}: {e}", path.display())))?;
    let info = source.info();
    if movie && info.kind != filmcraft_media::MediaKind::Movie {
        return Err(error(
            "probe",
            format!("{} has no movie stream", path.display()),
        ));
    }
    if !movie && info.audio_streams.is_empty() {
        return Err(error(
            "probe",
            format!("{} has no audio stream", path.display()),
        ));
    }
    let duration = info.duration.seconds();
    if !duration.is_finite() || duration <= 0.0 {
        return Err(error(
            "probe",
            format!("{} has no positive duration", path.display()),
        ));
    }
    Ok(duration)
}

fn open(project: &Path) -> Result<Session> {
    let project = std::fs::canonicalize(project)?;
    let mut session = Session::default();
    session
        .execute("file.open", json!({"path": project}))
        .map_err(|e| error("file.open", e))?;
    Ok(session)
}

pub(crate) fn execute(project: &Path, id: &str, params: Value) -> Result<Value> {
    open(project)?.execute(id, params).map_err(|e| error(id, e))
}

pub(crate) fn run(project: &Path, fresh: bool, steps: &[Value]) -> Result<Vec<Value>> {
    let _lock = crate::effectcraft::write_lock(project).map_err(|e| error("save lock", e))?;
    let mut session = if fresh {
        Session::default()
    } else {
        open(project)?
    };
    let mut results = Vec::with_capacity(steps.len());
    for step in steps {
        let id = step["id"]
            .as_str()
            .or_else(|| step["command"].as_str())
            .ok_or_else(|| error("command", "missing command id"))?;
        let params = step.get("params").cloned().unwrap_or_else(|| json!({}));
        results.push(session.execute(id, params).map_err(|e| error(id, e))?);
    }
    session
        .execute("file.saveAs", json!({"path": project}))
        .map_err(|e| error("file.saveAs", e))?;
    Ok(results)
}

pub(crate) fn export(project: &Path, output: &Path, draft: bool) -> Result<Value> {
    let mut params = json!({"path": output, "format": "h264", "wait": true});
    if draft {
        params["scale"] = json!(0.5);
        params["quality"] = json!(60);
    }
    let result = execute(project, "file.exportMedia", params)?;
    if result["offlineMedia"]
        .as_array()
        .is_some_and(|items| !items.is_empty())
    {
        return Err(error(
            "export",
            format!("offline media: {}", result["offlineMedia"]),
        ));
    }
    Ok(result)
}

pub(crate) fn frame(project: &Path, seconds: f64, output: &Path) -> Result<()> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(error("frame", "time must be finite and nonnegative"));
    }
    let session = open(project)?;
    let (frame, offline) = session
        .try_render_program_with_offline_at(0.5, filmcraft_engine::commands::secs(seconds))
        .map_err(|e| error("frame", e))?;
    if !offline.is_empty() {
        return Err(error(
            "frame",
            format!("{} offline media items", offline.len()),
        ));
    }
    image::save_buffer(
        output,
        &frame.over_black_rgba8(),
        frame.w as u32,
        frame.h as u32,
        image::ColorType::Rgba8,
    )
    .map_err(|e| error("PNG", e))
}
