//! PR-recorded task handoff. Rendering never contacts a harness or executes a resume command.
mod cast;
mod selected;
mod session;
mod table;
mod widgets;
use crate::App;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    widgets::{Paragraph, Wrap},
};
use widgets::*;

pub fn draw(f: &mut Frame, area: Rect, app: &mut App) {
    let mut actions = Vec::new();
    let mut rows = Vec::new();
    if area.width < 120 || area.height < 44 {
        let body = panel(f, area, "MOPS • TASK HANDOFF");
        f.render_widget(Paragraph::new("Enlarge the terminal for the task table and handoff panels.\nUnknown values are not inferred. Resume is copy-only; no harness is contacted.").wrap(Wrap{trim:false}).style(Style::default().fg(IVORY)),body);
        app.mops_rows = rows;
        app.mops_actions = actions;
        return;
    }
    let bands = Layout::vertical([
        Constraint::Percentage(36),
        Constraint::Min(27),
        Constraint::Length(2),
    ])
    .spacing(1)
    .split(area);
    table::draw(f, bands[0], &app.mops.tasks, app.mops_selected, &mut rows);
    let lower = Layout::horizontal([Constraint::Percentage(51), Constraint::Percentage(49)])
        .spacing(1)
        .split(bands[1]);
    let task = app.mops.tasks.get(app.mops_selected);
    selected::draw(f, lower[0], task, &mut actions);
    session::draw(f, lower[1], task, app.clipboard_supported, &mut actions);
    cast::draw(
        f,
        bands[2],
        &app.mops.tasks,
        task,
        &app.mops.source,
        app.mops.error.as_deref(),
    );
    app.mops_rows = rows;
    app.mops_actions = actions;
}
