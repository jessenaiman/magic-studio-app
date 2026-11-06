use std::path::Path;

use effectcraft_ui_egui::{EffectcraftApp, dock::PanelKind, panels};
use egui::{Context, Ui};
use serde_json::json;

pub(crate) struct AnimationEffectcraft(EffectcraftApp);

impl AnimationEffectcraft {
    pub fn open(path: &Path) -> Result<Self, String> {
        let path = path.canonicalize().map_err(|error| error.to_string())?;
        let mut session = effectcraft_host::session();
        session
            .execute_checked("file.open", json!({ "path": path }))
            .map_err(|error| error.to_string())?;
        Ok(Self(EffectcraftApp::new(session)))
    }

    pub fn is_dirty(&self) -> bool {
        self.0.session.is_dirty()
    }

    pub fn name(&self) -> String {
        panels::unsaved::project_name(&self.0)
    }

    pub fn save(&mut self) -> Result<(), String> {
        self.0
            .session
            .execute_checked("file.save", json!({}))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn tick(&mut self, ctx: &Context, frame: &eframe::Frame) {
        let previous_title = self.0.window_title().to_owned();
        self.0.tick(ctx, frame);
        if self.0.window_title() != previous_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title("Magic Studio".into()));
        }
    }

    pub fn show(&mut self, ui: &mut Ui, panel: PanelKind) {
        let rect = ui.available_rect_before_wrap();
        panels::show(&mut self.0, ui, panel, rect);
    }

    pub fn show_dialogs(&mut self, ctx: &Context) {
        panels::dialogs::show(&mut self.0, ctx);
    }
}
