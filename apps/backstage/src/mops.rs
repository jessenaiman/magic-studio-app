//! PR-owned handoffs. Actions remain explicit; resume text is copied, never executed.
use crate::{App, MopsAction, widgets::*};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
};
pub fn draw(f: &mut Frame, area: Rect, app: &mut App) {
    let bands = Layout::vertical([
        Constraint::Percentage(30),
        Constraint::Min(4),
        Constraint::Length(2),
    ])
    .split(area);
    let tasks = app
        .mops
        .tasks
        .iter()
        .map(|t| {
            vec![
                format!("#{}", t.pr_number),
                t.name.clone().unwrap_or_else(|| "Unknown".into()),
                t.agent.clone().unwrap_or_else(|| "Unknown".into()),
                t.action.clone().unwrap_or_else(|| t.review.clone()),
            ]
        })
        .collect();
    app.mops_rows = table(
        f,
        bands[0],
        "MOPS • recorded PR handoffs",
        &["PR", "Task", "Agent", "Action"],
        tasks,
        &[10, 45, 20, 25],
        app.mops_selected,
    );
    let task = app.mops.tasks.get(app.mops_selected);
    let content = task
        .map(|t| {
            format!(
                "{}\nAgent: {} · {}\nNext: {}\nBlocker: {}\nRole: {} · {}\nReview: {}",
                t.name.as_deref().unwrap_or("Unknown task"),
                t.agent.as_deref().unwrap_or("Unknown"),
                t.harness.as_deref().unwrap_or("Unknown"),
                t.next_action.as_deref().unwrap_or("Unknown"),
                t.blocker.as_deref().unwrap_or(if t.blocker_known {
                    "None recorded"
                } else {
                    "Unknown"
                }),
                t.department.as_deref().unwrap_or("Unknown"),
                t.implementation_role.as_deref().unwrap_or("Unknown"),
                t.review
            )
        })
        .unwrap_or_else(|| {
            app.mops.error.clone().unwrap_or_else(|| {
                if app.mops_inputs.is_some() {
                    "PR input configured; press r to read."
                } else {
                    "No PR input. Use --mops-repo and --mops-pr or --mops-pr-file."
                }
                .into()
            })
        });
    paragraph(f, bands[1], "Selected handoff • d full record", content, 0);
    let known = |n: Option<usize>| n.map(|n| n.to_string()).unwrap_or_else(|| "Unknown".into());
    let footer = format!(
        "{} · decision {} · blocked {} · {}",
        app.mops
            .source
            .split(" • ")
            .next()
            .unwrap_or("Unknown source"),
        known(app.mops.decisions()),
        known(app.mops.blocked()),
        if app.mops.complete {
            "observed"
        } else {
            "incomplete"
        }
    );
    let actions = [
        ("Enter PR", MopsAction::OpenPr),
        ("c Resume", MopsAction::CopyResume),
        ("f Feedback", MopsAction::CopyFeedback),
        ("p Worktree", MopsAction::OpenWorktree),
        ("l Role", MopsAction::OpenRole),
        ("m Reports", MopsAction::ViewMayhem),
    ];
    let cells = Layout::horizontal([Constraint::Percentage(16); 6]).split(bands[2]);
    text(
        f,
        Rect::new(bands[2].x, bands[2].y + 1, bands[2].width, 1),
        footer,
        MUTED,
        false,
    );
    for (i, (name, action)) in actions.iter().enumerate() {
        text(
            f,
            cells[i],
            *name,
            if task.is_some() { GOLD } else { MUTED },
            false,
        );
        if task.is_some() {
            app.mops_actions.push((cells[i], *action));
        }
    }
}
