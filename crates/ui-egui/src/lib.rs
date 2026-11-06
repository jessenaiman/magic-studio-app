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
pub mod automation;
mod control;
#[cfg(not(target_arch = "wasm32"))]
mod storyboard_workspace;
pub mod theme;
pub mod widgets;

#[cfg(not(target_arch = "wasm32"))]
mod animation_effectcraft;
#[cfg(not(target_arch = "wasm32"))]
mod studio_filmcraft;

#[cfg(test)]
mod control_conformance;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, OnceLock};

use egui::{
    ColorImage, Context, CornerRadius, RichText, ScrollArea, Sense, Stroke, TextureHandle, Ui,
};
use magic::Session;
use serde_json::{Value, json};

use crate::assistant::{AssistantDrawer, AssistantTab};
use crate::theme::{
    ACCENT, ACCENT_AMBER, ACCENT_RED, ACCENT_TEAL, ACCENT_VIOLET, BG_DEEP, BG_ELEVATED, BG_PANEL,
    BORDER_STRONG, BORDER_SUBTLE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
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
    Motion,
    Studio,
}

impl ViewMode {
    pub fn id(self) -> &'static str {
        match self {
            Self::Projects => "projects",
            Self::Storyboard => "storyboard",
            Self::Lyrics => "lyrics",
            Self::Animation => "animation",
            Self::Motion => "motion",
            Self::Studio => "studio",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Projects => "Projects",
            Self::Storyboard => "Storyboard",
            Self::Lyrics => "Lyrics",
            Self::Animation => "Animation",
            Self::Motion => "Motion",
            Self::Studio => "Studio",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "projects" | "home" => Some(Self::Projects),
            "storyboard" | "project" => Some(Self::Storyboard),
            "lyrics" | "words" | "design" => Some(Self::Lyrics),
            "animation" | "character" => Some(Self::Animation),
            "motion" => Some(Self::Motion),
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
    project: Option<PathBuf>,
    outcome: Result<Value, String>,
}

/// Pages served by the desktop's shared web workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebWorkspacePage {
    Projects,
    Storyboard,
    Lyrics,
    Motion,
    Studio,
    AnimationAssets,
    AnimationInspector,
}

/// The native webview host. egui reserves the rectangle and handles navigation.
pub trait LyricsHost {
    /// Show the designer inside `rect` (egui points); reload it when `project` changes.
    fn show(
        &mut self,
        frame: &eframe::Frame,
        ctx: &Context,
        rect: egui::Rect,
        project: &str,
        page: WebWorkspacePage,
    );
    /// Hide any webview that did not receive a layout this frame.
    fn finish_frame(&mut self);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialogAction {
    OpenProject,
    CreateProjectFolder,
    SongAudio,
    StoryboardHeroImage { scene: usize },
    AddProjectAsset,
    AssistantReferenceImage,
    OpenEffectcraftComposition,
    ImportStudioAnimation,
    WebProjectPicker,
}

pub struct MagicApp {
    pub session: Session,
    pub view: ViewMode,
    pub assistant: AssistantDrawer,
    pub auto: automation::Registry,
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
    pub words_doc: Value,

    // Animation state
    pub characters: Vec<Value>,
    animation_revision: u64,
    #[cfg(not(target_arch = "wasm32"))]
    animation_artwork_request: u64,
    pub selected_character: String,
    pub selected_view: String,
    pub anim_time: f32,
    pub anim_action: String,
    pub character_frame_path: Option<PathBuf>,
    #[cfg(not(target_arch = "wasm32"))]
    effectcraft_animation: Option<animation_effectcraft::AnimationEffectcraft>,

    // Studio / Film state
    pub film_state: Value,
    pub film_timeline: Value,
    pub studio_playhead: f32,
    pub studio_frame_path: Option<PathBuf>,
    #[cfg(not(target_arch = "wasm32"))]
    studio_film: Option<studio_filmcraft::StudioFilmcraft>,
    #[cfg(not(target_arch = "wasm32"))]
    studio_open_error: Option<(PathBuf, String)>,
    #[cfg(not(target_arch = "wasm32"))]
    studio_rendered_animation: Option<(PathBuf, PathBuf)>,
    #[cfg(not(target_arch = "wasm32"))]
    pending_studio_render: Option<PathBuf>,

    // Texture cache (`path` -> `(mtime_len, TextureHandle)`)
    textures: HashMap<PathBuf, (u64, TextureHandle)>,

    // Background job runner
    worker_tx: Sender<WorkerResult>,
    worker_rx: Receiver<WorkerResult>,
    pub busy_command: Option<String>,

    // Optional loopback `--control` receiver
    control_rx: Option<Receiver<ControlRequest>>,
    quit_requested: bool,

    synthetic: Vec<egui::Event>,
    lyrics_host: Option<Box<dyn LyricsHost>>,
    lyrics_song_details: bool,

