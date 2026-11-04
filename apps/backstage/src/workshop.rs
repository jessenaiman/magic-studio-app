//! Saved project navigation; selection never opens or changes a project.
use crate::{App, widgets::*};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
};
pub fn draw(f: &mut Frame, area: Rect, app: &mut App) {
    app.reconcile_workshop_selection();
    let bands = Layout::vertical([Constraint::Min(3), Constraint::Length(3)]).split(area);
    let columns = Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(bands[0]);
    let projects = app
        .project_data
        .projects
        .iter()
        .map(|p| {
            vec![
                p.name.clone(),
                if p.exists { "Saved" } else { "Missing" }.into(),
            ]
        })
        .collect();
    app.project_rows = table(
        f,
        columns[0],
        if !app.project_data.catalog_observed {
            "Projects • not observed"
        } else if app.focus_projects {
            "Projects • Tab → files"
        } else {
            "Projects"
        },
        &["Name", "State"],
        projects,
        &[75, 25],
        app.project_selected,
    );
    let project = app.project_data.projects.get(app.project_selected);
    let indices = app.visible_workshop_files();
    let files = if let Some(project) = project {
        indices
            .iter()
            .map(|i| {
                let file = &project.files[*i];
                vec![
                    file.name.clone(),
                    file.group.clone(),
                    bytes(file.bytes),
                    if file.available { "Ready" } else { "Missing" }.into(),
                ]
            })
            .collect()
    } else {
        Vec::new()
    };
    let selected = indices
        .iter()
        .position(|i| *i == app.project_file_selected)
        .unwrap_or(usize::MAX);
    app.project_file_rows = table(
        f,
        columns[1],
        "Saved references • 1–4 collapse groups",
        &["File", "Area", "Size", "State"],
        files,
        &[45, 25, 18, 12],
        selected,
    )
    .into_iter()
    .map(|(r, i)| (r, indices[i]))
    .collect();
    let content = project
        .map(|p| {
            if app.focus_projects {
                return format!(
                    "{}\n{}",
                    p.identity.display(),
                    p.error
                        .as_deref()
                        .unwrap_or("Saved project • Enter opens project • p opens folder")
                );
            }
            p.files
                .get(app.project_file_selected)
                .map(|file| {
                    format!(
                        "{}\n{} · {} · {}",
                        file.path.display(),
                        file.kind,
                        file.modified,
                        p.error.as_deref().unwrap_or("")
                    )
                })
                .unwrap_or_else(|| {
                    format!(
                        "{}\n{}",
                        p.path.display(),
                        p.error
                            .as_deref()
                            .unwrap_or("No visible references; expand groups 1–4")
                    )
                })
        })
        .unwrap_or_else(|| {
            app.project_data.error.clone().unwrap_or_else(|| {
                if app.refreshing {
                    "Reading saved projects…"
                } else {
                    "No saved project history. Press r to observe."
                }
                .into()
            })
        });
    paragraph(
        f,
        bands[1],
        "Selected • Enter opens • p folder • d full details",
        content,
        0,
    );
}
