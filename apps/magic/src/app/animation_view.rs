//! Animation view — prepares characters for motion in EffectCraft and reviews
//! saved animation revisions. Backed by `animation.*` engine commands.

use eframe::egui;
use magic_engine::Session;
use serde_json::{Value, json};

pub enum AnimationAction {
    None,
    Status(String),
    Error(String),
}

#[derive(Default)]
pub struct AnimationViewState {
    loaded_for: Option<String>,
    motion_options: Value,
    characters: Vec<Value>,
    status: Option<String>,
}

impl AnimationViewState {
    pub fn refresh(&mut self, engine: &mut Session) {
        let current = engine
            .project_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned());
        if self.loaded_for == current {
            return;
        }
        self.motion_options = engine
            .execute("animation.motion_options", json!({}))
            .unwrap_or(Value::Null);
        self.characters = self
            .motion_options
            .get("characters")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        self.loaded_for = current;
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, engine: &mut Session) -> AnimationAction {
        let mut action = AnimationAction::None;

        egui::Panel::left("animation_rail")
            .default_size(260.0)
            .show(ui, |ui| {
                ui.heading("Characters");
                egui::ScrollArea::vertical()
                    .id_salt("animation_characters_scroll")
                    .show(ui, |ui| {
                        if self.characters.is_empty() {
                            ui.label("No characters in project.");
                        } else {
                            for c in &self.characters {
                                let name = c
                                    .get("name")
                                    .and_then(Value::as_str)
                                    .unwrap_or("Character");
                                let ready = c
                                    .get("rigged")
                                    .and_then(Value::as_bool)
                                    .unwrap_or(false);
                                let label = if ready {
                                    format!("✓ {name}")
                                } else {
                                    format!("○ {name}")
                                };
                                if ui.selectable_label(false, label).clicked() {
                                    if let Err(e) = engine.execute(
                                        "animation.prepare_motion",
                                        json!({ "character": name }),
                                    ) {
                                        action = AnimationAction::Error(e.to_string());
                                    }
                                }
                                if !ready {
                                    ui.small_button("Rig");
                                    if ui.button("Rig").clicked() {
                                        if let Err(e) = engine.execute(
                                            "animation.prepare_motion",
                                            json!({ "character": name }),
                                        ) {
                                            action = AnimationAction::Error(e.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    });
            });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Motion Options");
            egui::ScrollArea::vertical().show(ui, |ui| {
                if let Some(options) = self.motion_options.get("options").and_then(Value::as_array) {
                    if options.is_empty() {
                        ui.label("No motion options available.");
                    }
                    for opt in options {
                        let label = opt
                            .get("label")
                            .and_then(Value::as_str)
                            .unwrap_or("Motion");
                        ui.button(label);
                        if ui.button(label).clicked() {
                            if let Err(e) = engine.execute(
                                "animation.motion_options",
                                json!({ "option": label }),
                            ) {
                                action = AnimationAction::Error(e.to_string());
                            }
                        }
                    }
                } else {
                    ui.label("Load a project to see motion options.");
                }
            });
        });

        action
    }

    pub fn review(&mut self, ui: &mut egui::Ui, engine: &mut Session) -> AnimationAction {
        let mut action = AnimationAction::None;
        ui.heading("Saved Animation Revisions");
        if ui.button("Refresh Revisions").clicked() {
            match engine.execute("animation.review", json!({})) {
                Ok(value) => {
                    let revisions = value
                        .get("revisions")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if revisions.is_empty() {
                            ui.label("No saved revisions.");
                        } else {
                            for r in revisions {
                                let id = r.get("id").and_then(Value::as_str).unwrap_or("?");
                                let status = r
                                    .get("status")
                                    .and_then(Value::as_str)
                                    .unwrap_or("unknown");
                                ui.horizontal(|ui| {
                                    ui.label(format!("{id} — {status}"));
                                    if ui.button("Apply").clicked() {
                                        if let Err(e) = engine.execute(
                                            "animation.save_revision",
                                            json!({ "revision": id }),
                                        ) {
                                            action = AnimationAction::Error(e.to_string());
                                        }
                                    }
                                });
                            }
                        }
                    });
                }
                Err(e) => action = AnimationAction::Error(e.to_string()),
            }
        }
        action
    }
}
