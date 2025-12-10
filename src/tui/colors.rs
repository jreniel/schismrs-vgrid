//! Cell color coding for the construction table
//!
//! Color scheme based on the LSC2 framework thesis:
//! - Green: N <= 10 (highly efficient)
//! - Blue: N 11-30 (efficient)
//! - Orange: N 31-60 (moderate)
//! - Red text: N 61-120 (computationally intensive)
//! - Red background: N > 120 (exceeds practical limit)
//! - Gray: Invalid cells (depth < dz * 0.5)

use ratatui::style::Color;

use super::table::{CellValidity, CellValue};

/// Returns (foreground, background) colors for a cell
pub fn get_cell_colors(cell: &CellValue, is_cursor: bool, is_selected: bool) -> (Color, Color) {
    let base_fg = match cell.validity {
        CellValidity::Invalid => Color::DarkGray,
        CellValidity::Efficient => Color::Green,
        CellValidity::Good => Color::Blue,
        CellValidity::Moderate => Color::Rgb(255, 165, 0), // Orange
        CellValidity::Intensive => Color::Red,
        CellValidity::Excessive => Color::White, // Text on red bg
    };

    let base_bg = match cell.validity {
        CellValidity::Invalid => Color::Reset,
        CellValidity::Excessive => Color::Red, // Red background for N > 120
        _ => Color::Reset,
    };

    // Apply cursor/selection highlighting
    let bg = if is_cursor {
        Color::White
    } else if is_selected {
        Color::Rgb(50, 100, 50) // Dark green for selected path
    } else {
        base_bg
    };

    let fg = if is_cursor {
        Color::Black
    } else if is_selected && cell.validity != CellValidity::Excessive {
        Color::White
    } else {
        base_fg
    };

    (fg, bg)
}
