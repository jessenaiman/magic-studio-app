//! Shared terminal presentation: one palette, safe text clipping and ordinary widgets.
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Cell, Paragraph, Row, Table, Wrap},
};
pub const BG: Color = Color::Rgb(24, 15, 27);
pub const IVORY: Color = Color::Rgb(242, 222, 188);
pub const GOLD: Color = Color::Rgb(245, 186, 56);
pub const LINE: Color = Color::Rgb(176, 126, 125);
pub const MUTED: Color = Color::Rgb(160, 149, 169);
pub const SELECT: Color = Color::Rgb(65, 44, 34);

pub fn clip(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if Span::raw(value).width() <= width {
        return value.into();
    }
    let mut result = String::new();
    for ch in value.chars() {
        if Span::raw(format!("{result}{ch}")).width() >= width {
            break;
        }
        result.push(ch);
    }
    result.push('…');
    result
}
pub fn text(f: &mut Frame, area: Rect, value: impl Into<String>, color: Color, bold: bool) {
    let style = Style::default().fg(color);
    let value = clip(&crate::data::clean(&value.into()), area.width as usize);
    f.render_widget(
        Paragraph::new(value).style(if bold {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        }),
        area,
    );
}
pub fn paragraph(f: &mut Frame, area: Rect, title: &str, content: impl Into<String>, scroll: u16) {
    f.render_widget(
        Paragraph::new(
            content
                .into()
                .chars()
                .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
                .collect::<String>(),
        )
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
        .style(Style::default().fg(IVORY))
        .block(
            Block::bordered()
                .title(title)
                .border_style(Style::default().fg(LINE)),
        ),
        area,
    );
}
pub fn table(
    f: &mut Frame,
    area: Rect,
    title: &str,
    headers: &[&str],
    data: Vec<Vec<String>>,
    widths: &[u16],
    selected: usize,
) -> Vec<(Rect, usize)> {
    let visible = area.height.saturating_sub(3) as usize;
    let start = selected
        .min(data.len().saturating_sub(1))
        .saturating_sub(visible / 2);
    let rows = data
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
        .map(|(i, row)| {
            Row::new(row.iter().map(|s| Cell::from(crate::data::clean(s)))).style(
                if i == selected {
                    Style::default().fg(GOLD).bg(SELECT)
                } else {
                    Style::default().fg(IVORY)
                },
            )
        });
    f.render_widget(
        Table::new(rows, widths.iter().map(|w| Constraint::Percentage(*w)))
            .header(Row::new(headers.iter().copied()).style(Style::default().fg(GOLD)))
            .block(
                Block::bordered()
                    .title(title)
                    .border_style(Style::default().fg(LINE)),
            ),
        area,
    );
    (start..data.len().min(start + visible))
        .enumerate()
        .map(|(r, i)| {
            (
                Rect::new(
                    area.x + 1,
                    area.y + 2 + r as u16,
                    area.width.saturating_sub(2),
                    1,
                ),
                i,
            )
        })
        .collect()
}
pub fn bytes(value: Option<u64>) -> String {
    match value {
        None => "Unknown".into(),
        Some(n) if n >= 1 << 30 => format!("{:.1} GiB", n as f64 / (1u64 << 30) as f64),
        Some(n) if n >= 1 << 20 => format!("{:.1} MiB", n as f64 / (1u64 << 20) as f64),
        Some(n) if n >= 1 << 10 => format!("{:.1} KiB", n as f64 / 1024.0),
        Some(n) => format!("{n} B"),
    }
}
