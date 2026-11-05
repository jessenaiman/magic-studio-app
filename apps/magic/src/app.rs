//! Main application shell — holds the shared engine session, serves the local
//! Unix socket for `magic-cli` agent coordination, and routes between
//! `ProjectList`, `ProjectView`, and `StoryboardView`.

use eframe::egui;
use magic_engine::Session;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

mod project_list;
mod project_view;
mod storyboard_view;
mod animation_view;
mod lyrics_view;

use project_list::{ProjectList, ProjectListAction};
use project_view::{ProjectViewAction, ProjectViewState};
use storyboard_view::{StoryboardAction, StoryboardViewState};
use animation_view::{AnimationAction, AnimationViewState};
use lyrics_view::{LyricsAction, LyricsViewState};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AppView {
    ProjectList,
    ProjectView,
    StoryboardView,
    AnimationView,
    LyricsView,
}

impl AppView {
    fn as_str(self) -> &'static str {
        match self {
            AppView::ProjectList => "projects",
            AppView::ProjectView => "project",
            AppView::StoryboardView => "storyboard",
            AppView::AnimationView => "animation",
            AppView::LyricsView => "lyrics",
        }
    }
}

pub struct SharedUiState {
    pub view: AppView,
    pub ctx: Option<egui::Context>,
    pub external_revision: AtomicU64,
}

pub struct magicApp {
    engine: Arc<Mutex<Session>>,
    shared_ui: Arc<Mutex<SharedUiState>>,
    seen_revision: u64,
    project_list: ProjectList,
    project_view: ProjectViewState,
    storyboard_view: StoryboardViewState,
    animation_view: AnimationViewState,
    lyrics_view: LyricsViewState,
    status: Option<String>,
    error: Option<String>,
    socket_path: Option<PathBuf>,
}

impl magicApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_project: Option<String>) -> Self {
        let mut session = Session::user_session();
        let mut initial_view = AppView::ProjectList;
        let mut error = None;

        if let Some(path) = initial_project {
            match session.execute("project.open", json!({ "path": path, "claim_owner": true })) {
                Ok(_) => initial_view = AppView::ProjectView,
                Err(e) => error = Some(e.to_string()),
            }
        }

        let engine = Arc::new(Mutex::new(session));
        let shared_ui = Arc::new(Mutex::new(SharedUiState {
            view: initial_view,
            ctx: Some(cc.egui_ctx.clone()),
            external_revision: AtomicU64::new(0),
        }));

        let socket_path = start_socket_listener(Arc::clone(&engine), Arc::clone(&shared_ui));

        Self {
            engine,
            shared_ui,
            seen_revision: 0,
            project_list: ProjectList::new(),
            project_view: ProjectViewState::default(),
            storyboard_view: StoryboardViewState::default(),
            animation_view: AnimationViewState::default(),
            lyrics_view: LyricsViewState::default(),
            status: None,
            error,
            socket_path,
        }
    }

    fn current_view(&self) -> AppView {
        self.shared_ui
            .lock()
            .map(|u| u.view)
            .unwrap_or(AppView::ProjectList)
    }

    fn set_view(&self, view: AppView) {
        if let Ok(mut u) = self.shared_ui.lock() {
            u.view = view;
        }
    }
}

