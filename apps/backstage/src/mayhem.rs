//! Mayhem is an observer-only inventory view. Unknown is never rendered as zero.
use std::path::PathBuf;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Paragraph, Wrap},
};

use crate::{
    App,
    project_data::{FolderRecord, ProjectRecord, ProjectSnapshot, WorktreeRecord},
};

const BG: Color = Color::Rgb(24, 15, 27);
const IVORY: Color = Color::Rgb(242, 222, 188);
const GOLD: Color = Color::Rgb(245, 186, 56);
const CORAL: Color = Color::Rgb(176, 126, 125);
const BORDER: Color = Color::Rgb(202, 169, 115);
const CYAN: Color = Color::Rgb(133, 210, 222);
const MUTED: Color = Color::Rgb(160, 149, 169);
const SELECT: Color = Color::Rgb(65, 44, 34);

pub fn draw(f: &mut Frame, area: Rect, app: &mut App) {
    let mut opens = Vec::new();
    let mut project_rows = Vec::new();
    let mut worktree_rows = Vec::new();
    let data = &app.project_data;
    if area.width < 120 || area.height < 26 {
        let body = panel(f, area, "MAYHEM", "read-only inventory");
        f.render_widget(Paragraph::new("Enlarge the terminal to inspect project and worktree inventory.\nNo agent or model is contacted.").wrap(Wrap{trim:false}).style(Style::default().fg(IVORY)),body);
        return;
    }
    let bands = Layout::vertical([
        Constraint::Percentage(39),
        Constraint::Percentage(29),
        Constraint::Percentage(32),
    ])
    .spacing(1)
    .split(area);
    let upper = Layout::horizontal([Constraint::Percentage(49), Constraint::Percentage(51)])
        .spacing(1)
        .split(bands[0]);
    projects(
        f,
        upper[0],
        data,
        app.project_selected,
        &mut opens,
        &mut project_rows,
    );
    worktrees(
        f,
        upper[1],
        data,
        app.worktree_selected,
        &mut opens,
        &mut worktree_rows,
    );
    if data.projects.is_empty() && !data.worktrees.is_empty() {
        selected_git_project(f, bands[1], data, app.worktree_selected, &mut opens);
    } else {
        selected_project(
            f,
            bands[1],
            data.projects.get(app.project_selected),
            &mut opens,
        );
    }
    let lower = Layout::horizontal([Constraint::Percentage(48), Constraint::Percentage(52)])
        .spacing(1)
        .split(bands[2]);
    tokens(
        f,
        lower[0],
        data.projects
            .get(app.project_selected)
            .map(|p| p.name.as_str())
            .or_else(|| data.worktrees.first().map(|w| w.project.as_str())),
    );
    selected_worktree(
        f,
        lower[1],
        data.worktrees.get(app.worktree_selected),
        &mut opens,
    );
    app.project_buttons.extend(opens);
    app.project_rows.extend(project_rows);
    app.worktree_rows.extend(worktree_rows);
}

fn panel(f: &mut Frame, area: Rect, title: &str, source: &str) -> Rect {
    f.render_widget(
        Block::bordered()
            .border_style(Style::default().fg(BORDER))
            .style(Style::default().bg(BG)),
        area,
    );
    let title_width = (Span::raw(title).width() as u16 + 4)
        .max(20)
        .min(area.width.saturating_mul(2) / 3);
    label(
        f,
        Rect::new(area.x + 2, area.y + 1, title_width.saturating_sub(2), 1),
        title,
        GOLD,
        true,
    );
    let source_x = area.x + title_width + 2;
    label(
        f,
        Rect::new(
            source_x,
            area.y + 1,
            area.width.saturating_sub(title_width + 4),
            1,
        ),
        source,
        CYAN,
        false,
    );
    rule(
        f,
        Rect::new(area.x + 1, area.y + 2, area.width.saturating_sub(2), 1),
    );
    Rect::new(
        area.x + 1,
        area.y + 3,
        area.width.saturating_sub(2),
        area.height.saturating_sub(4),
    )
}

fn label(f: &mut Frame, area: Rect, value: &str, color: Color, bold: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let mut value: String = value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if Span::raw(&value).width() > usize::from(area.width) {
        while !value.is_empty() && Span::raw(&value).width() >= usize::from(area.width) {
            value.pop();
        }
        value.push('…');
    }
    let style = Style::default().fg(color).add_modifier(if bold {
        Modifier::BOLD
    } else {
        Modifier::empty()
    });
    f.render_widget(Paragraph::new(value).style(style), area);
}

fn columns(area: Rect, widths: &[u16]) -> Vec<Rect> {
    Layout::horizontal(widths.iter().map(|w| Constraint::Percentage(*w)))
        .split(area)
        .iter()
        .map(|r| Rect::new(r.x + 1, r.y, r.width.saturating_sub(2), r.height))
        .collect()
}

fn header(f: &mut Frame, area: Rect, widths: &[u16], titles: &[&str]) {
    for (cell, title) in columns(area, widths).iter().zip(titles) {
        label(f, *cell, title, IVORY, false);
    }
}

fn rule(f: &mut Frame, area: Rect) {
    f.render_widget(
        Paragraph::new("─".repeat(usize::from(area.width))).style(Style::default().fg(CORAL)),
        area,
    );
}

fn button(f: &mut Frame, area: Rect, enabled: bool) {
    let color = if enabled { GOLD } else { MUTED };
    f.render_widget(
        Block::bordered().border_style(Style::default().fg(color)),
        area,
    );
    if area.height > 2 {
        f.render_widget(
            Paragraph::new("Open")
                .alignment(ratatui::layout::Alignment::Center)
                .style(Style::default().fg(color)),
            Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), 1),
        );
    }
}

fn size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn project_size(project: &ProjectRecord) -> Option<u64> {
    if !project.complete {
        return None;
    }
    project
        .files
        .iter()
        .try_fold(0_u64, |total, file| total.checked_add(file.bytes?))
}

fn project_in_repo(project: &ProjectRecord, data: &ProjectSnapshot) -> bool {
    if data
        .worktrees
        .iter()
        .any(|w| project.path.starts_with(&w.path))
    {
        return true;
    }
    std::env::current_dir()
        .ok()
        .and_then(|cwd| {
            cwd.ancestors()
                .find(|p| p.join(".git").exists())
                .map(std::path::Path::to_path_buf)
        })
        .is_some_and(|root| project.path.starts_with(&root))
}

