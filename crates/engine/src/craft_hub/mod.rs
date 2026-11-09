//! CraftHub: Adaptive transport router connecting Magic Studio to the ArtCraft suite.
//!
//! Reference:
//! - <https://github.com/storytold/filmcraft/blob/7322aad72491a294a06a26a63fb65a3fc30ac242/apps/filmcraft/src/control_server.rs>
//! - <https://github.com/storytold/effectcraft/blob/e1de87e8c3efaf02293728716d296342e0383a81/apps/effectcraft/src/control_server.rs>
//! - <https://github.com/storytold/designcraft/blob/4f3bb5340eeee5f747f7ee2a6905d3c990831d22/apps/designcraft/src/control_server.rs>
//! - <https://github.com/storytold/photocraft/blob/a3ea9a4277e1e1067acbb30d592c56e9545b949d/apps/photocraft/src/control_server.rs>
//! - <https://github.com/storytold/vectorcraft/blob/a479c8baa1b783b5002f8ebd02391aa3395b5b49/apps/vectorcraft/src/control_server.rs>

mod doc;
pub(crate) use doc::{CraftDoc, ScopedTempDir};

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde_json::{Value, json};

use crate::{EngineError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftTarget {
    Film,
    Photo,
    Effect,
    Vector,
    Design,
    Ani,
    Texture,
}

impl CraftTarget {
    /// Every Craft app Magic drives.
    pub const ALL: [CraftTarget; 7] =
        [CraftTarget::Film, CraftTarget::Photo, CraftTarget::Effect, CraftTarget::Vector, CraftTarget::Design, CraftTarget::Ani, CraftTarget::Texture];

    /// The canonical application name (e.g. `filmcraft`).
    pub fn name(&self) -> &'static str {
        match self {
            Self::Film => "filmcraft",
            Self::Photo => "photocraft",
            Self::Effect => "effectcraft",
            Self::Vector => "vectorcraft",
            Self::Design => "designcraft",
            Self::Ani => "anicraft",
            Self::Texture => "texturecraft",
        }
    }

    /// The companion CLI binary name on PATH (e.g. `filmcraft-cli`).
    pub fn cli_name(&self) -> &'static str {
        match self {
            Self::Film => "filmcraft-cli",
            Self::Photo => "photocraft-cli",
            Self::Effect => "effectcraft-cli",
            Self::Vector => "vectorcraft-cli",
            Self::Design => "designcraft-cli",
            Self::Ani => "anicraft-cli",
            Self::Texture => "texturecraft-cli",
        }
    }

    /// Standard default control port for the application (`9876` is reserved for `magic`).
    pub fn default_port(&self) -> u16 {
        match self {
            Self::Film => 9875,
            Self::Design => 9877,
            Self::Effect => 9878,
            Self::Photo => 9879,
            Self::Vector => 9880,
            Self::Ani => 9881,
            Self::Texture => 9882,
        }
    }

    /// Environment variable used to override the default port.
    pub fn port_env(&self) -> &'static str {
        match self {
            Self::Film => "FILMCRAFT_CONTROL_PORT",
            Self::Design => "DESIGNCRAFT_CONTROL_PORT",
            Self::Effect => "EFFECTCRAFT_CONTROL_PORT",
            Self::Photo => "PHOTOCRAFT_CONTROL_PORT",
            Self::Vector => "VECTORCRAFT_CONTROL_PORT",
            Self::Ani => "ANICRAFT_CONTROL_PORT",
            Self::Texture => "TEXTURECRAFT_CONTROL_PORT",
        }
    }

    /// Environment variable holding the authentication token, if required.
    pub fn token_env(&self) -> Option<&'static str> {
        match self {
            Self::Film => Some("FILMCRAFT_CONTROL_TOKEN"),
            Self::Photo => Some("PHOTOCRAFT_CONTROL_TOKEN"),
            Self::Texture => Some("TEXTURECRAFT_CONTROL_TOKEN"),
            _ => None,
        }
    }
}

fn target_token(target: CraftTarget) -> Option<String> {
    if let Some(var) = target.token_env()
        && let Ok(val) = std::env::var(var)
    {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_owned());
        }
    }
    if target == CraftTarget::Texture
        && let Ok(addr) = target_addr(target)
    {
        let token_file = std::env::temp_dir().join(format!("texturecraft-control-{}.token", addr.port()));
        if let Ok(text) = std::fs::read_to_string(token_file) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_owned());
            }
        }
    }
    None
}

/// Runs a command in a Craft app that lives in this process, or `None` if it does not have `doc`
/// open.
pub type FilmHost = Box<dyn Fn(&Path, &str, &Value) -> Option<std::result::Result<Value, String>> + Send + Sync>;
pub type OpenHost = Box<dyn Fn(&Path) -> std::result::Result<Value, String> + Send + Sync>;

type CachedSession<S> = std::sync::Mutex<Option<(PathBuf, Option<std::time::SystemTime>, S)>>;
static FILM_HOST: std::sync::OnceLock<FilmHost> = std::sync::OnceLock::new();
static EFFECT_HOST: std::sync::OnceLock<FilmHost> = std::sync::OnceLock::new();
static EFFECT_OPEN_HOST: std::sync::OnceLock<OpenHost> = std::sync::OnceLock::new();
static HEADLESS_FILM: std::sync::OnceLock<CachedSession<filmcraft_engine::Session>> = std::sync::OnceLock::new();
static HEADLESS_EFFECT: std::sync::OnceLock<CachedSession<effectcraft_engine::Session>> = std::sync::OnceLock::new();
static FILMCRAFT_GPU_INIT: std::sync::Once = std::sync::Once::new();

