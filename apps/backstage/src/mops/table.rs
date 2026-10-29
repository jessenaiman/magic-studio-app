//! Table framing, row presentation and selection share one fixed geometry.
use super::widgets::*;
use crate::handoff::Task;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders},
};
const HEADINGS: [&str; 7] = [
    "#",
    "Department",
    "Task",
    "Agent / Harness",
    "Issue / PR",
    "Review",
    "Your action",
];
fn columns(area: Rect) -> Vec<Rect> {
    Layout::horizontal([
        Constraint::Percentage(4),
        Constraint::Percentage(13),
        Constraint::Percentage(17),
        Constraint::Percentage(19),
        Constraint::Percentage(12),
        Constraint::Percentage(18),
        Constraint::Percentage(17),
    ])
    .split(area)
    .to_vec()
}
pub(super) fn draw(
    f: &mut Frame,
    area: Rect,
    tasks: &[Task],
    selected: usize,
    rows: &mut Vec<(Rect, usize)>,
) {
    f.render_widget(
        Block::bordered().border_style(Style::default().fg(LINE)),
        area,
    );
    let inner = Rect::new(
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    header(f, inner);
    if tasks.is_empty() {
        empty(f, inner);
        return;
    }
    let capacity = usize::from(inner.height.saturating_sub(3) / 3).max(1);
    let offset = selected.saturating_sub(capacity.saturating_sub(1));
    for (visible, index) in (offset..tasks.len()).take(capacity).enumerate() {
        let y = inner.y + 4 + visible as u16 * 3;
        let row = Rect::new(inner.x, y.saturating_sub(1), inner.width, 3);
        task_row(f, row, y, &tasks[index], index, index == selected, visible);
        rows.push((row, index));
    }
    if let Some((row, _)) = rows.iter().find(|(_, i)| *i == selected) {
        f.render_widget(
            Block::bordered().border_style(Style::default().fg(GOLD)),
            *row,
        );
    }
}
fn header(f: &mut Frame, inner: Rect) {
    for (column, title) in columns(inner).iter().zip(HEADINGS) {
        text(
            f,
            Rect::new(column.x + 1, inner.y, column.width.saturating_sub(2), 1),
            title,
            IVORY,
            true,
        );
        if column.x > inner.x {
            f.render_widget(
                Block::default()
                    .borders(Borders::LEFT)
                    .border_style(Style::default().fg(LINE)),
                Rect::new(column.x, inner.y, column.width, inner.height),
            );
        }
    }
    rule(f, Rect::new(inner.x, inner.y + 2, inner.width, 1));
}
fn empty(f: &mut Frame, inner: Rect) {
    for (i, column) in columns(inner).iter().enumerate() {
        text(
            f,
            Rect::new(column.x + 1, inner.y + 3, column.width.saturating_sub(2), 1),
            if i == 0 { "—" } else { "Unknown" },
            if i == 6 { GOLD } else { MUTED },
            false,
        );
    }
}
fn row_values(task: &Task, index: usize) -> [String; 6] {
    let issue = task
        .issue
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("Unknown");
    [
        format!("{}", index + 1),
        field(Some(task), |t| t.department.as_deref()).into(),
        field(Some(task), |t| t.name.as_deref()).into(),
        format!(
            "{} • {}",
            field(Some(task), |t| t.agent.as_deref()),
            field(Some(task), |t| t.harness.as_deref())
        ),
        format!("{issue} • {}", pr_label(Some(task))),
        if task.review.is_empty() {
            "Unknown".into()
        } else {
            task.review.clone()
        },
    ]
}
fn row_background(selected: bool, visible: usize) -> Color {
    if selected {
        SELECT
    } else if visible.is_multiple_of(2) {
        Color::Rgb(30, 20, 33)
    } else {
        BG
    }
}
fn task_row(
    f: &mut Frame,
    row: Rect,
    y: u16,
    task: &Task,
    index: usize,
    selected: bool,
    visible: usize,
) {
    f.render_widget(
        Block::default().style(Style::default().bg(row_background(selected, visible))),
        row,
    );
    let columns = columns(row);
    row_cells(f, &columns, y, task, index);
    rule(f, Rect::new(row.x, y + 1, row.width, 1));
    let action = field(Some(task), |t| t.action.as_deref());
    badge(
        f,
        Rect::new(columns[6].x, y, columns[6].width, 1),
        action,
        action_color(action),
    );
}
fn row_cells(f: &mut Frame, columns: &[Rect], y: u16, task: &Task, index: usize) {
    for (i, (column, value)) in columns.iter().zip(row_values(task, index)).enumerate() {
        let color = match i {
            4 => CYAN,
            5 => review_color(&task.review),
            _ => IVORY,
        };
        text(
            f,
            Rect::new(column.x + 1, y, column.width.saturating_sub(2), 1),
            value,
            color,
            false,
        );
    }
}
