//! Selected-task card: identity, review feedback and explicit role/worktree actions.
use super::widgets::*;
use crate::{MopsAction, data::clean, handoff::Task};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Paragraph},
};

pub(super) fn draw(
    f: &mut Frame,
    area: Rect,
    task: Option<&Task>,
    actions: &mut Vec<(Rect, MopsAction)>,
) {
    let name = field(task, |t| t.name.as_deref());
    let inner = panel(
        f,
        area,
        &format!("SELECTED TASK · {}", clean(name).to_uppercase()),
    );
    portrait(f, inner, task);
    identity(f, inner, task);
    let status_y = inner.y + 4;
    blocker(f, Rect::new(inner.x, status_y, inner.width, 3), task);
    let feedback_y = status_y + 4;
    let role_y = inner.bottom().saturating_sub(8);
    let action_y = role_y.saturating_sub(3);
    feedback(f, inner, task, feedback_y, action_y);
    task_actions(
        f,
        Rect::new(inner.x, action_y, inner.width, 3),
        task,
        actions,
    );
    role_worktree(f, inner, role_y, task, actions);
}
fn portrait(f: &mut Frame, inner: Rect, task: Option<&Task>) {
    let known = task
        .and_then(|t| t.agent.as_deref())
        .is_some_and(known_identity);
    let area = Rect::new(inner.x, inner.y, 6, 4);
    f.render_widget(
        Block::bordered().border_style(Style::default().fg(IVORY)),
        area,
    );
    f.render_widget(
        Paragraph::new(if known { "▟█▙\n▐▝▌" } else { " ? " })
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().fg(if known { GOLD } else { MUTED })),
        Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), 2),
    );
}
fn identity(f: &mut Frame, inner: Rect, task: Option<&Task>) {
    let issue = field(task, |t| t.issue.as_deref());
    let pr = pr_label(task);
    let lines = [
        (0, field(task, |t| t.agent.as_deref()).into(), IVORY, true),
        (
            2,
            format!(
                "Department: {}  │  Issue: {issue}  │  PR: {pr} • {}",
                field(task, |t| t.department.as_deref()),
                field(task, |t| t.state.as_deref()).to_uppercase()
            ),
            IVORY,
            false,
        ),
        (
            3,
            format!(
                "Implementation role: {}",
                field(task, |t| t.implementation_role.as_deref())
            ),
            CYAN,
            false,
        ),
    ];
    for (offset, value, color, bold) in lines {
        text(
            f,
            Rect::new(
                inner.x + 8,
                inner.y + offset,
                inner.width.saturating_sub(8),
                1,
            ),
            value,
            color,
            bold,
        );
    }
}
fn blocker_label(task: Option<&Task>) -> (String, bool) {
    let blocker = task
        .and_then(|t| t.blocker.as_deref())
        .filter(|s| !s.trim().is_empty());
    let action = field(task, |t| t.action.as_deref());
    let label = if let Some(blocker) = blocker {
        format!(
            "{} • {}",
            if normalized(action) == "on hold" {
                "ON HOLD"
            } else {
                "BLOCKER"
            },
            clean(blocker).to_uppercase()
        )
    } else {
        format!("YOUR ACTION • {}", clean(action).to_uppercase())
    };
    (label, blocker.is_some())
}
fn blocker(f: &mut Frame, area: Rect, task: Option<&Task>) {
    let (label, blocked) = blocker_label(task);
    f.render_widget(
        Block::bordered()
            .border_style(Style::default().fg(GOLD))
            .style(Style::default().bg(SELECT)),
        area,
    );
    f.render_widget(
        Paragraph::new(label)
            .alignment(ratatui::layout::Alignment::Center)
            .style(
                Style::default()
                    .fg(if blocked { CORAL } else { GOLD })
                    .add_modifier(Modifier::BOLD),
            ),
        Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), 1),
    );
}
fn feedback(f: &mut Frame, inner: Rect, task: Option<&Task>, title_y: u16, action_y: u16) {
    text(
        f,
        Rect::new(inner.x, title_y, inner.width, 1),
        "REVIEW FEEDBACK",
        GOLD,
        true,
    );
    let feedback = task.map(|t| t.feedback.as_slice()).unwrap_or(&[]);
    let capacity = usize::from(action_y.saturating_sub(title_y + 2));
    if feedback.is_empty() {
        feedback_line(
            f,
            inner,
            title_y + 2,
            task.map(|t| t.review_note.as_str())
                .unwrap_or("Unknown — no review snapshot supplied"),
            MUTED,
        );
    } else {
        for (i, line) in feedback.iter().take(capacity).enumerate() {
            feedback_line(
                f,
                inner,
                title_y + 2 + i as u16,
                &format!("●  {}", clean(line)),
                CORAL,
            );
        }
    }
}
fn feedback_line(f: &mut Frame, inner: Rect, y: u16, value: &str, color: ratatui::style::Color) {
    text(
        f,
        Rect::new(inner.x + 1, y, inner.width.saturating_sub(2), 1),
        value,
        color,
        false,
    );
}
fn task_actions(
    f: &mut Frame,
    area: Rect,
    task: Option<&Task>,
    actions: &mut Vec<(Rect, MopsAction)>,
) {
    let slots = Layout::horizontal([
        Constraint::Length(22),
        Constraint::Length(31),
        Constraint::Min(0),
    ])
    .spacing(2)
    .split(area);
    button(
        f,
        slots[0],
        "Open PR",
        task.is_some_and(|t| !t.pr_url.is_empty()),
        MopsAction::OpenPr,
        actions,
    );
    button(
        f,
        slots[1],
        "View Mayhem",
        task.is_some(),
        MopsAction::ViewMayhem,
        actions,
    );
}
struct RoleLink<'a> {
    value: &'a str,
    enabled: bool,
    action: MopsAction,
    label: &'static str,
}
fn role_links(task: Option<&Task>) -> [RoleLink<'_>; 2] {
    [
        RoleLink {
            value: field(task, |t| t.role_path.as_deref()),
            enabled: task.is_some_and(|t| {
                t.role_available && t.role_path.as_deref().is_some_and(|s| !s.trim().is_empty())
            }),
            action: MopsAction::OpenRole,
            label: "Open role",
        },
        RoleLink {
            value: field(task, |t| t.worktree.as_deref()),
            enabled: task.is_some_and(|t| {
                t.worktree_available && t.worktree.as_deref().is_some_and(|s| !s.trim().is_empty())
            }),
            action: MopsAction::OpenWorktree,
            label: "Open folder",
        },
    ]
}
fn role_worktree(
    f: &mut Frame,
    inner: Rect,
    title_y: u16,
    task: Option<&Task>,
    actions: &mut Vec<(Rect, MopsAction)>,
) {
    rule(f, Rect::new(inner.x, title_y, inner.width, 1));
    text(
        f,
        Rect::new(inner.x, title_y + 1, inner.width, 1),
        "ROLE & WORKTREE",
        GOLD,
        true,
    );
    for (i, link) in role_links(task).into_iter().enumerate() {
        let y = title_y + 3 + i as u16 * 3;
        text(
            f,
            Rect::new(inner.x + 1, y, inner.width.saturating_sub(26), 1),
            format!("▤  {}", link.value),
            IVORY,
            false,
        );
        button(
            f,
            Rect::new(inner.right().saturating_sub(24), y.saturating_sub(1), 24, 3),
            link.label,
            link.enabled,
            link.action,
            actions,
        );
    }
}
