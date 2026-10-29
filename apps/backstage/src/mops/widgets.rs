//! Shared presentation primitives for the PR handoff view. No IO or state ownership.
use crate::{MopsAction, data::clean, handoff::Task};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph},
};
pub(super) const BG: Color = Color::Rgb(24, 15, 27);
pub(super) const IVORY: Color = Color::Rgb(242, 222, 188);
pub(super) const GOLD: Color = Color::Rgb(245, 186, 56);
pub(super) const LINE: Color = Color::Rgb(176, 126, 125);
pub(super) const CYAN: Color = Color::Rgb(133, 210, 222);
pub(super) const GREEN: Color = Color::Rgb(80, 205, 139);
pub(super) const CORAL: Color = Color::Rgb(245, 112, 98);
pub(super) const MUTED: Color = Color::Rgb(160, 149, 169);
pub(super) const SELECT: Color = Color::Rgb(65, 44, 34);
pub(super) fn text(f: &mut Frame, area: Rect, value: impl Into<String>, color: Color, bold: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let value = clip(
        &clean(&value.into()),
        usize::from(area.width.saturating_sub(1)),
    );
    let style = Style::default().fg(color);
    f.render_widget(
        Paragraph::new(value).style(if bold {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        }),
        area,
    );
}
pub(super) fn clip(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if Span::raw(value).width() <= width {
        return value.into();
    }
    let mut result = String::new();
    let mut used = 0;
    for ch in value.chars() {
        let cell = Span::raw(ch.to_string()).width();
        if used + cell > width - 1 {
            break;
        }
        result.push(ch);
        used += cell;
    }
    result.push('…');
    result
}

pub(super) fn rule(f: &mut Frame, area: Rect) {
    f.render_widget(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(LINE)),
        area,
    );
}
pub(super) fn panel(f: &mut Frame, area: Rect, title: &str) -> Rect {
    f.render_widget(
        Block::bordered()
            .border_style(Style::default().fg(LINE))
            .style(Style::default().bg(BG)),
        area,
    );
    text(
        f,
        Rect::new(area.x + 2, area.y + 1, area.width.saturating_sub(4), 1),
        title,
        GOLD,
        true,
    );
    Rect::new(
        area.x + 2,
        area.y + 3,
        area.width.saturating_sub(4),
        area.height.saturating_sub(4),
    )
}
pub(super) fn field<'a>(
    task: Option<&'a Task>,
    get: impl FnOnce(&'a Task) -> Option<&'a str>,
) -> &'a str {
    task.and_then(get)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Unknown")
}
pub(super) fn normalized(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('_', " ")
}
pub(super) fn review_color(review: &str) -> Color {
    match normalized(review).as_str() {
        "changes requested" | "blocked" => CORAL,
        "approved" | "reviewed" => GREEN,
        "awaiting review" | "review required" | "pending" => GOLD,
        "draft" => CYAN,
        "unknown" | "dismissed" => MUTED,
        _ => IVORY,
    }
}
pub(super) fn action_color(action: &str) -> Color {
    match normalized(action).as_str() {
        "needs decision" => GOLD,
        "on hold" => CORAL,
        "in production" => CYAN,
        "pending" | "unknown" => MUTED,
        _ => IVORY,
    }
}

pub(super) fn button(
    f: &mut Frame,
    area: Rect,
    label: &str,
    enabled: bool,
    action: MopsAction,
    actions: &mut Vec<(Rect, MopsAction)>,
) {
    let color = if enabled { GOLD } else { MUTED };
    f.render_widget(
        Block::bordered().border_style(Style::default().fg(color)),
        area,
    );
    f.render_widget(
        Paragraph::new(format!("[ {label} ]"))
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().fg(color)),
        Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), 1),
    );
    if enabled {
        actions.push((area, action));
    }
}
pub(super) fn badge(f: &mut Frame, area: Rect, value: &str, color: Color) {
    let outline = Rect::new(
        area.x + 2,
        area.y.saturating_sub(1),
        area.width.saturating_sub(4),
        3,
    );
    f.render_widget(
        Block::bordered().border_style(Style::default().fg(color)),
        outline,
    );
    let label = clip(&clean(value), usize::from(outline.width.saturating_sub(4)));
    f.render_widget(
        Paragraph::new(label)
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().fg(color)),
        Rect::new(outline.x + 1, area.y, outline.width.saturating_sub(2), 1),
    );
}

pub(super) fn known_identity(value: &str) -> bool {
    !value.trim().is_empty()
        && !matches!(
            normalized(value).as_str(),
            "unknown" | "unavailable" | "unassigned"
        )
}
pub(super) fn pr_label(task: Option<&Task>) -> String {
    task.filter(|t| t.pr_number > 0)
        .map(|t| format!("#{}", t.pr_number))
        .unwrap_or_else(|| "Unknown".into())
}
