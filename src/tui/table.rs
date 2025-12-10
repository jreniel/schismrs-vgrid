//! Construction table for LSC2 grid design
//!
//! Implements the systematic framework where:
//! - Rows = depth values
//! - Columns = minimum delta-z values
//! - Cells = N = ceil(depth / dz) + 1

/// Represents the construction table for LSC2 grid design
#[derive(Debug, Clone)]
pub struct ConstructionTable {
    /// Depth values (rows) - configurable, sorted ascending
    pub depths: Vec<f64>,

    /// Minimum delta-z values (columns) - configurable, sorted ascending for display
    /// (smallest dz = finest resolution on the left)
    pub min_dzs: Vec<f64>,

    /// Cached N values for each (depth_idx, dz_idx)
    /// N = ceil(depth / dz) + 1
    pub cell_values: Vec<Vec<CellValue>>,

    /// Current cursor position (row_idx, col_idx)
    pub cursor: (usize, usize),

    /// Editing mode for row/column modification
    pub edit_mode: EditMode,

    /// Temporary input buffer for numeric entry
    pub input_buffer: String,

    /// Scroll offset for large tables
    pub scroll_offset: usize,
}

/// Value of a single cell in the construction table
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellValue {
    /// Computed N = ceil(depth/dz) + 1
    pub n: usize,
    /// Validity/color category
    pub validity: CellValidity,
}

/// Cell validity determines color coding
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellValidity {
    /// depth < dz * 0.5 (gray, cannot select)
    Invalid,
    /// N <= 10 (green)
    Efficient,
    /// N 11-30 (blue)
    Good,
    /// N 31-60 (orange)
    Moderate,
    /// N 61-120 (red text)
    Intensive,
    /// N > 120 (red background, cannot select)
    Excessive,
}

/// Edit modes for the construction table
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EditMode {
    /// Normal cursor movement
    #[default]
    Navigate,
    /// Adding a new depth row
    AddRow,
    /// Adding a new dz column
    AddColumn,
    /// Confirming deletion
    DeleteConfirm,
}

/// Maximum practical number of vertical levels
pub const MAX_PRACTICAL_LEVELS: usize = 120;

impl Default for ConstructionTable {
    fn default() -> Self {
        Self::new()
    }
}

impl ConstructionTable {
    /// Create a new construction table with default depths and dz values
    pub fn new() -> Self {
        // Default depths from the thesis table
        let depths = vec![
            0.5, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 7500.0,
            11000.0,
        ];

        // Default min_dzs from the thesis table (ascending = finest to coarsest left to right)
        let min_dzs = vec![0.5, 1.0, 3.0, 10.0, 50.0, 100.0];

        let mut table = Self {
            depths,
            min_dzs,
            cell_values: Vec::new(),
            cursor: (0, 0),
            edit_mode: EditMode::Navigate,
            input_buffer: String::new(),
            scroll_offset: 0,
        };
        table.recompute_cells();
        table
    }

    /// Create a table with custom initial depths and dz values
    pub fn with_values(mut depths: Vec<f64>, mut min_dzs: Vec<f64>) -> Self {
        // Sort depths ascending
        depths.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        // Sort min_dzs ascending (finest resolution first)
        min_dzs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let mut table = Self {
            depths,
            min_dzs,
            cell_values: Vec::new(),
            cursor: (0, 0),
            edit_mode: EditMode::Navigate,
            input_buffer: String::new(),
            scroll_offset: 0,
        };
        table.recompute_cells();
        table
    }

    /// Recompute all cell values after depth/dz changes
    pub fn recompute_cells(&mut self) {
        self.cell_values = self
            .depths
            .iter()
            .map(|&depth| {
                self.min_dzs
                    .iter()
                    .map(|&dz| Self::compute_cell(depth, dz))
                    .collect()
            })
            .collect();
    }

    /// Compute the value for a single cell
    fn compute_cell(depth: f64, dz: f64) -> CellValue {
        // Invalid if depth is too shallow for this dz
        if depth < dz * 0.5 {
            return CellValue {
                n: 0,
                validity: CellValidity::Invalid,
            };
        }

        let n = (depth / dz).ceil() as usize + 1;

        let validity = if n > MAX_PRACTICAL_LEVELS {
            CellValidity::Excessive
        } else if n <= 10 {
            CellValidity::Efficient
        } else if n <= 30 {
            CellValidity::Good
        } else if n <= 60 {
            CellValidity::Moderate
        } else {
            CellValidity::Intensive
        };

        CellValue { n, validity }
    }

