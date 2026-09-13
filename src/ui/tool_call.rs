use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::{
    app::ToolItem,
    session::{ToolCall, ToolStatus},
};

pub fn line(tool: &ToolItem) -> Line<'static> {
    let mut spans = vec![
        glyph(&tool.status),
        Span::raw(" "),
        Span::styled(tool.call.name.clone(), Style::new().bold()),
    ];

    if let Some(preview) = &tool.preview {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(preview.clone(), Style::new().dim()));
    }

    Line::from(spans)
}

fn glyph(status: &ToolStatus) -> Span<'static> {
    match status {
        ToolStatus::Pending => Span::styled("○", Style::new().dim()),
        ToolStatus::Running => Span::styled("◐", Style::new().yellow()),
        ToolStatus::Complete { .. } => Span::styled("✓", Style::new().green()),
        ToolStatus::Failed { .. } => Span::styled("✗", Style::new().red()),
    }
}
