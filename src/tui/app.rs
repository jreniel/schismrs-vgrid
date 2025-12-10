//! Main application state machine
//!
//! Manages the overall TUI state including the construction table,
//! path selection, focus, and user interactions.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::prelude::Rect;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::path::PathSelection;
use super::stretching::StretchingParams;
use super::table::{ConstructionTable, EditMode};

/// Main application state
pub struct App {
    /// The construction table
    pub table: ConstructionTable,

    /// Current path selection
    pub path: PathSelection,

    /// Active UI focus
    pub focus: Focus,

    /// Optional hgrid path for live VQS generation
    pub hgrid_path: Option<PathBuf>,

    /// Output directory for generated files
    pub output_dir: PathBuf,

    /// Whether to show help overlay
    pub show_help: bool,

    /// Animation frame counter (for spinners)
    pub frame: usize,

    /// Status message (bottom bar)
    pub status_message: Option<StatusMessage>,

    /// Export options
    pub export_options: ExportOptions,

    /// Cached table area for mouse hit detection
    pub table_area: Rect,

    /// Cached export panel area for mouse hit detection
    pub export_area: Rect,

    /// Cached path preview area for mouse hit detection
    pub preview_area: Rect,

    /// Whether the app should quit
    pub should_quit: bool,
}

/// Which panel has focus
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Focus {
    /// Main construction table
    #[default]
    Table,
    /// Right panel showing selected path
    PathPreview,
    /// Export options panel
    Export,
}

/// Status message displayed at the bottom
#[derive(Clone, Debug)]
pub struct StatusMessage {
    pub text: String,
    pub level: StatusLevel,
    pub expires: Instant,
}

/// Status message severity
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusLevel {
    Info,
    Warning,
    Error,
    Success,
}

/// Export configuration options
#[derive(Clone, Debug)]
pub struct ExportOptions {
    pub stretching: StretchingType,
    pub a_vqs0: f64,
    pub theta_f: f64,
    pub theta_b: f64,
    pub output_format: OutputFormat,
}

/// Stretching function type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StretchingType {
    Quadratic,
    S,
}

/// Output format for export
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    /// --depths "..." --nlevels "..." CLI arguments
    CliArgs,
    /// YAML config format for model-config.yml
    Yaml,
    /// Generate vgrid.in directly (requires hgrid)
    VgridFile,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            stretching: StretchingType::S,
            a_vqs0: -1.0,
            theta_f: 3.0,
            theta_b: 0.5,
            output_format: OutputFormat::CliArgs,
        }
    }
}

impl App {
    /// Create a new application with optional hgrid and output directory
    pub fn new(hgrid_path: Option<PathBuf>, output_dir: PathBuf) -> Self {
        Self {
            table: ConstructionTable::new(),
            path: PathSelection::new(),
            focus: Focus::Table,
            hgrid_path,
            output_dir,
            show_help: false,
            frame: 0,
            status_message: None,
            export_options: ExportOptions::default(),
            table_area: Rect::default(),
            export_area: Rect::default(),
            preview_area: Rect::default(),
            should_quit: false,
        }
    }

    /// Create app with custom initial table values
    pub fn with_table(
        depths: Vec<f64>,
        min_dzs: Vec<f64>,
        hgrid_path: Option<PathBuf>,
        output_dir: PathBuf,
    ) -> Self {
        Self {
            table: ConstructionTable::with_values(depths, min_dzs),
            path: PathSelection::new(),
            focus: Focus::Table,
            hgrid_path,
            output_dir,
            show_help: false,
            frame: 0,
            status_message: None,
            export_options: ExportOptions::default(),
            table_area: Rect::default(),
            export_area: Rect::default(),
            preview_area: Rect::default(),
            should_quit: false,
        }
    }