struct GpuFrameRenderer(filmcraft_gpu::ExportRenderer);

impl filmcraft_export::FrameRenderer for GpuFrameRenderer {
    fn render(
        &mut self,
        project: &filmcraft_project::Project,
        seq: filmcraft_project::ItemId,
        t: filmcraft_time::Tick,
        opts: filmcraft_render::RenderOptions,
        sources: &dyn filmcraft_render::SourceProvider,
    ) -> Option<filmcraft_render::Image> {
        self.0.render(project, seq, t, opts, sources)
    }

    fn caps(&self) -> filmcraft_render::plan::DeviceCaps {
        self.0.caps()
    }
}

pub(crate) fn ensure_filmcraft_gpu() {
    FILMCRAFT_GPU_INIT.call_once(|| {
        filmcraft_export::register_frame_renderer(|| {
            filmcraft_gpu::ExportRenderer::new().map(|r| Box::new(GpuFrameRenderer(r)) as Box<dyn filmcraft_export::FrameRenderer>)
        });
    });
}

pub(crate) fn run_effect_steps(project: Option<&Path>, save: bool, steps: &[(String, Value)]) -> Result<Vec<Value>> {
    let borrowed: Vec<(&str, Value)> = steps.iter().map(|(id, params)| (id.as_str(), params.clone())).collect();
    let starts_fresh = borrowed.iter().any(|(id, _)| *id == "file.new");
    if !starts_fresh
        && let Some(path) = project.filter(|p| p.is_file())
        && let Some(live_res) = try_live_batch(CraftTarget::Effect, path, &borrowed, save)
    {
        if save
            && let Some(cache) = HEADLESS_EFFECT.get()
            && let Ok(mut guard) = cache.lock()
        {
            *guard = None;
        }
        return live_res;
    }

    let cache = HEADLESS_EFFECT.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = cache.lock().map_err(|e| EngineError::CommandFailed("effectcraft".into(), e.to_string()))?;

    if !starts_fresh && let Some(path) = project.filter(|p| p.is_file()) {
        let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let mtime = std::fs::metadata(&canon).and_then(|m| m.modified()).ok();
        let needs_open = match guard.as_ref() {
            Some((loaded_path, loaded_mtime, _)) => loaded_path != &canon || *loaded_mtime != mtime,
            None => true,
        };
        if needs_open {
            let mut session = effectcraft_engine::Session::new();
            session.execute("file.open", json!({ "path": canon })).map_err(|e| EngineError::CommandFailed("file.open".into(), e.to_string()))?;
            *guard = Some((canon, mtime, session));
        }
        let Some((loaded_path, loaded_mtime, session)) = guard.as_mut() else {
            return Err(EngineError::CommandFailed("effectcraft".into(), "EffectCraft session unavailable".into()));
        };
        let mut results = Vec::with_capacity(borrowed.len());
        for (id, params) in &borrowed {
            match session.execute(id, params.clone()) {
                Ok(v) => results.push(v),
                Err(e) => {
                    *guard = None;
                    return Err(EngineError::CommandFailed((*id).to_string(), e.to_string()));
                }
            }
        }
        if save {
            if let Err(e) = session.execute("file.save", json!({})) {
                *guard = None;
                return Err(EngineError::CommandFailed("file.save".into(), e.to_string()));
            }
            *loaded_mtime = std::fs::metadata(&*loaded_path).and_then(|m| m.modified()).ok();
        }
        return Ok(results);
    }

    let mut session = effectcraft_engine::Session::new();
    if !starts_fresh && let Some(path) = project.filter(|p| p.is_file()) {
        session.execute("file.open", json!({ "path": path })).map_err(|e| EngineError::CommandFailed("file.open".into(), e.to_string()))?;
    }
    let mut results = Vec::with_capacity(borrowed.len());
    for (id, params) in &borrowed {
        let v = session.execute(id, params.clone()).map_err(|e| EngineError::CommandFailed((*id).to_string(), e.to_string()))?;
        results.push(v);
    }
    if save && let Some(path) = project {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if !borrowed.iter().any(|(id, _)| *id == "file.save" || *id == "file.saveAs") {
            let save_res =
                if path.is_file() && !starts_fresh { session.execute("file.save", json!({})) } else { session.execute("file.saveAs", json!({ "path": path })) };
            save_res.map_err(|e| EngineError::CommandFailed("file.save".into(), e.to_string()))?;
        }
    }
    if let Some(path) = project.filter(|p| p.is_file()) {
        let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let mtime = std::fs::metadata(&canon).and_then(|m| m.modified()).ok();
        *guard = Some((canon, mtime, session));
    }
    Ok(results)
}