fn projects(
    f: &mut Frame,
    area: Rect,
    data: &ProjectSnapshot,
    selected: usize,
    opens: &mut Vec<(Rect, PathBuf)>,
    rows: &mut Vec<(Rect, usize)>,
) {
    let git_group = data.projects.is_empty() && !data.worktrees.is_empty();
    let title = if git_group {
        "PROJECTS  (1 observed)".into()
    } else if data.projects.is_empty() {
        "PROJECTS  (?)".into()
    } else {
        format!("PROJECTS  ({})", data.projects.len())
    };
    let source = if git_group {
        "Git repository group • totals unavailable"
    } else {
        &data.source
    };
    let body = panel(f, area, &title, source);
    let widths = [5, 23, 15, 11, 13, 21, 12];
    header(
        f,
        Rect::new(body.x, body.y, body.width, 1),
        &widths,
        &["#", "NAME", "WORKTREES", "FILES", "SIZE", "USAGE", ""],
    );
    if git_group {
        draw_git_group_project_row(f, body, data, &widths, opens, rows);
        return;
    }
    if data.projects.is_empty() {
        draw_empty_projects(f, body, data.error.as_deref());
        return;
    }
    let max_project_bytes = data.projects.iter().filter_map(project_size).max();
    let capacity = usize::from(body.height.saturating_sub(2) / 3).max(1);
    let start = selected.saturating_sub(capacity.saturating_sub(1));
    for (slot, (index, project)) in data
        .projects
        .iter()
        .enumerate()
        .skip(start)
        .take(capacity)
        .enumerate()
    {
        let row = Rect::new(body.x, body.y + 2 + slot as u16 * 3, body.width, 3);
        if row.y + 2 >= body.y + body.height {
            break;
        }
        rows.push((row, index));
        draw_project_row(
            f,
            row,
            data,
            project,
            (index, index == selected, max_project_bytes),
            &widths,
            opens,
        );
    }
}

fn draw_git_group_project_row(
    f: &mut Frame,
    body: Rect,
    data: &ProjectSnapshot,
    widths: &[u16],
    opens: &mut Vec<(Rect, PathBuf)>,
    rows: &mut Vec<(Rect, usize)>,
) {
    let repository = &data.worktrees[0];
    let row = Rect::new(body.x, body.y + 2, body.width, 3);
    f.render_widget(
        Block::bordered()
            .border_style(Style::default().fg(GOLD))
            .style(Style::default().bg(SELECT)),
        row,
    );
    rows.push((row, 0));
    let cells = columns(Rect::new(row.x, row.y + 1, row.width, 1), widths);
    for (cell, value) in cells.iter().take(6).zip([
        "›1".into(),
        repository.project.clone(),
        data.worktrees.len().to_string(),
        "n/a".into(),
        "n/a".into(),
        "Unavailable".into(),
    ]) {
        label(f, *cell, &value, GOLD, false);
    }
    let action = Rect::new(cells[6].x, row.y, cells[6].width.max(5), 3);
    button(f, action, repository.available);
    if repository.available {
        opens.push((action, repository.path.clone()));
    }
    label(
        f,
        Rect::new(body.x + 2, body.y + 6, body.width.saturating_sub(4), 1),
        "Explicit repository scope; shared worktrees are not summed.",
        MUTED,
        false,
    );
}

fn draw_empty_projects(f: &mut Frame, body: Rect, error: Option<&str>) {
    let row = Rect::new(body.x + 1, body.y + 3, body.width.saturating_sub(15), 1);
    label(
        f,
        row,
        "Unavailable — no authoritative project inventory observed",
        GOLD,
        false,
    );
    button(
        f,
        Rect::new(body.x + body.width.saturating_sub(12), body.y + 2, 11, 3),
        false,
    );
    if let Some(error) = error {
        label(
            f,
            Rect::new(body.x + 2, body.y + 6, body.width.saturating_sub(4), 1),
            error,
            MUTED,
            false,
        );
    }
}

fn draw_project_row(
    f: &mut Frame,
    row: Rect,
    data: &ProjectSnapshot,
    project: &ProjectRecord,
    (index, active, max_project_bytes): (usize, bool, Option<u64>),
    widths: &[u16],
    opens: &mut Vec<(Rect, PathBuf)>,
) {
    if active {
        f.render_widget(
            Block::bordered()
                .border_style(Style::default().fg(GOLD))
                .style(Style::default().bg(SELECT)),
            row,
        );
    }
    let cells = columns(Rect::new(row.x, row.y + 1, row.width, 1), widths);
    let color = if active { GOLD } else { IVORY };
    label(f, cells[0], &(index + 1).to_string(), color, false);
    label(f, cells[1], &project.name, color, active);
    let in_repo = project_in_repo(project, data);
    let worktree_count = if in_repo {
        data.worktrees.len().to_string()
    } else {
        "—".into()
    };
    label(
        f,
        cells[2],
        &worktree_count,
        if in_repo { IVORY } else { MUTED },
        false,
    );
    let files_label = if project.complete {
        project.files.len().to_string()
    } else {
        "n/a".into()
    };
    label(f, cells[3], &files_label, IVORY, false);
    let bytes = project_size(project);
    let size_label = bytes.map(size).unwrap_or_else(|| "n/a".into());
    label(f, cells[4], &size_label, IVORY, false);
    if bytes.is_some() {
        bar_with_percent(f, cells[5], bytes, max_project_bytes, CYAN);
    } else {
        label(f, cells[5], "Unavailable", MUTED, false);
    }
    let action = Rect::new(cells[6].x, row.y, cells[6].width.max(5), 3);
    button(f, action, project.exists);
    if project.exists {
        opens.push((action, project.path.clone()));
    }
}

fn worktrees(
    f: &mut Frame,
    area: Rect,
    data: &ProjectSnapshot,
    selected: usize,
    opens: &mut Vec<(Rect, PathBuf)>,
    rows: &mut Vec<(Rect, usize)>,
) {
    let title = if data.worktrees.is_empty() {
        "WORKTREES  (?)".into()
    } else {
        format!("WORKTREES  ({})", data.worktrees.len())
    };
    let source = if data.worktree_source.is_empty() {
        "Git source unavailable"
    } else {
        "Git tracked paths • bars: max observed bytes"
    };
    let body = panel(f, area, &title, source);
    let widths = [4, 14, 14, 16, 8, 12, 20, 12];
    header(
        f,
        Rect::new(body.x, body.y, body.width, 1),
        &widths,
        &[
            "#", "PROJECT", "NAME", "BRANCH", "FILES", "SIZE", "USAGE", "",
        ],
    );
    if data.worktrees.is_empty() {
        draw_empty_worktrees(f, body, data.worktree_error.as_deref());
        return;
    }
    let maximum = data.worktrees.iter().filter_map(|w| w.bytes).max();
    let capacity = usize::from(body.height.saturating_sub(2) / 3).max(1);
    let start = selected.saturating_sub(capacity.saturating_sub(1));
    for (slot, (index, worktree)) in data
        .worktrees
        .iter()
        .enumerate()
        .skip(start)
        .take(capacity)
        .enumerate()
    {
        let row = Rect::new(body.x, body.y + 2 + slot as u16 * 3, body.width, 3);
        if row.y + 2 >= body.y + body.height {
            break;
        }
        rows.push((row, index));
        draw_worktree_row(
            f,
            row,
            worktree,
            (index, index == selected, maximum),
            &widths,
            opens,
        );
    }
}

