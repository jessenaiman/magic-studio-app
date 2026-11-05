//! Magic Studio egui frontend (`magic-ui-egui`).
//!
//! Shared across the native desktop binary (`apps/magic-gui`) and the browser WASM shell
//! (`apps/magic-web`). All domain state and persistence live in [`magic::Session`].

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unimplemented,
        clippy::todo,
        clippy::unreachable
    )
)]

pub mod assistant;
pub mod theme;
pub mod widgets;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

use egui::{ColorImage, Context, CornerRadius, RichText, ScrollArea, Sense, Stroke, TextureHandle, Ui};
use magic::Session;
use serde_json::{Value, json};

use crate::assistant::{AssistantDrawer, AssistantTab};
use crate::theme::{
    ACCENT, ACCENT_AMBER, ACCENT_GREEN, ACCENT_RED, ACCENT_TEAL, ACCENT_VIOLET, BG_DEEP,
    BG_ELEVATED, BG_PANEL, BORDER_STRONG, BORDER_SUBTLE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};
use crate::widgets::{
    card_frame, draw_spark_icon, empty_state, frame_outline, hot_number, pill_toggle,
    primary_button, prop_row, secondary_button, section_header, status_badge,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    Projects,
    Storyboard,
    Lyrics,
    Animation,
    Studio,
}

impl ViewMode {
    pub fn id(self) -> &'static str {
        match self {
            Self::Projects => "projects",
            Self::Storyboard => "storyboard",
            Self::Lyrics => "lyrics",
            Self::Animation => "animation",
            Self::Studio => "studio",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Projects => "Projects",
            Self::Storyboard => "Storyboard",
            Self::Lyrics => "Lyrics & Words",
            Self::Animation => "Animation",
            Self::Studio => "Studio",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "projects" | "home" => Some(Self::Projects),
            "storyboard" | "project" => Some(Self::Storyboard),
            "lyrics" | "words" | "design" => Some(Self::Lyrics),
            "animation" | "character" => Some(Self::Animation),
            "studio" | "film" | "video" => Some(Self::Studio),
            _ => None,
        }
    }
}

/// Request arriving over the loopback `--control` channel (`magic --control <port>`).
pub struct ControlRequest {
    pub method: String,
    pub params: Value,
    pub reply: Sender<Result<Value, String>>,
}

struct WorkerResult {
    id: String,
    outcome: Result<Value, String>,
}

pub struct MagicApp {
    pub session: Session,
    pub view: ViewMode,
    pub assistant: AssistantDrawer,
    pub status_line: String,
    pub last_error: Option<String>,
    theme_applied: bool,

    // Projects / Home state
    pub catalog_items: Vec<Value>,
    pub new_project_path: String,
    pub new_project_mode: String,
    pub new_project_audio: String,

    // Storyboard state
    pub storyboard_pages: Vec<Value>,
    pub storyboard_scenes: Vec<Value>,
    pub selected_page: usize,
    pub slot_fields: BTreeMap<String, String>,
    pub slot_images: BTreeMap<String, String>,
    pub preview_image_path: Option<PathBuf>,
    pub unfilled_checks: Vec<Value>,

    // Lyrics & Words state
    pub song_title: String,
    pub song_album: String,
    pub song_lyrics_text: String,
    pub song_notes: String,
    pub song_audio_path: String,
    pub lyrics_sections: Vec<Value>,

    // Animation state
    pub characters: Vec<Value>,
    pub selected_character: String,
    pub selected_view: String,
    pub anim_time: f32,
    pub anim_action: String,
    pub character_frame_path: Option<PathBuf>,

    // Studio / Film state
    pub film_state: Value,
    pub film_timeline: Value,
    pub studio_playhead: f32,
    pub studio_frame_path: Option<PathBuf>,

    // Texture cache (`path` -> `(mtime_len, TextureHandle)`)
    textures: HashMap<PathBuf, (u64, TextureHandle)>,

    // Background job runner
    worker_tx: Sender<WorkerResult>,
    worker_rx: Receiver<WorkerResult>,
    pub busy_command: Option<String>,

    // Optional loopback `--control` receiver
    control_rx: Option<Receiver<ControlRequest>>,
    quit_requested: bool,
}

impl MagicApp {
    pub fn new(mut session: Session) -> Self {
        let (worker_tx, worker_rx) = channel();
        let initial_view = if session.project_path.is_some() {
            ViewMode::Storyboard
        } else {
            ViewMode::Projects
        };
        let mut assistant = AssistantDrawer::default();
        assistant.refresh_from_session(&mut session);
        let mut app = Self {
            session,
            view: initial_view,
            assistant,
            status_line: "Ready".into(),
            last_error: None,
            theme_applied: false,
            catalog_items: Vec::new(),
            new_project_path: String::new(),
            new_project_mode: "project".into(),
            new_project_audio: String::new(),
            storyboard_pages: Vec::new(),
            storyboard_scenes: Vec::new(),
            selected_page: 0,
            slot_fields: BTreeMap::new(),
            slot_images: BTreeMap::new(),
            preview_image_path: None,
            unfilled_checks: Vec::new(),
            song_title: String::new(),
            song_album: String::new(),
            song_lyrics_text: String::new(),
            song_notes: String::new(),
            song_audio_path: String::new(),
            lyrics_sections: Vec::new(),
            characters: Vec::new(),
            selected_character: String::new(),
            selected_view: "front".into(),
            anim_time: 0.0,
            anim_action: "walk_right".into(),
            character_frame_path: None,
            film_state: json!({}),
            film_timeline: json!({}),
            studio_playhead: 0.0,
            studio_frame_path: None,
            textures: HashMap::new(),
            worker_tx,
            worker_rx,
            busy_command: None,
            control_rx: None,
            quit_requested: false,
        };
        app.refresh_all();
        app
    }

    pub fn with_control(mut self, rx: Receiver<ControlRequest>) -> Self {
        self.control_rx = Some(rx);
        self
    }

    pub fn refresh_all(&mut self) {
        if let Ok(cat) = self.session.execute("project.catalog", json!({})) {
            self.catalog_items = cat["projects"]
                .as_array()
                .cloned()
                .or_else(|| cat["items"].as_array().cloned())
                .unwrap_or_default();
        }
        self.assistant.refresh_from_session(&mut self.session);
        if self.session.project_path.is_some() {
            self.refresh_storyboard();
            self.refresh_lyrics();
            self.refresh_animation();
            self.refresh_studio();
        }
    }

