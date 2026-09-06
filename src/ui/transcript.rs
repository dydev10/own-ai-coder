use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, Padding, Paragraph, Wrap},
};

use crate::app::Item;

pub fn draw(frame: &mut Frame, area: Rect, items: &[Item]) {
    let block = Block::bordered()
        .border_style(Style::new().red())
        .padding(Padding::uniform(1));

    //    // to be used for wrapping and height
    //    let inner = block.inner(rect);

    let mut lines: Vec<Line> = Vec::new();
    for item in items {
        match item {
            Item::User(text) => {
                lines.push(Line::styled(text.clone(), Style::new().dim()).right_aligned())
            }
            Item::Assistant(text) => lines.push(Line::raw(text.clone())),
            Item::Error(text) => lines.push(Line::styled(text.clone(), Style::new().red())),
        }

        lines.push(Line::raw(""));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(block),
        area,
    );
}
