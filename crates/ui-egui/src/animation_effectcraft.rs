use std::path::Path;

use effectcraft_ui_egui::{EffectcraftApp, dock::PanelKind, gpu_failure::GpuFailureBridge, panels};
use egui::{Context, Ui};
use serde_json::{Value, json};

use crate::MagicApp;

pub(crate) struct AnimationEffectcraft {
    app: EffectcraftApp,
    preview_configured: bool,
}

impl AnimationEffectcraft {
    pub fn open(path: &Path) -> Result<Self, String> {
        let path = path.canonicalize().map_err(|error| error.to_string())?;
        let mut session = effectcraft_host::session();
        session
            .execute_checked("file.open", json!({ "path": path }))
            .map_err(|error| error.to_string())?;
        Ok(Self {
            app: EffectcraftApp::new(session),
            preview_configured: false,
        })
    }

    pub fn is_dirty(&self) -> bool {
        self.app.session.is_dirty()
    }

    pub fn name(&self) -> String {
        panels::unsaved::project_name(&self.app)
    }

    fn path(&self) -> Option<&str> {
        self.app.session.path.as_deref()
    }

    pub fn seek(&mut self, seconds: f64) -> Result<(), String> {
        self.app
            .session
            .execute_checked("time.set", json!({"time": seconds}))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn studio_render_request(&self) -> Result<Value, String> {
        if self.is_dirty() {
            return Err("Save the EffectCraft project before rendering to Studio.".into());
        }
        let session = &self.app.session;
        let path = session
            .path
            .as_ref()
            .ok_or("EffectCraft project has no file path.")?;
        let id = session
            .active_comp_id()
            .ok_or("Open an EffectCraft composition first.")?;
        let comp = session
            .active_comp()
            .ok_or("Open an EffectCraft composition first.")?;
        Ok(json!({
            "effectcraft_project": path,
            "comp": id,
            "start": comp.work_area.0.seconds(),
            "end": comp.work_area.1.seconds(),
            "format": "clip",
        }))
    }

    pub fn save(&mut self) -> Result<(), String> {
        self.app
            .session
            .execute_checked("file.save", json!({}))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn tick(&mut self, ctx: &Context, frame: &eframe::Frame) {
        if !self.preview_configured {
            self.preview_configured = true;
            if frame
                .wgpu_render_state()
                .is_some_and(|state| state.adapter.get_info().vendor == 0x10de)
            {
                // The embedded EffectCraft frame worker can hold this device through shutdown.
                let fallback = GpuFailureBridge::new(ctx);
                fallback.report(
                    "Use CPU compositing for the embedded preview on NVIDIA.",
                    false,
                );
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
    pub(crate) fn animation_workspace_command(
        &mut self,
        ctx: &Context,
        params: &Value,
    ) -> Result<Value, String> {
        let project = self
            .session
            .project_path
            .as_deref()
            .ok_or("Open a Magic project first.")?;
        let action = params["action"].as_str().unwrap_or("state");
        if action == "state" {
            return Ok(json!({
                "project": project, "character": self.selected_character, "view": self.selected_view,
                "composition": self.effectcraft_animation.as_ref().and_then(AnimationEffectcraft::path),
                "revision": self.animation_revision, "artwork_request": self.animation_artwork_request,
                "busy": self.busy_command, "error": self.last_error,
            }));
        }
        if params["project"].as_str() != project.to_str() {
            return Err("The open Magic project changed.".into());
        }
        let character_params =
            json!({"character": self.selected_character, "view": self.selected_view});
        match action {
            "catalog" => {
                return Ok(
                    json!({"characters": self.characters, "assets": self.session.project_assets}),
                );
            }
            "inspector" => {
                let metadata = if self.selected_character.is_empty() {
                    Value::Null
                } else {
                    self.execute_session_command("character.meta", character_params)?
                };
                let options =
                    self.execute_session_command("animation.motion_options", json!({}))?;
                return Ok(json!({"metadata": metadata, "options": options["options"]}));
            }
            _ => {}
        }
        if self.busy_command.is_some() {
            return Err("Wait for the current operation to finish.".into());
        }
        match action {
            "select" => {
                let character = params["character"].as_str().ok_or("Choose a character.")?;
                if !self
                    .characters
                    .iter()
                    .any(|entry| entry["id"].as_str() == Some(character))
                {
                    return Err("Choose a character from this project.".into());
                }
                character.clone_into(&mut self.selected_character);
            }
            "asset" => {
                self.select_animation_asset(params["id"].as_str().ok_or("Choose an asset.")?)?
            }
            "view" => {
                let view = params["view"]
                    .as_str()
                    .filter(|v| matches!(*v, "front" | "side" | "back"))
                    .ok_or("Choose front, side, or back.")?;
                view.clone_into(&mut self.selected_view);
            }
            "prepare-artwork" => {
                self.animation_artwork_request = self.animation_artwork_request.wrapping_add(1)
            }
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
                if self
                    .effectcraft_animation
                    .as_ref()
                    .is_some_and(AnimationEffectcraft::is_dirty)
                {
                    return Err("Save the open EffectCraft composition first.".into());
                }
                let metadata =
                    self.execute_session_command("character.meta", character_params.clone())?;
                let current = metadata["views"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|view| view["view"].as_str() == Some(self.selected_view.as_str()))
                    .and_then(|view| view["revision"].as_u64());
                if current.is_none() || current != params["revision"].as_u64() {
                    return Err(
                        "Artwork marks changed on disk. Reopen Animation to load them.".into(),
                    );
                }
                let mut command = character_params;
                command["parts"] = params["parts"].clone();
                let result = self.execute_session_command("character.save_marks", command)?;
                self.handle_command_success("character.save_marks", result);
            }
            "render-to-studio" => self.send_active_animation_to_studio(ctx)?,
            "mark" | "cut-rig" | "key-action" | "prepare-motion" => {
                if self
                    .effectcraft_animation
                    .as_ref()
                    .is_some_and(AnimationEffectcraft::is_dirty)
                {
                    return Err("Save the open EffectCraft composition first.".into());
                }
                let mut command = character_params;
                match action {
                    "mark" => {
                        let character = params["character"]
                            .as_str()
                            .map(str::trim)
                            .filter(|name| !name.is_empty())
                            .ok_or("Enter a character name.")?;
                        command["character"] = json!(character);
                        command["image"] = params["image"].clone();
                        magic::workspace::sanitize_asset_id(character)
                            .clone_into(&mut self.selected_character);
                        self.run_command(ctx, "character.mark", command);
                    }
                    "cut-rig" => self.start_command_worker(
                        ctx,
                        "character.cut",
                        command,
                        Some("character.rig"),
                    ),
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
    fn project_close_keeps_dirty_composition_and_clears_clean_composition() {
        let mut app = MagicApp::new(magic::Session::default());
        let project = Path::new("animation-test.magic");
        app.session.project_path = Some(project.into());
        let mut effect = effectcraft_host::session();
        effect.execute_checked("comp.new", json!({})).unwrap();
        app.effectcraft_animation = Some(AnimationEffectcraft {
            app: EffectcraftApp::new(effect),
            preview_configured: false,
        });
        let ctx = Context::default();
        let error = app
            .handle_control_request(&ctx, "project.close", json!({}))
            .unwrap_err();
        assert!(error.contains("Save the open EffectCraft composition"));
        assert_eq!(app.session.project_path.as_deref(), Some(project));

        app.effectcraft_animation = Some(AnimationEffectcraft {
            app: EffectcraftApp::new(effectcraft_host::session()),
            preview_configured: false,
        });
        app.handle_control_request(&ctx, "project.close", json!({}))
            .unwrap();
        assert!(app.effectcraft_animation.is_none());
        assert!(app.session.project_path.is_none());
    }
}