fn draw_empty_worktrees(f: &mut Frame, body: Rect, error: Option<&str>) {
    label(
        f,
        Rect::new(body.x + 2, body.y + 3, body.width.saturating_sub(16), 1),
        "Unavailable — no scoped worktree inventory",
        GOLD,
        false,
    );
    button(
        f,
        Rect::new(body.x + body.width.saturating_sub(12), body.y + 2, 11, 3),
        false,
    );
    if let Some(error) = error {
        label(
            f,
            Rect::new(body.x + 2, body.y + 6, body.width.saturating_sub(4), 1),
            error,
            MUTED,
            false,
        );
    }
}

fn draw_worktree_row(
    f: &mut Frame,
    row: Rect,
    worktree: &WorktreeRecord,
    (index, active, maximum): (usize, bool, Option<u64>),
    widths: &[u16],
    opens: &mut Vec<(Rect, PathBuf)>,
) {
    let color = if active { GOLD } else { IVORY };
    if active {
        f.render_widget(
            Block::bordered()
                .border_style(Style::default().fg(GOLD))
                .style(Style::default().bg(SELECT)),
            row,
        );
    }
    let cells = columns(Rect::new(row.x, row.y + 1, row.width, 1), widths);
    label(f, cells[0], &(index + 1).to_string(), color, false);
    label(
        f,
        cells[1],
        &worktree.project,
        if active { GOLD } else { CYAN },
        active,
    );
    label(f, cells[2], &worktree.name, IVORY, false);
    label(f, cells[3], &worktree.branch, IVORY, false);
    let files_text = worktree
        .files
        .map(|n| format!("{n}{}", if worktree.partial { "+" } else { "" }))
        .unwrap_or_else(|| "n/a".into());
    label(f, cells[4], &files_text, IVORY, false);
    let size_text = worktree
        .bytes
        .map(|n| format!("{}{}", if worktree.partial { "~" } else { "" }, size(n)))
        .unwrap_or_else(|| "n/a".into());
    label(f, cells[5], &size_text, IVORY, false);
    bar_with_percent(
        f,
        cells[6],
        worktree.bytes,
        maximum,
        [
            Color::Rgb(230, 119, 126),
            Color::Rgb(112, 196, 149),
            GOLD,
            CYAN,
        ][index % 4],
    );
    let action = Rect::new(cells[7].x, row.y, cells[7].width.max(5), 3);
    button(f, action, worktree.available);
    if worktree.available {
        opens.push((action, worktree.path.clone()));
    }
}

