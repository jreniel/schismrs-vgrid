//! Main UI layout and rendering
//!
//! Composes all panels into the final TUI layout

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::app::{AnchorEditMode, App, Focus, ProfileViewMode, StatusLevel, StretchingType};

/// Format a layer thickness value with adaptive precision.
/// Uses more decimal places for small values to avoid misleading "0.00m" displays.
fn format_dz(value: f64) -> String {
    if value >= 10.0 {
        format!("{:>6.1}m", value)
    } else if value >= 0.01 {
        format!("{:>6.2}m", value)
    } else if value >= 0.001 {
        format!("{:>5.3}m", value) // Show 3 decimals for mm-scale
    } else if value > 0.0 && value.is_finite() {
        format!("{:>5.1e}m", value) // Scientific notation for very small
    } else if value == f64::INFINITY {
        format!("{:>6}", "N/A") // No valid data
    } else {
        format!("{:>6.2}m", value)
    }
}

/// Format a depth range with precision matching the layer thickness.
/// Uses fixed-width formatting for consistent alignment.
/// Ensures z_bot - z_top visually matches the displayed dz value.
fn format_depth_range(z_top: f64, z_bot: f64) -> String {
    let dz = (z_bot - z_top).abs();
    if dz >= 10.0 {
        // Width 5 each: "  0.0→ 10.5" = 11 chars
        format!("{:>5.1}→{:>5.1}", z_top, z_bot)
    } else if dz >= 0.1 {
        // Width 6 each: "  0.00→ 10.35" = 13 chars
        format!("{:>6.2}→{:>6.2}", z_top, z_bot)
    } else if dz >= 0.01 {
        // Width 7 each: "  0.000→ 10.053" = 15 chars
        format!("{:>7.3}→{:>7.3}", z_top, z_bot)
    } else {
        // Width 8 each: "  0.0000→ 10.0053" = 17 chars
        format!("{:>8.4}→{:>8.4}", z_top, z_bot)
    }
}

/// Draw the complete UI
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Main layout: header, body, footer
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header/title
            Constraint::Min(15),   // Body
            Constraint::Length(3), // Status/help bar
        ])
        .split(area);

    render_header(frame, main_layout[0], app);
    render_body(frame, main_layout[1], app);
    render_footer(frame, main_layout[2], app);

    // Export modal if active
    if app.show_export_modal {
        render_export_modal(frame, area, app);
    }

    // Transform help overlay if active
    if app.show_transform_help {
        render_transform_help_overlay(frame, area, app);
    }

    // Help overlay if active (on top of everything)
    if app.show_help {
        render_help_overlay(frame, area);
    }
}

/// Render the export modal dialog
fn render_export_modal(frame: &mut Frame, area: Rect, app: &App) {
    // Center the modal
    let modal_width = 50u16;
    let modal_height = 14u16;
    let modal_x = area.x + (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = area.y + (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

    // Clear the area behind the modal
    frame.render_widget(Clear, modal_area);

    let can_export = app.path.is_valid() && app.mesh_info.is_some();

    let block = Block::default()
        .title(" Export vgrid.in ")
        .title_style(Style::default().fg(Color::Green).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let mut y = inner.y;

    // Show current selection summary
    if app.path.is_valid() {
        let anchor_count = app.path.anchors.len();
        let total_levels = app.path.anchors.last().map(|a| a.nlevels).unwrap_or(0);
        let summary = format!("{} anchors, {} max levels", anchor_count, total_levels);
        let summary_line = Paragraph::new(summary)
            .style(Style::default().fg(Color::Green))
            .alignment(Alignment::Center);
        frame.render_widget(summary_line, Rect::new(inner.x, y, inner.width, 1));
    } else {
        let summary_line = Paragraph::new("Invalid selection - need 2+ anchors")
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center);
        frame.render_widget(summary_line, Rect::new(inner.x, y, inner.width, 1));
    }
    y += 2;

    // Output path
    let output_path = app.output_dir.join("vgrid.in");
    let path_line = Paragraph::new(format!("Output: {}", output_path.display()))
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(path_line, Rect::new(inner.x + 2, y, inner.width - 4, 1));
    y += 2;

    // Show mesh requirement if not loaded
    if app.mesh_info.is_none() {
        let note = Paragraph::new("No hgrid loaded - run with -g <mesh.gr3>")
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(note, Rect::new(inner.x + 2, y, inner.width - 4, 1));
        y += 2;
    }

    // Show overwrite confirmation if pending
    if app.pending_overwrite {
        let warning = Paragraph::new("File already exists! Overwrite?")
            .style(Style::default().fg(Color::Yellow).bold())
            .alignment(Alignment::Center);
        frame.render_widget(warning, Rect::new(inner.x, y, inner.width, 1));
        y += 2;

        let confirm = Paragraph::new("  [Y] Yes, overwrite    [N] No, cancel")
            .style(Style::default().fg(Color::White));
        frame.render_widget(confirm, Rect::new(inner.x, y, inner.width, 1));
    } else {
        // Main action
        let action_style = if can_export {
            Style::default().fg(Color::Green).bold()
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let action = if can_export {
            "  [Enter] Write vgrid.in"
        } else {
            "  [Enter] Write vgrid.in (disabled)"
        };
        let action_line = Paragraph::new(action).style(action_style);
        frame.render_widget(action_line, Rect::new(inner.x, y, inner.width, 1));
        y += 1;

        // Grayed out future options
        let future_line1 = Paragraph::new("  [1] CLI arguments (coming soon)")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(future_line1, Rect::new(inner.x, y, inner.width, 1));
        y += 1;
        let future_line2 = Paragraph::new("  [2] YAML config (coming soon)")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(future_line2, Rect::new(inner.x, y, inner.width, 1));
    }

    // Footer
    let footer_y = inner.y + inner.height - 1;
    let footer = Paragraph::new("[Esc] Cancel")
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center);
    frame.render_widget(footer, Rect::new(inner.x, footer_y, inner.width, 1));
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    // Clean header: just title and mesh info
    let mesh_info = match &app.mesh_info {
        Some(mesh) => {
            let filename = mesh
                .path
                .file_name()
                .map(|s| s.to_string_lossy())
                .unwrap_or_default();
            format!(
                " │ {} │ {:.1}m - {:.1}m │ {} nodes",
                filename, mesh.min_depth, mesh.max_depth, mesh.node_count
            )
        }
        None => String::new(),
    };

    let header_line = Line::from(vec![
        Span::styled(" VQS Designer", Style::default().fg(Color::Cyan).bold()),
        Span::styled(mesh_info, Style::default().fg(Color::DarkGray)),
    ]);

    let title = Paragraph::new(header_line)
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, area);
}

fn render_body(frame: &mut Frame, area: Rect, app: &mut App) {
    // Single full-width panel - unified profile viewer with editing
    render_unified_viewer(frame, area, app);
}

/// Unified viewer - full-width panel combining profile visualization and anchor editing
fn render_unified_viewer(frame: &mut Frame, area: Rect, app: &mut App) {
    // Title shows suggestion mode indicator if active
    let in_suggestions = app.suggestion_visible;

    let mode_indicator = if in_suggestions {
        " SUGGESTIONS ".to_string()
    } else {
        match app.profile_view_mode {
            ProfileViewMode::SingleDepth => "Single Depth".to_string(),
            ProfileViewMode::MultiDepth => "Multi-Depth".to_string(),
            ProfileViewMode::MeshSummary => "Mesh Summary".to_string(),
        }
    };

    let edit_indicator = match app.anchor_edit_mode {
        AnchorEditMode::Navigate => String::new(),
        AnchorEditMode::AddDepth => format!(" │ Add Depth: {}_", app.anchor_input),
        AnchorEditMode::AddLevels => {
            let depth = app.anchor_pending_depth.unwrap_or(0.0);
            format!(" │ Depth {:.1}m, N: {}_", depth, app.anchor_input)
        }
        AnchorEditMode::EditDepth => format!(" │ Edit Depth: {}_", app.anchor_input),
        AnchorEditMode::EditLevels => format!(" │ Edit N: {}_", app.anchor_input),
    };

    let anchor_count = if in_suggestions {
        app.suggestion_mode.as_ref().map(|m| m.preview.len()).unwrap_or(0)
    } else {
        app.path.anchors.len()
    };

    let title = format!(" {} ({} anchors){} ", mode_indicator, anchor_count, edit_indicator);

    let border_color = if in_suggestions { Color::Magenta } else { Color::Cyan };
    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(border_color).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split-screen: left panel + divider (1 col) + profile view (right)
    let left_pct = app.panel_split;
    let right_pct = 100 - left_pct;

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(left_pct),
            Constraint::Length(1), // Divider
            Constraint::Percentage(right_pct),
        ])
        .split(inner);

    // Store areas for mouse hit detection and drag calculation
    app.table_area = layout[0];    // Left panel
    app.divider_area = layout[1];  // Divider
    app.preview_area = layout[2];  // Right panel (profile view)

    // Left: suggestion controls OR anchor list
    if in_suggestions {
        if let Some(ref mode) = app.suggestion_mode {
            render_suggestion_controls_unified(frame, layout[0], app, mode);
        }
    } else {
        render_anchor_list_panel(frame, layout[0], app);
    }

    // Divider (draggable)
    render_divider(frame, layout[1], app);

    // Right: ALWAYS show profile visualization (even in suggestion mode)
    render_single_depth_profile(frame, layout[2], app);
}