pub(super) fn run_embedded_doc_batch(target: CraftTarget, input: &Path, work: &Path, steps: &[(&str, Value)], save: bool) -> Option<Result<Vec<Value>>> {
    match target {
        CraftTarget::Effect => {
            let mut session = effectcraft_engine::Session::new();
            if input.is_file()
                && let Err(e) = session.execute("file.open", json!({ "path": input }))
            {
                return Some(Err(EngineError::CommandFailed("file.open".into(), e.to_string())));
            }
            let mut out = Vec::with_capacity(steps.len());
            for (id, params) in steps {
                match session.execute(id, params.clone()) {
                    Ok(v) => out.push(v),
                    Err(e) => return Some(Err(EngineError::CommandFailed((*id).to_string(), e.to_string()))),
                }
            }
            if save {
                let save_cmd = if input.is_file() && input == work {
                    session.execute("file.save", json!({}))
                } else {
                    session.execute("file.saveAs", json!({ "path": work }))
                };
                if let Err(e) = save_cmd {
                    return Some(Err(EngineError::CommandFailed("file.save".into(), e.to_string())));
                }
            }
            Some(Ok(out))
        }
        CraftTarget::Film => {
            ensure_filmcraft_gpu();
            let mut session = filmcraft_engine::Session::default();
            if input.is_file()
                && let Err(e) = session.execute("file.open", json!({ "path": input }))
            {
                return Some(Err(EngineError::CommandFailed("file.open".into(), e.to_string())));
            }
            let mut out = Vec::with_capacity(steps.len());
            for (id, params) in steps {
                match session.execute(id, params.clone()) {
                    Ok(v) => out.push(v),
                    Err(e) => return Some(Err(EngineError::CommandFailed((*id).to_string(), e.to_string()))),
                }
            }
            if save {
                let save_cmd = if input.is_file() && input == work {
                    session.execute("file.save", json!({}))
                } else {
                    session.execute("file.saveAs", json!({ "path": work }))
                };
                if let Err(e) = save_cmd {
                    return Some(Err(EngineError::CommandFailed("file.save".into(), e.to_string())));
                }
            }
            Some(Ok(out))
        }
        _ => None,
    }
}

fn run_embedded_film(doc: &Path, command: &str, params: &Value, save: bool) -> Option<std::result::Result<Value, String>> {
    if !doc.is_file() {
        return None;
    }
    ensure_filmcraft_gpu();
    let cache = HEADLESS_FILM.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = cache.lock().ok()?;
    let canon = std::fs::canonicalize(doc).unwrap_or_else(|_| doc.to_path_buf());
    let mtime = std::fs::metadata(&canon).and_then(|m| m.modified()).ok();
    let needs_open = match guard.as_ref() {
        Some((loaded_path, loaded_mtime, _)) => loaded_path != &canon || *loaded_mtime != mtime,
        None => true,
    };
    if needs_open {
        let mut session = filmcraft_engine::Session::default();
        if let Err(err) = session.execute("file.open", json!({ "path": canon })) {
            return Some(Err(err.to_string()));
        }
        *guard = Some((canon.clone(), mtime, session));
    }
    let (loaded_path, loaded_mtime, session) = guard.as_mut()?;
    let res = session.execute(command, params.clone()).map_err(|e| e.to_string()).and_then(|value| {
        if save {
            session.execute("file.save", json!({})).map_err(|e| e.to_string())?;
        }
        Ok(value)
    });
    if res.is_ok() {
        *loaded_mtime = std::fs::metadata(&*loaded_path).and_then(|m| m.modified()).ok();
    }
    Some(res)
}

fn run_embedded_effect(doc: &Path, command: &str, params: &Value, save: bool) -> Option<std::result::Result<Value, String>> {
    if !doc.is_file() {
        return None;
    }
    let cache = HEADLESS_EFFECT.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = cache.lock().ok()?;
    let canon = std::fs::canonicalize(doc).unwrap_or_else(|_| doc.to_path_buf());
    let mtime = std::fs::metadata(&canon).and_then(|m| m.modified()).ok();
    let needs_open = match guard.as_ref() {
        Some((loaded_path, loaded_mtime, _)) => loaded_path != &canon || *loaded_mtime != mtime,
        None => true,
    };
    if needs_open {
        let mut session = effectcraft_engine::Session::new();
        if let Err(err) = session.execute("file.open", json!({ "path": canon })) {
            return Some(Err(err.to_string()));
        }
        *guard = Some((canon.clone(), mtime, session));
    }
    let (loaded_path, loaded_mtime, session) = guard.as_mut()?;
    let res = session.execute(command, params.clone()).map_err(|e| e.to_string()).and_then(|value| {
        if save {
            session.execute("file.save", json!({})).map_err(|e| e.to_string())?;
        }
        Ok(value)
    });
    if res.is_ok() {
        *loaded_mtime = std::fs::metadata(&*loaded_path).and_then(|m| m.modified()).ok();
    }
    Some(res)
}

pub struct CraftHub;

enum TcpOutcome {
    Unreachable,
    DifferentDocumentOpen,
    Connected(Result<Value>),
}

impl CraftHub {
    /// Registers the FilmCraft that runs inside this process, so film commands reach its window.
    pub fn set_film_host(host: FilmHost) {
        let _ = FILM_HOST.set(host);
    }

    /// Registers the EffectCraft that runs inside this process, so effect/rig/character commands reach its window.
    pub fn set_effect_host(host: FilmHost) {
        let _ = EFFECT_HOST.set(host);
    }

    /// Registers the in-process EffectCraft window opener (`effectcraft-ui-egui` via `eframe`).
    pub fn set_effect_open_host(host: OpenHost) {
        let _ = EFFECT_OPEN_HOST.set(host);
    }