fn selected_project(
    f: &mut Frame,
    area: Rect,
    project: Option<&ProjectRecord>,
    opens: &mut Vec<(Rect, PathBuf)>,
) {
    let title = format!(
        "SELECTED PROJECT: {}",
        project.map(|p| p.name.as_str()).unwrap_or("Unavailable")
    );
    let body = panel(
        f,
        area,
        &title,
        "source: saved inventory • partial until verified",
    );
    let split =
        Layout::horizontal([Constraint::Percentage(54), Constraint::Percentage(46)]).split(body);
    let left = split[0];
    label(f, Rect::new(left.x + 2, left.y + 2, 5, 3), "▣", CORAL, true);
    let content_x = left.x + 10;
    let width = left.width.saturating_sub(12);
    let path = project
        .map(|p| p.path.display().to_string())
        .unwrap_or_else(|| "Unavailable".into());
    for (offset, (name, value)) in [
        ("Path:", path),
        ("Worktrees:", "Unavailable".into()),
        (
            "Files:",
            project
                .filter(|p| p.complete)
                .map(|p| p.files.len().to_string())
                .unwrap_or_else(|| "Unavailable".into()),
        ),
        (
            "Size:",
            project
                .and_then(project_size)
                .map(size)
                .unwrap_or_else(|| "Unavailable".into()),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let y = left.y + 1 + offset as u16 * 2;
        if y >= left.y + left.height {
            break;
        }
        label(f, Rect::new(content_x, y, 12, 1), name, IVORY, false);
        label(
            f,
            Rect::new(content_x + 12, y, width.saturating_sub(12), 1),
            &value,
            if offset == 0 { CYAN } else { IVORY },
            false,
        );
    }
    let right = split[1];
    label(
        f,
        Rect::new(right.x + 1, right.y, right.width.saturating_sub(2), 1),
        "FOLDER             SIZE        USAGE",
        IVORY,
        false,
    );
    label(
        f,
        Rect::new(right.x + 1, right.y + 3, right.width.saturating_sub(2), 1),
        "Unavailable — directory totals not observed",
        GOLD,
        false,
    );
    if let Some(project) = project {
        if let Some(error) = &project.error {
            label(
                f,
                Rect::new(right.x + 1, right.y + 5, right.width.saturating_sub(2), 1),
                error,
                MUTED,
                false,
            );
        }
        let action = Rect::new(
            right.x + right.width.saturating_sub(12),
            right.y + right.height.saturating_sub(3),
            11,
            3,
        );
        button(f, action, project.exists);
        if project.exists {
            opens.push((action, project.path.clone()));
        }
    }
}

fn read_saved_analytics_json(name: &str) -> Option<serde_json::Value> {
    let path = crate::audit::default_analytics_path(name)?;
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn draw_saved_mayhem_metrics(
    f: &mut Frame,
    area: Rect,
    audit: Option<&serde_json::Value>,
    lizard: Option<&serde_json::Value>,
) {
    let comp = audit
        .and_then(|v| v.get("complexity"))
        .or(lizard)
        .unwrap_or(&serde_json::Value::Null);
    let rev = audit
        .and_then(|v| v["source_revision"].as_str())
        .or_else(|| comp["source_revision"].as_str())
        .unwrap_or("unknown");
    let source = format!("saved target/analytics • rev {rev} • press a for tool details");
    let body = panel(f, area, "MAYHEM AUDIT & CODE METRICS", &source);
    let checks = audit
        .and_then(|a| {
            Some(format!(
                "{} / {} passed ({} failed)",
                a["passed"].as_u64()?,
                a["total"].as_u64()?,
                a["failed"].as_u64().unwrap_or(0)
            ))
        })
        .unwrap_or_else(|| "Run magic-cli audit".into());
    let rows = [
        ("Audit checks:", checks),
        (
            "Functions:",
            format!(
                "{} analyzed • {} over CCN 15 • max CCN {}",
                comp["functions_analyzed"].as_u64().unwrap_or(0),
                comp["functions_over_ccn_15"].as_u64().unwrap_or(0),
                comp["max_ccn"].as_u64().unwrap_or(0)
            ),
        ),
        (
            "Source lines:",
            format!(
                "{} lines across {} files ({} long >2k)",
                comp["total_source_lines"].as_u64().unwrap_or(0),
                comp["files_analyzed"].as_u64().unwrap_or(0),
                comp["long_files_over_2000"].as_u64().unwrap_or(0)
            ),
        ),
        (
            "Top hotspot:",
            comp["hotspots"]
                .as_array()
                .and_then(|h| h.first())
                .map(|top| {
                    format!(
                        "{} (CCN {}) • {}:{}",
                        top["name"].as_str().unwrap_or("?"),
                        top["ccn"].as_u64().unwrap_or(0),
                        top["file"].as_str().unwrap_or("?"),
                        top["start_line"].as_u64().unwrap_or(0)
                    )
                })
                .unwrap_or_else(|| "None".into()),
        ),
    ];
    for (idx, (k, v)) in rows.iter().enumerate() {
        let y = body.y + idx as u16 * 2;
        if y >= body.y + body.height {
            break;
        }
        label(f, Rect::new(body.x + 1, y, 15, 1), k, IVORY, false);
        label(
            f,
            Rect::new(body.x + 17, y, body.width.saturating_sub(18), 1),
            v,
            if idx == 0 { GOLD } else { CYAN },
            false,
        );
    }
}

fn tokens(f: &mut Frame, area: Rect, project: Option<&str>) {
    let audit = read_saved_analytics_json("audit.json");
    let lizard = read_saved_analytics_json("lizard.json");
    if audit.is_some() || lizard.is_some() {
        draw_saved_mayhem_metrics(f, area, audit.as_ref(), lizard.as_ref());
        return;
    }
    let title = "TOKEN USAGE";
    let source = project
        .map(|p| format!("selected: {p} • source unavailable"))
        .unwrap_or_else(|| "source: session usage • unavailable".into());
    let body = panel(f, area, title, &source);
    header(
        f,
        Rect::new(body.x, body.y, body.width, 1),
        &[24, 26, 26, 24],
        &["PROVIDER", "INPUT TOKENS", "OUTPUT TOKENS", "CACHE TOKENS"],
    );
    label(
        f,
        Rect::new(body.x + 2, body.y + 3, body.width.saturating_sub(4), 1),
        "Unavailable — no existing project-scoped usage record",
        GOLD,
        false,
    );
    label(
        f,
        Rect::new(body.x + 2, body.y + 5, body.width.saturating_sub(4), 1),
        "Browsing never calls a model or spends inference tokens.",
        MUTED,
        false,
    );
}

fn selected_worktree(
    f: &mut Frame,
    area: Rect,
    worktree: Option<&WorktreeRecord>,
    opens: &mut Vec<(Rect, PathBuf)>,
) {
    let title = worktree
        .map(|w| format!("SELECTED WORKTREE: {} / {}", w.project, w.name))
        .unwrap_or_else(|| "SELECTED WORKTREE: Unavailable".into());
    let body = panel(f, area, &title, "source: Git tracked paths only");
    let split =
        Layout::horizontal([Constraint::Percentage(53), Constraint::Percentage(47)]).split(body);
    let values = [
        (
            "Path:",
            worktree
                .map(|w| w.path.display().to_string())
                .unwrap_or_else(|| "Unavailable".into()),
        ),
        (
            "Branch:",
            worktree
                .map(|w| w.branch.clone())
                .unwrap_or_else(|| "Unavailable".into()),
        ),
        (
            "Files:",
            worktree
                .and_then(|w| {
                    w.files.map(|n| {
                        format!("{n} tracked{}", if w.partial { " (partial)" } else { "" })
                    })
                })
                .unwrap_or_else(|| "Unavailable".into()),
        ),
        (
            "Size:",
            worktree
                .and_then(|w| {
                    w.bytes
                        .map(|n| format!("{}{}", if w.partial { "~" } else { "" }, size(n)))
                })
                .unwrap_or_else(|| "Unavailable".into()),
        ),
    ];
    for (offset, (name, value)) in values.iter().enumerate() {
        let y = body.y + 1 + offset as u16 * 2;
        if y >= body.y + body.height {
            break;
        }
        label(f, Rect::new(split[0].x + 1, y, 9, 1), name, IVORY, false);
        label(
            f,
            Rect::new(split[0].x + 10, y, split[0].width.saturating_sub(11), 1),
            value,
            CYAN,
            false,
        );
    }
    if let Some(worktree) = worktree {
        folders(f, split[1], &worktree.folders, worktree.bytes, opens, false);
    } else {
        label(
            f,
            Rect::new(
                split[1].x + 2,
                split[1].y + 1,
                split[1].width.saturating_sub(3),
                1,
            ),
            "FOLDER   SIZE   USAGE",
            IVORY,
            false,
        );
        label(
            f,
            Rect::new(
                split[1].x + 2,
                split[1].y + 4,
                split[1].width.saturating_sub(3),
                1,
            ),
            "Unavailable",
            GOLD,
            false,
        );
    }
}

fn folders(
    f: &mut Frame,
    area: Rect,
    folders: &[FolderRecord],
    total: Option<u64>,
    opens: &mut Vec<(Rect, PathBuf)>,
    controls: bool,
) {
    if folders.is_empty() {
        label(
            f,
            Rect::new(area.x + 1, area.y + 2, area.width.saturating_sub(2), 1),
            "Directory usage unavailable",
            GOLD,
            false,
        );
        return;
    }
    let widths: &[u16] = if controls {
        &[37, 22, 27, 14]
    } else {
        &[40, 24, 36]
    };
    for (slot, folder) in folders
        .iter()
        .take(usize::from(area.height / 2))
        .enumerate()
    {
        draw_folder_row(f, area, (slot, folder, total, controls), widths, opens);
    }
}

fn draw_folder_row(
    f: &mut Frame,
    area: Rect,
    (slot, folder, total, controls): (usize, &FolderRecord, Option<u64>, bool),
    widths: &[u16],
    opens: &mut Vec<(Rect, PathBuf)>,
) {
    let colors = [
        Color::Rgb(230, 119, 126),
        Color::Rgb(112, 196, 149),
        GOLD,
        CYAN,
    ];
    let y = area.y + slot as u16 * 2;
    let cells = columns(Rect::new(area.x, y, area.width, 1), widths);
    let name_text = if controls {
        format!("▱ {} ({})", folder.name, folder.files)
    } else {
        format!("▱ {}", folder.name)
    };
    label(f, cells[0], &name_text, GOLD, false);
    label(
        f,
        cells[1],
        &folder_size_text(folder.bytes, controls),
        IVORY,
        false,
    );
    bar(f, cells[2], folder.bytes, total, colors[slot % 4]);
    // The reference lower worktree inspector has no Open column.
    if controls {
        label(
            f,
            cells[3],
            "[Open]",
            if folder.available { GOLD } else { MUTED },
            false,
        );
        if folder.available {
            opens.push((cells[3], folder.path.clone()));
        }
    }
}

fn folder_size_text(bytes: Option<u64>, controls: bool) -> String {
    let Some(n) = bytes else {
        return "n/a".into();
    };
    if controls {
        size(n)
    } else {
        size(n)
            .replace(' ', "")
            .replace("KB", "K")
            .replace("MB", "M")
            .replace("GB", "G")
    }
}

fn bar(f: &mut Frame, area: Rect, value: Option<u64>, maximum: Option<u64>, color: Color) {
    let (Some(value), Some(maximum)) = (value, maximum) else {
        label(f, area, "n/a", MUTED, false);
        return;
    };
    let width = usize::from(area.width);
    if width == 0 {
        return;
    }
    let filled = if maximum == 0 {
        0
    } else {
        ((value.min(maximum) as u128 * width as u128) / maximum as u128) as usize
    };
    f.render_widget(
        Paragraph::new("░".repeat(width)).style(Style::default().fg(Color::Rgb(65, 48, 70))),
        area,
    );
    if filled > 0 {
        f.render_widget(
            Paragraph::new("█".repeat(filled)).style(Style::default().fg(color)),
            area,
        );
    }
}

fn selected_git_project(
    f: &mut Frame,
    area: Rect,
    data: &ProjectSnapshot,
    selected: usize,
    opens: &mut Vec<(Rect, PathBuf)>,
) {
    let repository = &data.worktrees[0];
    let body = panel(
        f,
        area,
        &format!("SELECTED PROJECT: {}", repository.project),
        "Git repository group • no aggregate bytes",
    );
    let split =
        Layout::horizontal([Constraint::Percentage(54), Constraint::Percentage(46)]).split(body);
    let left = split[0];
    f.render_widget(
        Paragraph::new("● ●\n▟██▸\n ▀▀").style(Style::default().fg(Color::Rgb(230, 119, 126))),
        Rect::new(left.x + 2, left.y + 2, 6, 3),
    );
    for (offset, (name, value)) in [
        ("Path:", repository.path.display().to_string()),
        ("Worktrees:", data.worktrees.len().to_string()),
        ("Files:", "Unavailable (shared paths)".into()),
        ("Size:", "Unavailable (not summed)".into()),
    ]
    .iter()
    .enumerate()
    {
        let y = left.y + 1 + offset as u16 * 2;
        if y >= left.y + left.height {
            break;
        }
        label(f, Rect::new(left.x + 10, y, 12, 1), name, IVORY, false);
        label(
            f,
            Rect::new(left.x + 22, y, left.width.saturating_sub(24), 1),
            value,
            if offset == 0 { CYAN } else { IVORY },
            false,
        );
    }
    let right = split[1];
    let selected = data.worktrees.get(selected).unwrap_or(repository);
    label(
        f,
        Rect::new(right.x + 1, right.y, right.width.saturating_sub(2), 1),
        &format!("TRACKED PATH: {}", selected.name),
        CYAN,
        false,
    );
    let rows = Rect::new(
        right.x,
        right.y + 2,
        right.width,
        right.height.saturating_sub(2),
    );
    folders(f, rows, &selected.folders, selected.bytes, opens, true);
}

fn bar_with_percent(
    f: &mut Frame,
    area: Rect,
    value: Option<u64>,
    maximum: Option<u64>,
    color: Color,
) {
    let (Some(value), Some(maximum)) = (value, maximum.filter(|n| *n > 0)) else {
        label(f, area, "n/a", MUTED, false);
        return;
    };
    let width = area.width.saturating_sub(5);
    bar(
        f,
        Rect::new(area.x, area.y, width, 1),
        Some(value),
        Some(maximum),
        color,
    );
    let percent = (u128::from(value.min(maximum)) * 100 / u128::from(maximum)) as u64;
    label(
        f,
        Rect::new(area.x + width + 1, area.y, 4, 1),
        &format!("{percent:>3}%"),
        color,
        false,
    );
}

/// Read-only presentation of caller-supplied native reports; never executes an audit.
pub fn draw_audit(f: &mut Frame, area: Rect, app: &mut App) {
    let Some(comparison) = app.audit.as_ref() else {
        draw_empty_audit(f, area, app);
        return;
    };
    let area = draw_audit_stale_banner(f, area, app.audit_error.as_deref());
    if area.width < 120 || area.height < 40 {
        let body = panel(
            f,
            area,
            "MAYHEM — NATIVE AUDIT REPORTS",
            "enlarge for comparison grid",
        );
        f.render_widget(Paragraph::new("Baseline/candidate JSON loaded. Enlarge to at least 120 columns to view native toolResults, findings and errors.\n↑/↓ select tool • [/] select finding • d complete details • q quit\nNo audit or inference is started by navigation.").wrap(Wrap{trim:false}).style(Style::default().fg(IVORY)),body);
        return;
    }
    let ids = comparison.tool_ids();
    let selected = app.audit_selected.min(ids.len().saturating_sub(1));
    let id = ids.get(selected).map(String::as_str);
    let bands = Layout::vertical([
        Constraint::Percentage(39),
        Constraint::Percentage(29),
        Constraint::Percentage(32),
    ])
    .spacing(1)
    .split(area);
    let upper = Layout::horizontal([Constraint::Percentage(49), Constraint::Percentage(51)])
        .spacing(1)
        .split(bands[0]);
    let mut rows = Vec::new();
    let mut opens = Vec::new();
    audit_tools(
        f,
        upper[0],
        ("BASELINE", false),
        (&comparison.baseline, &comparison.candidate),
        &ids,
        selected,
        &mut rows,
    );
    audit_tools(
        f,
        upper[1],
        ("CANDIDATE", true),
        (&comparison.candidate, &comparison.baseline),
        &ids,
        selected,
        &mut rows,
    );
    audit_finding(
        f,
        bands[1],
        comparison,
        id,
        (app.audit_finding_selected, selected),
        &mut opens,
    );
    let lower = Layout::horizontal([Constraint::Percentage(48), Constraint::Percentage(52)])
        .spacing(1)
        .split(bands[2]);
    audit_errors(f, lower[0], comparison, id);
    audit_scope(f, lower[1], comparison, &mut opens);
    app.audit_rows.extend(rows);
    app.project_buttons.extend(opens);
}

fn draw_empty_audit(f: &mut Frame, area: Rect, app: &App) {
    if let Some(scope) = &app.audit_pr_context {
        let configured = app.audit_inputs.as_ref().is_some_and(|input| {
            scope.matches(
                input.baseline_sha.as_deref(),
                input.candidate_sha.as_deref(),
            )
        });
        let status = if configured {
            "Matching artifact inputs configured; not loaded/verified. Press R to load/verify."
        } else {
            "No matching native audit artifact inputs supplied."
        };
        draw_missing_pr(f, area, scope, status);
        return;
    }
    let body = panel(
        f,
        area,
        "MAYHEM — NATIVE AUDIT REPORTS",
        "read-only supplied JSON",
    );
    let error = app.audit_error.as_deref().unwrap_or(if app.refreshing {
        "Loading bounded supplied native report JSON; no checks run"
    } else {
        "No baseline/candidate reports supplied. Pass --audit-baseline and --audit-candidate."
    });
    f.render_widget(Paragraph::new(format!("{}\n\nNo checker, agent or model was run. Agentlinter: not run.\nReport provenance and native errors remain unavailable until existing native JSON reports are supplied.",crate::data::clean(error))).wrap(Wrap{trim:false}).style(Style::default().fg(GOLD)),body);
}

fn draw_audit_stale_banner(f: &mut Frame, area: Rect, error: Option<&str>) -> Rect {
    let Some(error) = error else {
        return area;
    };
    label(
        f,
        Rect::new(area.x, area.y, area.width, 1),
        &format!(
            "STALE — refresh failed; previous reports retained: {} • d details",
            crate::data::clean(error)
        ),
        Color::Rgb(245, 95, 105),
        true,
    );
    Rect::new(
        area.x,
        area.y + 2,
        area.width,
        area.height.saturating_sub(2),
    )
}

fn native_field(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .map(|field| {
            field
                .as_str()
                .map(crate::data::clean)
                .unwrap_or_else(|| crate::data::clean(&field.to_string()))
        })
        .unwrap_or_else(|| "Unavailable".into())
}
fn native_number(value: &serde_json::Value, key: &str) -> Option<i64> {
    value.get(key).and_then(serde_json::Value::as_i64)
}

fn audit_tools(
    f: &mut Frame,
    area: Rect,
    (title, candidate): (&str, bool),
    (report, other): (&crate::audit::Report, &crate::audit::Report),
    ids: &[String],
    selected: usize,
    rows: &mut Vec<(Rect, usize)>,
) {
    let body = panel(
        f,
        area,
        &format!("{title} • NATIVE TOOLRESULTS"),
        &format!("issues {} • errors {}", report.issues(), report.errors()),
    );
    let widths = [3, 26, 15, 15, 19, 13, 9];
    header(
        f,
        Rect::new(body.x, body.y, body.width, 1),
        &widths,
        &[
            "#",
            "toolId",
            "issueCount",
            "errorCount",
            "filesAnalyzed",
            "status",
            "Δ",
        ],
    );
    let capacity = usize::from(body.height.saturating_sub(2) / 2).max(1);
    let start = selected.saturating_sub(capacity.saturating_sub(1));
    for (slot, (index, id)) in ids
        .iter()
        .enumerate()
        .skip(start)
        .take(capacity)
        .enumerate()
    {
        let row = Rect::new(body.x, body.y + 2 + slot as u16 * 2, body.width, 2);
        if row.y >= body.y + body.height {
            break;
        }
        rows.push((row, index));
        draw_audit_tool_row(
            f,
            row,
            (report, other),
            (index, id.as_str(), index == selected, candidate),
            &widths,
        );
    }
}

fn draw_audit_tool_row(
    f: &mut Frame,
    row: Rect,
    (report, other): (&crate::audit::Report, &crate::audit::Report),
    (index, id, active, candidate): (usize, &str, bool, bool),
    widths: &[u16],
) {
    if active {
        f.render_widget(Block::default().style(Style::default().bg(SELECT)), row);
        f.render_widget(
            Block::default()
                .borders(ratatui::widgets::Borders::LEFT | ratatui::widgets::Borders::RIGHT)
                .border_style(Style::default().fg(GOLD)),
            row,
        );
    }
    let tool = report
        .tools()
        .iter()
        .find(|tool| tool["toolId"].as_str() == Some(id))
        .unwrap_or(&serde_json::Value::Null);
    let peer = other
        .tools()
        .iter()
        .find(|tool| tool["toolId"].as_str() == Some(id))
        .unwrap_or(&serde_json::Value::Null);
    let cells = columns(Rect::new(row.x, row.y, row.width, 1), widths);
    label(f, cells[0], &(index + 1).to_string(), GOLD, false);
    label(
        f,
        cells[1],
        crate::audit::tool_label(id),
        if active { GOLD } else { IVORY },
        active,
    );
    f.render_widget(
        Paragraph::new(crate::data::clean(id))
            .style(Style::default().fg(MUTED).add_modifier(Modifier::DIM)),
        Rect::new(
            cells[1].x + 1,
            row.y + 1,
            cells[1].width.saturating_sub(1),
            1,
        ),
    );
    draw_audit_tool_metrics(f, &cells, tool, peer, candidate);
}

fn draw_audit_tool_metrics(
    f: &mut Frame,
    cells: &[Rect],
    tool: &serde_json::Value,
    peer: &serde_json::Value,
    candidate: bool,
) {
    label(
        f,
        cells[2],
        &native_field(tool, "issueCount"),
        if native_number(tool, "issueCount").is_some_and(|n| n > 0) {
            GOLD
        } else {
            IVORY
        },
        false,
    );
    label(
        f,
        cells[3],
        &native_field(tool, "errorCount"),
        if native_number(tool, "errorCount").is_some_and(|n| n > 0) {
            Color::Rgb(242, 108, 115)
        } else {
            IVORY
        },
        false,
    );
    label(
        f,
        cells[4],
        &native_field(tool, "filesAnalyzed"),
        CYAN,
        false,
    );
    label(f, cells[5], &native_field(tool, "status"), IVORY, false);
    let delta = if candidate {
        native_number(tool, "issueCount")
            .zip(native_number(peer, "issueCount"))
            .map(|(after, before)| after - before)
    } else {
        None
    };
    let delta_text = delta
        .map(|n| format!("{n:+}"))
        .unwrap_or_else(|| "—".into());
    label(
        f,
        cells[6],
        &delta_text,
        match delta {
            Some(n) if n < 0 => Color::Rgb(112, 196, 149),
            Some(n) if n > 0 => GOLD,
            _ => MUTED,
        },
        false,
    );
}

fn audit_finding(
    f: &mut Frame,
    area: Rect,
    comparison: &crate::audit::Comparison,
    tool: Option<&str>,
    (selected, tool_index): (usize, usize),
    opens: &mut Vec<(Rect, PathBuf)>,
) {
    let issues: Vec<_> = comparison.candidate.native["issues"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|issue| tool.is_some_and(|id| issue["toolId"].as_str() == Some(id)))
        .collect();
    let index = selected.min(issues.len().saturating_sub(1));
    let title = if issues.is_empty() {
        format!("SELECTED NATIVE FINDING: {}", tool.unwrap_or("Unavailable"))
    } else {
        format!(
            "SELECTED NATIVE FINDING: {} • {} / {}",
            tool.unwrap_or("Unavailable"),
            index + 1,
            issues.len()
        )
    };
    let body = panel(f, area, &title, "candidate issues • [ / ] previous / next");
    let Some(issue) = issues.get(index) else {
        label(
            f,
            Rect::new(body.x + 2, body.y + 2, body.width.saturating_sub(4), 1),
            "No candidate issue records for this tool. Native error records below are separate evidence.",
            GOLD,
            false,
        );
        return;
    };
    let split =
        Layout::horizontal([Constraint::Percentage(39), Constraint::Percentage(61)]).split(body);
    draw_finding_metadata(f, split[0], body.height, issue);
    draw_finding_detail_pane(
        f,
        split[1],
        issue,
        comparison.finding_link(tool_index, index),
        opens,
    );
}

fn draw_finding_metadata(f: &mut Frame, pane: Rect, total_height: u16, issue: &serde_json::Value) {
    let step = if total_height >= 7 { 2 } else { 1 };
    for (offset, key) in ["toolId", "patternId", "line", "severity"]
        .iter()
        .enumerate()
    {
        let y = pane.y + offset as u16 * step;
        if y >= pane.y + total_height {
            break;
        }
        label(f, Rect::new(pane.x + 2, y, 12, 1), key, IVORY, false);
        let value = match *key {
            "toolId" => {
                crate::audit::tool_label(issue["toolId"].as_str().unwrap_or("Unavailable")).into()
            }
            "patternId" => {
                crate::audit::rule_label(issue["patternId"].as_str().unwrap_or("Unavailable"))
                    .into()
            }
            _ => native_field(issue, key),
        };
        label(
            f,
            Rect::new(pane.x + 15, y, pane.width.saturating_sub(17), 1),
            &value,
            if *key == "severity" { GOLD } else { CYAN },
            false,
        );
        if matches!(*key, "toolId" | "patternId") && total_height >= 7 {
            f.render_widget(
                Paragraph::new(format!("  {}", native_field(issue, key)))
                    .style(Style::default().fg(MUTED).add_modifier(Modifier::DIM)),
                Rect::new(pane.x + 15, y + 1, pane.width.saturating_sub(17), 1),
            );
        }
    }
}

fn draw_finding_detail_pane(
    f: &mut Frame,
    pane: Rect,
    issue: &serde_json::Value,
    link: Option<&std::path::Path>,
    opens: &mut Vec<(Rect, PathBuf)>,
) {
    label(
        f,
        Rect::new(pane.x + 1, pane.y, 10, 1),
        "filePath",
        IVORY,
        false,
    );
    f.render_widget(
        Paragraph::new(native_field(issue, "filePath"))
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(CYAN)),
        Rect::new(pane.x + 12, pane.y, pane.width.saturating_sub(14), 2),
    );
    label(
        f,
        Rect::new(pane.x + 1, pane.y + 3, 10, 1),
        "message",
        IVORY,
        false,
    );
    f.render_widget(
        Paragraph::new(native_field(issue, "message"))
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(IVORY)),
        Rect::new(
            pane.x + 12,
            pane.y + 3,
            pane.width.saturating_sub(14),
            pane.height.saturating_sub(6),
        ),
    );
    if let Some(path) = link {
        let action = Rect::new(
            pane.x + pane.width.saturating_sub(21),
            pane.y + pane.height.saturating_sub(3),
            20,
            3,
        );
        f.render_widget(
            Block::bordered().border_style(Style::default().fg(GOLD)),
            action,
        );
        f.render_widget(
            Paragraph::new("Open source file")
                .alignment(ratatui::layout::Alignment::Center)
                .style(Style::default().fg(GOLD)),
            Rect::new(
                action.x + 1,
                action.y + 1,
                action.width.saturating_sub(2),
                1,
            ),
        );
        opens.push((action, path.to_path_buf()));
    } else {
        label(
            f,
            Rect::new(
                pane.x + 1,
                pane.y + pane.height.saturating_sub(1),
                pane.width.saturating_sub(2),
                1,
            ),
            "Source link unavailable • d full native details",
            MUTED,
            false,
        );
    }
}

