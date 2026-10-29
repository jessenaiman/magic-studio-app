//! Session metadata and copy-only handoff controls remain separate from task feedback.
use super::widgets::*;
use crate::{MopsAction, data::clean, handoff::Task};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, Paragraph, Wrap},
};

pub(super) fn draw(
    f: &mut Frame,
    area: Rect,
    task: Option<&Task>,
    can_copy: bool,
    actions: &mut Vec<(Rect, MopsAction)>,
) {
    let inner = panel(f, area, "SESSION HANDOFF");
    fields(f, inner, task);
    let resume_y = inner.y + 4;
    resume(f, inner, resume_y, task, can_copy, actions);
    next_action(f, inner, resume_y + 9, task);
    button(
        f,
        Rect::new(inner.x, inner.bottom().saturating_sub(3), 25, 3),
        "Copy feedback",
        can_copy && task.is_some_and(|t| !t.feedback.is_empty()),
        MopsAction::CopyFeedback,
        actions,
    );
}
fn fields(f: &mut Frame, inner: Rect, task: Option<&Task>) {
    let values = [
        ("Harness:", field(task, |t| t.harness.as_deref())),
        ("Session ID:", field(task, |t| t.session_id.as_deref())),
        ("Branch:", field(task, |t| t.branch.as_deref())),
        (
            "Model / provider:",
            field(task, |t| t.model_provider.as_deref()),
        ),
    ];
    for (i, (label, value)) in values.into_iter().enumerate() {
        text(
            f,
            Rect::new(inner.x, inner.y + i as u16, 21, 1),
            label,
            IVORY,
            false,
        );
        text(
            f,
            Rect::new(
                inner.x + 22,
                inner.y + i as u16,
                inner.width.saturating_sub(22),
                1,
            ),
            value,
            CYAN,
            false,
        );
    }
}
fn resume(
    f: &mut Frame,
    inner: Rect,
    y: u16,
    task: Option<&Task>,
    can_copy: bool,
    actions: &mut Vec<(Rect, MopsAction)>,
) {
    rule(f, Rect::new(inner.x, y, inner.width, 1));
    text(
        f,
        Rect::new(inner.x, y + 1, inner.width, 1),
        "RESUME COMMAND FROM PR",
        GOLD,
        true,
    );
    command_box(
        f,
        Rect::new(inner.x, y + 3, inner.width, 3),
        field(task, |t| t.resume_command.as_deref()),
    );
    let verified = can_copy && resume_verified(task);
    button(
        f,
        Rect::new(inner.x, y + 6, 29, 3),
        "Copy resume command",
        verified,
        MopsAction::CopyResume,
        actions,
    );
    resume_note(
        f,
        Rect::new(inner.x + 31, y + 7, inner.width.saturating_sub(31), 1),
        verified,
    );
}
fn resume_verified(task: Option<&Task>) -> bool {
    task.is_some_and(|t| {
        t.verified_resume
            && t.resume_command
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty())
    })
}
fn command_box(f: &mut Frame, area: Rect, command: &str) {
    f.render_widget(
        Block::bordered().border_style(Style::default().fg(IVORY)),
        area,
    );
    f.render_widget(
        Paragraph::new(clean(command))
            .style(Style::default().fg(IVORY))
            .wrap(Wrap { trim: false }),
        Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), 1),
    );
}
fn resume_note(f: &mut Frame, area: Rect, verified: bool) {
    text(
        f,
        area,
        if verified {
            "Copy only — never executes"
        } else {
            "Unknown / unverified — copy disabled"
        },
        if verified { IVORY } else { MUTED },
        false,
    );
}
fn next_action(f: &mut Frame, inner: Rect, y: u16, task: Option<&Task>) {
    rule(f, Rect::new(inner.x, y, inner.width, 1));
    text(
        f,
        Rect::new(inner.x, y + 1, inner.width, 1),
        "NEXT ACTION",
        GOLD,
        true,
    );
    f.render_widget(
        Paragraph::new(clean(field(task, |t| t.next_action.as_deref())))
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(IVORY)),
        Rect::new(inner.x + 1, y + 3, inner.width.saturating_sub(2), 2),
    );
    warning(
        f,
        Rect::new(inner.x + 1, y + 5, inner.width.saturating_sub(2), 1),
        task,
    );
}
fn warning(f: &mut Frame, area: Rect, task: Option<&Task>) {
    let label = task
        .and_then(|t| t.warnings.first())
        .map(|s| format!("PR warning: {} • d details", clean(s)))
        .unwrap_or_else(|| "You decide when to resume. No harness is launched.".into());
    text(
        f,
        area,
        label,
        if task.is_some_and(|t| !t.warnings.is_empty()) {
            CORAL
        } else {
            IVORY
        },
        false,
    );
}
