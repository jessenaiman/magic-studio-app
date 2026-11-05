//! Craft dark studio palette and typography tokens for Magic Studio (`magic-ui-egui`).

use egui::{Color32, Context, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals};

pub const BG_DEEP: Color32 = Color32::from_rgb(10, 11, 14);
pub const BG_PANEL: Color32 = Color32::from_rgb(17, 19, 25);
pub const BG_ELEVATED: Color32 = Color32::from_rgb(23, 26, 34);
pub const BG_INPUT: Color32 = Color32::from_rgb(29, 33, 43);
pub const BG_HOVER: Color32 = Color32::from_rgb(36, 41, 54);
pub const BG_ACTIVE: Color32 = Color32::from_rgb(43, 50, 66);

/// Magic Studio primary teal action color (`magic-ui` skill) paired with Craft Cobalt.
pub const ACCENT_TEAL: Color32 = Color32::from_rgb(56, 198, 180);
pub const ACCENT: Color32 = Color32::from_rgb(59, 130, 246);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(37, 99, 235);
pub const ACCENT_GLOW: Color32 = Color32::from_rgba_premultiplied(56, 198, 180, 42);
pub const ACCENT_AMBER: Color32 = Color32::from_rgb(245, 158, 11);
pub const ACCENT_VIOLET: Color32 = Color32::from_rgb(139, 92, 246);
pub const ACCENT_GREEN: Color32 = Color32::from_rgb(34, 197, 94);
pub const ACCENT_RED: Color32 = Color32::from_rgb(239, 68, 68);

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(236, 239, 244);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(154, 163, 181);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(94, 103, 120);
pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(34, 38, 50);
pub const BORDER_STRONG: Color32 = Color32::from_rgb(47, 53, 69);

pub fn apply(ctx: &Context) {
    let mut style = (*ctx.global_style()).clone();
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(15.5, FontFamily::Proportional),
    );
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(12.5, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(12.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(11.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(11.5, FontFamily::Monospace),
    );

    style.spacing.item_spacing = egui::vec2(6.0, 5.0);
    style.spacing.button_padding = egui::vec2(9.0, 5.0);
    style.spacing.interact_size = egui::vec2(36.0, 24.0);

    let mut v = Visuals::dark();
    craft_ui::theme::WidgetPalette {
        noninteractive: BG_PANEL,
        inactive: BG_INPUT,
        hovered: BG_HOVER,
        active: BG_ACTIVE,
        open: BG_HOVER,
    }
    .apply_to(
        &mut v.widgets,
        CornerRadius::same(5),
        Stroke::new(1.0, TEXT_PRIMARY),
    );
    v.panel_fill = BG_PANEL;
    v.window_fill = BG_ELEVATED;
    v.faint_bg_color = BG_DEEP;
    v.extreme_bg_color = BG_DEEP;
    v.override_text_color = Some(TEXT_PRIMARY);
    v.window_corner_radius = CornerRadius::same(8);
    v.menu_corner_radius = CornerRadius::same(6);
    v.window_stroke = Stroke::new(1.0, BORDER_STRONG);

    v.widgets.noninteractive.bg_fill = BG_PANEL;
    v.widgets.noninteractive.weak_bg_fill = BG_ELEVATED;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER_SUBTLE);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_SECONDARY);
    v.widgets.noninteractive.corner_radius = CornerRadius::same(5);

    v.widgets.inactive.bg_fill = BG_INPUT;
    v.widgets.inactive.weak_bg_fill = BG_INPUT;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER_SUBTLE);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    v.widgets.inactive.corner_radius = CornerRadius::same(5);

    v.widgets.hovered.bg_fill = BG_HOVER;
    v.widgets.hovered.weak_bg_fill = BG_HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_TEAL);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    v.widgets.hovered.corner_radius = CornerRadius::same(5);

    v.widgets.active.bg_fill = BG_ACTIVE;
    v.widgets.active.weak_bg_fill = ACCENT_SOFT;
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT_TEAL);
    v.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    v.widgets.active.corner_radius = CornerRadius::same(5);

    v.selection.bg_fill = ACCENT_TEAL.gamma_multiply(0.32);
    v.selection.stroke = Stroke::new(1.0, ACCENT_TEAL);

    style.visuals = v;
    ctx.set_global_style(style);
}