    fn in_process_window(target: CraftTarget, doc: &Path, command: &str, params: &Value, save: bool) -> Option<Result<Value>> {
        let window_res = match target {
            CraftTarget::Film => FILM_HOST.get().and_then(|host| {
                Some(host(doc, command, params)?.and_then(|value| {
                    if save {
                        host(doc, "file.save", &json!({})).unwrap_or_else(|| Err("FilmCraft closed before saving".into()))?;
                    }
                    Ok(value)
                }))
            }),
            CraftTarget::Effect => EFFECT_HOST.get().and_then(|host| {
                Some(host(doc, command, params)?.and_then(|value| {
                    if save {
                        host(doc, "file.save", &json!({})).unwrap_or_else(|| Err("EffectCraft closed before saving".into()))?;
                    }
                    Ok(value)
                }))
            }),
            _ => None,
        }?;
        Some(window_res.map_err(|error| EngineError::CommandFailed(command.to_string(), error)))
    }

    /// Runs `command` in the in-process FilmCraft or EffectCraft when it has `doc` open (or via the
    /// shared embedded engine session for fast read/render calls), saving afterwards when asked.
    pub(crate) fn in_process(target: CraftTarget, doc: Option<&Path>, command: &str, params: &Value, save: bool) -> Option<Result<Value>> {
        let doc = doc?;
        if let Some(res) = Self::in_process_window(target, doc, command, params, save) {
            return Some(res);
        }
        let result = match target {
            CraftTarget::Film => run_embedded_film(doc, command, params, save),
            CraftTarget::Effect => run_embedded_effect(doc, command, params, save),
            _ => None,
        }?;
        Some(result.map_err(|error| EngineError::CommandFailed(command.to_string(), error)))
    }

    /// Run a batch of EffectCraft steps directly against the live window (if open on `project`) or
    /// the in-process `effectcraft_engine::Session`.
    pub(crate) fn run_effect_steps(project: Option<&Path>, save: bool, steps: &[(String, Value)]) -> Result<Vec<Value>> {
        run_effect_steps(project, save, steps)
    }

    /// Run a batch of FilmCraft steps directly against the live window (if open on `project`) or the
    /// in-process `filmcraft_engine::Session` with `filmcraft_gpu::ExportRenderer` registered.
    pub(crate) fn run_film_steps(project: Option<&Path>, fresh: bool, save: bool, steps: &[Value]) -> Result<Vec<Value>> {
        if !fresh
            && let Some(project) = project
            && project.is_file()
        {
            let mut pairs = Vec::with_capacity(steps.len());
            for step in steps {
                let id = step["id"]
                    .as_str()
                    .or_else(|| step["command"].as_str())
                    .ok_or_else(|| EngineError::InvalidParams(format!("missing command id in {step}")))?;
                let params = step.get("params").cloned().unwrap_or_else(|| json!({}));
                pairs.push((id, params));
            }
            if let Some(live_res) = try_live_batch(CraftTarget::Film, project, &pairs, save) {
                return live_res;
            }
        }
        ensure_filmcraft_gpu();
        let mut session = filmcraft_engine::Session::default();
        if !fresh
            && let Some(p) = project
            && p.is_file()
        {
            session.execute("file.open", json!({ "path": p })).map_err(|e| EngineError::CommandFailed("file.open".into(), e.to_string()))?;
        }
        let mut results = Vec::with_capacity(steps.len());
        for step in steps {
            let id =
                step["id"].as_str().or_else(|| step["command"].as_str()).ok_or_else(|| EngineError::InvalidParams(format!("missing command id in {step}")))?;
            let params = step.get("params").cloned().unwrap_or_else(|| json!({}));
            let res = session.execute(id, params).map_err(|e| EngineError::CommandFailed(id.to_string(), e.to_string()))?;
            results.push(res);
        }
        if save && let Some(p) = project {
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let save_res = if !fresh && p.is_file() { session.execute("file.save", json!({})) } else { session.execute("file.saveAs", json!({ "path": p })) };
            save_res.map_err(|e| EngineError::CommandFailed("file.save".into(), e.to_string()))?;
            if let Some(cache) = HEADLESS_FILM.get()
                && let Ok(mut guard) = cache.lock()
            {
                *guard = None;
            }
        }
        Ok(results)
    }

    /// Render a single Program frame at `tick` to `output` PNG using the live FilmCraft window if open,
    /// or directly in-process via `filmcraft_engine::Session::try_render_program_with_offline_at`.
    pub(crate) fn render_film_frame(project: &Path, tick: crate::time::Tick, scale: f32, output: &Path) -> Result<()> {
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if let Some(res) = Self::try_live(
            CraftTarget::Film,
            project,
            "file.exportFrame",
            &json!({
                "path": output,
                "format": "png",
                "target": "program",
                "time": tick.0
            }),
            false,
        ) {
            res?;
            return Ok(());
        }
        ensure_filmcraft_gpu();
        let cache = HEADLESS_FILM.get_or_init(|| std::sync::Mutex::new(None));
        let mut guard = cache.lock().map_err(|e| EngineError::CommandFailed("render".into(), e.to_string()))?;
        let canon = std::fs::canonicalize(project).unwrap_or_else(|_| project.to_path_buf());
        let mtime = std::fs::metadata(&canon).and_then(|m| m.modified()).ok();
        let needs_open = match guard.as_ref() {
            Some((loaded_path, loaded_mtime, _)) => loaded_path != &canon || *loaded_mtime != mtime,
            None => true,
        };
        if needs_open {
            let mut session = filmcraft_engine::Session::default();
            session.execute("file.open", json!({ "path": canon })).map_err(|e| EngineError::CommandFailed("file.open".into(), e.to_string()))?;
            *guard = Some((canon, mtime, session));
        }
        let (_, _, session) = guard.as_mut().ok_or_else(|| EngineError::CommandFailed("render".into(), "no FilmCraft session".into()))?;
        session.set_playhead(filmcraft_time::Tick(tick.0));
        let (img, _) =
            session.try_render_program_with_offline_at(scale, session.playhead()).map_err(|e| EngineError::CommandFailed("render".into(), e.to_string()))?;
        let rgba = img.over_black_rgba8();
        image::save_buffer(output, &rgba, img.w as u32, img.h as u32, image::ColorType::Rgba8)
            .map_err(|e| EngineError::CommandFailed("render".into(), e.to_string()))?;
        Ok(())
    }

