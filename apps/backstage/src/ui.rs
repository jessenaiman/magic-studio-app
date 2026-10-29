use crate::{App, View, data::clean};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use time::OffsetDateTime;
const BG: Color = Color::Rgb(24, 15, 27);
const IVORY: Color = Color::Rgb(242, 222, 188);
const GOLD: Color = Color::Rgb(245, 186, 56);
const CORAL: Color = Color::Rgb(176, 126, 125);
const CYAN: Color = Color::Rgb(133, 210, 222);
const GREEN: Color = Color::Rgb(80, 205, 139);
const MUTED: Color = Color::Rgb(160, 149, 169);
const SELECT: Color = Color::Rgb(65, 44, 34);
fn text(f: &mut Frame, area: Rect, value: impl Into<String>, color: Color) {
    let mut value = clean(&value.into());
    let width = area.width.saturating_sub(1) as usize;
    if Span::raw(&value).width() > width {
        while Span::raw(&value).width() >= width && !value.is_empty() {
            value.pop();
        }
        value.push('…');
    }
    f.render_widget(
        Paragraph::new(value).style(Style::default().fg(color)),
        area,
    );
}
fn panel(f: &mut Frame, area: Rect, title: &str) -> Rect {
    f.render_widget(
        Block::bordered().border_style(Style::default().fg(CORAL)),
        area,
    );
    f.render_widget(
        Paragraph::new(format!(" ▤  {title}"))
            .style(Style::default().fg(IVORY).add_modifier(Modifier::BOLD)),
        Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), 1),
    );
    f.render_widget(
        Paragraph::new("─".repeat(area.width.saturating_sub(2) as usize))
            .style(Style::default().fg(CORAL)),
        Rect::new(area.x + 1, area.y + 2, area.width.saturating_sub(2), 1),
    );
    Rect::new(
        area.x + 1,
        area.y + 3,
        area.width.saturating_sub(2),
        area.height.saturating_sub(4),
    )
}
fn button(f: &mut Frame, app: &mut App, area: Rect, title: &str, file: Option<usize>) {
    let enabled = file.is_some_and(|i| {
        if i == usize::MAX {
            app.data.projects_found
        } else {
            app.data.files.get(i).is_some_and(|r| r.found)
        }
    });
    let color = if enabled { GOLD } else { MUTED };
    let outline = if app.outlined_controls {
        Rect::new(area.x, area.y.saturating_sub(1), area.width, 3)
    } else {
        area
    };
    if app.outlined_controls {
        f.render_widget(
            Block::bordered().border_style(Style::default().fg(color)),
            outline,
        );
    }
    f.render_widget(
        Paragraph::new(format!("[ {title} ]"))
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().fg(color)),
        area,
    );
    if enabled && let Some(i) = file {
        app.buttons.push((outline, i));
    }
}
fn cells(area: Rect, widths: &[u16]) -> Vec<Rect> {
    Layout::horizontal(widths.iter().map(|w| Constraint::Percentage(*w)))
        .split(area)
        .iter()
        .map(|r| Rect::new(r.x + 2, r.y, r.width.saturating_sub(3), r.height))
        .collect()
}
fn row(f: &mut Frame, area: Rect, selected: bool, odd: bool, outlined: bool) {
    let paint = if outlined {
        Rect::new(area.x, area.y.saturating_sub(1), area.width, 3)
    } else {
        area
    };
    f.render_widget(
        Block::default().style(Style::default().bg(if selected {
            SELECT
        } else if odd {
            Color::Rgb(39, 26, 43)
        } else {
            BG
        })),
        paint,
    );
    if selected {
        if outlined {
            f.render_widget(
                Block::bordered().border_style(Style::default().fg(GOLD)),
                Rect::new(area.x, area.y.saturating_sub(1), area.width, 3),
            );
        }
        f.render_widget(
            Paragraph::new("▌").style(Style::default().fg(GOLD)),
            Rect::new(area.x, area.y, 1, area.height),
        );
    }
}
pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    app.outlined_controls = area.height >= 46;
    app.mops_rows.clear();
    app.mops_actions.clear();
    app.audit_rows.clear();
    app.tabs.clear();
    app.project_rows.clear();
    app.project_file_rows.clear();
    app.worktree_rows.clear();
    app.project_buttons.clear();
    app.group_rows.clear();
    app.buttons.clear();
    app.rows.clear();
    f.render_widget(
        Block::default().style(Style::default().bg(BG).fg(IVORY)),
        area,
    );
    if area.width < 90 || area.height < 28 {
        let inner = panel(f, area, "MAGIC / BACKSTAGE • SETTINGS");
        f.render_widget(Paragraph::new("Enlarge terminal to at least 90 × 28 for the observer grid.\n↑/↓ select files • Enter/o open • d details • r refresh • q quit\n\nWorkshop, Mayhem and Settings are available. No agents or models are contacted.").wrap(Wrap{trim:false}).style(Style::default().fg(IVORY)),inner);
        if app.details {
            details(f, app, area);
        }
        return;
    }
    if app.view == View::Workshop {
        workshop_shell(f, app, area);
        return;
    }
    let sections = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Length(1),
        if area.height >= 46 {
            Constraint::Length(21)
        } else {
            Constraint::Percentage(43)
        },
        Constraint::Min(10),
        Constraint::Length(2),
    ])
    .split(area);
    text(
        f,
        Rect::new(1, 0, area.width.saturating_sub(38), 1),
        "▪ ▪ ▪  M A G I C  /  B A C K S T A G E",
        IVORY,
    );
    if area.width > 115 {
        text(f, Rect::new(49, 0, 30, 1), "Technicolor Workshop", GOLD);
    }
    let now = OffsetDateTime::now_utc();
    text(
        f,
        Rect::new(area.width.saturating_sub(35), 0, 35, 1),
        format!(
            "v{}  {} {:02}:{:02} UTC ▪▪▪",
            env!("CARGO_PKG_VERSION"),
            now.date(),
            now.hour(),
            now.minute()
        ),
        IVORY,
    );
    let tab_area = if app.view == View::Mops {
        Rect::new(
            sections[1].x,
            sections[1].y,
            sections[1].width * 62 / 100,
            sections[1].height,
        )
    } else {
        sections[1]
    };
    navigation_tabs(f, app, tab_area);
    if app.view == View::Mops {
        crate::mops::draw(
            f,
            Rect::new(
                0,
                sections[3].y,
                area.width,
                sections[3].height + sections[4].height,
            ),
            app,
        );
    } else if app.view == View::Mayhem
        && (app.audit_inputs.is_some() || app.audit_pr_context.is_some())
    {
        crate::mayhem::draw_audit(
            f,
            Rect::new(
                0,
                sections[3].y,
                area.width,
                sections[3].height + sections[4].height,
            ),
            app,
        );
    } else if app.view == View::Mayhem {
        crate::mayhem::draw(
            f,
            Rect::new(
                0,
                sections[3].y,
                area.width,
                sections[3].height + sections[4].height,
            ),
            app,
        );
    } else {
        let upper = Layout::horizontal([Constraint::Percentage(75), Constraint::Percentage(25)])
            .spacing(1)
            .split(sections[3]);
        audit(f, app, upper[0]);
        inspector(f, app, upper[1]);
        files(f, app, sections[4]);
    }
    let foot = sections[5];
    f.render_widget(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(CORAL)),
        foot,
    );
    if app.view == View::Mops {
        mops_source_summary(f, app, foot, tab_area, area.width);
    } else {
        text(
            f,
            Rect::new(1, foot.y + 1, area.width.saturating_sub(2), 1),
            "CAST : Unavailable",
            GOLD,
        );
    }
    let help = if app.view == View::Mayhem
        && app.audit_inputs.is_some()
        && app.audit_error.is_none()
        && !app.refreshing
    {
        "↑↓ tool • [/] finding • d native details • B/C JSON • Enter source • r reread"
    } else if app.refreshing {
        "Observing config… • q cancels safely"
    } else if app.message.is_empty() {
        "↑↓ select  Enter/o open  p project folder  d details  r refresh  q quit"
    } else {
        "Notice available • d details"
    };
    if app.view != View::Mops {
        text(
            f,
            Rect::new(40, foot.y + 1, area.width.saturating_sub(41), 1),
            help,
            MUTED,
        );
    }
    if app.details {
        details(f, app, area);
    }
}
fn mops_source_summary(f: &mut Frame, app: &App, foot: Rect, tab_area: Rect, width: u16) {
    let source = app
        .mops
        .source
        .split(" • ")
        .next()
        .unwrap_or("Unknown source");
    text(
        f,
        Rect::new(1, foot.y + 1, width.saturating_sub(2), 1),
        format!(
            "{} | retrieved {} | R explicit reread; snapshots do not fetch GitHub",
            source, app.mops.retrieved
        ),
        CYAN,
    );
    let x = tab_area.right() + 2;
    let counts = format!(
        "{} TASKS | {} NEED DECISION | {} BLOCKED",
        if app.mops.complete {
            app.mops.tasks.len().to_string()
        } else {
            "Unknown".into()
        },
        app.mops
            .decisions()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "Unknown".into()),
        app.mops
            .blocked()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "Unknown".into())
    );
    text(
        f,
        Rect::new(x, tab_area.y + 1, width.saturating_sub(x), 1),
        counts,
        GOLD,
    );
}
fn navigation_tabs(f: &mut Frame, app: &mut App, tab_area: Rect) {
    let tabs = Layout::horizontal([Constraint::Percentage(20); 5]).split(tab_area);
    for (i, label) in [
        "◉  WORKSHOP  F1",
        "▰▣  BOOTH  F2",
        "▥▥  MOPS  F3",
        "✂  MAYHEM  F4",
        "⚙  SETTINGS  F5",
    ]
    .iter()
    .enumerate()
    {
        let view = match i {
            0 => Some(View::Workshop),
            2 => Some(View::Mops),
            3 => Some(View::Mayhem),
            4 => Some(View::Settings),
            _ => None,
        };
        navigation_tab(f, app, tabs[i], label, view);
    }
}
fn navigation_tab(f: &mut Frame, app: &mut App, area: Rect, label: &str, view: Option<View>) {
    if let Some(view) = view {
        app.tabs.push((area, view));
    }
    let active = view == Some(app.view);
    f.render_widget(
        Block::bordered()
            .border_style(Style::default().fg(CORAL))
            .style(Style::default().bg(BG)),
        area,
    );
    let inside = Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), 1);
    f.render_widget(
        Paragraph::new(label)
            .alignment(ratatui::layout::Alignment::Center)
            .style(
                Style::default()
                    .fg(if active {
                        BG
                    } else if view.is_none() {
                        MUTED
                    } else {
                        IVORY
                    })
                    .bg(if active { GOLD } else { BG })
                    .add_modifier(Modifier::BOLD),
            ),
        inside,
    );
}
fn audit(f: &mut Frame, app: &mut App, area: Rect) {
    let inner = panel(f, area, "CONFIGURATION AUDIT");
    let head = cells(
        Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(2), 1),
        &[29, 36, 18, 17],
    );
    for (r, s) in head
        .iter()
        .zip(["ITEM", "LOCATION / DETAILS", "STATUS", "ACTION"])
    {
        text(f, *r, s, IVORY);
    }
    text(
        f,
        Rect::new(inner.x, inner.y + 1, inner.width, 1),
        "─".repeat(inner.width as usize),
        CORAL,
    );
    let labels = [
        "Storyboard template location",
        "Magic projects directory",
        "Project count",
        "Unknown files",
        "Unknown folders",
    ];
    let stride = if app.outlined_controls {
        3
    } else {
        ((inner.height.saturating_sub(2)) / 5).max(1)
    };
    for (i, label) in labels.iter().enumerate() {
        let y = inner.y + 2 + i as u16 * stride;
        if y >= inner.y + inner.height {
            break;
        }
        let a = Rect::new(inner.x + 1, y, inner.width.saturating_sub(2), 1);
        row(
            f,
            a,
            i == 0 && app.selected == 0,
            i % 2 == 0,
            app.outlined_controls,
        );
        if i == 0 {
            app.rows.push((
                if app.outlined_controls {
                    Rect::new(a.x, a.y.saturating_sub(1), a.width, 3)
                } else {
                    a
                },
                0,
            ));
        }
        let c = cells(a, &[29, 36, 18, 17]);
        dividers(f, &c);
        text(f, c[0], format!(" {label}"), IVORY);
        text(
            f,
            c[1],
            if i == 0 {
                app.data.files[0].location()
            } else if i == 1 {
                app.data
                    .projects_root
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "Unavailable".into())
            } else {
                "Unavailable".into()
            },
            CYAN,
        );
        let found = if i == 0 {
            app.data.files[0].found
        } else {
            i == 1 && app.data.projects_found
        };
        text(
            f,
            c[2],
            if found {
                "● Found"
            } else {
                "● Unavailable"
            },
            if found { GREEN } else { GOLD },
        );
        button(
            f,
            app,
            c[3],
            if i == 1 {
                "Open folder"
            } else if i > 1 {
                "Inspect"
            } else {
                "Open"
            },
            if i == 0 {
                Some(0)
            } else if i == 1 {
                Some(usize::MAX)
            } else {
                None
            },
        );
    }
}
fn inspector(f: &mut Frame, app: &mut App, area: Rect) {
    let inner = panel(f, area, "SELECTED FILE");
    let Some(file) = app.data.files.get(app.selected) else {
        return;
    };
    let values = [
        ("Name", file.name.clone()),
        ("Path", file.location()),
        ("Modified", file.modified.clone()),
        ("Type", file.kind.into()),
        ("App", "Unavailable".into()),
    ];
    let stride = if inner.height >= 12 { 2 } else { 1 };
    for (i, (label, value)) in values.into_iter().enumerate() {
        let y = inner.y + i as u16 * stride;
        text(f, Rect::new(inner.x + 1, y, 9, 1), label, IVORY);
        text(
            f,
            Rect::new(inner.x + 10, y, inner.width.saturating_sub(11), 1),
            value,
            if label == "Name" { IVORY } else { CYAN },
        );
    }

    button(
        f,
        app,
        Rect::new(
            inner.x,
            inner.y + inner.height.saturating_sub(2),
            inner.width,
            1,
        ),
        "Open externally",
        Some(app.selected),
    );
}
fn files(f: &mut Frame, app: &mut App, area: Rect) {
    let inner = panel(f, area, "EXISTING FILES");
    let widths = [20, 18, 10, 13, 28, 11];
    let head = cells(
        Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(2), 1),
        &widths,
    );
    for (r, s) in head
        .iter()
        .zip(["NAME", "GROUP", "TYPE", "STATUS", "LOCATION", "ACTION"])
    {
        text(f, *r, s, IVORY);
    }
    text(
        f,
        Rect::new(inner.x, inner.y + 1, inner.width, 1),
        "─".repeat(inner.width as usize),
        CORAL,
    );
    let stride = if app.outlined_controls { 3 } else { 1 };
    let capacity = usize::from(inner.height.saturating_sub(2) / stride);
    if app.selected < app.offset {
        app.offset = app.selected;
    }
    if app.selected >= app.offset + capacity {
        app.offset = app.selected.saturating_sub(capacity.saturating_sub(1));
    }
    for visible in 0..capacity.min(app.data.files.len().saturating_sub(app.offset)) {
        let i = app.offset + visible;
        let a = Rect::new(
            inner.x + 1,
            inner.y + 2 + visible as u16 * stride,
            inner.width.saturating_sub(2),
            1,
        );
        let c = cells(a, &widths);
        dividers(f, &c);
        row(
            f,
            a,
            i == app.selected,
            visible % 2 == 0,
            app.outlined_controls,
        );
        let file = &app.data.files[i];
        let found = file.found;
        text(f, c[0], format!(" ▤ {}", file.name), IVORY);
        text(f, c[1], file.group, IVORY);
        text(f, c[2], file.kind, IVORY);
        text(
            f,
            c[3],
            if found {
                "● Found"
            } else {
                "● Unavailable"
            },
            if found { GREEN } else { GOLD },
        );
        text(f, c[4], file.location(), CYAN);
        app.rows.push((
            if app.outlined_controls {
                Rect::new(a.x, a.y.saturating_sub(1), a.width, 3)
            } else {
                a
            },
            i,
        ));
        button(f, app, c[5], "Open", Some(i));
    }
}

