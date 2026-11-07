use std::path::Path;
use std::time::SystemTime;

use effectcraft_ui_egui::{EffectcraftApp, dock::PanelKind, gpu_failure::GpuFailureBridge, panels};
use egui::{Context, Ui};
use serde_json::{Value, json};

use crate::MagicApp;

pub(crate) struct AnimationEffectcraft {
    app: EffectcraftApp,
    preview_configured: bool,
    modified: Option<SystemTime>,
}

fn modified(path: &Path) -> Option<SystemTime> {
    path.metadata().and_then(|meta| meta.modified()).ok()
}

impl AnimationEffectcraft {
    pub fn open(path: &Path) -> Result<Self, String> {
        let path = path.canonicalize().map_err(|error| error.to_string())?;
        let mut session = effectcraft_host::session();
        session.execute_checked("file.open", json!({ "path": path })).map_err(|error| error.to_string())?;
        Ok(Self { app: EffectcraftApp::new(session), preview_configured: false, modified: modified(&path) })
    }

    pub fn is_dirty(&self) -> bool {
        self.app.session.is_dirty()
    }

    pub fn name(&self) -> String {
        panels::unsaved::project_name(&self.app)
    }

    pub fn duration(&self) -> Option<f64> {
        self.app.session.active_comp().map(|comp| comp.duration.seconds())
    }

    pub fn set_duration(&mut self, seconds: f64) -> Result<f64, String> {
        if !seconds.is_finite() || seconds <= 0.0 || seconds > 86_400.0 {
            return Err("Enter a duration between one frame and 24 hours.".into());
        }
        self.stop_preview();
        self.edit("comp.settings", json!({"duration": seconds}))?;
        self.duration().ok_or_else(|| "Open an EffectCraft composition first.".into())
    }

    fn edit(&mut self, command: &str, params: Value) -> Result<Value, String> {
        self.app.session.execute_checked(command, params).map_err(|error| error.to_string())
    }

    fn path(&self) -> Option<&str> {
        self.app.session.path.as_deref()
    }

    pub fn seek(&mut self, seconds: f64) -> Result<(), String> {
        self.app.session.execute_checked("time.set", json!({"time": seconds})).map(|_| ()).map_err(|error| error.to_string())
    }

    pub fn is_playing(&self) -> bool {
        self.app.playback.playing
    }

    pub fn stop_preview(&mut self) {
        self.app.stop();
    }

    pub fn preview(&mut self, action: &str) -> Result<Value, String> {
        if self.app.session.active_comp_id().is_none() {
            return Err("Open an EffectCraft composition first.".into());
        }
        let was_playing = self.is_playing();
        let play = match action {
            "state" => was_playing,
            "play" => true,
            "stop" => false,
            "toggle" => !was_playing,
            _ => return Err("Choose play, stop, toggle, or state.".into()),
        };
        match (was_playing, play) {
            (false, true) => self.app.play(self.app.now()),
            (true, false) => self.stop_preview(),
            _ => {}
        }
        Ok(json!({ "playing": self.is_playing(), "time": self.app.session.time().seconds() }))
    }

    pub fn studio_render_request(&self) -> Result<Value, String> {
        if self.is_dirty() {
            return Err("Save the EffectCraft project before rendering to Studio.".into());
        }
        let session = &self.app.session;
        let path = session.path.as_ref().ok_or("EffectCraft project has no file path.")?;
        let id = session.active_comp_id().ok_or("Open an EffectCraft composition first.")?;
        let comp = session.active_comp().ok_or("Open an EffectCraft composition first.")?;
        Ok(json!({
            "effectcraft_project": path,
            "comp": id,
            "start": comp.work_area.0.seconds(),
            "end": comp.work_area.1.seconds(),
            "format": "clip",
        }))
    }