    /// Probe a media file directly in-process via `filmcraft_codecs` & `filmcraft_media`.
    pub(crate) fn probe_film_media(filename: &Path) -> Result<Value> {
        let bytes: std::sync::Arc<[u8]> =
            std::fs::read(filename).map_err(|e| EngineError::InvalidParams(format!("cannot read {}: {e}", filename.display())))?.into();
        let name = filename.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| filename.to_string_lossy().into_owned());
        let src = filmcraft_codecs::open_bytes(&name, bytes.clone())
            .map_err(|e| EngineError::InvalidParams(format!("cannot inspect {}: {e}", filename.display())))?;
        let mut info =
            serde_json::to_value(src.info()).map_err(|e| EngineError::InvalidParams(format!("cannot serialize media info for {}: {e}", filename.display())))?;
        let head = &bytes[..bytes.len().min(70_000)];
        if filmcraft_media::wav::sniff(head)
            && let Ok(w) = filmcraft_media::wav::WavSource::parse(&name, bytes)
            && let Some(tr) = w.time_reference()
        {
            info["bwf"] = json!({ "timeReference": tr });
        }
        Ok(info)
    }

    /// Execute a command against a Craft target application.
    ///
    /// 1. Connects to the live app (in-process window or loopback TCP `--control`). When `doc` is
    ///    given, verifies the live app has that exact canonical document open before dispatching.
    /// 2. Falls back to the in-process Craft engine (`filmcraft_engine` / `effectcraft_engine`) or
    ///    `<app>-cli` ONLY when TCP connect is refused/unreachable or the live app is editing a
    ///    different document — never after a connected live command errors or times out.
    pub fn execute(target: CraftTarget, doc: Option<&Path>, command: &str, params: &Value) -> Result<Value> {
        if let Some(doc_path) = doc
            && let Some(result) = Self::in_process_window(target, doc_path, command, params, false)
        {
            return result;
        }
        let addr = target_addr(target)?;
        let token = target_token(target);
        match call_tcp_at(target, addr, token.as_deref(), doc, command, params, false, Duration::from_millis(250)) {
            TcpOutcome::Connected(res) => res,
            TcpOutcome::Unreachable | TcpOutcome::DifferentDocumentOpen => {
                if let Some(result) = Self::in_process(target, doc, command, params, false) {
                    return result;
                }
                call_cli_with_save(target, doc, command, params, false)
            }
        }
    }

    /// Execute a mutating command against `doc` and persist it (`file.save` / `app.save` on a live
    /// `--control` session, or in-process / `--save` via `<app>-cli` when not open live).
    pub fn execute_and_save(target: CraftTarget, doc: &Path, command: &str, params: &Value) -> Result<Value> {
        if let Some(result) = Self::in_process_window(target, doc, command, params, true) {
            return result;
        }
        let addr = target_addr(target)?;
        let token = target_token(target);
        match call_tcp_at(target, addr, token.as_deref(), Some(doc), command, params, true, Duration::from_millis(250)) {
            TcpOutcome::Connected(res) => res,
            TcpOutcome::Unreachable | TcpOutcome::DifferentDocumentOpen => {
                if let Some(result) = Self::in_process(target, Some(doc), command, params, true) {
                    return result;
                }
                call_cli_with_save(target, Some(doc), command, params, true)
            }
        }
    }

    /// Send `command` to the live app ONLY if it is running AND currently has `doc` open.
    pub fn try_live(target: CraftTarget, doc: &Path, command: &str, params: &Value, save: bool) -> Option<Result<Value>> {
        if let Some(result) = Self::in_process_window(target, doc, command, params, save) {
            return Some(result);
        }
        let addr = target_addr(target).ok()?;
        let token = target_token(target);
        match call_tcp_at(target, addr, token.as_deref(), Some(doc), command, params, save, Duration::from_millis(250)) {
            TcpOutcome::Connected(res) => Some(res),
            TcpOutcome::Unreachable | TcpOutcome::DifferentDocumentOpen => None,
        }
    }

    /// Opens `file` in the installed app on its control port (or the in-process `eframe` window
    /// when registered), launching the app if it is not running, so later commands reach that
    /// window through [`CraftHub::try_live`].
    pub fn open_live(target: CraftTarget, file: &Path) -> Result<Value> {
        let addr = target_addr(target)?;
        if TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok() {
            return Self::live_execute(target, None, "file.open", &json!({"path": file}));
        }
        if target == CraftTarget::Effect
            && let Some(open_in_proc) = EFFECT_OPEN_HOST.get()
            && let Ok(val) = open_in_proc(file)
        {
            return Ok(val);
        }
        Command::new(crate::workspace::resolve_tool(target.name()))
            .arg("--control")
            .arg(addr.port().to_string())
            .arg(file)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| EngineError::ExternalTool(target.name().into(), e.to_string()))?;
        for _ in 0..120 {
            if TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok() {
                return Ok(json!({"ok": true, "port": addr.port()}));
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        Err(EngineError::ExternalTool(target.name().into(), format!("did not open its control port {addr}")))
    }

    /// The standard loopback control address for `target` (`<PORT_ENV>` override or suite default).
    pub fn control_addr(target: CraftTarget) -> Result<SocketAddr> {
        target_addr(target)
    }

    /// Execute an app-level command against the live target's `--control` endpoint only.
    ///
    /// Unlike [`CraftHub::execute`] there is no open-document gate — the command may target a
    /// document that is not open yet (e.g. `file.open`) — and there is no `<app>-cli` fallback:
    /// when no live app answers, the caller decides (typically by launching the editor).
    /// `port` overrides the standard [`CraftHub::control_addr`] endpoint.
    ///
    /// A transport failure (nothing listening, or a listener that does not answer the Craft
    /// control envelope) is an error; a command-level failure arrives as
    /// [`EngineError::CommandFailed`] and means the endpoint does speak the protocol.
    pub fn live_execute(target: CraftTarget, port: Option<u16>, command: &str, params: &Value) -> Result<Value> {
        let addr = match port {
            Some(port) => SocketAddr::from(([127, 0, 0, 1], port)),
            None => target_addr(target)?,
        };
        let token = target_token(target);
        match call_tcp_at(target, addr, token.as_deref(), None, command, params, false, Duration::from_millis(250)) {
            TcpOutcome::Connected(res) => res,
            TcpOutcome::Unreachable => Err(EngineError::ExternalTool(target.name().into(), format!("no live --control endpoint on {addr}"))),
            TcpOutcome::DifferentDocumentOpen => Err(EngineError::ExternalTool(target.name().into(), format!("document gate engaged on {addr}"))),
        }
    }
}

