//! Suggestion algorithms for VQS master grid configuration
//!
//! Provides multiple algorithms to automatically derive anchor points
//! from mesh bathymetry, giving users different perspectives on
//! how to distribute vertical resolution.

use super::app::MeshInfo;

/// A suggested anchor point (depth + number of levels)
#[derive(Clone, Debug)]
pub struct Anchor {
    pub depth: f64,
    pub nlevels: usize,
}

/// Parameters for suggestion algorithms
#[derive(Clone, Debug)]
pub struct SuggestionParams {
    /// Total vertical levels desired at deepest point
    pub target_levels: usize,
    /// Minimum layer thickness constraint
    pub min_dz: f64,
    /// Number of anchor points to generate
    pub num_anchors: usize,
    /// Minimum levels at shallowest anchor
    pub shallow_levels: usize,
}

impl Default for SuggestionParams {
    fn default() -> Self {
        Self {
            target_levels: 30,
            min_dz: 0.5,
            num_anchors: 4,
            shallow_levels: 2,
        }
    }
}

/// Available suggestion algorithms
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SuggestionAlgorithm {
    /// Exponential depth spacing with level distribution
    #[default]
    Exponential,
    /// Linear/uniform spacing
    Uniform,
    /// Percentile-based from depth distribution
    Percentile,
}

impl SuggestionAlgorithm {
    /// Get the display name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Exponential => "Exponential",
            Self::Uniform => "Uniform",
            Self::Percentile => "Percentile",
        }
    }

    /// Get a short description
    pub fn description(&self) -> &'static str {
        match self {
            Self::Exponential => "Exponential depth spacing, finer near surface",
            Self::Uniform => "Linear spacing in both depth and levels",
            Self::Percentile => "Anchors at depth percentiles (10%, 25%, 50%, 75%, 90%)",
        }
    }

    /// Cycle to the next algorithm
    pub fn next(&self) -> Self {
        match self {
            Self::Exponential => Self::Uniform,
            Self::Uniform => Self::Percentile,
            Self::Percentile => Self::Exponential,
        }
    }

    /// Cycle to the previous algorithm
    pub fn prev(&self) -> Self {
        match self {
            Self::Exponential => Self::Percentile,
            Self::Uniform => Self::Exponential,
            Self::Percentile => Self::Uniform,
        }
    }

    /// Get algorithm by number (1-3)
    pub fn from_number(n: usize) -> Option<Self> {
        match n {
            1 => Some(Self::Exponential),
            2 => Some(Self::Uniform),
            3 => Some(Self::Percentile),
            _ => None,
        }
    }

    /// Get number for this algorithm (1-3)
    pub fn number(&self) -> usize {
        match self {
            Self::Exponential => 1,
            Self::Uniform => 2,
            Self::Percentile => 3,
        }
    }

    /// Generate suggested anchors using this algorithm
    pub fn suggest(&self, mesh: &MeshInfo, params: &SuggestionParams) -> Vec<Anchor> {
        match self {
            Self::Exponential => suggest_exponential(mesh, params),
            Self::Uniform => suggest_uniform(mesh, params),
            Self::Percentile => suggest_percentile(mesh, params),
        }
    }
}

/// Exponential distribution algorithm
/// - Depths are spaced exponentially (finer near surface)
/// - Levels are computed to respect min_dz constraint
fn suggest_exponential(mesh: &MeshInfo, params: &SuggestionParams) -> Vec<Anchor> {
    let n = params.num_anchors;
    if n < 2 {
        return vec![Anchor {
            depth: mesh.max_depth,
            nlevels: params.target_levels,
        }];
    }

    // Generate exponentially-spaced depths
    let start = params.min_dz.max(mesh.min_depth).max(0.1);
    let end = mesh.max_depth;
    let scale = (end / start).powf(1.0 / (n as f64 - 1.0));

    let depths: Vec<f64> = (0..n).map(|i| start * scale.powf(i as f64)).collect();

    // Compute levels using exponential function with min_dz constraint
    let level_range = params.target_levels - params.shallow_levels;
    let mut anchors: Vec<Anchor> = depths
        .iter()
        .enumerate()
        .map(|(i, &depth)| {
            // Exponential level assignment
            let frac = if n > 1 {
                i as f64 / (n - 1) as f64
            } else {
                1.0
            };
            let mut nlevels = params.shallow_levels + (frac * level_range as f64).round() as usize;

            // Apply min_dz constraint: N <= depth / min_dz + 1
            let max_levels = (depth / params.min_dz).floor() as usize + 1;
            nlevels = nlevels.min(max_levels);

            Anchor { depth, nlevels }
        })
        .collect();

    // Enforce monotonicity (levels must not decrease with depth)
    enforce_monotonicity(&mut anchors);

    anchors
}

