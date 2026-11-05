//! Native egui theming for Magic — ported from the Tauri design system
//! (`apps/src-tauri/ui/style.css`, `global.css`). All styling is done in Rust;
//! no CSS, no Tauri, no npm, no Bun.
//!
//! Tokens follow magic-studio-timeline.figcraft. Apply once at app startup
//! via `apply_theme(ctx)`. Component helpers reproduce the shared panel/field/
//! toggle/dial layouts from global.css.

use eframe::egui::{self, Theme};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Design tokens (ported from style.css :root)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Background.
const BG: egui::Color32 = egui::Color32::from_rgb(11, 16, 23);        // --bg #0b1017
const SURFACE: egui::Color32 = egui::Color32::from_rgb(20, 28, 40);  // --surface #141c28
const SURFACE_2: egui::Color32 = egui::Color32::from_rgb(28, 42, 59); // --surface-2 #1c2a3b
const LINE: egui::Color32 = egui::Color32::from_rgb(43, 59, 80);     // --line #2b3b50

/// Text.
const TEXT: egui::Color32 = egui::Color32::from_rgb(233, 238, 246);  // --text #e9eef6
const MUTED: egui::Color32 = egui::Color32::from_rgb(156, 175, 198); // --muted #9cafc6
const FAINT: egui::Color32 = egui::Color32::from_rgb(102, 116, 135); // --faint #667487

/// Accent.
const ACCENT: egui::Color32 = egui::Color32::from_rgb(53, 225, 192); // --accent #35e1c0
const ACCENT_INK: egui::Color32 = egui::Color32::from_rgb(11, 16, 23); // --accent-ink #0b1017

/// Workflow stage tones.
const TONE_SONG: egui::Color32 = ACCENT;         // --tone-song #35e1c0
const TONE_LYRICS: egui::Color32 = egui::Color32::from_rgb(255, 208, 120); // --tone-lyrics #ffd078
const TONE_TRANSCRIPT: egui::Color32 = egui::Color32::from_rgb(141, 191, 231); // --tone-transcript #8dbfe7
const TONE_ANIMATE: egui::Color32 = egui::Color32::from_rgb(186, 165, 235); // --tone-animate #baa5eb
const TONE_ASSETS: egui::Color32 = egui::Color32::from_rgb(244, 160, 123); // --tone-assets #f4a07b

/// Status.
const AMBER: egui::Color32 = egui::Color32::from_rgb(240, 168, 60);    // --amber #f0a83c
const RUST: egui::Color32 = egui::Color32::from_rgb(208, 100, 74);     // --rust #d0644a
const CHECK_GREEN: egui::Color32 = egui::Color32::from_rgb(128, 218, 154); // --check-green #80da9a
const DANGER: egui::Color32 = egui::Color32::from_rgb(224, 82, 74);    // --danger #e0524a
const INFO: egui::Color32 = egui::Color32::from_rgb(110, 168, 255);    // --info #6ea8ff

/// Signal indicators.
const SIGNAL_0: egui::Color32 = FAINT;   // --signal-0
const SIGNAL_1: egui::Color32 = ACCENT;  // --signal-1
const SIGNAL_2: egui::Color32 = AMBER;   // --signal-2
const SIGNAL_3: egui::Color32 = DANGER;  // --signal-3

/// Sizes.
const RADIUS: f32 = 8.0;      // --radius 8px
const RADIUS_SM: f32 = 6.0;   // --radius-sm 6px

/// Spacing.
const SPACE_1: f32 = 4.0;   // --space-1 4px
const SPACE_2: f32 = 8.0;   // --space-2 8px
const SPACE_3: f32 = 12.0;  // --space-3 12px
const SPACE_4: f32 = 16.0;  // --space-4 16px
const SPACE_5: f32 = 28.0;  // --space-5 28px

