use crate::{App, data::clean};
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
    app.buttons.clear();
    app.rows.clear();
    f.render_widget(
        Block::default().style(Style::default().bg(BG).fg(IVORY)),
        area,
    );
    if area.width < 90 || area.height < 28 {
        let inner = panel(f, area, "MAGIC / BACKSTAGE • SETTINGS");
        f.render_widget(Paragraph::new("Enlarge terminal to at least 90 × 28 for the Settings grid.\n↑/↓ select files • Enter/o open • d details • r refresh • q quit\n\nOnly Settings is implemented. No agents or models are contacted.").wrap(Wrap{trim:false}).style(Style::default().fg(IVORY)),inner);
        if app.details {
            details(f, app, area);
        }
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
        "▪ ▪ ▪  MAGIC / BACKSTAGE",
        IVORY,
    );
    if area.width > 115 {
        text(f, Rect::new(43, 0, 30, 1), "Technicolor Workshop", GOLD);
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
    let tabs = Layout::horizontal([Constraint::Percentage(20); 5]).split(sections[1]);
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
        let active = i == 4;
        f.render_widget(
            Block::bordered()
                .border_style(Style::default().fg(CORAL))
                .style(Style::default().bg(BG)),
            tabs[i],
        );
        let inside = Rect::new(
            tabs[i].x + 1,
            tabs[i].y + 1,
            tabs[i].width.saturating_sub(2),
            1,
        );
        f.render_widget(
            Paragraph::new(*label)
                .alignment(ratatui::layout::Alignment::Center)
                .style(
                    Style::default()
                        .fg(if active { BG } else { IVORY })
                        .bg(if active { GOLD } else { BG })
                        .add_modifier(Modifier::BOLD),
                ),
            inside,
        );
    }
    let upper = Layout::horizontal([Constraint::Percentage(75), Constraint::Percentage(25)])
        .spacing(1)
        .split(sections[3]);
    audit(f, app, upper[0]);
    inspector(f, app, upper[1]);
    files(f, app, sections[4]);
    let foot = sections[5];
    f.render_widget(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(CORAL)),
        foot,
    );
    text(
        f,
        Rect::new(1, foot.y + 1, area.width.saturating_sub(2), 1),
        "CAST : Unavailable".to_string(),
        GOLD,
    );
    let help = if app.refreshing {
        "Observing config… • q cancels safely"
    } else if app.message.is_empty() {
        "↑↓ select  Enter/o open  p project folder  d details  r refresh  q quit"
    } else {
        "Notice available • d details"
    };
    text(
        f,
        Rect::new(40, foot.y + 1, area.width.saturating_sub(41), 1),
        help,
        MUTED,
    );
    if app.details {
        details(f, app, area);
    }
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