    // Non-blocking native file dialogs (`craft-dialogs`)
    #[cfg(not(target_arch = "wasm32"))]
    dialogs: craft_dialogs::Dialogs<DialogAction>,
    #[cfg(not(target_arch = "wasm32"))]
    repaint_ctx: Arc<OnceLock<Context>>,
    #[cfg(not(target_arch = "wasm32"))]
    pending_storyboard_slot: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    web_project_picker: Option<Sender<Result<Value, String>>>,
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
        #[cfg(not(target_arch = "wasm32"))]
        let repaint_ctx: Arc<OnceLock<Context>> = Arc::new(OnceLock::new());
        #[cfg(not(target_arch = "wasm32"))]
        let dialogs = {
            let wake_ctx = Arc::clone(&repaint_ctx);
            craft_dialogs::Dialogs::new(Arc::new(move || {
                if let Some(ctx) = wake_ctx.get() {
                    ctx.request_repaint();
                }
            }))
        };
        let mut app = Self {
            session,
            view: initial_view,
            assistant,
            auto: automation::Registry::default(),
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
            words_doc: json!({}),
            characters: Vec::new(),
            animation_revision: 0,
            #[cfg(not(target_arch = "wasm32"))]
            animation_artwork_request: 0,
            selected_character: String::new(),
            selected_view: "front".into(),
            anim_time: 0.0,
            anim_action: "walk_right".into(),
            character_frame_path: None,
            #[cfg(not(target_arch = "wasm32"))]
            effectcraft_animation: None,
            film_state: json!({}),
            film_timeline: json!({}),
            studio_playhead: 0.0,
            studio_frame_path: None,
            #[cfg(not(target_arch = "wasm32"))]
            studio_film: None,
            #[cfg(not(target_arch = "wasm32"))]
            studio_open_error: None,
            #[cfg(not(target_arch = "wasm32"))]
            studio_rendered_animation: None,
            #[cfg(not(target_arch = "wasm32"))]
            pending_studio_render: None,
            textures: HashMap::new(),
            worker_tx,
            worker_rx,
            busy_command: None,
            control_rx: None,
            quit_requested: false,
            synthetic: Vec::new(),
            lyrics_host: None,
            lyrics_song_details: false,
            #[cfg(not(target_arch = "wasm32"))]
            dialogs,
            #[cfg(not(target_arch = "wasm32"))]
            repaint_ctx,
            #[cfg(not(target_arch = "wasm32"))]
            pending_storyboard_slot: None,
            #[cfg(not(target_arch = "wasm32"))]
            web_project_picker: None,
        };
        app.refresh_all();
        app
    }

    pub fn with_control(mut self, rx: Receiver<ControlRequest>) -> Self {
        self.control_rx = Some(rx);
        self
    }

