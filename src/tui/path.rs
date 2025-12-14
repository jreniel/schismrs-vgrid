//! Path selection for master grid construction
//!
//! Implements the path building logic with monotonicity validation:
//! - Users select anchor points going down through the table
//! - N values (levels) must be non-decreasing with depth
//! - Can skip rows, but must maintain monotonicity

use super::table::{CellValidity, ConstructionTable};

/// Represents the selected path through the construction table
#[derive(Debug, Clone, Default)]
pub struct PathSelection {
    /// Selected anchor points as (depth_idx, dz_idx, depth, nlevels) tuples
    /// Stored in order of increasing depth
    pub anchors: Vec<PathAnchor>,

    /// Validation errors for the current path
    pub validation_errors: Vec<PathError>,
}

/// A single anchor point in the path
#[derive(Clone, Debug)]
pub struct PathAnchor {
    /// Row index in table
    pub depth_idx: usize,
    /// Column index in table
    pub dz_idx: usize,
    /// Actual depth value
    pub depth: f64,
    /// Computed N value (number of levels)
    pub nlevels: usize,
}

/// Path validation errors
#[derive(Clone, Debug)]
pub enum PathError {
    /// N values must be non-decreasing with depth
    MonotonicityViolation {
        at_depth: f64,
        n_value: usize,
        previous_n: usize,
    },
    /// Minimum 2 anchor points required
    InsufficientAnchors,
    /// Selected cell is invalid (too shallow or excessive)
    InvalidCellSelected { depth: f64, dz: f64 },
}

impl PathSelection {
    /// Create a new empty path selection
    pub fn new() -> Self {
        Self {
            anchors: Vec::new(),
            validation_errors: Vec::new(),
        }
    }

    /// Attempt to toggle an anchor point at the given cell
    /// Returns true if the action was performed
    ///
    /// Behavior:
    /// - If clicking exact same cell (same row AND column): deselect
    /// - If clicking different column at same depth: switch to new column
    /// - If clicking new depth: add anchor at that depth
    pub fn toggle_anchor(&mut self, table: &ConstructionTable, row: usize, col: usize) -> bool {
        // Check bounds
        if row >= table.depths.len() || col >= table.min_dzs.len() {
            return false;
        }

        let cell = &table.cell_values[row][col];

        // Cannot select invalid or excessive cells
        if cell.validity == CellValidity::Invalid || cell.validity == CellValidity::Excessive {
            return false;
        }

        let depth = table.depths[row];
        let nlevels = cell.n;

        // Check if already selected at this depth
        if let Some(idx) = self.anchors.iter().position(|a| a.depth_idx == row) {
            // If exact same cell (same row AND column), deselect
            if self.anchors[idx].dz_idx == col {
                self.anchors.remove(idx);
                self.validate();
                return true;
            } else {
                // Different column at same depth - switch to new column
                self.anchors[idx].dz_idx = col;
                self.anchors[idx].nlevels = nlevels;
                self.validate();
                return true;
            }
        }

        // Add new anchor at new depth
        let anchor = PathAnchor {
            depth_idx: row,
            dz_idx: col,
            depth,
            nlevels,
        };

        // Insert in sorted order by depth
        let insert_pos = self
            .anchors
            .iter()
            .position(|a| a.depth > depth)
            .unwrap_or(self.anchors.len());
        self.anchors.insert(insert_pos, anchor);

        self.validate();
        true
    }

    /// Update an existing anchor to a different column (different dz)
    /// This is useful when the user wants to change the resolution at a depth
    pub fn update_anchor(&mut self, table: &ConstructionTable, row: usize, col: usize) -> bool {
        if let Some(idx) = self.anchors.iter().position(|a| a.depth_idx == row) {
            if col >= table.min_dzs.len() {
                return false;
            }

            let cell = &table.cell_values[row][col];
            if cell.validity == CellValidity::Invalid || cell.validity == CellValidity::Excessive {
                return false;
            }

            self.anchors[idx].dz_idx = col;
            self.anchors[idx].nlevels = cell.n;
            self.validate();
            true
        } else {
            false
        }
    }

    /// Validate the current path
    pub fn validate(&mut self) {
        self.validation_errors.clear();

        if self.anchors.len() < 2 {
            self.validation_errors.push(PathError::InsufficientAnchors);
            return;
        }

        // Check monotonicity: nlevels must be non-decreasing with depth
        for i in 1..self.anchors.len() {
            if self.anchors[i].nlevels < self.anchors[i - 1].nlevels {
                self.validation_errors.push(PathError::MonotonicityViolation {
                    at_depth: self.anchors[i].depth,
                    n_value: self.anchors[i].nlevels,
                    previous_n: self.anchors[i - 1].nlevels,
                });
            }
        }
    }

    /// Check if the path is valid for export
    pub fn is_valid(&self) -> bool {
        self.validation_errors.is_empty() && self.anchors.len() >= 2
    }

    /// Check if a depth row is part of the selected path
    pub fn is_depth_selected(&self, row: usize) -> bool {
        self.anchors.iter().any(|a| a.depth_idx == row)
    }

    /// Check if a specific cell is selected
    pub fn is_cell_selected(&self, row: usize, col: usize) -> bool {
        self.anchors
            .iter()
            .any(|a| a.depth_idx == row && a.dz_idx == col)
    }