fn target_addr(target: CraftTarget) -> Result<SocketAddr> {
    let port = std::env::var(target.port_env()).ok().and_then(|p| p.parse::<u16>().ok()).unwrap_or_else(|| target.default_port());
    format!("127.0.0.1:{port}").parse().map_err(|e| EngineError::InvalidParams(format!("invalid control address: {e}")))
}

pub(crate) fn same_canonical_path(a: &Path, b: &Path) -> bool {
    matches!((std::fs::canonicalize(a), std::fs::canonicalize(b)), (Ok(ca), Ok(cb)) if ca == cb)
}

struct ControlConn {
    target: CraftTarget,
    inline_token: Option<String>,
    writer: TcpStream,
    reader: BufReader<TcpStream>,
    next_id: u64,
}

impl ControlConn {
    fn send_raw(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let mut req_obj = serde_json::Map::new();
        req_obj.insert("id".into(), json!(id));
        if let Some(tok) = &self.inline_token {
            req_obj.insert("token".into(), json!(tok));
        }
        req_obj.insert("method".into(), json!(method));
        req_obj.insert("params".into(), params);

        let mut payload = serde_json::to_vec(&Value::Object(req_obj)).map_err(|e| EngineError::InvalidParams(format!("serialize error: {e}")))?;
        payload.push(b'\n');
        self.writer.write_all(&payload).map_err(|e| EngineError::ExternalTool(self.target.name().into(), format!("control write error: {e}")))?;
        self.writer.flush().map_err(|e| EngineError::ExternalTool(self.target.name().into(), format!("control flush error: {e}")))?;

        let mut response_line = String::new();
        self.reader.read_line(&mut response_line).map_err(|e| EngineError::ExternalTool(self.target.name().into(), format!("control read error: {e}")))?;
        if response_line.trim().is_empty() {
            return Err(EngineError::ExternalTool(self.target.name().into(), "empty response from control server".into()));
        }
        let parsed: Value =
            serde_json::from_str(&response_line).map_err(|e| EngineError::ExternalTool(self.target.name().into(), format!("invalid JSON in reply: {e}")))?;
        if parsed["ok"] == true {
            Ok(parsed.get("result").cloned().unwrap_or(Value::Null))
        } else {
            let err = parsed["error"].as_str().unwrap_or("unknown control error");
            Err(EngineError::CommandFailed(method.to_string(), err.to_string()))
        }
    }

    fn execute_engine(&mut self, command: &str, params: &Value) -> Result<Value> {
        let (method, body) = match self.target {
            CraftTarget::Texture => ("command.run", json!({"id": command, "params": params})),
            _ => ("engine.execute", json!({"command": command, "params": params})),
        };
        self.send_raw(method, body).map_err(|e| match e {
            EngineError::CommandFailed(_, msg) => EngineError::CommandFailed(command.to_string(), msg),
            other => other,
        })
    }