    pub fn with_lyrics_host(mut self, host: Box<dyn LyricsHost>) -> Self {
        self.lyrics_host = Some(host);
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
            let sheet = if lyr["sheet"].is_object() {
                &lyr["sheet"]
            } else {
                &lyr
            };
            self.song_title = sheet["song"]["title"]
                .as_str()
                .or_else(|| sheet["title"].as_str())
                .unwrap_or_default()
                .to_owned();
            self.song_album = sheet["song"]["album"]
                .as_str()
                .or_else(|| sheet["album"].as_str())
                .unwrap_or_default()
                .to_owned();
            self.song_notes = sheet["notes"]
                .as_str()
                .or_else(|| sheet["artist_note"].as_str())
                .map(str::to_owned)
                .or_else(|| {
                    sheet["notes"].as_array().map(|arr| {
                        arr.iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                })
                .unwrap_or_default();
            self.lyrics_sections = sheet["sections"].as_array().cloned().unwrap_or_default();
            if let Some(raw) = sheet["lyrics"].as_str() {
                self.song_lyrics_text = raw.to_owned();
            } else if !self.lyrics_sections.is_empty() {
                let mut blocks = Vec::new();
                for sec in &self.lyrics_sections {
                    let sec_id = sec["label"]
                        .as_str()
                        .or_else(|| sec["section"].as_str())
                        .or_else(|| sec["id"].as_str())
                        .unwrap_or("section");
                    let lines: Vec<String> = sec["lines"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|l| l["text"].as_str().or_else(|| l.as_str()))
                        .map(str::to_owned)
                        .collect();
                    blocks.push(format!("[{sec_id}]\n{}", lines.join("\n")));
                }
                self.song_lyrics_text = blocks.join("\n\n");
            }
        }
        if let Ok(ws) = self.session.execute("workshop.status", json!({})) {
            let audio = ws["audio"]["path"].as_str().or_else(|| {
                ws["stages"]
                    .as_array()?
                    .iter()
                    .find(|s| s["id"] == "song")?["inputs"]
                    .as_array()?
                    .first()?["path"]
                    .as_str()
            });
            if let Some(a) = audio {
                self.song_audio_path = a.to_owned();
            }
        }
        if let Ok(wr) = self.session.execute("words.read", json!({})) {
            self.words_doc = wr["document"].clone();
        }
    }

    pub fn refresh_animation(&mut self) {
        if let Ok(chars) = self.session.execute("character.list", json!({})) {
            self.animation_revision = self.animation_revision.wrapping_add(1);
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
            self.start_command_worker(ctx, id, params, None);
            return;
        }
        match self.execute_session_command(id, params) {
            Ok(val) => self.handle_command_success(id, val),
            Err(err) => {
                self.status_line = format!("{id} failed: {err}");
                self.last_error = Some(err);
            }
        }
    }

    fn start_command_worker(
        &mut self,
        ctx: &Context,
        id: &str,
        params: Value,
        follow_up: Option<&'static str>,
    ) {
        let mut bg_session = Session {
            project_path: self.session.project_path.clone(),
            project_assets: self.session.project_assets.clone(),
            video_project_path: self.session.video_project_path.clone(),
            song_audio: self.session.song_audio.clone(),
            ..Session::default()
        };
        let project = bg_session.project_path.clone();
        let tx = self.worker_tx.clone();
        let ctx = ctx.clone();
        let mut id = id.to_owned();
        self.busy_command = Some(id.clone());
        self.last_error = None;
        self.status_line = format!("Running {id}…");
        std::thread::spawn(move || {
            let mut outcome = bg_session.execute(&id, params.clone());
            if let Some(next) = follow_up
                && outcome.is_ok()
            {
                next.clone_into(&mut id);
                outcome = bg_session.execute(next, params);
            }
            let _ = tx.send(WorkerResult {
                id,
                project,
                outcome: outcome.map_err(|error| error.to_string()),
            });
            ctx.request_repaint();
        });
    }

    fn execute_session_command(&mut self, id: &str, params: Value) -> Result<Value, String> {
        let previous_project = self.session.project_path.clone();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let switches_project = matches!(id, "project.new" | "project.open" | "project.close")
                || (id == "project.save" && params["path"].is_string());
            if switches_project {
                if self
                    .effectcraft_animation
                    .as_ref()
                    .is_some_and(animation_effectcraft::AnimationEffectcraft::is_dirty)
                {
                    return Err(
                        "Save the open EffectCraft composition before changing projects.".into(),
                    );
                }
                if self
                    .studio_film
                    .as_ref()
                    .is_some_and(studio_filmcraft::StudioFilmcraft::is_dirty)
                {
                    return Err("Save the open FilmCraft project before changing projects.".into());
                }
            }
        }
        let result = self.session.execute(id, params).map_err(|e| e.to_string());
        if self.session.project_path != previous_project {
            self.selected_character.clear();
            self.character_frame_path = None;
            self.selected_page = 0;
            self.storyboard_pages.clear();
            self.storyboard_scenes.clear();
            self.unfilled_checks.clear();
            self.preview_image_path = None;
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.effectcraft_animation = None;
                self.studio_film = None;
                self.studio_open_error = None;
            }
        }
        result
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
            "scribbles.generate" | "scribbles.new" | "scribbles.image" | "scribbles.sheet"
            | "scribbles.poses" | "scribbles.actions" | "scribbles.custom" => {
                self.assistant.last_receipt = Some(val);
                if let Some(path) = self.session.project_path.clone() {
                    let _ = self
                        .session
                        .execute("project.open", json!({ "path": path }));
                }
                self.refresh_all();
            }
            cmd if cmd.starts_with("storyboard.") => {
                self.refresh_storyboard();
            }
            cmd if cmd.starts_with("song.")
                || cmd.starts_with("words.")
                || cmd.starts_with("workshop.") =>
            {
                self.refresh_lyrics();
                self.refresh_storyboard();
            }
            cmd if cmd.starts_with("character.") || cmd.starts_with("animation.") => {
                #[cfg(not(target_arch = "wasm32"))]
                if self.view == ViewMode::Animation
                    && let Some(path) = match id {
                        "character.rig" | "character.action" => val["ecproj"].as_str(),
                        "animation.prepare_motion" => val["project"].as_str(),
                        _ => None,
                    }
                {
                    let result = self
                        .open_effectcraft_composition_at(Path::new(path), val["start"].as_f64());
                    self.set_status_result(result);
                }
                #[cfg(not(target_arch = "wasm32"))]
                if id == "character.render" {
                    let pending = self.pending_studio_render.take();
                    if let Some(project) = self.session.project_path.as_ref()
                        && let Some(clip) = val["clip"].as_str().map(PathBuf::from)
                        && project
                            .parent()
                            .is_some_and(|parent| clip.starts_with(parent))
                    {
                        self.studio_rendered_animation = Some((project.clone(), clip.clone()));
                        if pending.as_ref() == Some(project) {
                            let result = self.import_studio_animation(&clip);
                            self.set_status_result(result);
                        }
                    }
                }
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

    #[cfg(not(target_arch = "wasm32"))]
    fn poll_dialogs(&mut self, ctx: &Context) {
        for (action, answer) in self.dialogs.take_answers() {
            if action == DialogAction::WebProjectPicker {
                if let Some(reply) = self.web_project_picker.take() {
                    let result = match answer {
                        craft_dialogs::Answer::Paths(paths) => Ok(json!({ "path": paths.first() })),
                        craft_dialogs::Answer::Unavailable(error) => Err(error),
                        craft_dialogs::Answer::Cancelled | craft_dialogs::Answer::Message(_) => {
                            Ok(json!({ "path": null }))
                        }
                    };
                    let _ = reply.send(result);
                }
                continue;
            }
            match answer {
                craft_dialogs::Answer::Paths(paths) => {
                    if let Some(path) = paths.into_iter().next() {
                        let display = path.display().to_string();
                        match action {
                            DialogAction::OpenProject => {
                                self.run_command(ctx, "project.open", json!({ "path": display }));
                            }
                            DialogAction::CreateProjectFolder => {
                                self.new_project_path = display;
                            }
                            DialogAction::SongAudio => {
                                self.song_audio_path = display;
                            }
                            DialogAction::StoryboardHeroImage { scene: _ } => {
                                let slot = self
                                    .pending_storyboard_slot
                                    .take()
                                    .unwrap_or_else(|| "hero_image".into());
                                self.slot_images.insert(slot, display);
                            }
                            DialogAction::AddProjectAsset => {
                                self.new_project_audio = display;
                            }
                            DialogAction::AssistantReferenceImage => {
                                self.assistant.reference_path = display;
                            }
                            DialogAction::OpenEffectcraftComposition => {
                                let result = self.open_effectcraft_composition(&path);
                                self.set_status_result(result);
                            }
                            DialogAction::ImportStudioAnimation => {
                                let result = self.import_studio_animation(&path);
                                self.set_status_result(result);
                            }
                            DialogAction::WebProjectPicker => {}
                        }
                    }
                }
                craft_dialogs::Answer::Unavailable(why) => {
                    self.last_error = Some(why.clone());
                    self.status_line = format!("File dialog unavailable: {why}");
                }
                craft_dialogs::Answer::Cancelled | craft_dialogs::Answer::Message(_) => {}
            }
        }
    }

    fn poll_worker(&mut self) {
        while let Ok(done) = self.worker_rx.try_recv() {
            self.busy_command = None;
            if self.session.project_path != done.project {
                #[cfg(not(target_arch = "wasm32"))]
                if done.id == "character.render" {
                    self.pending_studio_render = None;
                }
                continue;
            }
            match done.outcome {
                Ok(val) => self.handle_command_success(&done.id, val),
                Err(err) => {
                    #[cfg(not(target_arch = "wasm32"))]
                    if done.id == "character.render" {
                        self.pending_studio_render = None;
                    }
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
            #[cfg(not(target_arch = "wasm32"))]
            if matches!(
                req.method.as_str(),
                "ui.projects.pick" | "ui.animation.pick_drawing" | "ui.storyboard.pick_image"
            ) {
                self.open_web_project_picker(req);
                continue;
            }
            let reply = self.handle_control_request(ctx, &req.method, req.params);
            let _ = req.reply.send(reply);
            ctx.request_repaint();
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn open_web_project_picker(&mut self, req: ControlRequest) {
        use craft_dialogs::FileDialog;
        let (expected_view, kind) = match req.method.as_str() {
            "ui.animation.pick_drawing" => (ViewMode::Animation, Some("image")),
            "ui.storyboard.pick_image" => (ViewMode::Storyboard, Some("image")),
            _ => (ViewMode::Projects, req.params["kind"].as_str()),
        };
        if self.view != expected_view || self.web_project_picker.is_some() {
            let _ = req
                .reply
                .send(Err("The file picker is unavailable.".into()));
            return;
        }
        let dialog = match kind {
            Some("project") => FileDialog::open()
                .title("Open Magic Studio project")
                .filter("Magic project", &["magic"]),
            Some("audio") => FileDialog::open()
                .title("Choose song audio")
                .filter("Audio", &["wav", "mp3", "flac", "m4a", "ogg", "aac"]),
            Some("image") => FileDialog::open()
                .title("Choose approved character art")
                .filter("Image", &["png", "webp", "jpg", "jpeg", "psd"]),
            Some("folder") => FileDialog::folder().title("Choose project folder"),
            _ => {
                let _ = req
                    .reply
                    .send(Err("Choose project, audio, image, or folder.".into()));
                return;
            }
        };
        self.web_project_picker = Some(req.reply);
        self.dialogs.ask(dialog, DialogAction::WebProjectPicker);
    }

    pub fn unconfigured_panels(&self) -> Vec<Value> {
        let mut out = Vec::new();
        if self.session.project_path.is_none() {
            out.push(
                json!({ "id": "project", "reason": "No project open", "fix": "project.open" }),
            );
            return out;
        }
        if self.song_audio_path.trim().is_empty() {
            out.push(json!({ "id": "song.audio", "reason": "Audio file path is empty", "fix": "workshop.set_audio" }));
        }
        if self.song_lyrics_text.trim().is_empty() && self.lyrics_sections.is_empty() {
            out.push(json!({ "id": "song.lyrics", "reason": "Canonical lyrics sheet is empty", "fix": "song.save" }));
        }
        if self.storyboard_pages.is_empty() {
            out.push(json!({ "id": "storyboard.pages", "reason": "No storyboard pages loaded", "fix": "storyboard.new" }));
        }
        if self.characters.is_empty() {
            out.push(json!({ "id": "animation.characters", "reason": "No character rigs in project", "fix": "character.intake" }));
        }
        out
    }

    fn handle_control_request(
        &mut self,
        ctx: &Context,
        method: &str,
        params: Value,
    ) -> Result<Value, String> {
        if let Some(reply) = self.handle_protocol(ctx, method, &params) {
            return reply;
        }
        let unconf = self.unconfigured_panels();
        match method {
            "ui.state" | "ui.inspect" => Ok(json!({
                "ok": true,
                "view": self.view.id(),
                "project": self.session.project_path,
                "assistant_open": self.assistant.open,
                "assistant_tab": format!("{:?}", self.assistant.active_tab),
                "selected_page": self.selected_page,
                "selected_character": self.selected_character,
                "song_title": self.song_title,
                "song_album": self.song_album,
                "song_audio_path": self.song_audio_path,
                "lyrics_len": self.song_lyrics_text.len(),
                "lyrics_preview": self.song_lyrics_text.lines().take(5).collect::<Vec<_>>().join(" / "),
                "lyrics_sections_count": self.lyrics_sections.len(),
                "storyboard_pages_count": self.storyboard_pages.len(),
                "characters_count": self.characters.len(),
                "busy": self.busy_command,
                "status": self.status_line,
                "last_error": self.last_error,
                "unconfigured": unconf,
                "empty_fields": self.auto.unconfigured(),
                "element_count": self.auto.list().len(),
                "widgets": self.auto.list(),
            })),
            "ui.feedback" => {
                let mut fb = self
                    .session
                    .execute("ui.feedback", json!({}))
                    .unwrap_or_else(|_| json!({}));
                fb["view"] = json!(self.view.id());
                fb["unconfigured"] = json!(unconf);
                fb["widgets_count"] = json!(self.auto.list().len());
                fb["song_audio_path"] = json!(self.song_audio_path);
                fb["lyrics_sections_count"] = json!(self.lyrics_sections.len());
                Ok(fb)
            }
            "ui.unconfigured" => Ok(json!({
                "ok": true,
                "view": self.view.id(),
                "unconfigured": unconf,
            })),
            "ui.elements" | "ui.components" | "ui.controls" => Ok(json!({
                "ok": true,
                "view": self.view.id(),
                "widgets": self.auto.list(),
                "elements": [
                    { "id": "nav-projects", "label": "Projects" },
                    { "id": "nav-storyboard", "label": "Storyboard" },
                    { "id": "nav-lyrics", "label": "Lyrics" },
                    { "id": "nav-animation", "label": "Animation" },
                    { "id": "nav-motion", "label": "Motion" },
                    { "id": "nav-studio", "label": "Studio" },
                    { "id": "drawer-fidget", "label": "Fidget" },
                    { "id": "drawer-scribbles", "label": "Scribbles" }
                ]
            })),
            "ui.focus" => {
                let id = params["id"].as_str().unwrap_or_default();
                let found = self.auto.focus(ctx, id);
                Ok(json!({ "ok": found, "id": id }))
            }
            "ui.projects.open" if self.view == ViewMode::Projects => {
                let path = params["path"]
                    .as_str()
                    .ok_or("A project path is required.")?;
                let result =
                    self.handle_control_request(ctx, "project.open", json!({ "path": path }))?;
                if let Some(view) = params["view"].as_str().and_then(ViewMode::from_name) {
                    self.view = view;
                }
                Ok(result)
            }
            #[cfg(not(target_arch = "wasm32"))]
            "ui.studio.toolbar" if self.view == ViewMode::Studio => {
                self.studio_toolbar_command(ctx, &params)
            }
            #[cfg(not(target_arch = "wasm32"))]
            "ui.animation.controls" if self.view == ViewMode::Animation => {
                self.animation_workspace_command(ctx, &params)
            }
            #[cfg(not(target_arch = "wasm32"))]
            "ui.storyboard.workspace" if self.view == ViewMode::Storyboard => {
                self.storyboard_workspace_command(&params)
            }
            #[cfg(not(target_arch = "wasm32"))]
            "ui.studio.import_animation" if self.view == ViewMode::Studio => {
                let path = params["path"].as_str().ok_or("path is required")?;
                let result = self.import_studio_animation(Path::new(path))?;
                self.status_line = result.clone();
                Ok(json!({ "ok": true, "status": result }))
            }
            "ui.set_view" => {
                let target = params["view"]
                    .as_str()
                    .or_else(|| params["id"].as_str())
                    .unwrap_or_default();
                let Some(v) = ViewMode::from_name(target) else {
                    return Err(format!("Unknown view `{target}`"));
                };
                self.view = v;
                Ok(json!({ "ok": true, "view": self.view.id() }))
            }
            "ui.click" => {
                let target = params["id"].as_str().unwrap_or_default();
                match target {
                    "nav-projects" | "projects" => self.view = ViewMode::Projects,
                    "nav-storyboard" | "storyboard" => self.view = ViewMode::Storyboard,
                    "nav-lyrics" | "lyrics" => self.view = ViewMode::Lyrics,
                    "nav-animation" | "animation" => self.view = ViewMode::Animation,
                    "nav-motion" | "motion" => self.view = ViewMode::Motion,
                    "nav-studio" | "studio" => self.view = ViewMode::Studio,
                    "drawer-fidget" | "fidget" => {
                        self.assistant.open = true;
                        self.assistant.active_tab = AssistantTab::Fidget;
                    }
                    "drawer-scribbles" | "scribbles" => {
                        self.assistant.open = true;
                        self.assistant.active_tab = AssistantTab::Scribbles;
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    "animation-send-to-studio" if self.view == ViewMode::Animation => {
                        self.send_active_animation_to_studio(ctx)?;
                    }
                    id if self.view == ViewMode::Animation
                        && id.starts_with("animation-asset-") =>
                    {
                        let asset = id.trim_start_matches("animation-asset-");
                        self.select_animation_asset(asset)?;
                    }
                    _ => {
                        return Err(format!("Unknown UI element `{target}`"));
                    }
                }
                Ok(
                    json!({ "ok": true, "view": self.view.id(), "assistant_open": self.assistant.open }),
                )
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
                let res = self.execute_session_command(other, params)?;
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
        let color_image =
            ColorImage::from_rgba_unmultiplied(size, decoded.as_flat_samples().as_slice());
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
                ViewMode::Motion,
                ViewMode::Studio,
            ] {
                let resp = pill_toggle(ui, self.view == mode, mode.label(), ACCENT);
                self.auto
                    .add_response(&format!("nav-{}", mode.id()), &resp, mode.label());
                if resp.clicked() {
                    self.view = mode;
                }
            }

            // Right-aligned Fidget & Scribbles drawer buttons + Save
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let scribbles_active =
                    self.assistant.open && self.assistant.active_tab == AssistantTab::Scribbles;
                let scrib_resp = pill_toggle(ui, scribbles_active, "✨ Scribbles", ACCENT_TEAL)
                    .on_hover_text("Open Scribbles template-gated image generator");
                self.auto
                    .add_response("drawer-scribbles", &scrib_resp, "Scribbles");
                if scrib_resp.clicked() {
                    if scribbles_active {
                        self.assistant.open = false;
                    } else {
                        self.assistant.open = true;
                        self.assistant.active_tab = AssistantTab::Scribbles;
                    }
                }

                let fidget_active =
                    self.assistant.open && self.assistant.active_tab == AssistantTab::Fidget;
                let fidget_resp = pill_toggle(ui, fidget_active, "⚙ Fidget", ACCENT_VIOLET)
                    .on_hover_text("Open Fidget Markdown frontmatter prompt architect");
                self.auto
                    .add_response("drawer-fidget", &fidget_resp, "Fidget");
                if fidget_resp.clicked() {
                    if fidget_active {
                        self.assistant.open = false;
                    } else {
                        self.assistant.open = true;
                        self.assistant.active_tab = AssistantTab::Fidget;
                    }
                }

                if self.session.project_path.is_some() {
                    let save_resp = secondary_button(ui, "Save Project");
                    self.auto
                        .add_response("btn-save-project", &save_resp, "Save Project");
                    if save_resp.clicked() {
                        self.run_command(ctx, "project.save", json!({}));
                    }
                }
            });
        });
    }

    fn show_projects_view(&mut self, ctx: &Context, ui: &mut Ui, frame: &eframe::Frame) {
        if let Some(host) = self.lyrics_host.as_mut() {
            let project = self
                .session
                .project_path
                .as_ref()
                .map(|path| path.to_string_lossy());
            let rect = ui.available_rect_before_wrap();
            ui.allocate_rect(rect, Sense::hover());
            host.show(
                frame,
                ctx,
                rect,
                project.as_deref().unwrap_or_default(),
                WebWorkspacePage::Projects,
            );
            return;
        }
        ScrollArea::vertical()
            .id_salt("projects_view_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.heading(RichText::new("Projects").size(18.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        #[cfg(not(target_arch = "wasm32"))]
                        if primary_button(ui, "Open .magic Project…").clicked() {
                            self.dialogs.ask(
                                craft_dialogs::FileDialog::open()
                                    .title("Open Magic Studio Project")
                                    .filter("Magic Studio Project", &["magic"]),
                                DialogAction::OpenProject,
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
                            if secondary_button(ui, "Save As…").clicked() {
                                self.dialogs.ask(
                                    craft_dialogs::FileDialog::save()
                                        .title("Create Magic Project")
                                        .file_name("song.magic")
                                        .filter("Magic Project", &["magic"]),
                                    DialogAction::CreateProjectFolder,
                                );
                            }
                            ui.add(
                                egui::TextEdit::singleline(&mut self.new_project_path)
                                    .desired_width(340.0)
                                    .hint_text("/path/to/my-song/my-song.magic"),
                            );
                        });
                        prop_row(ui, "Mode", |ui| {
                            craft_ui::ComboBox::from_id_salt("new_project_mode_combo")
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
                            if secondary_button(ui, "Browse…").clicked() {
                                self.dialogs.ask(
                                    craft_dialogs::FileDialog::open()
                                        .title("Select Project Audio")
                                        .filter("Audio", &["mp3", "wav", "flac", "ogg", "m4a"]),
                                    DialogAction::AddProjectAsset,
                                );
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
                                        ui.label(RichText::new(&path).size(11.0).color(TEXT_MUTED));
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

    fn show_storyboard_view(&mut self, ctx: &Context, ui: &mut Ui, frame: &eframe::Frame) {
        if self.session.project_path.is_none() {
            empty_state(
                ui,
                "Open a Project First",
                "Switch to the Projects tab to open or create a .magic project.",
            );
            return;
        }

        if let Some(host) = self.lyrics_host.as_mut() {
            let project = self
                .session
                .project_path
                .as_ref()
                .map(|path| path.to_string_lossy());
            let rect = ui.available_rect_before_wrap();
            ui.allocate_rect(rect, Sense::hover());
            host.show(
                frame,
                ctx,
                rect,
                project.as_deref().unwrap_or_default(),
                WebWorkspacePage::Storyboard,
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
                            .on_hover_text(
                                "Add a new scene page from the template (`storyboard.add`)",
                            )
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
                                                if ui.small_button("Browse…").clicked() {
                                                    self.pending_storyboard_slot = Some(k.clone());
                                                    self.dialogs.ask(
                                                        craft_dialogs::FileDialog::open()
                                                            .title("Select Storyboard Image")
                                                            .filter("Image", &["png", "jpg", "jpeg"]),
                                                        DialogAction::StoryboardHeroImage {
                                                            scene: current_scene.unwrap_or(1),
                                                        },
                                                    );
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

    fn show_lyrics_view(&mut self, ctx: &Context, ui: &mut Ui, frame: &eframe::Frame) {
        let Some(project) = self.session.project_path.clone() else {
            empty_state(
                ui,
                "Open a Project First",
                "Switch to Projects to open a .magic song project.",
            );
            return;
        };
        if self.lyrics_host.is_some() && !self.lyrics_song_details {
            if secondary_button(ui, "Song details…").clicked() {
                self.lyrics_song_details = true;
            }
            let rect = ui.available_rect_before_wrap();
            ui.allocate_rect(rect, Sense::hover());
            if let Some(host) = self.lyrics_host.as_mut() {
                host.show(
                    frame,
                    ctx,
                    rect,
                    &project.display().to_string(),
                    WebWorkspacePage::Lyrics,
                );
            }
            return;
        }
        ScrollArea::vertical()
            .id_salt("lyrics_view_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Lyrics");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let save_resp = primary_button(ui, "Save Song Details (`song.save`)");
                        self.auto
                            .add_response("btn-save-song", &save_resp, "Save Song Details");
                        if save_resp.clicked() {
                            self.run_command(
                                ctx,
                                "song.save",
                                json!({
                                    "title": self.song_title,
                                    "album": self.song_album,
                                    "lyrics": self.song_lyrics_text,
                                    "notes": self.song_notes,
                                    "document": self.words_doc,
                                }),
                            );
                        }
                        if self.lyrics_host.is_some()
                            && secondary_button(ui, "◂ Lyrics designer").clicked()
                        {
                            self.lyrics_song_details = false;
                        }
                    });
                });
                ui.add_space(6.0);

                if section_header(ui, "Song Audio (`workshop.set_audio`)", true) {
                    card_frame().show(ui, |ui| {
                        prop_row(ui, "Audio File", |ui| {
                            let link_resp = primary_button(ui, "Link Audio");
                            self.auto
                                .add_response("btn-link-audio", &link_resp, "Link Audio");
                            if link_resp.clicked() && !self.song_audio_path.trim().is_empty() {
                                self.run_command(
                                    ctx,
                                    "workshop.set_audio",
                                    json!({ "path": self.song_audio_path.trim() }),
                                );
                            }
                            #[cfg(not(target_arch = "wasm32"))]
                            if secondary_button(ui, "Browse…").clicked() {
                                self.dialogs.ask(
                                    craft_dialogs::FileDialog::open()
                                        .title("Select Song Audio")
                                        .filter("Audio", &["mp3", "wav", "flac", "ogg", "m4a"]),
                                    DialogAction::SongAudio,
                                );
                            }
                            let audio_resp = ui.add(
                                egui::TextEdit::singleline(&mut self.song_audio_path)
                                    .desired_width(300.0),
                            );
                            self.auto.add_field(
                                "song.audio",
                                &audio_resp,
                                "Audio File",
                                &self.song_audio_path,
                            );
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let beats_resp =
                                secondary_button(ui, "Analyze Song Beats (`audio.analyze`)");
                            self.auto.add_response(
                                "btn-analyze-beats",
                                &beats_resp,
                                "Analyze Song Beats",
                            );
                            if beats_resp.clicked() {
                                self.run_command(ctx, "audio.analyze", json!({}));
                            }
                            let design_resp =
                                secondary_button(ui, "Design Sung Lines (`words.design`)");
                            self.auto.add_response(
                                "btn-design-lines",
                                &design_resp,
                                "Design Sung Lines",
                            );
                            if design_resp.clicked() {
                                self.run_command(ctx, "words.design", json!({}));
                            }
                            let letter_resp =
                                secondary_button(ui, "Letter in EffectCraft (`words.letter`)");
                            self.auto.add_response(
                                "btn-letter-effectcraft",
                                &letter_resp,
                                "Letter in EffectCraft",
                            );
                            if letter_resp.clicked() {
                                self.run_command(ctx, "words.letter", json!({}));
                            }
                        });
                    });
                }

                ui.add_space(8.0);
                if section_header(ui, "Song Metadata & Lyrics Sheet", true) {
                    card_frame().show(ui, |ui| {
                        prop_row(ui, "Song Title", |ui| {
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut self.song_title)
                                    .desired_width(320.0),
                            );
                            self.auto.add_field(
                                "song.title",
                                &resp,
                                "Song Title",
                                &self.song_title,
                            );
                        });
                        prop_row(ui, "Album", |ui| {
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut self.song_album)
                                    .desired_width(320.0),
                            );
                            self.auto
                                .add_field("song.album", &resp, "Album", &self.song_album);
                        });
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("Canonical Artist Lyrics")
                                .size(11.5)
                                .color(TEXT_SECONDARY),
                        );
                        let lyr_resp = ui.add(
                            egui::TextEdit::multiline(&mut self.song_lyrics_text)
                                .desired_rows(12)
                                .desired_width(f32::INFINITY),
                        );
                        self.auto.add_field(
                            "song.lyrics",
                            &lyr_resp,
                            "Canonical Artist Lyrics",
                            &self.song_lyrics_text,
                        );
                    });
                }
            });
    }

    fn select_animation_asset(&mut self, id: &str) -> Result<(), String> {
        let asset = self
            .session
            .project_assets
            .iter()
            .find(|asset| asset.id == id)
            .ok_or_else(|| format!("Unknown Animation asset `{id}`"))?;
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = asset.path.clone()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("ecproj"))
        {
            let message = self.open_effectcraft_composition(&path)?;
            self.set_status_result(Ok(message));
            return Ok(());
        }
        self.selected_character.clone_from(&asset.id);
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn open_effectcraft_composition(&mut self, path: &Path) -> Result<String, String> {
        self.open_effectcraft_composition_at(path, None)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn open_effectcraft_composition_at(
        &mut self,
        path: &Path,
        seconds: Option<f64>,
    ) -> Result<String, String> {
        if self
            .effectcraft_animation
            .as_ref()
            .is_some_and(animation_effectcraft::AnimationEffectcraft::is_dirty)
        {
            return Err("Save the open EffectCraft composition first.".into());
        }
        let mut composition = animation_effectcraft::AnimationEffectcraft::open(path)
            .map_err(|error| format!("Open {}: {error}", path.display()))?;
        if let Some(seconds) = seconds {
            composition.seek(seconds)?;
        }
        self.effectcraft_animation = Some(composition);
        Ok(format!("Opened {}", path.display()))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn send_active_animation_to_studio(&mut self, ctx: &Context) -> Result<(), String> {
        if self.busy_command.is_some() {
            return Err("Wait for the current operation before rendering.".into());
        }
        let params = self
            .effectcraft_animation
            .as_ref()
            .ok_or("Open an EffectCraft project first.")?
            .studio_render_request()?;
        self.pending_studio_render = Some(
            self.session
                .project_path
                .clone()
                .ok_or("Open a Magic project first.")?,
        );
        self.run_command(ctx, "character.render", params);
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn effectcraft_animation_toolbar(&mut self, ctx: &Context, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            let open = secondary_button(ui, "Open EffectCraft Project…");
            self.auto.add_response(
                "animation-open-effectcraft",
                &open,
                "Open EffectCraft Project",
            );
            if open.clicked() {
                self.dialogs.ask(
                    craft_dialogs::FileDialog::open()
                        .title("Open EffectCraft Project")
                        .filter("EffectCraft Project", &["ecproj"]),
                    DialogAction::OpenEffectcraftComposition,
                );
            }
            let rig_button = ui.add_enabled(
                !self.selected_character.is_empty(),
                egui::Button::new("Open Character Rig"),
            );
            self.auto.add_response(
                "animation-open-character-rig",
                &rig_button,
                "Open Character Rig",
            );
            if rig_button.clicked() {
                let params = json!({ "character": self.selected_character });
                match self.session.execute("character.meta", params) {
                    Ok(metadata) => {
                        let rig =
                            metadata["views"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .find_map(|view| {
                                    (view["view"].as_str() == Some(self.selected_view.as_str()))
                                        .then(|| view["ecproj"].as_str())
                                        .flatten()
                                });
                        let result = if let Some(path) = rig {
                            self.open_effectcraft_composition(Path::new(path))
                        } else {
                            Err("This character view has no EffectCraft rig.".into())
                        };
                        self.set_status_result(result);
                    }
                    Err(error) => self.status_line = error.to_string(),
                }
            }
            if let Some(composition) = self.effectcraft_animation.as_mut() {
                let save = ui.add_enabled(
                    composition.is_dirty(),
                    egui::Button::new("Save EffectCraft Project"),
                );
                self.auto.add_response(
                    "animation-save-effectcraft",
                    &save,
                    "Save EffectCraft Project",
                );
                if save.clicked() {
                    self.status_line = match composition.save() {
                        Ok(()) => "Saved EffectCraft project.".into(),
                        Err(error) => format!("Save EffectCraft project: {error}"),
                    };
                }
                ui.label(composition.name());
                let send = secondary_button(ui, "Render to Studio");
                self.auto
                    .add_response("animation-send-to-studio", &send, "Render to Studio");
                if send.clicked()
                    && let Err(error) = self.send_active_animation_to_studio(ctx)
                {
                    self.set_status_result(Err(error));
                }
            }
        });
    }

    fn show_motion_view(&mut self, ctx: &Context, ui: &mut Ui, frame: &eframe::Frame) {
        let Some(project) = self.session.project_path.as_ref() else {
            empty_state(
                ui,
                "Open a Project First",
                "Open a .magic project to view its motion compositions.",
            );
            return;
        };
        let Some(host) = self.lyrics_host.as_mut() else {
            empty_state(
                ui,
                "Motion Webview Unavailable",
                "Open Motion in the desktop build with webview support.",
            );
            return;
        };
        let rect = ui.available_rect_before_wrap();
        ui.allocate_rect(rect, Sense::hover());
        host.show(
            frame,
            ctx,
            rect,
            &project.display().to_string(),
            WebWorkspacePage::Motion,
        );
    }

    fn show_animation_view(&mut self, ctx: &Context, ui: &mut Ui, frame: &eframe::Frame) {
        if self.session.project_path.is_none() {
            empty_state(
                ui,
                "Open a Project First",
                "Switch to Projects to open a .magic project or character study.",
            );
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(composition) = self.effectcraft_animation.as_mut() {
            egui::Panel::bottom("animation_effectcraft_timeline")
                .resizable(true)
                .default_size(280.0)
                .min_size(240.0)
                .show(ui, |ui| {
                    ui.set_min_height(ui.available_height());
                    composition.show(ui, effectcraft_ui_egui::dock::PanelKind::Timeline);
                });
        }

        if let Some(host) = self.lyrics_host.as_mut() {
            let project = self
                .session
                .project_path
                .as_ref()
                .map(|path| path.to_string_lossy());
            let project = project.as_deref().unwrap_or_default();
            egui::Panel::left("animation_assets_web")
                .resizable(true)
                .default_size(210.0)
                .min_size(165.0)
                .show(ui, |ui| {
                    let rect = ui.available_rect_before_wrap();
                    ui.allocate_rect(rect, Sense::hover());
                    host.show(frame, ctx, rect, project, WebWorkspacePage::AnimationAssets);
                });
            egui::Panel::right("animation_inspector_web")
                .resizable(true)
                .default_size(290.0)
                .min_size(215.0)
                .show(ui, |ui| {
                    let rect = ui.available_rect_before_wrap();
                    ui.allocate_rect(rect, Sense::hover());
                    host.show(
                        frame,
                        ctx,
                        rect,
                        project,
                        WebWorkspacePage::AnimationInspector,
                    );
                });
        } else {
            self.show_animation_fallback_rails(ctx, ui);
        }
        self.show_animation_stage(ctx, ui);
    }

    fn show_animation_fallback_rails(&mut self, ctx: &Context, ui: &mut Ui) {
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
                            let button = pill_toggle(ui, selected, &id, ACCENT_TEAL);
                            self.auto.add_response(
                                &format!("animation-character-{id}"),
                                &button,
                                &id,
                            );
                            if button.clicked() {
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
                                    let is_composition = asset.path.as_ref().is_some_and(|path| {
                                        path.extension().is_some_and(|extension| {
                                            extension.eq_ignore_ascii_case("ecproj")
                                        })
                                    });
                                    let select = ui.small_button(if is_composition {
                                        "Open Composition"
                                    } else {
                                        "Select"
                                    });
                                    self.auto.add_response(
                                        &format!("animation-asset-{}", asset.id),
                                        &select,
                                        &asset.name,
                                    );
                                    if select.clicked()
                                        && let Err(error) = self.select_animation_asset(&asset.id)
                                    {
                                        self.last_error = Some(error);
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
                            craft_ui::ComboBox::from_id_salt("char_view_combo")
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
                            craft_ui::ComboBox::from_id_salt("char_action_combo")
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
                                        ui.selectable_value(&mut self.anim_action, a.to_owned(), a);
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
    }

    fn show_animation_stage(&mut self, ctx: &Context, ui: &mut Ui) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG_DEEP)
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ui, |ui| {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.effectcraft_animation_toolbar(ctx, ui);
                    if let Some(composition) = self.effectcraft_animation.as_mut() {
                        composition.show(ui, effectcraft_ui_egui::dock::PanelKind::Composition);
                        return;
                    }
                }
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
                        "Open an Animation",
                        "Open an EffectCraft composition from the asset list, or select a character and open its rig.",
                    );
                }
            });
    }

