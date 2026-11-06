use std::path::{Path, PathBuf};
use std::time::SystemTime;

use egui::{Context, Ui};
use filmcraft_ui_egui::{FilmcraftApp, dock::PanelKind, panels};
use serde_json::json;

use crate::{
    DialogAction, MagicApp,
    widgets::{empty_state, secondary_button},
};

pub(crate) struct StudioFilmcraft {
    app: FilmcraftApp,
    path: PathBuf,
    modified: Option<SystemTime>,
    first_tick: bool,
}

impl StudioFilmcraft {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut session = filmcraft_engine::Session::default();
        session
            .execute("file.open", json!({ "path": path }))
            .map_err(|error| error.to_string())?;
        Ok(Self {
            app: FilmcraftApp::new(session),
            path: path.into(),
            modified: modified(path),
            first_tick: true,
        })
    }

    pub fn create(path: &Path) -> Result<Self, String> {
        if path.exists() {
            return Self::open(path);
        }
        let mut session = filmcraft_engine::Session::default();
        let name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("Film");
        let parent = path
            .parent()
            .ok_or("FilmCraft project has no parent directory")?;
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        for (id, params) in [
            ("file.newProject", json!({ "name": name })),
            (
                "file.newSequence",
                json!({ "name": name, "width": 1920, "height": 1080, "fps": 30 }),
            ),
            ("file.saveAs", json!({ "path": path })),
        ] {
            session
                .execute(id, params)
                .map_err(|error| error.to_string())?;
        }
        Self::open(path)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn is_dirty(&self) -> bool {
        self.app.session.is_dirty()
    }

    pub fn changed_on_disk(&self) -> bool {
        self.modified != modified(&self.path)
    }

    pub fn save(&mut self) -> Result<(), String> {
        if self.changed_on_disk() {
            return Err("FilmCraft project changed on disk. Reopen it before saving.".into());
        }
        self.app
            .session
            .execute("file.save", json!({}))
            .map_err(|error| error.to_string())?;
        self.modified = modified(&self.path);
        Ok(())
    }

    pub fn import_animation(&mut self, clip: &Path) -> Result<(), String> {
        if self.is_dirty() {
            return Err("Save the FilmCraft project before adding an animation.".into());
        }
        if self.changed_on_disk() {
            return Err("FilmCraft project changed on disk. Reopen it before importing.".into());
        }
        let clip = clip.canonicalize().map_err(|error| error.to_string())?;
        let imported = self
            .app
            .session
            .execute("file.import", json!({ "paths": [clip] }))
            .map_err(|error| error.to_string())?;
        if imported["items"].as_array().is_none_or(Vec::is_empty) {
            return Err(format!("FilmCraft did not import {}.", clip.display()));
        }
        self.save()
    }

    pub fn tick(&mut self, ctx: &Context) {
        let magic_style = self.first_tick.then(|| {
            let theme = ctx.theme();
            (theme, ctx.style_of(theme))
        });
        self.app.logic_inner(ctx);
        if let Some((theme, style)) = magic_style {
            ctx.set_style_of(theme, style);
            self.first_tick = false;
        }
    }

    pub fn show(&mut self, ui: &mut Ui, panel: PanelKind) {
        let rect = ui.available_rect_before_wrap();
        ui.set_min_size(rect.size());
        panels::show(&mut self.app, ui, panel, rect);
    }

    pub fn show_dialogs(&mut self, ctx: &Context) {
        panels::dialogs::show(&mut self.app, ctx);
    }
}

impl Drop for StudioFilmcraft {
    fn drop(&mut self) {
        eframe::App::on_exit(&mut self.app);
    }
}

impl MagicApp {
    fn studio_film_path(&self) -> Option<PathBuf> {
        self.film_state["timeline"]["path"]
            .as_str()
            .map(PathBuf::from)
    }

    fn load_studio_film(&mut self, path: &Path, create: bool) -> Result<(), String> {
        if let Some(current) = self.studio_film.as_ref() {
            if current.path() == path && !current.changed_on_disk() {
                return Ok(());
            }
            if current.is_dirty() {
                return Err(
                    "Save the open FilmCraft project before switching or reloading.".into(),
                );
            }
        }
        self.studio_film = Some(if create {
            StudioFilmcraft::create(path)?
        } else {
            StudioFilmcraft::open(path)?
        });
        self.studio_open_error = None;
        Ok(())
    }

