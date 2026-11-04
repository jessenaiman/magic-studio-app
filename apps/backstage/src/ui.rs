//! Responsive navigation and one details drawer; data belongs to the observer modules.
use crate::{App, View, widgets::*};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    widgets::{Block, Clear},
};

pub fn draw(f: &mut Frame, app: &mut App) {
    app.mops_rows.clear();
    app.mops_actions.clear();
    app.audit_rows.clear();
    app.tabs.clear();
    app.project_rows.clear();
    app.project_file_rows.clear();
    app.worktree_rows.clear();
    app.project_buttons.clear();
    app.rows.clear();
    let area = f.area();
    f.render_widget(
        Block::default().style(Style::default().bg(BG).fg(IVORY)),
        area,
    );
    if area.width < 40 || area.height < 10 {
        paragraph(
            f,
            area,
            "Backstage",
            "F1 Workshop · F3 MOPS · F4 Mayhem · F5 Settings\nq quits. Minimum 40×10.",
            0,
        );
        return;
    }
    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Min(3),
        Constraint::Length(2),
    ])
    .split(area);
    text(
        f,
        layout[0],
        format!("MAGIC / BACKSTAGE {}", env!("CARGO_PKG_VERSION")),
        GOLD,
        true,
    );
    let tabs = Layout::horizontal([Constraint::Percentage(25); 4]).split(layout[1]);
    for (i, (name, view)) in [
        ("F1 Workshop", View::Workshop),
        ("F3 MOPS", View::Mops),
        ("F4 Mayhem", View::Mayhem),
        ("F5 Settings", View::Settings),
    ]
    .iter()
    .enumerate()
    {
        let active = app.view == *view;
        f.render_widget(
            Block::bordered()
                .title(*name)
                .border_style(Style::default().fg(if active { GOLD } else { LINE }))
                .style(Style::default().bg(if active { SELECT } else { BG })),
            tabs[i],
        );
        app.tabs.push((tabs[i], *view));
    }
    match app.view {
        View::Workshop => crate::workshop::draw(f, layout[2], app),
        View::Mops => crate::mops::draw(f, layout[2], app),
        View::Mayhem if app.audit_inputs.is_some() || app.audit_pr_context.is_some() => {
            crate::mayhem::draw_audit(f, layout[2], app)
        }
        View::Mayhem => crate::mayhem::draw(f, layout[2], app),
        View::Settings => settings(f, layout[2], app),
    }
    let footer = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(layout[3]);
    text(
        f,
        footer[0],
        if app.refreshing {
            "Reading observations…"
        } else {
            &app.message
        },
        MUTED,
        false,
    );
    let help = Rect::new(
        footer[1].x,
        footer[1].y,
        footer[1].width.saturating_sub(8),
        1,
    );
    text(
        f,
        help,
        "↑↓ select · Tab focus · Enter/o open · d details · r reread · q quit",
        GOLD,
        false,
    );
    let open = Rect::new(help.right(), help.y, 8, 1);
    let target = if app.is_audit_mayhem() {
        app.audit
            .as_ref()
            .and_then(|a| a.artifact(app.audit_selected))
    } else {
        app.current_target_path(false)
    };
    text(
        f,
        open,
        "[ Open ]",
        if target.is_some() { GOLD } else { MUTED },
        false,
    );
    if let Some(path) = target {
        app.project_buttons.push((open, path));
    }
    if app.details {
        details(f, app, area);
    }
}
fn settings(f: &mut Frame, area: Rect, app: &mut App) {
    let layout = Layout::vertical([Constraint::Min(3), Constraint::Length(3)]).split(area);
    let files = app
        .data
        .files
        .iter()
        .map(|r| {
            vec![
                r.name.clone(),
                if r.found { "Found" } else { "Unavailable" }.into(),
                r.location(),
                r.modified.clone(),
            ]
        })
        .collect();
    app.rows = table(
        f,
        layout[0],
        "Settings • observed files and role documents",
        &["Item", "State", "Path", "Modified"],
        files,
        &[25, 15, 40, 20],
        app.selected,
    );
    paragraph(
        f,
        layout[1],
        "Source • p opens observed project root • d complete evidence",
        format!("{}\n{}", app.data.scope, app.data.provider_detail),
        0,
    );
}
fn details(f: &mut Frame, app: &App, area: Rect) {
    let content = match app.view {
        View::Mayhem if app.audit_inputs.is_some() || app.audit_pr_context.is_some() => app
            .audit
            .as_ref()
            .map(|r| r.detail(app.audit_selected))
            .unwrap_or_else(|| {
                app.audit_error
                    .clone()
                    .unwrap_or_else(|| "No saved report loaded".into())
            }),
        View::Mops => app.mops.tasks.get(app.mops_selected).map(|t|format!("{}\n{}\nRetrieved: {} · complete: {}\n{}\nAuthor: {} · state: {} · merged: {:?}\nBranch: {} · issue: {}\nAgent: {} · harness: {} · model: {}\nSession: {}\n\n{}\n\nWarnings:\n{}",t.pr_url,t.source,app.mops.retrieved,app.mops.complete,t.review_note,t.author.as_deref().unwrap_or("Unknown"),t.state.as_deref().unwrap_or("Unknown"),t.merged,t.branch.as_deref().unwrap_or("Unknown"),t.issue.as_deref().unwrap_or("Unknown"),t.agent.as_deref().unwrap_or("Unknown"),t.harness.as_deref().unwrap_or("Unknown"),t.model_provider.as_deref().unwrap_or("Unknown"),t.session_id.as_deref().unwrap_or("Unknown"),t.body,t.warnings.join("\n"))).unwrap_or_else(||app.mops.error.clone().unwrap_or_else(||"No recorded PR handoff".into())),
        View::Settings => {
            let file = app
                .data
                .files
                .get(app.selected)
                .map(|r| {
                    format!(
                        "{}\n{}\n{} · {}\n{}",
                        r.name,
                        r.location(),
                        r.kind,
                        r.modified,
                        r.reason
                    )
                })
                .unwrap_or_else(|| "No selected file".into());
            let library = match &app.data.library {
                Some(Ok(l)) => l
                    .rows()
                    .iter()
                    .map(|(name, count, _, paths)| {
                        format!(
                            "{name}: {count}\n{}",
                            paths
                                .iter()
                                .map(|p| p.display().to_string())
                                .collect::<Vec<_>>()
                                .join("\n")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n"),
                Some(Err(e)) => e.clone(),
                None => "Project root inventory not observed".into(),
            };
            format!(
                "{file}\n\n{}\n{}\n\n{library}",
                app.data.scope, app.data.provider_detail
            )
        }
        _ => {
            let project = app
                .project_data
                .projects
                .get(app.project_selected)
                .map(|p| {
                    format!(
                        "{}\n{}\n{}\n{}",
                        p.name,
                        p.path.display(),
                        p.error.as_deref().unwrap_or(""),
                        p.files
                            .iter()
                            .map(|r| format!(
                                "{} · {} · {} · {}\n{}",
                                r.name,
                                r.group,
                                bytes(r.bytes),
                                r.modified,
                                r.path.display()
                            ))
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                })
                .unwrap_or_else(|| "No selected project".into());
            let worktree = app
                .project_data
                .worktrees
                .get(app.worktree_selected)
                .map(|w| {
                    format!(
                        "{}\n{}\n{}\n{}",
                        w.name,
                        w.path.display(),
                        w.branch,
                        w.folders
                            .iter()
                            .map(|r| format!(
                                "{} · {} · {} refs\n{}",
                                r.name,
                                bytes(r.bytes),
                                r.files,
                                r.path.display()
                            ))
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                })
                .unwrap_or_default();
            format!(
                "{project}\n\n{worktree}\n\n{}\n{}\n{}\n\nHerdr snapshot observation:\n{}",
                app.project_data.source,
                app.project_data.worktree_source,
                app.project_data.error.as_deref().unwrap_or(""),
                serde_json::to_string_pretty(&app.project_data.activity).unwrap_or_default()
            )
        }
    };
    let inset = Rect::new(
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    f.render_widget(Clear, inset);
    f.render_widget(Block::default().style(Style::default().bg(BG)), inset);
    paragraph(
        f,
        inset,
        "Complete details • PgUp/PgDn scroll • d/Esc closes",
        format!("{content}\n\n{}", app.message),
        app.detail_scroll,
    );
}