    pub fn refresh_storyboard(&mut self) {
        if self.session.project_path.is_none() {
            return;
        }
        if let Ok(pages) = self.session.execute("storyboard.pages", json!({})) {
            self.storyboard_pages = pages["pages"].as_array().cloned().unwrap_or_default();
            if self.selected_page >= self.storyboard_pages.len() {
                self.selected_page = 0;
            }
            self.load_selected_page_slots();
            self.render_selected_page_preview();
        }
        if let Ok(scenes) = self.session.execute("storyboard.scenes", json!({})) {
            self.storyboard_scenes = scenes["scenes"].as_array().cloned().unwrap_or_default();
            if let Some(proj) = self.session.project_path.as_ref()
                && let Some(parent) = proj.parent()
            {
                let out_dir = parent.join("cache/storyboard");
                let _ = self.session.execute(
                    "storyboard.thumbnail",
                    json!({ "out_dir": out_dir, "scale": 0.25 }),
                );
            }
        }
        if let Ok(chk) = self.session.execute("storyboard.check", json!({})) {
            self.unfilled_checks = chk["unfilled"].as_array().cloned().unwrap_or_default();
        }
    }

    fn load_selected_page_slots(&mut self) {
        self.slot_fields.clear();
        self.slot_images.clear();
        let Some(page) = self.storyboard_pages.get(self.selected_page) else {
            return;
        };
        if let Some(slots) = page["slots"].as_array() {
            for slot in slots {
                let Some(label) = slot["label"].as_str() else {
                    continue;
                };
                let kind = slot["kind"].as_str().unwrap_or("text");
                if kind == "text" {
                    let text = slot["text"].as_str().unwrap_or_default().to_owned();
                    self.slot_fields.insert(label.to_owned(), text);
                } else if kind == "image" {
                    let img = slot["image"].as_str().unwrap_or_default().to_owned();
                    self.slot_images.insert(label.to_owned(), img);
                }
            }
        }
    }

    fn render_selected_page_preview(&mut self) {
        let Some(proj) = self.session.project_path.as_ref() else {
            return;
        };
        let Some(parent) = proj.parent() else {
            return;
        };
        let out = parent
            .join("cache/storyboard")
            .join(format!("page-{}.png", self.selected_page));
        if self
            .session
            .execute(
                "storyboard.thumbnail",
                json!({
                    "page": self.selected_page,
                    "out": out,
                    "scale": 0.65
                }),
            )
            .is_ok()
        {
            self.preview_image_path = Some(out);
        }
    }

    pub fn refresh_lyrics(&mut self) {
        if let Ok(lyr) = self.session.execute("song.lyrics", json!({})) {
            self.song_title = lyr["title"].as_str().unwrap_or_default().to_owned();
            self.song_album = lyr["album"].as_str().unwrap_or_default().to_owned();
            self.song_lyrics_text = lyr["lyrics"].as_str().unwrap_or_default().to_owned();
            self.song_notes = lyr["notes"].as_str().unwrap_or_default().to_owned();
            self.lyrics_sections = lyr["sections"].as_array().cloned().unwrap_or_default();
        }
        if let Ok(ws) = self.session.execute("workshop.status", json!({}))
            && let Some(audio) = ws["audio"]["path"].as_str()
        {
            self.song_audio_path = audio.to_owned();
        }
    }

    pub fn refresh_animation(&mut self) {
        if let Ok(chars) = self.session.execute("character.list", json!({})) {
            self.characters = chars["characters"].as_array().cloned().unwrap_or_default();
            if self.selected_character.is_empty()
                && let Some(first) = self.characters.first()
                && let Some(id) = first["id"].as_str()
            {
                self.selected_character = id.to_owned();
            }
        }
    }

    pub fn refresh_studio(&mut self) {
        if let Ok(film) = self.session.execute("video.film", json!({})) {
            self.film_state = film;
        }
        if let Ok(tl) = self.session.execute("video.timeline", json!({})) {
            self.film_timeline = tl;
        }
    }

    /// Execute an engine command, running long-running commands on a background thread when
    /// `project_path` is open so the UI remains responsive.
    pub fn run_command(&mut self, ctx: &Context, id: &str, params: Value) {
        let is_long = magic::find_command(id).is_some_and(|c| c.long_running);
        if is_long && self.busy_command.is_none() {
            let mut bg_session = Session::default();
            bg_session.project_path = self.session.project_path.clone();
            bg_session.project_assets = self.session.project_assets.clone();
            let tx = self.worker_tx.clone();
            let ctx_clone = ctx.clone();
            let id_owned = id.to_owned();
            self.busy_command = Some(id_owned.clone());
            self.status_line = format!("Running {id}…");
            std::thread::spawn(move || {
                let outcome = bg_session
                    .execute(&id_owned, params)
                    .map_err(|e| e.to_string());
                let _ = tx.send(WorkerResult {
                    id: id_owned,
                    outcome,
                });
                ctx_clone.request_repaint();
            });
            return;
        }
        match self.session.execute(id, params) {
            Ok(val) => self.handle_command_success(id, val),
            Err(err) => {
                let msg = err.to_string();
                self.last_error = Some(msg.clone());
                self.status_line = format!("{id} failed: {msg}");
            }
        }
    }