/// Typography.
const TEXT_XS: f32 = 10.0; // --text-xs 10px
const TEXT_SM: f32 = 11.0; // --text-sm 11px
const TEXT_MD: f32 = 13.0; // --text-md 13px
const TEXT_LG: f32 = 15.0; // --text-lg 15px
const TEXT_XL: f32 = 18.0; // --text-xl 18px

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Theme application
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Apply the Magic design system to an egui context.
pub fn apply_theme(ctx: &egui::Context) {
    // --- Visuals (colors) ---
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = SURFACE;
    visuals.window_fill = BG;
    visuals.selection = egui::style::Selection {
        bg_fill: ACCENT.gamma_multiply(0.25),
        stroke: egui::Stroke::new(1.0, ACCENT),
    };
    visuals.window_stroke = egui::Stroke::new(1.0, LINE);

    // Widget rounding — set on each widget state
    let rounded = egui::CornerRadius::same(RADIUS as u8);
    let rounded_sm = egui::CornerRadius::same(RADIUS_SM as u8);
    visuals.widgets.noninteractive.bg_fill = SURFACE_2;
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT);
    visuals.widgets.noninteractive.corner_radius = rounded_sm;
    visuals.widgets.inactive.bg_fill = SURFACE_2;
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.corner_radius = rounded;
    visuals.widgets.hovered.bg_fill = ACCENT.gamma_multiply(0.1);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, TEXT);
    visuals.widgets.hovered.corner_radius = rounded;
    visuals.widgets.active.bg_fill = ACCENT.gamma_multiply(0.2);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, ACCENT_INK);
    visuals.widgets.active.corner_radius = rounded;
    visuals.widgets.open.bg_fill = ACCENT.gamma_multiply(0.15);
    visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0, ACCENT_INK);
    visuals.widgets.open.corner_radius = rounded;
    ctx.set_visuals(visuals);

    // --- Style (spacing, text styles) ---
    ctx.style_mut_of(Theme::Dark, |style| {
        style.spacing.item_spacing = egui::vec2(SPACE_2, SPACE_1);
        style.spacing.button_padding = egui::vec2(SPACE_3, SPACE_1);
        style.spacing.menu_margin = egui::vec2(SPACE_1, SPACE_1).into();
        style.spacing.indent = SPACE_3;

        let mut text_styles = style.text_styles.clone();
        text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::new(TEXT_XL, egui::FontFamily::Proportional),
        );
        text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(TEXT_MD, egui::FontFamily::Proportional),
        );
        text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::new(TEXT_SM, egui::FontFamily::Monospace),
        );
        text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(TEXT_MD, egui::FontFamily::Proportional),
        );
        text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::new(TEXT_SM, egui::FontFamily::Proportional),
        );
        style.text_styles = text_styles;
    });
}

/// Tone color for a workflow stage.
pub fn stage_tone(stage: &str) -> egui::Color32 {
    match stage {
        "song" => TONE_SONG,
        "lyrics" => TONE_LYRICS,
        "transcript" => TONE_TRANSCRIPT,
        "animate" => TONE_ANIMATE,
        "assets" => TONE_ASSETS,
        _ => ACCENT,
    }
}

/// Signal dot color.
pub fn signal_color(level: u8) -> egui::Color32 {
    match level {
        0 => SIGNAL_0,
        1 => SIGNAL_1,
        2 => SIGNAL_2,
        _ => SIGNAL_3,
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Component helpers (ported from global.css)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Inspector field row — shared property-row layout (label + value).
pub fn inspector_field(ui: &mut egui::Ui, label: &str, add_control: impl FnOnce(&mut egui::Ui)) {
    ui.columns(2, |cols| {
        let (label_col, control_col) = cols.split_at_mut(1);
        let label_col = &mut label_col[0];
        let control_col = &mut control_col[0];
        label_col.set_min_width(0.0);
        control_col.set_min_width(0.0);

        label_col.spacing_mut().item_spacing = egui::vec2(SPACE_2, SPACE_1);
        control_col.spacing_mut().item_spacing = egui::vec2(SPACE_2, SPACE_1);

        label_col.label(
            egui::RichText::new(label)
                .size(TEXT_SM)
                .color(MUTED)
                .family(egui::FontFamily::Proportional),
        );

        control_col.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            add_control(ui);
        });
    });
    ui.separator();
}