    /// Handle tick events (animation, message expiry)
    pub fn on_tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);

        // Clear expired status messages
        if let Some(ref msg) = self.status_message {
            if Instant::now() > msg.expires {
                self.status_message = None;
            }
        }
    }

    /// Handle key events
    pub fn on_key(&mut self, key: KeyEvent) {
        // Global shortcuts (work in any mode/focus)
        // Note: 'q' quits except in Export panel where it selects Quadratic
        match key.code {
            KeyCode::Char('q') if self.focus != Focus::Export && !matches!(self.table.edit_mode, EditMode::AddRow | EditMode::AddColumn) => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('?') | KeyCode::F(1) => {
                self.show_help = !self.show_help;
                return;
            }
            KeyCode::Esc if self.show_help => {
                self.show_help = false;
                return;
            }
            _ => {}
        }

        // Handle based on edit mode first
        if self.table.edit_mode != EditMode::Navigate {
            self.handle_edit_mode_key(key);
            return;
        }

        // Handle based on focus
        match self.focus {
            Focus::Table => self.handle_table_key(key),
            Focus::PathPreview => self.handle_preview_key(key),
            Focus::Export => self.handle_export_key(key),
        }
    }

    /// Handle mouse events
    pub fn on_mouse(&mut self, mouse: MouseEvent) {
        let x = mouse.column;
        let y = mouse.row;

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Check which panel was clicked
                if self.is_in_rect(x, y, self.table_area) {
                    self.focus = Focus::Table;
                    // Check if within table area
                    if let Some((row, col)) = self.mouse_to_cell(x, y) {
                        // Toggle selection on click
                        if self.path.toggle_anchor(&self.table, row, col) {
                            self.table.cursor = (row, col);
                            if self.path.is_cell_selected(row, col) {
                                self.set_status("Anchor added", StatusLevel::Success);
                            } else {
                                self.set_status("Anchor removed", StatusLevel::Info);
                            }
                        } else {
                            self.set_status("Cannot select this cell", StatusLevel::Warning);
                        }
                    }
                } else if self.is_in_rect(x, y, self.export_area) {
                    self.focus = Focus::Export;
                    self.handle_export_click(x, y);
                } else if self.is_in_rect(x, y, self.preview_area) {
                    self.focus = Focus::PathPreview;
                    self.handle_preview_click(x, y);
                }
            }
            MouseEventKind::ScrollUp => {
                self.table.cursor_up();
            }
            MouseEventKind::ScrollDown => {
                self.table.cursor_down();
            }
            _ => {}
        }
    }

    /// Check if coordinates are within a rect
    fn is_in_rect(&self, x: u16, y: u16, rect: Rect) -> bool {
        x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
    }

    /// Handle click within the export panel
    fn handle_export_click(&mut self, x: u16, y: u16) {
        // Calculate relative position within export panel (accounting for border)
        let rel_y = y.saturating_sub(self.export_area.y + 1);

        // Layout of export panel (0-indexed lines inside border):
        // 0: "Format:"
        // 1:  >[1] CLI Args
        // 2:   [2] YAML
        // 3:   [3] vgrid.in
        // 4: (empty)
        // 5: "Stretching:"
        // 6:  >[s] S-transform
        // 7:   [q] Quadratic
        // 8: (empty)
        // 9: "Parameters:"
        // 10:  θf: 3.0 [f/F]
        // 11:  θb: 0.5 [b/B]
        // 12:  a: -1.0 [v/V]
        // 13: (empty)
        // 14: [Enter] Export

        match rel_y {
            1 => {
                self.export_options.output_format = OutputFormat::CliArgs;
                self.set_status("Format: CLI Arguments", StatusLevel::Info);
            }
            2 => {
                self.export_options.output_format = OutputFormat::Yaml;
                self.set_status("Format: YAML Config", StatusLevel::Info);
            }
            3 => {
                self.export_options.output_format = OutputFormat::VgridFile;
                self.set_status("Format: vgrid.in File", StatusLevel::Info);
            }
            6 => {
                self.export_options.stretching = StretchingType::S;
                self.set_status("Stretching: S-transform", StatusLevel::Info);
            }
            7 => {
                self.export_options.stretching = StretchingType::Quadratic;
                self.set_status("Stretching: Quadratic", StatusLevel::Info);
            }
            10 => {
                // theta_f row - check if clicking left or right half
                let rel_x = x.saturating_sub(self.export_area.x);
                if rel_x > 12 {
                    self.export_options.theta_f = (self.export_options.theta_f - 0.5).max(0.1);
                } else {
                    self.export_options.theta_f = (self.export_options.theta_f + 0.5).min(20.0);
                }
                self.set_status(format!("theta_f: {:.1}", self.export_options.theta_f), StatusLevel::Info);
            }
            11 => {
                // theta_b row
                let rel_x = x.saturating_sub(self.export_area.x);
                if rel_x > 12 {
                    self.export_options.theta_b = (self.export_options.theta_b - 0.1).max(0.0);
                } else {
                    self.export_options.theta_b = (self.export_options.theta_b + 0.1).min(1.0);
                }
                self.set_status(format!("theta_b: {:.1}", self.export_options.theta_b), StatusLevel::Info);
            }
            12 => {
                // a_vqs0 row
                let rel_x = x.saturating_sub(self.export_area.x);
                if rel_x > 12 {
                    self.export_options.a_vqs0 = (self.export_options.a_vqs0 - 0.1).max(-1.0);
                } else {
                    self.export_options.a_vqs0 = (self.export_options.a_vqs0 + 0.1).min(1.0);
                }
                self.set_status(format!("a_vqs0: {:.1}", self.export_options.a_vqs0), StatusLevel::Info);
            }
            14 => {
                // Export button
                self.perform_export();
            }
            _ => {}
        }
    }

    /// Handle click within the preview panel
    fn handle_preview_click(&mut self, _x: u16, y: u16) {
        // Calculate relative position within preview panel (accounting for border)
        let rel_y = y.saturating_sub(self.preview_area.y + 1);

        // Layout of preview panel (0-indexed lines inside border):
        // 0: Header "Depth N minΔz avgΔz maxΔz"
        // 1: Separator "─────────────────────"
        // 2+: Anchor entries

        if rel_y >= 2 {
            let anchor_idx = (rel_y - 2) as usize;
            if anchor_idx < self.path.anchors.len() {
                if let Some(removed) = self.path.remove_anchor_by_index(anchor_idx) {
                    self.set_status(
                        format!("Removed anchor at {:.1}m", removed.depth),
                        StatusLevel::Info,
                    );
                }
            }
        }
    }

    fn handle_table_key(&mut self, key: KeyEvent) {
        match key.code {
            // Navigation
            KeyCode::Up | KeyCode::Char('k') => self.table.cursor_up(),
            KeyCode::Down | KeyCode::Char('j') => self.table.cursor_down(),
            KeyCode::Left | KeyCode::Char('h') => self.table.cursor_left(),
            KeyCode::Right | KeyCode::Char('l') => self.table.cursor_right(),

            // Selection
            KeyCode::Enter | KeyCode::Char(' ') => {
                let (row, col) = self.table.cursor;
                if self.path.toggle_anchor(&self.table, row, col) {
                    if self.path.is_cell_selected(row, col) {
                        self.set_status("Anchor added", StatusLevel::Success);
                    } else {
                        self.set_status("Anchor removed", StatusLevel::Info);
                    }
                } else {
                    self.set_status("Cannot select this cell", StatusLevel::Warning);
                }
            }

            // Table modification
            KeyCode::Char('a') => {
                self.table.edit_mode = EditMode::AddRow;
                self.table.input_buffer.clear();
                self.set_status("Enter depth value (m):", StatusLevel::Info);
            }
            KeyCode::Char('A') => {
                self.table.edit_mode = EditMode::AddColumn;
                self.table.input_buffer.clear();
                self.set_status("Enter min dz value (m):", StatusLevel::Info);
            }
            KeyCode::Char('d') => {
                self.table.edit_mode = EditMode::DeleteConfirm;
                self.set_status(
                    "Delete: [r]ow, [c]olumn, [Esc] cancel",
                    StatusLevel::Warning,
                );
            }

            // Clear path
            KeyCode::Char('c') => {
                self.path.clear();
                self.set_status("Path cleared", StatusLevel::Info);
            }

            // Focus change
            KeyCode::Tab => {
                self.focus = Focus::PathPreview;
            }
            KeyCode::BackTab => {
                self.focus = Focus::Export;
            }

            // Export shortcut
            KeyCode::Char('e') => {
                self.focus = Focus::Export;
            }

            _ => {}
        }
    }

    fn handle_edit_mode_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.table.edit_mode = EditMode::Navigate;
                self.table.input_buffer.clear();
                self.status_message = None;
            }
            KeyCode::Enter => {
                self.commit_edit();
            }
            KeyCode::Char(c) if c.is_ascii_digit() || c == '.' => {
                self.table.input_buffer.push(c);
            }
            KeyCode::Backspace => {
                self.table.input_buffer.pop();
            }
            // Handle delete confirmation
            KeyCode::Char('r') if self.table.edit_mode == EditMode::DeleteConfirm => {
                self.delete_current_row();
                self.table.edit_mode = EditMode::Navigate;
            }
            KeyCode::Char('c') if self.table.edit_mode == EditMode::DeleteConfirm => {
                self.delete_current_column();
                self.table.edit_mode = EditMode::Navigate;
            }
            _ => {}
        }
    }

    fn commit_edit(&mut self) {
        if let Ok(value) = self.table.input_buffer.parse::<f64>() {
            if value > 0.0 {
                match self.table.edit_mode {
                    EditMode::AddRow => {
                        if self.table.add_depth(value) {
                            self.set_status(format!("Added depth: {}m", value), StatusLevel::Success);
                        } else {
                            self.set_status("Depth already exists or invalid", StatusLevel::Error);
                        }
                    }
                    EditMode::AddColumn => {
                        if self.table.add_min_dz(value) {
                            self.set_status(format!("Added min dz: {}m", value), StatusLevel::Success);
                        } else {
                            self.set_status("Min dz already exists or invalid", StatusLevel::Error);
                        }
                    }
                    _ => {}
                }
            } else {
                self.set_status("Value must be positive", StatusLevel::Error);
            }
        } else if !self.table.input_buffer.is_empty() {
            self.set_status("Invalid number format", StatusLevel::Error);
        }

        self.table.edit_mode = EditMode::Navigate;
        self.table.input_buffer.clear();
    }

    fn delete_current_row(&mut self) {
        let row = self.table.cursor.0;

        // Remove any path anchors at this row first
        if self.path.is_depth_selected(row) {
            self.path.anchors.retain(|a| a.depth_idx != row);
        }

        if self.table.remove_depth(row) {
            // Update anchor indices for rows after the deleted one
            for anchor in &mut self.path.anchors {
                if anchor.depth_idx > row {
                    anchor.depth_idx -= 1;
                }
            }
            self.path.validate();
            self.set_status("Row deleted", StatusLevel::Info);
        } else {
            self.set_status("Cannot delete: minimum 2 rows required", StatusLevel::Warning);
        }
    }

    fn delete_current_column(&mut self) {
        let col = self.table.cursor.1;

        if self.table.remove_min_dz(col) {
            // Update path anchor dz indices
            for anchor in &mut self.path.anchors {
                if anchor.dz_idx == col {
                    // Anchor's column was deleted - move to adjacent column
                    anchor.dz_idx = col.saturating_sub(1).min(self.table.min_dzs.len().saturating_sub(1));
                    // Update nlevels from new column
                    if let Some(cell) = self.table.cell_values.get(anchor.depth_idx).and_then(|r| r.get(anchor.dz_idx)) {
                        anchor.nlevels = cell.n;
                    }
                } else if anchor.dz_idx > col {
                    anchor.dz_idx -= 1;
                }
            }
            self.path.validate();
            self.set_status("Column deleted", StatusLevel::Info);
        } else {
            self.set_status("Cannot delete: minimum 1 column required", StatusLevel::Warning);
        }
    }

    fn handle_preview_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Tab => self.focus = Focus::Export,
            KeyCode::BackTab => self.focus = Focus::Table,
            KeyCode::Esc => self.focus = Focus::Table,
            _ => {}
        }
    }

    fn handle_export_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Tab => self.focus = Focus::Table,
            KeyCode::BackTab => self.focus = Focus::PathPreview,
            KeyCode::Esc => self.focus = Focus::Table,
            KeyCode::Char('1') => {
                self.export_options.output_format = OutputFormat::CliArgs;
                self.set_status("Format: CLI Arguments", StatusLevel::Info);
            }
            KeyCode::Char('2') => {
                self.export_options.output_format = OutputFormat::Yaml;
                self.set_status("Format: YAML Config", StatusLevel::Info);
            }
            KeyCode::Char('3') => {
                self.export_options.output_format = OutputFormat::VgridFile;
                self.set_status("Format: vgrid.in File", StatusLevel::Info);
            }
            KeyCode::Char('s') => {
                self.export_options.stretching = StretchingType::S;
                self.set_status("Stretching: S-transform", StatusLevel::Info);
            }
            KeyCode::Char('q') => {
                self.export_options.stretching = StretchingType::Quadratic;
                self.set_status("Stretching: Quadratic", StatusLevel::Info);
            }
            // theta_f adjustment (surface/bottom focusing intensity)
            KeyCode::Char('f') => {
                self.export_options.theta_f = (self.export_options.theta_f + 0.5).min(20.0);
                self.set_status(format!("theta_f: {:.1}", self.export_options.theta_f), StatusLevel::Info);
            }
            KeyCode::Char('F') => {
                self.export_options.theta_f = (self.export_options.theta_f - 0.5).max(0.1);
                self.set_status(format!("theta_f: {:.1}", self.export_options.theta_f), StatusLevel::Info);
            }
            // theta_b adjustment (bottom layer focusing)
            KeyCode::Char('b') => {
                self.export_options.theta_b = (self.export_options.theta_b + 0.1).min(1.0);
                self.set_status(format!("theta_b: {:.1}", self.export_options.theta_b), StatusLevel::Info);
            }
            KeyCode::Char('B') => {
                self.export_options.theta_b = (self.export_options.theta_b - 0.1).max(0.0);
                self.set_status(format!("theta_b: {:.1}", self.export_options.theta_b), StatusLevel::Info);
            }
            // a_vqs0 adjustment (stretching amplitude)
            KeyCode::Char('v') => {
                self.export_options.a_vqs0 = (self.export_options.a_vqs0 + 0.1).min(1.0);
                self.set_status(format!("a_vqs0: {:.1}", self.export_options.a_vqs0), StatusLevel::Info);
            }
            KeyCode::Char('V') => {
                self.export_options.a_vqs0 = (self.export_options.a_vqs0 - 0.1).max(-1.0);
                self.set_status(format!("a_vqs0: {:.1}", self.export_options.a_vqs0), StatusLevel::Info);
            }
            KeyCode::Enter => {
                self.perform_export();
            }
            _ => {}
        }
    }

    /// Get stretching parameters from export options
    pub fn get_stretching_params(&self) -> StretchingParams {
        StretchingParams {
            theta_f: self.export_options.theta_f,
            theta_b: self.export_options.theta_b,
            a_vqs0: self.export_options.a_vqs0,
            etal: 0.0,
        }
    }

    /// Convert mouse coordinates to table cell indices
    fn mouse_to_cell(&self, x: u16, y: u16) -> Option<(usize, usize)> {
        // Check if within table area
        if x < self.table_area.x || y < self.table_area.y {
            return None;
        }
        if x >= self.table_area.x + self.table_area.width
            || y >= self.table_area.y + self.table_area.height
        {
            return None;
        }

        // Account for table structure:
        // - Border: 1 char
        // - Title row: 1 line
        // - Dz header row: 1 line
        // - Depth labels column: ~10 chars
        let header_offset_x: u16 = 10; // Space for depth labels
        let header_offset_y: u16 = 3; // Border + title + dz header
        let cell_width: u16 = 8; // Width of each cell

        let rel_x = x.saturating_sub(self.table_area.x + header_offset_x);
        let rel_y = y.saturating_sub(self.table_area.y + header_offset_y);

        let col = (rel_x / cell_width) as usize;
        let row = rel_y as usize;

        if row < self.table.depths.len() && col < self.table.min_dzs.len() {
            Some((row, col))
        } else {
            None
        }
    }

    /// Set a status message
    pub fn set_status(&mut self, text: impl Into<String>, level: StatusLevel) {
        self.status_message = Some(StatusMessage {
            text: text.into(),
            level,
            expires: Instant::now() + Duration::from_secs(5),
        });
    }

    /// Get spinner character for current frame
    pub fn spinner(&self) -> char {
        const SPINNER: &[char] = &['|', '/', '-', '\\'];
        SPINNER[self.frame % SPINNER.len()]
    }

    fn perform_export(&mut self) {
        if !self.path.is_valid() {
            self.set_status("Cannot export: path is invalid", StatusLevel::Error);
            return;
        }

        match self.export_options.output_format {
            OutputFormat::CliArgs => {
                let output = self.generate_cli_args();
                self.set_status(output, StatusLevel::Success);
            }
            OutputFormat::Yaml => {
                let output = self.generate_yaml();
                self.set_status(format!("YAML:\n{}", output), StatusLevel::Success);
            }
            OutputFormat::VgridFile => {
                if self.hgrid_path.is_none() {
                    self.set_status(
                        "Cannot generate vgrid.in: no hgrid specified (use -g flag)",
                        StatusLevel::Error,
                    );
                    return;
                }
                self.set_status("VGrid generation not yet implemented", StatusLevel::Warning);
            }
        }
    }

    /// Generate CLI arguments string
    pub fn generate_cli_args(&self) -> String {
        let (depths, nlevels) = self.path.to_hsm_config();

        let depths_str: String = depths
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join(" ");

        let nlevels_str: String = nlevels
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(" ");

        let transform = match self.export_options.stretching {
            StretchingType::Quadratic => "quadratic",
            StretchingType::S => "s",
        };

        format!(
            "--transform {} --depths \"{}\" --nlevels \"{}\"",
            transform, depths_str, nlevels_str
        )
    }

    /// Generate YAML configuration
    pub fn generate_yaml(&self) -> String {
        let (depths, nlevels) = self.path.to_hsm_config();

        let depths_yaml: String = depths
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        let nlevels_yaml: String = nlevels
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        let stretching = match self.export_options.stretching {
            StretchingType::Quadratic => "quadratic",
            StretchingType::S => "s",
        };

        format!(
            r#"vgrid:
  type: vqs
  method: hsm
  depths: [{}]
  nlevels: [{}]
  stretching:
    function: {}
    a_vqs0: {}"#,
            depths_yaml, nlevels_yaml, stretching, self.export_options.a_vqs0
        )
    }
}
