use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Stylize},
    text::Line,
    widgets::Block,
};

use crate::app::Status;

pub fn draw(frame: &mut Frame, area: Rect, status: &Status) {
    frame.render_widget(Block::new().bg(Color::Blue), area);
    frame.render_widget(Line::raw("OwnCode"), area);

    let status_text = match status {
        Status::Idle => "Idle",
        Status::Streaming => "Streaming",
        Status::Cancelling => "Cancelling",
    };

    frame.render_widget(Line::raw(status_text).right_aligned(), area);
}