fn collect_audit_errors<'a>(
    comparison: &'a crate::audit::Comparison,
    tool: Option<&str>,
) -> Vec<(&'static str, &'a serde_json::Value)> {
    let mut records = Vec::new();
    for (source, report) in [("B", &comparison.baseline), ("C", &comparison.candidate)] {
        for error in report.native["errors"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|error| tool.is_none_or(|id| error["toolId"].as_str() == Some(id)))
        {
            records.push((source, error));
        }
    }
    records
}

fn audit_errors(
    f: &mut Frame,
    area: Rect,
    comparison: &crate::audit::Comparison,
    tool: Option<&str>,
) {
    let body = panel(
        f,
        area,
        "NATIVE ERRORS",
        &format!(
            "baseline {} • candidate {}",
            comparison.baseline.errors(),
            comparison.candidate.errors()
        ),
    );
    let widths = [8, 19, 18, 20, 35];
    header(
        f,
        Rect::new(body.x, body.y, body.width, 1),
        &widths,
        &["FROM", "toolId", "phase", "kind", "message"],
    );
    let records = collect_audit_errors(comparison, tool);
    let capacity = usize::from(body.height.saturating_sub(4) / 2);
    if records.is_empty() {
        label(
            f,
            Rect::new(body.x + 2, body.y + 2, body.width.saturating_sub(4), 1),
            "No native error records for selected tool (not a report-wide pass).",
            MUTED,
            false,
        );
    }
    for (slot, (source, error)) in records.iter().take(capacity).enumerate() {
        let cells = columns(
            Rect::new(body.x, body.y + 2 + slot as u16 * 2, body.width, 1),
            &widths,
        );
        label(f, cells[0], source, CYAN, false);
        for (cell, key) in cells
            .iter()
            .skip(1)
            .zip(["toolId", "phase", "kind", "message"])
        {
            label(
                f,
                *cell,
                &native_field(error, key),
                if key == "message" { IVORY } else { GOLD },
                false,
            );
        }
    }
    draw_audit_errors_footer(f, body, comparison, records.len(), capacity);
}

