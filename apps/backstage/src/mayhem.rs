//! One view of the same native reports used by desktop Mayhem; inventory stays separate.
use crate::{App, widgets::*};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
};

pub fn draw(f: &mut Frame, area: Rect, app: &mut App) {
    let bands = Layout::vertical([Constraint::Percentage(52), Constraint::Min(3)]).split(area);
    let columns = Layout::horizontal([Constraint::Percentage(36), Constraint::Percentage(64)])
        .split(bands[0]);
    let projects = app
        .project_data
        .projects
        .iter()
        .map(|p| vec![p.name.clone(), p.path.display().to_string()])
        .collect();
    app.project_rows = table(
        f,
        columns[0],
        if app.focus_projects {
            "Saved projects • focused"
        } else {
            "Saved projects"
        },
        &["Project", "Directory"],
        projects,
        &[45, 55],
        app.project_selected,
    );
    let worktrees = app
        .project_data
        .worktrees
        .iter()
        .map(|w| {
            vec![
                format!("{}/{}", w.project, w.name),
                w.branch.clone(),
                bytes(w.bytes),
                if w.partial {
                    "Partial"
                } else if w.available {
                    "Observed"
                } else {
                    "Unknown"
                }
                .into(),
            ]
        })
        .collect();
    app.worktree_rows = table(
        f,
        columns[1],
        if app.focus_projects {
            "Git worktrees"
        } else {
            "Git worktrees • focused"
        },
        &["Worktree", "Branch", "Tracked size", "State"],
        worktrees,
        &[28, 32, 23, 17],
        app.worktree_selected,
    );
    let detail = if app.focus_projects {
        app.project_data
            .projects
            .get(app.project_selected)
            .map(|p| {
                format!(
                    "{}\n{}",
                    p.path.display(),
                    p.error.as_deref().unwrap_or("Saved references only")
                )
            })
    } else {
        app.project_data
            .worktrees
            .get(app.worktree_selected)
            .map(|w| {
                format!(
                    "{}\n{}\n{}",
                    w.path.display(),
                    w.error.as_deref().unwrap_or("Native Git metadata"),
                    w.folders
                        .iter()
                        .map(|p| format!(
                            "{} · {} · {} refs · {}",
                            p.name,
                            bytes(p.bytes),
                            p.files,
                            if p.available {
                                "observed"
                            } else {
                                "unavailable"
                            }
                        ))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            })
    };
    let activity = &app.project_data.activity;
    let agents = if activity["available"] == true {
        activity["rows"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|r| {
                format!(
                    "{} · {} · terminal {} · tab {} ({}) · pane {} ({})",
                    r["agent_name"].as_str().unwrap_or("Unknown agent"),
                    r["agent_status"].as_str().unwrap_or("Unknown"),
                    r["terminal_id"],
                    r["tab_name"].as_str().unwrap_or("Unknown"),
                    r["tab_id"],
                    r["pane_name"].as_str().unwrap_or("Unknown"),
                    r["pane_id"]
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        activity["error"]
            .as_str()
            .unwrap_or("Herdr activity not observed")
            .into()
    };
    paragraph(
        f,
        bands[1],
        "Selection and Herdr • a native reports • d full details",
        format!(
            "Herdr: {agents}\n\n{}\n{}",
            detail.unwrap_or_else(|| app
                .project_data
                .worktree_error
                .clone()
                .or(app.project_data.error.clone())
                .unwrap_or_else(|| "No observed inventory; r rereads.".into())),
            app.project_data.worktree_source
        ),
        0,
    );
}

pub fn draw_audit(f: &mut Frame, area: Rect, app: &mut App) {
    let Some(report) = app.audit.as_ref() else {
        paragraph(
            f,
            area,
            "Native reports",
            app.audit_error
                .as_deref()
                .unwrap_or("Reading saved report…"),
            0,
        );
        return;
    };
    let tools: &[serde_json::Value] = report.candidate.native["tools"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let wide = area.width >= 110;
    let panels = if wide {
        Layout::horizontal([Constraint::Percentage(28), Constraint::Percentage(72)]).split(area)
    } else {
        Layout::vertical([
            Constraint::Length((tools.len() as u16 + 3).min(area.height / 2).max(3)),
            Constraint::Min(3),
        ])
        .split(area)
    };
    let data = tools
        .iter()
        .map(|t| {
            vec![
                t["tool"].as_str().unwrap_or("Unknown").into(),
                t["exit_code"]
                    .as_i64()
                    .map(|n| format!("Exit {n}"))
                    .unwrap_or_else(|| {
                        if !t["signal"].is_null() {
                            format!("Signal {}", t["signal"])
                        } else {
                            "Not executed".into()
                        }
                    }),
            ]
        })
        .collect();
    app.audit_rows = table(
        f,
        panels[0],
        "Native commands • ↑↓",
        &["Tool", "Outcome"],
        data,
        &[60, 40],
        app.audit_selected,
    );
    paragraph(
        f,
        panels[1],
        "Original output • [/] scroll • d full • Enter artifact",
        report.detail(app.audit_selected),
        app.audit_scroll.min(u16::MAX as usize) as u16,
    );
}
