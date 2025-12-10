//! Path preview panel rendering
//!
//! Shows the currently selected anchor points, validation status,
//! and zone statistics with layer distribution based on stretching.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

use super::app::{App, Focus, StretchingType};
use super::path::PathError;
use super::stretching::compute_path_zone_stats;

/// Render the path preview panel
pub fn render_path_preview(frame: &mut Frame, area: Rect, app: &App) {
    let is_focused = app.focus == Focus::PathPreview;

    let block = Block::default()
        .title(" Selected Path ")
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

    // Header
    lines.push(Line::from(vec![
        Span::styled(
            format!("{:>8}", "Depth"),
            Style::default().fg(Color::White).bold(),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{:>4}", "N"),
            Style::default().fg(Color::White).bold(),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{:>6}", "minΔz"),
            Style::default().fg(Color::White).bold(),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{:>6}", "avgΔz"),
            Style::default().fg(Color::White).bold(),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{:>6}", "maxΔz"),
            Style::default().fg(Color::White).bold(),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        "─".repeat(38),
        Style::default().fg(Color::DarkGray),
    )));

    // Path anchors with zone stats
    if app.path.anchors.is_empty() {
        lines.push(Line::from(Span::styled(
            "No anchors selected",
            Style::default().fg(Color::DarkGray).italic(),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Click cells or press",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "Space/Enter to select",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        // Compute zone stats with stretching
        let (depths, nlevels) = app.path.to_hsm_config();
        let params = app.get_stretching_params();
        let use_s_transform = app.export_options.stretching == StretchingType::S;
        let zone_stats = compute_path_zone_stats(&depths, &nlevels, &params, use_s_transform);

        for (i, anchor) in app.path.anchors.iter().enumerate() {
            // Check for monotonicity issues
            let has_error = i > 0 && anchor.nlevels < app.path.anchors[i - 1].nlevels;

            let style = if has_error {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::Green)
            };

            // Get zone stats for this anchor
            let (min_dz, avg_dz, max_dz) = if let Some(stats) = zone_stats.get(i) {
                (stats.min_dz, stats.avg_dz, stats.max_dz)
            } else {
                (0.0, 0.0, 0.0)
            };

            let mut spans = vec![
                Span::styled(format!("{:>7.1}m", anchor.depth), style),
                Span::raw(" "),
                Span::styled(format!("{:>4}", anchor.nlevels), style),
                Span::raw(" "),
                Span::styled(format!("{:>5.2}m", min_dz), Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::styled(format!("{:>5.2}m", avg_dz), Style::default().fg(Color::White)),
                Span::raw(" "),
                Span::styled(format!("{:>5.2}m", max_dz), Style::default().fg(Color::Yellow)),
            ];

            if has_error {
                spans.push(Span::styled(" !", Style::default().fg(Color::Red).bold()));
            }

            lines.push(Line::from(spans));
        }
    }

    // Separator
    lines.push(Line::from(""));

    // Validation status
    if app.path.is_valid() {
        lines.push(Line::from(Span::styled(
            "Path is valid",
            Style::default().fg(Color::Green).bold(),
        )));

        // Show total levels
        if let Some(last) = app.path.anchors.last() {
            lines.push(Line::from(Span::styled(
                format!("Total: {} levels", last.nlevels),
                Style::default().fg(Color::DarkGray),
            )));
        }
    } else {
        for error in &app.path.validation_errors {
            let error_text = match error {
                PathError::MonotonicityViolation {
                    at_depth,
                    n_value,
                    previous_n,
                } => {
                    format!("N={} < {} at {}m", n_value, previous_n, at_depth)
                }
                PathError::InsufficientAnchors => "Need 2+ anchors".to_string(),
                PathError::InvalidCellSelected { .. } => "Invalid cell".to_string(),
            };
            lines.push(Line::from(Span::styled(
                error_text,
                Style::default().fg(Color::Red),
            )));
        }
    }

    // Show stretching info
    lines.push(Line::from(""));
    let stretch_name = match app.export_options.stretching {
        StretchingType::S => "S-transform",
        StretchingType::Quadratic => "Quadratic",
    };
    lines.push(Line::from(Span::styled(
        format!("Stretch: {}", stretch_name),
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        format!(
            "θf={:.1} θb={:.1} a={:.1}",
            app.export_options.theta_f, app.export_options.theta_b, app.export_options.a_vqs0
        ),
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}
