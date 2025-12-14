//! Path preview panel rendering
//!
//! Shows the currently selected anchor points, validation status,
//! and zone statistics with layer distribution based on stretching.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use super::app::{App, Focus, StretchingType};
use super::stretching::compute_path_zone_stats;

/// Render the path preview panel with scrollable anchor list
pub fn render_path_preview(frame: &mut Frame, area: Rect, app: &App) {
    let is_focused = app.focus == Focus::PathPreview;

    // Count anchors for scroll indicator
    let anchor_count = app.path.anchors.len();
    let title = if anchor_count > 0 {
        format!(" Anchors ({}) ", anchor_count)
    } else {
        " Selected Path ".to_string()
    };

    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(Color::Cyan).bold())
        .borders(Borders::ALL)
        .border_style(if is_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        });

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Calculate available height for anchor list
    // Reserve: 2 for header, 1 for separator, 3 for validation/status
    let header_lines = 2u16;
    let footer_lines = 4u16;
    let available_for_anchors = inner.height.saturating_sub(header_lines + footer_lines) as usize;

    let mut y = inner.y;

    // Header
    let header = Line::from(vec![
        Span::styled(format!("{:>8}", "Depth"), Style::default().fg(Color::White).bold()),
        Span::raw(" "),
        Span::styled(format!("{:>4}", "N"), Style::default().fg(Color::White).bold()),
        Span::raw(" "),
        Span::styled(format!("{:>6}", "minΔz"), Style::default().fg(Color::White).bold()),
        Span::raw(" "),
        Span::styled(format!("{:>6}", "avgΔz"), Style::default().fg(Color::White).bold()),
        Span::raw(" "),
        Span::styled(format!("{:>6}", "maxΔz"), Style::default().fg(Color::White).bold()),
    ]);
    frame.render_widget(Paragraph::new(header), Rect::new(inner.x, y, inner.width, 1));
    y += 1;

    // Separator
    let sep = Paragraph::new("─".repeat((inner.width.saturating_sub(1)) as usize))
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(sep, Rect::new(inner.x, y, inner.width, 1));
    y += 1;

    // Path anchors with zone stats
    if app.path.anchors.is_empty() {
        let empty_msg = Paragraph::new("No anchors selected")
            .style(Style::default().fg(Color::DarkGray).italic());
        frame.render_widget(empty_msg, Rect::new(inner.x, y, inner.width, 1));
        y += 1;
        let hint1 = Paragraph::new("Press Space to select cells")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint1, Rect::new(inner.x, y, inner.width, 1));
        y += 1;
        let hint2 = Paragraph::new("or S for suggestions")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint2, Rect::new(inner.x, y, inner.width, 1));
    } else {
        // Compute zone stats with stretching
        let (depths, nlevels) = app.path.to_hsm_config();
        let params = app.get_stretching_params();
        let stretching = app.get_stretching_kind();
        let zone_stats = compute_path_zone_stats(&depths, &nlevels, &params, stretching);

        // Apply scroll offset
        let scroll_offset = app.preview_scroll.min(anchor_count.saturating_sub(1));
        let visible_anchors = app.path.anchors.iter().enumerate().skip(scroll_offset);

        let mut rendered = 0;
        for (i, anchor) in visible_anchors {
            if rendered >= available_for_anchors {
                break;
            }

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

            let line = Paragraph::new(Line::from(spans));
            frame.render_widget(line, Rect::new(inner.x, y, inner.width, 1));
            y += 1;
            rendered += 1;
        }

        // Show scroll indicator if there are more anchors
        if anchor_count > available_for_anchors {
            let remaining = anchor_count.saturating_sub(scroll_offset + rendered);
            if remaining > 0 {
                let more = Paragraph::new(format!("↓ {} more", remaining))
                    .style(Style::default().fg(Color::DarkGray));
                frame.render_widget(more, Rect::new(inner.x, y, inner.width, 1));
            } else if scroll_offset > 0 {
                let more = Paragraph::new(format!("↑ {} above", scroll_offset))
                    .style(Style::default().fg(Color::DarkGray));
                frame.render_widget(more, Rect::new(inner.x, y, inner.width, 1));
            }

            // Render scrollbar on the right side
            let scrollbar_area = Rect::new(
                inner.x + inner.width - 1,
                inner.y + header_lines,
                1,
                available_for_anchors as u16,
            );
            let mut scrollbar_state = ScrollbarState::new(anchor_count)
                .position(scroll_offset)
                .viewport_content_length(available_for_anchors);
            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(Some("↑"))
                    .end_symbol(Some("↓")),
                scrollbar_area,
                &mut scrollbar_state,
            );
        }
    }

    // Footer section: validation + stretching info + hints
    let footer_y = inner.y + inner.height - footer_lines;
    let mut fy = footer_y;

    // Separator
    let sep2 = Paragraph::new("─".repeat((inner.width.saturating_sub(1)) as usize))
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(sep2, Rect::new(inner.x, fy, inner.width, 1));
    fy += 1;

    // Stretching info - show relevant parameters for each transform type
    let (stretch_info, param_hint) = match app.export_options.stretching {
        StretchingType::Quadratic => (
            format!("Quad a={:.1}", app.export_options.a_vqs0),
            "[a/A]",
        ),
        StretchingType::S => (
            format!("S θf={:.1} θb={:.1}", app.export_options.theta_f, app.export_options.theta_b),
            "[f/F b/B]",
        ),
        StretchingType::Shchepetkin2005 => (
            format!("Shch05 θs={:.1} θb={:.1} hc={:.0}", app.export_options.theta_s, app.export_options.theta_b, app.export_options.hc),
            "[s/S b/B h/H]",
        ),
        StretchingType::Shchepetkin2010 => (
            format!("Shch10 θs={:.1} θb={:.1} hc={:.0}", app.export_options.theta_s, app.export_options.theta_b, app.export_options.hc),
            "[s/S b/B h/H]",
        ),
        StretchingType::Geyer => (
            format!("Geyer θs={:.1} θb={:.1} hc={:.0}", app.export_options.theta_s, app.export_options.theta_b, app.export_options.hc),
            "[s/S b/B h/H]",
        ),
    };
    let stretch_line = Paragraph::new(Line::from(vec![
        Span::styled("[t]", Style::default().fg(Color::DarkGray)),
        Span::styled(stretch_info, Style::default().fg(Color::Cyan)),
        Span::styled(format!(" {}", param_hint), Style::default().fg(Color::DarkGray)),
    ]));
    frame.render_widget(stretch_line, Rect::new(inner.x, fy, inner.width, 1));
    fy += 1;

    // Validation status + export
    if app.path.is_valid() {
        let total_levels = app.path.anchors.last().map(|a| a.nlevels).unwrap_or(0);
        let status = Paragraph::new(Line::from(vec![
            Span::styled("✓", Style::default().fg(Color::Green).bold()),
            Span::styled(format!(" {} levels  ", total_levels), Style::default().fg(Color::DarkGray)),
            Span::styled("[e] Export", Style::default().fg(Color::Green)),
        ]));
        frame.render_widget(status, Rect::new(inner.x, fy, inner.width, 1));
    } else {
        let error = if app.path.anchors.len() < 2 { "Need 2+ anchors" } else { "N↓ error" };
        let status = Paragraph::new(error).style(Style::default().fg(Color::Red));
        frame.render_widget(status, Rect::new(inner.x, fy, inner.width, 1));
    }
}