    /// Check if a cell can be selected for a path
    pub fn is_selectable(&self, row: usize, col: usize) -> bool {
        if row >= self.depths.len() || col >= self.min_dzs.len() {
            return false;
        }
        let cell = &self.cell_values[row][col];
        cell.validity != CellValidity::Invalid && cell.validity != CellValidity::Excessive
    }

    /// Add a new depth row, maintaining sorted order
    pub fn add_depth(&mut self, depth: f64) -> bool {
        if depth <= 0.0 {
            return false;
        }
        // Check for duplicates (within tolerance)
        if self.depths.iter().any(|&d| (d - depth).abs() < 0.001) {
            return false;
        }

        let pos = self
            .depths
            .iter()
            .position(|&d| d > depth)
            .unwrap_or(self.depths.len());
        self.depths.insert(pos, depth);
        self.recompute_cells();
        true
    }

    /// Add a new min_dz column, maintaining sorted order
    pub fn add_min_dz(&mut self, dz: f64) -> bool {
        if dz <= 0.0 {
            return false;
        }
        // Check for duplicates (within tolerance)
        if self.min_dzs.iter().any(|&d| (d - dz).abs() < 0.001) {
            return false;
        }

        let pos = self
            .min_dzs
            .iter()
            .position(|&d| d > dz)
            .unwrap_or(self.min_dzs.len());
        self.min_dzs.insert(pos, dz);
        self.recompute_cells();
        true
    }

    /// Remove a depth row by index
    pub fn remove_depth(&mut self, idx: usize) -> bool {
        if self.depths.len() <= 2 || idx >= self.depths.len() {
            return false;
        }
        self.depths.remove(idx);
        self.recompute_cells();

        // Adjust cursor if needed
        if self.cursor.0 >= self.depths.len() {
            self.cursor.0 = self.depths.len().saturating_sub(1);
        }
        true
    }

    /// Remove a min_dz column by index
    pub fn remove_min_dz(&mut self, idx: usize) -> bool {
        if self.min_dzs.len() <= 1 || idx >= self.min_dzs.len() {
            return false;
        }
        self.min_dzs.remove(idx);
        self.recompute_cells();

        // Adjust cursor if needed
        if self.cursor.1 >= self.min_dzs.len() {
            self.cursor.1 = self.min_dzs.len().saturating_sub(1);
        }
        true
    }

    /// Get the cell value at the cursor position
    pub fn current_cell(&self) -> Option<&CellValue> {
        self.cell_values
            .get(self.cursor.0)
            .and_then(|row| row.get(self.cursor.1))
    }

    /// Get the depth at the cursor position
    pub fn current_depth(&self) -> Option<f64> {
        self.depths.get(self.cursor.0).copied()
    }

    /// Get the min_dz at the cursor position
    pub fn current_min_dz(&self) -> Option<f64> {
        self.min_dzs.get(self.cursor.1).copied()
    }

    /// Move cursor up
    pub fn cursor_up(&mut self) {
        if self.cursor.0 > 0 {
            self.cursor.0 -= 1;
        }
    }

    /// Move cursor down
    pub fn cursor_down(&mut self) {
        if self.cursor.0 < self.depths.len().saturating_sub(1) {
            self.cursor.0 += 1;
        }
    }

    /// Move cursor left
    pub fn cursor_left(&mut self) {
        if self.cursor.1 > 0 {
            self.cursor.1 -= 1;
        }
    }

    /// Move cursor right
    pub fn cursor_right(&mut self) {
        if self.cursor.1 < self.min_dzs.len().saturating_sub(1) {
            self.cursor.1 += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_computation() {
        // From thesis: 10m depth with 1.0m dz should give N = ceil(10/1) + 1 = 11
        let cell = ConstructionTable::compute_cell(10.0, 1.0);
        assert_eq!(cell.n, 11);
        assert_eq!(cell.validity, CellValidity::Good);

        // 5m depth with 0.5m dz should give N = ceil(5/0.5) + 1 = 11
        let cell = ConstructionTable::compute_cell(5.0, 0.5);
        assert_eq!(cell.n, 11);

        // Invalid: depth < dz * 0.5
        let cell = ConstructionTable::compute_cell(0.2, 1.0);
        assert_eq!(cell.validity, CellValidity::Invalid);
    }

    #[test]
    fn test_add_remove_depth() {
        let mut table = ConstructionTable::new();
        let initial_count = table.depths.len();

        // Add a depth
        assert!(table.add_depth(15.0));
        assert_eq!(table.depths.len(), initial_count + 1);
        assert!(table.depths.contains(&15.0));

        // Cannot add duplicate
        assert!(!table.add_depth(15.0));

        // Remove depth
        let idx = table.depths.iter().position(|&d| d == 15.0).unwrap();
        assert!(table.remove_depth(idx));
        assert_eq!(table.depths.len(), initial_count);
    }
}
