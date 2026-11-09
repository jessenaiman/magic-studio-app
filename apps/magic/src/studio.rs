//! FilmCraft (`filmcraft-ui-egui`) and EffectCraft (`effectcraft-ui-egui`) in windows of this process.
//!
//! Tauri owns the main thread, so each eframe app runs on a thread of its own, which stays
//! alive between windows because eframe reuses its event loop per thread.
use effectcraft_ui_egui::{ControlRequest as EffectControlRequest, EffectcraftApp};
use filmcraft_ui_egui::{ControlRequest, FilmcraftApp};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

type Ready = Sender<Result<(), String>>;

struct Live {
    path: PathBuf,
    control: Sender<ControlRequest>,
    ctx: eframe::egui::Context,
}

struct Studio {
    open: Sender<(PathBuf, Ready)>,
    live: Mutex<Option<Live>>,
}

static STUDIO: OnceLock<Studio> = OnceLock::new();

struct EffectLive {
    path: PathBuf,
    control: Sender<EffectControlRequest>,
    ctx: eframe::egui::Context,
}

struct EffectStudio {
    open: Sender<(PathBuf, Ready)>,
    live: Mutex<Option<EffectLive>>,
}

static EFFECT_STUDIO: OnceLock<EffectStudio> = OnceLock::new();

fn studio() -> &'static Studio {
    STUDIO.get_or_init(|| {
        let (open, requests) = mpsc::channel::<(PathBuf, Ready)>();
        // A thread that cannot start leaves `open` disconnected, which `open` reports.
        let _ = std::thread::Builder::new().name("filmcraft".into()).spawn(move || {
            while let Ok((path, ready)) = requests.recv() {
                run(path, ready);
            }
        });
        Studio { open, live: Mutex::new(None) }
    })
}

fn set_live(live: Option<Live>) {
    if let Ok(mut slot) = studio().live.lock() {
        *slot = live;
    }
}

/// Runs FilmCraft on this thread until its window closes.
fn run(path: PathBuf, ready: Ready) {
    let (control, requests) = mpsc::channel();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_title("FilmCraft").with_inner_size([1600.0, 980.0]).with_drag_and_drop(true),
        event_loop_builder: Some(Box::new(|builder| {
            #[cfg(target_os = "linux")]
            {
                use winit::platform::x11::EventLoopBuilderExtX11;
                EventLoopBuilderExtX11::with_any_thread(builder, true);
                // As FilmCraft does: winit has no file drag-and-drop on Wayland.
                if std::env::var_os("DISPLAY").is_some() {
                    EventLoopBuilderExtX11::with_x11(builder);
                }
            }
        })),
        ..Default::default()
    };
    let created = ready.clone();
    let opened = path.clone();
    let result = eframe::run_native(
        "FilmCraft",
        options,
        Box::new(move |cc| {
            filmcraft_ui_egui::install_ime_reset_callbacks(&cc.egui_ctx);
            let mut session = filmcraft_engine::Session::default();
            session.execute("file.open", json!({ "path": opened })).map_err(|e| e.to_string())?;
            let mut app = FilmcraftApp::new(session).with_control(requests);
            if let Some(state) = cc.wgpu_render_state.clone() {
                app.set_wgpu(state);
            }
            match crate::studio_audio::open() {
                Ok(audio) => app.audio = Some(audio),
                Err(error) => log::warn!("FilmCraft audio output unavailable: {error}"),
            }
            set_live(Some(Live { path: opened, control, ctx: cc.egui_ctx.clone() }));
            let _ = created.send(Ok(()));
            Ok(Box::new(app))
        }),
    );
    set_live(None);
    // After a window that opened, nobody is waiting on this any more.
    if let Err(error) = result {
        let _ = ready.send(Err(error.to_string()));
    }
}

