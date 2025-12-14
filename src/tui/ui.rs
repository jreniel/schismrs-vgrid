//! Main UI layout and rendering
//!
//! Composes all panels into the final TUI layout

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::app::{AnchorEditMode, App, Focus, StatusLevel, StretchingType, ViewMode};
use super::table::EditMode;
use super::colors::get_cell_colors;
use super::preview::render_path_preview;
use super::stretching::{compute_z_with_truncation, compute_layer_thicknesses, StretchingParams};

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
    // Split body: table (left) + divider (1 col) + side panel (right)
    // Use app.panel_split for the ratio (user adjustable)
    let table_pct = app.panel_split;
    let side_pct = 100 - table_pct;

    let body_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(table_pct),
            Constraint::Length(1), // Divider
            Constraint::Percentage(side_pct),
        ])
        .split(area);

    // Render left panel based on view mode
    match app.view_mode {
        ViewMode::Table => render_table(frame, body_layout[0], app),
        ViewMode::Anchors => render_anchor_view(frame, body_layout[0], app),
    }

    render_divider(frame, body_layout[1], app);
    render_side_panel(frame, body_layout[2], app);

    // Store divider area for mouse hit detection
    app.divider_area = body_layout[1];
}

fn render_divider(frame: &mut Frame, area: Rect, app: &App) {
    // Vertical divider that can be dragged to resize panels
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

fn render_side_panel(frame: &mut Frame, area: Rect, app: &mut App) {
    // Single unified panel - either suggestions or path preview
    // Stretching controls are integrated into the panel content
    app.preview_area = area;
    app.export_area = area; // Same area for keyboard focus

    if app.suggestion_mode.is_some() {
        render_suggestion_panel(frame, area, app);
    } else {
        render_path_preview(frame, area, app);
    }
}

fn render_table(frame: &mut Frame, area: Rect, app: &mut App) {
    let is_focused = app.focus == Focus::Table;

    // Calculate dimensions
    let cell_width: u16 = 8;
    let depth_label_width: u16 = 10;
    let header_height: u16 = 2;

    let title = if app.table.edit_mode != EditMode::Navigate {
        match app.table.edit_mode {
            EditMode::AddRow => format!(" Add Depth: {}_ ", app.table.input_buffer),
            EditMode::AddColumn => format!(" Add Min Δz: {}_ ", app.table.input_buffer),
            EditMode::DeleteConfirm => " Delete: [r]ow [c]ol [Esc] ".to_string(),
            _ => " Construction Table ".to_string(),
        }
    } else {
        // Show scroll info in title if scrolled, always show view toggle hint
        let total_rows = app.table.depths.len();
        if app.table_scroll_row > 0 || app.table_scroll_col > 0 {
            format!(" Table [row {}/{}] [v: Anchors] ", app.table_scroll_row + 1, total_rows)
        } else {
            " Construction Table [v: Anchors] ".to_string()
        }
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

    // Store table area for mouse hit detection (inner area)
    app.table_area = inner;

    // Available space for cells (reserve 1 col for scroll indicator)
    let available_cols = ((inner.width.saturating_sub(depth_label_width + 1)) / cell_width) as usize;
    let available_rows = (inner.height.saturating_sub(header_height + 1)) as usize; // Reserve 1 for scroll indicator

    let total_rows = app.table.depths.len();
    let total_cols = app.table.min_dzs.len();

    // Clamp scroll offsets
    let max_row_scroll = total_rows.saturating_sub(available_rows);
    let max_col_scroll = total_cols.saturating_sub(available_cols);
    app.table_scroll_row = app.table_scroll_row.min(max_row_scroll);
    app.table_scroll_col = app.table_scroll_col.min(max_col_scroll);

    // Auto-scroll to keep cursor visible
    let (cursor_row, cursor_col) = app.table.cursor;
    if cursor_row < app.table_scroll_row {
        app.table_scroll_row = cursor_row;
    } else if cursor_row >= app.table_scroll_row + available_rows {
        app.table_scroll_row = cursor_row.saturating_sub(available_rows - 1);
    }
    if cursor_col < app.table_scroll_col {
        app.table_scroll_col = cursor_col;
    } else if cursor_col >= app.table_scroll_col + available_cols {
        app.table_scroll_col = cursor_col.saturating_sub(available_cols - 1);
    }

    // Render column headers (min dz values) with scroll offset
    render_column_headers(
        frame, inner, &app.table.min_dzs,
        depth_label_width, cell_width, available_cols, app.table_scroll_col,
    );

    // Show horizontal scroll indicator in header if needed
    if app.table_scroll_col > 0 {
        let indicator = Paragraph::new("◀").style(Style::default().fg(Color::Yellow));
        frame.render_widget(indicator, Rect::new(inner.x + depth_label_width - 1, inner.y + 1, 1, 1));
    }
    if app.table_scroll_col + available_cols < total_cols {
        let x = inner.x + depth_label_width + (available_cols as u16 * cell_width);
        let indicator = Paragraph::new("▶").style(Style::default().fg(Color::Yellow));
        frame.render_widget(indicator, Rect::new(x, inner.y + 1, 1, 1));
    }

    // Render rows with scroll offset
    let table_start_y = inner.y + header_height;
    let mesh_max_depth = app.mesh_info.as_ref().map(|m| m.max_depth);

    let visible_rows = total_rows.saturating_sub(app.table_scroll_row).min(available_rows);
    for vis_row in 0..visible_rows {
        let row_idx = app.table_scroll_row + vis_row;
        let row_y = table_start_y + vis_row as u16;

        // Depth label
        let depth = app.table.depths[row_idx];
        let label = if depth >= 1000.0 {
            format!("{:>7.0}m", depth)
        } else if depth >= 100.0 {
            format!("{:>7.1}m", depth)
        } else {
            format!("{:>7.2}m", depth)
        };

        let exceeds_mesh = mesh_max_depth.map(|max| depth > max).unwrap_or(false);
        let is_row_selected = app.path.is_depth_selected(row_idx);
        let label_style = if exceeds_mesh {
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)
        } else if app.table.cursor.0 == row_idx {
            Style::default().fg(Color::Yellow).bold()
        } else if is_row_selected {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::White)
        };

        let label_widget = Paragraph::new(label).style(label_style);
        frame.render_widget(label_widget, Rect::new(inner.x, row_y, depth_label_width, 1));

        // Cells for this row with scroll offset
        let visible_cols = total_cols.saturating_sub(app.table_scroll_col).min(available_cols);
        for vis_col in 0..visible_cols {
            let col_idx = app.table_scroll_col + vis_col;
            let cell_x = inner.x + depth_label_width + (vis_col as u16 * cell_width);

            if let Some(cell) = app.table.cell_values.get(row_idx).and_then(|r| r.get(col_idx)) {
                let is_cursor = app.table.cursor == (row_idx, col_idx);
                let is_selected = app.path.is_cell_selected(row_idx, col_idx);

                let (mut fg, mut bg) = get_cell_colors(cell, is_cursor, is_selected);

                if exceeds_mesh && !is_cursor {
                    fg = Color::DarkGray;
                    bg = Color::Reset;
                }

                let text = if cell.validity == super::table::CellValidity::Invalid {
                    "   -   ".to_string()
                } else if cell.validity == super::table::CellValidity::Excessive {
                    "  >120 ".to_string()
                } else {
                    format!("{:^7}", cell.n)
                };

                let mut style = Style::default().fg(fg).bg(bg);
                if exceeds_mesh {
                    style = style.add_modifier(Modifier::DIM);
                }
                let widget = Paragraph::new(text).style(style);
                frame.render_widget(widget, Rect::new(cell_x, row_y, cell_width, 1));
            }
        }
    }

    // Show vertical scroll indicators
    if app.table_scroll_row > 0 {
        let indicator = Paragraph::new(format!("▲{}", app.table_scroll_row))
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(indicator, Rect::new(inner.x, inner.y + header_height - 1, depth_label_width, 1));
    }
    if app.table_scroll_row + available_rows < total_rows {
        let remaining = total_rows - app.table_scroll_row - available_rows;
        let indicator = Paragraph::new(format!("▼{}", remaining))
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(indicator, Rect::new(inner.x, inner.y + inner.height - 1, depth_label_width, 1));
    }
}

