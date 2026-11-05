//! Native EffectCraft operations through its published host and engine crates.
use crate::{EngineError, Result};
use effectcraft_engine::{Session, project::render_queue::RenderStatus, time::Tick};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn error(context: &str, e: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidParams(format!("EffectCraft {context}: {e}"))
}

fn absolute(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

fn open(project: &Path) -> Result<Session> {
    let path = std::fs::canonicalize(project)?;
    let mut session = effectcraft_host::session();
    session
        .execute("file.open", json!({"path": path}))
        .map_err(|e| error("file.open", e))?;
    Ok(session)
}

/// Use the same sidecar lock name as EffectCraft's saving CLI, held from read to save.
pub(crate) fn write_lock(project: &Path) -> Result<std::fs::File> {
    let name = project
        .file_name()
        .ok_or_else(|| error("save", "project needs a filename"))?
        .to_string_lossy();
    let name = name
        .get(..name.floor_char_boundary(200))
        .unwrap_or_default();
    let path = project.with_file_name(format!(".{name}.lock"));
    let file = std::fs::File::options()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)?;
    file.try_lock().map_err(|e| {
        error(
            "save",
            format!("{} is busy or unavailable: {e}", path.display()),
        )
    })?;
    Ok(file)
}

/// Run the real command registry directly, returning typed results without a CLI batch file.
pub(crate) fn run(base: &Path, project: Option<&Path>, steps: &[Value]) -> Result<Vec<Value>> {
    let base = std::fs::canonicalize(base)?;
    let project = project.map(|p| absolute(&base, p));
    let mut targets = project.iter().cloned().collect::<Vec<_>>();
    for step in steps {
        let id = step["command"].as_str().or_else(|| step["id"].as_str());
        if matches!(id, Some("file.saveAs" | "file.save"))
            && let Some(path) = step["params"]["path"].as_str()
        {
            let target = absolute(&base, Path::new(path));
            if !targets.contains(&target) {
                targets.push(target);
            }
        }
    }
    let _locks = targets
        .iter()
        .map(|p| write_lock(p))
        .collect::<Result<Vec<_>>>()?;
    let mut session = match project.as_deref() {
        Some(path) => open(path)?,
        None => effectcraft_host::session(),
    };
    let mut results = Vec::with_capacity(steps.len());
    for step in steps {
        let id = step["command"]
            .as_str()
            .or_else(|| step["id"].as_str())
            .ok_or_else(|| error("command", "missing command id"))?;
        let mut params = step.get("params").cloned().unwrap_or_else(|| json!({}));
        if matches!(
            id,
            "file.open" | "file.save" | "file.saveAs" | "file.import" | "layer.newModel"
        ) {
            if let Some(path) = params["path"].as_str() {
                params["path"] = json!(absolute(&base, Path::new(path)));
            }
            if let Some(paths) = params.get_mut("paths").and_then(Value::as_array_mut) {
                for path in paths {
                    if let Some(text) = path.as_str() {
                        *path = json!(absolute(&base, Path::new(text)));
                    }
                }
            }
        }
        results.push(
            session
                .execute_checked(id, params)
                .map_err(|e| error(id, e))?,
        );
    }
    if let Some(path) = project {
        session
            .execute("file.save", json!({"path": path}))
            .map_err(|e| error("file.save", e))?;
    }
    Ok(results)
}

pub(crate) fn frame(
    project: &Path,
    comp: &str,
    seconds: f64,
    max_side: u32,
    output: &Path,
) -> Result<()> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(error("frame", "time must be finite and nonnegative"));
    }
    let session = open(project)?;
    let comp = session
        .resolve_comp(Some(&json!(comp)))
        .map_err(|e| error("composition", e))?;
    let (width, height, rgba) = session
        .render_rgba8_alpha(comp, Tick::from_seconds_f64(seconds), max_side, true)
        .map_err(|e| error("frame", e))?;
    image::save_buffer(output, &rgba, width, height, image::ColorType::Rgba8)
        .map_err(|e| error("PNG", e))
}

/// Queue exactly the requested output through EffectCraft's own renderer and exporters.
pub(crate) fn render(project: &Path, params: Value) -> Result<Value> {
    let mut session = open(project)?;
    for index in 0..session.project.render_queue.len() {
        session
            .execute(
                "renderQueue.setRender",
                json!({"index": index + 1, "render": false}),
            )
            .map_err(|e| error("render queue", e))?;
    }
    let added = session
        .execute_checked("renderQueue.add", params)
        .map_err(|e| error("renderQueue.add", e))?;
    let ids = session.start_render(true).map_err(|e| error("render", e))?;
    session.poll_render();
    if ids.is_empty() {
        return Err(error("render", "no render item started"));
    }
    for item in session
        .project
        .render_queue
        .iter()
        .filter(|item| ids.contains(&item.id))
    {
        match &item.status {
            RenderStatus::Done => {}
            RenderStatus::Failed(message) => return Err(error("render", message)),
            status => {
                return Err(error(
                    "render",
                    format!("render did not complete: {status:?}"),
                ));
            }
        }
    }
    Ok(added)
}