fn request(live: &Live, method: &str, params: Value) -> Result<Value, String> {
    let (request, reply) = ControlRequest::new(method, params);
    live.control.send(request).map_err(|_| "FilmCraft has closed".to_string())?;
    live.ctx.request_repaint();
    reply.recv_timeout(Duration::from_secs(60)).map_err(|_| "FilmCraft did not answer within 60 seconds".to_string())
}

/// Shows `path` in FilmCraft, opening its window if it is not open.
pub(super) fn open(path: PathBuf) -> Result<Value, String> {
    let studio = studio();
    {
        let mut slot = studio.live.lock().map_err(|e| e.to_string())?;
        if let Some(live) = slot.as_mut() {
            if live.path != path {
                request(live, "engine.execute", json!({"command": "file.open", "params": {"path": path}}))?;
                live.path = path.clone();
            }
            request(live, "ui.focus", json!({}))?;
            return Ok(json!({"ok": true, "timeline": path, "window": "focused"}));
        }
    }
    let (ready, answer) = mpsc::channel();
    studio.open.send((path.clone(), ready)).map_err(|_| "FilmCraft could not start".to_string())?;
    answer.recv().map_err(|_| "FilmCraft stopped before its window opened".to_string())??;
    Ok(json!({"ok": true, "timeline": path, "window": "opened"}))
}

/// Sends the engine's film commands to the open window, so they never edit its file behind it.
pub(super) fn install_film_host() {
    magic::CraftHub::set_film_host(Box::new(|doc, command, params| {
        let slot = STUDIO.get()?.live.lock().ok()?;
        let live = slot.as_ref().filter(|live| std::fs::canonicalize(&live.path).ok() == std::fs::canonicalize(doc).ok())?;
        Some(request(live, "engine.execute", json!({"command": command, "params": params})).and_then(|reply| {
            if reply["ok"] == true { Ok(reply["result"].clone()) } else { Err(reply["error"].as_str().unwrap_or("FilmCraft refused the command").to_owned()) }
        }))
    }));
}

/// `video.open`: the project's timeline in FilmCraft.
pub(super) fn open_project(app: &tauri::AppHandle) -> Result<Value, String> {
    let film = super::state(app).session.lock().map_err(|e| e.to_string())?.execute("video.film", json!({})).map_err(|e| e.to_string())?;
    let path = film["timeline"].as_object().filter(|t| t["exists"] == true).and_then(|t| t["path"].as_str());
    open(path.ok_or("No FilmCraft timeline yet; run Assemble first")?.into())
}

/// `ui.filmcraft {method, params?}`: any FilmCraft control method, on the open window.
pub(super) fn control(params: &Value) -> Result<Value, String> {
    super::only_keys(params, "ui.filmcraft", &["method", "params"])?;
    let method = params["method"].as_str().ok_or("ui.filmcraft needs a method")?;
    let slot = studio().live.lock().map_err(|e| e.to_string())?;
    let live = slot.as_ref().ok_or("FilmCraft is not open; run video.open first")?;
    request(live, method, params.get("params").cloned().unwrap_or_else(|| json!({})))
}

fn effect_studio() -> &'static EffectStudio {
    EFFECT_STUDIO.get_or_init(|| {
        let (open, requests) = mpsc::channel::<(PathBuf, Ready)>();
        let _ = std::thread::Builder::new().name("effectcraft".into()).spawn(move || {
            while let Ok((path, ready)) = requests.recv() {
                run_effect(path, ready);
            }
        });
        EffectStudio { open, live: Mutex::new(None) }
    })
}

fn set_effect_live(live: Option<EffectLive>) {
    if let Ok(mut slot) = effect_studio().live.lock() {
        *slot = live;
    }
}

