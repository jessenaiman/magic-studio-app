//! Automation registry: every interactive element registers a stable id, its screen rect, and
//! optional current value each frame, so agents can click/inspect by id (`ui.click`, `ui.elements`,
//! `ui.unconfigured`, `ui.state`) with minimal code.
//!
//! Adapted from FilmCraft's `crates/ui-egui/src/automation.rs`.

use egui::Rect;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Element {
    pub id: String,
    pub label: String,
    pub rect: [f32; 4],
    #[serde(rename = "widgetId", skip_serializing_if = "Option::is_none")]
    pub widget_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focusable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip)]
    pub(crate) response_id: Option<egui::Id>,
}

#[derive(Default)]
pub struct Registry {
    pub elements: Vec<Element>,
    /// Last frame's elements (for lookups from control handlers between frames).
    pub previous: Vec<Element>,
}

impl Registry {
    pub fn begin_frame(&mut self) {
        self.previous = std::mem::take(&mut self.elements);
    }

    pub fn add(&mut self, id: &str, rect: Rect, label: &str) {
        self.elements.push(Element {
            id: id.to_string(),
            label: label.to_string(),
            rect: [rect.min.x, rect.min.y, rect.width(), rect.height()],
            widget_id: None,
            enabled: None,
            focusable: None,
            value: None,
            response_id: None,
        });
    }

    pub fn add_response(&mut self, id: &str, response: &egui::Response, label: &str) {
        self.add(id, response.rect, label);
        if let Some(element) = self.elements.last_mut() {
            element.widget_id = Some(format!("{:016x}", response.id.value()));
            element.enabled = Some(response.enabled());
            element.focusable = Some(response.sense.is_focusable());
            element.response_id = Some(response.id);
        }
    }

    pub fn add_field(&mut self, id: &str, response: &egui::Response, label: &str, value: &str) {
        self.add_response(id, response, label);
        if let Some(element) = self.elements.last_mut() {
            element.value = Some(value.to_string());
        }
    }

    pub fn all(&self) -> &[Element] {
        if self.elements.is_empty() {
            &self.previous
        } else {
            &self.elements
        }
    }

    pub fn list(&self) -> &[Element] {
        self.all()
    }

    pub fn focus(&self, ctx: &egui::Context, id: &str) -> bool {
        let Some(el) = self.find(id) else {
            return false;
        };
        if let Some(rid) = el.response_id {
            ctx.memory_mut(|m| m.request_focus(rid));
            return true;
        }
        false
    }

    pub fn find(&self, id: &str) -> Option<&Element> {
        self.all().iter().find(|e| e.id == id)
    }

    pub fn query(&self, prefix: &str) -> Vec<&Element> {
        self.all()
            .iter()
            .filter(|e| prefix.is_empty() || e.id.starts_with(prefix))
            .collect()
    }

    /// Fields registered in the current view whose value is empty (`""`).
    pub fn unconfigured(&self) -> Vec<&Element> {
        self.all()
            .iter()
            .filter(|e| e.value.as_deref().is_some_and(|v| v.trim().is_empty()))
            .collect()
    }
}