fn details(f: &mut Frame, app: &App, area: Rect) {
    if app.view != View::Settings {
        project_details(f, app, area);
        return;
    }
    let modal = Rect::new(
        area.x + 3,
        area.y + 3,
        area.width.saturating_sub(6),
        area.height.saturating_sub(6),
    );
    f.render_widget(Clear, modal);
    let inner = panel(f, modal, "DETAILS • PgUp/PgDn scroll • d / Esc close");
    let file = &app.data.files[app.selected];
    let content = format!(
        "{}\n\nPath: {}\n\nModified: {}\nType: {}\nAssociation: Unavailable\n\n{}\n\n{}\n\n{}\n\nProject root: {}\nProject count: unavailable — no authoritative complete-library source.\nUnknown files/folders: unavailable — no authoritative classification source.\nCast: unavailable — no runtime agents observed or contacted.\n\n{}",
        file.name,
        file.location(),
        file.modified,
        file.kind,
        file.reason,
        app.data.scope,
        app.data.provider_detail,
        app.data
            .projects_root
            .as_ref()
            .map(|p| clean(&p.display().to_string()))
            .unwrap_or_else(|| "Unavailable".into()),
        app.message
    );
    f.render_widget(
        Paragraph::new(content)
            .scroll((app.detail_scroll, 0))
            .style(Style::default().fg(IVORY))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn dividers(f: &mut Frame, columns: &[Rect]) {
    for column in columns.iter().skip(1) {
        f.render_widget(
            Paragraph::new("│").style(Style::default().fg(CORAL)),
            Rect::new(column.x.saturating_sub(2), column.y, 1, 1),
        );
    }
}

fn workshop_shell(f: &mut Frame, app: &mut App, area: Rect) {
    f.render_widget(
        Paragraph::new("╭●╮\n╰●╯").style(Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
        Rect::new(1, 0, 3, 2),
    );
    f.render_widget(
        Paragraph::new("MAGIC / BACKSTAGE")
            .style(Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
        Rect::new(7, 1, 24, 1),
    );
    if area.width >= 150 {
        text(f, Rect::new(31, 1, 30, 1), "│ Technicolor Workshop", IVORY);
    }
    let tab_start = if area.width >= 150 { 62 } else { 42 };
    let tab_width = area.width.saturating_sub(tab_start + 23);
    let tabs = Layout::horizontal([Constraint::Percentage(20); 5])
        .split(Rect::new(tab_start, 0, tab_width, 3));
    for (i, label) in [
        "WORKSHOP F1",
        "BOOTH F2",
        "MOPS F3",
        "MAYHEM F4",
        "SETTINGS F5",
    ]
    .into_iter()
    .enumerate()
    {
        let label = if tab_width < 35 {
            ["F1", "F2", "F3", "F4", "F5"][i]
        } else if tab_width < 60 {
            ["W F1", "B F2", "M F3", "Y F4", "S F5"][i]
        } else {
            label
        };
        let view = match i {
            0 => Some(View::Workshop),
            2 => Some(View::Mops),
            3 => Some(View::Mayhem),
            4 => Some(View::Settings),
            _ => None,
        };
        if let Some(view) = view {
            app.tabs.push((tabs[i], view));
        }
        f.render_widget(
            Block::bordered().border_style(Style::default().fg(CORAL)),
            tabs[i],
        );
        f.render_widget(
            Paragraph::new(label)
                .alignment(ratatui::layout::Alignment::Center)
                .style(
                    Style::default()
                        .fg(if i == 0 {
                            BG
                        } else if view.is_none() {
                            MUTED
                        } else {
                            IVORY
                        })
                        .bg(if i == 0 { GOLD } else { BG })
                        .add_modifier(Modifier::BOLD),
                ),
            Rect::new(tabs[i].x + 1, 1, tabs[i].width.saturating_sub(2), 1),
        );
    }
    let now = OffsetDateTime::now_utc();
    text(
        f,
        Rect::new(area.width.saturating_sub(22), 1, 22, 1),
        format!("{} {:02}:{:02} UTC", now.date(), now.hour(), now.minute()),
        CYAN,
    );
    f.render_widget(
        Paragraph::new("─".repeat(area.width as usize)).style(Style::default().fg(CORAL)),
        Rect::new(0, 2, area.width, 1),
    );
    crate::workshop::draw(
        f,
        Rect::new(0, 3, area.width, area.height.saturating_sub(6)),
        app,
    );
    let y = area.height.saturating_sub(3);
    f.render_widget(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(CORAL)),
        Rect::new(0, y, area.width, 3),
    );
    text(f, Rect::new(1, y + 1, 27, 1), "CAST : Unavailable", GOLD);
    text(
        f,
        Rect::new(30, y + 1, area.width.saturating_sub(31), 1),
        if app.refreshing {
            "Observing saved refs / Git… q cancels"
        } else {
            "Tab focus • ↑↓ select • 1–4 collapse • Enter open • p folder • d details • r refresh"
        },
        MUTED,
    );
    if app.details {
        project_details(f, app, area);
    }
}
fn project_details(f: &mut Frame, app: &App, area: Rect) {
    if app.view == View::Mops {
        mops_details(f, app, area);
        return;
    }
    if app.view == View::Mayhem && app.audit_inputs.is_some() {
        audit_details(f, app, area);
        return;
    }
    let modal = Rect::new(
        3,
        3,
        area.width.saturating_sub(6),
        area.height.saturating_sub(6),
    );
    f.render_widget(Clear, modal);
    let inner = panel(
        f,
        modal,
        "OBSERVATION DETAILS • PgUp/PgDn scroll • d/Esc close",
    );
    let selected = if app.view == View::Mayhem {
        app.project_data
            .worktrees
            .get(app.worktree_selected)
            .map(|w| {
                format!(
                    "{} / {}\nPath: {}\nBranch: {}\nTracked files: {:?} • bytes {:?} • partial: {}\n{}",
                    w.project,
                    w.name,
                    clean(&w.path.display().to_string()),
                    w.branch,
                    w.files,
                    w.bytes,
                    w.partial,
                    w.error.as_deref().unwrap_or("No observation error")
                )
            })
    } else {
        app.project_data
            .projects
            .get(app.project_selected)
            .map(|p| {
                let file = p.files.get(app.project_file_selected);
                format!(
                    "{}\nFolder: {}\n{}\n{}",
                    p.name,
                    clean(&p.path.display().to_string()),
                    file.map(|f| format!(
                        "File: {}\nType: {}\nSource group: {}\nModified: {}",
                        clean(&f.path.display().to_string()),
                        f.kind,
                        f.group,
                        f.modified
                    ))
                    .unwrap_or_else(|| "Selected file unavailable".into()),
                    p.error
                        .as_deref()
                        .unwrap_or("Saved references only; external folder inventory unavailable")
                )
            })
    };
    let content = format!(
        "{}\n\nProject source: {}\n{}\n\nGit source: {}\n{}\n\n{}",
        selected.unwrap_or_else(|| "Selection unavailable".into()),
        app.project_data.source,
        app.project_data.error.as_deref().unwrap_or(""),
        app.project_data.worktree_source,
        app.project_data.worktree_error.as_deref().unwrap_or(""),
        app.message
    );
    f.render_widget(
        Paragraph::new(content)
            .style(Style::default().fg(IVORY))
            .wrap(Wrap { trim: false })
            .scroll((app.detail_scroll, 0)),
        inner,
    );
}

fn audit_details(f: &mut Frame, app: &App, area: Rect) {
    let modal = Rect::new(
        3,
        3,
        area.width.saturating_sub(6),
        area.height.saturating_sub(6),
    );
    f.render_widget(Clear, modal);
    let inner = panel(
        f,
        modal,
        "NATIVE AUDIT DETAILS • PgUp/PgDn scroll • d/Esc close",
    );
    let content = if let Some(audit) = &app.audit {
        let finding = audit
            .finding(app.audit_selected, app.audit_finding_selected)
            .map(|i| serde_json::to_string_pretty(i).unwrap_or_else(|e| e.to_string()))
            .unwrap_or_else(|| "No candidate issue for selected tool".into());
        let id = audit.tool_ids().get(app.audit_selected).cloned();
        let errors: Vec<_> = [
            ("baseline", &audit.baseline),
            ("candidate", &audit.candidate),
        ]
        .into_iter()
        .flat_map(|(source, r)| {
            r.native["errors"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| id.as_deref().is_some_and(|id| e["toolId"] == id))
                .map(move |e| serde_json::json!({"source":source,"native":e}))
        })
        .collect();
        let finding = format!(
            "{}\n\nSelected-tool native error records (complete):\n{}",
            finding,
            serde_json::to_string_pretty(&errors).unwrap_or_else(|e| e.to_string())
        );
        format!(
            "Selected candidate native finding:\n{}\n\n{}\n\n{}",
            finding,
            audit.markdown(),
            app.message
        )
    } else {
        format!(
            "Audit artifact unavailable: {}\n{}",
            app.audit_error
                .as_deref()
                .unwrap_or("Loading bounded JSON report files"),
            app.message
        )
    };
    scrollable_details(f, inner, &content, app.detail_scroll);
}

fn mops_details(f: &mut Frame, app: &App, area: Rect) {
    let modal = Rect::new(
        3,
        3,
        area.width.saturating_sub(6),
        area.height.saturating_sub(6),
    );
    f.render_widget(Clear, modal);
    let inner = panel(
        f,
        modal,
        "PR HANDOFF DETAILS • PgUp/PgDn scroll • d/Esc close",
    );
    let content=app.mops.tasks.get(app.mops_selected).map(|task|format!("PR #{} {}\n{}\nAuthor: {}\nHead SHA: {}\nBase SHA: {}\n{}\nCI (not review approval): {}\n{}\n\nImplementation role: {}\nHarness: {}\nSession: {}\nWorktree: {}\nRole: {}\nResume command (copy only; never executes): {}\nAuthor verified: {}\nNext action: {}\nBlocker: {}\n\nWarnings:\n{}\n\nNative PR body:\n{}\n\nReview feedback:\n{}",task.pr_number,task.name.as_deref().unwrap_or("Unknown"),task.pr_url,task.author.as_deref().unwrap_or("Unknown"),task.head_sha.as_deref().unwrap_or("Unknown"),task.base_sha.as_deref().unwrap_or("Unknown"),task.source,task.ci,task.review_note,task.implementation_role.as_deref().unwrap_or("Unknown"),task.harness.as_deref().unwrap_or("Unknown"),task.session_id.as_deref().unwrap_or("Unknown"),task.worktree.as_deref().unwrap_or("Unknown"),task.role_path.as_deref().unwrap_or("Unknown"),task.resume_command.as_deref().unwrap_or("Unknown"),task.verified_resume,task.next_action.as_deref().unwrap_or("Unknown"),task.blocker.as_deref().unwrap_or("Unknown"),task.warnings.join("\n"),task.body,task.feedback.join("\n\n"))).unwrap_or_else(||"No PR metadata read".into());
    let state = app
        .mops
        .tasks
        .get(app.mops_selected)
        .map(|task| {
            format!(
                "Native PR state: {} • merged: {}",
                task.state.as_deref().unwrap_or("Unknown"),
                task.merged
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "Unknown".into())
            )
        })
        .unwrap_or_default();
    let content = format!("{}\n{}", state, content);
    let content = format!(
        "{}\nRetrieved: {}\n{}\n\n{}\n\n{}",
        app.mops.source,
        app.mops.retrieved,
        app.mops.error.as_deref().unwrap_or(""),
        content,
        app.message
    );
    scrollable_details(f, inner, &content, app.detail_scroll);
}

fn scrollable_details(f: &mut Frame, area: Rect, content: &str, scroll: u16) {
    let content = content
        .chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' {
                ' '
            } else {
                c
            }
        })
        .collect::<String>();
    f.render_widget(
        Paragraph::new(content)
            .style(Style::default().fg(IVORY))
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        area,
    );
}