    fn handle_command_success(&mut self, id: &str, val: Value) {
        self.last_error = None;
        self.status_line = format!("{id}: ok");
        match id {
            "project.open" | "project.new" => {
                self.view = ViewMode::Storyboard;
                self.refresh_all();
            }
            "project.close" => {
                self.view = ViewMode::Projects;
                self.refresh_all();
            }
            "fidget.prepare" => {
                if let Some(fidget) = val.get("fidget") {
                    self.assistant.prepared_markdown = fidget["markdown"]
                        .as_str()
                        .or_else(|| val["markdown"].as_str())
                        .unwrap_or_default()
                        .to_owned();
                    self.assistant.prepared_fidget = Some(fidget.clone());
                }
            }
            "sprocket.send" => {
                if let Some(md) = val["markdown"].as_str().or_else(|| val["text"].as_str()) {
                    self.assistant.prepared_markdown = md.to_owned();
                }
            }
            "scribbles.generate"
            | "scribbles.new"
            | "scribbles.image"
            | "scribbles.sheet"
            | "scribbles.poses"
            | "scribbles.actions"
            | "scribbles.custom" => {
                self.assistant.last_receipt = Some(val);
                if let Some(path) = self.session.project_path.clone() {
                    let _ = self.session.execute("project.open", json!({ "path": path }));
                }
                self.refresh_all();
            }
            cmd if cmd.starts_with("storyboard.") => {
                self.refresh_storyboard();
            }
            cmd if cmd.starts_with("song.") || cmd.starts_with("words.") || cmd.starts_with("workshop.") => {
                self.refresh_lyrics();
                self.refresh_storyboard();
            }
            cmd if cmd.starts_with("character.") || cmd.starts_with("animation.") => {
                if let Some(frame_path) = val["path"]
                    .as_str()
                    .or_else(|| val["image"].as_str())
                    .or_else(|| val["frame"].as_str())
                {
                    self.character_frame_path = Some(PathBuf::from(frame_path));
                }
                self.refresh_animation();
            }
            cmd if cmd.starts_with("video.") => {
                if let Some(frame_path) = val["path"]
                    .as_str()
                    .or_else(|| val["image"].as_str())
                    .or_else(|| val["frame"].as_str())
                {
                    self.studio_frame_path = Some(PathBuf::from(frame_path));
                }
                self.refresh_studio();
            }
            _ => {
                self.refresh_all();
            }
        }
    }

    fn poll_worker(&mut self) {
        while let Ok(done) = self.worker_rx.try_recv() {
            self.busy_command = None;
            match done.outcome {
                Ok(val) => self.handle_command_success(&done.id, val),
                Err(err) => {
                    self.last_error = Some(err.clone());
                    self.status_line = format!("{} failed: {err}", done.id);
                }
            }
        }
    }

    fn poll_control(&mut self, ctx: &Context) {
        let mut pending = Vec::new();
        if let Some(rx) = &self.control_rx {
            while let Ok(req) = rx.try_recv() {
                pending.push(req);
            }
        }
        for req in pending {
            let reply = self.handle_control_request(ctx, &req.method, req.params);
            let _ = req.reply.send(reply);
            ctx.request_repaint();
        }
    }

    fn handle_control_request(
        &mut self,
        ctx: &Context,
        method: &str,
        params: Value,
    ) -> Result<Value, String> {
        match method {
            "ui.state" => Ok(json!({
                "ok": true,
                "view": self.view.id(),
                "project": self.session.project_path,
                "assistant_open": self.assistant.open,
                "assistant_tab": format!("{:?}", self.assistant.active_tab),
                "selected_page": self.selected_page,
                "selected_character": self.selected_character,
                "busy": self.busy_command,
                "status": self.status_line,
                "last_error": self.last_error,
            })),
            "ui.elements" | "ui.components" | "ui.controls" => Ok(json!({
                "ok": true,
                "view": self.view.id(),
                "elements": [
                    { "id": "nav-projects", "label": "Projects" },
                    { "id": "nav-storyboard", "label": "Storyboard" },
                    { "id": "nav-lyrics", "label": "Lyrics & Words" },
                    { "id": "nav-animation", "label": "Animation" },
                    { "id": "nav-studio", "label": "Studio" },
                    { "id": "drawer-fidget", "label": "Fidget" },
                    { "id": "drawer-scribbles", "label": "Scribbles" }
                ]
            })),
            "ui.click" => {
                let target = params["id"].as_str().unwrap_or_default();
                match target {
                    "nav-projects" | "projects" => self.view = ViewMode::Projects,
                    "nav-storyboard" | "storyboard" => self.view = ViewMode::Storyboard,
                    "nav-lyrics" | "lyrics" => self.view = ViewMode::Lyrics,
                    "nav-animation" | "animation" => self.view = ViewMode::Animation,
                    "nav-studio" | "studio" => self.view = ViewMode::Studio,
                    "drawer-fidget" | "fidget" => {
                        self.assistant.open = true;
                        self.assistant.active_tab = AssistantTab::Fidget;
                    }
                    "drawer-scribbles" | "scribbles" => {
                        self.assistant.open = true;
                        self.assistant.active_tab = AssistantTab::Scribbles;
                    }
                    _ => {
                        return Err(format!("Unknown UI element `{target}`"));
                    }
                }
                Ok(json!({ "ok": true, "view": self.view.id(), "assistant_open": self.assistant.open }))
            }
            "ui.set" => {
                let id = params["id"].as_str().unwrap_or_default();
                let val = params["value"].as_str().unwrap_or_default();
                match id {
                    "view" => {
                        if let Some(v) = ViewMode::from_name(val) {
                            self.view = v;
                        }
                    }
                    "template" => self.assistant.selected_template = val.to_owned(),
                    "prompt" => self.assistant.prompt_input = val.to_owned(),
                    _ => {}
                }
                Ok(json!({ "ok": true }))
            }
            "app.quit" => {
                self.quit_requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                Ok(json!({ "ok": true, "quitting": true }))
            }
            other => {
                let res = self.session.execute(other, params).map_err(|e| e.to_string())?;
                self.handle_command_success(other, res.clone());
                Ok(res)
            }
        }
    }

    fn load_texture(&mut self, ctx: &Context, path: &Path) -> Option<TextureHandle> {
        let meta = std::fs::metadata(path).ok()?;
        let stamp = meta.len()
            ^ meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
        if let Some((cached_stamp, tex)) = self.textures.get(path)
            && *cached_stamp == stamp
        {
            return Some(tex.clone());
        }
        let bytes = std::fs::read(path).ok()?;
        let decoded = image::load_from_memory(&bytes).ok()?.to_rgba8();
        let size = [decoded.width() as usize, decoded.height() as usize];
        let color_image = ColorImage::from_rgba_unmultiplied(size, decoded.as_flat_samples().as_slice());
        let handle = ctx.load_texture(
            path.to_string_lossy(),
            color_image,
            egui::TextureOptions::LINEAR,
        );
        self.textures
            .insert(path.to_path_buf(), (stamp, handle.clone()));
        Some(handle)
    }

    fn show_top_bar(&mut self, ctx: &Context, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            // Brand cluster
            let (icon_rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), Sense::hover());
            draw_spark_icon(ui.painter(), icon_rect, ACCENT_TEAL);
            ui.label(
                RichText::new("Magic Studio")
                    .size(13.5)
                    .strong()
                    .color(TEXT_PRIMARY),
            );

            if let Some(proj) = &self.session.project_path {
                let name = proj
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("project");
                status_badge(ui, name, ACCENT_TEAL);
            } else {
                status_badge(ui, "No project open", TEXT_MUTED);
            }

