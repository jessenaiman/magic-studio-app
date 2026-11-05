//! Lyrics view — displays timed lyrics with word-level highlighting, captions,
//! and design controls. Backed by `words.*`, `song.*`, and `lyrics.*` engine commands.
//!
//! Styling is done in native egui (colors, fonts, spacing). If advanced HTML/CSS
//! karaoke styling is needed later, a webview-gtk dependency can be added
//! targeted to this view only — no Tauri, no npm, no Bun.

use eframe::egui;
use magic_engine::Session;
use serde_json::{Value, json};

pub enum LyricsAction {
    None,
    Status(String),
    Error(String),
}

#[derive(Default)]
pub struct LyricsViewState {
    loaded_for: Option<String>,
    lyrics: Vec<Value>,
    current_line: usize,
    status: Option<String>,
}

impl LyricsViewState {
    pub fn refresh(&mut self, engine: &mut Session) {
        let current = engine
            .project_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned());
        if self.loaded_for == current {
            return;
        }
        self.lyrics = engine
            .execute("song.lyrics", json!({}))
            .ok()
            .and_then(|v| v.get("lyrics").and_then(Value::as_array).cloned())
            .unwrap_or_default();
        self.current_line = 0;
        self.loaded_for = current;
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, engine: &mut Session) -> LyricsAction {
        let mut action = LyricsAction::None;

        egui::Panel::left("lyrics_rail")
            .default_size(300.0)
            .show(ui, |ui| {
                ui.heading("Lyrics Lines");
                egui::ScrollArea::vertical()
                    .id_salt("lyrics_lines_scroll")
                    .show(ui, |ui| {
                        if self.lyrics.is_empty() {
                            ui.label("No lyrics in project.");
                        } else {
                            for (i, line) in self.lyrics.iter().enumerate() {
                                let text = line
                                    .get("text")
                                    .and_then(Value::as_str)
                                    .unwrap_or("");
                                let start = line
                                    .get("start")
                                    .and_then(Value::as_f64)
                                    .unwrap_or(0.0);
                                let is_current = i == self.current_line;

                                let label = if is_current {
                                    egui::RichText::new(format!("▶ {text}"))
                                        .color(egui::Color32::from_rgb(100, 215, 194))
                                        .strong()
                                } else {
                                    egui::RichText::new(format!("  {text}"))
                                        .color(egui::Color32::from_rgb(142, 151, 168))
                                };

                                ui.add_space(4.0);
                                ui.label(label);
                                ui.label(
                                    egui::RichText::new(format!("({start:.1}s)"))
                                        .small()
                                        .color(egui::Color32::from_rgb(93, 101, 117)),
                                );

                                if ui.button("Edit").clicked() {
                                    if let Err(e) = engine.execute(
                                        "words.design",
                                        json!({ "line": i }),
                                    ) {
                                        action = LyricsAction::Error(e.to_string());
                                    }
                                }
                            }
                        }
                    });
            });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Current Line");
            if let Some(line) = self.lyrics.get(self.current_line) {
                let text = line.get("text").and_then(Value::as_str).unwrap_or("");
                ui.add_space(20.0);
                ui.label(
                    egui::RichText::new(text)
                        .heading()
                        .color(egui::Color32::from_rgb(232, 236, 243)),
                );
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button("Previous").clicked() && self.current_line > 0 {
                        self.current_line -= 1;
                    }
                    if ui.button("Next").clicked() && self.current_line + 1 < self.lyrics.len() {
                        self.current_line += 1;
                    }
                    if ui.button("Save").clicked() {
                        match engine.execute("song.save", json!({})) {
                            Ok(_) => {
                                self.status = Some("Lyrics saved.".into());
                            }
                            Err(e) => {
                                action = LyricsAction::Error(e.to_string());
                            }
                        }
                    }
                });
            } else {
                ui.label("No lyrics loaded.");
            }
        });

        action
    }
}