    #[cfg(target_arch = "wasm32")]
    fn show_studio_view(&mut self, ctx: &Context, ui: &mut Ui, _frame: &eframe::Frame) {
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
    fn on_exit(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.effectcraft_animation.take();
            self.studio_film.take();
        }
    }

    fn raw_input_hook(&mut self, _ctx: &Context, raw_input: &mut egui::RawInput) {
        raw_input.events.append(&mut self.synthetic);
    }

    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        self.auto.begin_frame();
        let ctx = ui.ctx().clone();
        #[cfg(not(target_arch = "wasm32"))]
        let _ = self.repaint_ctx.set(ctx.clone());
        let ctx = &ctx;
        if !self.theme_applied {
            theme::apply(ctx);
            self.theme_applied = true;
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.poll_dialogs(ctx);
        self.poll_worker();
        self.poll_control(ctx);

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(composition) = self.effectcraft_animation.as_mut()
            && (self.view == ViewMode::Animation || ctx.input(|i| i.viewport().close_requested()))
        {
            composition.tick(ctx, frame);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if ctx.input(|input| input.viewport().close_requested())
            && self
                .studio_film
                .as_ref()
                .is_some_and(studio_filmcraft::StudioFilmcraft::is_dirty)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.view = ViewMode::Studio;
            self.status_line = "Save the open FilmCraft project before closing Magic.".into();
        }

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
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("Fidget (text.complete) · Scribbles (image.generate)")
                            .size(10.5)
                            .color(TEXT_MUTED),
                    );
                    let running;
                    let (status, color) = if let Some(busy) = &self.busy_command {
                        running = format!("Running {busy}…");
                        (running.as_str(), ACCENT_TEAL)
                    } else if let Some(error) = &self.last_error {
                        (error.as_str(), ACCENT_RED)
                    } else {
                        (self.status_line.as_str(), TEXT_SECONDARY)
                    };
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        if self.busy_command.is_some() {
                            ui.spinner();
                        }
                        ui.add(
                            egui::Label::new(RichText::new(status).size(11.0).color(color))
                                .truncate(),
                        )
                        .on_hover_text(status);
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
            #[cfg(not(target_arch = "wasm32"))]
            if self.assistant.browse_reference_requested {
                self.assistant.browse_reference_requested = false;
                self.dialogs.ask(
                    craft_dialogs::FileDialog::open()
                        .title("Select Reference PNG")
                        .filter("PNG Image", &["png"]),
                    DialogAction::AssistantReferenceImage,
                );
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
                ViewMode::Projects => self.show_projects_view(ctx, ui, frame),
                ViewMode::Storyboard => self.show_storyboard_view(ctx, ui, frame),
                ViewMode::Lyrics => self.show_lyrics_view(ctx, ui, frame),
                ViewMode::Animation => self.show_animation_view(ctx, ui, frame),
                ViewMode::Motion => self.show_motion_view(ctx, ui, frame),
                ViewMode::Studio => self.show_studio_view(ctx, ui, frame),
            });

        if let Some(host) = self.lyrics_host.as_mut() {
            host.finish_frame();
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(composition) = self.effectcraft_animation.as_mut() {
                composition.show_dialogs(ctx);
            }
            self.dialogs.show_pending(Some(&*frame));
        }
    }
}
