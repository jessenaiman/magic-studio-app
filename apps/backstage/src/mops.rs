//! PR-recorded task handoff. Rendering never contacts a harness or executes a resume command.
use crate::{App, MopsAction, data::clean, handoff::Task};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph, Wrap},
};
const BG: Color = Color::Rgb(24, 15, 27);
const IVORY: Color = Color::Rgb(242, 222, 188);
const GOLD: Color = Color::Rgb(245, 186, 56);
const LINE: Color = Color::Rgb(176, 126, 125);
const CYAN: Color = Color::Rgb(133, 210, 222);
const GREEN: Color = Color::Rgb(80, 205, 139);
const CORAL: Color = Color::Rgb(245, 112, 98);
const MUTED: Color = Color::Rgb(160, 149, 169);
const SELECT: Color = Color::Rgb(65, 44, 34);
fn text(f: &mut Frame, area: Rect, value: impl Into<String>, color: Color, bold: bool) {
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
fn clip(value: &str, width: usize) -> String {
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

fn rule(f: &mut Frame, area: Rect) {
    f.render_widget(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(LINE)),
        area,
    );
}
fn panel(f: &mut Frame, area: Rect, title: &str) -> Rect {
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
fn field<'a>(task: Option<&'a Task>, get: impl FnOnce(&'a Task) -> Option<&'a str>) -> &'a str {
    task.and_then(get)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Unknown")
}
fn normalized(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('_', " ")
}
fn review_color(review: &str) -> Color {
    match normalized(review).as_str() {
        "changes requested" | "blocked" => CORAL,
        "approved" | "reviewed" => GREEN,
        "awaiting review" | "review required" | "pending" => GOLD,
        "draft" => CYAN,
        "unknown" | "dismissed" => MUTED,
        _ => IVORY,
    }
}
fn action_color(action: &str) -> Color {
    match normalized(action).as_str() {
        "needs decision" => GOLD,
        "on hold" => CORAL,
        "in production" => CYAN,
        "pending" | "unknown" => MUTED,
        _ => IVORY,
    }
}

fn button(
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
fn badge(f: &mut Frame, area: Rect, value: &str, color: Color) {
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
    table(f, bands[0], &app.mops.tasks, app.mops_selected, &mut rows);
    let lower = Layout::horizontal([Constraint::Percentage(51), Constraint::Percentage(49)])
        .spacing(1)
        .split(bands[1]);
    let task = app.mops.tasks.get(app.mops_selected);
    selected(f, lower[0], task, &mut actions);
    handoff(f, lower[1], task, app.clipboard_supported, &mut actions);
    cast(
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
fn table(
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
    let cols = columns(inner);
    for (column, title) in cols.iter().zip([
        "#",
        "Department",
        "Task",
        "Agent / Harness",
        "Issue / PR",
        "Review",
        "Your action",
    ]) {
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
    if tasks.is_empty() {
        for (i, column) in cols.iter().enumerate() {
            text(
                f,
                Rect::new(column.x + 1, inner.y + 3, column.width.saturating_sub(2), 1),
                if i == 0 { "—" } else { "Unknown" },
                if i == 6 { GOLD } else { MUTED },
                false,
            );
        }
        return;
    }
    let capacity = usize::from(inner.height.saturating_sub(3) / 3).max(1);
    let offset = selected.saturating_sub(capacity.saturating_sub(1));
    for (visible, index) in (offset..tasks.len()).take(capacity).enumerate() {
        let task = &tasks[index];
        let y = inner.y + 4 + visible as u16 * 3;
        let row = Rect::new(inner.x, y.saturating_sub(1), inner.width, 3);
        f.render_widget(
            Block::default().style(Style::default().bg(if index == selected {
                SELECT
            } else if visible % 2 == 0 {
                Color::Rgb(30, 20, 33)
            } else {
                BG
            })),
            row,
        );
        let c = columns(row);
        let issue = task
            .issue
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("Unknown");
        let pr = if task.pr_number > 0 {
            format!("#{}", task.pr_number)
        } else {
            "Unknown".into()
        };
        let values = [
            format!("{}", index + 1),
            field(Some(task), |t| t.department.as_deref()).into(),
            field(Some(task), |t| t.name.as_deref()).into(),
            format!(
                "{} • {}",
                field(Some(task), |t| t.agent.as_deref()),
                field(Some(task), |t| t.harness.as_deref())
            ),
            format!("{issue} • {pr}"),
            if task.review.is_empty() {
                "Unknown".into()
            } else {
                task.review.clone()
            },
            field(Some(task), |t| t.action.as_deref()).into(),
        ];
        for (i, (column, value)) in c.iter().zip(values).enumerate() {
            if i == 6 {
                continue;
            }
            text(
                f,
                Rect::new(column.x + 1, y, column.width.saturating_sub(2), 1),
                value,
                match i {
                    4 => CYAN,
                    5 => review_color(&task.review),
                    6 => {
                        if task.blocker.is_some() {
                            CORAL
                        } else {
                            GOLD
                        }
                    }
                    _ => IVORY,
                },
                i == 6,
            );
        }
        rule(f, Rect::new(inner.x, y + 1, inner.width, 1));
        let action = field(Some(task), |t| t.action.as_deref());
        badge(
            f,
            Rect::new(c[6].x, y, c[6].width, 1),
            action,
            action_color(action),
        );
        rows.push((row, index));
    }
    if let Some((row, _)) = rows.iter().find(|(_, i)| *i == selected) {
        f.render_widget(
            Block::bordered().border_style(Style::default().fg(GOLD)),
            *row,
        );
    }
}
fn selected(f: &mut Frame, area: Rect, task: Option<&Task>, actions: &mut Vec<(Rect, MopsAction)>) {
    let name = field(task, |t| t.name.as_deref());
    let inner = panel(
        f,
        area,
        &format!("SELECTED TASK · {}", clean(name).to_uppercase()),
    );
    let known_agent = task.and_then(|t| t.agent.as_deref()).filter(|s| {
        !s.trim().is_empty()
            && !matches!(
                normalized(s).as_str(),
                "unknown" | "unavailable" | "unassigned"
            )
    });
    let portrait = Rect::new(inner.x, inner.y, 6, 4);
    f.render_widget(
        Block::bordered().border_style(Style::default().fg(IVORY)),
        portrait,
    );
    f.render_widget(
        Paragraph::new(if known_agent.is_some() {
            "▟█▙\n▐▝▌"
        } else {
            " ? "
        })
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().fg(if known_agent.is_some() { GOLD } else { MUTED })),
        Rect::new(
            portrait.x + 1,
            portrait.y + 1,
            portrait.width.saturating_sub(2),
            2,
        ),
    );
    text(
        f,
        Rect::new(inner.x + 8, inner.y, inner.width.saturating_sub(8), 1),
        field(task, |t| t.agent.as_deref()),
        IVORY,
        true,
    );
    let issue = field(task, |t| t.issue.as_deref());
    let pr = task
        .filter(|t| t.pr_number > 0)
        .map(|t| format!("#{}", t.pr_number))
        .unwrap_or_else(|| "Unknown".into());
    text(
        f,
        Rect::new(inner.x + 8, inner.y + 2, inner.width.saturating_sub(8), 1),
        format!(
            "Department: {}  │  Issue: {issue}  │  PR: {pr} • {}",
            field(task, |t| t.department.as_deref()),
            field(task, |t| t.state.as_deref()).to_uppercase()
        ),
        IVORY,
        false,
    );
    text(
        f,
        Rect::new(inner.x + 8, inner.y + 3, inner.width.saturating_sub(8), 1),
        format!(
            "Implementation role: {}",
            field(task, |t| t.implementation_role.as_deref())
        ),
        CYAN,
        false,
    );
    let status_y = inner.y + 4;
    let blocker = task
        .and_then(|t| t.blocker.as_deref())
        .filter(|s| !s.trim().is_empty());
    let action = field(task, |t| t.action.as_deref());
    let status = if let Some(blocker) = blocker {
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
    let strip = Rect::new(inner.x, status_y, inner.width, 3);
    f.render_widget(
        Block::bordered()
            .border_style(Style::default().fg(GOLD))
            .style(Style::default().bg(SELECT)),
        strip,
    );
    f.render_widget(
        Paragraph::new(status)
            .alignment(ratatui::layout::Alignment::Center)
            .style(
                Style::default()
                    .fg(if blocker.is_some() { CORAL } else { GOLD })
                    .add_modifier(Modifier::BOLD),
            ),
        Rect::new(strip.x + 1, strip.y + 1, strip.width.saturating_sub(2), 1),
    );
    let feedback_y = status_y + 4;
    text(
        f,
        Rect::new(inner.x, feedback_y, inner.width, 1),
        "REVIEW FEEDBACK",
        GOLD,
        true,
    );
    let role_y = inner.bottom().saturating_sub(8);
    let action_y = role_y.saturating_sub(3);
    let feedback = task.map(|t| t.feedback.as_slice()).unwrap_or(&[]);
    let capacity = usize::from(action_y.saturating_sub(feedback_y + 2));
    if feedback.is_empty() {
        text(
            f,
            Rect::new(
                inner.x + 1,
                feedback_y + 2,
                inner.width.saturating_sub(2),
                1,
            ),
            task.map(|t| t.review_note.as_str())
                .unwrap_or("Unknown — no review snapshot supplied"),
            MUTED,
            false,
        );
    } else {
        for (i, line) in feedback.iter().take(capacity).enumerate() {
            text(
                f,
                Rect::new(
                    inner.x + 1,
                    feedback_y + 2 + i as u16,
                    inner.width.saturating_sub(2),
                    1,
                ),
                format!("●  {}", clean(line)),
                CORAL,
                false,
            );
        }
    }
    let buttons = Layout::horizontal([
        Constraint::Length(22),
        Constraint::Length(31),
        Constraint::Min(0),
    ])
    .spacing(2)
    .split(Rect::new(inner.x, action_y, inner.width, 3));
    button(
        f,
        buttons[0],
        "Open PR",
        task.is_some_and(|t| !t.pr_url.is_empty()),
        MopsAction::OpenPr,
        actions,
    );
    button(
        f,
        buttons[1],
        "View Mayhem",
        task.is_some(),
        MopsAction::ViewMayhem,
        actions,
    );
    rule(f, Rect::new(inner.x, role_y, inner.width, 1));
    text(
        f,
        Rect::new(inner.x, role_y + 1, inner.width, 1),
        "ROLE & WORKTREE",
        GOLD,
        true,
    );
    let role = field(task, |t| t.role_path.as_deref());
    let worktree = field(task, |t| t.worktree.as_deref());
    for (i, (label, value, enabled, action)) in [
        (
            "▤",
            role,
            task.is_some_and(|t| {
                t.role_available && t.role_path.as_deref().is_some_and(|s| !s.trim().is_empty())
            }),
            MopsAction::OpenRole,
        ),
        (
            "▤",
            worktree,
            task.is_some_and(|t| {
                t.worktree_available && t.worktree.as_deref().is_some_and(|s| !s.trim().is_empty())
            }),
            MopsAction::OpenWorktree,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let y = role_y + 3 + i as u16 * 3;
        text(
            f,
            Rect::new(inner.x + 1, y, inner.width.saturating_sub(26), 1),
            format!("{label}  {value}"),
            IVORY,
            false,
        );
        button(
            f,
            Rect::new(inner.right().saturating_sub(24), y.saturating_sub(1), 24, 3),
            if i == 0 { "Open role" } else { "Open folder" },
            enabled,
            action,
            actions,
        );
    }
}
fn handoff(
    f: &mut Frame,
    area: Rect,
    task: Option<&Task>,
    can_copy: bool,
    actions: &mut Vec<(Rect, MopsAction)>,
) {
    let inner = panel(f, area, "SESSION HANDOFF");
    for (i, (label, value)) in [
        ("Harness:", field(task, |t| t.harness.as_deref())),
        ("Session ID:", field(task, |t| t.session_id.as_deref())),
        ("Branch:", field(task, |t| t.branch.as_deref())),
        (
            "Model / provider:",
            field(task, |t| t.model_provider.as_deref()),
        ),
    ]
    .into_iter()
    .enumerate()
    {
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
    let resume_y = inner.y + 4;
    rule(f, Rect::new(inner.x, resume_y, inner.width, 1));
    text(
        f,
        Rect::new(inner.x, resume_y + 1, inner.width, 1),
        "RESUME COMMAND FROM PR",
        GOLD,
        true,
    );
    let command = field(task, |t| t.resume_command.as_deref());
    let command_box = Rect::new(inner.x, resume_y + 3, inner.width, 3);
    f.render_widget(
        Block::bordered().border_style(Style::default().fg(IVORY)),
        command_box,
    );
    f.render_widget(
        Paragraph::new(clean(command))
            .style(Style::default().fg(IVORY))
            .wrap(Wrap { trim: false }),
        Rect::new(
            command_box.x + 1,
            command_box.y + 1,
            command_box.width.saturating_sub(2),
            1,
        ),
    );
    let verified = can_copy
        && task.is_some_and(|t| {
            t.verified_resume
                && t.resume_command
                    .as_deref()
                    .is_some_and(|s| !s.trim().is_empty())
        });
    button(
        f,
        Rect::new(inner.x, resume_y + 6, 29, 3),
        "Copy resume command",
        verified,
        MopsAction::CopyResume,
        actions,
    );
    text(
        f,
        Rect::new(
            inner.x + 31,
            resume_y + 7,
            inner.width.saturating_sub(31),
            1,
        ),
        if verified {
            "Copy only — never executes"
        } else {
            "Unknown / unverified — copy disabled"
        },
        if verified { IVORY } else { MUTED },
        false,
    );
    let next_y = resume_y + 9;
    rule(f, Rect::new(inner.x, next_y, inner.width, 1));
    text(
        f,
        Rect::new(inner.x, next_y + 1, inner.width, 1),
        "NEXT ACTION",
        GOLD,
        true,
    );
    f.render_widget(
        Paragraph::new(clean(field(task, |t| t.next_action.as_deref())))
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(IVORY)),
        Rect::new(inner.x + 1, next_y + 3, inner.width.saturating_sub(2), 2),
    );
    text(
        f,
        Rect::new(inner.x + 1, next_y + 5, inner.width.saturating_sub(2), 1),
        task.and_then(|t| t.warnings.first())
            .map(|s| format!("PR warning: {} • d details", clean(s)))
            .unwrap_or_else(|| "You decide when to resume. No harness is launched.".into()),
        if task.is_some_and(|t| !t.warnings.is_empty()) {
            CORAL
        } else {
            IVORY
        },
        false,
    );
    button(
        f,
        Rect::new(inner.x, inner.bottom().saturating_sub(3), 25, 3),
        "Copy feedback",
        can_copy && task.is_some_and(|t| !t.feedback.is_empty()),
        MopsAction::CopyFeedback,
        actions,
    );
}
fn cast(
    f: &mut Frame,
    area: Rect,
    tasks: &[Task],
    selected: Option<&Task>,
    source: &str,
    error: Option<&str>,
) {
    rule(f, area);
    text(
        f,
        Rect::new(area.x + 1, area.y + 1, 7, 1),
        "CAST",
        GOLD,
        true,
    );
    let selected_agent = selected.and_then(|t| t.agent.as_deref());
    let mut names = Vec::new();
    for task in tasks {
        if let Some(agent) = task.agent.as_deref().filter(|s| {
            !s.trim().is_empty()
                && !matches!(
                    normalized(s).as_str(),
                    "unknown" | "unavailable" | "unassigned"
                )
        }) && !names.contains(&agent)
        {
            names.push(agent);
        }
    }
    let mut x = area.x + 10;
    if names.is_empty() {
        text(
            f,
            Rect::new(x, area.y + 1, area.width.saturating_sub(11), 1),
            format!("Unknown • {} • d source details", error.unwrap_or(source)),
            MUTED,
            false,
        );
    } else {
        for (index, name) in names.iter().enumerate() {
            let width = (Span::raw(*name).width() as u16 + 7).min(28);
            if x + width >= area.right().saturating_sub(25) {
                let more = format!("+{} more", names.len() - index);
                text(
                    f,
                    Rect::new(x, area.y + 1, area.right().saturating_sub(x), 1),
                    &more,
                    MUTED,
                    false,
                );
                x = x
                    .saturating_add(Span::raw(&more).width() as u16 + 2)
                    .min(area.right());
                break;
            }
            let active = Some(*name) == selected_agent;
            let label = format!("{} {}", if active { "▣" } else { "□" }, name);
            text(
                f,
                Rect::new(x, area.y + 1, width, 1),
                label,
                if active { GOLD } else { IVORY },
                active,
            );
            x += width;
        }
        text(
            f,
            Rect::new(x, area.y + 1, area.right().saturating_sub(x), 1),
            if error.is_some() {
                "PR records • source warning • d details".into()
            } else {
                format!("PR records, not live • {source}")
            },
            MUTED,
            false,
        );
    }
}
