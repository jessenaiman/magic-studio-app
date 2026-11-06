//! Reusable Craft pro-app widgets and vector icons for Magic Studio (`magic-ui-egui`).

use egui::{Color32, CornerRadius, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui};

use crate::theme::{
    ACCENT, ACCENT_TEAL, BG_ACTIVE, BG_ELEVATED, BG_HOVER, BG_INPUT, BORDER_STRONG, BORDER_SUBTLE,
    TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};

/// Uppercase collapsible section header with vector chevron indicator.
pub fn section_header(ui: &mut Ui, title: &str, default_open: bool) -> bool {
    let id = ui.make_persistent_id(("magic_sec", title));
    let mut open = ui
        .ctx()
        .data_mut(|d| *d.get_persisted_mut_or(id, default_open));

    let desired = egui::vec2(ui.available_width(), 24.0);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
    if resp.clicked() {
        open = !open;
        ui.ctx().data_mut(|d| d.insert_persisted(id, open));
    }

    let bg = if resp.hovered() {
        BG_HOVER
    } else {
        BG_ELEVATED
    };
    ui.painter().rect_filled(rect, CornerRadius::same(4), bg);

    // Draw chevron with Painter line primitives
    let cx = rect.left() + 11.0;
    let cy = rect.center().y;
    let stroke = Stroke::new(1.5, TEXT_SECONDARY);
    if open {
        ui.painter().line_segment(
            [Pos2::new(cx - 3.5, cy - 1.5), Pos2::new(cx, cy + 2.5)],
            stroke,
        );
        ui.painter().line_segment(
            [Pos2::new(cx, cy + 2.5), Pos2::new(cx + 3.5, cy - 1.5)],
            stroke,
        );
    } else {
        ui.painter().line_segment(
            [Pos2::new(cx - 1.5, cy - 3.5), Pos2::new(cx + 2.5, cy)],
            stroke,
        );
        ui.painter().line_segment(
            [Pos2::new(cx + 2.5, cy), Pos2::new(cx - 1.5, cy + 3.5)],
            stroke,
        );
    }

    ui.painter().text(
        Pos2::new(rect.left() + 22.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        title.to_uppercase(),
        egui::FontId::proportional(11.0),
        TEXT_SECONDARY,
    );

    open
}

/// Aligned 2-column property row (`label` on left, control on right).
pub fn prop_row<R>(ui: &mut Ui, label: &str, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(88.0, 22.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.label(RichText::new(label).color(TEXT_SECONDARY).size(11.5));
            },
        );
        ui.with_layout(
            egui::Layout::right_to_left(egui::Align::Center),
            add_contents,
        )
        .inner
    })
    .inner
}

/// Scrubbable hot-number DragValue with cyan underline when hovered or dragged.
pub fn hot_number(
    ui: &mut Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    speed: f64,
    suffix: &str,
) -> Response {
    let resp = ui.add(
        egui::DragValue::new(value)
            .range(range)
            .speed(speed)
            .suffix(suffix)
            .max_decimals(2),
    );
    if resp.hovered() || resp.dragged() {
        let y = resp.rect.bottom() - 1.0;
        ui.painter().line_segment(
            [
                Pos2::new(resp.rect.left() + 2.0, y),
                Pos2::new(resp.rect.right() - 2.0, y),
            ],
            Stroke::new(1.5, ACCENT_TEAL),
        );
    }
    resp
}

/// Compact pill toggle button used for mode switchers and drawers.
pub fn pill_toggle(ui: &mut Ui, selected: bool, label: &str, accent: Color32) -> Response {
    let text = RichText::new(label).size(12.0).color(if selected {
        Color32::WHITE
    } else {
        TEXT_PRIMARY
    });
    let btn = egui::Button::new(text)
        .fill(if selected {
            accent.gamma_multiply(0.35)
        } else {
            BG_INPUT
        })
        .stroke(Stroke::new(
            1.0,
            if selected { accent } else { BORDER_SUBTLE },
        ))
        .corner_radius(CornerRadius::same(6));
    ui.add(btn)
}

/// Primary Teal action button (`magic-ui` skill: one primary teal action per surface).
pub fn primary_button(ui: &mut Ui, label: &str) -> Response {
    let text = RichText::new(label)
        .size(12.0)
        .strong()
        .color(Color32::from_rgb(8, 18, 18));
    ui.add(
        egui::Button::new(text)
            .fill(ACCENT_TEAL)
            .stroke(Stroke::new(1.0, ACCENT_TEAL))
            .corner_radius(CornerRadius::same(6)),
    )
}

/// Secondary elevated button.
pub fn secondary_button(ui: &mut Ui, label: &str) -> Response {
    let text = RichText::new(label).size(11.5).color(TEXT_PRIMARY);
    ui.add(
        egui::Button::new(text)
            .fill(BG_ACTIVE)
            .stroke(Stroke::new(1.0, BORDER_STRONG))
            .corner_radius(CornerRadius::same(5)),
    )
}

/// Small status pill badge.
pub fn status_badge(ui: &mut Ui, label: &str, color: Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.18))
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.75)))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(label).size(10.5).color(color).strong());
        });
}

/// Paint a crisp 4-point Magic Studio star/sparkle vector icon using `egui::Painter`.
pub fn draw_spark_icon(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.42;
    let inner = r * 0.32;
    let pts = vec![
        Pos2::new(c.x, c.y - r),
        Pos2::new(c.x + inner, c.y - inner),
        Pos2::new(c.x + r, c.y),
        Pos2::new(c.x + inner, c.y + inner),
        Pos2::new(c.x, c.y + r),
        Pos2::new(c.x - inner, c.y + inner),
        Pos2::new(c.x - r, c.y),
        Pos2::new(c.x - inner, c.y - inner),
    ];
    painter.add(egui::Shape::convex_polygon(
        pts,
        color,
        Stroke::new(1.0, color),
    ));
}

/// Framed card container with subtle border and hover elevation.
pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(BG_ELEVATED)
        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
        .corner_radius(CornerRadius::same(7))
        .inner_margin(egui::Margin::same(10))
}

/// Empty state card with muted title and hint.
pub fn empty_state(ui: &mut Ui, title: &str, subtitle: &str) {
    card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.vertical_centered(|ui| {
            ui.add_space(10.0);
            let (icon_rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), Sense::hover());
            draw_spark_icon(ui.painter(), icon_rect, ACCENT);
            ui.add_space(4.0);
            ui.label(RichText::new(title).size(13.0).strong().color(TEXT_PRIMARY));
            ui.label(RichText::new(subtitle).size(11.5).color(TEXT_MUTED));
            ui.add_space(10.0);
        });
    });
}

/// Draw a subtle 1px border around a preview rect.
pub fn frame_outline(painter: &egui::Painter, rect: Rect, color: Color32) {
    painter.rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0, color),
        StrokeKind::Inside,
    );
}
