//! Active `.magic` project container view — orchestrates Song audio/transcript,
//! Asset library & linked `.magic` sub-projects, and Craft project targets.

use eframe::egui;
use magic_engine::Session;
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;

pub enum ProjectViewAction {
    None,
    OpenStoryboard,
    SwitchProject(String),
    Status(String),
    Error(String),
}

pub struct ProjectViewState {
    pub loaded_for: Option<String>,
    pub status: Value,
    pub assets: Vec<Value>,
    pub monitoring: Value,
}

impl Default for ProjectViewState {
    fn default() -> Self {
        Self {
            loaded_for: None,
            status: Value::Null,
            assets: Vec::new(),
            monitoring: Value::Null,
        }
    }
}

impl ProjectViewState {
    pub fn refresh(&mut self, engine: &mut Session) {
        let current = engine
            .project_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned());
        self.status = engine
            .execute("workshop.status", json!({}))
            .unwrap_or(Value::Null);
        self.assets = engine
            .execute("project.list_assets", json!({}))
            .ok()
            .and_then(|v| v.get("assets").and_then(Value::as_array).cloned())
            .unwrap_or_default();
        self.monitoring = engine
            .execute("center.status", json!({}))
            .unwrap_or(Value::Null);
        self.loaded_for = current;
    }

    fn prereq_state(&self, id: &str) -> &str {
        self.monitoring
            .get("prerequisites")
            .and_then(Value::as_array)
            .and_then(|arr| {
                arr.iter()
                    .find(|item| item.get("id").and_then(Value::as_str) == Some(id))
            })
            .and_then(|item| item.get("state").and_then(Value::as_str))
            .unwrap_or("unknown")
    }

    fn stage(&self, id: &str) -> Option<&Value> {
        self.status
            .get("stages")
            .and_then(Value::as_array)
            .and_then(|arr| {
                arr.iter()
                    .find(|item| item.get("id").and_then(Value::as_str) == Some(id))
            })
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, engine: &mut Session) -> ProjectViewAction {
        let Some(project_path) = engine.project_path.clone() else {
            ui.label("No project is open.");
            return ProjectViewAction::None;
        };
        let project_str = project_path.to_string_lossy().into_owned();
        if self.loaded_for.as_deref() != Some(project_str.as_str()) {
            self.refresh(engine);
        }

        let mut action = ProjectViewAction::None;
        let project_dir = project_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let is_animation = engine.project_mode.as_deref() == Some("animation");

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Project Identity Banner
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let name = project_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Untitled");
                    ui.heading(name);
                    let badge = if is_animation {
                        "[Animation / Character Study]"
                    } else {
                        "[Song Project]"
                    };
                    ui.strong(badge);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Refresh State").clicked() {
                            self.refresh(engine);
                        }
                        if ui.button("Open Project Folder").clicked() {
                            open_os_path(&project_dir);
                        }
                    });
                });
                ui.small(&project_str);
            });

            ui.add_space(8.0);

            // 1. Song & Transcript Block (shown for Song projects)
            if !is_animation {
                egui::CollapsingHeader::new(format!(
                    "1. Song & Transcript  [{}]",
                    self.prereq_state("song")
                ))
                .default_open(true)
                .show(ui, |ui| {
                    let audio_path = engine
                        .song_audio
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned());

                    ui.horizontal(|ui| {
                        ui.strong("Audio Recording:");
                        match &audio_path {
                            Some(path) => {
                                ui.label(path);
                                if ui.small_button("Open").clicked() {
                                    open_os_path(Path::new(path));
                                }
                            }
                            None => {
                                ui.label("No audio linked yet.");
                            }
                        }
                        if ui
                            .button(if audio_path.is_some() {
                                "Change Audio…"
                            } else {
                                "Choose Audio…"
                            })
                            .clicked()
                            && let Some(file) = rfd::FileDialog::new()
                                .set_title("Choose Song Audio")
                                .add_filter("Audio", &["wav", "mp3", "flac", "m4a", "aac", "ogg"])
                                .pick_file()
                        {
                            let path = file.to_string_lossy().into_owned();
                            match run_and_save(
                                engine,
                                "workshop.set_audio",
                                json!({ "path": path }),
                            ) {
                                Ok(_) => {
                                    self.refresh(engine);
                                    action =
                                        ProjectViewAction::Status("Audio linked and saved.".into());
                                }
                                Err(e) => action = ProjectViewAction::Error(e),
                            }
                        }
                    });

                    ui.separator();

                    let transcript_state = self.prereq_state("transcript").to_owned();
                    let word_count = self
                        .stage("transcript")
                        .and_then(|s| s.get("document"))
                        .and_then(|d| d.get("words"))
                        .and_then(Value::as_array)
                        .map(|w| w.len())
                        .unwrap_or(0);

                    ui.horizontal(|ui| {
                        ui.strong("Transcript:");
                        ui.label(format!(
                            "Status: {} ({} timed words)",
                            transcript_state, word_count
                        ));

                        if ui.button("Import Timed Lyrics (.json)…").clicked()
                            && let Some(file) = rfd::FileDialog::new()
                                .set_title("Import Transcript JSON")
                                .add_filter("Transcript JSON", &["json"])
                                .pick_file()
                        {
                            let path = file.to_string_lossy().into_owned();
                            match run_and_save(
                                engine,
                                "workshop.import_transcript",
                                json!({ "path": path }),
                            ) {
                                Ok(_) => {
                                    self.refresh(engine);
                                    action =
                                        ProjectViewAction::Status("Transcript imported.".into());
                                }
                                Err(e) => action = ProjectViewAction::Error(e),
                            }
                        }

                        if audio_path.is_some() && ui.button("Analyze Song (HyperFrames)").clicked()
                        {
                            match run_and_save(
                                engine,
                                "audio.analyze",
                                json!({ "analyze_beats": false }),
                            ) {
                                Ok(_) => {
                                    self.refresh(engine);
                                    action = ProjectViewAction::Status("Audio analyzed.".into());
                                }
                                Err(e) => action = ProjectViewAction::Error(e),
                            }
                        }
                    });
                });

                ui.add_space(8.0);
            }

            // 2. Assets & Sub-Projects Block
            egui::CollapsingHeader::new(format!(
                "2. Assets & Linked Sub-Projects ({} items)  [{}]",
                self.assets.len(),
                self.prereq_state("assets")
            ))
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Add Artwork / Asset…").clicked()
                        && let Some(files) = rfd::FileDialog::new()
                            .set_title("Add Project Artwork")
                            .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                            .pick_files()
                    {
                        for file in files {
                            let path = file.to_string_lossy().into_owned();
                            let name = file
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("asset")
                                .to_string();
                            if let Err(e) = run_and_save(
                                engine,
                                "project.add_asset",
                                json!({
                                    "path": path,
                                    "kind": "image",
                                    "name": name,
                                    "image": name,
                                    "load": false
                                }),
                            ) {
                                action = ProjectViewAction::Error(e);
                                break;
                            }
                        }
                        self.refresh(engine);
                    }

                    if ui.button("Link Sub-Project (.magic)…").clicked()
                        && let Some(file) = rfd::FileDialog::new()
                            .set_title("Link Character Study or Sub-Project (.magic)")
                            .add_filter("Magic Studio Project", &["magic"])
                            .pick_file()
                    {
                        let path = file.to_string_lossy().into_owned();
                        let name = file
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("subproject")
                            .to_string();
                        match run_and_save(
                            engine,
                            "project.add_asset",
                            json!({
                                "path": path,
                                "kind": "character",
                                "name": name,
                                "character": name.to_lowercase().replace(' ', "-"),
                                "load": false
                            }),
                        ) {
                            Ok(_) => {
                                self.refresh(engine);
                                action = ProjectViewAction::Status(format!("Linked {name}."));
                            }
                            Err(e) => action = ProjectViewAction::Error(e),
                        }
                    }
                });

                ui.separator();

                if self.assets.is_empty() {
                    ui.label(
                        "No assets connected yet. Add artwork or link a character study (.magic).",
                    );
                } else {
                    let mut remove_id: Option<String> = None;
                    for asset in &self.assets {
                        let id = asset
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        let name = asset.get("name").and_then(Value::as_str).unwrap_or(&id);
                        let kind = asset.get("kind").and_then(Value::as_str).unwrap_or("asset");
                        let path = asset.get("path").and_then(Value::as_str).unwrap_or("");
                        let is_subproject =
                            path.ends_with(".magic") || kind == "project" || kind == "character";

                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.strong(name);
                                ui.small(format!("({id})"));
                                ui.small(if is_subproject {
                                    "[Sub-Project / Character]"
                                } else {
                                    kind
                                });
                                ui.small(path);

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("Remove").clicked() {
                                            remove_id = Some(id.clone());
                                        }
                                        if !path.is_empty() {
                                            if path.ends_with(".magic") {
                                                if ui.small_button("Open Project").clicked() {
                                                    action = ProjectViewAction::SwitchProject(
                                                        path.to_string(),
                                                    );
                                                }
                                            } else if ui.small_button("Open File").clicked() {
                                                open_os_path(Path::new(path));
                                            }
                                        }
                                    },
                                );
                            });
                        });
                    }

                    if let Some(id) = remove_id {
                        match run_and_save(engine, "project.remove_asset", json!({ "id": id })) {
                            Ok(_) => {
                                self.refresh(engine);
                                action = ProjectViewAction::Status("Removed asset.".into());
                            }
                            Err(e) => action = ProjectViewAction::Error(e),
                        }
                    }
                }
            });

            ui.add_space(8.0);

            // 3. Connected Craft Projects (Storyboard, Animation, Lyrics, Studio)
            egui::CollapsingHeader::new("3. Creative Suite & Project Targets")
                .default_open(true)
                .show(ui, |ui| {
                    let deck_path = project_dir.join("storyboard/storyboard.deckcraft");
                    ui.horizontal(|ui| {
                        ui.strong("Storyboard (DeckCraft):");
                        if deck_path.is_file() {
                            ui.label("storyboard/storyboard.deckcraft");
                            if ui.button("Open Storyboard View").clicked() {
                                action = ProjectViewAction::OpenStoryboard;
                            }
                            if ui.button("Edit in DeckCraft").clicked() {
                                open_os_path(&deck_path);
                            }
                        } else {
                            ui.label("Not created yet (create via Storyboard view or magic-cli).");
                            if ui.button("Open Storyboard View").clicked() {
                                action = ProjectViewAction::OpenStoryboard;
                            }
                        }
                    });

                    ui.separator();

                    let lyrics_path = project_dir.join("lyrics-design.json");
                    ui.horizontal(|ui| {
                        ui.strong("Animation & Lyrics (EffectCraft):");
                        ui.label(format!(
                            "{} assets · Lyrics design: {}",
                            self.assets.len(),
                            if lyrics_path.is_file() {
                                "present"
                            } else {
                                "not started"
                            }
                        ));
                    });

                    ui.separator();

                    let timeline = self.status.get("film_timeline");
                    let timeline_exists = timeline
                        .and_then(|t| t.get("exists"))
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let timeline_path = timeline
                        .and_then(|t| t.get("path"))
                        .and_then(Value::as_str)
                        .unwrap_or("No FilmCraft timeline linked");

                    ui.horizontal(|ui| {
                        ui.strong("Studio (FilmCraft / HyperFrames):");
                        ui.label(timeline_path);
                        if timeline_exists && ui.button("Open FilmCraft Timeline").clicked() {
                            open_os_path(Path::new(timeline_path));
                        }
                    });
                });
        });

        action
    }
}

fn run_and_save(engine: &mut Session, id: &str, params: Value) -> Result<Value, String> {
    let res = engine.execute(id, params).map_err(|e| e.to_string())?;
    let _ = engine.execute("project.save", json!({}));
    Ok(res)
}

pub fn open_os_path(path: &Path) {
    let _ = Command::new("xdg-open").arg(path).spawn();
}
