//! Magic Studio in the browser (`apps/magic-web`).
//!
//! Mounts [`magic_ui_egui::MagicApp`] on an HTML `<canvas id="the_canvas_id">` via
//! [`eframe::WebRunner`], matching `craft-anything-web` and `filmcraft-web`.

#![cfg(target_arch = "wasm32")]
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unimplemented,
        clippy::todo,
        clippy::unreachable
    )
)]

use magic::Session;
use magic_ui_egui::MagicApp;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub async fn start() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no browser window"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("no document"))?;
    let canvas = document
        .get_element_by_id("the_canvas_id")
        .ok_or_else(|| JsValue::from_str("missing #the_canvas_id"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    let runner = eframe::WebRunner::new();
    runner
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(|cc| {
                magic_ui_egui::theme::apply(&cc.egui_ctx);
                Ok(Box::new(MagicApp::new(Session::default())))
            }),
        )
        .await?;
    std::mem::forget(runner);
    if let Some(loading) = document.get_element_by_id("loading") {
        loading.remove();
    }
    Ok(())
}
