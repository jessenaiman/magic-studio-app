//! Project list view — lists recent `.magic` projects, opens existing files, and creates new projects.

use eframe::egui;
use magic_engine::Session;
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct CatalogEntry {
    pub name: String,
    pub path: String,
    pub folder: String,
    pub availability: String,
    pub next_step: String,
    pub audio_ready: bool,
    pub transcript_ready: bool,
    pub editor_ready: bool,
}

pub enum ProjectListAction {
    None,
    Opened(String),
    Error(String),
}

pub struct ProjectList {
    projects: Vec<CatalogEntry>,
    loaded: bool,
    new_name: String,
    new_root: String,
    new_audio: String,
    new_is_animation: bool,
}

impl ProjectList {
    pub fn new() -> Self {
        Self {
            projects: Vec::new(),
            loaded: false,
            new_name: String::new(),
            new_root: String::new(),
            new_audio: String::new(),
            new_is_animation: false,
        }
    }

    pub fn reload(&mut self, engine: &mut Session) {
        if self.new_root.is_empty()
            && let Ok(root) = engine.execute("project.root", json!({}))
            && let Some(path) = root.get("root").and_then(Value::as_str)
        {
            self.new_root = path.to_string();
        }
        if let Ok(result) = engine.execute("project.catalog", json!({})) {
            self.projects = parse_catalog(&result);
        }
        self.loaded = true;
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, engine: &mut Session) -> ProjectListAction {
        if !self.loaded {
            self.reload(engine);
        }

        let mut action = ProjectListAction::None;

        ui.horizontal(|ui| {
            ui.heading("Recent Projects");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Refresh").clicked() {
                    self.reload(engine);
                }
                if ui.button("Open .magic File…").clicked()
                    && let Some(file) = rfd::FileDialog::new()
                        .set_title("Open Magic Studio Project")
                        .add_filter("Magic Studio Project", &["magic"])
                        .pick_file()
                {
                    let path = file.to_string_lossy().into_owned();
                    match engine.execute("project.open", json!({ "path": path })) {
                        Ok(_) => action = ProjectListAction::Opened(path),
                        Err(e) => action = ProjectListAction::Error(e.to_string()),
                    }
                }
            });
        });

        ui.separator();

        if self.projects.is_empty() {
            ui.label("No recent projects yet. Open an existing .magic file or create a new project below.");
        } else {
            let mut forget_target: Option<String> = None;
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(ui, |ui| {
                    for entry in &self.projects {
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let can_open = entry.availability == "available";
                                if ui
                                    .add_enabled(can_open, egui::Button::new("Open"))
                                    .clicked()
                                {
                                    match engine
                                        .execute("project.open", json!({ "path": entry.path }))
                                    {
                                        Ok(_) => {
                                            action = ProjectListAction::Opened(entry.path.clone())
                                        }
                                        Err(e) => action = ProjectListAction::Error(e.to_string()),
                                    }
                                }

                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.strong(&entry.name);
                                        ui.small(format!("[{}]", entry.availability));
                                        if entry.audio_ready {
                                            ui.small("• Audio");
                                        }
                                        if entry.transcript_ready {
                                            ui.small("• Transcript");
                                        }
                                        if entry.editor_ready {
                                            ui.small("• Studio");
                                        }
                                    });
                                    ui.small(&entry.folder);
                                    ui.small(&entry.next_step);
                                });

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .small_button("×")
                                            .on_hover_text(
                                                "Remove from Recent (files stay on disk)",
                                            )
                                            .clicked()
                                        {
                                            forget_target = Some(entry.path.clone());
                                        }
                                    },
                                );
                            });
                        });
                    }
                });

            if let Some(path) = forget_target
                && let Ok(result) = engine.execute("project.catalog", json!({ "forget": path }))
            {
                self.projects = parse_catalog(&result);
            }
        }

        ui.add_space(12.0);
        ui.separator();
        ui.heading("Create New Project");

        egui::Grid::new("new_project_grid")
            .num_columns(3)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label("Project Type:");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.new_is_animation, false, "Song Project");
                    ui.radio_value(
                        &mut self.new_is_animation,
                        true,
                        "Animation / Character Study",
                    );
                });
                ui.end_row();

                ui.label("Name:");
                ui.text_edit_singleline(&mut self.new_name);
                ui.label("");
                ui.end_row();

                ui.label("Production Root:");
                ui.text_edit_singleline(&mut self.new_root);
                if ui.button("Browse Folder…").clicked()
                    && let Some(dir) = rfd::FileDialog::new()
                        .set_title("Choose Production Root Folder")
                        .pick_folder()
                {
                    self.new_root = dir.to_string_lossy().into_owned();
                }
                ui.end_row();

                if !self.new_is_animation {
                    ui.label("Song Audio (optional):");
                    ui.text_edit_singleline(&mut self.new_audio);
                    if ui.button("Choose Audio…").clicked()
                        && let Some(file) = rfd::FileDialog::new()
                            .set_title("Choose Song Audio")
                            .add_filter("Audio", &["wav", "mp3", "flac", "m4a", "aac", "ogg"])
                            .pick_file()
                    {
                        if self.new_name.trim().is_empty()
                            && let Some(stem) = file.file_stem().and_then(|s| s.to_str())
                        {
                            self.new_name = stem.replace(['_', '-'], " ");
                        }
                        self.new_audio = file.to_string_lossy().into_owned();
                    }
                    ui.end_row();
                }
            });

        ui.add_space(6.0);
        let can_create = !self.new_name.trim().is_empty() && !self.new_root.trim().is_empty();
        if ui
            .add_enabled(can_create, egui::Button::new("Create .magic Project"))
            .clicked()
        {
            let clean_name = self.new_name.trim();
            let path = PathBuf::from(self.new_root.trim())
                .join(clean_name)
                .join(format!("{clean_name}.magic"));
            let path_str = path.to_string_lossy().into_owned();
            let mode = if self.new_is_animation {
                "animation"
            } else {
                "project"
            };
            let mut params = json!({
                "path": path_str,
                "mode": mode,
            });
            if !self.new_is_animation && !self.new_audio.trim().is_empty() {
                params["audio"] = json!(self.new_audio.trim());
            }
            match engine.execute("project.new", params) {
                Ok(_) => {
                    self.new_name.clear();
                    self.new_audio.clear();
                    self.reload(engine);
                    action = ProjectListAction::Opened(path_str);
                }
                Err(e) => action = ProjectListAction::Error(e.to_string()),
            }
        }

        action
    }
}

fn parse_catalog(result: &Value) -> Vec<CatalogEntry> {
    result
        .get("projects")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    let path = v.get("path")?.as_str()?.to_string();
                    let name = v
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or(&path)
                        .to_string();
                    let folder = v
                        .get("folder")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let availability = v
                        .get("availability")
                        .and_then(Value::as_str)
                        .unwrap_or("available")
                        .to_string();
                    let readiness = v.get("readiness").cloned().unwrap_or_default();
                    let next_step = readiness
                        .get("next_step")
                        .and_then(Value::as_str)
                        .unwrap_or("Open project")
                        .to_string();
                    let audio_ready = readiness
                        .get("audio_available")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let transcript_ready = readiness
                        .get("transcript_valid")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let editor_ready = readiness
                        .get("editor_ready")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    Some(CatalogEntry {
                        name,
                        path,
                        folder,
                        availability,
                        next_step,
                        audio_ready,
                        transcript_ready,
                        editor_ready,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}