            ui.separator();

            // Mode switcher tabs
            for mode in [
                ViewMode::Projects,
                ViewMode::Storyboard,
                ViewMode::Lyrics,
                ViewMode::Animation,
                ViewMode::Studio,
            ] {
                if pill_toggle(ui, self.view == mode, mode.label(), ACCENT).clicked() {
                    self.view = mode;
                }
            }

            // Right-aligned Fidget & Scribbles drawer buttons + Save
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let scribbles_active =
                    self.assistant.open && self.assistant.active_tab == AssistantTab::Scribbles;
                if pill_toggle(ui, scribbles_active, "✨ Scribbles", ACCENT_TEAL)
                    .on_hover_text("Open Scribbles template-gated image generator")
                    .clicked()
                {
                    if scribbles_active {
                        self.assistant.open = false;
                    } else {
                        self.assistant.open = true;
                        self.assistant.active_tab = AssistantTab::Scribbles;
                    }
                }

                let fidget_active =
                    self.assistant.open && self.assistant.active_tab == AssistantTab::Fidget;
                if pill_toggle(ui, fidget_active, "⚙ Fidget", ACCENT_VIOLET)
                    .on_hover_text("Open Fidget Markdown frontmatter prompt architect")
                    .clicked()
                {
                    if fidget_active {
                        self.assistant.open = false;
                    } else {
                        self.assistant.open = true;
                        self.assistant.active_tab = AssistantTab::Fidget;
                    }
                }

