use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, Padding, Paragraph, Wrap},
};

use crate::{
    app::{Item, ScrollState},
    ui::tool_call,
};

const H_PADDING: u16 = 1;

fn build_lines(items: &[Item]) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();

    for item in items {
        match item {
            Item::User(text) => {
                lines.push(Line::styled(text.clone(), Style::new().dim()).right_aligned())
            }
            Item::Assistant(text) => lines.push(Line::raw(text.clone())),
            Item::Error(text) => lines.push(Line::styled(text.clone(), Style::new().red())),
            Item::Tool(tool) => lines.push(tool_call::line(tool)),
        }

        lines.push(Line::raw(""));
    }

    lines
}

fn block() -> Block<'static> {
    Block::bordered()
        .border_style(Style::new().red())
        .padding(Padding::horizontal(H_PADDING))
}

pub fn draw(frame: &mut Frame, area: Rect, items: &[Item], scroll: &ScrollState) {
    let block = block();

    let para = Paragraph::new(build_lines(items))
        .wrap(Wrap { trim: false })
        .scroll((scroll.offset, 0))
        .block(block);

    frame.render_widget(para, area);
}

pub fn total_height(items: &[Item], content_width: u16) -> u16 {
    Paragraph::new(build_lines(items))
        .wrap(Wrap { trim: false })
        .line_count(content_width) as u16
}

pub fn max_offset(items: &[Item], area: Rect) -> u16 {
    let inner = block().inner(area);
    total_height(items, inner.width).saturating_sub(inner.height)
}

pub fn page_height(area: Rect) -> u16 {
    block().inner(area).height.saturating_sub(H_PADDING).max(1)
}
