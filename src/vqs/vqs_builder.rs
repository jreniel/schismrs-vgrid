// schismrs-vgrid/src/vqs/vqs_builder.rs

use super::errors::VQSBuilderError;
use super::vqs::VQS;
use crate::transforms::StretchingFunction;
use log::{debug, error, info, trace, warn};
use ndarray::Array2;
use ndarray::Array1;
use schismrs_hgrid::hgrid::Hgrid;
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
        info!("========================================");
        info!("Starting VQS build process");
        info!("========================================");
        let build_start = Instant::now();

        let hgrid = self
            .hgrid
            .ok_or_else(|| VQSBuilderError::UninitializedFieldError("hgrid".to_string()))?;

        info!("Hgrid loaded:");
        info!("  - Total nodes: {}", hgrid.nodes().len());
        info!("  - Total elements: {}", hgrid.elements().hash_map().len());

        // Analyze depth distribution
        let node_depths = hgrid.depths();
        let mut dry_nodes = 0;
        let mut wet_nodes = 0;
        let mut max_depth = 0.0f64;
        let mut min_wet_depth = f64::MAX;

        for (idx, &depth) in node_depths.iter().enumerate() {
            if depth >= 0.0 {
                dry_nodes += 1;
            } else {
                wet_nodes += 1;
                let abs_depth = -depth;
                max_depth = max_depth.max(abs_depth);
                if abs_depth > 0.0 {
                    min_wet_depth = min_wet_depth.min(abs_depth);
                }

                // Log first few wet nodes for debugging
                if wet_nodes <= 5 {
                    debug!("  Wet node {}: depth = {:.3}m", idx, abs_depth);
                }
            }
        }

        info!("Node statistics:");
        info!("  - Dry nodes: {}", dry_nodes);
        info!("  - Wet nodes: {}", wet_nodes);
        info!("  - Min wet depth: {:.3}m", min_wet_depth);
        info!("  - Max depth: {:.3}m", max_depth);

        let depths = self
            .depths
            .as_ref()
            .ok_or_else(|| VQSBuilderError::UninitializedFieldError("depths".to_string()))?;

        info!("Master depths: {} grids", depths.len());
        for (i, &d) in depths.iter().enumerate() {
            info!("  Grid {}: {:.3}m", i + 1, d);
        }

        let nlevels = self
            .nlevels
            .as_ref()
            .ok_or_else(|| VQSBuilderError::UninitializedFieldError("nlevels".to_string()))?;

        info!("Master levels:");
        for (i, &n) in nlevels.iter().enumerate() {
            info!("  Grid {}: {} levels", i + 1, n);
            if n < 2 {
                error!(
                    "  WARNING: Grid {} has only {} levels (TRIDAG requires >= 2)",
                    i + 1,
                    n
                );
            }
        }

        // Check for potential issues
        if depths.len() != nlevels.len() {
            error!(
                "MISMATCH: {} depths but {} level specifications",
                depths.len(),
                nlevels.len()
            );
        }

        let stretching = self
            .stretching
            .clone()
            .ok_or_else(|| VQSBuilderError::UninitializedFieldError("stretching".to_string()))?;

        info!("Stretching function: {:?}", stretching);

        let min_bottom_layer_thickness = match self.dz_bottom_min {
            Some(value) => {
                info!("Using provided dz_bottom_min: {:.6}", value);
                *value
            }
            None => {
                // Use a fraction of the average layer thickness in the shallowest region
                let shallow_depth = depths[0];
                let shallow_levels = nlevels[0];
                let calculated = shallow_depth / (shallow_levels as f64) * 0.5;
                info!(
                    "Calculated dz_bottom_min: {:.6} (from shallow_depth={:.3}, shallow_levels={})",
                    calculated, shallow_depth, shallow_levels
                );
                calculated
            }
        };

        Self::validate_dz_bottom_min(&min_bottom_layer_thickness)?;

        info!("Creating transform...");
        let transform_start = Instant::now();

        // Add error handling with more context
        let transform = match stretching.transform(hgrid, depths, nlevels) {
            Ok(t) => {
                info!("Transform created successfully");
                t
            }
            Err(e) => {
                error!("Failed to create transform: {}", e);
                error!("  depths: {:?}", depths);
                error!("  nlevels: {:?}", nlevels);
                return Err(e.into());
            }
        };

        let transform_elapsed = transform_start.elapsed();
        info!("Transform created in {:?}", transform_elapsed);

        let z_mas = transform.zmas();
        let etal = transform.etal();

        info!("Transform properties:");
        info!("  - etal: {:.6}", etal);
        info!("  - a_vqs0: {:.6}", transform.a_vqs0());
        info!("  - z_mas shape: [{}, {}]", z_mas.nrows(), z_mas.ncols());

        // Check z_mas for potential issues
        for j in 0..z_mas.ncols() {
            let col_min = z_mas.column(j).iter().fold(f64::MAX, |a, &b| a.min(b));
            let col_max = z_mas.column(j).iter().fold(f64::MIN, |a, &b| a.max(b));
            info!(
                "  - z_mas column {} (depth={:.3}): range [{:.3}, {:.3}]",
                j,
                depths.get(j).unwrap_or(&0.0),
                col_min,
                col_max
            );
        }

        info!("Building sigma_vqs and znd arrays...");
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

        // Final statistics
        info!("Final VQS statistics:");
        info!(
            "  - sigma_vqs shape: [{}, {}]",
            sigma_vqs.nrows(),
            sigma_vqs.ncols()
        );
        info!("  - znd shape: [{}, {}]", znd.nrows(), znd.ncols());
        info!("  - kbp length: {}", kbp.len());

        // Check for nodes with too few levels
        let mut level_histogram = std::collections::HashMap::new();
        for &k in kbp.iter() {
            *level_histogram.entry(k).or_insert(0) += 1;
        }

        info!("Level distribution:");
        let mut sorted_levels: Vec<_> = level_histogram.iter().collect();
        sorted_levels.sort_by_key(|&(k, _)| k);
        for (levels, count) in sorted_levels {
            info!("  {} levels: {} nodes", levels, count);
            if *levels < 2 {
                error!("  WARNING: {} nodes have only {} levels!", count, levels);
            }
        }

        if arrays_elapsed.as_secs() > 10 {
            warn!(
                "Array building took longer than expected: {:?}",
                arrays_elapsed
            );
        }

        let total_elapsed = build_start.elapsed();
        info!("VQS build completed in {:?}", total_elapsed);
        info!("========================================");

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

        info!("========================================");
        info!("Starting build_sigma_vqs");
        info!("========================================");

        let bathymetry = hgrid.depths();
        let node_count = bathymetry.len();

        info!("Input parameters:");
        info!("  - Node count: {}", node_count);
        info!("  - Master depths: {:?}", master_depths);
        info!("  - Master levels: {:?}", master_levels);
        info!("  - etal: {:.6}", etal);
        info!("  - a_vqs0: {:.6}", a_vqs0);
        info!(
            "  - min_bottom_layer_thickness: {:.6}",
            min_bottom_layer_thickness
        );

        // Convert to positive depths for calculations
        let depths: Vec<f64> = bathymetry.iter().map(|&d| -d).collect();

        // Maximum number of vertical levels
        let max_levels = *master_levels.iter().max().unwrap_or(&0);
        info!("Maximum vertical levels: {}", max_levels);

        if max_levels < 2 {
            error!(
                "CRITICAL: max_levels = {} is too small for TRIDAG!",
                max_levels
            );
        }

        // Initialize arrays with -9.0 for below-bottom points (SCHISM convention)
        let mut sigma = Array2::<f64>::from_elem((max_levels, node_count), -9.0);
        let mut z_coords = Array2::<f64>::from_elem((max_levels, node_count), f64::NAN);

        // Track the actual bottom level for each node (number of levels with data)
        let mut kbp = vec![0usize; node_count];

        // Elevation at each node
        let elevations = Array1::from_elem(node_count, *etal);

        let mut shallow_count = 0;
        let mut deep_count = 0;
        let _dry_count = 0;
        let mut error_count = 0;
        let mut problem_nodes = Vec::new();

        info!("Processing {} nodes...", node_count);

        // Process each node
        for (node_idx, &depth) in depths.iter().enumerate() {
            if node_idx % 10000 == 0 && node_idx > 0 {
                info!(
                    "Progress: {} nodes processed ({} shallow, {} deep)",
                    node_idx, shallow_count, deep_count
                );
            }

            // Log details for first few nodes and any problematic ones
            let should_log_details = node_idx < 10 ||
                                    (node_idx % 10000 == 0) ||
                                    depth < 0.01 ||  // Very shallow
                                    depth > master_depths[master_depths.len() - 1] * 1.5; // Very deep

            if should_log_details {
                debug!("Processing node {}: depth = {:.3}m", node_idx, depth);
            }

            if depth <= 0.0 {
                // Dry node
                if node_idx < 5 {
                    debug!("  Node {} is dry (depth = {:.3})", node_idx, depth);
                }
                kbp[node_idx] = 0;
                continue;
            }

            if depth <= master_depths[0] {
                shallow_count += 1;

                if should_log_details {
                    debug!(
                        "  Node {} is shallow (depth {:.3} <= {:.3})",
                        node_idx, depth, master_depths[0]
                    );
                }

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

                if kbp[node_idx] < 2 {
                    error!(
                        "WARNING: Shallow node {} has only {} levels!",
                        node_idx, kbp[node_idx]
                    );
                    problem_nodes.push((node_idx, depth, kbp[node_idx]));
                }
            } else {
                deep_count += 1;

                if should_log_details {
                    debug!(
                        "  Node {} is deep (depth {:.3} > {:.3})",
                        node_idx, depth, master_depths[0]
                    );
                }

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
                    Ok(_) => {
                        if kbp[node_idx] < 2 {
                            error!(
                                "WARNING: Deep node {} has only {} levels!",
                                node_idx, kbp[node_idx]
                            );
                            problem_nodes.push((node_idx, depth, kbp[node_idx]));
                        }
                    }
                    Err(e) => {
                        error_count += 1;
                        error!("ERROR processing deep node {}: {}", node_idx, e);
                        error!(
                            "  Node details: depth={:.3}, elevation={:.3}",
                            depth, elevations[node_idx]
                        );

                        // Try to find which master grid would be used
                        for i in 1..master_depths.len() {
                            if depth > master_depths[i - 1] && depth <= master_depths[i] {
                                error!("  Would use master grid {}: depths [{:.3}, {:.3}], levels [{}, {}]",
                                      i, master_depths[i-1], master_depths[i],
                                      master_levels[i-1], master_levels[i]);
                            }
                        }

                        if error_count <= 5 {
                            // Only return error for first few failures
                            return Err(e);
                        }
                    }
                }
            }
        }

        info!("Node processing complete:");
        info!("  - Shallow nodes: {}", shallow_count);
        info!("  - Deep nodes: {}", deep_count);
        info!("  - Error count: {}", error_count);

        if !problem_nodes.is_empty() {
            error!(
                "CRITICAL: {} nodes have fewer than 2 levels:",
                problem_nodes.len()
            );
            for (idx, depth, levels) in problem_nodes.iter().take(10) {
                error!("  Node {}: depth={:.3}m, levels={}", idx, depth, levels);
            }
        }

        // Convert to SCHISM output format where level 1 is bottom, level nvrt is surface
        info!("Converting to SCHISM output format...");
        let mut output_sigma = Array2::<f64>::from_elem((max_levels, node_count), -9.0);

        let mut min_levels = usize::MAX;
        let mut max_used_levels = 0usize;

        for node_idx in 0..node_count {
            let bottom_level = kbp[node_idx];

            if bottom_level > 0 {
                min_levels = min_levels.min(bottom_level);
                max_used_levels = max_used_levels.max(bottom_level);
            }

            // Copy values in reverse order (our surface-to-bottom becomes SCHISM's bottom-to-surface)
            for k in 0..bottom_level {
                let output_level = max_levels - bottom_level + k;
                output_sigma[[output_level, node_idx]] = sigma[[bottom_level - 1 - k, node_idx]];
            }
        }

        info!("Level statistics after conversion:");
        info!("  - Minimum levels used: {}", min_levels);
        info!("  - Maximum levels used: {}", max_used_levels);

        if min_levels < 2 {
            error!(
                "CRITICAL: Minimum levels {} is less than 2 - TRIDAG will fail!",
                min_levels
            );
        }

        let elapsed = start.elapsed();
        info!("Sigma_vqs and znd arrays built in {:?}", elapsed);
        info!("========================================");

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
        trace!(
            "process_shallow_node: node={}, depth={:.3}, levels={}",
            node_idx,
            depth,
            levels
        );

        if levels < 2 {
            warn!(
                "  Shallow node {} assigned only {} levels - forcing to 2",
                node_idx, levels
            );
            // Force minimum 2 levels for TRIDAG
            kbp[node_idx] = 2;

            // Simple linear distribution for 2 levels
            sigma[[0, node_idx]] = 0.0; // Surface
            sigma[[1, node_idx]] = -1.0; // Bottom

            z_coords[[0, node_idx]] = elevation;
            z_coords[[1, node_idx]] = -depth;
        } else {
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

        trace!(
            "  Shallow node {} complete: kbp={}",
            node_idx,
            kbp[node_idx]
        );
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
        trace!("process_deep_node: node={}, depth={:.3}", node_idx, depth);

        // Find the appropriate master grid
        let grid_idx = Self::find_master_grid(depth, master_depths)?;
        trace!("  Using master grid {}", grid_idx);
        trace!(
            "  Master grid range: [{:.3}, {:.3}]",
            master_depths[grid_idx - 1],
            master_depths[grid_idx]
        );
        trace!(
            "  Master levels: [{}, {}]",
            master_levels[grid_idx - 1],
            master_levels[grid_idx]
        );

        // Calculate interpolation factor
        let zrat = (depth - master_depths[grid_idx - 1])
            / (master_depths[grid_idx] - master_depths[grid_idx - 1]);
        trace!("  Interpolation factor zrat: {:.4}", zrat);

        // Find bottom level and interpolate z-coordinates
        let mut bottom_level = 0;
        let mut last_z3 = elevation;

        for k in 0..master_levels[grid_idx] {
            // Interpolate between master grids
            let k_prev = std::cmp::min(k, master_levels[grid_idx - 1] - 1);
            let z1 = z_mas[[k_prev, grid_idx - 1]];
            let z2 = z_mas[[k, grid_idx]];
            let z3 = z1 + (z2 - z1) * zrat;

            if k < 5 || k >= master_levels[grid_idx] - 2 {
                trace!(
                    "    k={}: z1={:.3}, z2={:.3}, z3={:.3}, threshold={:.3}",
                    k,
                    z1,
                    z2,
                    z3,
                    -depth + min_bottom_layer_thickness
                );
            }

            if z3 >= -depth + min_bottom_layer_thickness {
                // Store z-coordinate
                z_coords[[k, node_idx]] = z3;
                bottom_level = k + 1; // +1 because we want the count, not index
                last_z3 = z3;
            } else {
                // We've reached the bottom
                trace!(
                    "  Reached bottom at k={}, z3={:.3} < threshold={:.3}",
                    k,
                    z3,
                    -depth + min_bottom_layer_thickness
                );
                break;
            }
        }

        debug!("  Node {} bottom_level: {}", node_idx, bottom_level);

        if bottom_level == 0 {
            error!("  Failed to find bottom for node {}", node_idx);
            error!(
                "    depth: {:.3}, min_thickness: {:.3}",
                depth, min_bottom_layer_thickness
            );
            error!(
                "    grid_idx: {}, master levels: {}",
                grid_idx, master_levels[grid_idx]
            );
            return Err(VQSBuilderError::FailedToFindABottom(
                node_idx + 1,
                depth,
                -depth,
                Array1::zeros(1),
            ));
        }

        if bottom_level == 1 {
            warn!("  Node {} has only 1 level - forcing to 2", node_idx);
            bottom_level = 2;
            // Add a mid-level
            z_coords[[0, node_idx]] = elevation;
            z_coords[[1, node_idx]] = -depth;
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
                error!(
                    "  Z-inversion at node {} level {}: z[{}]={:.3} <= z[{}]={:.3}",
                    node_idx,
                    k,
                    k - 1,
                    z_coords[[k - 1, node_idx]],
                    k,
                    z_coords[[k, node_idx]]
                );
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

        trace!("  Deep node {} complete: kbp={}", node_idx, kbp[node_idx]);

        Ok(())
    }

    /// Find which master grid to use for a given depth
    fn find_master_grid(depth: f64, master_depths: &[f64]) -> Result<usize, VQSBuilderError> {
        for i in 1..master_depths.len() {
            if depth > master_depths[i - 1] && depth <= master_depths[i] {
                trace!(
                    "  Depth {:.3} falls in master grid {} (range: {:.3}-{:.3})",
                    depth,
                    i,
                    master_depths[i - 1],
                    master_depths[i]
                );
                return Ok(i);
            }
        }

        // Check if depth is beyond last master depth
        if depth > master_depths[master_depths.len() - 1] {
            error!(
                "  Depth {:.3} exceeds maximum master depth {:.3}",
                depth,
                master_depths[master_depths.len() - 1]
            );
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
            error!("Invalid dz_bottom_min: {} (must be >= 0)", dz_bottom_min);
            return Err(VQSBuilderError::InvalidDzBottomMin);
        }
        Ok(())
    }
}
