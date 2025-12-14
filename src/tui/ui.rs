//! Main UI layout and rendering
//!
//! Composes all panels into the final TUI layout

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use super::app::{App, Focus, StatusLevel, StretchingType};
use super::table::EditMode;
use super::colors::get_cell_colors;
use super::export::render_export_panel;
use super::preview::render_path_preview;
use super::stretching::{compute_s_transform_z, compute_quadratic_z, compute_layer_thicknesses, StretchingParams};

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

    // Help overlay if active
    if app.show_help {
        render_help_overlay(frame, area);
    }
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let title_text = match &app.mesh_info {
        Some(mesh) => {
            let filename = mesh
                .path
                .file_name()
                .map(|s| s.to_string_lossy())
                .unwrap_or_default();
            format!(
                "VQS Designer | {} | {:.1}m - {:.1}m | {} nodes",
                filename, mesh.min_depth, mesh.max_depth, mesh.node_count
            )
        }
        None => "VQS Master Grid Designer - LSC2 Framework (no mesh loaded)".to_string(),
    };

    // Add suggestion mode indicator
    let title_text = if app.suggestion_mode.is_some() {
        format!("{} [SUGGEST]", title_text)
    } else {
        title_text
    };

    let title = Paragraph::new(title_text)
        .style(Style::default().fg(Color::Cyan).bold())
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, area);
}

fn render_body(frame: &mut Frame, area: Rect, app: &mut App) {
    // Split body: table (left) + preview/export (right)
    let body_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(65), // Construction table
            Constraint::Percentage(35), // Preview/export panel
        ])
        .split(area);

    render_table(frame, body_layout[0], app);
    render_side_panel(frame, body_layout[1], app);
}

fn render_side_panel(frame: &mut Frame, area: Rect, app: &mut App) {
    // If in suggestion mode, show suggestion panel instead of path preview
    if app.suggestion_mode.is_some() {
        // Split side panel: suggestion panel (top) + export options (bottom)
        let side_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(55), // Suggestion panel
                Constraint::Percentage(45), // Export options
            ])
            .split(area);

        app.preview_area = side_layout[0];
        app.export_area = side_layout[1];

        render_suggestion_panel(frame, side_layout[0], app);
        render_export_panel(frame, side_layout[1], app);
    } else {
        // Split side panel: path preview (top) + export options (bottom)
        let side_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(55), // Path preview
                Constraint::Percentage(45), // Export options
            ])
            .split(area);

        // Store areas for mouse hit detection
        app.preview_area = side_layout[0];
        app.export_area = side_layout[1];

        render_path_preview(frame, side_layout[0], app);
        render_export_panel(frame, side_layout[1], app);
    }
}

