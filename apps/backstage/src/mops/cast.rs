//! Adaptive identities are PR records, never runtime agent-presence observations.
use super::widgets::*;
use crate::handoff::Task;
use ratatui::{Frame, layout::Rect, text::Span};

pub(super) fn draw(
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
    let names = recorded_agents(tasks);
    let x = area.x + 10;
    if names.is_empty() {
        text(
            f,
            Rect::new(x, area.y + 1, area.width.saturating_sub(11), 1),
            format!("Unknown • {} • d source details", error.unwrap_or(source)),
            MUTED,
            false,
        );
        return;
    }
    let selected_agent = selected.and_then(|t| t.agent.as_deref());
    let x = identities(f, area, &names, selected_agent, x);
    source_note(f, area, x, source, error);
}
fn recorded_agents(tasks: &[Task]) -> Vec<&str> {
    let mut names = Vec::new();
    for task in tasks {
        if let Some(agent) = task.agent.as_deref().filter(|s| known_identity(s))
            && !names.contains(&agent)
        {
            names.push(agent);
        }
    }
    names
}
fn identities(
    f: &mut Frame,
    area: Rect,
    names: &[&str],
    selected: Option<&str>,
    mut x: u16,
) -> u16 {
    for (index, name) in names.iter().enumerate() {
        let width = (Span::raw(*name).width() as u16 + 7).min(28);
        if x + width >= area.right().saturating_sub(25) {
            return overflow(f, area, x, names.len() - index);
        }
        identity(
            f,
            Rect::new(x, area.y + 1, width, 1),
            name,
            Some(*name) == selected,
        );
        x += width;
    }
    x
}
fn identity(f: &mut Frame, area: Rect, name: &str, active: bool) {
    let label = format!("{} {}", if active { "▣" } else { "□" }, name);
    text(f, area, label, if active { GOLD } else { IVORY }, active);
}
fn overflow(f: &mut Frame, area: Rect, x: u16, remaining: usize) -> u16 {
    let more = format!("+{remaining} more");
    text(
        f,
        Rect::new(x, area.y + 1, area.right().saturating_sub(x), 1),
        &more,
        MUTED,
        false,
    );
    x.saturating_add(Span::raw(&more).width() as u16 + 2)
        .min(area.right())
}
fn source_note(f: &mut Frame, area: Rect, x: u16, source: &str, error: Option<&str>) {
    let label = if error.is_some() {
        "PR records • source warning • d details".into()
    } else {
        format!("PR records, not live • {source}")
    };
    text(
        f,
        Rect::new(x, area.y + 1, area.right().saturating_sub(x), 1),
        label,
        MUTED,
        false,
    );
}
