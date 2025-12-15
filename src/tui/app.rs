//! Main application state machine
//!
//! Manages the overall TUI state including the construction table,
//! path selection, focus, and user interactions.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::prelude::Rect;
use schismrs_hgrid::Hgrid;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::path::PathSelection;
use super::stretching::StretchingParams;
use super::table::{ConstructionTable, EditMode};

use crate::transforms::geyer::GeyerOpts;
use crate::transforms::quadratic::QuadraticTransformOpts;
use crate::transforms::s::STransformOpts;
use crate::transforms::shchepetkin2005::Shchepetkin2005Opts;
use crate::transforms::shchepetkin2010::Shchepetkin2010Opts;
use crate::transforms::StretchingFunction;
use crate::vqs::VQSBuilder;

use super::suggestions::SuggestionMode;

/// Information about a loaded mesh (hgrid)
#[derive(Debug)]
pub struct MeshInfo {
    /// Path to the hgrid file
    pub path: PathBuf,
    /// The loaded hgrid (needed for VQS generation)
    pub hgrid: Hgrid,
    /// Number of nodes in the mesh
    pub node_count: usize,
    /// Minimum depth for S-transform reference (10th percentile, floored at 0.1m)
    pub min_depth: f64,
    /// Smallest positive wet node depth (absolute minimum)
    pub min_wet_depth: f64,
    /// Maximum depth
    pub max_depth: f64,
    /// Mean depth
    pub mean_depth: f64,
    /// Median depth
    pub median_depth: f64,
    /// Depth percentiles: 10%, 25%, 50%, 75%, 90%
    pub percentiles: [f64; 5],
}

/// Main application state
pub struct App {
    /// The construction table
    pub table: ConstructionTable,

    /// Current path selection
    pub path: PathSelection,

    /// Active UI focus
    pub focus: Focus,

    /// Loaded mesh information (if hgrid was provided)
    pub mesh_info: Option<MeshInfo>,

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

    /// Suggestion mode state (persistent, keeps params across toggles)
    pub suggestion_mode: Option<SuggestionMode>,

    /// Whether suggestion panel is visible (toggled with S key)
    pub suggestion_visible: bool,

    /// Cached table area for mouse hit detection
    pub table_area: Rect,

    /// Cached export panel area for mouse hit detection
    pub export_area: Rect,

    /// Cached path preview area for mouse hit detection
    pub preview_area: Rect,

    /// Cached divider area for mouse drag detection
    pub divider_area: Rect,

    /// Scroll offset for the path preview anchor list
    pub preview_scroll: usize,

    /// Whether to show export modal
    pub show_export_modal: bool,

    /// Whether the app should quit
    pub should_quit: bool,

    /// Table scroll offset (row)
    pub table_scroll_row: usize,

    /// Table scroll offset (column)
    pub table_scroll_col: usize,

    /// Panel split ratio (percentage for left/anchor panel, 20-80)
    pub panel_split: u16,

    /// Whether we're in panel resize mode (dragging)
    pub resizing_panels: bool,

    /// Profile viewer mode (right panel)
    pub profile_view_mode: ProfileViewMode,

    /// Selected depth for profile viewer (index into anchor depths, or custom)
    pub profile_depth_idx: usize,

    /// Custom depth for profile viewer (when not using anchor depth)
    pub profile_custom_depth: Option<f64>,

    /// Selected anchor index in anchor editor
    pub anchor_selected: usize,

    /// Edit mode for anchor view
    pub anchor_edit_mode: AnchorEditMode,

    /// Input buffer for anchor editing
    pub anchor_input: String,

    /// Temporary depth value when adding anchor (after depth entered, before N)
    pub anchor_pending_depth: Option<f64>,

    /// Pending overwrite confirmation (shows confirm dialog)
    pub pending_overwrite: bool,

    /// Whether to show transform help overlay
    pub show_transform_help: bool,

    /// Cached zone stats to avoid expensive recomputation every frame
    /// Uses RefCell for interior mutability during rendering
    pub cached_zone_stats: std::cell::RefCell<Option<CachedZoneStats>>,

    /// Cached single-depth profile data to avoid expensive recomputation every frame
    pub cached_profile_data: std::cell::RefCell<Option<CachedProfileData>>,

    /// Cached truncation data for all anchors
    pub cached_truncation_data: std::cell::RefCell<Option<CachedTruncationData>>,
}

/// Cached zone statistics with invalidation key
#[derive(Clone, Debug)]
pub struct CachedZoneStats {
    /// The computed zone stats
    pub stats: Vec<super::stretching::ZoneStats>,
    /// Cache key: anchor depths
    pub anchor_depths: Vec<f64>,
    /// Cache key: anchor nlevels
    pub anchor_nlevels: Vec<usize>,
    /// Cache key: stretching type
    pub stretching: StretchingType,
    /// Cache key: theta_f
    pub theta_f: f64,
    /// Cache key: theta_b
    pub theta_b: f64,
    /// Cache key: theta_s
    pub theta_s: f64,
    /// Cache key: hc
    pub hc: f64,
    /// Cache key: a_vqs0
    pub a_vqs0: f64,
}

/// Cached single-depth profile data with invalidation key
#[derive(Clone, Debug)]
pub struct CachedProfileData {
    /// Z-coordinates (depths) for each level
    pub z_coords: Vec<f64>,
    /// Layer thicknesses
    pub thicknesses: Vec<f64>,
    /// Truncation info
    pub was_truncated: bool,
    pub actual_levels: usize,
    /// Cache keys
    pub depth: f64,
    pub nlevels: usize,
    pub stretching: StretchingType,
    pub theta_f: f64,
    pub theta_b: f64,
    pub theta_s: f64,
    pub hc: f64,
    pub a_vqs0: f64,
    pub dz_bottom_min: f64,
    pub first_depth: f64,
}

/// Truncation info for a single anchor
#[derive(Clone, Debug)]
pub struct AnchorTruncation {
    pub requested_levels: usize,
    pub actual_levels: usize,
    pub was_truncated: bool,
}

/// Cached truncation data for all anchors
#[derive(Clone, Debug)]
pub struct CachedTruncationData {
    /// Truncation info per anchor
    pub truncations: Vec<AnchorTruncation>,
    /// Cache keys
    pub anchor_depths: Vec<f64>,
    pub anchor_nlevels: Vec<usize>,
    pub stretching: StretchingType,
    pub theta_f: f64,
    pub theta_b: f64,
    pub theta_s: f64,
    pub hc: f64,
    pub a_vqs0: f64,
    pub dz_bottom_min: f64,
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
    /// S-transform theta_f: surface/bottom focusing intensity (0.1-20)
    pub theta_f: f64,
    /// S-transform theta_b: bottom layer focusing weight (0-1)
    pub theta_b: f64,
    /// ROMS theta_s: surface stretching parameter (0-10)
    pub theta_s: f64,
    /// ROMS hc: critical depth in meters (>0) - controls stretching transition width
    pub hc: f64,
    /// Minimum bottom layer thickness - prevents thin slivers at seabed
    pub dz_bottom_min: f64,
    pub output_format: OutputFormat,
}

/// Stretching function type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StretchingType {
    /// Quadratic stretching (simple, fast)
    Quadratic,
    /// S-transform (SCHISM default)
    S,
    /// Shchepetkin (2005) UCLA-ROMS stretching
    Shchepetkin2005,
    /// Shchepetkin (2010) UCLA-ROMS double stretching
    Shchepetkin2010,
    /// R. Geyer stretching for high bottom boundary layer resolution
    Geyer,
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

/// View mode for the right panel (profile viewer)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ProfileViewMode {
    /// Single depth profile - detailed layer breakdown
    #[default]
    SingleDepth,
    /// Multi-depth comparison - compact profiles at multiple depths
    MultiDepth,
    /// Mesh summary - layer thickness distribution across mesh (requires hgrid)
    MeshSummary,
}

