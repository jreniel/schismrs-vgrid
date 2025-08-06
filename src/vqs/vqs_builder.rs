// schismrs-vgrid/src/vqs/vqs_builder.rs

use super::errors::VQSBuilderError;
use super::vqs::VQS;
use crate::transforms::StretchingFunction;
use log::{debug, info, trace, warn};
use ndarray::Array2;
use ndarray::{Array, Array1};
use schismrs_hgrid::hgrid::Hgrid;
use std::rc::Rc;
use std::time::Instant;

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
        info!("Starting VQS build process");
        let build_start = Instant::now();
        
        let hgrid = self
            .hgrid
            .ok_or_else(|| VQSBuilderError::UninitializedFieldError("hgrid".to_string()))?;
        
        debug!("Hgrid nodes: {}", hgrid.nodes().len());
        
        let depths = self
            .depths
            .as_ref()
            .ok_or_else(|| VQSBuilderError::UninitializedFieldError("depths".to_string()))?;
        
        debug!("Master depths: {:?}", depths);
        
        let nlevels = self
            .nlevels
            .as_ref()
            .ok_or_else(|| VQSBuilderError::UninitializedFieldError("nlevels".to_string()))?;
        
        debug!("Master levels: {:?}", nlevels);
        
        let stretching = self
            .stretching
            .clone()
            .ok_or_else(|| VQSBuilderError::UninitializedFieldError("stretching".to_string()))?;
        
        let min_bottom_layer_thickness = match self.dz_bottom_min {
            Some(value) => {
                debug!("Using provided dz_bottom_min: {}", value);
                *value
            }
            None => {
                // Use a fraction of the average layer thickness in the shallowest region
                let shallow_depth = depths[0];
                let shallow_levels = nlevels[0];
                let calculated = shallow_depth / (shallow_levels as f64) * 0.5;
                info!("Calculated dz_bottom_min: {} (from shallow_depth={}, shallow_levels={})", 
                     calculated, shallow_depth, shallow_levels);
                calculated
            }
        };
        
        Self::validate_dz_bottom_min(&min_bottom_layer_thickness)?;
        
        info!("Creating transform");
        let transform_start = Instant::now();
        let transform = stretching.transform(hgrid, depths, nlevels)?;
        let transform_elapsed = transform_start.elapsed();
        debug!("Transform created in {:?}", transform_elapsed);
        
        let z_mas = transform.zmas();
        let etal = transform.etal();
        
        info!("Building sigma_vqs and znd arrays");
        let arrays_start = Instant::now();
        
        let (sigma_vqs, znd, kbp) = Self::build_sigma_vqs(
            z_mas,
            hgrid,
            depths,
            nlevels,
            etal,
            transform.a_vqs0(),
            &min_bottom_layer_thickness,
        )?;
        
        let arrays_elapsed = arrays_start.elapsed();
        info!("Arrays built in {:?}", arrays_elapsed);
        
        if arrays_elapsed.as_secs() > 10 {
            warn!("Array building took longer than expected: {:?}", arrays_elapsed);
        }
        
        let total_elapsed = build_start.elapsed();
        info!("VQS build completed in {:?}", total_elapsed);
        
        Ok(VQS::new(sigma_vqs, znd, transform, kbp))
    }

    fn build_sigma_vqs(
        z_mas: &Array2<f64>,
        hgrid: &Hgrid,
        master_depths: &[f64],
        master_levels: &[usize],
        etal: &f64,
        a_vqs0: &f64,
        min_bottom_layer_thickness: &f64,
    ) -> Result<(Array2<f64>, Array2<f64>, Vec<usize>), VQSBuilderError> {
        let start = Instant::now();
        
        let bathymetry = hgrid.depths();
        let node_count = bathymetry.len();
        
        info!("Processing {} nodes", node_count);
        debug!("Master depths range: [{}, {}]", 
               master_depths.first().unwrap_or(&0.0), 
               master_depths.last().unwrap_or(&0.0));

        // Convert to positive depths for calculations
        let depths: Vec<f64> = bathymetry.iter().map(|&d| -d).collect();

        // Maximum number of vertical levels
        let max_levels = *master_levels.iter().max().unwrap_or(&0);
        debug!("Maximum vertical levels: {}", max_levels);

        // Initialize arrays with -9.0 for below-bottom points (SCHISM convention)
        let mut sigma = Array2::<f64>::from_elem((max_levels, node_count), -9.0);
        let mut z_coords = Array2::<f64>::from_elem((max_levels, node_count), f64::NAN);
        
        // Track the actual bottom level for each node (number of levels with data)
        let mut kbp = vec![0usize; node_count];

        // Elevation at each node
        let elevations = Array1::from_elem(node_count, *etal);
        
        let mut shallow_count = 0;
        let mut deep_count = 0;
        let mut error_count = 0;
        
        // Process each node
        for (node_idx, &depth) in depths.iter().enumerate() {
            if node_idx % 10000 == 0 && node_idx > 0 {
                debug!("Processed {} nodes ({} shallow, {} deep)", 
                      node_idx, shallow_count, deep_count);
            }
            
            if depth <= master_depths[0] {
                shallow_count += 1;
                // Handle shallow areas
                Self::process_shallow_node(
                    node_idx,
                    depth,
                    master_levels[0],
                    a_vqs0,
                    elevations[node_idx],
                    &mut sigma,
                    &mut z_coords,
                    &mut kbp,
                );
            } else {
                deep_count += 1;
                // Handle deeper areas
                match Self::process_deep_node(
                    node_idx,
                    depth,
                    master_depths,
                    master_levels,
                    z_mas,
                    elevations[node_idx],
                    min_bottom_layer_thickness,
                    &mut sigma,
                    &mut z_coords,
                    &mut kbp,
                ) {
                    Ok(_) => {},
                    Err(e) => {
                        error_count += 1;
                        if error_count <= 5 {
                            warn!("Error processing deep node {}: {}", node_idx, e);
                        }
                        return Err(e);
                    }
                }
            }
        }
        
        info!("Node processing complete: {} shallow, {} deep", 
              shallow_count, deep_count);

        // Convert to SCHISM output format where level 1 is bottom, level nvrt is surface
        let mut output_sigma = Array2::<f64>::from_elem((max_levels, node_count), -9.0);
        
        for node_idx in 0..node_count {
            let bottom_level = kbp[node_idx];
            
            // Copy values in reverse order (our surface-to-bottom becomes SCHISM's bottom-to-surface)
            for k in 0..bottom_level {
                let output_level = max_levels - bottom_level + k;
                output_sigma[[output_level, node_idx]] = sigma[[bottom_level - 1 - k, node_idx]];
            }
        }
        
        let elapsed = start.elapsed();
        info!("Sigma_vqs and znd arrays built in {:?}", elapsed);

        Ok((output_sigma, z_coords, kbp))
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
        kbp: &mut Vec<usize>,
    ) {
        trace!("Processing shallow node {} with depth {}", node_idx, depth);
        
        kbp[node_idx] = levels;
        
        for k in 0..levels {
            // Calculate sigma using quadratic transformation
            // Note: k=0 is surface, k=levels-1 is bottom
            let s = (k as f64) / (1.0 - levels as f64);
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
        kbp: &mut Vec<usize>,
    ) -> Result<(), VQSBuilderError> {
        trace!("Processing deep node {} with depth {}", node_idx, depth);
        
        // Find the appropriate master grid
        let grid_idx = Self::find_master_grid(depth, master_depths)?;
        trace!("Using master grid {}", grid_idx);

        // Calculate interpolation factor
        let zrat = (depth - master_depths[grid_idx - 1])
            / (master_depths[grid_idx] - master_depths[grid_idx - 1]);

        // Find bottom level and interpolate z-coordinates
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
                bottom_level = k + 1; // +1 because we want the count, not index
            } else {
                // We've reached the bottom
                break;
            }
        }

        if bottom_level == 0 {
            return Err(VQSBuilderError::FailedToFindABottom(
                node_idx + 1,
                depth,
                -depth,
                Array1::zeros(1),
            ));
        }

        kbp[node_idx] = bottom_level;

        // Set bottom z-coordinate to exactly match bathymetry
        z_coords[[bottom_level - 1, node_idx]] = -depth;

        // Calculate sigma values
        sigma[[0, node_idx]] = 0.0; // Surface
        sigma[[bottom_level - 1, node_idx]] = -1.0; // Bottom

        // Calculate intermediate sigma values
        for k in 1..bottom_level - 1 {
            sigma[[k, node_idx]] = (z_coords[[k, node_idx]] - elevation) / (elevation + depth);
        }

        // Check for inversions
        for k in 1..bottom_level {
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
                trace!("Depth {} falls in master grid {} (range: {}-{})", 
                      depth, i, master_depths[i - 1], master_depths[i]);
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