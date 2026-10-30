//! Workshop is an observer: selection/collapse are local; only explicit Open registers a launch.
use crate::{App, data::clean};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph},
};
use std::path::{Path, PathBuf};

const BG: Color = Color::Rgb(24, 15, 27);
const IVORY: Color = Color::Rgb(242, 222, 188);
const GOLD: Color = Color::Rgb(245, 186, 56);
const LINE: Color = Color::Rgb(176, 126, 125);
const CORAL: Color = Color::Rgb(239, 117, 132);
const CYAN: Color = Color::Rgb(133, 210, 222);
const GREEN: Color = Color::Rgb(105, 215, 145);
const MUTED: Color = Color::Rgb(160, 149, 169);
const SELECT: Color = Color::Rgb(65, 44, 34);

fn text(f: &mut Frame, area: Rect, value: impl Into<String>, color: Color, bold: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let mut value = clean(&value.into());
    let width = usize::from(area.width.saturating_sub(1));
    if Span::raw(&value).width() > width {
        while !value.is_empty() && Span::raw(&value).width() >= width {
            value.pop();
        }
        value.push('…');
    }
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
fn rule(f: &mut Frame, area: Rect) {
    f.render_widget(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(LINE)),
        area,
    );
}
fn panel(f: &mut Frame, area: Rect) -> Rect {
    f.render_widget(
        Block::bordered()
            .border_style(Style::default().fg(LINE))
            .style(Style::default().bg(BG)),
        area,
    );
    Rect::new(
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    )
}
fn bytes(value: Option<u64>) -> String {
    match value {
        None => "Unavailable".into(),
        Some(n) if n >= 1024 * 1024 * 1024 => {
            format!("{:.1} GB", n as f64 / (1024.0 * 1024.0 * 1024.0))
        }
        Some(n) if n >= 1024 * 1024 => format!("{:.1} MB", n as f64 / (1024.0 * 1024.0)),
        Some(n) if n >= 1024 => format!("{:.1} KB", n as f64 / 1024.0),
        Some(n) => format!("{n} B"),
    }
}
fn total(project: &crate::project_data::ProjectRecord) -> Option<u64> {
    project
        .files
        .iter()
        .try_fold(0_u64, |sum, file| sum.checked_add(file.bytes?))
}
fn button(f: &mut Frame, app: &mut App, area: Rect, label: &str, path: Option<&Path>) {
    let enabled = path.is_some();
    let color = if enabled { GOLD } else { MUTED };
    f.render_widget(
        Block::bordered().border_style(Style::default().fg(color)),
        area,
    );
    let inside = Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), 1);
    f.render_widget(
        Paragraph::new(format!("[ {label} ]"))
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().fg(color)),
        inside,
    );
    if let Some(path) = path {
        app.project_buttons.push((area, path.to_path_buf()));
    }
}
fn columns(area: Rect) -> Vec<Rect> {
    Layout::horizontal([
        Constraint::Percentage(42),
        Constraint::Percentage(13),
        Constraint::Percentage(21),
        Constraint::Percentage(11),
        Constraint::Percentage(13),
    ])
    .split(area)
    .to_vec()
}

fn group_header(
    f: &mut Frame,
    area: Rect,
    name: &str,
    count: &str,
    icon: &str,
    color: Color,
    collapsed: bool,
) {
    text(
        f,
        Rect::new(area.x, area.y, 2, 1),
        if collapsed { "▸" } else { "▾" },
        color,
        true,
    );
    let art = match icon {
        "▥" => "┌╥┐\n└╨┘",
        "◇" => "╱┬╲\n╲┴╱",
        "♫" => "┌╮ \n♫♪ ",
        "□" => "┌─┐\n└─┘",
        _ => " ? \n   ",
    };
    f.render_widget(
        Paragraph::new(art).style(Style::default().fg(color)),
        Rect::new(area.x + 2, area.y, 3, area.height.min(2)),
    );
    text(
        f,
        Rect::new(area.x + 6, area.y, area.width.saturating_sub(6), 1),
        format!("{name}  ({count})"),
        color,
        true,
    );
}