    pub(crate) fn import_studio_animation(&mut self, clip: &Path) -> Result<String, String> {
        if !clip.is_file() {
            return Err(format!("Animation file does not exist: {}", clip.display()));
        }
        let film = self
            .studio_film_path()
            .ok_or("Open a Magic project first.")?;
        self.load_studio_film(&film, true)?;
        let document = self
            .studio_film
            .as_mut()
            .ok_or("No FilmCraft project open.")?;
        document.import_animation(clip)?;
        self.refresh_studio();
        Ok(format!(
            "Added {} to FilmCraft.",
            clip.file_name().unwrap_or_default().to_string_lossy()
        ))
    }

    pub(crate) fn set_studio_status(&mut self, result: Result<String, String>) {
        self.status_line = match result {
            Ok(message) => {
                self.last_error = None;
                message
            }
            Err(error) => {
                self.last_error = Some(error.clone());
                error
            }
        };
    }

    fn studio_film_toolbar(&mut self, ui: &mut Ui, path: &Path) {
        ui.horizontal_wrapped(|ui| {
            if !path.is_file() && secondary_button(ui, "Create FilmCraft Project").clicked() {
                let result = self
                    .load_studio_film(path, true)
                    .map(|()| "Created FilmCraft project.".into());
                self.set_studio_status(result);
                self.refresh_studio();
            }
            if self.studio_open_error.is_some() && secondary_button(ui, "Reload").clicked() {
                self.studio_open_error = None;
            }
            if let Some(document) = self.studio_film.as_mut()
                && document.is_dirty()
                && secondary_button(ui, "Save FilmCraft Project").clicked()
            {
                let result = document.save().map(|()| "Saved FilmCraft project.".into());
                self.set_studio_status(result);
            }
            if secondary_button(ui, "Import Rendered Animation…").clicked() {
                self.dialogs.ask(
                    craft_dialogs::FileDialog::open()
                        .title("Import Rendered Animation")
                        .filter("Video", &["mov", "mp4", "mkv", "webm"]),
                    DialogAction::ImportStudioAnimation,
                );
            }
            if let Some((project, clip)) = self.studio_rendered_animation.clone()
                && self.session.project_path.as_ref() == Some(&project)
                && secondary_button(ui, "Add Last Animation Render").clicked()
            {
                let result = self.import_studio_animation(&clip);
                self.set_studio_status(result);
            }
            ui.label(path.display().to_string());
        });
    }

    pub(crate) fn show_studio_view(&mut self, ctx: &Context, ui: &mut Ui) {
        if self.session.project_path.is_none() {
            empty_state(
                ui,
                "Open a Project First",
                "Open a .magic project to edit its film.",
            );
            return;
        }
        let Some(path) = self.studio_film_path() else {
            empty_state(
                ui,
                "FilmCraft Project Unavailable",
                "Magic could not resolve this project's film path.",
            );
            return;
        };
        if path.is_file()
            && self
                .studio_open_error
                .as_ref()
                .is_none_or(|(failed, _)| failed != &path)
            && let Err(error) = self.load_studio_film(&path, false)
        {
            self.studio_open_error = Some((path.clone(), error));
        }
        self.studio_film_toolbar(ui, &path);
        if let Some((failed, error)) = self.studio_open_error.as_ref()
            && failed == &path
        {
            ui.label(error);
        }
        let Some(document) = self
            .studio_film
            .as_mut()
            .filter(|document| document.path() == path)
        else {
            empty_state(
                ui,
                "Open the FilmCraft Project",
                "Create the project's film or import an animation to get started.",
            );
            return;
        };
        document.tick(ctx);
        egui::Panel::bottom("studio_filmcraft_timeline")
            .resizable(true)
            .default_size(300.0)
            .min_size(240.0)
            .show(ui, |ui| document.show(ui, PanelKind::Timeline));
        egui::Panel::left("studio_filmcraft_project")
            .resizable(true)
            .default_size(240.0)
            .min_size(180.0)
            .show(ui, |ui| document.show(ui, PanelKind::Project));
        egui::CentralPanel::default().show(ui, |ui| document.show(ui, PanelKind::Program));
        document.show_dialogs(ctx);
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    path.metadata()
        .and_then(|metadata| metadata.modified())
        .ok()
}