/// Render anchor view - direct list of (depth, N) pairs
fn render_anchor_view(frame: &mut Frame, area: Rect, app: &mut App) {
    let is_focused = app.focus == Focus::Table; // Same focus as table

    // Title shows edit mode or view name
    let title = match app.anchor_edit_mode {
        AnchorEditMode::Navigate => " Anchors [v: Table] ".to_string(),
        AnchorEditMode::AddDepth => format!(" Add Depth: {}_ ", app.anchor_input),
        AnchorEditMode::AddLevels => {
            let depth = app.anchor_pending_depth.unwrap_or(0.0);
            format!(" Depth {:.1}m, N: {}_ ", depth, app.anchor_input)
        }
        AnchorEditMode::EditDepth => format!(" Edit Depth: {}_ ", app.anchor_input),
        AnchorEditMode::EditLevels => format!(" Edit N: {}_ ", app.anchor_input),
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

    // Store area for mouse hit detection
    app.table_area = inner;

    let mut y = inner.y;

    // Header row
    let header = Line::from(vec![
        Span::styled(format!("{:>3}", "#"), Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
        Span::styled(format!("{:>8}", "Depth"), Style::default().fg(Color::White).bold()),
        Span::raw("  "),
        Span::styled(format!("{:>4}", "N"), Style::default().fg(Color::White).bold()),
        Span::raw("  "),
        Span::styled(format!("{:>6}", "minΔz"), Style::default().fg(Color::White).bold()),
        Span::raw(" "),
        Span::styled(format!("{:>6}", "avgΔz"), Style::default().fg(Color::White).bold()),
        Span::raw(" "),
        Span::styled(format!("{:>6}", "maxΔz"), Style::default().fg(Color::White).bold()),
    ]);
    frame.render_widget(Paragraph::new(header), Rect::new(inner.x, y, inner.width, 1));
    y += 1;

    // Separator
    let sep = "─".repeat(inner.width.saturating_sub(1) as usize);
    frame.render_widget(
        Paragraph::new(sep).style(Style::default().fg(Color::DarkGray)),
        Rect::new(inner.x, y, inner.width, 1),
    );
    y += 1;

    // Get stretching params for dz computation
    let stretch_params = StretchingParams {
        theta_f: app.export_options.theta_f,
        theta_b: app.export_options.theta_b,
        a_vqs0: app.export_options.a_vqs0,
        etal: 0.0,
    };
    let use_s_transform = matches!(app.export_options.stretching, StretchingType::S);
    let first_depth = app.path.anchors.first().map(|a| a.depth).unwrap_or(1.0);
    let dz_bottom_min = app.export_options.dz_bottom_min;

    // Clamp selected index
    if !app.path.anchors.is_empty() {
        app.anchor_selected = app.anchor_selected.min(app.path.anchors.len() - 1);
    }

    // Render each anchor
    let footer_y = inner.y + inner.height - 2; // Reserve 2 lines for footer
    for (i, anchor) in app.path.anchors.iter().enumerate() {
        if y >= footer_y {
            break;
        }

        let is_selected = i == app.anchor_selected;

        // Compute dz stats with truncation
        let truncation = compute_z_with_truncation(
            anchor.depth,
            anchor.nlevels,
            &stretch_params,
            first_depth,
            dz_bottom_min,
            use_s_transform,
        );
        let thicknesses = compute_layer_thicknesses(&truncation.z_coords);

        let (min_dz, avg_dz, max_dz) = if !thicknesses.is_empty() {
            let min = thicknesses.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = thicknesses.iter().cloned().fold(0.0, f64::max);
            let avg = thicknesses.iter().sum::<f64>() / thicknesses.len() as f64;
            (min, avg, max)
        } else {
            (anchor.depth, anchor.depth, anchor.depth)
        };

        // Format N with truncation indicator
        let n_text = if truncation.was_truncated {
            format!("{:>2}→{:<2}", anchor.nlevels, truncation.actual_levels)
        } else {
            format!("{:>4}", anchor.nlevels)
        };

        let row_style = if is_selected {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };

        let line = Line::from(vec![
            Span::styled(format!("{:>3}", i + 1), Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled(format!("{:>7.1}m", anchor.depth), Style::default().fg(Color::Green)),
            Span::raw("  "),
            Span::styled(
                n_text,
                if truncation.was_truncated {
                    Style::default().fg(Color::Yellow).bold()
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
            Span::raw("  "),
            Span::styled(format!("{:>5.1}m", min_dz), Style::default().fg(Color::Cyan)),
            Span::raw(" "),
            Span::styled(format!("{:>5.1}m", avg_dz), Style::default().fg(Color::White)),
            Span::raw(" "),
            Span::styled(format!("{:>5.1}m", max_dz), Style::default().fg(Color::Yellow)),
            if is_selected {
                Span::styled(" ←", Style::default().fg(Color::Cyan).bold())
            } else {
                Span::raw("")
            },
        ]);

        let para = Paragraph::new(line).style(row_style);
        frame.render_widget(para, Rect::new(inner.x, y, inner.width, 1));
        y += 1;
    }

    // Empty state
    if app.path.anchors.is_empty() {
        let empty = Paragraph::new("  No anchors. Press [a] to add.")
            .style(Style::default().fg(Color::DarkGray).italic());
        frame.render_widget(empty, Rect::new(inner.x, y, inner.width, 1));
    }

    // Footer with controls
    let footer_line = inner.y + inner.height - 1;
    let footer = Paragraph::new("[a] Add  [d] Del  [e/Enter] Edit  [E] Export  [v] Table")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(footer, Rect::new(inner.x, footer_line, inner.width, 1));
}

fn render_column_headers(
    frame: &mut Frame,
    area: Rect,
    min_dzs: &[f64],
    depth_label_width: u16,
    cell_width: u16,
    available_cols: usize,
    scroll_col: usize,
) {
    // First row: "min Δz" label
    let label = Paragraph::new("   min Δz:").style(Style::default().fg(Color::DarkGray));
    frame.render_widget(label, Rect::new(area.x, area.y, depth_label_width, 1));

    // Second row: dz values with scroll offset
    let visible_cols = min_dzs.len().saturating_sub(scroll_col).min(available_cols);
    for vis_col in 0..visible_cols {
        let col_idx = scroll_col + vis_col;
        let dz = min_dzs[col_idx];
        let cell_x = area.x + depth_label_width + (vis_col as u16 * cell_width);

        let text = if dz >= 100.0 {
            format!("{:>6.0}m", dz)
        } else if dz >= 10.0 {
            format!("{:>6.1}m", dz)
        } else {
            format!("{:>6.2}m", dz)
        };

        let style = Style::default().fg(Color::Cyan);
        let widget = Paragraph::new(text).style(style).alignment(Alignment::Center);
        frame.render_widget(widget, Rect::new(cell_x, area.y + 1, cell_width, 1));
    }
}

/// Render suggestion panel in the side panel
fn render_suggestion_panel(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Suggestions ")
        .title_style(Style::default().fg(Color::Magenta).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(ref mode) = app.suggestion_mode {
        let mut y = inner.y;

        // Algorithm selector (compact)
        let algorithms = [(1, "Exp"), (2, "Uni"), (3, "Pct")];
        let mut spans = vec![Span::styled("Alg: ", Style::default().fg(Color::White))];
        for (num, name) in algorithms {
            let is_selected = mode.algorithm.number() == num;
            let style = if is_selected {
                Style::default().fg(Color::Cyan).bold()
            } else {
                Style::default().fg(Color::DarkGray)
            };
            spans.push(Span::styled(format!("[{}]{} ", num, name), style));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), Rect::new(inner.x, y, inner.width, 1));
        y += 1;

        // Description
        let desc = Paragraph::new(mode.algorithm.description())
            .style(Style::default().fg(Color::DarkGray).italic());
        frame.render_widget(desc, Rect::new(inner.x, y, inner.width, 1));
        y += 2;

        // Check if shallow_levels is constrained by min_dz at the first anchor
        let effective_shallow = mode.preview.first().map(|a| a.nlevels).unwrap_or(mode.params.shallow_levels);
        let shallow_constrained = effective_shallow < mode.params.shallow_levels;

        // Parameters (compact, 2 per line)
        let line1 = Line::from(vec![
            Span::styled("Lvls:", Style::default().fg(Color::White)),
            Span::styled(format!("{:>3}", mode.params.target_levels), Style::default().fg(Color::Cyan).bold()),
            Span::styled(" [+/-]  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Anch:", Style::default().fg(Color::White)),
            Span::styled(format!("{:>2}", mode.params.num_anchors), Style::default().fg(Color::Cyan).bold()),
            Span::styled(" [</>]", Style::default().fg(Color::DarkGray)),
        ]);
        frame.render_widget(Paragraph::new(line1), Rect::new(inner.x, y, inner.width, 1));
        y += 1;

        // Show shallow levels with constraint indicator
        let shallow_style = if shallow_constrained {
            Style::default().fg(Color::Yellow).bold()
        } else {
            Style::default().fg(Color::Cyan).bold()
        };
        let shallow_indicator = if shallow_constrained {
            format!("{:>3}→{}", mode.params.shallow_levels, effective_shallow)
        } else {
            format!("{:>3}", mode.params.shallow_levels)
        };

        let line2 = Line::from(vec![
            Span::styled("Shal:", Style::default().fg(Color::White)),
            Span::styled(shallow_indicator, shallow_style),
            Span::styled(" [↑/↓]  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Δz:", Style::default().fg(Color::White)),
            Span::styled(format!("{:>4.1}m", mode.params.min_dz), Style::default().fg(Color::Cyan).bold()),
            Span::styled(" [[/]]", Style::default().fg(Color::DarkGray)),
        ]);
        frame.render_widget(Paragraph::new(line2), Rect::new(inner.x, y, inner.width, 1));
        y += 1;

        // Show constraint warning if applicable
        if shallow_constrained {
            let first_depth = mode.preview.first().map(|a| a.depth).unwrap_or(0.0);
            let max_possible = (first_depth / mode.params.min_dz).floor() as usize + 1;
            let warn = Paragraph::new(format!("  (capped: {:.1}m/{:.1}m = {} max)", first_depth, mode.params.min_dz, max_possible))
                .style(Style::default().fg(Color::Yellow).italic());
            frame.render_widget(warn, Rect::new(inner.x, y, inner.width, 1));
            y += 1;
        }

        // Stretching parameters (affects dz computation)
        let is_s_transform = matches!(app.export_options.stretching, StretchingType::S);
        let stretch_line = if is_s_transform {
            Line::from(vec![
                Span::styled("[t]S ", Style::default().fg(Color::Green).bold()),
                Span::styled("θf:", Style::default().fg(Color::White)),
                Span::styled(format!("{:.1}", app.export_options.theta_f), Style::default().fg(Color::Cyan).bold()),
                Span::styled(" [f/F] ", Style::default().fg(Color::DarkGray)),
                Span::styled("θb:", Style::default().fg(Color::White)),
                Span::styled(format!("{:.1}", app.export_options.theta_b), Style::default().fg(Color::Cyan).bold()),
                Span::styled(" [b/B]", Style::default().fg(Color::DarkGray)),
            ])
        } else {
            Line::from(vec![
                Span::styled("[t]Quad ", Style::default().fg(Color::Green).bold()),
                Span::styled("a:", Style::default().fg(Color::White)),
                Span::styled(format!("{:.1}", app.export_options.a_vqs0), Style::default().fg(Color::Cyan).bold()),
                Span::styled(" [a/A]", Style::default().fg(Color::DarkGray)),
            ])
        };
        frame.render_widget(Paragraph::new(stretch_line), Rect::new(inner.x, y, inner.width, 1));
        y += 1;

        // Bottom layer minimum thickness
        let bottom_line = Line::from(vec![
            Span::styled("Δz_bot:", Style::default().fg(Color::White)),
            Span::styled(format!("{:>4.1}m", app.export_options.dz_bottom_min), Style::default().fg(Color::Cyan).bold()),
            Span::styled(" [z/Z]", Style::default().fg(Color::DarkGray)),
        ]);
        frame.render_widget(Paragraph::new(bottom_line), Rect::new(inner.x, y, inner.width, 1));
        y += 2;

        // Preview header - show N→actual when truncation happens
        let header = Line::from(vec![
            Span::styled(format!("{:>6}", "Depth"), Style::default().fg(Color::White).bold()),
            Span::raw(" "),
            Span::styled(format!("{:>6}", "N"), Style::default().fg(Color::White).bold()),
            Span::raw(" "),
            Span::styled(format!("{:>5}", "minΔz"), Style::default().fg(Color::White).bold()),
            Span::raw(" "),
            Span::styled(format!("{:>5}", "avgΔz"), Style::default().fg(Color::White).bold()),
            Span::raw(" "),
            Span::styled(format!("{:>5}", "maxΔz"), Style::default().fg(Color::White).bold()),
        ]);
        frame.render_widget(Paragraph::new(header), Rect::new(inner.x, y, inner.width, 1));
        y += 1;

        // Separator
        let sep = "─".repeat((inner.width.saturating_sub(1)) as usize);
        frame.render_widget(
            Paragraph::new(sep).style(Style::default().fg(Color::DarkGray)),
            Rect::new(inner.x, y, inner.width, 1),
        );
        y += 1;

        // Get stretching params for dz computation
        let stretch_params = StretchingParams {
            theta_f: app.export_options.theta_f,
            theta_b: app.export_options.theta_b,
            a_vqs0: app.export_options.a_vqs0,
            etal: 0.0,
        };
        let use_s_transform = matches!(app.export_options.stretching, StretchingType::S);
        let first_depth = mode.preview.first().map(|a| a.depth).unwrap_or(1.0);
        let dz_bottom_min = app.export_options.dz_bottom_min;

        // Preview anchors with bottom truncation
        let footer_y = inner.y + inner.height - 1;
        for anchor in &mode.preview {
            if y >= footer_y {
                break;
            }

            // Compute z-coordinates with bottom truncation applied
            let truncation = compute_z_with_truncation(
                anchor.depth,
                anchor.nlevels,
                &stretch_params,
                first_depth,
                dz_bottom_min,
                use_s_transform,
            );

            let thicknesses = compute_layer_thicknesses(&truncation.z_coords);

            let (min_dz, avg_dz, max_dz) = if !thicknesses.is_empty() {
                let min = thicknesses.iter().cloned().fold(f64::INFINITY, f64::min);
                let max = thicknesses.iter().cloned().fold(0.0, f64::max);
                let avg = thicknesses.iter().sum::<f64>() / thicknesses.len() as f64;
                (min, avg, max)
            } else {
                (anchor.depth, anchor.depth, anchor.depth)
            };

            // Show N→actual if truncated, otherwise just N
            let (n_text, n_style) = if truncation.was_truncated {
                (
                    format!("{}→{}", anchor.nlevels, truncation.actual_levels),
                    Style::default().fg(Color::Yellow).bold(),
                )
            } else {
                (
                    format!("{:>3}", truncation.actual_levels),
                    Style::default().fg(Color::Green),
                )
            };

            let line = Line::from(vec![
                Span::styled(format!("{:>5.1}m", anchor.depth), Style::default().fg(Color::Green)),
                Span::raw(" "),
                Span::styled(format!("{:>6}", n_text), n_style),
                Span::raw(" "),
                Span::styled(format!("{:>4.1}m", min_dz), Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::styled(format!("{:>4.1}m", avg_dz), Style::default().fg(Color::White)),
                Span::raw(" "),
                Span::styled(format!("{:>4.1}m", max_dz), Style::default().fg(Color::Yellow)),
            ]);
            frame.render_widget(Paragraph::new(line), Rect::new(inner.x, y, inner.width, 1));
            y += 1;
        }

        // Footer
        let footer = Paragraph::new("[Enter] Accept  [Esc] Cancel")
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        frame.render_widget(footer, Rect::new(inner.x, footer_y, inner.width, 1));
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
        let help = if app.suggestion_mode.is_some() {
            "1-3: alg | +/-: lvls | [/]: dz | </>: anch | ↑↓: shal | z/Z: bot | t f/F b/B: stretch"
        } else {
            match app.focus {
                Focus::Table => {
                    match app.view_mode {
                        ViewMode::Table => "Space: select | a/A: add | d: del | v: anchors | e: export | ?: help",
                        ViewMode::Anchors => "a: add | d: del | e: edit | v: table | E: export | ?: help",
                    }
                }
                Focus::PathPreview | Focus::Export => {
                    "s/q: stretch | f/F b/B: params | e: export | ?: help"
                }
            }
        };
        Line::from(Span::styled(help, Style::default().fg(Color::DarkGray)))
    };

    // Right side: mode tabs
    let is_suggest = app.suggestion_mode.is_some();
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
   arrows / hjkl    Move cursor in table
   Tab / Shift+Tab  Cycle between panels
   Esc              Return to table / close dialogs

 SELECTION (Table panel)
   Space / Enter    Toggle anchor at cursor
   c                Clear all selections
   S                Enter suggestion mode (requires mesh)

 TABLE EDITING
   a                Add new depth row
   A                Add new min Δz column
   d                Delete row or column

 PANEL RESIZE
   { / }            Shrink / expand table panel
   Mouse drag       Drag the divider to resize

 SCROLLING
   Mouse wheel      Scroll table or preview
   Cursor movement  Auto-scrolls to keep cursor visible

 SUGGESTION MODE
   1-3              Select algorithm
   + / -            Adjust target levels
   [ / ]            Adjust min dz
   < / >            Adjust number of anchors
   ↑ / ↓            Adjust shallow levels
   z / Z            Adjust min bottom layer thickness
   t/f/F/b/B        Stretching params (affects dz preview)
   Enter            Accept & update table
   Esc              Cancel

 STRETCHING (right panel)
   s / q            S-transform / Quadratic
   f / F            Increase / decrease θf
   b / B            Increase / decrease θb

 EXPORT
   e                Open export dialog

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