fn draw_audit_errors_footer(
    f: &mut Frame,
    body: Rect,
    comparison: &crate::audit::Comparison,
    total_records: usize,
    capacity: usize,
) {
    label(
        f,
        Rect::new(
            body.x + 1,
            body.y + body.height.saturating_sub(2),
            body.width.saturating_sub(2),
            1,
        ),
        &format!(
            "Showing {} of {} selected-tool native errors • d full records",
            total_records.min(capacity),
            total_records
        ),
        MUTED,
        false,
    );
    let agentlinter = comparison
        .tool_ids()
        .iter()
        .any(|id| id.eq_ignore_ascii_case("agentlinter"));
    let note = if agentlinter {
        "Native status/errorCount retained independently; d full JSON details"
    } else {
        "Agentlinter: not present in supplied toolResults • not run"
    };
    label(
        f,
        Rect::new(
            body.x + 1,
            body.y + body.height.saturating_sub(1),
            body.width.saturating_sub(2),
            1,
        ),
        note,
        GOLD,
        false,
    );
}

fn audit_scope(
    f: &mut Frame,
    area: Rect,
    comparison: &crate::audit::Comparison,
    opens: &mut Vec<(Rect, PathBuf)>,
) {
    let body = panel(
        f,
        area,
        "SOURCES / SCOPE",
        if comparison.baseline.links_partial || comparison.candidate.links_partial {
            "sourceLinksPartial: true • bounded metadata cache"
        } else {
            "caller-supplied provenance • not current HEAD"
        },
    );
    for (index, (name, value)) in [
        (
            "Baseline SHA",
            comparison.baseline.sha.as_deref().unwrap_or("Unavailable"),
        ),
        (
            "Candidate SHA",
            comparison.candidate.sha.as_deref().unwrap_or("Unavailable"),
        ),
    ]
    .iter()
    .enumerate()
    {
        label(
            f,
            Rect::new(body.x + 1, body.y + index as u16 * 2, 16, 1),
            name,
            IVORY,
            false,
        );
        label(
            f,
            Rect::new(
                body.x + 18,
                body.y + index as u16 * 2,
                body.width.saturating_sub(20),
                1,
            ),
            value,
            CYAN,
            false,
        );
    }
    draw_audit_scope_summaries(f, body, comparison);
    draw_audit_scope_actions(f, body, comparison, opens);
}