impl Drop for magicApp {
    fn drop(&mut self) {
        if let Some(path) = &self.socket_path {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl eframe::App for magicApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let ext_rev = self
            .shared_ui
            .lock()
            .map(|u| u.external_revision.load(Ordering::Relaxed))
            .unwrap_or(0);

        let engine_arc = Arc::clone(&self.engine);
        let Ok(mut engine) = engine_arc.lock() else {
            return;
        };

        if ext_rev != self.seen_revision {
            self.seen_revision = ext_rev;
            self.project_list.reload(&mut engine);
            self.project_view.refresh(&mut engine);
            self.storyboard_view.reload(&engine);
        }

        let has_project = engine.project_path.is_some();
        let mut view = self.current_view();
        if !has_project && view != AppView::ProjectList {
            view = AppView::ProjectList;
            self.set_view(view);
        }

        egui::Panel::top("top_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("Magic Studio");
                ui.separator();

                if ui
                    .selectable_label(view == AppView::ProjectList, "Projects")
                    .clicked()
                {
                    self.project_list.reload(&mut engine);
                    view = AppView::ProjectList;
                    self.set_view(view);
                }

                ui.add_enabled_ui(has_project, |ui| {
                    if ui
                        .selectable_label(view == AppView::ProjectView, "Project Container")
                        .clicked()
                    {
                        self.project_view.refresh(&mut engine);
                        view = AppView::ProjectView;
                        self.set_view(view);
                    }
                    if ui
                        .selectable_label(view == AppView::StoryboardView, "Storyboard")
                        .clicked()
                    {
                        self.storyboard_view.reload(&engine);
                        view = AppView::StoryboardView;
                        self.set_view(view);
                    }
                    if ui
                        .selectable_label(view == AppView::AnimationView, "Animation")
                        .clicked()
                    {
                        self.animation_view.refresh(&mut engine);
                        view = AppView::AnimationView;
                        self.set_view(view);
                    }
                    if ui
                        .selectable_label(view == AppView::LyricsView, "Lyrics")
                        .clicked()
                    {
                        self.lyrics_view.refresh(&mut engine);
                        view = AppView::LyricsView;
                        self.set_view(view);
                    }
                });

                if let Some(path) = engine.project_path.clone() {
                    ui.separator();
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Project");
                    ui.label(format!("Active: {name}"));

                    if ui.button("Save").clicked() {
                        match engine.execute("project.save", json!({})) {
                            Ok(_) => {
                                self.status = Some("Project saved.".into());
                                self.error = None;
                            }
                            Err(e) => self.error = Some(e.to_string()),
                        }
                    }
                    if ui.button("Close Project").clicked() {
                        let _ = engine.execute("project.close", json!({}));
                        self.project_list.reload(&mut engine);
                        view = AppView::ProjectList;
                        self.set_view(view);
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if let Some(msg) = &self.status {
                        ui.label(msg);
                    }
                });
            });
        });

        if self.error.is_some() {
            egui::Panel::bottom("error_bar").show(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some(err) = &self.error {
                        ui.colored_label(egui::Color32::from_rgb(240, 90, 90), err);
                    }
                    if ui.small_button("Dismiss").clicked() {
                        self.error = None;
                    }
                });
            });
        }

        egui::CentralPanel::default().show(ui, |ui| match view {
            AppView::ProjectList => match self.project_list.ui(ui, &mut engine) {
                ProjectListAction::None => {}
                ProjectListAction::Opened(path) => {
                    self.error = None;
                    self.status = Some(format!("Opened {path}"));
                    self.project_view.refresh(&mut engine);
                    self.storyboard_view.reload(&engine);
                    self.set_view(AppView::ProjectView);
                }
                ProjectListAction::Error(e) => self.error = Some(e),
            },
            AppView::ProjectView => match self.project_view.ui(ui, &mut engine) {
                ProjectViewAction::None => {}
                ProjectViewAction::OpenStoryboard => {
                    self.storyboard_view.reload(&engine);
                    self.set_view(AppView::StoryboardView);
                }
                ProjectViewAction::SwitchProject(path) => {
                    match engine
                        .execute("project.open", json!({ "path": path, "claim_owner": true }))
                    {
                        Ok(_) => {
                            self.error = None;
                            self.status = Some("Switched project.".into());
                            self.project_view.refresh(&mut engine);
                            self.storyboard_view.reload(&engine);
                        }
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
                ProjectViewAction::Status(msg) => {
                    self.error = None;
                    self.status = Some(msg);
                }
                ProjectViewAction::Error(e) => self.error = Some(e),
            },
            AppView::StoryboardView => match self.storyboard_view.ui(ui, &mut engine) {
                StoryboardAction::None => {}
                StoryboardAction::Status(msg) => {
                    self.error = None;
                    self.status = Some(msg);
                }
                StoryboardAction::Error(e) => self.error = Some(e),
            },
            AppView::AnimationView => match self.animation_view.ui(ui, &mut engine) {
                AnimationAction::None => {}
                AnimationAction::Status(msg) => {
                    self.error = None;
                    self.status = Some(msg);
                }
                AnimationAction::Error(e) => self.error = Some(e),
            },
            AppView::LyricsView => match self.lyrics_view.ui(ui, &mut engine) {
                LyricsAction::None => {}
                LyricsAction::Status(msg) => {
                    self.error = None;
                    self.status = Some(msg);
                }
                LyricsAction::Error(e) => self.error = Some(e),
            },
        });
    }
}

