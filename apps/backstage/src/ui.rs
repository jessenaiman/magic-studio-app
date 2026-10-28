use crate::{
    model::{App, text},
    system::Metrics,
    theme::Theme,
};
use ratatui::{prelude::*, widgets::*};
use serde_json::Value;
use std::time::Instant;

pub const TABS: [&str; 5] = ["OVERVIEW", "BOOTH", "MOPS", "CODE", "MAYHEM"];
fn panel<'a>(title: impl Into<Line<'a>>, t: &Theme) -> Block<'a> {
    Block::bordered()
        .title(title)
        .border_style(Style::default().fg(t.muted))
        .style(t.base())
}
fn para(f: &mut Frame, area: Rect, title: &str, lines: Vec<Line<'_>>, t: &Theme) {
    f.render_widget(
        Paragraph::new(lines)
            .block(panel(title, t))
            .wrap(Wrap { trim: false }),
        area,
    );
}
fn value(v: &Value) -> String {
    text(v).replace('\n', " ")
}
fn short_path(v: &Value) -> String {
    v.as_str()
        .and_then(|s| std::path::Path::new(s).file_name())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| value(v))
}

pub fn draw(f: &mut Frame, a: &App, t: &Theme, m: &Metrics, thumb: Option<&image::RgbImage>) {
    let full = f.area();
    f.render_widget(Block::new().style(t.base()), full);
    let size = Rect::new(
        full.x + full.width.saturating_sub(180) / 2,
        full.y,
        full.width.min(180),
        full.height,
    );
    if size.width < 60 || size.height < 18 {
        f.render_widget(
            Paragraph::new(
                "BACKSTAGE\nResize to at least 60 × 18.\nQ exits. Production continues elsewhere.",
            )
            .style(t.accent()),
            size,
        );
        return;
    }
    let areas = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Min(8),
        Constraint::Length(2),
    ])
    .split(size);
    let title = Line::from(vec![
        Span::styled(" BACKSTAGE ", t.accent()),
        Span::styled(" / THE PRODUCTION DESK", t.base().fg(t.muted)),
        Span::raw(format!("   {}", t.name)),
    ]);
    f.render_widget(
        Paragraph::new(vec![
            title,
            Line::from(" ▪ ▪ ▪   A quiet look behind great things."),
        ])
        .style(t.base()),
        areas[0],
    );
    f.render_widget(
        Tabs::new(
            TABS.iter()
                .enumerate()
                .map(|(i, n)| format!("{} {n}", i + 1)),
        )
        .select(a.tab)
        .highlight_style(t.base().bg(t.accent).fg(t.bg).bold())
        .divider(" ")
        .padding(" ", " "),
        areas[1],
    );
    match a.tab {
        0 => overview(f, areas[2], a, t),
        1 => booth(f, areas[2], a, t, thumb),
        2 => mops(f, areas[2], a, t),
        3 => code(f, areas[2], a, t),
        _ => mayhem(f, areas[2], m, t),
    };
    let status = if a.searching {
        format!(" /{}▏   Enter to finish · Esc clears", a.query)
    } else {
        format!(
            " [A] Audit  [R] Refresh  [/] Find  [1–5] Views  [T] Theme  [Q] Exit  {}",
            if a.busy { "· Reading…" } else { "" }
        )
    };
    f.render_widget(
        Paragraph::new(vec![
            Line::styled(status, t.accent()),
            Line::styled(
                format!(" {}", value(&Value::String(a.status.clone()))),
                t.base().fg(t.muted),
            ),
        ]),
        areas[3],
    );
}
fn overview(f: &mut Frame, area: Rect, a: &App, t: &Theme) {
    let (list, productivity) = if area.width >= 100 {
        let p = Layout::horizontal([Constraint::Percentage(72), Constraint::Percentage(28)])
            .split(area);
        (p[0], Some(p[1]))
    } else {
        (area, None)
    };
    let parts = Layout::vertical([Constraint::Min(6), Constraint::Length(5)]).split(list);
    let projects = a.projects();
    let entries: Vec<Row> = projects
        .iter()
        .map(|p| {
            let state = p["readiness"]["state"]
                .as_str()
                .unwrap_or_else(|| p["open_state"].as_str().unwrap_or("Not reported"));
            Row::new([
                value(&p["name"]),
                state.into(),
                value(&p["readiness"]["next_step"]),
            ])
            .height(2)
        })
        .collect();
    let table = Table::new(
        entries,
        [
            Constraint::Percentage(36),
            Constraint::Percentage(18),
            Constraint::Percentage(46),
        ],
    )
    .header(
        Row::new(["VIDEO", "OBSERVED", "NEXT REVIEW"])
            .style(t.accent())
            .bottom_margin(1),
    )
    .row_highlight_style(t.base().bg(t.selected).bold())
    .highlight_symbol("▸ ")
    .block(panel(" TODAY'S CALL SHEET ", t));
    let mut selection = TableState::default().with_selected(if projects.is_empty() {
        None
    } else {
        Some(a.selected.min(projects.len() - 1))
    });
    f.render_stateful_widget(table, parts[0], &mut selection);
    if projects.is_empty() {
        let inner = parts[0].inner(Margin::new(2, 3));
        f.render_widget(
            Paragraph::new(
                "No reported projects match this list.\nR refreshes Magic's catalog; / filters it.",
            ),
            inner,
        );
    }
    let selected = a.project().unwrap_or(Value::Null);
    para(
        f,
        parts[1],
        " SELECTED / SCOPE ",
        vec![
            Line::styled(value(&selected["name"]), t.accent()),
            Line::from(value(&selected["path"])),
            Line::styled(value(&a.catalog["scope"]), t.base().fg(t.muted)),
        ],
        t,
    );
    if let Some(p) = productivity {
        let mut lines = vec![
            Line::from(""),
            Line::styled(" RELEASES", t.accent()),
            Line::from(""),
            Line::styled("  —", t.accent().bold()),
            Line::from("  Total not reported"),
            Line::from(""),
            Line::from("  Today       —"),
            Line::from("  Daily goal  —"),
            Line::from("  In review   —"),
            Line::from(""),
            Line::styled(" FROM MAGIC AUDIT", t.accent()),
            Line::from(""),
            Line::from(" A retrieves the report."),
            Line::from(" Counts stay unknown until"),
            Line::from(" the source supplies them."),
            Line::from(""),
            Line::styled("   ╭────╮", t.base().fg(t.yellow)),
            Line::styled("   │ o o│   ON YOUR SIDE", t.base().fg(t.yellow)),
            Line::styled("   ╰─┬──╯", t.base().fg(t.yellow)),
        ];
        if !a.report.is_null() {
            lines.push(Line::from("Audit available in CODE."));
        }
        para(f, p, " PRODUCTIVITY ", lines, t);
    }
}
fn booth(f: &mut Frame, area: Rect, a: &App, t: &Theme, thumb: Option<&image::RgbImage>) {
    let parts = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(6),
        Constraint::Length(5),
    ])
    .split(area);
    let p = a.project().unwrap_or(Value::Null);
    para(
        f,
        parts[0],
        " BOOTH / SELECTED VIDEO ",
        vec![Line::styled(value(&p["name"]), t.accent())],
        t,
    );
    let cols = Layout::horizontal([Constraint::Percentage(44), Constraint::Percentage(56)])
        .split(parts[1]);
    let block = panel(" LAST AVAILABLE STILL ", t);
    let inside = block.inner(cols[0]);
    f.render_widget(block, cols[0]);
    if let Some(img) = thumb {
        let w = u32::from(inside.width);
        let h = u32::from(inside.height) * 2;
        if w > 0 && h > 0 {
            let scaled = image::imageops::thumbnail(img, w, h);
            for y in (0..scaled.height()).step_by(2) {
                for x in 0..scaled.width() {
                    let top = scaled.get_pixel(x, y).0;
                    let bottom = scaled.get_pixel(x, (y + 1).min(scaled.height() - 1)).0;
                    f.buffer_mut()[(inside.x + x as u16, inside.y + (y / 2) as u16)]
                        .set_symbol("▀")
                        .set_fg(Color::Rgb(top[0], top[1], top[2]))
                        .set_bg(Color::Rgb(bottom[0], bottom[1], bottom[2]));
                }
            }
        }
    } else {
        f.render_widget(Paragraph::new("\n  No cached still reported.\n\n  Backstage does not render\n  or generate a preview.").wrap(Wrap{trim:false}),inside);
    }
    let mut lines = vec![
        Line::styled("EVIDENCE, NOT ASSUMPTIONS", t.accent()),
        Line::from(""),
        Line::from(format!("Project: {}", short_path(&p["path"]))),
        Line::from(format!("Observed: {}", value(&p["open_state"]))),
        Line::from(""),
        Line::from("App motion proof: not reported"),
        Line::from("Saved source: not reported"),
        Line::from("Video placement: not reported"),
        Line::from("Release: not reported"),
        Line::from(""),
        Line::styled("ANIMATION IS ITERATIVE", t.base().fg(t.yellow)),
        Line::from("Prove motion in Magic; save;"),
        Line::from("review the placed video; repeat."),
    ];
    if !a.project_report.is_null() {
        lines = vec![
            Line::styled("MAGIC OBSERVATION / DISK", t.accent()),
            Line::from(""),
        ];
        if let Some(items) = a.project_report["prerequisites"].as_array() {
            for item in items {
                lines.push(Line::from(format!(
                    "{}: {}",
                    value(&item["label"]),
                    value(&item["state"])
                )));
            }
        }
        if let Some(problems) = a.project_report["problems"].as_array() {
            for problem in problems.iter().take(5) {
                lines.push(Line::styled(value(problem), t.base().fg(t.yellow)));
            }
        }
        lines.push(Line::from(""));
        lines.push(Line::from("Studio acknowledgement: unobserved"));
        lines.push(Line::from("Saved-file facts do not prove live motion."));
    } else if !a.project_error.is_empty() {
        lines.push(Line::styled(a.project_error.clone(), t.base().fg(t.red)));
    }
    lines.push(Line::styled(
        "R refreshes this selected project",
        t.base().fg(t.muted),
    ));
    para(f, cols[1], " REVIEW DESK ", lines, t);
    para(
        f,
        parts[2],
        " NEXT REPORTED STEP ",
        vec![
            Line::styled(value(&p["readiness"]["next_step"]), t.accent()),
            Line::from("Cross into Magic to change the production."),
            Line::styled(
                "This view observes. It never launches, saves or approves.",
                t.base().fg(t.muted),
            ),
        ],
        t,
    );
}
fn mops(f: &mut Frame, area: Rect, a: &App, t: &Theme) {
    let parts = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(6),
        Constraint::Length(3),
    ])
    .split(area);
    para(
        f,
        parts[0],
        " MOPS / PERFORMERS ",
        vec![Line::from(a.agent_status.clone())],
        t,
    );
    let pulse = Instant::now() < a.effect_until && !a.reduce_motion;
    let frame = (a
        .effect_until
        .saturating_duration_since(Instant::now())
        .as_millis()
        / 180)
        % 2;
    let rows: Vec<Row> = a
        .agents
        .iter()
        .skip(usize::from(a.scroll))
        .map(|p| {
            let status = p["agent_status"].as_str().unwrap_or("unknown");
            let ink = match status {
                "working" => t.green,
                "blocked" => t.yellow,
                "done" => t.blue,
                _ => t.muted,
            };
            let face = if pulse && frame == 0 {
                "╭─╮\n│o│╱\n╰┬╯"
            } else {
                "╭─╮\n│o│\n╰┬╯"
            };
            let name = p["name"]
                .as_str()
                .or(p["agent"].as_str())
                .unwrap_or("Unknown");
            Row::new(vec![
                Cell::from(face).style(Style::default().fg(ink)),
                Cell::from(format!(
                    "{}\n{}",
                    value(&Value::String(name.to_owned())),
                    value(&p["pane_id"])
                )),
                Cell::from(status.to_owned()).style(Style::default().fg(ink)),
                Cell::from(value(&p["terminal_title_stripped"])),
                Cell::from(short_path(&p["foreground_cwd"])),
            ])
            .height(4)
        })
        .collect();
    let table = Table::new(
        rows,
        [
            Constraint::Length(5),
            Constraint::Length(15),
            Constraint::Length(12),
            Constraint::Percentage(50),
            Constraint::Min(10),
        ],
    )
    .header(Row::new(["", "PERFORMER", "STATE", "OBSERVED TITLE", "WORKSPACE"]).style(t.accent()))
    .block(panel(" CAST / READ ONLY ", t));
    f.render_widget(table, parts[1]);
    para(
        f,
        parts[2],
        " BOUNDARY ",
        vec![Line::from(
            "No messages or prompts. Activity outside the observed host remains unknown.",
        )],
        t,
    );
}
fn code(f: &mut Frame, area: Rect, a: &App, t: &Theme) {
    let parts = Layout::vertical([Constraint::Length(4), Constraint::Min(5)]).split(area);
    let age = a
        .observed
        .map(|at| format!("{}s ago", at.elapsed().as_secs()))
        .unwrap_or("never".into());
    para(
        f,
        parts[0],
        " CODE / MAGIC AUDIT ",
        vec![
            Line::styled(
                "Provider report · displayed without reinterpretation",
                t.accent(),
            ),
            Line::from(format!(
                "Last observation: {age} · A requests magic-cli audit"
            )),
        ],
        t,
    );
    let raw = if a.report.is_null() {
        "No audit report available.\n\nThe audit command belongs to Magic.\nBackstage will display its returned JSON here.\nNo complexity score or successful check is inferred.".into()
    } else {
        serde_json::to_string_pretty(&a.report).unwrap_or_default()
    };
    f.render_widget(
        Paragraph::new(raw)
            .block(panel(" AS REPORTED ", t))
            .scroll((a.scroll, 0))
            .wrap(Wrap { trim: false }),
        parts[1],
    );
}
fn mayhem(f: &mut Frame, area: Rect, m: &Metrics, t: &Theme) {
    let sections = Layout::vertical([
        Constraint::Length(4),
        Constraint::Length(7),
        Constraint::Min(5),
        Constraint::Length(3),
    ])
    .split(area);
    para(
        f,
        sections[0],
        " MAYHEM / THE ELECTRIC INSTRUMENT ROOM ",
        vec![
            Line::styled(
                "  ╲╱  ▄██▄  ╲╱     LIVE SYSTEM · NO PRODUCTION CONTROLS",
                t.accent(),
            ),
            Line::styled(
                "      ▀██▀         Sampling only while this view is visible",
                t.base().fg(t.magenta),
            ),
        ],
        t,
    );
    let top = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(sections[1]);
    let cpu = m
        .cpu
        .map(|v| format!(" CPU {v}% "))
        .unwrap_or(" CPU / WARMING UP ".into());
    let history: Vec<u64> = m.history.iter().copied().collect();
    f.render_widget(
        Sparkline::default()
            .block(panel(cpu, t))
            .data(&history)
            .max(100)
            .style(Style::default().fg(t.accent)),
        top[0],
    );
    let ratio = if m.memory_total > 0 {
        m.memory_used as f64 / m.memory_total as f64
    } else {
        0.0
    };
    let ram = if m.memory_total == 0 {
        "Not observed".into()
    } else {
        format!(
            "{:.1} / {:.1} GiB",
            m.memory_used as f64 / 1073741824.0,
            m.memory_total as f64 / 1073741824.0
        )
    };
    f.render_widget(
        Gauge::default()
            .block(panel(" MEMORY ", t))
            .gauge_style(Style::default().fg(t.magenta).bg(t.surface))
            .ratio(ratio.clamp(0.0, 1.0))
            .label(ram),
        top[1],
    );
    let lower = Layout::horizontal([Constraint::Percentage(62), Constraint::Percentage(38)])
        .split(sections[2]);
    let rows = m.processes.iter().map(|(pid, name, rss)| {
        Row::new([
            pid.to_string(),
            name.clone(),
            format!("{:.1} MiB", *rss as f64 / 1024.0),
        ])
    });
    f.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(9),
                Constraint::Min(12),
                Constraint::Length(12),
            ],
        )
        .header(Row::new(["PID", "PROCESS", "RSS"]).style(t.accent()))
        .block(panel(" TOP MEMORY / OBSERVED ", t)),
        lower[0],
    );
    let net = m
        .network
        .map(|(r, w)| {
            format!(
                "↓ {:.1} KiB/s\n↑ {:.1} KiB/s",
                r as f64 / 1024.0,
                w as f64 / 1024.0
            )
        })
        .unwrap_or("Waiting for second sample".into());
    let right = Layout::vertical([Constraint::Length(9), Constraint::Min(3)]).split(lower[1]);
    let labels: Vec<String> = (1..=m.cores.len()).map(|n| n.to_string()).collect();
    let cores: Vec<(&str, u64)> = labels
        .iter()
        .zip(&m.cores)
        .map(|(s, v)| (s.as_str(), *v))
        .collect();
    f.render_widget(
        BarChart::default()
            .block(panel(" CORES / FIRST 16 ", t))
            .data(&cores)
            .bar_width(2)
            .bar_gap(1)
            .max(100)
            .bar_style(Style::default().fg(t.blue))
            .value_style(t.base().fg(t.blue)),
        right[1],
    );
    para(
        f,
        right[0],
        " NETWORK & LOAD ",
        vec![
            Line::from(""),
            Line::styled("NETWORK", t.accent()),
            Line::from(net),
            Line::from(""),
            Line::styled("LOAD  1 / 5 / 15 MIN", t.accent()),
            Line::from(m.load.clone()),
            Line::from(""),
            Line::from("History: last 120 samples"),
            Line::from("Process list: top 12 by RSS"),
        ],
        t,
    );
    para(
        f,
        sections[3],
        " MEASUREMENTS ",
        vec![Line::from(m.error.clone().unwrap_or(
            "Linux /proc · 1 second cadence · stops on tab exit".into(),
        ))],
        t,
    );
}