/// Edit mode for anchor view
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AnchorEditMode {
    /// Normal navigation
    #[default]
    Navigate,
    /// Adding new anchor - entering depth
    AddDepth,
    /// Adding new anchor - entering N levels
    AddLevels,
    /// Editing existing anchor depth
    EditDepth,
    /// Editing existing anchor levels
    EditLevels,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            stretching: StretchingType::S,
            a_vqs0: -1.0,
            theta_f: 3.0,
            theta_b: 0.5,
            theta_s: 5.0,
            hc: 5.0,
            dz_bottom_min: 0.5,
            output_format: OutputFormat::CliArgs,
        }
    }
}

impl App {
    /// Create a new application with optional hgrid and output directory
    pub fn new(hgrid_path: Option<PathBuf>, output_dir: PathBuf) -> Self {
        let mut table = ConstructionTable::new();
        let mesh_info = hgrid_path.and_then(|path| Self::load_mesh(&path, &mut table));

        // Start in suggestion mode if mesh is loaded, with dz_surf defaulting to 10th percentile
        let suggestion_mode = mesh_info.as_ref().map(|mesh| {
            SuggestionMode::new_from_mesh(mesh.percentiles[0])
        });

        // Start with suggestions visible if mesh is loaded
        let suggestion_visible = suggestion_mode.is_some();

        let mut app = Self {
            table,
            path: PathSelection::new(),
            focus: Focus::Table,
            mesh_info,
            output_dir,
            show_help: false,
            frame: 0,
            status_message: None,
            export_options: ExportOptions::default(),
            suggestion_mode,
            suggestion_visible,
            table_area: Rect::default(),
            export_area: Rect::default(),
            preview_area: Rect::default(),
            divider_area: Rect::default(),
            preview_scroll: 0,
            show_export_modal: false,
            should_quit: false,
            table_scroll_row: 0,
            table_scroll_col: 0,
            panel_split: 55,
            resizing_panels: false,
            profile_view_mode: ProfileViewMode::default(),
            profile_depth_idx: 0,
            profile_custom_depth: None,
            anchor_selected: 0,
            anchor_edit_mode: AnchorEditMode::default(),
            anchor_input: String::new(),
            anchor_pending_depth: None,
            pending_overwrite: false,
            show_transform_help: false,
            cached_zone_stats: std::cell::RefCell::new(None),
            cached_profile_data: std::cell::RefCell::new(None),
            cached_truncation_data: std::cell::RefCell::new(None),
        };

        // Compute initial suggestions if visible
        if app.suggestion_visible {
            app.compute_suggestions();
        }

        app
    }

    /// Clear all cached data to force recomputation
    pub fn clear_caches(&mut self) {
        *self.cached_zone_stats.borrow_mut() = None;
        *self.cached_profile_data.borrow_mut() = None;
        *self.cached_truncation_data.borrow_mut() = None;
    }

    /// Create app with custom initial anchor values
    pub fn with_anchors(
        depths: Vec<f64>,
        nlevels: Vec<usize>,
        hgrid_path: Option<PathBuf>,
        output_dir: PathBuf,
    ) -> Self {
        let mut table = ConstructionTable::new();
        let mesh_info = hgrid_path.and_then(|path| Self::load_mesh(&path, &mut table));

        // Create suggestion mode if mesh is loaded, but don't show it (we have explicit anchors)
        let suggestion_mode = mesh_info.as_ref().map(|mesh| {
            SuggestionMode::new_from_mesh(mesh.percentiles[0])
        });

        let mut app = Self {
            table,
            path: PathSelection::new(),
            focus: Focus::Table,
            mesh_info,
            output_dir,
            show_help: false,
            frame: 0,
            status_message: None,
            export_options: ExportOptions::default(),
            suggestion_mode,
            suggestion_visible: false, // Don't show suggestions when anchors provided
            table_area: Rect::default(),
            export_area: Rect::default(),
            preview_area: Rect::default(),
            divider_area: Rect::default(),
            preview_scroll: 0,
            show_export_modal: false,
            should_quit: false,
            table_scroll_row: 0,
            table_scroll_col: 0,
            panel_split: 55,
            resizing_panels: false,
            profile_view_mode: ProfileViewMode::default(),
            profile_depth_idx: 0,
            profile_custom_depth: None,
            anchor_selected: 0,
            anchor_edit_mode: AnchorEditMode::default(),
            anchor_input: String::new(),
            anchor_pending_depth: None,
            pending_overwrite: false,
            show_transform_help: false,
            cached_zone_stats: std::cell::RefCell::new(None),
            cached_profile_data: std::cell::RefCell::new(None),
            cached_truncation_data: std::cell::RefCell::new(None),
        };

        // Add provided anchors to path
        for (depth, n) in depths.into_iter().zip(nlevels.into_iter()) {
            app.path.add_direct_anchor(depth, n);
        }
        app.path.validate();

        app
    }

    /// Recompute suggestions based on current algorithm and parameters
    pub fn compute_suggestions(&mut self) {
        let mesh = match &self.mesh_info {
            Some(m) => m,
            None => return,
        };

        if let Some(ref mut mode) = self.suggestion_mode {
            mode.update_preview(mesh);
        }
    }

