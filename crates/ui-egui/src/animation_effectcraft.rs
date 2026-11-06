use std::path::Path;

use effectcraft_ui_egui::{EffectcraftApp, dock::PanelKind, gpu_failure::GpuFailureBridge, panels};
use egui::{Context, Ui};
use serde_json::{Value, json};

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