fn draw_audit_scope_summaries(f: &mut Frame, body: Rect, comparison: &crate::audit::Comparison) {
    let before = comparison.baseline.issues();
    let after = comparison.candidate.issues();
    if body.height >= 8 {
        label(
            f,
            Rect::new(body.x + 1, body.y + 4, body.width.saturating_sub(2), 1),
            &format!(
                "Primary issues {before} → {after} (Δ {:+}); errors {} → {}",
                after as i64 - before as i64,
                comparison.baseline.errors(),
                comparison.candidate.errors()
            ),
            GOLD,
            false,
        );
    }
    let supplements = match (
        &comparison.baseline_supplement,
        &comparison.candidate_supplement,
    ) {
        (Some(a), Some(b)) => format!(
            "Supplement separately: issues {} → {}; errors {} → {}",
            a.issues(),
            b.issues(),
            a.errors(),
            b.errors()
        ),
        _ => "Supplement comparison unavailable (separate inputs required)".into(),
    };
    if body.height >= 10 {
        label(
            f,
            Rect::new(body.x + 1, body.y + 6, body.width.saturating_sub(2), 1),
            &supplements,
            MUTED,
            false,
        );
    }
}

fn draw_audit_scope_actions(
    f: &mut Frame,
    body: Rect,
    comparison: &crate::audit::Comparison,
    opens: &mut Vec<(Rect, PathBuf)>,
) {
    let actions_y = body.y + body.height.saturating_sub(3);
    let actions = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .spacing(1)
        .split(Rect::new(body.x, actions_y, body.width, 3));
    for (action, (name, report)) in actions.iter().zip([
        ("Open baseline JSON", &comparison.baseline),
        ("Open candidate JSON", &comparison.candidate),
    ]) {
        f.render_widget(
            Block::bordered().border_style(Style::default().fg(GOLD)),
            *action,
        );
        f.render_widget(
            Paragraph::new(name)
                .alignment(ratatui::layout::Alignment::Center)
                .style(Style::default().fg(GOLD)),
            Rect::new(
                action.x + 1,
                action.y + 1,
                action.width.saturating_sub(2),
                1,
            ),
        );
        opens.push((*action, report.path.clone()));
    }
}

