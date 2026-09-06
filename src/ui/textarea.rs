use ratatui::{Frame, layout::Rect, widgets::Block};
use ratatui_textarea::TextArea;

fn block() -> Block<'static> {
    Block::bordered()
}

pub fn draw(frame: &mut Frame, area: Rect, input: &TextArea) {
    let b = block();
    frame.render_widget(&b, area);
    frame.render_widget(input, b.inner(area));
}
