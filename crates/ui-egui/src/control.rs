//! Control-protocol methods beyond `ui.*` state (see `docs/control-protocol.md`), following
//! FilmCraft's `crates/ui-egui/src/control.rs`: synthetic pointer and keyboard input is queued here
//! and handed to egui in [`eframe::App::raw_input_hook`].

use egui::{Context, Event, Modifiers, Pos2};
use serde_json::{Value, json};

#[cfg(not(target_arch = "wasm32"))]
use crate::ViewMode;
use crate::{AssistantTab, MagicApp};

/// Synthetic input is consumed once per frame; bound what a client can queue ahead of that.
const MAX_SYNTHETIC_EVENTS: usize = 16_384;

fn modifiers(p: &Value) -> Modifiers {
    let m = p.get("modifiers").unwrap_or(p);
    let b = |k: &str| m.get(k).and_then(Value::as_bool).unwrap_or(false);
    let primary = b("ctrl") || b("command");
    Modifiers {
        alt: b("alt"),
        shift: b("shift"),
        // Off macOS Control is the primary modifier: a press of either carries both.
        ctrl: primary && !cfg!(target_os = "macos") || b("ctrl"),
        command: primary,
        mac_cmd: cfg!(target_os = "macos") && b("command"),
    }
}

impl MagicApp {
    fn point(&self, p: &Value) -> Result<Pos2, String> {
        if let Some(id) = p.get("id").and_then(Value::as_str) {
            let e = self
                .auto
                .find(id)
                .ok_or_else(|| format!("no element `{id}` (see ui.elements)"))?;
            return Ok(egui::pos2(
                e.rect[0] + e.rect[2] * 0.5,
                e.rect[1] + e.rect[3] * 0.5,
            ));
        }
        let x = p
            .get("x")
            .and_then(Value::as_f64)
            .ok_or("need `id` or `x`,`y`")?;
        let y = p.get("y").and_then(Value::as_f64).ok_or("need `y`")?;
        Ok(egui::pos2(x as f32, y as f32))
    }

    fn synthetic_input(&mut self, method: &str, p: &Value) -> Result<Value, String> {
        if self.synthetic.len() >= MAX_SYNTHETIC_EVENTS {
            return Err(
                "too many synthetic input events are queued; wait for them to be processed".into(),
            );
        }
        match method {
            "ui.move" => {
                let pos = self.point(p)?;
                self.synthetic.push(Event::PointerMoved(pos));
            }
            "ui.scroll" => {
                let pos = self.point(p)?;
                let delta = |k: &str| p.get(k).and_then(Value::as_f64).unwrap_or(0.0) as f32;
                let m = modifiers(p);
                self.synthetic.push(Event::ModifiersChanged(m));
                self.synthetic.push(Event::PointerMoved(pos));
                self.synthetic.push(Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    source: egui::MouseWheelSource::Wheel,
                    delta: egui::vec2(delta("dx"), delta("dy")),
                    modifiers: m,
                    phase: egui::TouchPhase::Move,
                });
                self.synthetic
                    .push(Event::ModifiersChanged(Modifiers::NONE));
            }
            _ => {
                let text = p
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or("missing `text`")?;
                self.synthetic.push(Event::Text(text.to_owned()));
            }
        }
        Ok(json!({ "ok": true, "queued": self.synthetic.len() }))
    }

    /// Answer a documented protocol method, or `None` when `method` is not one of them.
    pub(crate) fn handle_protocol(
        &mut self,
        ctx: &Context,
        method: &str,
        p: &Value,
    ) -> Option<Result<Value, String>> {
        Some(match method {
            "engine.execute" | "ui.menu.invoke" => {
                let command = p["command"].as_str().or_else(|| p["id"].as_str());
                match command {
                    Some(c) if !c.starts_with("engine.") => {
                        let params = p.get("params").cloned().unwrap_or_else(|| json!({}));
                        self.handle_control_request(ctx, c, params)
                    }
                    _ => Err("missing `command` (an engine or ui command id)".into()),
                }
            }
            "engine.commands" | "ui.menu.list" => self
                .session
                .execute("command.list", json!({}))
                .map_err(|e| e.to_string()),
            // Magic's egui views are not docked: report the one layout there is.
            "ui.docking" => Ok(json!({
                "ok": true,
                "layout": {
                    "view": self.view.id(),
                    "assistant": { "open": self.assistant.open, "tab": format!("{:?}", self.assistant.active_tab) },
                },
                "focused": null,
                "maximized": null,
                "hitRects": [],
            })),
            "ui.panel.show" | "ui.panel.close" => {
                let panel = p["panel"].as_str().unwrap_or_default();
                let tab = match panel {
                    "fidget" => AssistantTab::Fidget,
                    "scribbles" => AssistantTab::Scribbles,
                    _ => return Some(Err(format!("unknown panel `{panel}` (fidget, scribbles)"))),
                };
                self.assistant.open = method == "ui.panel.show";
                self.assistant.active_tab = tab;
                Ok(json!({ "ok": true, "panel": panel, "open": self.assistant.open }))
            }
            "ui.move" | "ui.scroll" | "ui.type" => self.synthetic_input(method, p),
            #[cfg(not(target_arch = "wasm32"))]
            "ui.playback" if self.view == ViewMode::Animation => self
                .effectcraft_animation
                .as_mut()
                .ok_or_else(|| "Open an EffectCraft composition first.".to_owned())
                .and_then(|composition| {
                    composition.preview(p["action"].as_str().unwrap_or("state"))
                }),
            #[cfg(not(target_arch = "wasm32"))]
            "ui.playback" if self.view == ViewMode::Studio => self.studio_program_preview(
                p["action"].as_str().unwrap_or("state"), p["seconds"].as_f64(),
            ),
            "ui.playback" => Err(
                "ui.playback: the Magic views have no transport; preview runs through the `video.preview` command"
                    .into(),
            ),
            "ui.resize" => {
                let side = |k: &str| p[k].as_f64().filter(|v| (1.0..=16_384.0).contains(v));
                match (side("width"), side("height")) {
                    (Some(w), Some(h)) => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            w as f32, h as f32,
                        )));
                        Ok(json!({ "ok": true, "width": w, "height": h }))
                    }
                    _ => Err("`width` and `height` must be numbers from 1 to 16384".into()),
                }
            }
            _ => return None,
        })
    }
}
