// schishmrs-vgrid/src/vqs/vqs_builder.rs

use super::errors::VQSBuilderError;
use super::vqs::VQS;
use crate::transforms::StretchingFunction;
use ndarray::Array2;
use ndarray::Axis;
use ndarray::{Array, Array1};
use schismrs_hgrid::hgrid::Hgrid;
use std::rc::Rc;

#[derive(Default)]
pub struct VQSBuilder<'a> {
    hgrid: Option<&'a Hgrid>,
    depths: Option<&'a Vec<f64>>,
    nlevels: Option<&'a Vec<usize>>,
    stretching: Option<&'a StretchingFunction<'a>>,
    dz_bottom_min: Option<&'a f64>,
}

impl<'a> VQSBuilder<'a> {
    pub fn build(&self) -> Result<VQS, VQSBuilderError> {
        let hgrid = self
            .hgrid
            .ok_or_else(|| VQSBuilderError::UninitializedFieldError("hgrid".to_string()))?;
        let depths = self
            .depths
            .as_ref()
            .ok_or_else(|| VQSBuilderError::UninitializedFieldError("depths".to_string()))?;
        let nlevels = self
            .nlevels
            .as_ref()
            .ok_or_else(|| VQSBuilderError::UninitializedFieldError("nlevels".to_string()))?;
        let stretching = self
            .stretching
            .clone()
            .ok_or_else(|| VQSBuilderError::UninitializedFieldError("stretching".to_string()))?;
        
        let min_bottom_layer_thickness = match self.dz_bottom_min {
            Some(value) => *value,
            None => {
                // Use a fraction of the average layer thickness in the shallowest region
                let shallow_depth = depths[0];
                let shallow_levels = nlevels[0];
                shallow_depth / (shallow_levels as f64) * 0.5 // Half the average layer thickness
            }
        };
        Self::validate_dz_bottom_min(&min_bottom_layer_thickness)?;
        let transform = stretching.transform(hgrid, depths, nlevels)?;
        let z_mas = transform.zmas();
        let etal = transform.etal();
        let (sigma_vqs, znd) = Self::build_sigma_vqs(
            z_mas,
            hgrid,
            depths,
            nlevels,
            etal,
            transform.a_vqs0(),
            &min_bottom_layer_thickness,
        )?;
        
        Ok(VQS::new(sigma_vqs, znd, transform))
    }

    fn build_sigma_vqs(
        z_mas: &Array2<f64>,
        hgrid: &Hgrid,
        master_depths: &[f64],
        master_levels: &[usize],
        etal: &f64,
        a_vqs0: &f64,
        min_bottom_layer_thickness: &f64,
    ) -> Result<(Array2<f64>, Array2<f64>), VQSBuilderError> {
        let bathymetry = hgrid.depths();
        let node_count = bathymetry.len();

        // Convert to positive depths for calculations
        let depths: Vec<f64> = bathymetry.iter().map(|&d| -d).collect();

        // Maximum number of vertical levels
        let max_levels = *master_levels.iter().max().unwrap_or(&0);

        // Initialize arrays with NaN values
        let mut sigma = Array2::<f64>::from_elem((max_levels, node_count), f64::NAN);
        let mut z_coords = Array2::<f64>::from_elem((max_levels, node_count), f64::NAN);

        // Elevation at each node
        let elevations = Array1::from_elem(node_count, *etal);
        // Process each node
        for (node_idx, &depth) in depths.iter().enumerate() {
            if depth <= master_depths[0] {
                // Handle shallow areas
                Self::process_shallow_node(
                    node_idx,
                    depth,
                    master_levels[0],
                    a_vqs0,
                    elevations[node_idx],
                    &mut sigma,
                    &mut z_coords,
                );
            } else {
                // Handle deeper areas
                Self::process_deep_node(
                    node_idx,
                    depth,
                    master_depths,
                    master_levels,
                    z_mas,
                    elevations[node_idx],
                    min_bottom_layer_thickness,
                    &mut sigma,
                    &mut z_coords,
                )?;
            }
        }

        // Flip arrays to match expected orientation (bottom to surface)
        sigma.invert_axis(Axis(0));

        Ok((sigma, z_coords))
    }
    