/// Craft dial — circular coordinate control (ported from .craft-dial).
pub fn craft_dial(
    ui: &mut egui::Ui,
    tone: egui::Color32,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    speed: f32,
) -> bool {
    let mut changed = false;
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new(label)
                .size(TEXT_XS)
                .color(MUTED)
                .family(egui::FontFamily::Proportional),
        );
        ui.add_space(SPACE_1);

        // Outer circle frame
        let (rect, response) = ui.allocate_exact_size(egui::vec2(48.0, 48.0), egui::Sense::hover());
        ui.painter()
            .circle_stroke(rect.center(), 22.0, egui::Stroke::new(2.0, LINE));
        ui.painter().circle_filled(rect.center(), 1.0, SURFACE_2);

        // Value readout
        ui.add_space(SPACE_1);
        ui.label(
            egui::RichText::new(format!("{value:.1}"))
                .size(TEXT_SM)
                .color(tone)
                .family(egui::FontFamily::Monospace),
        );

        // Drag to change
        if response.hovered() {
            let delta = ui.ctx().input(|i| i.pointer.delta());
            *value = (*value + delta.y * speed * 0.01).clamp(*range.start(), *range.end());
            changed = true;
        }
    });
    changed
}

/// Craft toggle / switch (ported from .craft-switch).
pub fn craft_switch(ui: &mut egui::Ui, tone: egui::Color32, label: &str, on: &mut bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label)
                .size(TEXT_SM)
                .color(MUTED)
                .family(egui::FontFamily::Proportional),
        );

        let (rect, response) = ui.allocate_exact_size(egui::vec2(32.0, 18.0), egui::Sense::click());
        if response.clicked() {
            *on = !*on;
            changed = true;
        }

        let bg = if *on {
            tone.gamma_multiply(0.2)
        } else {
            BG
        };
        let border = if *on { tone } else { LINE };
        ui.painter()
            .rect_filled(rect, 9.0, bg);
        ui.painter()
            .rect_stroke(rect, 9.0, egui::Stroke::new(1.0, border), egui::StrokeKind::Middle);

        let knob_x = if *on { rect.left() + 14.0 } else { rect.left() + 2.0 };
        let knob_rect = egui::Rect::from_min_size(
            egui::pos2(knob_x, rect.top() + 2.0),
            egui::vec2(12.0, 12.0),
        );
        ui.painter()
            .circle_filled(knob_rect.center(), 6.0, if *on { tone } else { MUTED });
    });
    changed
}

/// Status row — signal dot + label.
pub fn status_row(ui: &mut egui::Ui, level: u8, label: &str) {
    ui.horizontal(|ui| {
        let color = signal_color(level);
        let center = ui.cursor().min + egui::vec2(4.0, 4.0);
        ui.painter().circle_filled(center, 4.0, color);
        ui.add_space(SPACE_1);
        ui.label(
            egui::RichText::new(label)
                .size(TEXT_SM)
                .color(TEXT)
                .family(egui::FontFamily::Proportional),
        );
    });
    ui.add_space(SPACE_1);
}

/// Section header — title + meta with separator.
pub fn section_header(ui: &mut egui::Ui, title: &str, meta: &str) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(title)
                .strong()
                .size(TEXT_SM)
                .color(TEXT)
                .family(egui::FontFamily::Proportional),
        );
        ui.label(
            egui::RichText::new(meta)
                .size(TEXT_XS)
                .color(MUTED)
                .family(egui::FontFamily::Proportional),
        );
    });
    ui.separator();
    ui.add_space(SPACE_1);
}

/// Prop row — label left, control right.
pub fn prop_row(ui: &mut egui::Ui, label: &str, add_control: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label)
                .size(TEXT_SM)
                .color(MUTED)
                .family(egui::FontFamily::Proportional),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            add_control(ui);
        });
    });
}

/// Number field with readout and range slider.
pub fn number_field(
    ui: &mut egui::Ui,
    tone: egui::Color32,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) -> bool {
    let mut changed = false;
    ui.columns(2, |cols| {
        let (label_col, control_col) = cols.split_at_mut(1);
        let label_col = &mut label_col[0];
        let control_col = &mut control_col[0];
        label_col.set_min_width(0.0);
        control_col.set_min_width(0.0);

        label_col.label(
            egui::RichText::new(label)
                .size(TEXT_SM)
                .color(MUTED)
                .family(egui::FontFamily::Proportional),
        );

        control_col.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!("{value:.1}"))
                    .size(TEXT_SM)
                    .color(tone)
                    .family(egui::FontFamily::Monospace),
            );
            ui.add_space(SPACE_1);
            if ui
                .add(egui::Slider::new(value, range).show_value(false))
                .changed()
            {
                changed = true;
            }
        });
    });
    changed
}