    fn has_document_open(&mut self, expected: &Path) -> Result<bool> {
        match self.target {
            CraftTarget::Film => {
                let info = self.execute_engine("project.inspect", &json!({}))?;
                Ok(info["path"].as_str().is_some_and(|p| same_canonical_path(Path::new(p), expected)))
            }
            CraftTarget::Design => {
                let info = self.execute_engine("document.inspect", &json!({}))?;
                Ok(info["path"].as_str().is_some_and(|p| same_canonical_path(Path::new(p), expected)))
            }
            CraftTarget::Effect => {
                let info = self.execute_engine("project.summary", &json!({}))?;
                Ok(info["path"].as_str().is_some_and(|p| same_canonical_path(Path::new(p), expected)))
            }
            CraftTarget::Photo => {
                let info = self.send_raw("ui.inspect", json!({}))?;
                let active = info.pointer("/session/active").and_then(Value::as_u64).and_then(|index| usize::try_from(index).ok());
                let document = active.and_then(|index| info.pointer("/session/documents")?.as_array()?.get(index));
                Ok(document.and_then(|d| d["path"].as_str()).is_some_and(|p| same_canonical_path(Path::new(p), expected)))
            }
            CraftTarget::Vector => {
                let info = self.send_raw("document.inspect", json!({}))?;
                Ok(info["path"].as_str().is_some_and(|p| same_canonical_path(Path::new(p), expected)))
            }
            CraftTarget::Texture => {
                let info = self.send_raw("session.inspect", json!({}))?;
                Ok(info["path"].as_str().is_some_and(|p| same_canonical_path(Path::new(p), expected)))
            }
            CraftTarget::Ani => {
                let info = self.execute_engine("doc.info", &json!({}))?;
                Ok(info["path"].as_str().is_some_and(|p| same_canonical_path(Path::new(p), expected)))
            }
        }
    }

    fn save_open_document(&mut self, doc: &Path) -> Result<()> {
        match self.target {
            CraftTarget::Film | CraftTarget::Design | CraftTarget::Vector => {
                let _ = self.execute_engine("file.save", &json!({}))?;
            }
            CraftTarget::Effect | CraftTarget::Texture | CraftTarget::Ani => {
                let _ = self.execute_engine("file.save", &json!({"path": doc, "confirm": true}))?;
            }
            CraftTarget::Photo => {
                let _ = self.execute_engine("app.save", &json!({}))?;
            }
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn call_tcp_at(
    target: CraftTarget,
    addr: SocketAddr,
    token: Option<&str>,
    doc: Option<&Path>,
    command: &str,
    params: &Value,
    save: bool,
    connect_timeout: Duration,
) -> TcpOutcome {
    let Ok(stream) = TcpStream::connect_timeout(&addr, connect_timeout) else {
        return TcpOutcome::Unreachable;
    };
    let setup = (|| -> Result<ControlConn> {
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|e| EngineError::ExternalTool(target.name().into(), format!("set read timeout: {e}")))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| EngineError::ExternalTool(target.name().into(), format!("set write timeout: {e}")))?;
        let writer = stream.try_clone().map_err(|e| EngineError::ExternalTool(target.name().into(), format!("clone stream: {e}")))?;
        let inline_token = if target == CraftTarget::Film { token.map(str::to_owned) } else { None };
        let mut conn = ControlConn { target, inline_token, writer, reader: BufReader::new(stream), next_id: 1 };
        if matches!(target, CraftTarget::Photo | CraftTarget::Texture)
            && let Some(tok) = token
        {
            conn.send_raw("auth", json!({"token": tok}))?;
        }
        Ok(conn)
    })();
    let mut conn = match setup {
        Ok(c) => c,
        Err(e) => return TcpOutcome::Connected(Err(e)),
    };

    if let Some(expected) = doc {
        match conn.has_document_open(expected) {
            Ok(true) => {}
            Ok(false) => return TcpOutcome::DifferentDocumentOpen,
            Err(e) => return TcpOutcome::Connected(Err(e)),
        }
    }

    let result = conn.execute_engine(command, params).and_then(|val| {
        if save && let Some(path) = doc {
            conn.save_open_document(path)?;
        }
        Ok(val)
    });
    TcpOutcome::Connected(result)
}

/// Run `steps` against the live app when it is running AND has `doc` open (in-process or via
/// loopback `--control`), saving the open document when `save` is true.
pub(crate) fn try_live_batch(target: CraftTarget, doc: &Path, steps: &[(&str, Value)], save: bool) -> Option<Result<Vec<Value>>> {
    if let Some((host, app_label)) = match target {
        CraftTarget::Film => FILM_HOST.get().map(|h| (h, "FilmCraft")),
        CraftTarget::Effect => EFFECT_HOST.get().map(|h| (h, "EffectCraft")),
        _ => None,
    } {
        let mut results = Vec::with_capacity(steps.len());
        for (idx, (id, params)) in steps.iter().enumerate() {
            match host(doc, id, params) {
                Some(Ok(v)) => results.push(v),
                Some(Err(e)) => return Some(Err(EngineError::CommandFailed((*id).to_string(), e))),
                None if idx == 0 => break,
                None => return Some(Err(EngineError::CommandFailed((*id).to_string(), format!("{app_label} closed during batch")))),
            }
        }
        if results.len() == steps.len() {
            if save {
                match host(doc, "file.save", &json!({})) {
                    Some(Ok(_)) => {}
                    Some(Err(e)) => return Some(Err(EngineError::CommandFailed("file.save".into(), e))),
                    None => return Some(Err(EngineError::CommandFailed("file.save".into(), format!("{app_label} closed before saving")))),
                }
            }
            return Some(Ok(results));
        }
    }

    let addr = target_addr(target).ok()?;
    let token = target_token(target);
    let Ok(stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(250)) else {
        return None;
    };
    let setup = (|| -> Result<ControlConn> {
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|e| EngineError::ExternalTool(target.name().into(), format!("set read timeout: {e}")))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| EngineError::ExternalTool(target.name().into(), format!("set write timeout: {e}")))?;
        let writer = stream.try_clone().map_err(|e| EngineError::ExternalTool(target.name().into(), format!("clone stream: {e}")))?;
        let inline_token = if target == CraftTarget::Film { token.as_deref().map(str::to_owned) } else { None };
        let mut conn = ControlConn { target, inline_token, writer, reader: BufReader::new(stream), next_id: 1 };
        if matches!(target, CraftTarget::Photo | CraftTarget::Texture)
            && let Some(tok) = token.as_deref()
        {
            conn.send_raw("auth", json!({"token": tok}))?;
        }
        Ok(conn)
    })();
    let mut conn = match setup {
        Ok(c) => c,
        Err(e) => return Some(Err(e)),
    };
    match conn.has_document_open(doc) {
        Ok(true) => {}
        Ok(false) => return None,
        Err(e) => return Some(Err(e)),
    }
    let result = (|| -> Result<Vec<Value>> {
        let mut out = Vec::with_capacity(steps.len());
        for (id, params) in steps {
            out.push(conn.execute_engine(id, params)?);
        }
        if save {
            conn.save_open_document(doc)?;
        }
        Ok(out)
    })();
    Some(result)
}