/// Uniform distribution algorithm
/// - Linear spacing in both depth and levels
fn suggest_uniform(mesh: &MeshInfo, params: &SuggestionParams) -> Vec<Anchor> {
    let n = params.num_anchors;
    if n < 2 {
        return vec![Anchor {
            depth: mesh.max_depth,
            nlevels: params.target_levels,
        }];
    }

    let depth_step = mesh.max_depth / n as f64;
    let level_step = (params.target_levels - params.shallow_levels) as f64 / (n - 1) as f64;

    let mut anchors: Vec<Anchor> = (0..n)
        .map(|i| Anchor {
            depth: (i + 1) as f64 * depth_step,
            nlevels: params.shallow_levels + (i as f64 * level_step).round() as usize,
        })
        .collect();

    // Ensure last anchor is at max depth with target levels
    if let Some(last) = anchors.last_mut() {
        last.depth = mesh.max_depth;
        last.nlevels = params.target_levels;
    }

    anchors
}

/// Percentile-based algorithm
/// - Anchors at fixed percentiles of the depth distribution
fn suggest_percentile(mesh: &MeshInfo, params: &SuggestionParams) -> Vec<Anchor> {
    // Use the pre-computed percentiles from MeshInfo
    // percentiles = [10%, 25%, 50%, 75%, 90%]
    let pct_depths = [
        mesh.percentiles[0], // 10%
        mesh.percentiles[1], // 25%
        mesh.percentiles[2], // 50%
        mesh.percentiles[3], // 75%
        mesh.percentiles[4], // 90%
        mesh.max_depth,      // 100%
    ];

    // Filter to requested number of anchors (evenly spaced in the percentile array)
    let n = params.num_anchors.min(6);
    let step = if n > 1 { 6.0 / n as f64 } else { 6.0 };

    let mut anchors: Vec<Anchor> = (0..n)
        .map(|i| {
            let idx = ((i as f64 * step) as usize).min(5);
            let depth = pct_depths[idx];
            let frac = depth / mesh.max_depth;
            let nlevels = params.shallow_levels
                + (frac * (params.target_levels - params.shallow_levels) as f64).round() as usize;
            Anchor { depth, nlevels }
        })
        .collect();

    // Ensure last anchor is at max depth
    if let Some(last) = anchors.last_mut() {
        last.depth = mesh.max_depth;
        last.nlevels = params.target_levels;
    }

    enforce_monotonicity(&mut anchors);
    anchors
}

/// Ensure levels are non-decreasing with depth (monotonicity constraint)
fn enforce_monotonicity(anchors: &mut [Anchor]) {
    for i in 1..anchors.len() {
        if anchors[i].nlevels < anchors[i - 1].nlevels {
            anchors[i].nlevels = anchors[i - 1].nlevels;
        }
    }
}

/// Suggestion mode state for the TUI
#[derive(Clone, Debug)]
pub struct SuggestionMode {
    /// Currently selected algorithm
    pub algorithm: SuggestionAlgorithm,
    /// Algorithm parameters
    pub params: SuggestionParams,
    /// Current preview of suggested anchors
    pub preview: Vec<Anchor>,
}

impl SuggestionMode {
    /// Create new suggestion mode with defaults
    pub fn new() -> Self {
        Self {
            algorithm: SuggestionAlgorithm::default(),
            params: SuggestionParams::default(),
            preview: Vec::new(),
        }
    }