fn render_table(frame: &mut Frame, area: Rect, app: &mut App) {
    let is_focused = app.focus == Focus::Table;

    let title = if app.table.edit_mode != EditMode::Navigate {
        match app.table.edit_mode {
            EditMode::AddRow => format!(" Add Depth: {}_ ", app.table.input_buffer),
            EditMode::AddColumn => format!(" Add Min Δz: {}_ ", app.table.input_buffer),
            EditMode::DeleteConfirm => " Delete: [r]ow [c]ol [Esc] ".to_string(),
            _ => " Construction Table ".to_string(),
        }
    } else {
        " Construction Table ".to_string()
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

    // Calculate cell dimensions
    let cell_width: u16 = 8;
    let depth_label_width: u16 = 10;
    let header_height: u16 = 2;

    // Available space for cells
    let available_cols = ((inner.width.saturating_sub(depth_label_width)) / cell_width) as usize;
    let available_rows = (inner.height.saturating_sub(header_height)) as usize;

    // Render column headers (min dz values)
    render_column_headers(frame, inner, &app.table.min_dzs, depth_label_width, cell_width, available_cols);

    // Render rows
    let table_start_y = inner.y + header_height;
    let num_rows = app.table.depths.len().min(available_rows);

    // Get max depth from mesh if available (for dimming)
    let mesh_max_depth = app.mesh_info.as_ref().map(|m| m.max_depth);

    for row_idx in 0..num_rows {
        let row_y = table_start_y + row_idx as u16;

        // Depth label
        let depth = app.table.depths[row_idx];
        let label = if depth >= 1000.0 {
            format!("{:>7.0}m", depth)
        } else if depth >= 100.0 {
            format!("{:>7.1}m", depth)
        } else {
            format!("{:>7.2}m", depth)
        };

        // Check if row exceeds mesh max depth (should be dimmed)
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
        frame.render_widget(
            label_widget,
            Rect::new(inner.x, row_y, depth_label_width, 1),
        );

        // Cells for this row
        let num_cols = app.table.min_dzs.len().min(available_cols);
        for col_idx in 0..num_cols {
            let cell_x = inner.x + depth_label_width + (col_idx as u16 * cell_width);

            if let Some(cell) = app.table.cell_values.get(row_idx).and_then(|r| r.get(col_idx)) {
                let is_cursor = app.table.cursor == (row_idx, col_idx);
                let is_selected = app.path.is_cell_selected(row_idx, col_idx);

                let (mut fg, mut bg) = get_cell_colors(cell, is_cursor, is_selected);

                // Dim cells in rows exceeding mesh max depth
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
}

fn render_column_headers(
    frame: &mut Frame,
    area: Rect,
    min_dzs: &[f64],
    depth_label_width: u16,
    cell_width: u16,
    available_cols: usize,
) {
    // First row: "min Δz" label
    let label = Paragraph::new("   min Δz:").style(Style::default().fg(Color::DarkGray));
    frame.render_widget(label, Rect::new(area.x, area.y, depth_label_width, 1));

    // Second row: dz values
    let num_cols = min_dzs.len().min(available_cols);
    for (col_idx, &dz) in min_dzs.iter().take(num_cols).enumerate() {
        let cell_x = area.x + depth_label_width + (col_idx as u16 * cell_width);

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

        // Algorithm selector
        let alg_text = format!(
            "Algorithm: [{}] {}",
            mode.algorithm.number(),
            mode.algorithm.name()
        );
        let alg_widget = Paragraph::new(alg_text).style(Style::default().fg(Color::Cyan).bold());
        frame.render_widget(alg_widget, Rect::new(inner.x + 1, y, inner.width - 2, 1));
        y += 1;

        // Algorithm description
        let desc_widget = Paragraph::new(mode.algorithm.description())
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(desc_widget, Rect::new(inner.x + 1, y, inner.width - 2, 1));
        y += 2;

        // Grid Parameters
        let params_label = Paragraph::new("Grid:")
            .style(Style::default().fg(Color::White).bold());
        frame.render_widget(params_label, Rect::new(inner.x + 1, y, inner.width - 2, 1));
        y += 1;

        let target_text = format!("  Levels: {}  [+/-]  Anchors: {}  [</>]",
            mode.params.target_levels, mode.params.num_anchors);
        let target_widget = Paragraph::new(target_text).style(Style::default().fg(Color::White));
        frame.render_widget(target_widget, Rect::new(inner.x + 1, y, inner.width - 2, 1));
        y += 1;

        let shallow_text = format!("  Shallow: {}  [↑/↓]  Min Δz: {:.1}m  [/]",
            mode.params.shallow_levels, mode.params.min_dz);
        let shallow_widget = Paragraph::new(shallow_text).style(Style::default().fg(Color::White));
        frame.render_widget(shallow_widget, Rect::new(inner.x + 1, y, inner.width - 2, 1));
        y += 2;

        // Stretching Parameters
        let stretch_label = Paragraph::new("Stretching:")
            .style(Style::default().fg(Color::White).bold());
        frame.render_widget(stretch_label, Rect::new(inner.x + 1, y, inner.width - 2, 1));
        y += 1;

        let is_quadratic = matches!(app.export_options.stretching, StretchingType::Quadratic);
        let stretch_type = if is_quadratic { "Quadratic" } else { "S-transform" };
        let type_text = format!("  Type: {} [t]", stretch_type);
        let type_widget = Paragraph::new(type_text).style(Style::default().fg(Color::White));
        frame.render_widget(type_widget, Rect::new(inner.x + 1, y, inner.width - 2, 1));
        y += 1;

        if is_quadratic {
            // Quadratic: show a_vqs0
            let a_vqs_text = format!("  a_vqs: {:.1} [a+/-]", app.export_options.a_vqs0);
            let a_vqs_widget = Paragraph::new(a_vqs_text).style(Style::default().fg(Color::White));
            frame.render_widget(a_vqs_widget, Rect::new(inner.x + 1, y, inner.width - 2, 1));
            y += 1;
        } else {
            // S-transform: show theta_f and theta_b
            let theta_text = format!("  θf: {:.1} [f+/-]  θb: {:.1} [b+/-]",
                app.export_options.theta_f, app.export_options.theta_b);
            let theta_widget = Paragraph::new(theta_text).style(Style::default().fg(Color::White));
            frame.render_widget(theta_widget, Rect::new(inner.x + 1, y, inner.width - 2, 1));
            y += 1;
        }
        y += 1;

        // Preview header
        let preview_label = Paragraph::new("Preview (min/avg/max Δz):")
            .style(Style::default().fg(Color::White).bold());
        frame.render_widget(preview_label, Rect::new(inner.x + 1, y, inner.width - 2, 1));
        y += 1;

        // Preview table header
        let header_text = "  Depth     N   min   avg   max";
        let header_widget = Paragraph::new(header_text)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(header_widget, Rect::new(inner.x + 1, y, inner.width - 2, 1));
        y += 1;

        // Get stretching params for computation
        let stretch_params = StretchingParams {
            theta_f: app.export_options.theta_f,
            theta_b: app.export_options.theta_b,
            a_vqs0: app.export_options.a_vqs0,
            etal: 0.0,
        };
        let use_s_transform = matches!(app.export_options.stretching, StretchingType::S);
        let first_depth = mode.preview.first().map(|a| a.depth).unwrap_or(1.0);

        // Preview anchors with computed dz stats
        for anchor in &mode.preview {
            if y >= inner.y + inner.height - 1 {
                break;
            }

            // Compute actual z-coordinates and layer thicknesses
            let z_coords = if use_s_transform {
                compute_s_transform_z(anchor.depth, anchor.nlevels, &stretch_params, first_depth)
            } else {
                compute_quadratic_z(anchor.depth, anchor.nlevels, &stretch_params)
            };
            let thicknesses = compute_layer_thicknesses(&z_coords);

            let (min_dz, avg_dz, max_dz) = if !thicknesses.is_empty() {
                let min = thicknesses.iter().cloned().fold(f64::INFINITY, f64::min);
                let max = thicknesses.iter().cloned().fold(0.0, f64::max);
                let avg = thicknesses.iter().sum::<f64>() / thicknesses.len() as f64;
                (min, avg, max)
            } else {
                let dz = anchor.depth;
                (dz, dz, dz)
            };

            let preview_text = format!(
                "  {:>5.1}m {:>3}  {:.1}  {:.1}  {:.1}",
                anchor.depth, anchor.nlevels, min_dz, avg_dz, max_dz
            );
            let preview_widget = Paragraph::new(preview_text)
                .style(Style::default().fg(Color::Green));
            frame.render_widget(preview_widget, Rect::new(inner.x + 1, y, inner.width - 2, 1));
            y += 1;
        }

        // Footer
        if y < inner.y + inner.height {
            y = inner.y + inner.height - 1;
            let footer_widget = Paragraph::new("[Enter] Accept  [Esc] Cancel")
                .style(Style::default().fg(Color::Yellow))
                .alignment(Alignment::Center);
            frame.render_widget(footer_widget, Rect::new(inner.x, y, inner.width, 1));
        }
    }
}

fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    let content = if let Some(ref msg) = app.status_message {
        let style = match msg.level {
            StatusLevel::Info => Style::default().fg(Color::White),
            StatusLevel::Warning => Style::default().fg(Color::Yellow),
            StatusLevel::Error => Style::default().fg(Color::Red),
            StatusLevel::Success => Style::default().fg(Color::Green),
        };
        Paragraph::new(msg.text.as_str())
            .style(style)
            .wrap(Wrap { trim: true })
    } else {
        let help = if app.suggestion_mode.is_some() {
            "1-4: algorithm | +/-: levels | [/]: min dz | </>: anchors | ↑↓: shallow | Enter: accept | Esc: cancel"
        } else {
            match app.focus {
                Focus::Table => {
                    "Space: select | S: suggest | a/A: add row/col | d: delete | c: clear | Tab: next | ?: help"
                }
                Focus::PathPreview => "click anchor: remove | Tab: next | Esc: back | ?: help",
                Focus::Export => {
                    "click or 1/2/3 s/q f/F b/B v/V: adjust | Enter: export | ?: help"
                }
            }
        };
        Paragraph::new(help).style(Style::default().fg(Color::DarkGray))
    };

    let block = Block::default().borders(Borders::TOP);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(content.alignment(Alignment::Center), inner);
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
   Esc              Return to table / close help

 SELECTION
   Space / Enter    Toggle anchor at cursor
   c                Clear all selections

 SUGGESTION MODE (requires mesh via -g flag)
   S                Enter suggestion mode
   1-4              Select algorithm (Exp/Kmeans/Uniform/Pct)
   + / -            Adjust target levels
   [ / ]            Adjust min dz
   < / >            Adjust number of anchors
   ↑ / ↓            Adjust shallow levels
   Enter            Accept suggestions
   Esc              Cancel suggestion mode

 TABLE EDITING
   a                Add new depth row
   A                Add new min Δz column
   d                Delete row or column

 EXPORT (in Export panel)
   1/2/3 s/q        Format / stretching type
   f/F b/B v/V      Adjust parameters
   Enter            Export configuration

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
