use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::{app::ToolItem, session::ToolStatus};

pub fn line(tool: &ToolItem) -> Line<'_> {
    let mut spans = vec![
        glyph(&tool.status),
        Span::raw(" "),
        Span::styled(tool.call.name.as_str(), Style::new().bold()),
    ];

    if let Some(preview) = &tool.preview {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(preview.as_str(), Style::new().dim()));
    }

    Line::from(spans)
}

fn glyph(status: &ToolStatus) -> Span<'_> {
    match status {
        ToolStatus::Pending => Span::styled("○", Style::new().dim()),
        ToolStatus::Running => Span::styled("◐", Style::new().yellow()),
        ToolStatus::Complete { .. } => Span::styled("✓", Style::new().green()),
        ToolStatus::Failed { .. } => Span::styled("✗", Style::new().red()),
    }
}
