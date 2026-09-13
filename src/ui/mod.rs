pub mod status;
pub mod textarea;
pub mod tool_call;
pub mod transcript;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
};

use crate::app::App;

pub fn layout(area: Rect, input_height: u16) -> [Rect; 3] {
    Layout::vertical(vec![
        Constraint::Min(0),
        Constraint::Length(input_height),
        Constraint::Length(1),
    ])
    .areas(area)
}

pub fn main_area(app: &App) -> Rect {
    let (w, h) = crossterm::terminal::size().unwrap_or((80, 24));
    layout(Rect::new(0, 0, w, h), textarea::height(&app.input))[0]
}

pub fn draw(frame: &mut Frame, app: &App) {
    let [main_area, input_area, status_area] = layout(frame.area(), textarea::height(&app.input));

    transcript::draw(frame, main_area, &app.transcript, &app.scroll);

    textarea::draw(frame, input_area, &app.input);

    status::draw(frame, status_area, &app.status);
}
