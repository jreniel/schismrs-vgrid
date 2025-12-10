//! Main UI layout and rendering
//!
//! Composes all panels into the final TUI layout

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use super::app::{App, Focus, StatusLevel};
use super::table::EditMode;
use super::colors::get_cell_colors;
use super::export::render_export_panel;
use super::preview::render_path_preview;

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

fn render_header(frame: &mut Frame, area: Rect, _app: &App) {
    let title = Paragraph::new("VQS Master Grid Designer - LSC2 Framework")
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

        let is_row_selected = app.path.is_depth_selected(row_idx);
        let label_style = if app.table.cursor.0 == row_idx {
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

                let (fg, bg) = get_cell_colors(cell, is_cursor, is_selected);

                let text = if cell.validity == super::table::CellValidity::Invalid {
                    "   -   ".to_string()
                } else if cell.validity == super::table::CellValidity::Excessive {
                    "  >120 ".to_string()
                } else {
                    format!("{:^7}", cell.n)
                };

                let style = Style::default().fg(fg).bg(bg);
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
        let help = match app.focus {
            Focus::Table => {
                "click/Space: select | a/A: add row/col | d: delete | c: clear | Tab: next | ?: help"
            }
            Focus::PathPreview => "click anchor: remove | Tab: next | Esc: back | ?: help",
            Focus::Export => {
                "click or 1/2/3 s/q f/F b/B v/V: adjust | Enter: export | ?: help"
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
    let popup_width = 60u16;
    let popup_height = 28u16;
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

 MOUSE
   Click table      Select/deselect anchor at cell
   Click preview    Remove anchor from path
   Click export     Change format/stretching/params
   Scroll wheel     Navigate table rows

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