/// Empty state when no anchors are defined
fn render_empty_state(frame: &mut Frame, area: Rect) {
    let center_y = area.y + area.height / 2 - 2;

    let msg1 = Paragraph::new("No anchors defined")
        .style(Style::default().fg(Color::DarkGray).italic())
        .alignment(Alignment::Center);
    frame.render_widget(msg1, Rect::new(area.x, center_y, area.width, 1));

    let msg2 = Paragraph::new("Press [a] to add an anchor, or [S] for suggestions")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(msg2, Rect::new(area.x, center_y + 2, area.width, 1));
}

/// Left panel: anchor list with editing controls
fn render_anchor_list_panel(frame: &mut Frame, area: Rect, app: &App) {
    if app.path.anchors.is_empty() {
        render_empty_state(frame, area);
        return;
    }

    let mut y = area.y;

    // Header
    let header = Line::from(vec![
        Span::styled(format!("{:>3}", "#"), Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
        Span::styled(format!("{:>8}", "Depth"), Style::default().fg(Color::White).bold()),
        Span::raw("  "),
        Span::styled(format!("{:>8}", "N"), Style::default().fg(Color::White).bold()),
    ]);
    frame.render_widget(Paragraph::new(header), Rect::new(area.x, y, area.width, 1));
    y += 1;

    let sep = "─".repeat(area.width.saturating_sub(1) as usize);
    frame.render_widget(
        Paragraph::new(sep).style(Style::default().fg(Color::DarkGray)),
        Rect::new(area.x, y, area.width, 1),
    );
    y += 1;

    // Get truncation data (cached)
    let anchor_depths: Vec<f64> = app.path.anchors.iter().map(|a| a.depth).collect();
    let anchor_nlevels: Vec<usize> = app.path.anchors.iter().map(|a| a.nlevels).collect();
    let truncation_data = app.get_cached_truncation_data(&anchor_depths, &anchor_nlevels);

    // Anchors
    let footer_y = area.y + area.height - 2;
    for (i, anchor) in app.path.anchors.iter().enumerate() {
        if y >= footer_y {
            break;
        }

        let is_selected = i == app.anchor_selected;
        let row_style = if is_selected {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };

        // Format N with truncation indicator
        let (n_text, n_style) = if let Some(trunc) = truncation_data.get(i) {
            if trunc.was_truncated {
                (
                    format!("{:>3}→{:<3}", trunc.requested_levels, trunc.actual_levels),
                    Style::default().fg(Color::Yellow),
                )
            } else {
                (
                    format!("{:>8}", anchor.nlevels),
                    Style::default().fg(Color::Green),
                )
            }
        } else {
            (
                format!("{:>8}", anchor.nlevels),
                Style::default().fg(Color::Green),
            )
        };

        let line = Line::from(vec![
            Span::styled(format!("{:>3}", i + 1), Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled(format!("{:>7.1}m", anchor.depth), Style::default().fg(Color::Green)),
            Span::raw("  "),
            Span::styled(n_text, n_style),
            if is_selected {
                Span::styled(" ←", Style::default().fg(Color::Cyan).bold())
            } else {
                Span::raw("")
            },
        ]);
        frame.render_widget(Paragraph::new(line).style(row_style), Rect::new(area.x, y, area.width, 1));
        y += 1;
    }

    // Footer with controls
    let footer_line = area.y + area.height - 1;
    let footer = Paragraph::new("[a]dd [d]el [e]dit [S]uggest")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, Rect::new(area.x, footer_line, area.width, 1));
}

/// Right panel: single depth bar chart
fn render_single_depth_profile(frame: &mut Frame, area: Rect, app: &App) {
    // Use suggestion preview anchors when suggestions are visible, otherwise use path anchors
    let anchors: Vec<_> = if app.suggestion_visible {
        if let Some(ref mode) = app.suggestion_mode {
            mode.preview.iter().map(|a| (a.depth, a.nlevels)).collect()
        } else {
            vec![]
        }
    } else {
        app.path.anchors.iter().map(|a| (a.depth, a.nlevels)).collect()
    };

    if anchors.is_empty() {
        let msg = Paragraph::new("No anchors - adjust parameters")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(msg, Rect::new(area.x, area.y + area.height / 2, area.width, 1));
        return;
    }

    let depth_idx = app.profile_depth_idx.min(anchors.len().saturating_sub(1));
    let (anchor_depth, anchor_nlevels) = anchors[depth_idx];
    let depth = app.profile_custom_depth.unwrap_or(anchor_depth);
    let nlevels = anchor_nlevels;

    let mut y = area.y;

    // Get first_depth (h_s) from mesh - 10th percentile of positive depths
    // This reference depth controls S-transform stretching behavior
    let first_depth = app.mesh_info.as_ref().map(|m| m.min_depth).unwrap_or(0.1);

    // Header showing key parameters (user can modify with f/F, b/B keys)
    let header = Line::from(vec![
        Span::styled(format!("{:.1}m", depth), Style::default().fg(Color::Green).bold()),
        Span::styled(format!(" {} lvl ", nlevels), Style::default().fg(Color::DarkGray)),
        Span::styled(format!("θf={:.1}", app.export_options.theta_f), Style::default().fg(Color::Cyan)),
        Span::styled(" [f/F] ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("θb={:.1}", app.export_options.theta_b), Style::default().fg(Color::Yellow)),
        Span::styled(" [b/B]", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(header), Rect::new(area.x, y, area.width, 1));
    y += 2;

    let (z_coords, thicknesses, _was_truncated, _actual_levels) = app.get_cached_profile_data(depth, nlevels, first_depth);

    if thicknesses.is_empty() {
        let msg = Paragraph::new("No layers")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(msg, Rect::new(area.x, y, area.width, 1));
        return;
    }

    let max_dz = thicknesses.iter().cloned().fold(0.0, f64::max);
    let min_dz = thicknesses.iter().cloned().fold(f64::INFINITY, f64::min);
    let avg_dz = thicknesses.iter().sum::<f64>() / thicknesses.len() as f64;

    // Calculate adaptive widths based on actual data
    // Sample first and last layer to determine range string width
    let sample_range = format_depth_range(0.0, thicknesses.first().copied().unwrap_or(1.0));
    let range_width = sample_range.len() + 1; // +1 for trailing space
    let dz_width = 7; // " X.XXm " format

    // Allocate remaining space to bar, minimum 10 chars
    let bar_width = (area.width as usize).saturating_sub(range_width + dz_width).max(10);
    let available_height = area.height.saturating_sub(y - area.y + 4) as usize;

    // Pre-compute reference bar lengths for min/avg/max
    let min_bar_len = if max_dz > 0.0 {
        ((min_dz / max_dz) * bar_width as f64) as usize
    } else {
        0
    };
    let avg_bar_len = if max_dz > 0.0 {
        ((avg_dz / max_dz) * bar_width as f64) as usize
    } else {
        0
    };

    // Show layers with depth ranges
    // Each row shows the layer's thickness bar colored by where it falls relative to min/avg/max
    // z_coords[i] is top of layer i, z_coords[i+1] is bottom
    // z values are negative (depth below surface), so we show absolute values
    let layers_to_show = thicknesses.len().min(available_height);
    for (i, dz) in thicknesses.iter().take(layers_to_show).enumerate() {
        // Get depth range for this layer (convert from negative z to positive depth)
        let z_top = z_coords.get(i).copied().unwrap_or(0.0).abs();
        let z_bot = z_coords.get(i + 1).copied().unwrap_or(depth).abs();

        // Format depth range with precision matching layer thickness
        let range_str = format_depth_range(z_top, z_bot);

        // Calculate bar length for this layer's thickness
        let layer_bar_len = if max_dz > 0.0 {
            ((dz / max_dz) * bar_width as f64) as usize
        } else {
            0
        };

        // Color the bar based on where this layer's thickness falls
        // Split the bar into segments: up to min (cyan), min to avg (white), avg to max (yellow)
        let cyan_len = layer_bar_len.min(min_bar_len).max(1);
        let white_len = if layer_bar_len > min_bar_len {
            (layer_bar_len - min_bar_len).min(avg_bar_len.saturating_sub(min_bar_len))
        } else {
            0
        };
        let yellow_len = if layer_bar_len > avg_bar_len {
            layer_bar_len - avg_bar_len
        } else {
            0
        };

        // Color dz based on where it falls: cyan for thin, white for avg, yellow for thick
        let dz_color = if *dz <= min_dz * 1.1 {
            Color::Cyan
        } else if *dz >= max_dz * 0.9 {
            Color::Yellow
        } else {
            Color::White
        };

        let line = Line::from(vec![
            Span::styled(format!("{} ", range_str), Style::default().fg(Color::DarkGray)),
            Span::styled("█".repeat(cyan_len), Style::default().fg(Color::Cyan)),
            Span::styled("█".repeat(white_len), Style::default().fg(Color::White)),
            Span::styled("█".repeat(yellow_len), Style::default().fg(Color::Yellow)),
            Span::styled(format!(" {}", format_dz(*dz).trim()), Style::default().fg(dz_color)),
        ]);
        frame.render_widget(Paragraph::new(line), Rect::new(area.x, y, area.width, 1));
        y += 1;
    }

    if thicknesses.len() > layers_to_show {
        let more = Paragraph::new(format!("... {} more", thicknesses.len() - layers_to_show))
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(more, Rect::new(area.x, y, area.width, 1));
    }

    // Stats footer
    let footer_y = area.y + area.height - 2;
    let ratio = if min_dz > 0.0 { max_dz / min_dz } else { 0.0 };
    let stats = Line::from(vec![
        Span::styled("min:", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{} ", format_dz(min_dz).trim()), Style::default().fg(Color::Cyan)),
        Span::styled("avg:", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{} ", format_dz(avg_dz).trim()), Style::default().fg(Color::White)),
        Span::styled("max:", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{} ", format_dz(max_dz).trim()), Style::default().fg(Color::Yellow)),
        Span::styled(format!("({:.1}x)", ratio), Style::default().fg(Color::Magenta)),
    ]);
    frame.render_widget(Paragraph::new(stats), Rect::new(area.x, footer_y, area.width, 1));
}

/// Right panel: multi-depth stats table
#[allow(dead_code)]
fn render_multi_depth_profile(frame: &mut Frame, area: Rect, app: &App) {
    if app.path.anchors.is_empty() {
        return;
    }

    let mut y = area.y;

    // Header
    let header = Line::from(vec![
        Span::styled(format!("{:>11}", "Depth Range"), Style::default().fg(Color::White).bold()),
        Span::raw(" "),
        Span::styled(format!("{:>7}", "minΔz"), Style::default().fg(Color::White).bold()),
        Span::raw(" "),
        Span::styled(format!("{:>7}", "avgΔz"), Style::default().fg(Color::White).bold()),
        Span::raw(" "),
        Span::styled(format!("{:>7}", "maxΔz"), Style::default().fg(Color::White).bold()),
    ]);
    frame.render_widget(Paragraph::new(header), Rect::new(area.x, y, area.width, 1));
    y += 1;

    let sep = "─".repeat(area.width.saturating_sub(1) as usize);
    frame.render_widget(
        Paragraph::new(sep).style(Style::default().fg(Color::DarkGray)),
        Rect::new(area.x, y, area.width, 1),
    );
    y += 1;

    // Get zone stats (cached)
    let anchor_depths: Vec<f64> = app.path.anchors.iter().map(|a| a.depth).collect();
    let anchor_nlevels: Vec<usize> = app.path.anchors.iter().map(|a| a.nlevels).collect();
    let zone_stats = app.get_cached_zone_stats(&anchor_depths, &anchor_nlevels);

    // Get mesh min depth for first zone start
    let mesh_min = app.mesh_info.as_ref().map(|m| m.min_depth).unwrap_or(0.0);

    // Rows - each row shows a depth range zone
    for (i, anchor) in app.path.anchors.iter().enumerate() {
        if y >= area.y + area.height - 1 {
            break;
        }

        let is_selected = i == app.anchor_selected;
        let (min_dz, avg_dz, max_dz) = if let Some(stats) = zone_stats.get(i) {
            (stats.min_dz, stats.avg_dz, stats.max_dz)
        } else {
            (0.0, 0.0, 0.0)
        };

        // Compute depth range: from previous anchor (or mesh min) to current anchor
        let depth_start = if i == 0 { mesh_min } else { anchor_depths[i - 1] };
        let depth_end = anchor.depth;

        let row_style = if is_selected {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };

        // Format depth range compactly
        let range_str = format!("{:.0}→{:.0}m", depth_start, depth_end);
        let line = Line::from(vec![
            Span::styled(format!("{:>11}", range_str), Style::default().fg(Color::Green)),
            Span::raw(" "),
            Span::styled(format_dz(min_dz), Style::default().fg(Color::Cyan)),
            Span::raw(" "),
            Span::styled(format_dz(avg_dz), Style::default().fg(Color::White)),
            Span::raw(" "),
            Span::styled(format_dz(max_dz), Style::default().fg(Color::Yellow)),
        ]);
        frame.render_widget(Paragraph::new(line).style(row_style), Rect::new(area.x, y, area.width, 1));
        y += 1;
    }
}

/// Right panel: mesh summary
#[allow(dead_code)]
fn render_mesh_summary_profile(frame: &mut Frame, area: Rect, app: &App) {
    let y = area.y;

    if app.mesh_info.is_none() {
        let msg = Paragraph::new("No mesh loaded - run with -g <hgrid.gr3>")
            .style(Style::default().fg(Color::DarkGray).italic());
        frame.render_widget(msg, Rect::new(area.x, y, area.width, 1));
        return;
    }

    let mesh = app.mesh_info.as_ref().unwrap();
    let mut y = area.y;

    // Mesh info
    let mesh_header = Paragraph::new(format!(
        "Mesh: {} ({} nodes)",
        mesh.path.file_name().map(|s| s.to_string_lossy()).unwrap_or_default(),
        mesh.node_count
    )).style(Style::default().fg(Color::Cyan).bold());
    frame.render_widget(mesh_header, Rect::new(area.x, y, area.width, 1));
    y += 1;

    let depth_range = Paragraph::new(format!("Depth: {:.1}m - {:.1}m", mesh.min_depth, mesh.max_depth))
        .style(Style::default().fg(Color::Green));
    frame.render_widget(depth_range, Rect::new(area.x, y, area.width, 1));
    y += 2;

    // Depth percentiles
    let pct_header = Paragraph::new("Depth percentiles:")
        .style(Style::default().fg(Color::White).bold());
    frame.render_widget(pct_header, Rect::new(area.x, y, area.width, 1));
    y += 1;

    let pct_labels = ["10%", "25%", "50%", "75%", "90%"];
    let bar_width = area.width.saturating_sub(16) as usize;

    for (i, &label) in pct_labels.iter().enumerate() {
        let depth = mesh.percentiles[i];
        let bar_len = if mesh.max_depth > 0.0 {
            ((depth / mesh.max_depth) * bar_width as f64) as usize
        } else {
            0
        };
        let bar = "▓".repeat(bar_len);
        let line = Line::from(vec![
            Span::styled(format!("{:>4}", label), Style::default().fg(Color::DarkGray)),
            Span::raw(" "),
            Span::styled(bar, Style::default().fg(Color::Blue)),
            Span::styled(format!(" {:.1}m", depth), Style::default().fg(Color::White)),
        ]);
        frame.render_widget(Paragraph::new(line), Rect::new(area.x, y, area.width, 1));
        y += 1;
    }

    // Coverage
    if !app.path.anchors.is_empty() {
        y += 1;
        let max_anchor_depth = app.path.anchors.last().map(|a| a.depth).unwrap_or(0.0);
        let coverage = if mesh.max_depth > 0.0 {
            (max_anchor_depth / mesh.max_depth * 100.0).min(100.0)
        } else {
            0.0
        };

        let coverage_color = if coverage >= 100.0 {
            Color::Green
        } else if coverage >= 90.0 {
            Color::Yellow
        } else {
            Color::Red
        };

        let coverage_line = Line::from(vec![
            Span::styled("Coverage: ", Style::default().fg(Color::White).bold()),
            Span::styled(format!("{:.0}%", coverage), Style::default().fg(coverage_color).bold()),
            Span::styled(format!(" ({:.1}m / {:.1}m)", max_anchor_depth, mesh.max_depth), Style::default().fg(Color::DarkGray)),
        ]);
        frame.render_widget(Paragraph::new(coverage_line), Rect::new(area.x, y, area.width, 1));
    }
}

/// Full-width suggestion panel - split-screen with draggable divider
#[allow(dead_code)]
fn render_suggestion_panel_fullwidth(frame: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .title(" Suggestions ")
        .title_style(Style::default().fg(Color::Magenta).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(ref mode) = app.suggestion_mode {
        // Split-screen: controls (left) + divider (1 col) + preview (right)
        // Use app.panel_split for the ratio (user adjustable via mouse drag)
        let left_pct = app.panel_split;
        let right_pct = 100 - left_pct;

        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(left_pct),
                Constraint::Length(1), // Divider
                Constraint::Percentage(right_pct),
            ])
            .split(inner);

        // Store areas for mouse hit detection and drag calculation
        app.table_area = layout[0];    // Left panel (controls)
        app.divider_area = layout[1];  // Divider
        app.preview_area = layout[2];  // Right panel (preview)

        // Left: controls
        render_suggestion_controls(frame, layout[0], app, mode);

        // Divider (draggable)
        render_divider(frame, layout[1], app);

        // Right: preview table with truncation display
        render_suggestion_preview_with_truncation(frame, layout[2], app, mode);
    }
}

/// Render draggable vertical divider
fn render_divider(frame: &mut Frame, area: Rect, app: &App) {
    let style = if app.resizing_panels {
        Style::default().fg(Color::Cyan).bold()
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // Draw vertical line with drag indicator
    for y in area.y..area.y + area.height {
        let char = if y == area.y + area.height / 2 {
            "┃" // Center indicator
        } else {
            "│"
        };
        frame.render_widget(
            Paragraph::new(char).style(style),
            Rect::new(area.x, y, 1, 1),
        );
    }
}

/// Suggestion mode controls (left panel)
#[allow(dead_code)]
fn render_suggestion_controls(frame: &mut Frame, area: Rect, app: &App, mode: &super::suggestions::SuggestionMode) {
    let mut y = area.y;

    // Algorithm selector
    let algorithms = [(1, "Exponential"), (2, "Uniform"), (3, "Percentile")];
    let mut spans = vec![Span::styled("Algorithm: ", Style::default().fg(Color::White).bold())];
    for (num, name) in algorithms {
        let is_selected = mode.algorithm.number() == num;
        let style = if is_selected {
            Style::default().fg(Color::Cyan).bold()
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(format!("[{}]{} ", num, name), style));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), Rect::new(area.x, y, area.width, 1));
    y += 1;

    // Description
    let desc = Paragraph::new(mode.algorithm.description())
        .style(Style::default().fg(Color::DarkGray).italic());
    frame.render_widget(desc, Rect::new(area.x, y, area.width, 1));
    y += 2;

    // Parameters header
    let params_header = Paragraph::new("Parameters:")
        .style(Style::default().fg(Color::White).bold());
    frame.render_widget(params_header, Rect::new(area.x, y, area.width, 1));
    y += 1;

    let param_lines = [
        (format!("  Levels: {}", mode.params.target_levels), "[+/-]"),
        (format!("  Anchors: {}", mode.params.num_anchors), "[</>]"),
        (format!("  Shallow: {}", mode.params.shallow_levels), "[↑/↓]"),
        (format!("  Δz_surf: {:.1}m", mode.params.dz_surf), "[[/]]"),
    ];
    for (text, keys) in param_lines {
        let line = Line::from(vec![
            Span::styled(text, Style::default().fg(Color::Cyan)),
            Span::styled(format!(" {}", keys), Style::default().fg(Color::DarkGray)),
        ]);
        frame.render_widget(Paragraph::new(line), Rect::new(area.x, y, area.width, 1));
        y += 1;
    }
    y += 1;

    // Stretching
    let stretch_header = Paragraph::new("Stretching:")
        .style(Style::default().fg(Color::White).bold());
    frame.render_widget(stretch_header, Rect::new(area.x, y, area.width, 1));
    y += 1;

    let stretch_name = match app.export_options.stretching {
        StretchingType::Quadratic => "Quadratic",
        StretchingType::S => "S-transform",
        StretchingType::Shchepetkin2005 => "Shchepetkin2005",
        StretchingType::Shchepetkin2010 => "Shchepetkin2010",
        StretchingType::Geyer => "Geyer",
    };
    let stretch_line = Line::from(vec![
        Span::styled(format!("  {}", stretch_name), Style::default().fg(Color::Green).bold()),
        Span::styled(" [t] [i]info", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(stretch_line), Rect::new(area.x, y, area.width, 1));
    y += 1;

    // Stretch params based on type
    let stretch_params = match app.export_options.stretching {
        StretchingType::Quadratic => format!("  a: {:.1} [a/A]", app.export_options.a_vqs0),
        StretchingType::S => format!("  θf:{:.1}[f/F] θb:{:.1}[b/B]", app.export_options.theta_f, app.export_options.theta_b),
        _ => format!("  θs:{:.1}[s] θb:{:.1}[b] hc:{:.0}[h]", app.export_options.theta_s, app.export_options.theta_b, app.export_options.hc),
    };
    frame.render_widget(
        Paragraph::new(stretch_params).style(Style::default().fg(Color::Cyan)),
        Rect::new(area.x, y, area.width, 1),
    );
    y += 1;

    let dz_bot = format!("  Δz_bot: {:.1}m [z/Z]", app.export_options.dz_bottom_min);
    frame.render_widget(
        Paragraph::new(dz_bot).style(Style::default().fg(Color::Cyan)),
        Rect::new(area.x, y, area.width, 1),
    );

    // Footer with actions
    let footer_y = area.y + area.height - 1;
    let footer = Paragraph::new("[Enter] Accept  [Esc] Cancel")
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(footer, Rect::new(area.x, footer_y, area.width, 1));
}

/// Unified suggestion controls: compact controls at top + preview table below
fn render_suggestion_controls_unified(frame: &mut Frame, area: Rect, app: &App, mode: &super::suggestions::SuggestionMode) {
    let mut y = area.y;

    // Algorithm selector with clear labels
    let alg_name = match mode.algorithm.number() {
        1 => "Exponential",
        2 => "Uniform",
        3 => "Percentile",
        _ => "?",
    };
    let alg_line = Line::from(vec![
        Span::styled("Algorithm: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{} ", alg_name), Style::default().fg(Color::Cyan).bold()),
        Span::styled("[1/2/3]", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(alg_line), Rect::new(area.x, y, area.width, 1));
    y += 1;

    // Parameters line 1: Levels and Anchors
    let line1 = Line::from(vec![
        Span::styled("Levels: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{}", mode.params.target_levels), Style::default().fg(Color::Cyan).bold()),
        Span::styled(" [+/-]  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Anchors: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{}", mode.params.num_anchors), Style::default().fg(Color::Cyan).bold()),
        Span::styled(" [</>]", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(line1), Rect::new(area.x, y, area.width, 1));
    y += 1;

    // Parameters line 2: Surface dz and Shallow levels
    let line2 = Line::from(vec![
        Span::styled("Surface Δz: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:.2}m", mode.params.dz_surf), Style::default().fg(Color::Cyan).bold()),
        Span::styled(" [[/]]  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Shallow: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{}", mode.params.shallow_levels), Style::default().fg(Color::Cyan).bold()),
        Span::styled(" [↑/↓]", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(line2), Rect::new(area.x, y, area.width, 1));
    y += 1;

    // Stretching line
    let stretch_name = match app.export_options.stretching {
        StretchingType::Quadratic => "Quadratic",
        StretchingType::S => "S-transform",
        StretchingType::Shchepetkin2005 => "Shchepetkin2005",
        StretchingType::Shchepetkin2010 => "Shchepetkin2010",
        StretchingType::Geyer => "Geyer",
    };
    let stretch_params = match app.export_options.stretching {
        StretchingType::Quadratic => format!("a={:.1}", app.export_options.a_vqs0),
        StretchingType::S => format!("θf={:.1} θb={:.1}", app.export_options.theta_f, app.export_options.theta_b),
        _ => format!("θs={:.1} θb={:.1}", app.export_options.theta_s, app.export_options.theta_b),
    };
    let line3 = Line::from(vec![
        Span::styled("Stretch: ", Style::default().fg(Color::DarkGray)),
        Span::styled(stretch_name, Style::default().fg(Color::Green).bold()),
        Span::styled(format!(" ({}) ", stretch_params), Style::default().fg(Color::Green)),
        Span::styled("[t]", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(line3), Rect::new(area.x, y, area.width, 1));
    y += 1;

    // Separator
    let sep = "─".repeat(area.width.saturating_sub(1) as usize);
    frame.render_widget(
        Paragraph::new(sep).style(Style::default().fg(Color::DarkGray)),
        Rect::new(area.x, y, area.width, 1),
    );
    y += 1;

    // Preview table header
    let header = Line::from(vec![
        Span::styled(format!("{:>7}", "Depth"), Style::default().fg(Color::White).bold()),
        Span::styled(format!("{:>8}", "N"), Style::default().fg(Color::White).bold()),
        Span::styled(format!("{:>8}", "minΔz"), Style::default().fg(Color::Cyan)),
        Span::styled(format!("{:>8}", "avgΔz"), Style::default().fg(Color::White)),
        Span::styled(format!("{:>8}", "maxΔz"), Style::default().fg(Color::Yellow)),
        Span::raw("   "), // Space for arrow
    ]);
    frame.render_widget(Paragraph::new(header), Rect::new(area.x, y, area.width, 1));
    y += 1;

    // Get zone stats and truncation data for preview
    let anchor_depths: Vec<f64> = mode.preview.iter().map(|a| a.depth).collect();
    let anchor_nlevels: Vec<usize> = mode.preview.iter().map(|a| a.nlevels).collect();
    let zone_stats = app.get_cached_zone_stats(&anchor_depths, &anchor_nlevels);
    let truncation_data = app.get_cached_truncation_data(&anchor_depths, &anchor_nlevels);

    // Preview rows with truncation and selection indicator
    let available_rows = (area.y + area.height).saturating_sub(y + 2) as usize;
    let selected_idx = app.profile_depth_idx.min(mode.preview.len().saturating_sub(1));

    for (i, anchor) in mode.preview.iter().take(available_rows).enumerate() {
        let (min_dz, avg_dz, max_dz) = if let Some(stats) = zone_stats.get(i) {
            (stats.min_dz, stats.avg_dz, stats.max_dz)
        } else {
            (f64::INFINITY, 0.0, 0.0)
        };

        // Format N with truncation indicator (right-aligned)
        let (n_text, n_style) = if let Some(trunc) = truncation_data.get(i) {
            if trunc.was_truncated {
                (
                    format!("{:>3}→{:<3}", trunc.requested_levels, trunc.actual_levels),
                    Style::default().fg(Color::Yellow),
                )
            } else {
                (
                    format!("{:>7}", anchor.nlevels),
                    Style::default().fg(Color::White),
                )
            }
        } else {
            (
                format!("{:>7}", anchor.nlevels),
                Style::default().fg(Color::White),
            )
        };

        let is_selected = i == selected_idx;
        let arrow = if is_selected { " ←" } else { "  " };

        let row = Line::from(vec![
            Span::styled(format!("{:>6.1}m", anchor.depth), Style::default().fg(Color::Green)),
            Span::styled(format!(" {:>7}", n_text), n_style),
            Span::styled(format!("{:>8}", format_dz(min_dz).trim()), Style::default().fg(Color::Cyan)),
            Span::styled(format!("{:>8}", format_dz(avg_dz).trim()), Style::default().fg(Color::White)),
            Span::styled(format!("{:>8}", format_dz(max_dz).trim()), Style::default().fg(Color::Yellow)),
            Span::styled(arrow, Style::default().fg(Color::Cyan).bold()),
        ]);
        frame.render_widget(Paragraph::new(row), Rect::new(area.x, y, area.width, 1));
        y += 1;
    }

    // Footer
    let footer_y = area.y + area.height - 1;
    let footer = Paragraph::new("[Enter] Accept  [Esc] Cancel  [↑/↓] Select  [f/F b/B] θ params")
        .style(Style::default().fg(Color::Magenta));
    frame.render_widget(footer, Rect::new(area.x, footer_y, area.width, 1));
}

/// Suggestion preview table with truncation display (right panel)
#[allow(dead_code)]
fn render_suggestion_preview_with_truncation(frame: &mut Frame, area: Rect, app: &App, mode: &super::suggestions::SuggestionMode) {
    let mut y = area.y;

    // Header
    let header = Line::from(vec![
        Span::styled(format!("{:>3}", "#"), Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
        Span::styled(format!("{:>7}", "Depth"), Style::default().fg(Color::White).bold()),
        Span::raw("  "),
        Span::styled(format!("{:>8}", "N"), Style::default().fg(Color::White).bold()),
        Span::raw("  "),
        Span::styled(format!("{:>7}", "minΔz"), Style::default().fg(Color::White).bold()),
        Span::raw("  "),
        Span::styled(format!("{:>7}", "avgΔz"), Style::default().fg(Color::White).bold()),
        Span::raw("  "),
        Span::styled(format!("{:>7}", "maxΔz"), Style::default().fg(Color::White).bold()),
    ]);
    frame.render_widget(Paragraph::new(header), Rect::new(area.x, y, area.width, 1));
    y += 1;

    let sep = "─".repeat(area.width.saturating_sub(1) as usize);
    frame.render_widget(
        Paragraph::new(sep).style(Style::default().fg(Color::DarkGray)),
        Rect::new(area.x, y, area.width, 1),
    );
    y += 1;

    // Get zone stats and truncation data for preview
    let anchor_depths: Vec<f64> = mode.preview.iter().map(|a| a.depth).collect();
    let anchor_nlevels: Vec<usize> = mode.preview.iter().map(|a| a.nlevels).collect();
    let zone_stats = app.get_cached_zone_stats(&anchor_depths, &anchor_nlevels);
    let truncation_data = app.get_cached_truncation_data(&anchor_depths, &anchor_nlevels);

    // Preview rows
    for (i, anchor) in mode.preview.iter().enumerate() {
        if y >= area.y + area.height {
            break;
        }

        let (min_dz, avg_dz, max_dz) = if let Some(stats) = zone_stats.get(i) {
            (stats.min_dz, stats.avg_dz, stats.max_dz)
        } else {
            (0.0, 0.0, 0.0)
        };

        // Format N with truncation indicator
        let (n_text, n_style) = if let Some(trunc) = truncation_data.get(i) {
            if trunc.was_truncated {
                (
                    format!("{:>3}→{:<3}", trunc.requested_levels, trunc.actual_levels),
                    Style::default().fg(Color::Yellow),
                )
            } else {
                (
                    format!("{:>8}", anchor.nlevels),
                    Style::default().fg(Color::Green),
                )
            }
        } else {
            (
                format!("{:>8}", anchor.nlevels),
                Style::default().fg(Color::Green),
            )
        };

        let line = Line::from(vec![
            Span::styled(format!("{:>3}", i + 1), Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled(format!("{:>6.1}m", anchor.depth), Style::default().fg(Color::Green)),
            Span::raw("  "),
            Span::styled(n_text, n_style),
            Span::raw("  "),
            Span::styled(format_dz(min_dz), Style::default().fg(Color::Cyan)),
            Span::raw("  "),
            Span::styled(format_dz(avg_dz), Style::default().fg(Color::White)),
            Span::raw("  "),
            Span::styled(format_dz(max_dz), Style::default().fg(Color::Yellow)),
        ]);
        frame.render_widget(Paragraph::new(line), Rect::new(area.x, y, area.width, 1));
        y += 1;
    }
}

fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default().borders(Borders::TOP);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Left side: status message or context help
    let left_content = if let Some(ref msg) = app.status_message {
        let style = match msg.level {
            StatusLevel::Info => Style::default().fg(Color::White),
            StatusLevel::Warning => Style::default().fg(Color::Yellow),
            StatusLevel::Error => Style::default().fg(Color::Red),
            StatusLevel::Success => Style::default().fg(Color::Green),
        };
        Line::from(Span::styled(msg.text.as_str(), style))
    } else {
        let help = if app.suggestion_visible {
            "1-3: alg | +/-: lvls | [/]: dz | </>: anch | ↑↓: shal | z/Z: bot | t: stretch"
        } else {
            match app.focus {
                Focus::Table => "a: add | d: del | e: edit | E: export | v: profile view | ?: help",
                Focus::PathPreview | Focus::Export => {
                    "↑/↓: depth | t: stretch | a/A f/F b/B: params | e: export | v: view | ?: help"
                }
            }
        };
        Line::from(Span::styled(help, Style::default().fg(Color::DarkGray)))
    };

    // Right side: mode tabs
    let is_suggest = app.suggestion_visible;
    let mode_tabs = Line::from(vec![
        Span::styled(" [", Style::default().fg(Color::DarkGray)),
        if is_suggest {
            Span::styled("S", Style::default().fg(Color::DarkGray))
        } else {
            Span::styled("S", Style::default().fg(Color::Yellow).bold())
        },
        Span::styled("] Manual ", if !is_suggest {
            Style::default().fg(Color::Green).bold()
        } else {
            Style::default().fg(Color::DarkGray)
        }),
        Span::styled("│", Style::default().fg(Color::DarkGray)),
        Span::styled(" Suggest ", if is_suggest {
            Style::default().fg(Color::Magenta).bold()
        } else {
            Style::default().fg(Color::DarkGray)
        }),
        if is_suggest {
            Span::styled("[Esc]", Style::default().fg(Color::Yellow).bold())
        } else {
            Span::styled("[Esc]", Style::default().fg(Color::DarkGray))
        },
        Span::styled(" ", Style::default()),
    ]);

    // Render left-aligned help text
    let left_para = Paragraph::new(left_content);
    frame.render_widget(left_para, Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(30), inner.height));

    // Render right-aligned mode tabs
    let tabs_width = 28u16;
    let tabs_x = inner.x + inner.width.saturating_sub(tabs_width);
    let tabs_para = Paragraph::new(mode_tabs);
    frame.render_widget(tabs_para, Rect::new(tabs_x, inner.y, tabs_width, inner.height));
}

fn render_help_overlay(frame: &mut Frame, area: Rect) {
    // Center the help popup
    let popup_width = 62u16;
    let popup_height = 36u16;
    let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let help_text = r#"
 VQS Master Grid Designer - Keyboard & Mouse

 NAVIGATION
   ↑/↓ or j/k       Navigate anchors / profile depths
   Tab / Shift+Tab  Switch between anchor editor and profile viewer
   Esc              Return to anchor editor / close dialogs

 ANCHOR EDITOR (left panel)
   a                Add new anchor (depth + levels)
   d                Delete selected anchor
   e / Enter        Edit selected anchor
   c                Clear all anchors
   S                Enter suggestion mode (requires mesh)

 PROFILE VIEWER (right panel)
   v                Cycle view mode (Single/Multi/Mesh)
   ↑/↓              Select depth to visualize
   t                Cycle stretching type (Quad/S/Shch05/Shch10/Geyer)

 SUGGESTION MODE
   1-3              Select algorithm
   + / -            Adjust target levels
   [ / ]            Adjust min dz
   < / >            Adjust number of anchors
   ↑ / ↓            Adjust shallow levels
   z / Z            Adjust min bottom layer thickness
   t                Cycle stretching type
   Enter            Accept suggestions
   Esc              Cancel

 STRETCHING PARAMETERS
   t                Cycle transform type
   i                Show transform info & parameters help
   f / F            Increase / decrease θf (S-transform)
   b / B            Increase / decrease θb
   s / S            Increase / decrease θs (ROMS transforms)
   h / H            Increase / decrease hc (ROMS transforms)
   a / A            Increase / decrease a_vqs0 (Quadratic)

 PANEL RESIZE
   { / }            Shrink / expand left panel
   Mouse drag       Drag the divider to resize

 EXPORT
   E                Open export dialog (from anchor editor)
   e                Open export dialog (from profile viewer)

 OTHER
   ? / F1           Toggle this help
   q                Quit application
"#;

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .title(" Help ")
                .title_style(Style::default().fg(Color::Cyan).bold())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::Black)),
        );

    frame.render_widget(help, popup_area);
}

/// Render the transform help overlay with information about the current stretching function
fn render_transform_help_overlay(frame: &mut Frame, area: Rect, app: &App) {
    // Center the help popup
    let popup_width = 72u16;
    let popup_height = 32u16;
    let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let (title, help_text) = match app.export_options.stretching {
        StretchingType::Quadratic => (
            " Quadratic Transform ",
            r#"
 QUADRATIC STRETCHING FUNCTION

 Description:
   Simple quadratic vertical coordinate transformation. Provides basic
   control over layer distribution with minimal parameters.

 Best for:
   • Quick testing and simple applications
   • Uniform or nearly-uniform layer distributions
   • Cases where computational simplicity is preferred

 Parameters:
   a_vqs0 [-1, 1]    Stretching amplitude
                     • -1: Focus resolution at bottom (thinner bottom layers)
                     •  0: Uniform layer distribution
                     • +1: Focus resolution at surface (thinner surface layers)

 Keyboard:
   a / A             Decrease / increase a_vqs0 by 0.1
   t                 Cycle to next transform type

 Typical values:
   a_vqs0 = -1.0     Bottom-focused (good for benthic processes)
   a_vqs0 =  0.0     Uniform (general purpose)
"#,
        ),
        StretchingType::S => (
            " S-Transform (SCHISM Default) ",
            r#"
 S-TRANSFORM STRETCHING FUNCTION

 Description:
   SCHISM's native S-coordinate transformation using sinh/tanh functions.
   Provides smooth layer distribution with good control over surface and
   bottom resolution.

 Best for:
   • General SCHISM applications
   • Estuarine and coastal modeling
   • Cases requiring balanced surface/bottom resolution

 Parameters:
   θf (theta_f) [0.1, 20]   Surface/bottom focusing intensity
                            • Higher = sharper transition, more concentrated layers
                            • Lower = smoother, more gradual distribution
                            • Typical: 3-5

   θb (theta_b) [0, 1]      Bottom layer focusing weight
                            • 0: Pure surface focusing
                            • 1: Maximum bottom focusing
                            • Typical: 0.5 (balanced)

 Keyboard:
   f / F             Decrease / increase θf by 0.5
   b / B             Decrease / increase θb by 0.1
   t                 Cycle to next transform type

 Typical values:
   θf=3.0, θb=0.5    Balanced resolution (default)
   θf=5.0, θb=0.8    Enhanced bottom resolution
"#,
        ),
        StretchingType::Shchepetkin2005 => (
            " Shchepetkin 2005 (ROMS) ",
            r#"
 SHCHEPETKIN 2005 STRETCHING (vstretching=2)

 Reference:
   Shchepetkin, A.F. and J.C. McWilliams, 2005. The Regional Oceanic
   Modeling System (ROMS): A split-explicit, free-surface, topography-
   following-coordinate oceanic model.

 Description:
   Original UCLA-ROMS stretching function. Uses cosh/sinh functions for
   smooth transitions. Good general-purpose choice for ocean modeling.

 Best for:
   • Legacy ROMS compatibility
   • General ocean modeling
   • Moderate depth ranges (shelf to slope)

 Parameters:
   θs (theta_s) [0, 10]     Surface control parameter
                            • Higher = more surface resolution
                            • Typical: 5-7

   θb (theta_b) [0, 4]      Bottom control parameter
                            • Higher = more bottom resolution
                            • Typical: 0.4-2

   hc [1, 100] meters       Critical depth (stretching transition width)
                            • Controls where stretching transitions
                            • Smaller = sharper transition near surface
                            • Typical: 5-20m

 Keyboard:
   s / S             Decrease / increase θs by 0.5
   b / B             Decrease / increase θb by 0.1
   h / H             Decrease / increase hc by 1m
   t                 Cycle to next transform type
"#,
        ),
        StretchingType::Shchepetkin2010 => (
            " Shchepetkin 2010 (ROMS Double) ",
            r#"
 SHCHEPETKIN 2010 DOUBLE STRETCHING (vstretching=4)

 Reference:
   Shchepetkin, A.F., 2010. UCLA-ROMS User Manual.

 Description:
   Enhanced "double stretching" function that applies stretching twice
   for improved control. Provides better resolution at both surface AND
   bottom simultaneously.

 Best for:
   • Deep ocean applications
   • Cases needing both surface and bottom resolution
   • Thermocline/pycnocline studies
   • When Shchepetkin2005 doesn't provide enough control

 Parameters:
   θs (theta_s) [0, 10]     Surface stretching parameter
                            • Controls surface layer compression
                            • Higher = thinner surface layers
                            • Typical: 5-7

   θb (theta_b) [0, 4]      Bottom stretching parameter
                            • Controls bottom layer compression
                            • Higher = thinner bottom layers
                            • Typical: 0.4-2

   hc [1, 100] meters       Critical depth
                            • Defines the surface layer thickness scale
                            • Typical: 5-50m depending on application

 Keyboard:
   s / S             Decrease / increase θs by 0.5
   b / B             Decrease / increase θb by 0.1
   h / H             Decrease / increase hc by 1m
   t                 Cycle to next transform type
"#,
        ),
        StretchingType::Geyer => (
            " Geyer (Bottom Boundary Layer) ",
            r#"
 R. GEYER STRETCHING FUNCTION (vstretching=3)

 Reference:
   R. Geyer stretching function for enhanced bottom boundary layer
   resolution in shallow coastal and estuarine applications.

 Description:
   Specialized stretching designed for high-resolution bottom boundary
   layer studies. Uses log-cosh functions with a fixed HSCALE=3.0 to
   create very fine resolution near the seabed.

 Best for:
   • Shallow coastal and estuarine modeling
   • Bottom boundary layer studies
   • Sediment transport modeling
   • Benthic ecosystem studies
   • Tidal flats and shallow embayments

 Parameters:
   θs (theta_s) [0, 10]     Surface exponent
                            • Controls surface layer distribution
                            • Lower values = more uniform near surface
                            • Typical: 1-3

   θb (theta_b) [0, 4]      Bottom exponent
                            • Controls bottom layer concentration
                            • Higher = more resolution at seabed
                            • Typical: 1-3

   hc [1, 100] meters       Critical depth
                            • Sets the transition scale
                            • Typically smaller for shallow applications
                            • Typical: 3-10m

 Keyboard:
   s / S             Decrease / increase θs by 0.5
   b / B             Decrease / increase θb by 0.1
   h / H             Decrease / increase hc by 1m
   t                 Cycle to next transform type

 Note: This transform produces very thin bottom layers. Ensure your
 dz_bottom_min setting is appropriate for your timestep.
"#,
        ),
    };

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .title(title)
                .title_style(Style::default().fg(Color::Cyan).bold())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::Black)),
        );

    frame.render_widget(help, popup_area);

    // Render close hint at bottom
    let hint = Paragraph::new(" Press [i] or [Esc] to close ")
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center);
    let hint_area = Rect::new(
        popup_area.x,
        popup_area.y + popup_area.height - 1,
        popup_area.width,
        1,
    );
    frame.render_widget(hint, hint_area);
}