#[cfg(unix)]
fn start_socket_listener(
    engine: Arc<Mutex<Session>>,
    shared_ui: Arc<Mutex<SharedUiState>>,
) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;

    let runtime = magic_engine::studio_sessions::runtime_dir()?;
    let dir = runtime.join("magic");
    std::fs::create_dir_all(&dir).ok()?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).ok()?;

    let pid = std::process::id();
    let sock_path = dir.join(format!("{pid}-1.sock"));
    let _ = std::fs::remove_file(&sock_path);
    let listener = UnixListener::bind(&sock_path).ok()?;
    std::fs::set_permissions(&sock_path, std::fs::Permissions::from_mode(0o600)).ok()?;

    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            if magic_engine::local_ipc::peer(&stream).is_err() {
                continue;
            }
            let engine = Arc::clone(&engine);
            let shared_ui = Arc::clone(&shared_ui);
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                if reader.read_line(&mut line).is_ok() && !line.trim().is_empty() {
                    let reply = handle_socket_request(&line, &engine, &shared_ui);
                    let _ = writeln!(reader.get_mut(), "{reply}");
                }
            });
        }
    });

    Some(sock_path)
}

#[cfg(unix)]
fn handle_socket_request(
    line: &str,
    engine: &Arc<Mutex<Session>>,
    shared_ui: &Arc<Mutex<SharedUiState>>,
) -> Value {
    let Ok(req) = serde_json::from_str::<Value>(line) else {
        return json!({ "ok": false, "error": "Invalid JSON request" });
    };
    let Some(id) = req.get("id").and_then(Value::as_str) else {
        return json!({ "ok": false, "error": "Missing command id" });
    };
    let params = req.get("params").cloned().unwrap_or_else(|| json!({}));

    let Ok(mut s) = engine.lock() else {
        return json!({ "ok": false, "error": "Studio session lock poisoned" });
    };

    let active_project = s
        .project_path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned());
    let writer_owner = s.project_owner.is_some();
    let view_name = shared_ui
        .lock()
        .map(|u| u.view.as_str())
        .unwrap_or("project");

    let identity = json!({
        "application": "Magic Studio",
        "frontend": "egui",
        "pid": std::process::id(),
        "executable": std::env::current_exe().ok(),
        "version": env!("CARGO_PKG_VERSION"),
        "build_id": env!("magic_BUILD_ID"),
        "branch": env!("magic_BUILD_BRANCH"),
    });

    match id {
        "ui.feedback" | "ui.state" => json!({
            "ok": true,
            "result": {
                "ok": true,
                "ui_ready": true,
                "phase": "ready",
                "rendered": true,
                "errors": [],
                "view": view_name,
                "active_project": active_project,
                "writer_owner": writer_owner,
                "identity": identity,
            }
        }),
        "ui.focus" => {
            if let Ok(u) = shared_ui.lock()
                && let Some(ctx) = &u.ctx
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                ctx.request_repaint();
            }
            json!({
                "ok": true,
                "result": {
                    "ok": true,
                    "focused": true,
                    "active_project": active_project,
                }
            })
        }
        _ => match s.execute(id, params) {
            Ok(res) => {
                if magic_engine::find_command(id).is_some_and(|c| c.journal)
                    && id != "project.save"
                    && s.project_path.is_some()
                {
                    let _ = s.execute("project.save", json!({}));
                }
                if let Ok(mut u) = shared_ui.lock() {
                    if s.project_path.is_some() && u.view == AppView::ProjectList {
                        u.view = AppView::ProjectView;
                    } else if s.project_path.is_none() {
                        u.view = AppView::ProjectList;
                    }
                    u.external_revision.fetch_add(1, Ordering::Relaxed);
                    if let Some(ctx) = &u.ctx {
                        ctx.request_repaint();
                    }
                }
                json!({ "ok": true, "result": res })
            }
            Err(e) => json!({ "ok": false, "error": e.to_string() }),
        },
    }
}
