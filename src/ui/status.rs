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

    match status {
        Status::Idle => {
            frame.render_widget(Line::raw("Idle").right_aligned(), area);
        }
    }
}
