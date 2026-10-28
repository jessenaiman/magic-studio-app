use crate::{
    model::{App, text},
    system::Metrics,
    theme::Theme,
};
use ratatui::{prelude::*, widgets::*};
use serde_json::Value;

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

#[derive(Default)]
pub struct HitMap {
    pub tabs: Vec<(Rect, usize)>,
    pub projects: Vec<(Rect, usize)>,
    pub agents: Vec<(Rect, usize)>,
}

pub fn draw(
    f: &mut Frame,
    a: &App,
    t: &Theme,
    m: &Metrics,
    thumb: Option<&mut ratatui_image::protocol::StatefulProtocol>,
    hits: &mut HitMap,
) {
    hits.tabs.clear();
    hits.projects.clear();
    hits.agents.clear();
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
    let tall = size.height >= 28 && size.width >= 100;
    let areas = Layout::vertical([
        Constraint::Length(if tall { 7 } else { 3 }),
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(2),
    ])
    .split(size);
    if tall {
        let brand = Rect::new(
            areas[0].x + 1,
            areas[0].y,
            areas[0].width.saturating_sub(28),
            5,
        );
        crate::art::big(f, brand, "BACKSTAGE", t.accent);
        f.render_widget(
            Paragraph::new(format!(
                " ▪ ▪ ▪   {}   /   THE PRODUCTION DESK",
                if t.name == "daylight" {
                    "DAYLIGHT PLAYHOUSE"
                } else if t.name == "midnight" {
                    "MIDNIGHT BROADCAST"
                } else {
                    &t.name
                }
            ))
            .style(t.base().fg(t.muted)),
            Rect::new(areas[0].x, areas[0].y + 5, areas[0].width, 1),
        );
        crate::art::moppet(
            f,
            Rect::new(areas[0].right().saturating_sub(23), areas[0].y, 16, 6),
            t,
            0,
            if a.busy { "working" } else { "idle" },
            animation_frame(a),
        );
        f.render_widget(
            Paragraph::new("OBSERVE · REVIEW").style(t.accent()),
            Rect::new(areas[0].right().saturating_sub(24), areas[0].y + 6, 24, 1),
        );
    } else {
        f.render_widget(
            Paragraph::new(vec![
                Line::styled(" BACKSTAGE / PRODUCTION DESK", t.accent()),
                Line::from(format!(" {} · observer only", t.name)),
            ]),
            areas[0],
        );
    }
    let mut tab_x = areas[1].x + 1;
    for (index, label) in TABS.iter().enumerate() {
        let width = (label.len() + 4) as u16;
        hits.tabs.push((
            Rect::new(tab_x, areas[1].y + 1, width, 1).intersection(areas[1]),
            index,
        ));
        tab_x = tab_x.saturating_add(width + 1);
    }
    f.render_widget(
        Tabs::new(
            TABS.iter()
                .enumerate()
                .map(|(i, n)| format!("{} {n}", i + 1)),
        )
        .block(Block::bordered().border_style(Style::default().fg(t.accent)))
        .select(a.tab)
        .highlight_style(t.base().bg(t.accent).fg(t.bg).bold())
        .divider(" ")
        .padding(" ", " "),
        areas[1],
    );
    match a.tab {
        0 => overview(f, areas[2], a, t, hits),
        1 => booth(f, areas[2], a, t, thumb),
        2 => mops(f, areas[2], a, t, hits),
        3 => code(f, areas[2], a, t),
        _ => mayhem(f, areas[2], a, m, t),
    };
    let status = if a.searching {
        format!(" /{}▏   Enter to finish · Esc clears", a.query)
    } else {
        format!(
            " [A] Audit  [R] Refresh  [/] Find  [1–5/←→/click] Views  [D] Details  [?] Help  [T] Theme  [Q] Exit  {}",
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
    if a.help || a.detail {
        details(f, size, a, t);
    }
}
fn overview(f: &mut Frame, area: Rect, a: &App, t: &Theme, hits: &mut HitMap) {
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
            .height(1)
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
    .row_highlight_style(t.base().bg(t.accent).fg(t.bg).bold())
    .highlight_symbol("▸ ")
    .block(panel(" TODAY'S CALL SHEET ", t));
    let mut selection = TableState::default().with_selected(if projects.is_empty() {
        None
    } else {
        Some(a.selected.min(projects.len() - 1))
    });
    f.render_stateful_widget(table, parts[0], &mut selection);
    let body = Rect::new(
        parts[0].x + 1,
        parts[0].y + 3,
        parts[0].width.saturating_sub(2),
        parts[0].height.saturating_sub(4),
    );
    for (row, index) in (selection.offset()..projects.len())
        .take(body.height as usize)
        .enumerate()
    {
        hits.projects.push((
            Rect::new(body.x, body.y + row as u16, body.width, 1).intersection(body),
            index,
        ));
    }

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
        let block = panel(" PRODUCTIVITY SNAPSHOT ", t);
        let inside = block.inner(p);
        f.render_widget(block, p);
        let sections = Layout::vertical([
            Constraint::Length(8),
            Constraint::Length(5),
            Constraint::Min(3),
        ])
        .split(inside);
        let counters = Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(sections[0]);
        crate::art::big(
            f,
            Rect::new(
                counters[0].x + 2,
                counters[0].y + 1,
                counters[0].width.saturating_sub(3),
                5,
            ),
            "—",
            t.accent,
        );
        f.render_widget(
            Paragraph::new(" RELEASED TOTAL").style(t.accent()),
            Rect::new(counters[0].x, counters[0].y + 6, counters[0].width, 1),
        );
        f.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::styled("—  TODAY", t.accent()),
                Line::from(""),
                Line::styled("—  DAILY GOAL", t.accent()),
                Line::from(""),
                Line::styled("—  IN REVIEW", t.accent()),
            ]),
            counters[1],
        );
        para(
            f,
            sections[1],
            " LAST 7 DAYS ",
            vec![
                Line::from("Release history unavailable"),
                Line::styled("No release source has been defined.", t.base().fg(t.muted)),
            ],
            t,
        );
        let guide = sections[2];
        if guide.height >= 8 && guide.width >= 36 {
            crate::art::moppet(
                f,
                Rect::new(guide.x + 1, guide.y, 16, guide.height.min(10)),
                t,
                0,
                if a.busy { "working" } else { "idle" },
                animation_frame(a),
            );
            f.render_widget(
                Paragraph::new(vec![
                    Line::styled("ON YOUR SIDE", t.accent()),
                    Line::from(""),
                    Line::from("A requests the genuine audit."),
                    Line::from("? shows source coverage."),
                    Line::from(""),
                    Line::styled("Unknown is not zero.", t.base().fg(t.yellow)),
                ])
                .wrap(Wrap { trim: false }),
                Rect::new(
                    guide.x + 19,
                    guide.y + 1,
                    guide.width.saturating_sub(20),
                    guide.height.saturating_sub(1),
                ),
            );
        } else {
            f.render_widget(
                Paragraph::new("No release counts inferred. A: audit · ?: sources")
                    .wrap(Wrap { trim: false }),
                guide,
            );
        }
    }
}
fn booth(
    f: &mut Frame,
    area: Rect,
    a: &App,
    t: &Theme,
    thumb: Option<&mut ratatui_image::protocol::StatefulProtocol>,
) {
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
    let block = panel(
        format!(" STILL / {} / NOT HASH-VERIFIED ", a.graphics_status),
        t,
    );
    let inside = block.inner(cols[0]);
    f.render_widget(block, cols[0]);
    if let Some(img) = thumb {
        f.render_stateful_widget(
            ratatui_image::StatefulImage::default().resize(ratatui_image::Resize::Fit(Some(
                image::imageops::FilterType::Lanczos3,
            ))),
            inside,
            img,
        );
    } else {
        f.render_widget(Paragraph::new(format!("\n  No cached image displayed.\n\n  {}\n\n  A readable catalog still is required.\n  Backstage does not generate video frames.", a.graphics_status)).wrap(Wrap{trim:false}),inside);
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
        let report = &a.project_report;
        let observation = &report["observation"];
        lines = vec![
            Line::styled("SAVED PROJECT FILE / POINT-IN-TIME READ", t.accent()),
            Line::from(""),
            Line::from(format!("Saved name: {}", value(&report["name"]))),
            Line::from(format!(
                "Asset records: {}",
                report["assets"]
                    .as_array()
                    .map(|a| a.len().to_string())
                    .unwrap_or("Not reported".into())
            )),
            Line::from(format!(
                "Observed (Unix µs): {}",
                value(&observation["observed_unix_micros"])
            )),
            Line::from(""),
            Line::styled("SAVED BYTES / SHA-256", t.accent()),
            Line::from(value(&observation["sha256"])),
            Line::from(""),
            Line::from(format!(
                "Live unsaved state: {}",
                value(&observation["live_unsaved_state"])
            )),
            Line::from(format!(
                "External files: {}",
                value(&observation["external_files"])
            )),
            Line::from("Motion proof / release totals: not reported"),
            Line::from(""),
            Line::styled("SAVED WORKSHOP / AS REPORTED", t.accent()),
        ];
        let workshop = serde_json::to_string_pretty(&report["workshop"]).unwrap_or_default();
        lines.extend(workshop.lines().map(|line| Line::from(line.to_owned())));
    } else if !a.project_error.is_empty() {
        lines.push(Line::styled(a.project_error.clone(), t.base().fg(t.red)));
    }
    lines.push(Line::styled(
        "R refreshes this selected project",
        t.base().fg(t.muted),
    ));
    f.render_widget(
        Paragraph::new(lines)
            .block(panel(" REVIEW DESK / SCROLL FOR DETAILS ", t))
            .wrap(Wrap { trim: false })
            .scroll((a.scroll, 0)),
        cols[1],
    );
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
fn animation_frame(a: &App) -> usize {
    if a.reduce_motion {
        return 0;
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_millis() / 250) as usize % 4)
        .unwrap_or(0)
}
fn utc(seconds: u64) -> String {
    format!(
        "{:02}:{:02}:{:02} UTC",
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    )
}
fn agent_name(p: &Value) -> String {
    p["name"]
        .as_str()
        .or(p["display_agent"].as_str())
        .or(p["agent"].as_str())
        .map(|s| value(&Value::String(s.into())))
        .unwrap_or("Unknown".into())
}
fn mops(f: &mut Frame, area: Rect, a: &App, t: &Theme, hits: &mut HitMap) {
    let parts = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(3),
    ])
    .split(area);
    let working = a
        .agents
        .iter()
        .filter(|p| p["agent_status"] == "working")
        .count();
    para(
        f,
        parts[0],
        " MOPS / THE CAST ",
        vec![Line::from(format!(
            "{} observed · {} working · Last read {}",
            a.agents.len(),
            working,
            a.agents_observed_at
                .map(utc)
                .unwrap_or("unavailable".into())
        ))],
        t,
    );
    let cols = if area.width >= 140 {
        3
    } else if area.width >= 90 {
        2
    } else {
        1
    };
    let rows = if parts[1].height >= 26 { 2 } else { 1 };
    let per_page = cols * rows;
    if a.agents.is_empty() {
        para(
            f,
            parts[1],
            " PERFORMERS ",
            vec![
                Line::from(a.agent_status.clone()),
                Line::from("No agent activity is invented for unobserved hosts."),
            ],
            t,
        );
    } else {
        let start = a.agent_selected / per_page * per_page;
        let height = parts[1].height / rows as u16;
        let width = parts[1].width / cols as u16;
        for (index, p) in a.agents.iter().enumerate().skip(start).take(per_page) {
            let slot = index - start;
            let rect = Rect::new(
                parts[1].x + (slot % cols) as u16 * width,
                parts[1].y + (slot / cols) as u16 * height,
                width,
                height,
            );
            hits.agents.push((rect, index));
            let selected = index == a.agent_selected;
            let name = agent_name(p);
            let state = p["agent_status"].as_str().unwrap_or("unknown");
            let ink = match state {
                "working" => t.green,
                "blocked" => t.yellow,
                "done" => t.blue,
                _ => t.muted,
            };
            let block = panel(format!(" {}{} ", if selected { "▸ " } else { "" }, name), t)
                .border_style(Style::default().fg(if selected { t.accent } else { t.muted }));
            let inner = block.inner(rect);
            f.render_widget(block, rect);
            crate::art::moppet(
                f,
                Rect::new(inner.x + 1, inner.y + 1, 16, 10).intersection(inner),
                t,
                crate::art::seed(p["pane_id"].as_str().unwrap_or(&name)),
                state,
                animation_frame(a),
            );
            let info = Rect::new(
                inner.x + 19,
                inner.y + 1,
                inner.width.saturating_sub(20),
                inner.height.saturating_sub(1),
            );
            f.render_widget(
                Paragraph::new(vec![
                    Line::styled(
                        format!("● {}", state.to_uppercase()),
                        t.base().fg(ink).bold(),
                    ),
                    Line::from(value(&p["pane_id"])),
                    Line::from(""),
                    Line::styled("OBSERVED TITLE", t.accent()),
                    Line::from(value(&p["terminal_title_stripped"])),
                    Line::from(""),
                    Line::styled("WORKSPACE", t.accent()),
                    Line::from(short_path(&p["foreground_cwd"])),
                ])
                .wrap(Wrap { trim: false }),
                info,
            );
        }
    }
    para(
        f,
        parts[2],
        " OBSERVER BOUNDARY ",
        vec![Line::from(format!(
            "↑↓ / wheel chooses performer · D inspects record · page {}/{} · no messages or prompts",
            a.agent_selected / per_page + 1,
            a.agents.len().div_ceil(per_page).max(1)
        ))],
        t,
    );
}
fn details(f: &mut Frame, area: Rect, a: &App, t: &Theme) {
    let rect = Rect::new(
        area.x + 3,
        area.y + 2,
        area.width.saturating_sub(6),
        area.height.saturating_sub(4),
    );
    f.render_widget(Clear, rect);
    let (title, raw) = if a.help {
        (
            " BACKSTAGE / HELP & OBSERVATION COVERAGE ",
            format!(
                "NAVIGATION\nClick tabs, 1–5, F1–F5, ←→ or Tab select a view.\n↑↓ / mouse wheel select a video or performer; Enter opens inspection.\n/ filters videos. D shows the source record. A requests Magic audit.\nR refreshes observations. T switches the shared themes. Q exits.\nESC closes this panel; PgUp/PgDn scroll.\n\nCHANGE NOTIFICATIONS\n{}\n\nProvider metadata (as reported):\n{}\n\nMissing data remains unavailable. Saved-file reads do not prove live motion, a release, or external-file freshness. Backstage never sends prompts or changes production.",
                a.notification_status,
                serde_json::to_string_pretty(&a.changes).unwrap_or_default()
            ),
        )
    } else {
        let record = match a.tab {
            0 => a.project().unwrap_or(Value::Null),
            1 => a.project_report.clone(),
            2 => a
                .agents
                .get(a.agent_selected)
                .cloned()
                .unwrap_or(Value::Null),
            3 => a.report.clone(),
            _ => a.changes.clone(),
        };
        (
            " OBSERVED RECORD / ESC CLOSES ",
            serde_json::to_string_pretty(&record).unwrap_or_default(),
        )
    };
    f.render_widget(
        Paragraph::new(raw)
            .block(panel(title, t).border_style(Style::default().fg(t.accent)))
            .wrap(Wrap { trim: false })
            .scroll((a.detail_scroll, 0)),
        rect,
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
    let report_area = if let Some(fields) = a.report.as_object() {
        let split = Layout::horizontal([Constraint::Percentage(38), Constraint::Percentage(62)])
            .split(parts[1]);
        let rows = fields
            .iter()
            .skip(usize::from(a.scroll))
            .map(|(key, value)| Row::new([key.clone(), text(value)]));
        f.render_widget(
            Table::new(
                rows,
                [Constraint::Percentage(40), Constraint::Percentage(60)],
            )
            .header(Row::new(["SOURCE FIELD", "REPORTED VALUE"]).style(t.accent()))
            .block(panel(" REPORT FIELDS / NO DERIVED SCORES ", t)),
            split[0],
        );
        split[1]
    } else {
        parts[1]
    };
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
        report_area,
    );
}
fn mayhem(f: &mut Frame, area: Rect, a: &App, m: &Metrics, t: &Theme) {
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
    crate::art::moppet(
        f,
        Rect::new(sections[0].right().saturating_sub(19), sections[0].y, 16, 4),
        t,
        2,
        if m.cpu.is_some() { "working" } else { "idle" },
        m.samples as usize,
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
    let memory = Layout::vertical([Constraint::Length(3), Constraint::Min(3)]).split(top[1]);
    let memory_history: Vec<u64> = m.memory_history.iter().copied().collect();
    f.render_widget(
        Sparkline::default()
            .block(panel(" RAM / VISIBLE SAMPLES ", t))
            .data(&memory_history)
            .max(100)
            .style(Style::default().fg(t.magenta)),
        memory[1],
    );
    f.render_widget(
        Gauge::default()
            .block(panel(" MEMORY ", t))
            .gauge_style(Style::default().fg(t.magenta).bg(t.surface))
            .ratio(ratio.clamp(0.0, 1.0))
            .label(ram),
        memory[0],
    );
    let lower = Layout::horizontal([Constraint::Percentage(62), Constraint::Percentage(38)])
        .split(sections[2]);
    let rows = m
        .processes
        .iter()
        .skip(usize::from(a.scroll).min(m.processes.len().saturating_sub(1)))
        .map(|(pid, name, rss, cpu)| {
            Row::new([
                pid.to_string(),
                name.clone(),
                cpu.map(|v| format!("{:.1}%", v as f64 / 10.0))
                    .unwrap_or("…".into()),
                format!("{:.1} MiB", *rss as f64 / 1024.0),
            ])
        });
    f.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(9),
                Constraint::Min(12),
                Constraint::Length(9),
                Constraint::Length(12),
            ],
        )
        .header(Row::new(["PID", "PROCESS", "CPU / CORE", "RSS"]).style(t.accent()))
        .block(panel(" PROCESSES / SORTED BY RSS · SCROLL ", t)),
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
            Line::from(net.lines().next().unwrap_or("Not reported").to_owned()),
            Line::from(net.lines().nth(1).unwrap_or("").to_owned()),
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
        vec![Line::from(m.error.clone().unwrap_or(format!(
            "Linux /proc · {} processes · uptime {}h {}m · 1s visible-only samples",
            m.process_count,
            m.uptime / 3600,
            m.uptime % 3600 / 60
        )))],
        t,
    );
}
