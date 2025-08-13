// schismrs-vgrid/src/transforms/reconstructed.rs

use super::traits::Transform;
use ndarray::Array2;
use std::f64::NAN;

/// A transform that represents a reconstructed VQS from a loaded file.
/// Since we're reverse-engineering, we don't have the original transform parameters,
/// so this acts as a placeholder that stores the extracted master grids.
pub struct ReconstructedTransform {
    zmas: Array2<f64>,
    etal: f64,
    a_vqs0: f64,
    master_depths: Vec<f64>,
    master_levels: Vec<usize>,
}

impl ReconstructedTransform {
    /// Create a new reconstructed transform from extracted master grids
    pub fn new(
        master_depths: Vec<f64>,
        master_levels: Vec<usize>,
        etal: f64,
        a_vqs0: f64,
    ) -> Self {
        let zmas = Self::build_zmas(&master_depths, &master_levels, etal);
        
        Self {
            zmas,
            etal,
            a_vqs0,
            master_depths,
            master_levels,
        }
    }
    
    /// Build z_mas array from master depths and levels
    /// This is a simplified version that assumes linear interpolation between levels
    fn build_zmas(depths: &[f64], levels: &[usize], etal: f64) -> Array2<f64> {
        let num_grids = depths.len();
        let max_levels = *levels.iter().max().unwrap_or(&0);
        let mut z_mas = Array2::from_elem((max_levels, num_grids), NAN);
        
        for (m, &depth) in depths.iter().enumerate() {
            let nlev = levels[m];
            for k in 0..nlev {
                // Simple linear distribution for reconstructed grids
                let sigma = k as f64 / (nlev - 1) as f64;
                z_mas[[k, m]] = etal - sigma * depth;
            }
        }
        
        z_mas
    }
    
    /// Get the master depths that were extracted
    pub fn master_depths(&self) -> &[f64] {
        &self.master_depths
    }
    
    /// Get the master levels that were extracted
    pub fn master_levels(&self) -> &[usize] {
        &self.master_levels
    }
}

impl Transform for ReconstructedTransform {
    fn zmas(&self) -> &Array2<f64> {
        &self.zmas
    }
    
    fn etal(&self) -> &f64 {
        &self.etal
    }
    
    fn a_vqs0(&self) -> &f64 {
        &self.a_vqs0
    }
}