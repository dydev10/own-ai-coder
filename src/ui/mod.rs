mod status;
mod textarea;
mod transcript;

use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::Block,
};

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App) {
    let [main_area, input_area, status_area] = Layout::vertical(vec![
        Constraint::Min(0),
        Constraint::Length(3),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    transcript::draw(frame, main_area, &app.transcript);

    //frame.render_widget(
    //    Block::bordered().border_style(Style::new().green()),
    //    input_area,
    //);

    textarea::draw(frame, input_area, &app.input);

    status::draw(frame, status_area, &app.status);
}