fn call_cli_with_save(target: CraftTarget, doc: Option<&Path>, command: &str, params: &Value, save: bool) -> Result<Value> {
    let cli = crate::workspace::resolve_tool(target.cli_name());
    let mut cmd = Command::new(&cli);
    let params_str = serde_json::to_string(params).map_err(|e| EngineError::InvalidParams(e.to_string()))?;

    match target {
        CraftTarget::Film => {
            if let Some(path) = doc {
                cmd.arg("--project").arg(path);
                if save {
                    cmd.arg("--save");
                }
            }
            cmd.arg("exec").arg(command).arg(&params_str);
        }
        CraftTarget::Effect => {
            cmd.arg("--json");
            if let Some(path) = doc {
                cmd.arg("--project").arg(path);
                if save {
                    cmd.arg("--save");
                }
            } else {
                cmd.arg("--empty");
            }
            cmd.arg("exec").arg(command).arg("--params").arg(&params_str);
        }
        CraftTarget::Vector => {
            cmd.arg("run");
            if let Some(path) = doc {
                cmd.arg("--in").arg(path);
                if save {
                    cmd.arg("--out").arg(path);
                }
            }
            cmd.arg("--cmd").arg(command).arg("--params").arg(&params_str);
        }
        CraftTarget::Photo => {
            cmd.arg("run");
            if let Some(path) = doc {
                cmd.arg(path);
                if save {
                    cmd.arg("--out").arg(path);
                }
            } else {
                cmd.arg("--new").arg("{}");
            }
            cmd.arg("--cmd").arg(command).arg("--params").arg(&params_str);
        }
        CraftTarget::Design => {
            if save && let Some(path) = doc {
                cmd.arg("script").arg("--in").arg(path).arg("--save").arg(path);
                cmd.arg("--cmd").arg(format!("{command}={params_str}"));
            } else {
                cmd.arg("run");
                if let Some(path) = doc {
                    cmd.arg("--in").arg(path);
                }
                cmd.arg("--cmd").arg(format!("{command}={params_str}"));
            }
        }
        CraftTarget::Texture => {
            if command == "info" {
                let input = doc.and_then(|p| p.to_str()).or_else(|| params["path"].as_str()).unwrap_or("builtin:material_ball");
                cmd.arg("info").arg(input);
            } else {
                cmd.arg("run");
                if let Some(path) = doc {
                    cmd.arg(path);
                } else {
                    cmd.arg("builtin:material_ball");
                }
                cmd.arg(command).arg(&params_str);
                if save && let Some(path) = doc {
                    cmd.arg("-o").arg(path);
                }
            }
        }
        CraftTarget::Ani => {
            cmd.arg("exec");
            if let Some(path) = doc {
                cmd.arg(path);
            } else {
                cmd.arg("--sample");
            }
            cmd.arg(command).arg(&params_str);
            if save && let Some(path) = doc {
                cmd.arg("--save").arg(path);
            }
        }
    }

    let output = cmd.output().map_err(|e| EngineError::ExternalTool(target.cli_name().to_string(), e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(EngineError::CommandFailed(command.to_string(), stderr.trim().to_string()));
    }

    let stdout_trim = std::str::from_utf8(&output.stdout).map_err(|e| EngineError::InvalidParams(format!("invalid UTF-8 from CLI: {e}")))?.trim();

    if stdout_trim.is_empty() || stdout_trim == "null" {
        return Ok(Value::Null);
    }

    let parsed: Value = serde_json::from_str(stdout_trim).map_err(|e| EngineError::InvalidParams(format!("invalid JSON from CLI: {e}")))?;

    if let Some(res) = parsed.get("result") { Ok(res.clone()) } else { Ok(parsed) }
}