                if self.session.project_path.is_some()
                    && secondary_button(ui, "Save Project").clicked()
                {
                    self.run_command(ctx, "project.save", json!({}));
                }
            });
        });
    }

    fn show_projects_view(&mut self, ctx: &Context, ui: &mut Ui) {
        ScrollArea::vertical()
            .id_salt("projects_view_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.heading(RichText::new("Projects").size(18.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        #[cfg(not(target_arch = "wasm32"))]
                        if primary_button(ui, "Open .magic Project…").clicked()
                            && let Some(file) = rfd::FileDialog::new()
                                .add_filter("Magic Studio Project", &["magic"])
                                .pick_file()
                        {
                            self.run_command(
                                ctx,
                                "project.open",
                                json!({ "path": file.display().to_string() }),
                            );
                        }
                        if secondary_button(ui, "Refresh Catalog").clicked() {
                            self.refresh_all();
                        }
                    });
                });
                ui.add_space(8.0);

                // Create New Project Card
                if section_header(ui, "Create New Project", true) {
                    ui.add_space(4.0);
                    card_frame().show(ui, |ui| {
                        prop_row(ui, "Project File", |ui| {
                            #[cfg(not(target_arch = "wasm32"))]
                            if secondary_button(ui, "Save As…").clicked()
                                && let Some(file) = rfd::FileDialog::new()
                                    .set_file_name("song.magic")
                                    .add_filter("Magic Project", &["magic"])
                                    .save_file()
                            {
                                self.new_project_path = file.display().to_string();
                            }
                            ui.add(
                                egui::TextEdit::singleline(&mut self.new_project_path)
                                    .desired_width(340.0)
                                    .hint_text("/path/to/my-song/my-song.magic"),
                            );
                        });
                        prop_row(ui, "Mode", |ui| {
                            egui::ComboBox::from_id_salt("new_project_mode_combo")
                                .selected_text(&self.new_project_mode)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.new_project_mode,
                                        "project".into(),
                                        "project (full song & storyboard)",
                                    );
                                    ui.selectable_value(
                                        &mut self.new_project_mode,
                                        "animation".into(),
                                        "animation (character study)",
                                    );
                                });
                        });
                        prop_row(ui, "Audio (opt)", |ui| {
                            #[cfg(not(target_arch = "wasm32"))]
                            if secondary_button(ui, "Browse…").clicked()
                                && let Some(file) = rfd::FileDialog::new()
                                    .add_filter("Audio", &["mp3", "wav", "flac", "ogg", "m4a"])
                                    .pick_file()
                            {
                                self.new_project_audio = file.display().to_string();
                            }
                            ui.add(
                                egui::TextEdit::singleline(&mut self.new_project_audio)
                                    .desired_width(340.0)
                                    .hint_text("optional song audio path"),
                            );
                        });
                        ui.add_space(4.0);
                        if primary_button(ui, "Create & Open Project").clicked()
                            && !self.new_project_path.trim().is_empty()
                        {
                            let mut p = json!({
                                "path": self.new_project_path.trim(),
                                "mode": self.new_project_mode,
                            });
                            if !self.new_project_audio.trim().is_empty() {
                                p["audio"] = json!(self.new_project_audio.trim());
                            }
                            self.run_command(ctx, "project.new", p);
                        }
                    });
                }

                ui.add_space(10.0);
                if section_header(ui, "Recent Projects (`project.catalog`)", true) {
                    ui.add_space(4.0);
                    if self.catalog_items.is_empty() {
                        empty_state(
                            ui,
                            "No recent Magic Studio projects",
                            "Create a new .magic project above or click 'Open .magic Project…'.",
                        );
                    } else {
                        let items = self.catalog_items.clone();
                        for item in items {
                            let path = item["path"].as_str().unwrap_or_default().to_owned();
                            let name = item["name"]
                                .as_str()
                                .or_else(|| item["title"].as_str())
                                .unwrap_or(&path)
                                .to_owned();
                            let mode = item["mode"].as_str().unwrap_or("project");
                            card_frame().show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.label(
                                            RichText::new(&name)
                                                .size(13.5)
                                                .strong()
                                                .color(TEXT_PRIMARY),
                                        );
                                        ui.label(
                                            RichText::new(&path).size(11.0).color(TEXT_MUTED),
                                        );
                                    });
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if primary_button(ui, "Open").clicked() {
                                                self.run_command(
                                                    ctx,
                                                    "project.open",
                                                    json!({ "path": path }),
                                                );
                                            }
                                            status_badge(ui, mode, ACCENT);
                                        },
                                    );
                                });
                            });
                            ui.add_space(4.0);
                        }
                    }
                }
            });
    }

    fn show_storyboard_view(&mut self, ctx: &Context, ui: &mut Ui) {
        if self.session.project_path.is_none() {
            empty_state(
                ui,
                "Open a Project First",
                "Switch to the Projects tab to open or create a .magic project.",
            );
            return;
        }

        // Left Rail: Storyboard Pages & Scenes
        egui::Panel::left("storyboard_left_rail")
            .resizable(true)
            .default_size(250.0)
            .min_size(200.0)
            .frame(
                egui::Frame::new()
                    .fill(BG_PANEL)
                    .inner_margin(egui::Margin::same(8))
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("PAGES & SCENES")
                            .size(11.0)
                            .strong()
                            .color(TEXT_SECONDARY),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if secondary_button(ui, "+ Scene")
                            .on_hover_text("Add a new scene page from the template (`storyboard.add`)")
                            .clicked()
                        {
                            self.run_command(
                                ctx,
                                "storyboard.add",
                                json!({ "archetype": "scene" }),
                            );
                        }
                        if self.storyboard_pages.is_empty()
                            && primary_button(ui, "New Board").clicked()
                        {
                            self.run_command(ctx, "storyboard.new", json!({}));
                        }
                    });
                });
                ui.add_space(6.0);

                ScrollArea::vertical()
                    .id_salt("sb_pages_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let pages = self.storyboard_pages.clone();
                        for (idx, p) in pages.iter().enumerate() {
                            let archetype = p["archetype"].as_str().unwrap_or("page");
                            let name = p["name"].as_str().unwrap_or(archetype);
                            let scene_num = p["scene"].as_u64();
                            let selected = self.selected_page == idx;
                            let border = if selected { ACCENT_TEAL } else { BORDER_SUBTLE };
                            let fill = if selected {
                                ACCENT_TEAL.gamma_multiply(0.16)
                            } else {
                                BG_ELEVATED
                            };
                            let resp = egui::Frame::new()
                                .fill(fill)
                                .stroke(Stroke::new(1.0, border))
                                .corner_radius(CornerRadius::same(6))
                                .inner_margin(egui::Margin::symmetric(8, 6))
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        let badge = if let Some(sc) = scene_num {
                                            format!("Scene {sc}")
                                        } else {
                                            format!("P{}", idx + 1)
                                        };
                                        status_badge(
                                            ui,
                                            &badge,
                                            if scene_num.is_some() {
                                                ACCENT_TEAL
                                            } else {
                                                ACCENT
                                            },
                                        );
                                        ui.label(
                                            RichText::new(name)
                                                .size(12.0)
                                                .strong()
                                                .color(TEXT_PRIMARY),
                                        );
                                    });
                                    // Show scene thumbnail if rendered in cache/storyboard
                                    if let Some(sc) = scene_num
                                        && let Some(proj) = self.session.project_path.as_ref()
                                        && let Some(parent) = proj.parent()
                                    {
                                        let thumb = parent
                                            .join("cache/storyboard")
                                            .join(format!("scene-{sc}.png"));
                                        if let Some(tex) = self.load_texture(ctx, &thumb) {
                                            ui.add_space(4.0);
                                            let w = ui.available_width();
                                            let h = w * (9.0 / 16.0);
                                            ui.image((tex.id(), egui::vec2(w, h)));
                                        }
                                    }
                                })
                                .response
                                .interact(Sense::click());
                            if resp.clicked() {
                                self.selected_page = idx;
                                self.load_selected_page_slots();
                                self.render_selected_page_preview();
                            }
                            ui.add_space(4.0);
                        }
                    });
            });

        // Right Rail: Slot Inspector for selected page
        egui::Panel::right("storyboard_right_inspector")
            .resizable(true)
            .default_size(310.0)
            .min_size(250.0)
            .frame(
                egui::Frame::new()
                    .fill(BG_PANEL)
                    .inner_margin(egui::Margin::same(8))
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE)),
            )
            .show(ui, |ui| {
                let page_meta = self.storyboard_pages.get(self.selected_page).cloned();
                let current_scene = page_meta
                    .as_ref()
                    .and_then(|p| p["scene"].as_u64())
                    .map(|n| n as usize);

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("PAGE SLOTS (`storyboard.set`)")
                            .size(11.0)
                            .strong()
                            .color(TEXT_SECONDARY),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary_button(ui, "Apply Edits").clicked() {
                            let mut fields = serde_json::Map::new();
                            for (k, v) in &self.slot_fields {
                                fields.insert(k.clone(), json!(v));
                            }
                            let mut images = serde_json::Map::new();
                            for (k, v) in &self.slot_images {
                                if !v.trim().is_empty() {
                                    images.insert(k.clone(), json!(v.trim()));
                                }
                            }
                            self.run_command(
                                ctx,
                                "storyboard.set",
                                json!({
                                    "page": self.selected_page,
                                    "fields": fields,
                                    "images": images,
                                }),
                            );
                        }
                    });
                });
                ui.add_space(6.0);

                ScrollArea::vertical()
                    .id_salt("sb_slots_inspector_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if section_header(ui, "Text Slots", true) {
                            ui.add_space(4.0);
                            let keys: Vec<String> = self.slot_fields.keys().cloned().collect();
                            for k in keys {
                                if let Some(val) = self.slot_fields.get_mut(&k) {
                                    ui.label(
                                        RichText::new(&k).size(11.0).color(TEXT_SECONDARY),
                                    );
                                    if k.contains("body") || k.contains("notes") || k == "cue" {
                                        ui.add(
                                            egui::TextEdit::multiline(val)
                                                .desired_rows(2)
                                                .desired_width(f32::INFINITY),
                                        );
                                    } else {
                                        ui.add(
                                            egui::TextEdit::singleline(val)
                                                .desired_width(f32::INFINITY),
                                        );
                                    }
                                    ui.add_space(3.0);
                                }
                            }
                        }

                        ui.add_space(6.0);
                        if section_header(ui, "Image Slots & Scribbles Handoff", true) {
                            ui.add_space(4.0);
                            let img_keys: Vec<String> = self.slot_images.keys().cloned().collect();
                            let cue_hint = self
                                .slot_fields
                                .get("cue")
                                .cloned()
                                .unwrap_or_default();
                            for k in img_keys {
                                card_frame().show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(&k)
                                                .size(11.5)
                                                .strong()
                                                .color(TEXT_PRIMARY),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if secondary_button(ui, "✨ Draw in Scribbles")
                                                    .on_hover_text(
                                                        "Open Scribbles with a template that saves directly to this slot",
                                                    )
                                                    .clicked()
                                                {
                                                    self.assistant.open_for_storyboard_slot(
                                                        current_scene.unwrap_or(1),
                                                        &k,
                                                        &cue_hint,
                                                    );
                                                }
                                                #[cfg(not(target_arch = "wasm32"))]
                                                if ui.small_button("Browse…").clicked()
                                                    && let Some(file) = rfd::FileDialog::new()
                                                        .add_filter("Image", &["png", "jpg", "jpeg"])
                                                        .pick_file()
                                                    && let Some(slot_val) =
                                                        self.slot_images.get_mut(&k)
                                                {
                                                    *slot_val = file.display().to_string();
                                                }
                                            },
                                        );
                                    });
                                    if let Some(slot_val) = self.slot_images.get_mut(&k) {
                                        ui.add(
                                            egui::TextEdit::singleline(slot_val)
                                                .desired_width(f32::INFINITY)
                                                .hint_text("/path/to/image.png"),
                                        );
                                    }
                                });
                                ui.add_space(4.0);
                            }
                        }

                        if !self.unfilled_checks.is_empty() {
                            ui.add_space(6.0);
                            if section_header(
                                ui,
                                &format!("Template Defaults ({})", self.unfilled_checks.len()),
                                false,
                            ) {
                                for item in self.unfilled_checks.iter().take(12) {
                                    let slot = item["slot"].as_str().unwrap_or_default();
                                    let page = item["page"].as_u64().unwrap_or(0);
                                    ui.label(
                                        RichText::new(format!("Page {page} · {slot}"))
                                            .size(10.5)
                                            .color(ACCENT_AMBER),
                                    );
                                }
                            }
                        }
                    });
            });

        // Center Spread Canvas
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG_DEEP)
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "Spread Preview — Page {}",
                            self.selected_page + 1
                        ))
                        .size(13.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if secondary_button(ui, "Export PDF").clicked()
                            && let Some(proj) = self.session.project_path.as_ref()
                            && let Some(parent) = proj.parent()
                        {
                            let pdf = parent.join("storyboard.pdf");
                            self.run_command(
                                ctx,
                                "storyboard.export",
                                json!({ "pdf": pdf }),
                            );
                        }
                        if secondary_button(ui, "Re-render Spread").clicked() {
                            self.render_selected_page_preview();
                        }
                    });
                });
                ui.add_space(8.0);

                if let Some(preview_path) = self.preview_image_path.clone()
                    && let Some(tex) = self.load_texture(ctx, &preview_path)
                {
                    let avail = ui.available_size();
                    let tex_size = tex.size_vec2();
                    let scale = (avail.x / tex_size.x)
                        .min(avail.y / tex_size.y)
                        .clamp(0.1, 1.5);
                    let draw_size = tex_size * scale;
                    ui.centered_and_justified(|ui| {
                        let resp = ui.image((tex.id(), draw_size));
                        frame_outline(ui.painter(), resp.rect, BORDER_STRONG);
                    });
                } else {
                    empty_state(
                        ui,
                        "No Storyboard Spread Rendered Yet",
                        "Click 'New Board' in the left rail to initialize storyboard.designcraft from the template.",
                    );
                }
            });
    }

    fn show_lyrics_view(&mut self, ctx: &Context, ui: &mut Ui) {
        if self.session.project_path.is_none() {
            empty_state(
                ui,
                "Open a Project First",
                "Switch to Projects to open a .magic song project.",
            );
            return;
        }
        ScrollArea::vertical()
            .id_salt("lyrics_view_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Song Audio, Lyrics & Kinetic Words");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary_button(ui, "Save Song Details (`song.save`)").clicked() {
                            self.run_command(
                                ctx,
                                "song.save",
                                json!({
                                    "title": self.song_title,
                                    "album": self.song_album,
                                    "lyrics": self.song_lyrics_text,
                                    "notes": self.song_notes,
                                }),
                            );
                        }
                    });
                });
                ui.add_space(6.0);

                if section_header(ui, "Song Audio & Timing (`workshop.set_audio` / `song.timing`)", true) {
                    card_frame().show(ui, |ui| {
                        prop_row(ui, "Audio File", |ui| {
                            if primary_button(ui, "Link Audio").clicked()
                                && !self.song_audio_path.trim().is_empty()
                            {
                                self.run_command(
                                    ctx,
                                    "workshop.set_audio",
                                    json!({ "path": self.song_audio_path.trim() }),
                                );
                            }
                            #[cfg(not(target_arch = "wasm32"))]
                            if secondary_button(ui, "Browse…").clicked()
                                && let Some(file) = rfd::FileDialog::new()
                                    .add_filter("Audio", &["mp3", "wav", "flac", "ogg", "m4a"])
                                    .pick_file()
                            {
                                self.song_audio_path = file.display().to_string();
                            }
                            ui.add(
                                egui::TextEdit::singleline(&mut self.song_audio_path)
                                    .desired_width(300.0),
                            );
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if secondary_button(ui, "Analyze Song Beats (`audio.analyze`)").clicked() {
                                self.run_command(ctx, "audio.analyze", json!({}));
                            }
                            if secondary_button(ui, "Sync Song Timing (`song.timing`)").clicked() {
                                self.run_command(ctx, "song.timing", json!({}));
                            }
                            if secondary_button(ui, "Design Sung Lines (`words.design`)").clicked() {
                                self.run_command(ctx, "words.design", json!({}));
                            }
                            if secondary_button(ui, "Letter in EffectCraft (`words.letter`)").clicked() {
                                self.run_command(ctx, "words.letter", json!({}));
                            }
                        });
                    });
                }

                ui.add_space(8.0);
                if section_header(ui, "Song Metadata & Lyrics Sheet", true) {
                    card_frame().show(ui, |ui| {
                        prop_row(ui, "Song Title", |ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.song_title)
                                    .desired_width(320.0),
                            );
                        });
                        prop_row(ui, "Album", |ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.song_album)
                                    .desired_width(320.0),
                            );
                        });
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("Canonical Artist Lyrics")
                                .size(11.5)
                                .color(TEXT_SECONDARY),
                        );
                        ui.add(
                            egui::TextEdit::multiline(&mut self.song_lyrics_text)
                                .desired_rows(12)
                                .desired_width(f32::INFINITY),
                        );
                    });
                }
            });
    }

    fn show_animation_view(&mut self, ctx: &Context, ui: &mut Ui) {
        if self.session.project_path.is_none() {
            empty_state(
                ui,
                "Open a Project First",
                "Switch to Projects to open a .magic project or character study.",
            );
            return;
        }

        // Left Rail: Characters & Project Assets
        egui::Panel::left("animation_left_rail")
            .resizable(true)
            .default_size(240.0)
            .frame(
                egui::Frame::new()
                    .fill(BG_PANEL)
                    .inner_margin(egui::Margin::same(8))
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("CHARACTERS & ASSETS")
                            .size(11.0)
                            .strong()
                            .color(TEXT_SECONDARY),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if secondary_button(ui, "+ Scribbles").clicked() {
                            self.assistant.open = true;
                            self.assistant.active_tab = AssistantTab::Scribbles;
                            self.assistant.selected_template = "new".into();
                        }
                    });
                });
                ui.add_space(6.0);

                ScrollArea::vertical()
                    .id_salt("anim_chars_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for ch in &self.characters {
                            let id = ch["id"].as_str().unwrap_or_default().to_owned();
                            let selected = self.selected_character == id;
                            if pill_toggle(ui, selected, &id, ACCENT_TEAL).clicked() {
                                self.selected_character = id;
                            }
                        }
                        ui.separator();
                        ui.label(
                            RichText::new("Connected Project Assets")
                                .size(11.0)
                                .color(TEXT_MUTED),
                        );
                        let assets = self.session.project_assets.clone();
                        for asset in assets {
                            card_frame().show(ui, |ui| {
                                ui.label(
                                    RichText::new(&asset.name)
                                        .size(11.5)
                                        .strong()
                                        .color(TEXT_PRIMARY),
                                );
                                ui.horizontal(|ui| {
                                    status_badge(ui, &asset.kind, ACCENT);
                                    if ui.small_button("Select").clicked() {
                                        self.selected_character = asset.id.clone();
                                    }
                                });
                            });
                            ui.add_space(3.0);
                        }
                    });
            });

        // Right Inspector: Pipeline & Keyframes
        egui::Panel::right("animation_right_inspector")
            .resizable(true)
            .default_size(290.0)
            .frame(
                egui::Frame::new()
                    .fill(BG_PANEL)
                    .inner_margin(egui::Margin::same(8))
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE)),
            )
            .show(ui, |ui| {
                if section_header(ui, "2D Puppet Pipeline", true) {
                    ui.add_space(4.0);
                    card_frame().show(ui, |ui| {
                        prop_row(ui, "Character", |ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.selected_character)
                                    .desired_width(140.0),
                            );
                        });
                        prop_row(ui, "View", |ui| {
                            egui::ComboBox::from_id_salt("char_view_combo")
                                .selected_text(&self.selected_view)
                                .show_ui(ui, |ui| {
                                    for v in ["front", "back", "side"] {
                                        ui.selectable_value(
                                            &mut self.selected_view,
                                            v.to_owned(),
                                            v,
                                        );
                                    }
                                });
                        });
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            if secondary_button(ui, "1. Mark (Scribbles)").clicked()
                                && !self.selected_character.is_empty()
                            {
                                self.run_command(
                                    ctx,
                                    "character.mark",
                                    json!({
                                        "character": self.selected_character,
                                        "view": self.selected_view,
                                    }),
                                );
                            }
                            if secondary_button(ui, "2. Cut (PhotoCraft)").clicked()
                                && !self.selected_character.is_empty()
                            {
                                self.run_command(
                                    ctx,
                                    "character.cut",
                                    json!({
                                        "character": self.selected_character,
                                        "view": self.selected_view,
                                    }),
                                );
                            }
                            if secondary_button(ui, "3. Rig (EffectCraft)").clicked()
                                && !self.selected_character.is_empty()
                            {
                                self.run_command(
                                    ctx,
                                    "character.rig",
                                    json!({
                                        "character": self.selected_character,
                                        "view": self.selected_view,
                                    }),
                                );
                            }
                        });
                    });
                }

                ui.add_space(6.0);
                if section_header(ui, "Key Action (`character.action`)", true) {
                    ui.add_space(4.0);
                    card_frame().show(ui, |ui| {
                        prop_row(ui, "Action", |ui| {
                            egui::ComboBox::from_id_salt("char_action_combo")
                                .selected_text(&self.anim_action)
                                .show_ui(ui, |ui| {
                                    for a in [
                                        "idle",
                                        "walk_down",
                                        "walk_up",
                                        "walk_left",
                                        "walk_right",
                                        "jump",
                                        "turn",
                                        "wave",
                                    ] {
                                        ui.selectable_value(
                                            &mut self.anim_action,
                                            a.to_owned(),
                                            a,
                                        );
                                    }
                                });
                        });
                        prop_row(ui, "Time (s)", |ui| {
                            hot_number(ui, &mut self.anim_time, 0.0..=60.0, 0.05, "s");
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if primary_button(ui, "Key Action").clicked()
                                && !self.selected_character.is_empty()
                            {
                                self.run_command(
                                    ctx,
                                    "character.action",
                                    json!({
                                        "character": self.selected_character,
                                        "view": self.selected_view,
                                        "action": self.anim_action,
                                        "at": self.anim_time,
                                        "times": 1,
                                    }),
                                );
                            }
                            if secondary_button(ui, "Render Clip").clicked()
                                && !self.selected_character.is_empty()
                            {
                                self.run_command(
                                    ctx,
                                    "character.render",
                                    json!({
                                        "character": self.selected_character,
                                        "view": self.selected_view,
                                        "format": "clip",
                                    }),
                                );
                            }
                        });
                    });
                }

                ui.add_space(6.0);
                if section_header(ui, "Open in Craft Suite App", true) {
                    ui.add_space(4.0);
                    card_frame().show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            for app in ["effectcraft", "photocraft", "blender"] {
                                if secondary_button(ui, app).clicked()
                                    && !self.selected_character.is_empty()
                                {
                                    self.run_command(
                                        ctx,
                                        "character.open",
                                        json!({
                                            "character": self.selected_character,
                                            "view": self.selected_view,
                                            "app": app,
                                        }),
                                    );
                                }
                            }
                        });
                    });
                }
            });

        // Center Stage Preview
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG_DEEP)
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("EffectCraft Puppet Stage")
                            .size(13.0)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if secondary_button(ui, "Sample Frame (`character.frame`)").clicked()
                            && !self.selected_character.is_empty()
                        {
                            self.run_command(
                                ctx,
                                "character.frame",
                                json!({
                                    "character": self.selected_character,
                                    "view": self.selected_view,
                                    "time": self.anim_time,
                                }),
                            );
                        }
                        hot_number(ui, &mut self.anim_time, 0.0..=30.0, 0.05, "s");
                    });
                });
                ui.add_space(8.0);
                if let Some(frame_path) = self.character_frame_path.clone()
                    && let Some(tex) = self.load_texture(ctx, &frame_path)
                {
                    let avail = ui.available_size();
                    let tex_size = tex.size_vec2();
                    let scale = (avail.x / tex_size.x)
                        .min(avail.y / tex_size.y)
                        .clamp(0.1, 1.5);
                    ui.centered_and_justified(|ui| {
                        let resp = ui.image((tex.id(), tex_size * scale));
                        frame_outline(ui.painter(), resp.rect, BORDER_STRONG);
                    });
                } else {
                    empty_state(
                        ui,
                        "Select a Character and Sample Frame",
                        "Use Scribbles to draw a turnaround/pose sheet or click 'Sample Frame' to preview the EffectCraft rig.",
                    );
                }
            });
    }

    fn show_studio_view(&mut self, ctx: &Context, ui: &mut Ui) {
        if self.session.project_path.is_none() {
            empty_state(
                ui,
                "Open a Project First",
                "Switch to Projects to open a .magic project.",
            );
            return;
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG_DEEP)
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Studio · FilmCraft Timeline & Assembly");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary_button(ui, "Assemble & Export MP4 (`video.assemble`)").clicked() {
                            self.run_command(ctx, "video.assemble", json!({}));
                        }
                        if secondary_button(ui, "Open in FilmCraft (`video.open`)").clicked() {
                            self.run_command(ctx, "video.open", json!({}));
                        }
                        if secondary_button(ui, "Sample Frame").clicked() {
                            self.run_command(
                                ctx,
                                "video.frame",
                                json!({ "seconds": self.studio_playhead }),
                            );
                        }
                        hot_number(ui, &mut self.studio_playhead, 0.0..=300.0, 0.1, "s");
                    });
                });
                ui.add_space(8.0);

                if let Some(frame_path) = self.studio_frame_path.clone()
                    && let Some(tex) = self.load_texture(ctx, &frame_path)
                {
                    let avail = egui::vec2(ui.available_width(), (ui.available_height() * 0.6).max(200.0));
                    let tex_size = tex.size_vec2();
                    let scale = (avail.x / tex_size.x)
                        .min(avail.y / tex_size.y)
                        .clamp(0.1, 1.2);
                    ui.vertical_centered(|ui| {
                        let resp = ui.image((tex.id(), tex_size * scale));
                        frame_outline(ui.painter(), resp.rect, BORDER_STRONG);
                    });
                    ui.add_space(8.0);
                }

                if section_header(ui, "FilmCraft Tracks & Timeline (`video.timeline`)", true) {
                    ui.add_space(4.0);
                    card_frame().show(ui, |ui| {
                        if let Some(tracks) = self.film_timeline["tracks"].as_array() {
                            for tr in tracks {
                                let name = tr["name"].as_str().unwrap_or("Track");
                                let clips = tr["clips"].as_array().map(Vec::len).unwrap_or(0);
                                prop_row(ui, name, |ui| {
                                    status_badge(ui, &format!("{clips} clips"), ACCENT_TEAL);
                                });
                            }
                        } else {
                            ui.label(
                                RichText::new(
                                    "Click 'Assemble & Export MP4' above to build the FilmCraft timeline from your storyboard scenes, character shots, and lyric lettering.",
                                )
                                .size(11.5)
                                .color(TEXT_MUTED),
                            );
                        }
                    });
                }
            });
    }
}