    pub fn save(&mut self) -> Result<(), String> {
        if self.path().and_then(|path| modified(Path::new(path))) != self.modified {
            return Err("Animation changed on disk. Save as a new file, or reopen it before editing.".into());
        }
        self.edit("file.save", json!({}))?;
        self.modified = self.path().and_then(|path| modified(Path::new(path)));
        Ok(())
    }

    pub fn save_as(&mut self, path: &Path) -> Result<(), String> {
        if !path.is_absolute() || !path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("ecproj")) {
            return Err("Choose an absolute .ecproj path.".into());
        }
        if path.exists() {
            return Err("Choose a new filename, or use Save to update the open project.".into());
        }
        self.edit("file.saveAs", json!({"path": path}))?;
        self.modified = modified(path);
        Ok(())
    }

    pub fn tick(&mut self, ctx: &Context, frame: &eframe::Frame) {
        if !self.preview_configured {
            self.preview_configured = true;
            if frame.wgpu_render_state().is_some_and(|state| state.adapter.get_info().vendor == 0x10de) {
                // The embedded EffectCraft frame worker can hold this device through shutdown.
                let fallback = GpuFailureBridge::new(ctx);
                fallback.report("Use CPU compositing for the embedded preview on NVIDIA.", false);
                self.app.set_gpu_failure_bridge(fallback);
            }
        }
        let previous_title = self.app.window_title().to_owned();
        self.app.tick(ctx, frame);
        if self.app.window_title() != previous_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title("Magic Studio".into()));
        }
    }

    pub fn show(&mut self, ui: &mut Ui, panel: PanelKind) {
        let rect = ui.available_rect_before_wrap();
        panels::show(&mut self.app, ui, panel, rect);
    }

    pub fn show_dialogs(&mut self, ctx: &Context) {
        panels::dialogs::show(&mut self.app, ctx);
    }
}

impl Drop for AnimationEffectcraft {
    fn drop(&mut self) {
        eframe::App::on_exit(&mut self.app);
    }
}