    /// Load an hgrid file and compute mesh statistics
    /// Returns None if loading fails (error will be logged)
    fn load_mesh(path: &PathBuf, table: &mut ConstructionTable) -> Option<MeshInfo> {
        // Try to load the hgrid
        let hgrid = match Hgrid::try_from(path) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("Warning: Failed to load hgrid '{}': {}", path.display(), e);
                return None;
            }
        };

        // Get ALL depths from mesh in positive-down convention
        let all_depths = hgrid.depths_positive_down();

        // Filter to ONLY positive values (underwater nodes)
        // This explicitly excludes zero and negative depths (dry/land nodes)
        let positive_depths: Vec<f64> = all_depths
            .iter()
            .copied()
            .filter(|&d| d > 0.0)
            .collect();

        if positive_depths.is_empty() {
            eprintln!("Warning: No underwater nodes found in hgrid (no positive depths)");
            return None;
        }

        let node_count = positive_depths.len();
        let max_depth = positive_depths.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let mean_depth = positive_depths.iter().sum::<f64>() / positive_depths.len() as f64;

        // Sort depths to compute percentiles
        let mut sorted_depths = positive_depths.clone();
        sorted_depths.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Smallest positive wet node depth (absolute minimum)
        let min_wet_depth = sorted_depths[0];

        let percentile = |p: f64| -> f64 {
            let idx = (p * (sorted_depths.len() - 1) as f64) as usize;
            sorted_depths[idx]
        };

        let percentiles = [
            percentile(0.10), // 10%
            percentile(0.25), // 25%
            percentile(0.50), // 50% (median)
            percentile(0.75), // 75%
            percentile(0.90), // 90%
        ];

        // Use 10th percentile as min_depth for S-transform reference (h_s)
        // The absolute minimum can be extremely small (e.g., 0.00001m for tidal flats)
        // which makes S-transform degenerate to uniform layers.
        // The 10th percentile provides a more representative "shallow water" reference.
        let min_depth = percentiles[0].max(0.1); // 10th percentile, floor at 0.1m

        let median_depth = percentiles[2];

        // Constrain the table to the mesh depth range
        table.constrain_to_depth(max_depth);

        Some(MeshInfo {
            path: path.clone(),
            hgrid,
            node_count,
            min_depth,
            min_wet_depth,
            max_depth,
            mean_depth,
            median_depth,
            percentiles,
        })
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
            // Transform info: 'i' toggles transform help overlay
            KeyCode::Char('i') if !matches!(self.table.edit_mode, EditMode::AddRow | EditMode::AddColumn) => {
                self.show_transform_help = !self.show_transform_help;
                return;
            }
            KeyCode::Esc if self.show_transform_help => {
                self.show_transform_help = false;
                return;
            }
            // Panel resize: { shrinks left, } expands left
            KeyCode::Char('{') => {
                self.panel_split = self.panel_split.saturating_sub(5).max(20);
                return;
            }
            KeyCode::Char('}') => {
                self.panel_split = (self.panel_split + 5).min(80);
                return;
            }
            // Profile view toggle: 'v' cycles through profile view modes
            KeyCode::Char('v') if self.suggestion_mode.is_none()
                && self.anchor_edit_mode == AnchorEditMode::Navigate => {
                self.profile_view_mode = match self.profile_view_mode {
                    ProfileViewMode::SingleDepth => ProfileViewMode::MultiDepth,
                    ProfileViewMode::MultiDepth => ProfileViewMode::MeshSummary,
                    ProfileViewMode::MeshSummary => ProfileViewMode::SingleDepth,
                };
                let mode_name = match self.profile_view_mode {
                    ProfileViewMode::SingleDepth => "Single Depth",
                    ProfileViewMode::MultiDepth => "Multi-Depth",
                    ProfileViewMode::MeshSummary => "Mesh Summary",
                };
                self.set_status(format!("Profile: {}", mode_name), StatusLevel::Info);
                return;
            }
            _ => {}
        }

        // Handle based on edit mode first (table)
        if self.table.edit_mode != EditMode::Navigate {
            self.handle_edit_mode_key(key);
            return;
        }

        // Handle anchor edit mode
        if self.anchor_edit_mode != AnchorEditMode::Navigate {
            self.handle_anchor_edit_mode_key(key);
            return;
        }

        // Handle export modal if active
        if self.show_export_modal {
            self.handle_export_modal_key(key);
            return;
        }

        // Handle suggestion mode if visible
        if self.suggestion_visible {
            self.handle_suggestion_mode_key(key);
            return;
        }

        // Single unified panel - use anchor view keys (which include profile navigation)
        self.handle_unified_view_key(key);
    }

    /// Handle keyboard input in export modal
    fn handle_export_modal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.show_export_modal = false;
                self.pending_overwrite = false;
            }
            KeyCode::Char('1') => {
                if self.path.is_valid() {
                    let output = self.generate_cli_args();
                    self.show_export_modal = false;
                    self.set_status(format!("CLI: {}", output), StatusLevel::Success);
                }
            }
            KeyCode::Char('2') => {
                if self.path.is_valid() {
                    let output = self.generate_yaml();
                    self.show_export_modal = false;
                    self.set_status(format!("YAML copied to status. Use --output to save."), StatusLevel::Success);
                    // Print to stderr so it can be captured
                    eprintln!("\n{}", output);
                }
            }
            KeyCode::Char('3') | KeyCode::Enter => {
                if self.path.is_valid() && self.mesh_info.is_some() {
                    let output_path = self.output_dir.join("vgrid.in");
                    if output_path.exists() {
                        // File exists - ask for confirmation
                        self.pending_overwrite = true;
                    } else {
                        // File doesn't exist - proceed directly
                        self.export_options.output_format = OutputFormat::VgridFile;
                        self.show_export_modal = false;
                        self.perform_export();
                    }
                } else if !self.path.is_valid() {
                    self.set_status("Cannot export: path is invalid", StatusLevel::Error);
                } else {
                    self.set_status("Cannot export: no hgrid loaded (use -g flag)", StatusLevel::Error);
                }
            }
            // Handle overwrite confirmation
            KeyCode::Char('y') | KeyCode::Char('Y') if self.pending_overwrite => {
                self.pending_overwrite = false;
                self.export_options.output_format = OutputFormat::VgridFile;
                self.show_export_modal = false;
                self.perform_export();
            }
            KeyCode::Char('n') | KeyCode::Char('N') if self.pending_overwrite => {
                self.pending_overwrite = false;
                self.set_status("Export cancelled", StatusLevel::Info);
            }
            _ => {}
        }
    }

    /// Handle mouse events
    pub fn on_mouse(&mut self, mouse: MouseEvent) {
        let x = mouse.column;
        let y = mouse.row;

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Check if clicking on divider for resize
                if self.is_in_rect(x, y, self.divider_area) ||
                   (x > 0 && self.is_in_rect(x - 1, y, self.divider_area)) ||
                   self.is_in_rect(x + 1, y, self.divider_area) {
                    self.resizing_panels = true;
                    return;
                }

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
            MouseEventKind::Up(MouseButton::Left) => {
                self.resizing_panels = false;
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.resizing_panels {
                    // Calculate new split percentage based on mouse position
                    // Total width = left panel + divider + right panel
                    let total_width = self.table_area.width + self.divider_area.width + self.preview_area.width;
                    if total_width > 0 {
                        // Mouse position relative to start of left panel
                        let rel_x = x.saturating_sub(self.table_area.x);
                        let new_split = ((rel_x as u32 * 100) / total_width as u32) as u16;
                        self.panel_split = new_split.clamp(20, 80);
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                // Scroll based on which panel is under cursor
                if self.is_in_rect(x, y, self.preview_area) {
                    if self.preview_scroll > 0 {
                        self.preview_scroll -= 1;
                    }
                } else if self.is_in_rect(x, y, self.table_area) {
                    // Scroll table vertically
                    if self.table_scroll_row > 0 {
                        self.table_scroll_row -= 1;
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                if self.is_in_rect(x, y, self.preview_area) {
                    let max_scroll = self.path.anchors.len().saturating_sub(1);
                    if self.preview_scroll < max_scroll {
                        self.preview_scroll += 1;
                    }
                } else if self.is_in_rect(x, y, self.table_area) {
                    // Scroll table vertically
                    self.table_scroll_row += 1; // Will be clamped in render
                }
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

    /// Handle keyboard input in unified view (combines anchor editing + profile controls)
    fn handle_unified_view_key(&mut self, key: KeyEvent) {
        match key.code {
            // Navigation - moves both anchor selection and profile depth
            KeyCode::Up | KeyCode::Char('k') => {
                if self.anchor_selected > 0 {
                    self.anchor_selected -= 1;
                    self.profile_depth_idx = self.anchor_selected;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.path.anchors.is_empty() && self.anchor_selected < self.path.anchors.len() - 1 {
                    self.anchor_selected += 1;
                    self.profile_depth_idx = self.anchor_selected;
                }
            }
            KeyCode::Home => {
                self.anchor_selected = 0;
                self.profile_depth_idx = 0;
            }
            KeyCode::End => {
                if !self.path.anchors.is_empty() {
                    self.anchor_selected = self.path.anchors.len() - 1;
                    self.profile_depth_idx = self.anchor_selected;
                }
            }

            // Add new anchor
            KeyCode::Char('a') => {
                self.anchor_edit_mode = AnchorEditMode::AddDepth;
                self.anchor_input.clear();
                self.anchor_pending_depth = None;
                self.set_status("Enter depth (m):", StatusLevel::Info);
            }

            // Delete selected anchor
            KeyCode::Char('d') => {
                if !self.path.anchors.is_empty() {
                    let idx = self.anchor_selected;
                    self.path.anchors.remove(idx);
                    // Adjust selection
                    if self.anchor_selected >= self.path.anchors.len() && self.anchor_selected > 0 {
                        self.anchor_selected -= 1;
                    }
                    self.profile_depth_idx = self.anchor_selected;
                    self.path.validate();
                    self.set_status("Anchor deleted", StatusLevel::Info);
                } else {
                    self.set_status("No anchors to delete", StatusLevel::Warning);
                }
            }

            // Edit selected anchor
            KeyCode::Char('e') | KeyCode::Enter => {
                if !self.path.anchors.is_empty() {
                    self.anchor_edit_mode = AnchorEditMode::EditDepth;
                    let anchor = &self.path.anchors[self.anchor_selected];
                    self.anchor_input = format!("{:.1}", anchor.depth);
                    self.set_status("Edit depth (Enter to keep, then N):", StatusLevel::Info);
                }
            }

            // Clear all anchors
            KeyCode::Char('c') => {
                self.path.clear();
                self.anchor_selected = 0;
                self.profile_depth_idx = 0;
                self.set_status("All anchors cleared", StatusLevel::Info);
            }

            // Export modal
            KeyCode::Char('E') => {
                self.show_export_modal = true;
            }

            // Toggle suggestion mode
            KeyCode::Char('S') => {
                if self.mesh_info.is_none() {
                    self.set_status("Suggestions require mesh (-g)", StatusLevel::Warning);
                } else if self.suggestion_visible {
                    // Hide suggestions
                    self.suggestion_visible = false;
                    self.set_status("Suggestions hidden", StatusLevel::Info);
                } else {
                    // Show suggestions - sync from current anchors if we have any
                    self.suggestion_visible = true;

                    // Sync suggestion params from current anchor table
                    if !self.path.anchors.is_empty() {
                        let anchors: Vec<(f64, usize)> = self.path.anchors
                            .iter()
                            .map(|a| (a.depth, a.nlevels))
                            .collect();
                        if let Some(ref mut mode) = self.suggestion_mode {
                            mode.sync_from_anchors(&anchors);
                        }
                    }
                    self.compute_suggestions();
                    self.set_status("Suggestions (synced from table)", StatusLevel::Info);
                }
            }

            // View mode cycle
            KeyCode::Char('v') => {
                self.profile_view_mode = match self.profile_view_mode {
                    ProfileViewMode::SingleDepth => ProfileViewMode::MultiDepth,
                    ProfileViewMode::MultiDepth => ProfileViewMode::MeshSummary,
                    ProfileViewMode::MeshSummary => ProfileViewMode::SingleDepth,
                };
                let name = match self.profile_view_mode {
                    ProfileViewMode::SingleDepth => "Single Depth",
                    ProfileViewMode::MultiDepth => "Multi-Depth",
                    ProfileViewMode::MeshSummary => "Mesh Summary",
                };
                self.set_status(format!("View: {}", name), StatusLevel::Info);
            }

            // Stretching type cycle
            KeyCode::Char('t') => {
                self.export_options.stretching = match self.export_options.stretching {
                    StretchingType::Quadratic => StretchingType::S,
                    StretchingType::S => StretchingType::Shchepetkin2005,
                    StretchingType::Shchepetkin2005 => StretchingType::Shchepetkin2010,
                    StretchingType::Shchepetkin2010 => StretchingType::Geyer,
                    StretchingType::Geyer => StretchingType::Quadratic,
                };
                let name = match self.export_options.stretching {
                    StretchingType::Quadratic => "Quadratic",
                    StretchingType::S => "S-transform",
                    StretchingType::Shchepetkin2005 => "Shchepetkin2005",
                    StretchingType::Shchepetkin2010 => "Shchepetkin2010",
                    StretchingType::Geyer => "Geyer",
                };
                self.set_status(format!("Transform: {}", name), StatusLevel::Info);
            }

            // Stretching parameter controls
            KeyCode::Char('f') => {
                self.export_options.theta_f = (self.export_options.theta_f + 0.5).min(20.0);
                self.set_status(format!("θf: {:.1}", self.export_options.theta_f), StatusLevel::Info);
            }
            KeyCode::Char('F') => {
                self.export_options.theta_f = (self.export_options.theta_f - 0.5).max(0.1);
                self.set_status(format!("θf: {:.1}", self.export_options.theta_f), StatusLevel::Info);
            }
            KeyCode::Char('b') => {
                self.export_options.theta_b = (self.export_options.theta_b + 0.1).min(4.0);
                self.set_status(format!("θb: {:.1}", self.export_options.theta_b), StatusLevel::Info);
            }
            KeyCode::Char('B') => {
                self.export_options.theta_b = (self.export_options.theta_b - 0.1).max(0.0);
                self.set_status(format!("θb: {:.1}", self.export_options.theta_b), StatusLevel::Info);
            }
            KeyCode::Char('s') => {
                self.export_options.theta_s = (self.export_options.theta_s + 0.5).min(10.0);
                self.set_status(format!("θs: {:.1}", self.export_options.theta_s), StatusLevel::Info);
            }
            KeyCode::Char('h') => {
                self.export_options.hc = (self.export_options.hc + 1.0).min(100.0);
                self.set_status(format!("hc: {:.0}m", self.export_options.hc), StatusLevel::Info);
            }
            KeyCode::Char('H') => {
                self.export_options.hc = (self.export_options.hc - 1.0).max(1.0);
                self.set_status(format!("hc: {:.0}m", self.export_options.hc), StatusLevel::Info);
            }
            KeyCode::Char('A') => {
                self.export_options.a_vqs0 = (self.export_options.a_vqs0 + 0.1).min(1.0);
                self.set_status(format!("a: {:.1}", self.export_options.a_vqs0), StatusLevel::Info);
            }
            KeyCode::Char('z') => {
                self.export_options.dz_bottom_min = (self.export_options.dz_bottom_min - 0.1).max(0.1);
                self.set_status(format!("Δz_bot: {:.1}m", self.export_options.dz_bottom_min), StatusLevel::Info);
            }
            KeyCode::Char('Z') => {
                self.export_options.dz_bottom_min += 0.1;
                self.set_status(format!("Δz_bot: {:.1}m", self.export_options.dz_bottom_min), StatusLevel::Info);
            }

            _ => {}
        }
    }

    /// Handle keyboard input when editing an anchor
    fn handle_anchor_edit_mode_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.anchor_edit_mode = AnchorEditMode::Navigate;
                self.anchor_input.clear();
                self.anchor_pending_depth = None;
                self.status_message = None;
            }
            KeyCode::Enter => {
                self.commit_anchor_edit();
            }
            KeyCode::Char(c) if c.is_ascii_digit() || c == '.' => {
                self.anchor_input.push(c);
            }
            KeyCode::Backspace => {
                self.anchor_input.pop();
            }
            _ => {}
        }
    }

    /// Commit the current anchor edit
    fn commit_anchor_edit(&mut self) {
        match self.anchor_edit_mode {
            AnchorEditMode::AddDepth => {
                // Parse depth, then prompt for N
                if let Ok(depth) = self.anchor_input.parse::<f64>() {
                    if depth > 0.0 {
                        self.anchor_pending_depth = Some(depth);
                        self.anchor_edit_mode = AnchorEditMode::AddLevels;
                        self.anchor_input.clear();
                        self.set_status(format!("Depth: {:.1}m. Enter N levels:", depth), StatusLevel::Info);
                        return;
                    } else {
                        self.set_status("Depth must be positive", StatusLevel::Error);
                    }
                } else if !self.anchor_input.is_empty() {
                    self.set_status("Invalid number format", StatusLevel::Error);
                }
            }
            AnchorEditMode::AddLevels => {
                // Parse N levels, add the anchor
                if let (Some(depth), Ok(nlevels)) = (self.anchor_pending_depth, self.anchor_input.parse::<usize>()) {
                    if nlevels >= 2 {
                        // Add anchor directly to path (not through table)
                        self.path.add_direct_anchor(depth, nlevels);
                        self.path.validate();
                        // Select the new anchor
                        if let Some(idx) = self.path.anchors.iter().position(|a| (a.depth - depth).abs() < 0.001) {
                            self.anchor_selected = idx;
                        }
                        self.set_status(format!("Added anchor: {:.1}m, {} levels", depth, nlevels), StatusLevel::Success);
                    } else {
                        self.set_status("N must be >= 2", StatusLevel::Error);
                        return;
                    }
                } else if !self.anchor_input.is_empty() {
                    self.set_status("Invalid number format", StatusLevel::Error);
                    return;
                }
            }
            AnchorEditMode::EditDepth => {
                // Parse new depth, then prompt for N
                if let Ok(depth) = self.anchor_input.parse::<f64>() {
                    if depth > 0.0 {
                        self.anchor_pending_depth = Some(depth);
                        self.anchor_edit_mode = AnchorEditMode::EditLevels;
                        // Pre-fill with current N
                        let current_n = self.path.anchors.get(self.anchor_selected).map(|a| a.nlevels).unwrap_or(2);
                        self.anchor_input = current_n.to_string();
                        self.set_status(format!("Depth: {:.1}m. Edit N levels:", depth), StatusLevel::Info);
                        return;
                    } else {
                        self.set_status("Depth must be positive", StatusLevel::Error);
                    }
                } else if !self.anchor_input.is_empty() {
                    self.set_status("Invalid number format", StatusLevel::Error);
                }
            }
            AnchorEditMode::EditLevels => {
                // Parse N levels, update the anchor
                if let (Some(depth), Ok(nlevels)) = (self.anchor_pending_depth, self.anchor_input.parse::<usize>()) {
                    if nlevels >= 2 {
                        if let Some(anchor) = self.path.anchors.get_mut(self.anchor_selected) {
                            anchor.depth = depth;
                            anchor.nlevels = nlevels;
                            // Clear table indices since this is now a direct anchor
                            anchor.depth_idx = usize::MAX;
                            anchor.dz_idx = usize::MAX;
                        }
                        self.path.validate();
                        self.set_status(format!("Updated anchor: {:.1}m, {} levels", depth, nlevels), StatusLevel::Success);
                    } else {
                        self.set_status("N must be >= 2", StatusLevel::Error);
                        return;
                    }
                } else if !self.anchor_input.is_empty() {
                    self.set_status("Invalid number format", StatusLevel::Error);
                    return;
                }
            }
            AnchorEditMode::Navigate => {}
        }

        // Reset edit state
        self.anchor_edit_mode = AnchorEditMode::Navigate;
        self.anchor_input.clear();
        self.anchor_pending_depth = None;
    }

    /// Handle keyboard input in suggestion mode
    fn handle_suggestion_mode_key(&mut self, key: KeyEvent) {
        // Handle Esc first - hide suggestion panel (but keep state)
        if key.code == KeyCode::Esc {
            self.suggestion_visible = false;
            self.set_status("Suggestions hidden (S to reopen)", StatusLevel::Info);
            return;
        }

        // Handle arrow keys for profile navigation
        if matches!(key.code, KeyCode::Up | KeyCode::Down | KeyCode::Char('k') | KeyCode::Char('j')) {
            let anchor_count = self.suggestion_mode.as_ref()
                .map(|m| m.preview.len())
                .unwrap_or(0);
            if anchor_count > 0 {
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if self.profile_depth_idx > 0 {
                            self.profile_depth_idx -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if self.profile_depth_idx < anchor_count - 1 {
                            self.profile_depth_idx += 1;
                        }
                    }
                    _ => {}
                }
            }
            return;
        }

        // Handle Enter - accept suggestions and hide panel
        if key.code == KeyCode::Enter {
            // Clone preview to avoid borrow issues
            let preview: Vec<_> = self.suggestion_mode.as_ref()
                .map(|m| m.preview.clone())
                .unwrap_or_default();

            if !preview.is_empty() {
                self.path.clear();

                // First pass: add all depths and calculate dz values
                // We need to do this in two passes because adding depths/dz columns
                // changes indices, so we need to add all values first, then find indices
                let mut anchor_data: Vec<(f64, f64, usize)> = Vec::new(); // (depth, implied_dz, nlevels)

                for anchor in &preview {
                    // Calculate the implied dz from the suggestion
                    // The formula: depth = (nlevels - 1) * dz, so dz = depth / (nlevels - 1)
                    let implied_dz = if anchor.nlevels > 1 {
                        anchor.depth / (anchor.nlevels - 1) as f64
                    } else {
                        anchor.depth
                    };

                    // Add exact depth to table (will be ignored if duplicate)
                    self.table.add_depth(anchor.depth);

                    // Add the implied dz column (will be ignored if duplicate)
                    // Round to 2 decimal places for cleaner display
                    let rounded_dz = (implied_dz * 100.0).round() / 100.0;
                    self.table.add_min_dz(rounded_dz);

                    anchor_data.push((anchor.depth, rounded_dz, anchor.nlevels));
                }

                // Second pass: find exact indices and add anchors to path
                for (depth, dz, nlevels) in anchor_data {
                    // Find exact depth index (using tolerance for floating point)
                    let depth_idx = self
                        .table
                        .depths
                        .iter()
                        .position(|&d| (d - depth).abs() < 0.001)
                        .unwrap_or(0);

                    // Find exact dz index (using tolerance for floating point)
                    let dz_idx = self
                        .table
                        .min_dzs
                        .iter()
                        .position(|&d| (d - dz).abs() < 0.001)
                        .unwrap_or(0);

                    // Add anchor to path with exact values
                    self.path.add_anchor(depth_idx, dz_idx, depth, nlevels);
                }

                let count = preview.len();
                self.suggestion_visible = false; // Hide suggestions after accepting
                self.profile_depth_idx = 0; // Reset profile selection
                self.set_status(
                    format!("Applied {} anchors", count),
                    StatusLevel::Success,
                );
            }
            return;
        }

        // Track if we need to recompute
        let mut needs_recompute = false;

        // Handle parameter adjustments
        let status_msg: Option<(String, StatusLevel)> = match key.code {
            // Algorithm selection (1-3)
            KeyCode::Char(c @ '1'..='3') => {
                let n = c.to_digit(10).unwrap_or(1) as usize;
                if let Some(ref mut mode) = self.suggestion_mode {
                    if mode.select_algorithm(n) {
                        needs_recompute = true;
                        Some((format!("Algorithm: {}", mode.algorithm.name()), StatusLevel::Info))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }

            // Adjust target levels
            KeyCode::Char('+') | KeyCode::Char('=') => {
                if let Some(ref mut mode) = self.suggestion_mode {
                    mode.adjust_target_levels(1);
                    needs_recompute = true;
                    Some((format!("Target levels: {}", mode.params.target_levels), StatusLevel::Info))
                } else {
                    None
                }
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                if let Some(ref mut mode) = self.suggestion_mode {
                    mode.adjust_target_levels(-1);
                    needs_recompute = true;
                    Some((format!("Target levels: {}", mode.params.target_levels), StatusLevel::Info))
                } else {
                    None
                }
            }

            // Adjust dz_surf (target surface layer thickness)
            KeyCode::Char(']') => {
                if let Some(ref mut mode) = self.suggestion_mode {
                    let floor = self.mesh_info.as_ref().map(|m| m.min_wet_depth).unwrap_or(0.01);
                    mode.adjust_dz_surf(0.1, floor);
                    needs_recompute = true;
                    Some((format!("Δz₀: {:.2}m", mode.params.dz_surf), StatusLevel::Info))
                } else {
                    None
                }
            }
            KeyCode::Char('[') => {
                if let Some(ref mut mode) = self.suggestion_mode {
                    let floor = self.mesh_info.as_ref().map(|m| m.min_wet_depth).unwrap_or(0.01);
                    mode.adjust_dz_surf(-0.1, floor);
                    needs_recompute = true;
                    Some((format!("Δz₀: {:.2}m", mode.params.dz_surf), StatusLevel::Info))
                } else {
                    None
                }
            }

            // Adjust number of anchors
            KeyCode::Char('>') | KeyCode::Char('.') => {
                if let Some(ref mut mode) = self.suggestion_mode {
                    mode.adjust_num_anchors(1);
                    needs_recompute = true;
                    Some((format!("Anchors: {}", mode.params.num_anchors), StatusLevel::Info))
                } else {
                    None
                }
            }
            KeyCode::Char('<') | KeyCode::Char(',') => {
                if let Some(ref mut mode) = self.suggestion_mode {
                    mode.adjust_num_anchors(-1);
                    needs_recompute = true;
                    Some((format!("Anchors: {}", mode.params.num_anchors), StatusLevel::Info))
                } else {
                    None
                }
            }

            // Adjust shallow levels
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(ref mut mode) = self.suggestion_mode {
                    mode.adjust_shallow_levels(1);
                    needs_recompute = true;
                    Some((format!("Shallow levels: {}", mode.params.shallow_levels), StatusLevel::Info))
                } else {
                    None
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(ref mut mode) = self.suggestion_mode {
                    mode.adjust_shallow_levels(-1);
                    needs_recompute = true;
                    Some((format!("Shallow levels: {}", mode.params.shallow_levels), StatusLevel::Info))
                } else {
                    None
                }
            }

            // Cycle stretching transform type through all options
            KeyCode::Char('t') => {
                self.export_options.stretching = match self.export_options.stretching {
                    StretchingType::Quadratic => StretchingType::S,
                    StretchingType::S => StretchingType::Shchepetkin2005,
                    StretchingType::Shchepetkin2005 => StretchingType::Shchepetkin2010,
                    StretchingType::Shchepetkin2010 => StretchingType::Geyer,
                    StretchingType::Geyer => StretchingType::Quadratic,
                };
                let name = match self.export_options.stretching {
                    StretchingType::Quadratic => "Quadratic [a/A]",
                    StretchingType::S => "S-transform [f/F b/B]",
                    StretchingType::Shchepetkin2005 => "Shchepetkin2005 [s/S b/B h/H]",
                    StretchingType::Shchepetkin2010 => "Shchepetkin2010 [s/S b/B h/H]",
                    StretchingType::Geyer => "Geyer [s/S b/B h/H]",
                };
                Some((format!("Transform: {}", name), StatusLevel::Info))
            }

            // Adjust theta_f (S-transform only): F/f = increase/decrease
            KeyCode::Char('F') => {
                if matches!(self.export_options.stretching, StretchingType::S) {
                    self.export_options.theta_f = (self.export_options.theta_f + 0.5).min(20.0);
                    Some((format!("θf: {:.1}", self.export_options.theta_f), StatusLevel::Info))
                } else {
                    None
                }
            }
            KeyCode::Char('f') => {
                if matches!(self.export_options.stretching, StretchingType::S) {
                    self.export_options.theta_f = (self.export_options.theta_f - 0.5).max(0.1);
                    Some((format!("θf: {:.1}", self.export_options.theta_f), StatusLevel::Info))
                } else {
                    None
                }
            }

            // Adjust theta_b (S-transform and ROMS): B/b = increase/decrease
            // For S-transform: range [0, 1], for ROMS: range [0, 4]
            KeyCode::Char('B') => {
                match self.export_options.stretching {
                    StretchingType::S => {
                        self.export_options.theta_b = (self.export_options.theta_b + 0.1).min(1.0);
                        Some((format!("θb: {:.1}", self.export_options.theta_b), StatusLevel::Info))
                    }
                    StretchingType::Shchepetkin2005 | StretchingType::Shchepetkin2010 | StretchingType::Geyer => {
                        self.export_options.theta_b = (self.export_options.theta_b + 0.1).min(4.0);
                        Some((format!("θb: {:.1}", self.export_options.theta_b), StatusLevel::Info))
                    }
                    _ => None
                }
            }
            KeyCode::Char('b') => {
                match self.export_options.stretching {
                    StretchingType::S | StretchingType::Shchepetkin2005 | StretchingType::Shchepetkin2010 | StretchingType::Geyer => {
                        self.export_options.theta_b = (self.export_options.theta_b - 0.1).max(0.0);
                        Some((format!("θb: {:.1}", self.export_options.theta_b), StatusLevel::Info))
                    }
                    _ => None
                }
            }

            // Adjust theta_s (ROMS transforms only): S/s = increase/decrease
            KeyCode::Char('S') => {
                match self.export_options.stretching {
                    StretchingType::Shchepetkin2005 | StretchingType::Shchepetkin2010 | StretchingType::Geyer => {
                        self.export_options.theta_s = (self.export_options.theta_s + 0.5).min(10.0);
                        Some((format!("θs: {:.1}", self.export_options.theta_s), StatusLevel::Info))
                    }
                    _ => None
                }
            }
            KeyCode::Char('s') => {
                match self.export_options.stretching {
                    StretchingType::Shchepetkin2005 | StretchingType::Shchepetkin2010 | StretchingType::Geyer => {
                        self.export_options.theta_s = (self.export_options.theta_s - 0.5).max(0.0);
                        Some((format!("θs: {:.1}", self.export_options.theta_s), StatusLevel::Info))
                    }
                    _ => None
                }
            }

            // Adjust hc (ROMS transforms only): H/h = increase/decrease
            KeyCode::Char('H') => {
                match self.export_options.stretching {
                    StretchingType::Shchepetkin2005 | StretchingType::Shchepetkin2010 | StretchingType::Geyer => {
                        self.export_options.hc = (self.export_options.hc + 1.0).min(100.0);
                        Some((format!("hc: {:.0}m", self.export_options.hc), StatusLevel::Info))
                    }
                    _ => None
                }
            }
            KeyCode::Char('h') => {
                match self.export_options.stretching {
                    StretchingType::Shchepetkin2005 | StretchingType::Shchepetkin2010 | StretchingType::Geyer => {
                        self.export_options.hc = (self.export_options.hc - 1.0).max(1.0);
                        Some((format!("hc: {:.0}m", self.export_options.hc), StatusLevel::Info))
                    }
                    _ => None
                }
            }

            // Adjust a_vqs0 (Quadratic): A/a = increase/decrease
            KeyCode::Char('A') => {
                if matches!(self.export_options.stretching, StretchingType::Quadratic) {
                    self.export_options.a_vqs0 = (self.export_options.a_vqs0 + 0.1).min(1.0);
                    Some((format!("a_vqs: {:.1}", self.export_options.a_vqs0), StatusLevel::Info))
                } else {
                    None
                }
            }
            KeyCode::Char('a') => {
                if matches!(self.export_options.stretching, StretchingType::Quadratic) {
                    self.export_options.a_vqs0 = (self.export_options.a_vqs0 - 0.1).max(-1.0);
                    Some((format!("a_vqs: {:.1}", self.export_options.a_vqs0), StatusLevel::Info))
                } else {
                    None
                }
            }

            // Adjust dz_bottom_min: Z/z = increase/decrease (no upper cap)
            KeyCode::Char('Z') => {
                self.export_options.dz_bottom_min += 0.1;
                Some((format!("Δz_bot: {:.1}m", self.export_options.dz_bottom_min), StatusLevel::Info))
            }
            KeyCode::Char('z') => {
                self.export_options.dz_bottom_min = (self.export_options.dz_bottom_min - 0.1).max(0.1);
                Some((format!("Δz_bot: {:.1}m", self.export_options.dz_bottom_min), StatusLevel::Info))
            }

            _ => None,
        };

        // Set status message
        if let Some((msg, level)) = status_msg {
            self.set_status(msg, level);
        }

        // Trigger recomputation if needed
        if needs_recompute {
            self.compute_suggestions();
        }
    }

    /// Get stretching parameters from export options
    pub fn get_stretching_params(&self) -> StretchingParams {
        StretchingParams {
            theta_f: self.export_options.theta_f,
            theta_b: self.export_options.theta_b,
            a_vqs0: self.export_options.a_vqs0,
            etal: 0.0,
            theta_s: self.export_options.theta_s,
            hc: self.export_options.hc,
        }
    }

    /// Convert StretchingType to StretchingKind for stretching calculations
    pub fn get_stretching_kind(&self) -> super::stretching::StretchingKind {
        use super::stretching::StretchingKind;
        match self.export_options.stretching {
            StretchingType::Quadratic => StretchingKind::Quadratic,
            StretchingType::S => StretchingKind::S,
            StretchingType::Shchepetkin2005 => StretchingKind::Shchepetkin2005,
            StretchingType::Shchepetkin2010 => StretchingKind::Shchepetkin2010,
            StretchingType::Geyer => StretchingKind::Geyer,
        }
    }

    /// Get zone stats with caching - avoids expensive recomputation on every frame
    /// Uses interior mutability (RefCell) so it can be called during rendering with &self
    pub fn get_cached_zone_stats(
        &self,
        anchor_depths: &[f64],
        anchor_nlevels: &[usize],
    ) -> Vec<super::stretching::ZoneStats> {
        use super::stretching::{compute_mesh_zone_stats, compute_path_zone_stats};

        // Check if cache is valid
        let cache_valid = {
            let cache_ref = self.cached_zone_stats.borrow();
            if let Some(ref cache) = *cache_ref {
                cache.anchor_depths == anchor_depths
                    && cache.anchor_nlevels == anchor_nlevels
                    && cache.stretching == self.export_options.stretching
                    && (cache.theta_f - self.export_options.theta_f).abs() < 0.001
                    && (cache.theta_b - self.export_options.theta_b).abs() < 0.001
                    && (cache.theta_s - self.export_options.theta_s).abs() < 0.001
                    && (cache.hc - self.export_options.hc).abs() < 0.001
                    && (cache.a_vqs0 - self.export_options.a_vqs0).abs() < 0.001
            } else {
                false
            }
        };

        if cache_valid {
            let cache_ref = self.cached_zone_stats.borrow();
            return cache_ref.as_ref().unwrap().stats.clone();
        }

        // Compute new stats
        let params = self.get_stretching_params();
        let stretching = self.get_stretching_kind();

        let stats = if let Some(mesh) = &self.mesh_info {
            let mesh_depths: Vec<f64> = mesh.hgrid.depths_positive_down().to_vec();
            compute_mesh_zone_stats(anchor_depths, anchor_nlevels, &mesh_depths, &params, stretching)
        } else {
            compute_path_zone_stats(anchor_depths, anchor_nlevels, &params, stretching)
        };

        // Cache the result
        *self.cached_zone_stats.borrow_mut() = Some(CachedZoneStats {
            stats: stats.clone(),
            anchor_depths: anchor_depths.to_vec(),
            anchor_nlevels: anchor_nlevels.to_vec(),
            stretching: self.export_options.stretching.clone(),
            theta_f: self.export_options.theta_f,
            theta_b: self.export_options.theta_b,
            theta_s: self.export_options.theta_s,
            hc: self.export_options.hc,
            a_vqs0: self.export_options.a_vqs0,
        });

        stats
    }

    /// Get cached profile data for single-depth profile view
    /// Uses interior mutability (RefCell) so it can be called during rendering with &self
    /// Returns (z_coords, thicknesses, was_truncated, actual_levels)
    pub fn get_cached_profile_data(
        &self,
        depth: f64,
        nlevels: usize,
        first_depth: f64,
    ) -> (Vec<f64>, Vec<f64>, bool, usize) {
        use super::stretching::{compute_z_with_truncation, compute_layer_thicknesses};

        // Check if cache is valid
        let cache_valid = {
            let cache_ref = self.cached_profile_data.borrow();
            if let Some(ref cache) = *cache_ref {
                (cache.depth - depth).abs() < 0.001
                    && cache.nlevels == nlevels
                    && (cache.first_depth - first_depth).abs() < 0.001
                    && cache.stretching == self.export_options.stretching
                    && (cache.theta_f - self.export_options.theta_f).abs() < 0.001
                    && (cache.theta_b - self.export_options.theta_b).abs() < 0.001
                    && (cache.theta_s - self.export_options.theta_s).abs() < 0.001
                    && (cache.hc - self.export_options.hc).abs() < 0.001
                    && (cache.a_vqs0 - self.export_options.a_vqs0).abs() < 0.001
                    && (cache.dz_bottom_min - self.export_options.dz_bottom_min).abs() < 0.001
            } else {
                false
            }
        };

        if cache_valid {
            let cache_ref = self.cached_profile_data.borrow();
            let cache = cache_ref.as_ref().unwrap();
            return (cache.z_coords.clone(), cache.thicknesses.clone(), cache.was_truncated, cache.actual_levels);
        }

        // Compute new profile data
        let stretch_params = self.get_stretching_params();
        let stretching_kind = self.get_stretching_kind();
        let dz_bottom_min = self.export_options.dz_bottom_min;

        let truncation = compute_z_with_truncation(
            depth, nlevels, &stretch_params, first_depth, dz_bottom_min, stretching_kind,
        );
        let thicknesses = compute_layer_thicknesses(&truncation.z_coords);

        // Cache the result
        *self.cached_profile_data.borrow_mut() = Some(CachedProfileData {
            z_coords: truncation.z_coords.clone(),
            thicknesses: thicknesses.clone(),
            was_truncated: truncation.was_truncated,
            actual_levels: truncation.actual_levels,
            depth,
            nlevels,
            stretching: self.export_options.stretching,
            theta_f: self.export_options.theta_f,
            theta_b: self.export_options.theta_b,
            theta_s: self.export_options.theta_s,
            hc: self.export_options.hc,
            a_vqs0: self.export_options.a_vqs0,
            dz_bottom_min: self.export_options.dz_bottom_min,
            first_depth,
        });

        (truncation.z_coords, thicknesses, truncation.was_truncated, truncation.actual_levels)
    }

    /// Get cached truncation data for all anchors
    /// Uses interior mutability (RefCell) so it can be called during rendering with &self
    pub fn get_cached_truncation_data(
        &self,
        anchor_depths: &[f64],
        anchor_nlevels: &[usize],
    ) -> Vec<AnchorTruncation> {
        use super::stretching::compute_z_with_truncation;

        if anchor_depths.is_empty() || anchor_nlevels.is_empty() {
            return vec![];
        }

        // Check if cache is valid
        let cache_valid = {
            let cache_ref = self.cached_truncation_data.borrow();
            if let Some(ref cache) = *cache_ref {
                cache.anchor_depths == anchor_depths
                    && cache.anchor_nlevels == anchor_nlevels
                    && cache.stretching == self.export_options.stretching
                    && (cache.theta_f - self.export_options.theta_f).abs() < 0.001
                    && (cache.theta_b - self.export_options.theta_b).abs() < 0.001
                    && (cache.theta_s - self.export_options.theta_s).abs() < 0.001
                    && (cache.hc - self.export_options.hc).abs() < 0.001
                    && (cache.a_vqs0 - self.export_options.a_vqs0).abs() < 0.001
                    && (cache.dz_bottom_min - self.export_options.dz_bottom_min).abs() < 0.001
            } else {
                false
            }
        };

        if cache_valid {
            let cache_ref = self.cached_truncation_data.borrow();
            return cache_ref.as_ref().unwrap().truncations.clone();
        }

        // Compute truncation for all anchors
        let stretch_params = self.get_stretching_params();
        let stretching_kind = self.get_stretching_kind();
        let dz_bottom_min = self.export_options.dz_bottom_min;
        let first_depth = anchor_depths.first().copied().unwrap_or(1.0);

        let truncations: Vec<AnchorTruncation> = anchor_depths
            .iter()
            .zip(anchor_nlevels.iter())
            .map(|(&depth, &nlevels)| {
                let result = compute_z_with_truncation(
                    depth, nlevels, &stretch_params, first_depth, dz_bottom_min, stretching_kind,
                );
                AnchorTruncation {
                    requested_levels: nlevels,
                    actual_levels: result.actual_levels,
                    was_truncated: result.was_truncated,
                }
            })
            .collect();

        // Cache the result
        *self.cached_truncation_data.borrow_mut() = Some(CachedTruncationData {
            truncations: truncations.clone(),
            anchor_depths: anchor_depths.to_vec(),
            anchor_nlevels: anchor_nlevels.to_vec(),
            stretching: self.export_options.stretching,
            theta_f: self.export_options.theta_f,
            theta_b: self.export_options.theta_b,
            theta_s: self.export_options.theta_s,
            hc: self.export_options.hc,
            a_vqs0: self.export_options.a_vqs0,
            dz_bottom_min: self.export_options.dz_bottom_min,
        });

        truncations
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
                let mesh = match &self.mesh_info {
                    Some(m) => m,
                    None => {
                        self.set_status(
                            "Cannot generate vgrid.in: no hgrid loaded (use -g flag)",
                            StatusLevel::Error,
                        );
                        return;
                    }
                };

                // Extract HSM config from path
                let (depths, nlevels) = self.path.to_hsm_config();

                if depths.is_empty() {
                    self.set_status("Cannot export: no anchors selected", StatusLevel::Error);
                    return;
                }

                // Validate: deepest anchor must cover mesh max depth
                if let Some(&deepest) = depths.last() {
                    if deepest < mesh.max_depth {
                        self.set_status(
                            format!(
                                "Error: deepest anchor ({:.1}m) < mesh max ({:.1}m)",
                                deepest, mesh.max_depth
                            ),
                            StatusLevel::Error,
                        );
                        return;
                    }
                }

                // Build VQS using the configured stretching function
                let a_vqs0 = self.export_options.a_vqs0;
                let etal = 0.0;
                let theta_f = self.export_options.theta_f;
                let theta_b = self.export_options.theta_b;
                let theta_s = self.export_options.theta_s;
                let hc = self.export_options.hc;
                let skew_decay_rate = 0.03;
                let dz_bottom_min = self.export_options.dz_bottom_min;

                let result = match self.export_options.stretching {
                    StretchingType::S => {
                        let opts = STransformOpts {
                            a_vqs0: &a_vqs0,
                            etal: &etal,
                            theta_b: &theta_b,
                            theta_f: &theta_f,
                        };
                        let transform = StretchingFunction::S(opts);
                        VQSBuilder::default()
                            .hgrid(&mesh.hgrid)
                            .depths(&depths)
                            .nlevels(&nlevels)
                            .stretching(&transform)
                            .dz_bottom_min(&dz_bottom_min)
                            .build()
                    }
                    StretchingType::Quadratic => {
                        let opts = QuadraticTransformOpts {
                            a_vqs0: &a_vqs0,
                            etal: &etal,
                            skew_decay_rate: &skew_decay_rate,
                        };
                        let transform = StretchingFunction::Quadratic(opts);
                        VQSBuilder::default()
                            .hgrid(&mesh.hgrid)
                            .depths(&depths)
                            .nlevels(&nlevels)
                            .stretching(&transform)
                            .dz_bottom_min(&dz_bottom_min)
                            .build()
                    }
                    StretchingType::Shchepetkin2005 => {
                        let opts = Shchepetkin2005Opts::new(
                            &etal,
                            &a_vqs0,
                            &theta_s,
                            &theta_b,
                            &hc,
                        );
                        let transform = StretchingFunction::Shchepetkin2005(opts);
                        VQSBuilder::default()
                            .hgrid(&mesh.hgrid)
                            .depths(&depths)
                            .nlevels(&nlevels)
                            .stretching(&transform)
                            .dz_bottom_min(&dz_bottom_min)
                            .build()
                    }
                    StretchingType::Shchepetkin2010 => {
                        let opts = Shchepetkin2010Opts::new(
                            &etal,
                            &a_vqs0,
                            &theta_s,
                            &theta_b,
                            &hc,
                        );
                        let transform = StretchingFunction::Shchepetkin2010(opts);
                        VQSBuilder::default()
                            .hgrid(&mesh.hgrid)
                            .depths(&depths)
                            .nlevels(&nlevels)
                            .stretching(&transform)
                            .dz_bottom_min(&dz_bottom_min)
                            .build()
                    }
                    StretchingType::Geyer => {
                        let opts = GeyerOpts::new(
                            &etal,
                            &a_vqs0,
                            &theta_s,
                            &theta_b,
                            &hc,
                        );
                        let transform = StretchingFunction::Geyer(opts);
                        VQSBuilder::default()
                            .hgrid(&mesh.hgrid)
                            .depths(&depths)
                            .nlevels(&nlevels)
                            .stretching(&transform)
                            .dz_bottom_min(&dz_bottom_min)
                            .build()
                    }
                };

                match result {
                    Ok(vqs) => {
                        let output_path = self.output_dir.join("vgrid.in");
                        match vqs.write_to_file(&output_path) {
                            Ok(_) => self.set_status(
                                format!("Wrote {}", output_path.display()),
                                StatusLevel::Success,
                            ),
                            Err(e) => self.set_status(
                                format!("Write error: {}", e),
                                StatusLevel::Error,
                            ),
                        }
                    }
                    Err(e) => self.set_status(format!("VQS build error: {}", e), StatusLevel::Error),
                }
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
            StretchingType::Shchepetkin2005 => "shchepetkin2005",
            StretchingType::Shchepetkin2010 => "shchepetkin2010",
            StretchingType::Geyer => "geyer",
        };

        // Build base command
        let mut cmd = format!(
            "--transform {} --depths \"{}\" --nlevels \"{}\"",
            transform, depths_str, nlevels_str
        );

        // Add ROMS-specific parameters if needed
        match self.export_options.stretching {
            StretchingType::Shchepetkin2005 | StretchingType::Shchepetkin2010 | StretchingType::Geyer => {
                cmd.push_str(&format!(
                    " --theta-s {:.1} --theta-b {:.1} --hc {:.1}",
                    self.export_options.theta_s,
                    self.export_options.theta_b,
                    self.export_options.hc
                ));
            }
            _ => {}
        }

        cmd
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
            StretchingType::Shchepetkin2005 => "shchepetkin2005",
            StretchingType::Shchepetkin2010 => "shchepetkin2010",
            StretchingType::Geyer => "geyer",
        };

        // Base YAML with common parameters
        let mut yaml = format!(
            r#"vgrid:
  type: vqs
  method: hsm
  depths: [{}]
  nlevels: [{}]
  stretching:
    function: {}
    a_vqs0: {}"#,
            depths_yaml, nlevels_yaml, stretching, self.export_options.a_vqs0
        );

        // Add ROMS-specific parameters if needed
        match self.export_options.stretching {
            StretchingType::Shchepetkin2005 | StretchingType::Shchepetkin2010 | StretchingType::Geyer => {
                yaml.push_str(&format!(
                    r#"
    theta_s: {}
    theta_b: {}
    hc: {}"#,
                    self.export_options.theta_s,
                    self.export_options.theta_b,
                    self.export_options.hc
                ));
            }
            _ => {}
        }

        yaml
    }
}
