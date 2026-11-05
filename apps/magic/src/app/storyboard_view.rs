//! Storyboard view — displays rendered DeckCraft pages (`storyboard/pages/*.png`),
//! lyric cues per scene, page navigation, and triggers `storyboard.create` / `storyboard.render`.

use eframe::egui;
use magic_engine::Session;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::project_view::open_os_path;

pub enum StoryboardAction {
    None,
    Status(String),
    Error(String),
}

#[derive(Default)]
pub struct StoryboardViewState {
    loaded_for: Option<String>,
    deck_path: Option<PathBuf>,
    previews: Vec<PathBuf>,
    page_titles: Vec<String>,
    page_cues: Vec<Vec<String>>,
    current_page: usize,
    cached_texture_path: Option<PathBuf>,
    cached_texture: Option<egui::TextureHandle>,
}

impl StoryboardViewState {
    pub fn reload(&mut self, engine: &Session) {
        self.deck_path = None;
        self.previews.clear();
        self.page_titles.clear();
        self.page_cues.clear();
        self.cached_texture_path = None;
        self.cached_texture = None;

        let Some(project_path) = &engine.project_path else {
            self.loaded_for = None;
            return;
        };
        self.loaded_for = Some(project_path.to_string_lossy().into_owned());
        let Some(dir) = project_path.parent() else {
            return;
        };

        let deck_candidate = dir.join("storyboard/storyboard.deckcraft");
        if deck_candidate.is_file() {
            self.deck_path = Some(deck_candidate);
        }

        let manifest = dir.join("storyboard/movie-board.json");
        if let Ok(bytes) = std::fs::read(&manifest)
            && let Ok(board) = serde_json::from_slice::<Value>(&bytes)
        {
            if let Some(deck_rel) = board.get("deck").and_then(Value::as_str) {
                let resolved = dir.join(deck_rel);
                if resolved.is_file() {
                    self.deck_path = Some(resolved);
                }
            }
            if let Some(arr) = board.get("previews").and_then(Value::as_array) {
                self.previews = arr
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|p| {
                        let raw = PathBuf::from(p);
                        if raw.is_absolute() {
                            raw
                        } else {
                            dir.join(raw)
                        }
                    })
                    .collect();
            }
            if let Some(arr) = board.get("page_titles").and_then(Value::as_array) {
                self.page_titles = arr
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        v.as_str()
                            .filter(|s| !s.trim().is_empty())
                            .map(str::to_owned)
                            .unwrap_or_else(|| format!("Page {}", i + 1))
                    })
                    .collect();
            }
            if let Some(arr) = board.get("page_cues").and_then(Value::as_array) {
                self.page_cues = arr
                    .iter()
                    .map(|cue_list| {
                        cue_list
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|item| item.get("text").and_then(Value::as_str))
                            .map(str::to_owned)
                            .collect()
                    })
                    .collect();
            }
        }

        if self.current_page >= self.previews.len() {
            self.current_page = 0;
        }
    }

    fn page_title(&self, index: usize) -> String {
        self.page_titles
            .get(index)
            .cloned()
            .unwrap_or_else(|| format!("Page {}", index + 1))
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, engine: &mut Session) -> StoryboardAction {
        let current_project = engine
            .project_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned());
        if self.loaded_for != current_project {
            self.reload(engine);
        }

        let mut action = StoryboardAction::None;
        let total = self.previews.len();

        // Keyboard arrow navigation
        if total > 0 {
            if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) && self.current_page > 0 {
                self.current_page -= 1;
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) && self.current_page + 1 < total {
                self.current_page += 1;
            }
        }

        // Toolbar
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    total > 0 && self.current_page > 0,
                    egui::Button::new("◀ Prev"),
                )
                .clicked()
            {
                self.current_page -= 1;
            }
            let page_label = if total > 0 {
                format!(
                    "{} / {} — {}",
                    self.current_page + 1,
                    total,
                    self.page_title(self.current_page)
                )
            } else {
                "No pages rendered".to_string()
            };
            ui.strong(page_label);
            if ui
                .add_enabled(
                    total > 0 && self.current_page + 1 < total,
                    egui::Button::new("Next ▶"),
                )
                .clicked()
            {
                self.current_page += 1;
            }

            ui.separator();

            if ui
                .add_enabled(
                    self.deck_path.is_some(),
                    egui::Button::new("Edit in DeckCraft"),
                )
                .clicked()
                && let Some(deck) = &self.deck_path
            {
                open_os_path(deck);
            }

            if ui
                .add_enabled(
                    self.deck_path.is_some(),
                    egui::Button::new("Re-render Pages (DeckCraft)"),
                )
                .clicked()
            {
                match engine.execute("storyboard.render", json!({})) {
                    Ok(_) => {
                        self.reload(engine);
                        action = StoryboardAction::Status("Storyboard pages rendered.".into());
                    }
                    Err(e) => action = StoryboardAction::Error(e.to_string()),
                }
            }

            if ui.button("Create from Scene Plan (.json)…").clicked()
                && let Some(file) = rfd::FileDialog::new()
                    .set_title("Select Storyboard Scene Plan JSON")
                    .add_filter("Scene Plan JSON", &["json"])
                    .pick_file()
            {
                let plan_path = file.to_string_lossy().into_owned();
                match engine.execute("storyboard.create", json!({ "plan": plan_path })) {
                    Ok(_) => {
                        self.reload(engine);
                        action =
                            StoryboardAction::Status("Storyboard created and rendered.".into());
                    }
                    Err(e) => action = StoryboardAction::Error(e.to_string()),
                }
            }

            if ui.button("Reload").clicked() {
                self.reload(engine);
            }
        });

        ui.separator();

        if total == 0 {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                if self.deck_path.is_some() {
                    ui.heading("Storyboard deck exists, but pages are not rendered yet.");
                    ui.label("Click 'Re-render Pages (DeckCraft)' above or edit in DeckCraft.");
                } else {
                    ui.heading("No Storyboard Created Yet");
                    ui.label(
                        "Create a storyboard using 'Create from Scene Plan (.json)…' above or via `magic-cli storyboard create`.",
                    );
                }
            });
            return action;
        }

        // Split layout: Left rail (page list + lyric cues) and Right canvas (slide preview)
        egui::Panel::left("storyboard_rail")
            .resizable(true)
            .default_size(260.0)
            .show(ui, |ui| {
                ui.heading("Pages");
                egui::ScrollArea::vertical()
                    .id_salt("sb_pages_scroll")
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for i in 0..total {
                            let label = format!("{}. {}", i + 1, self.page_title(i));
                            if ui.selectable_label(self.current_page == i, label).clicked() {
                                self.current_page = i;
                            }
                        }
                    });

                if let Some(cues) = self.page_cues.get(self.current_page)
                    && !cues.is_empty()
                {
                    ui.separator();
                    ui.heading("Scene Lyrics");
                    egui::ScrollArea::vertical()
                        .id_salt("sb_cues_scroll")
                        .show(ui, |ui| {
                            for line in cues {
                                ui.label(line);
                            }
                        });
                }
            });

        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(img_path) = self.previews.get(self.current_page).cloned() {
                self.ensure_texture(ui.ctx(), &img_path);
                if let Some(texture) = &self.cached_texture {
                    let avail = ui.available_size();
                    let tex_size = texture.size_vec2();
                    let scale = (avail.x / tex_size.x).min(avail.y / tex_size.y).min(1.0);
                    let display_size = tex_size * scale;
                    ui.centered_and_justified(|ui| {
                        ui.image((texture.id(), display_size));
                    });
                } else {
                    ui.label(format!(
                        "Could not load preview image: {}",
                        img_path.display()
                    ));
                }
            }
        });

        action
    }

    fn ensure_texture(&mut self, ctx: &egui::Context, path: &Path) {
        if self.cached_texture_path.as_deref() == Some(path) && self.cached_texture.is_some() {
            return;
        }
        self.cached_texture_path = Some(path.to_path_buf());
        self.cached_texture = load_image_texture(ctx, path);
    }
}

fn load_image_texture(ctx: &egui::Context, path: &Path) -> Option<egui::TextureHandle> {
    let img = image::open(path).ok()?.into_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let pixels = img.into_raw();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
    Some(ctx.load_texture(
        path.to_string_lossy(),
        color_image,
        egui::TextureOptions::LINEAR,
    ))
}