/// Runs EffectCraft (`effectcraft_ui_egui::EffectcraftApp`) on this thread until its window closes.
fn run_effect(path: PathBuf, ready: Ready) {
    let (control, requests) = mpsc::channel();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_title("EffectCraft").with_inner_size([1600.0, 980.0]).with_drag_and_drop(true),
        event_loop_builder: Some(Box::new(|builder| {
            #[cfg(target_os = "linux")]
            {
                use winit::platform::x11::EventLoopBuilderExtX11;
                EventLoopBuilderExtX11::with_any_thread(builder, true);
                if std::env::var_os("DISPLAY").is_some() {
                    EventLoopBuilderExtX11::with_x11(builder);
                }
            }
        })),
        ..Default::default()
    };
    let created = ready.clone();
    let opened = path.clone();
    let result = eframe::run_native(
        "EffectCraft",
        options,
        Box::new(move |cc| {
            let mut session = effectcraft_engine::Session::default();
            session.execute("file.open", json!({ "path": opened })).map_err(|e| e.to_string())?;
            let mut app = EffectcraftApp::new(session).with_control(requests);
            app.ui.start_screen = false;
            set_effect_live(Some(EffectLive { path: opened, control, ctx: cc.egui_ctx.clone() }));
            let _ = created.send(Ok(()));
            Ok(Box::new(app))
        }),
    );
    set_effect_live(None);
    if let Err(error) = result {
        let _ = ready.send(Err(error.to_string()));
    }
}

fn request_effect(live: &EffectLive, method: &str, params: Value) -> Result<Value, String> {
    let (request, reply) = EffectControlRequest::new(method, params);
    live.control.send(request).map_err(|_| "EffectCraft has closed".to_string())?;
    live.ctx.request_repaint();
    reply.recv_timeout(Duration::from_secs(60)).map_err(|_| "EffectCraft did not answer within 60 seconds".to_string())
}

/// Shows `path` in in-process `EffectcraftApp` (`effectcraft-ui-egui`), opening its window if it is not open.
pub(super) fn open_effect(path: PathBuf) -> Result<Value, String> {
    let studio = effect_studio();
    {
        let mut slot = studio.live.lock().map_err(|e| e.to_string())?;
        if let Some(live) = slot.as_mut() {
            if live.path != path {
                request_effect(live, "engine.execute", json!({"command": "file.open", "params": {"path": path}}))?;
                live.path = path.clone();
            }
            request_effect(live, "ui.focus", json!({}))?;
            return Ok(json!({"ok": true, "app": "effectcraft", "file": path, "window": "focused"}));
        }
    }
    let (ready, answer) = mpsc::channel();
    studio.open.send((path.clone(), ready)).map_err(|_| "EffectCraft could not start".to_string())?;
    answer.recv().map_err(|_| "EffectCraft stopped before its window opened".to_string())??;
    Ok(json!({"ok": true, "app": "effectcraft", "file": path, "window": "opened"}))
}

/// Sends the engine's EffectCraft commands to the open in-process window, so they never edit its file behind it.
pub(super) fn install_effect_host() {
    magic::CraftHub::set_effect_host(Box::new(|doc, command, params| {
        let slot = EFFECT_STUDIO.get()?.live.lock().ok()?;
        let live = slot.as_ref().filter(|live| std::fs::canonicalize(&live.path).ok() == std::fs::canonicalize(doc).ok())?;
        Some(request_effect(live, "engine.execute", json!({"command": command, "params": params})).and_then(|reply| {
            if reply["ok"] == true { Ok(reply["result"].clone()) } else { Err(reply["error"].as_str().unwrap_or("EffectCraft refused the command").to_owned()) }
        }))
    }));
    magic::CraftHub::set_effect_open_host(Box::new(|doc| open_effect(doc.to_path_buf())));
}

/// `ui.effectcraft {method, params?}`: any EffectCraft control method, on the open in-process window.
pub(super) fn control_effect(params: &Value) -> Result<Value, String> {
    super::only_keys(params, "ui.effectcraft", &["method", "params"])?;
    let method = params["method"].as_str().ok_or("ui.effectcraft needs a method")?;
    let slot = effect_studio().live.lock().map_err(|e| e.to_string())?;
    let live = slot.as_ref().ok_or("EffectCraft is not open; run animation.open or character.open first")?;
    request_effect(live, method, params.get("params").cloned().unwrap_or_else(|| json!({})))
}