impl eframe::App for MagicApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let ctx = &ctx;
        if !self.theme_applied {
            theme::apply(ctx);
            self.theme_applied = true;
        }
        self.poll_worker();
        self.poll_control(ctx);

        // Top Navigation & Assistant Bar
        egui::Panel::top("magic_top_bar")
            .exact_size(40.0)
            .frame(
                egui::Frame::new()
                    .fill(BG_PANEL)
                    .inner_margin(egui::Margin::symmetric(12, 6))
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE)),
            )
            .show(ui, |ui| {
                self.show_top_bar(ctx, ui);
            });

        // Bottom Status Bar
        egui::Panel::bottom("magic_status_bar")
            .exact_size(24.0)
            .frame(
                egui::Frame::new()
                    .fill(BG_PANEL)
                    .inner_margin(egui::Margin::symmetric(12, 3))
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some(busy) = &self.busy_command {
                        ui.spinner();
                        ui.label(
                            RichText::new(format!("Running `{busy}`…"))
                                .size(11.0)
                                .color(ACCENT_TEAL),
                        );
                    } else if let Some(err) = &self.last_error {
                        ui.label(RichText::new(err).size(11.0).color(ACCENT_RED));
                    } else {
                        ui.label(
                            RichText::new(&self.status_line)
                                .size(11.0)
                                .color(TEXT_SECONDARY),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new("Fidget (text.complete) · Scribbles (image.generate)")
                                .size(10.5)
                                .color(TEXT_MUTED),
                        );
                    });
                });
            });

        // Right-hand Fidget & Scribbles Assistant Drawer
        if self.assistant.open {
            let mut queued_cmd = None;
            let assets = self.session.project_assets.clone();
            let busy = self.busy_command.is_some();
            egui::Panel::right("magic_assistant_drawer")
                .resizable(true)
                .default_size(390.0)
                .min_size(320.0)
                .frame(
                    egui::Frame::new()
                        .fill(BG_PANEL)
                        .inner_margin(egui::Margin::same(10))
                        .stroke(Stroke::new(1.0, BORDER_STRONG)),
                )
                .show(ui, |ui| {
                    queued_cmd = self.assistant.show(ui, &assets, busy);
                });
            if let Some((cmd_id, cmd_params)) = queued_cmd {
                self.run_command(ctx, &cmd_id, cmd_params);
            }
        }

        // Main Workspace View
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG_DEEP)
                    .inner_margin(egui::Margin::same(8)),
            )
            .show(ui, |ui| match self.view {
                ViewMode::Projects => self.show_projects_view(ctx, ui),
                ViewMode::Storyboard => self.show_storyboard_view(ctx, ui),
                ViewMode::Lyrics => self.show_lyrics_view(ctx, ui),
                ViewMode::Animation => self.show_animation_view(ctx, ui),
                ViewMode::Studio => self.show_studio_view(ctx, ui),
            });
    }
}