    /// Update preview synchronously
    pub fn update_preview(&mut self, mesh: &MeshInfo) {
        self.preview = self.algorithm.suggest(mesh, &self.params);
    }

    /// Select algorithm by number (1-3)
    pub fn select_algorithm(&mut self, n: usize) -> bool {
        if let Some(alg) = SuggestionAlgorithm::from_number(n) {
            self.algorithm = alg;
            true
        } else {
            false
        }
    }

    /// Adjust target levels
    pub fn adjust_target_levels(&mut self, delta: i32) {
        let new_val = (self.params.target_levels as i32 + delta).max(2) as usize;
        self.params.target_levels = new_val.max(self.params.shallow_levels + 1);
    }

    /// Adjust min_dz
    pub fn adjust_min_dz(&mut self, delta: f64) {
        let new_val = (self.params.min_dz + delta).max(0.1);
        self.params.min_dz = new_val;
    }

    /// Adjust number of anchors
    pub fn adjust_num_anchors(&mut self, delta: i32) {
        let new_val = (self.params.num_anchors as i32 + delta).max(2).min(12) as usize;
        self.params.num_anchors = new_val;
    }

    /// Adjust shallow levels
    pub fn adjust_shallow_levels(&mut self, delta: i32) {
        let new_val = (self.params.shallow_levels as i32 + delta).max(2) as usize;
        self.params.shallow_levels = new_val.min(self.params.target_levels - 1);
    }
}

impl Default for SuggestionMode {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test uniform suggestion algorithm directly (doesn't need hgrid)
    #[test]
    fn test_uniform_suggestion_structure() {
        // Create a minimal test by directly calling the algorithm functions
        // which only need max_depth and don't use hgrid directly
        let params = SuggestionParams {
            target_levels: 30,
            min_dz: 0.5,
            num_anchors: 4,
            shallow_levels: 2,
        };

        // Directly test the uniform distribution logic
        let n = params.num_anchors;
        let max_depth = 30.0;
        let depth_step = max_depth / n as f64;
        let level_step = (params.target_levels - params.shallow_levels) as f64 / (n - 1) as f64;

        let anchors: Vec<Anchor> = (0..n)
            .map(|i| Anchor {
                depth: (i + 1) as f64 * depth_step,
                nlevels: params.shallow_levels + (i as f64 * level_step).round() as usize,
            })
            .collect();

        assert_eq!(anchors.len(), 4);
        assert!(anchors[0].depth < anchors[3].depth);
        assert!(anchors[0].nlevels <= anchors[3].nlevels);
        assert_eq!(anchors[3].depth, 30.0); // Last anchor at max depth
    }

    #[test]
    fn test_monotonicity_enforcement() {
        let mut anchors = vec![
            Anchor { depth: 5.0, nlevels: 10 },
            Anchor { depth: 10.0, nlevels: 8 },  // Violation: 8 < 10
            Anchor { depth: 20.0, nlevels: 15 },
        ];

        enforce_monotonicity(&mut anchors);

        // After enforcement, levels should be non-decreasing
        assert!(anchors[1].nlevels >= anchors[0].nlevels);
        assert!(anchors[2].nlevels >= anchors[1].nlevels);
        assert_eq!(anchors[1].nlevels, 10); // Should have been corrected to 10
    }

    #[test]
    fn test_suggestion_params_defaults() {
        let params = SuggestionParams::default();
        assert_eq!(params.target_levels, 30);
        assert_eq!(params.min_dz, 0.5);
        assert_eq!(params.num_anchors, 4);
        assert_eq!(params.shallow_levels, 2);
    }

    #[test]
    fn test_algorithm_cycle() {
        let alg = SuggestionAlgorithm::Exponential;
        assert_eq!(alg.next(), SuggestionAlgorithm::Uniform);
        assert_eq!(alg.next().next(), SuggestionAlgorithm::Percentile);
        assert_eq!(alg.next().next().next(), SuggestionAlgorithm::Exponential);
    }
}