pub fn draw(f: &mut Frame, area: Rect, app: &mut App) {
    app.project_rows.clear();
    app.project_file_rows.clear();
    app.project_buttons.clear();
    app.group_rows.clear();
    if area.width < 70 || area.height < 19 {
        text(
            f,
            area,
            "WORKSHOP • enlarge terminal for project and file panels",
            GOLD,
            true,
        );
        return;
    }
    let vertical = Layout::vertical([
        Constraint::Min(12),
        Constraint::Length(0),
        Constraint::Length(6),
    ])
    .split(area);
    let main = Layout::horizontal([Constraint::Percentage(21), Constraint::Percentage(79)])
        .spacing(1)
        .split(vertical[0]);
    sidebar(f, app, main[0]);
    inventory(f, app, main[1]);
    inspector(f, app, vertical[2]);
}
fn sidebar(f: &mut Frame, app: &mut App, area: Rect) {
    let inner = panel(f, area);
    let empty_state = if app.refreshing {
        (
            "Loading",
            "Reading saved projects",
            "Reading existing project history.",
        )
    } else if let Some(error) = app.project_data.error.as_deref() {
        ("Unavailable", "Unavailable", error)
    } else if app.project_data.catalog_observed {
        (
            "0 observed",
            "No saved projects",
            "Open a project in Magic Studio to add it to recent history.",
        )
    } else {
        (
            "Not observed",
            "Not observed",
            "Press r to read existing project history.",
        )
    };
    let count = if app.project_data.projects.is_empty() {
        empty_state.0.into()
    } else {
        format!("{} observed", app.project_data.projects.len())
    };
    text(
        f,
        Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(2), 1),
        format!("PROJECTS  [{count}]"),
        IVORY,
        true,
    );
    rule(f, Rect::new(inner.x, inner.y + 2, inner.width, 1));
    if app.project_data.projects.is_empty() {
        text(
            f,
            Rect::new(inner.x + 2, inner.y + 4, inner.width.saturating_sub(4), 1),
            format!("▥  {}", empty_state.1),
            GOLD,
            true,
        );
        text(
            f,
            Rect::new(inner.x + 2, inner.y + 6, inner.width.saturating_sub(4), 2),
            empty_state.2,
            MUTED,
            false,
        );
        text(
            f,
            Rect::new(inner.x + 2, inner.y + 9, inner.width.saturating_sub(4), 1),
            "d  source details",
            CYAN,
            false,
        );
        return;
    }
    let available = usize::from(inner.height.saturating_sub(3) / 5);
    let offset = app
        .project_selected
        .saturating_sub(available.saturating_sub(1));
    for (visible, index) in (offset..app.project_data.projects.len())
        .take(available)
        .enumerate()
    {
        let project = &app.project_data.projects[index];
        let row = Rect::new(inner.x, inner.y + 3 + visible as u16 * 5, inner.width, 4);
        let selected = index == app.project_selected;
        f.render_widget(
            Block::default().style(Style::default().bg(if selected { GOLD } else { BG })),
            row,
        );
        let color = if selected { BG } else { IVORY };
        text(f, Rect::new(row.x + 1, row.y + 1, 3, 1), "▥", color, true);
        text(
            f,
            Rect::new(row.x + 5, row.y + 1, row.width.saturating_sub(7), 1),
            &project.name,
            color,
            true,
        );
        let count = if project.complete {
            format!("{} files", project.files.len())
        } else {
            format!("{} observed • partial", project.files.len())
        };
        text(
            f,
            Rect::new(row.x + 5, row.y + 2, row.width.saturating_sub(7), 1),
            format!("{count} • {}", bytes(total(project))),
            color,
            false,
        );
        if selected {
            text(
                f,
                Rect::new(row.right().saturating_sub(2), row.y + 1, 2, 1),
                "›",
                BG,
                true,
            );
        }
        app.project_rows.push((row, index));
        if !selected {
            rule(f, Rect::new(row.x, row.bottom(), row.width, 1));
        }
    }
}
fn inventory(f: &mut Frame, app: &mut App, area: Rect) {
    let inner = panel(f, area);
    let Some(project) = app.project_data.projects.get(app.project_selected) else {
        text(
            f,
            Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(2), 1),
            "Project unavailable  │  Files / size unavailable",
            GOLD,
            true,
        );
        rule(f, Rect::new(inner.x, inner.y + 2, inner.width, 1));
        headings(
            f,
            Rect::new(inner.x + 1, inner.y + 3, inner.width.saturating_sub(2), 1),
        );
        rule(f, Rect::new(inner.x, inner.y + 4, inner.width, 1));
        empty_groups(
            f,
            app,
            Rect::new(
                inner.x + 1,
                inner.y + 5,
                inner.width.saturating_sub(2),
                inner.height.saturating_sub(5),
            ),
        );
        return;
    };
    let name = project.name.clone();
    let complete = project.complete;
    let size = total(project)
        .map(|n| format!("{} observed", bytes(Some(n))))
        .unwrap_or_else(|| "Size unavailable".into());
    let count = project.files.len();
    text(
        f,
        Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(2), 1),
        format!(
            "{name}  │  Areas unavailable • {count} {} • {size}",
            if complete {
                "files"
            } else {
                "observed files • partial"
            }
        ),
        GOLD,
        true,
    );
    rule(f, Rect::new(inner.x, inner.y + 2, inner.width, 1));
    headings(
        f,
        Rect::new(inner.x + 1, inner.y + 3, inner.width.saturating_sub(2), 1),
    );
    rule(f, Rect::new(inner.x, inner.y + 4, inner.width, 1));
    let body = Rect::new(
        inner.x + 1,
        inner.y + 5,
        inner.width.saturating_sub(2),
        inner.height.saturating_sub(5),
    );
    if count == 0 {
        empty_groups(f, app, body);
        return;
    }
    let has_other = project.files.iter().any(|file| {
        !["Storyboard", "Animations", "Lyrics", "Studio"].contains(&file.group.as_str())
    });
    let groups = if has_other { 5 } else { 4 };
    if body.height < groups as u16 * 5 {
        text(
            f,
            Rect::new(body.x, body.y, body.width, 1),
            "Compact grid: enlarge for grouped rows; selected file is inspectable below / d",
            GOLD,
            false,
        );
        if let Some(file) = app.project_data.projects[app.project_selected]
            .files
            .get(app.project_file_selected)
        {
            text(
                f,
                Rect::new(body.x + 2, body.y + 2, body.width.saturating_sub(2), 1),
                format!("▤ {}  • {}  • {}", file.name, file.kind, bytes(file.bytes)),
                IVORY,
                true,
            );
        }
        for (i, (name, color)) in [
            ("Storyboard", CORAL),
            ("Animations", GREEN),
            ("Lyrics", GOLD),
            ("Studio", CYAN),
            ("Unclassified", MUTED),
        ]
        .into_iter()
        .take(groups as usize)
        .enumerate()
        {
            if 4 + (i as u16) < body.height {
                text(
                    f,
                    Rect::new(
                        body.x + 2,
                        body.y + 4 + i as u16,
                        body.width.saturating_sub(2),
                        1,
                    ),
                    name,
                    color,
                    true,
                );
            }
        }
        return;
    }
    let sections = Layout::vertical((0..groups).map(|_| Constraint::Ratio(1, groups))).split(body);
    let specs = [
        ("Storyboard", "▥", CORAL),
        ("Animations", "◇", GREEN),
        ("Lyrics", "♫", GOLD),
        ("Studio", "□", CYAN),
        ("Unclassified", "?", MUTED),
    ];
    for (group, (name, icon, color)) in specs.iter().take(groups as usize).enumerate() {
        let section = sections[group];
        let file_ids: Vec<usize> = app.project_data.projects[app.project_selected]
            .files
            .iter()
            .enumerate()
            .filter(|(_, file)| {
                if group == 4 {
                    !["Storyboard", "Animations", "Lyrics", "Studio"].contains(&file.group.as_str())
                } else {
                    file.group == *name
                }
            })
            .map(|(i, _)| i)
            .collect();
        let collapsed = group < 4 && app.collapsed[group];
        let count = if file_ids.is_empty() && !complete {
            "Unavailable".into()
        } else {
            format!(
                "{} {}",
                file_ids.len(),
                if complete { "files" } else { "observed" }
            )
        };
        group_header(f, section, name, &count, icon, *color, collapsed);
        if group < 4 {
            app.group_rows
                .push((Rect::new(section.x, section.y, section.width, 2), group));
        }
        if !collapsed {
            let capacity = usize::from(section.height.saturating_sub(2) / 3);
            let selected_pos = file_ids
                .iter()
                .position(|i| *i == app.project_file_selected)
                .unwrap_or(0);
            let offset = selected_pos.saturating_sub(capacity.saturating_sub(1));
            if file_ids.is_empty() {
                text(
                    f,
                    Rect::new(
                        section.x + 5,
                        section.y + 2,
                        section.width.saturating_sub(5),
                        1,
                    ),
                    if complete {
                        "No observed files in this area"
                    } else {
                        "Unavailable — classification incomplete"
                    },
                    MUTED,
                    false,
                );
            }
            for (visible, index) in file_ids.iter().skip(offset).take(capacity).enumerate() {
                file_row(
                    f,
                    app,
                    Rect::new(
                        section.x + 5,
                        section.y + 2 + visible as u16 * 3,
                        section.width.saturating_sub(5),
                        1,
                    ),
                    *index,
                );
            }
        }
        if group + 1 < groups as usize {
            rule(
                f,
                Rect::new(
                    section.x,
                    section.bottom().saturating_sub(1),
                    section.width,
                    1,
                ),
            );
        }
    }
}
fn headings(f: &mut Frame, area: Rect) {
    for (column, name) in columns(area)
        .iter()
        .zip(["File", "Type", "Modified ▼", "Size", ""])
    {
        text(f, *column, name, IVORY, false);
    }
}
fn empty_groups(f: &mut Frame, app: &mut App, area: Rect) {
    let groups = Layout::vertical([Constraint::Percentage(25); 4]).split(area);
    for (i, (name, icon, color)) in [
        ("Storyboard", "▥", CORAL),
        ("Animations", "◇", GREEN),
        ("Lyrics", "♫", GOLD),
        ("Studio", "□", CYAN),
    ]
    .iter()
    .enumerate()
    {
        let section = groups[i];
        group_header(
            f,
            section,
            name,
            "Unavailable",
            icon,
            *color,
            app.collapsed[i],
        );
        app.group_rows
            .push((Rect::new(section.x, section.y, section.width, 2), i));
        if !app.collapsed[i] && section.height > 2 {
            text(
                f,
                Rect::new(
                    section.x + 5,
                    section.y + 2,
                    section.width.saturating_sub(5),
                    1,
                ),
                "File inventory unavailable",
                MUTED,
                false,
            );
        }
        if i < 3 {
            rule(
                f,
                Rect::new(
                    section.x,
                    section.bottom().saturating_sub(1),
                    section.width,
                    1,
                ),
            );
        }
    }
}
fn file_row(f: &mut Frame, app: &mut App, area: Rect, index: usize) {
    let file = &app.project_data.projects[app.project_selected].files[index];
    let name = file.name.clone();
    let kind = file.kind.clone();
    let modified = file.modified.clone();
    let size = bytes(file.bytes);
    let path = file.path.clone();
    let available = file.available;
    let row = Rect::new(area.x, area.y.saturating_sub(1), area.width, 3);
    if index == app.project_file_selected {
        f.render_widget(
            Block::bordered()
                .border_style(Style::default().fg(GOLD))
                .style(Style::default().bg(SELECT)),
            row,
        );
    }
    let cols = columns(area);
    text(f, cols[0], format!("▤  {name}"), IVORY, false);
    text(f, cols[1], kind, IVORY, false);
    text(f, cols[2], modified, IVORY, false);
    text(f, cols[3], size, IVORY, false);
    app.project_file_rows.push((row, index));
    button(
        f,
        app,
        Rect::new(
            cols[4].x,
            area.y.saturating_sub(1),
            cols[4].width.saturating_sub(1),
            3,
        ),
        "Open",
        if available { Some(&path) } else { None },
    );
}
fn inspector(f: &mut Frame, app: &mut App, area: Rect) {
    let inner = panel(f, area);
    let selected = app
        .project_data
        .projects
        .get(app.project_selected)
        .and_then(|p| p.files.get(app.project_file_selected));
    let name = selected
        .map(|f| f.name.clone())
        .unwrap_or_else(|| "File unavailable".into());
    let display_path = selected.map(|f| f.path.clone());
    let path = selected.filter(|f| f.available).map(|f| f.path.clone());
    let kind = selected
        .map(|f| f.kind.clone())
        .unwrap_or_else(|| "Unavailable".into());
    let columns = Layout::horizontal([
        Constraint::Percentage(22),
        Constraint::Percentage(55),
        Constraint::Percentage(23),
    ])
    .split(inner);
    f.render_widget(
        Paragraph::new("┌╲ \n│═│\n└─┘").style(Style::default().fg(IVORY)),
        Rect::new(columns[0].x + 2, columns[0].y, 3, 3),
    );
    text(
        f,
        Rect::new(
            columns[0].x + 9,
            columns[0].y + 1,
            columns[0].width.saturating_sub(11),
            1,
        ),
        name,
        IVORY,
        true,
    );
    f.render_widget(
        Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(LINE)),
        columns[1],
    );
    for (i, (label, value)) in [
        (
            "Path:",
            display_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "Unavailable".into()),
        ),
        ("Type:", kind),
        ("Source app:", "Unavailable".into()),
    ]
    .into_iter()
    .enumerate()
    {
        text(
            f,
            Rect::new(columns[1].x + 2, columns[1].y + i as u16, 12, 1),
            label,
            CYAN,
            false,
        );
        text(
            f,
            Rect::new(
                columns[1].x + 14,
                columns[1].y + i as u16,
                columns[1].width.saturating_sub(15),
                1,
            ),
            value,
            IVORY,
            false,
        );
    }
    let actions = Layout::horizontal([Constraint::Percentage(50); 2])
        .spacing(1)
        .split(columns[2]);
    button(
        f,
        app,
        Rect::new(actions[0].x, actions[0].y + 1, actions[0].width, 3),
        "Open file",
        path.as_deref(),
    );
    let folder: Option<PathBuf> = path
        .as_ref()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    button(
        f,
        app,
        Rect::new(actions[1].x, actions[1].y + 1, actions[1].width, 3),
        "Open folder",
        folder.as_deref(),
    );
}