    /// Process a node in a shallow area (depth <= first master depth)
    fn process_shallow_node(
        node_idx: usize,
        depth: f64,
        levels: usize,
        a_vqs0: &f64,
        elevation: f64,
        sigma: &mut Array2<f64>,
        z_coords: &mut Array2<f64>,
    ) {
        for k in 0..levels {
            // Calculate sigma using quadratic transformation
            // Note: Fortran uses 1-based indexing, so adjust formula
            let s = (k as f64 - 1.0) / (1.0 - levels as f64);
            let transformed_sigma = a_vqs0 * s * s + (1.0 + a_vqs0) * s;

            // Store sigma value
            sigma[[k, node_idx]] = transformed_sigma;

            // Calculate and store z-coordinate
            z_coords[[k, node_idx]] = transformed_sigma * (elevation + depth) + elevation;
        }
    }

    /// Process a node in a deeper area (depth > first master depth)
    fn process_deep_node(
        node_idx: usize,
        depth: f64,
        master_depths: &[f64],
        master_levels: &[usize],
        z_mas: &Array2<f64>,
        elevation: f64,
        min_bottom_layer_thickness: &f64,
        sigma: &mut Array2<f64>,
        z_coords: &mut Array2<f64>,
    ) -> Result<(), VQSBuilderError> {
        // Find the appropriate master grid
        let grid_idx = Self::find_master_grid(depth, master_depths)?;

        // Calculate interpolation factor
        let zrat = (depth - master_depths[grid_idx - 1])
            / (master_depths[grid_idx] - master_depths[grid_idx - 1]);

        // Find bottom level and interpolate z-coordinates
        let mut bottom_level_found = false;
        let mut bottom_level = 0;

        for k in 0..master_levels[grid_idx] {
            // Interpolate between master grids
            let z1 = z_mas[[
                std::cmp::min(k, master_levels[grid_idx - 1] - 1),
                grid_idx - 1,
            ]];
            let z2 = z_mas[[k, grid_idx]];
            let z3 = z1 + (z2 - z1) * zrat;

            if z3 >= -depth + min_bottom_layer_thickness {
                // Store z-coordinate
                z_coords[[k, node_idx]] = z3;
            } else {
                // We've reached the bottom
                bottom_level = k;
                bottom_level_found = true;
                break;
            }
        }

        if !bottom_level_found {
            return Err(VQSBuilderError::FailedToFindABottom(
                node_idx + 1,
                depth,
                z_coords[[bottom_level, node_idx]],
                z_mas.row(0).to_owned(),
            ));
        }

        // Set bottom z-coordinate to exactly match bathymetry
        z_coords[[bottom_level, node_idx]] = -depth;

        // Calculate sigma values
        sigma[[0, node_idx]] = 0.0; // Surface
        sigma[[bottom_level, node_idx]] = -1.0; // Bottom

        // Calculate intermediate sigma values
        for k in 1..bottom_level {
            sigma[[k, node_idx]] = (z_coords[[k, node_idx]] - elevation) / (elevation + depth);
        }

        // Check for inversions
        for k in 1..=bottom_level {
            if z_coords[[k - 1, node_idx]] <= z_coords[[k, node_idx]] {
                return Err(VQSBuilderError::InvertedZ(
                    node_idx + 1,
                    depth,
                    grid_idx,
                    k,
                    z_coords[[k - 1, node_idx]],
                    z_coords[[k, node_idx]],
                ));
            }
        }

        Ok(())
    }

    /// Find which master grid to use for a given depth
    fn find_master_grid(depth: f64, master_depths: &[f64]) -> Result<usize, VQSBuilderError> {
        for i in 1..master_depths.len() {
            if depth > master_depths[i - 1] && depth <= master_depths[i] {
                return Ok(i);
            }
        }

        Err(VQSBuilderError::FailedToFindAMasterVgrid(0, depth))
    }

    pub fn hgrid(&mut self, hgrid: &'a Hgrid) -> &mut Self {
        self.hgrid = Some(hgrid);
        self
    }

    pub fn depths(&mut self, depths: &'a Vec<f64>) -> &mut Self {
        self.depths = Some(depths);
        self
    }
    
    pub fn nlevels(&mut self, nlevels: &'a Vec<usize>) -> &mut Self {
        self.nlevels = Some(nlevels);
        self
    }
    
    pub fn stretching(&mut self, stretching: &'a StretchingFunction) -> &mut Self {
        self.stretching = Some(stretching);
        self
    }
    
    pub fn dz_bottom_min(&mut self, dz_bottom_min: &'a f64) -> &mut Self {
        self.dz_bottom_min = Some(dz_bottom_min);
        self
    }
    
    pub fn validate_dz_bottom_min(dz_bottom_min: &f64) -> Result<(), VQSBuilderError> {
        if *dz_bottom_min < 0. {
            return Err(VQSBuilderError::InvalidDzBottomMin);
        }
        Ok(())
    }
}