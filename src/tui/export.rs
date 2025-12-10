//! Export options panel rendering
//!
//! Shows export format selection and stretching parameter controls

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

use super::app::{App, Focus, OutputFormat, StretchingType};

/// Render the export options panel
pub fn render_export_panel(frame: &mut Frame, area: Rect, app: &App) {
    let is_focused = app.focus == Focus::Export;

    let block = Block::default()
        .title(" Export Options ")
        .title_style(Style::default().fg(Color::Cyan).bold())
        .borders(Borders::ALL)
        .border_style(if is_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        });

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = vec![];

    // Output format selection
    lines.push(Line::from(Span::styled(
        "Format:",
        Style::default().bold(),
    )));

    let formats = [
        (OutputFormat::CliArgs, "1", "CLI Args"),
        (OutputFormat::Yaml, "2", "YAML"),
        (OutputFormat::VgridFile, "3", "vgrid.in"),
    ];

    for (fmt, key, label) in formats {
        let is_selected = app.export_options.output_format == fmt;
        let style = if is_selected {
            Style::default().fg(Color::Cyan).bold()
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let prefix = if is_selected { ">" } else { " " };
        lines.push(Line::from(Span::styled(
            format!(" {}[{}] {}", prefix, key, label),
            style,
        )));
    }

    lines.push(Line::from(""));

    // Stretching function
    lines.push(Line::from(Span::styled(
        "Stretching:",
        Style::default().bold(),
    )));

    let stretches = [
        (StretchingType::S, "s", "S-transform"),
        (StretchingType::Quadratic, "q", "Quadratic"),
    ];

    for (st, key, label) in stretches {
        let is_selected = app.export_options.stretching == st;
        let style = if is_selected {
            Style::default().fg(Color::Cyan).bold()
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let prefix = if is_selected { ">" } else { " " };
        lines.push(Line::from(Span::styled(
            format!(" {}[{}] {}", prefix, key, label),
            style,
        )));
    }

    lines.push(Line::from(""));

    // Stretching parameters
    lines.push(Line::from(Span::styled(
        "Parameters:",
        Style::default().bold(),
    )));

    // theta_f
    lines.push(Line::from(vec![
        Span::styled(" θf: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:>4.1}", app.export_options.theta_f),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled(" [f/F]", Style::default().fg(Color::DarkGray).dim()),
    ]));

    // theta_b
    lines.push(Line::from(vec![
        Span::styled(" θb: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:>4.1}", app.export_options.theta_b),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled(" [b/B]", Style::default().fg(Color::DarkGray).dim()),
    ]));

    // a_vqs0
    lines.push(Line::from(vec![
        Span::styled(" a:  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:>4.1}", app.export_options.a_vqs0),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled(" [v/V]", Style::default().fg(Color::DarkGray).dim()),
    ]));

    lines.push(Line::from(""));

    // Export button
    let can_export = app.path.is_valid();
    let export_style = if can_export && is_focused {
        Style::default().fg(Color::Green).bold()
    } else if can_export {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    lines.push(Line::from(Span::styled("[Enter] Export", export_style)));

    if !can_export {
        lines.push(Line::from(Span::styled(
            "(select 2+ anchors)",
            Style::default().fg(Color::Red).dim(),
        )));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}