    /// Get the anchor at a specific depth row, if any
    pub fn get_anchor_at_depth(&self, row: usize) -> Option<&PathAnchor> {
        self.anchors.iter().find(|a| a.depth_idx == row)
    }

    /// Get depths and nlevels vectors for VQSBuilder
    pub fn to_hsm_config(&self) -> (Vec<f64>, Vec<usize>) {
        let depths: Vec<f64> = self.anchors.iter().map(|a| a.depth).collect();
        let nlevels: Vec<usize> = self.anchors.iter().map(|a| a.nlevels).collect();
        (depths, nlevels)
    }

    /// Clear all selections
    pub fn clear(&mut self) {
        self.anchors.clear();
        self.validation_errors.clear();
        self.validation_errors.push(PathError::InsufficientAnchors);
    }

    /// Add an anchor directly (for suggestion mode)
    /// Does not toggle - always adds if not already present at this depth
    pub fn add_anchor(&mut self, depth_idx: usize, dz_idx: usize, depth: f64, nlevels: usize) {
        // Check if already have an anchor at this depth - skip if so
        if self.anchors.iter().any(|a| a.depth_idx == depth_idx) {
            return;
        }

        let anchor = PathAnchor {
            depth_idx,
            dz_idx,
            depth,
            nlevels,
        };

        // Insert in sorted order by depth
        let insert_pos = self
            .anchors
            .iter()
            .position(|a| a.depth > depth)
            .unwrap_or(self.anchors.len());
        self.anchors.insert(insert_pos, anchor);

        self.validate();
    }

    /// Add an anchor with arbitrary depth/nlevels (not tied to table)
    /// Used for direct anchor editing in anchor view
    pub fn add_direct_anchor(&mut self, depth: f64, nlevels: usize) {
        // Check if already have an anchor at this depth (within tolerance)
        if self.anchors.iter().any(|a| (a.depth - depth).abs() < 0.001) {
            return;
        }

        let anchor = PathAnchor {
            depth_idx: usize::MAX, // Marker for "not from table"
            dz_idx: usize::MAX,
            depth,
            nlevels,
        };

        // Insert in sorted order by depth
        let insert_pos = self
            .anchors
            .iter()
            .position(|a| a.depth > depth)
            .unwrap_or(self.anchors.len());
        self.anchors.insert(insert_pos, anchor);

        self.validate();
    }

    /// Get the number of selected anchors
    pub fn len(&self) -> usize {
        self.anchors.len()
    }

    /// Check if path is empty
    pub fn is_empty(&self) -> bool {
        self.anchors.is_empty()
    }

    /// Remove an anchor by its index in the anchors list
    /// Returns the removed anchor if successful
    pub fn remove_anchor_by_index(&mut self, index: usize) -> Option<PathAnchor> {
        if index < self.anchors.len() {
            let removed = self.anchors.remove(index);
            self.validate();
            Some(removed)
        } else {
            None
        }
    }

    /// Check if an anchor at a given depth would violate monotonicity
    /// Returns Some(previous_n) if it would violate, None if OK
    pub fn would_violate_monotonicity(
        &self,
        table: &ConstructionTable,
        row: usize,
        col: usize,
    ) -> Option<usize> {
        if row >= table.depths.len() || col >= table.min_dzs.len() {
            return None;
        }

        let cell = &table.cell_values[row][col];
        let candidate_n = cell.n;
        let candidate_depth = table.depths[row];

        // Find the anchor immediately before this depth
        let prev_anchor = self.anchors.iter().rev().find(|a| a.depth < candidate_depth);

        // Find the anchor immediately after this depth
        let next_anchor = self.anchors.iter().find(|a| a.depth > candidate_depth);

        // Check against previous anchor
        if let Some(prev) = prev_anchor {
            if candidate_n < prev.nlevels {
                return Some(prev.nlevels);
            }
        }

        // Check against next anchor
        if let Some(next) = next_anchor {
            if candidate_n > next.nlevels {
                return Some(next.nlevels);
            }
        }

        None
    }
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathError::MonotonicityViolation {
                at_depth,
                n_value,
                previous_n,
            } => {
                write!(
                    f,
                    "N={} < {} at {}m (monotonicity)",
                    n_value, previous_n, at_depth
                )
            }
            PathError::InsufficientAnchors => {
                write!(f, "Need at least 2 anchor points")
            }
            PathError::InvalidCellSelected { depth, dz } => {
                write!(f, "Invalid cell at depth={}m, dz={}m", depth, dz)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_selection() {
        let table = ConstructionTable::new();
        let mut path = PathSelection::new();

        // Initially invalid (not enough anchors)
        assert!(!path.is_valid());

        // Add first anchor (shallow)
        assert!(path.toggle_anchor(&table, 2, 0)); // 5m depth, 0.5m dz
        assert!(!path.is_valid()); // Still need 2

        // Add second anchor (deeper)
        assert!(path.toggle_anchor(&table, 5, 1)); // 50m depth, 1.0m dz
        assert!(path.is_valid());
    }

    #[test]
    fn test_monotonicity_check() {
        let table = ConstructionTable::new();
        let mut path = PathSelection::new();

        // Add anchors with valid monotonicity
        path.toggle_anchor(&table, 2, 2); // Lower depth, fewer levels
        path.toggle_anchor(&table, 5, 1); // Higher depth, more levels

        assert!(path.is_valid());
    }
}