impl MagicApp {
    pub(crate) fn animation_workspace_command(&mut self, ctx: &Context, params: &Value) -> Result<Value, String> {
        let project = self.session.project_path.as_deref().ok_or("Open a Magic project first.")?;
        let action = params["action"].as_str().unwrap_or("state");
        if action == "state" {
            return Ok(json!({
                "project": project, "character": self.selected_character, "view": self.selected_view,
                "composition": self.effectcraft_animation.as_ref().and_then(AnimationEffectcraft::path),
                "duration": self.effectcraft_animation.as_ref().and_then(AnimationEffectcraft::duration),
                "dirty": self.effectcraft_animation.as_ref().is_some_and(AnimationEffectcraft::is_dirty),
                "revision": self.animation_revision, "artwork_request": self.animation_artwork_request,
                "busy": self.busy_command, "error": self.last_error,
            }));
        }
        if params["project"].as_str() != project.to_str() {
            return Err("The open Magic project changed.".into());
        }
        let character_params = json!({"character": self.selected_character, "view": self.selected_view});
        match action {
            "catalog" => {
                return Ok(json!({"characters": self.characters, "assets": self.session.project_assets}));
            }
            "inspector" => {
                let metadata = if self.selected_character.is_empty() { Value::Null } else { self.execute_session_command("character.meta", character_params)? };
                let options = self.execute_session_command("animation.motion_options", json!({}))?;
                return Ok(json!({"metadata": metadata, "options": options["options"]}));
            }
            _ => {}
        }
        if self.busy_command.is_some() {
            return Err("Wait for the current operation to finish.".into());
        }
        if action == "open" {
            let path = params["path"].as_str().ok_or("Choose an EffectCraft project.")?;
            let message = self.open_effectcraft_composition(Path::new(path))?;
            self.set_status_result(Ok(message));
            return Ok(json!({"ok": true, "composition": path}));
        }
        if matches!(action, "duration" | "edit" | "save" | "save-as") {
            let composition = self.effectcraft_animation.as_mut().ok_or("Open an EffectCraft composition first.")?;
            let result = match action {
                "duration" => json!(composition.set_duration(params["seconds"].as_f64().ok_or("Enter a duration in seconds.")?)?),
                "edit" => {
                    let command = params["command"]
                        .as_str()
                        .filter(|command| ["comp.", "layer.", "prop.", "time.", "mask.", "effect."].iter().any(|prefix| command.starts_with(prefix)))
                        .ok_or("Choose an EffectCraft composition, layer, property, time, mask, or effect command.")?;
                    composition.stop_preview();
                    composition.edit(command, params.get("params").cloned().unwrap_or_else(|| json!({})))?
                }
                "save" => {
                    composition.save()?;
                    Value::Null
                }
                _ => {
                    let path = params["path"].as_str().ok_or("Choose a new EffectCraft project path.")?;
                    composition.save_as(Path::new(path))?;
                    json!(path)
                }
            };
            return Ok(json!({"ok":true,"result":result,"composition":composition.path(),"dirty":composition.is_dirty()}));
        }
        match action {
            "select" => {
                let character = params["character"].as_str().ok_or("Choose a character.")?;
                if !self.characters.iter().any(|entry| entry["id"].as_str() == Some(character)) {
                    return Err("Choose a character from this project.".into());
                }
                character.clone_into(&mut self.selected_character);
            }
            "asset" => self.select_animation_asset(params["id"].as_str().ok_or("Choose an asset.")?)?,
            "view" => {
                let view = params["view"].as_str().filter(|v| matches!(*v, "front" | "side" | "back")).ok_or("Choose front, side, or back.")?;
                view.clone_into(&mut self.selected_view);
            }
            "prepare-artwork" => self.animation_artwork_request = self.animation_artwork_request.wrapping_add(1),
            "rig" | "rest" => {
                let metadata = self.execute_session_command("character.meta", character_params)?;
                let path = metadata["views"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|view| view["view"].as_str() == Some(self.selected_view.as_str()))
                    .and_then(|view| view["ecproj"].as_str())
                    .ok_or("Rig this character view first.")?;
                let seconds = (action == "rest").then_some(0.0);
                let message = self.open_effectcraft_composition_at(Path::new(path), seconds)?;
                self.set_status_result(Ok(message));
            }
            "open-craft" => {
                let mut command = character_params;
                command["app"] = params["app"].clone();
                self.execute_session_command("character.open", command)?;
            }
            "save-marks" => {
                if self.effectcraft_animation.as_ref().is_some_and(AnimationEffectcraft::is_dirty) {
                    return Err("Save the open EffectCraft composition first.".into());
                }
                let metadata = self.execute_session_command("character.meta", character_params.clone())?;
                let current = metadata["views"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|view| view["view"].as_str() == Some(self.selected_view.as_str()))
                    .and_then(|view| view["revision"].as_u64());
                if current.is_none() || current != params["revision"].as_u64() {
                    return Err("Artwork marks changed on disk. Reopen Animation to load them.".into());
                }
                let mut command = character_params;
                command["parts"] = params["parts"].clone();
                let result = self.execute_session_command("character.save_marks", command)?;
                self.handle_command_success("character.save_marks", result);
            }
            "render-to-studio" => self.send_active_animation_to_studio(ctx)?,
            "mark" | "cut-rig" | "key-action" | "prepare-motion" => {
                if self.effectcraft_animation.as_ref().is_some_and(AnimationEffectcraft::is_dirty) {
                    return Err("Save the open EffectCraft composition first.".into());
                }
                let mut command = character_params;
                match action {
                    "mark" => {
                        let character = params["character"].as_str().map(str::trim).filter(|name| !name.is_empty()).ok_or("Enter a character name.")?;
                        command["character"] = json!(character);
                        command["image"] = params["image"].clone();
                        magic::workspace::sanitize_asset_id(character).clone_into(&mut self.selected_character);
                        self.run_command(ctx, "character.mark", command);
                    }
                    "cut-rig" => self.start_command_worker(ctx, "character.cut", command, Some("character.rig")),
                    "key-action" => {
                        command["action"] = params["name"].clone();
                        self.run_command(ctx, "character.action", command);
                    }
                    _ => self.run_command(
                        ctx,
                        "animation.prepare_motion",
                        json!({
                            "character": self.selected_character, "clip": params["clip"],
                            "option": params["option"], "duration": params["duration"],
                        }),
                    ),
                }
            }
            _ => return Err(format!("Unknown Animation action `{action}`.")),
        }
        Ok(json!({"ok": true, "busy": self.busy_command}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_control_edits_the_open_animation_session() {
        let mut app = MagicApp::new(magic::Session::default());
        app.view = crate::ViewMode::Animation;
        app.session.project_path = Some("animation-test.magic".into());
        let mut effect = effectcraft_host::session();
        effect.execute_checked("comp.new", json!({"duration": 2.0, "frameRate": 30})).unwrap();
        app.effectcraft_animation = Some(AnimationEffectcraft { app: EffectcraftApp::new(effect), preview_configured: false, modified: None });
        let ctx = Context::default();
        app.handle_control_request(
            &ctx,
            "ui.animation.controls",
            json!({
                "action": "duration", "project": "animation-test.magic", "seconds": 3.0
            }),
        )
        .unwrap();
        let info = app.handle_control_request(&ctx, "ui.animation.controls", json!({"action":"state"})).unwrap();
        assert_eq!(info["duration"], 3.0);
        assert_eq!(info["dirty"], true);
    }

    #[test]
    fn animation_playback_controls_the_embedded_preview() {
        let mut app = MagicApp::new(magic::Session::default());
        app.view = crate::ViewMode::Animation;
        let ctx = Context::default();
        let playback = |app: &mut MagicApp, action| app.handle_control_request(&ctx, "ui.playback", json!({ "action": action }));
        assert!(playback(&mut app, "play").is_err());

        let mut effect = effectcraft_host::session();
        effect.execute_checked("comp.new", json!({})).unwrap();
        app.effectcraft_animation = Some(AnimationEffectcraft { app: EffectcraftApp::new(effect), preview_configured: false, modified: None });

        assert_eq!(playback(&mut app, "play").unwrap()["playing"], true);
        assert_eq!(playback(&mut app, "play").unwrap()["playing"], true);
        assert!(playback(&mut app, "invalid").is_err());
        assert_eq!(playback(&mut app, "stop").unwrap()["playing"], false);
        assert_eq!(playback(&mut app, "toggle").unwrap()["playing"], true);
        assert_eq!(playback(&mut app, "toggle").unwrap()["playing"], false);
    }

    #[test]
    fn project_close_keeps_dirty_composition_and_clears_clean_composition() {
        let mut app = MagicApp::new(magic::Session::default());
        let project = Path::new("animation-test.magic");
        app.session.project_path = Some(project.into());
        let mut effect = effectcraft_host::session();
        effect.execute_checked("comp.new", json!({})).unwrap();
        app.effectcraft_animation = Some(AnimationEffectcraft { app: EffectcraftApp::new(effect), preview_configured: false, modified: None });
        let ctx = Context::default();
        let error = app.handle_control_request(&ctx, "project.close", json!({})).unwrap_err();
        assert!(error.contains("Save the open EffectCraft composition"));
        assert_eq!(app.session.project_path.as_deref(), Some(project));

        app.effectcraft_animation =
            Some(AnimationEffectcraft { app: EffectcraftApp::new(effectcraft_host::session()), preview_configured: false, modified: None });
        app.handle_control_request(&ctx, "project.close", json!({})).unwrap();
        assert!(app.effectcraft_animation.is_none());
        assert!(app.session.project_path.is_none());
    }
}