fn draw_missing_pr(f: &mut Frame, area: Rect, scope: &crate::handoff::PrScope, status: &str) {
    let bands = Layout::vertical([
        Constraint::Percentage(34),
        Constraint::Percentage(36),
        Constraint::Percentage(30),
    ])
    .spacing(1)
    .split(area);
    missing_revisions(f, bands[0], scope, status);
    let body = panel(
        f,
        bands[1],
        &format!("PR #{} — AUDIT SCOPE", scope.number),
        "imported GitHub metadata • not a code audit",
    );
    let content = format!(
        "{}\n{}\n\n{}\n\nUnrelated whole-project reports are not substituted for this PR.\nMOPS shows ownership, review state, blockers and recorded session handoff.",
        scope.title, scope.url, scope.source
    );
    wrapped_evidence(f, body, content, CYAN);
    let lower = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .spacing(1)
        .split(bands[2]);
    let left = panel(
        f,
        lower[0],
        "AUDIT COVERAGE",
        "unsupported fields remain Unknown",
    );
    f.render_widget(Paragraph::new("Baseline/head native findings and audit errors: Unknown.\nNo checker, agent or model was started.\nNo inference or unrelated historical totals are used.\nPress d for full PR-scoped evidence text; F3 returns to MOPS.").style(Style::default().fg(GOLD)).wrap(Wrap{trim:false}),left);
    let right = panel(
        f,
        lower[1],
        "NATIVE CI — SEPARATE EVIDENCE",
        "not review approval or full-video acceptance",
    );
    f.render_widget(Paragraph::new(format!("{}\n\nThis is supplied check-run metadata, not Codacy findings.\nMissing matching code-audit artifacts remain explicit.",scope.ci)).style(Style::default().fg(IVORY)).wrap(Wrap{trim:false}),right);
}

fn missing_revisions(f: &mut Frame, area: Rect, scope: &crate::handoff::PrScope, status: &str) {
    let upper = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .spacing(1)
        .split(area);
    for (slot, (title, sha)) in upper.iter().zip([
        ("BASELINE CODE FINDINGS", scope.base.as_deref()),
        ("CANDIDATE CODE FINDINGS", scope.head.as_deref()),
    ]) {
        let body = panel(f, *slot, title, "exact PR revision scope");
        let content = format!(
            "Native SHA: {}\n\nNative toolResults: Unavailable\nissueCount: Unknown    errorCount: Unknown\n\n{}\nNot zero findings; not an audit pass.",
            sha.unwrap_or("Unknown"),
            status
        );
        wrapped_evidence(f, body, content, IVORY);
    }
}

fn wrapped_evidence(f: &mut Frame, area: Rect, content: String, color: Color) {
    f.render_widget(
        Paragraph::new(content)
            .style(Style::default().fg(color))
            .wrap(Wrap { trim: false }),
        area,
    );
}
