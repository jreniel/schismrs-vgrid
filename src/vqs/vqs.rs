// schismrs-vgrid/src/vqs/vqs.rs

use super::errors::{ReconstructionError, VQSLoadError};
use crate::transforms::traits::{Transform, TransformPlotterError};
use crate::transforms::StretchingFunction;
use log::{debug, info, trace, warn};
use ndarray::{Array1, Array2, Axis};
use plotly::Plot;
use schismrs_hgrid::hgrid::Hgrid;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

pub struct VQS {
    sigma_vqs: Array2<f64>,
    _znd: Array2<f64>,
    transform: Rc<dyn Transform>,
    kbp: Vec<usize>, // Bottom level indices for each node
}

impl VQS {
    pub fn new(
        sigma_vqs: Array2<f64>,
        _znd: Array2<f64>,
        transform: Rc<dyn Transform>,
        kbp: Vec<usize>,
    ) -> Self {
        let shape = sigma_vqs.shape();
        info!(
            "Creating VQS with sigma array shape: [{}, {}]",
            shape[0], shape[1]
        );
        debug!("VQS z_nd array shape: {:?}", _znd.shape());
        debug!("VQS kbp vector length: {}", kbp.len());

        Self {
            sigma_vqs,
            _znd,
            transform,
            kbp,
        }
    }

    /// Load VQS from an existing vgrid.in file
    pub fn try_from_file(hgrid: &Hgrid, vgrid_path: &Path) -> Result<Self, VQSLoadError> {
        info!("Loading VQS from file: {:?}", vgrid_path);
        let start = Instant::now();

        let file = File::open(vgrid_path)?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();

        // Read ivcor
        reader.read_line(&mut line)?;
        let ivcor: i32 = line
            .trim()
            .parse()
            .map_err(|_| VQSLoadError::ParseError("Failed to parse ivcor".to_string()))?;

        if ivcor != 1 {
            return Err(VQSLoadError::UnsupportedIvcor(ivcor));
        }

        // Read nvrt
        line.clear();
        reader.read_line(&mut line)?;
        let nvrt: usize = line
            .trim()
            .parse()
            .map_err(|_| VQSLoadError::ParseError("Failed to parse nvrt".to_string()))?;

        // Read bottom level indices
        line.clear();
        reader.read_line(&mut line)?;
        let bottom_indices: Vec<usize> = line
            .trim()
            .split_whitespace()
            .map(|s| s.parse::<usize>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| VQSLoadError::ParseError("Failed to parse bottom indices".to_string()))?;

        let node_count = bottom_indices.len();
        if node_count != hgrid.nodes().len() {
            return Err(VQSLoadError::NodeCountMismatch(
                hgrid.nodes().len(),
                node_count,
            ));
        }

        // Convert bottom indices to kbp (actual number of levels per node)
        let kbp: Vec<usize> = bottom_indices.iter().map(|&idx| nvrt + 1 - idx).collect();

        // Initialize sigma array
        let mut sigma_vqs = Array2::<f64>::from_elem((nvrt, node_count), -9.0);

        // Read sigma values level by level
        for level in 1..=nvrt {
            line.clear();
            reader.read_line(&mut line)?;
            let parts: Vec<&str> = line.trim().split_whitespace().collect();

            if parts.is_empty() {
                return Err(VQSLoadError::InvalidFormat(format!(
                    "Empty line at level {}",
                    level
                )));
            }

            // First value is the level number
            let level_num: usize = parts[0].parse().map_err(|_| {
                VQSLoadError::ParseError(format!("Failed to parse level number at level {}", level))
            })?;

            if level_num != level {
                return Err(VQSLoadError::InvalidFormat(format!(
                    "Expected level {}, got {}",
                    level, level_num
                )));
            }

            // Remaining values are sigma values for each node
            if parts.len() - 1 != node_count {
                return Err(VQSLoadError::InconsistentDimensions(
                    node_count,
                    parts.len() - 1,
                ));
            }

            for (node_idx, sigma_str) in parts[1..].iter().enumerate() {
                let sigma_val: f64 = sigma_str.parse().map_err(|_| {
                    VQSLoadError::ParseError(format!(
                        "Failed to parse sigma at level {}, node {}",
                        level, node_idx
                    ))
                })?;
                sigma_vqs[[level - 1, node_idx]] = sigma_val;
            }
        }

        // Extract master grids from the loaded data for the reconstructed transform
        // We'll use a simplified extraction for the initial load
        let (extracted_depths, extracted_levels) =
            Self::quick_extract_master_grids(&sigma_vqs, &kbp, hgrid).map_err(|e| {
                VQSLoadError::ParseError(format!("Failed to extract master grids: {}", e))
            })?;

        // Create a reconstructed transform with the extracted master grids
        let reconstructed_opts = crate::transforms::transforms::ReconstructedOpts {
            master_depths: extracted_depths.clone(),
            master_levels: extracted_levels.clone(),
            etal: 0.0,    // Default etal for loaded files
            a_vqs0: -1.0, // Default a_vqs0 for loaded files
        };
        let stretching = StretchingFunction::Reconstructed(reconstructed_opts);

        // The transform method still expects depths and levels, but for Reconstructed type they're ignored
        let transform = stretching
            .transform(hgrid, &extracted_depths, &extracted_levels)
            .map_err(|e| VQSLoadError::ParseError(format!("Failed to create transform: {}", e)))?;

        // Create empty znd array (not needed for reconstruction)
        let znd = Array2::<f64>::zeros((nvrt, node_count));

        let elapsed = start.elapsed();
        info!("VQS loaded from file in {:?}", elapsed);

        Ok(Self::new(sigma_vqs, znd, transform, kbp))
    }

    /// Extract master grids from the VQS data using depth-level relationships
    pub fn extract_master_grids(
        &self,
        hgrid: &Hgrid,
    ) -> Result<(Vec<f64>, Vec<usize>), ReconstructionError> {
        info!("Starting master grid extraction");
        let start = Instant::now();

        let depths = hgrid.depths();
        let node_count = depths.len();

        // Collect wet nodes (depth > 0.0 means below water in SCHISM convention)
        let mut depth_level_pairs: Vec<(f64, usize)> = Vec::new();
        let mut wet_node_indices: Vec<usize> = Vec::new();

        for (idx, &depth) in depths.iter().enumerate() {
            if depth < 0.0 {
                // Negative depth means underwater
                let positive_depth = -depth;
                let levels = self.kbp[idx];
                depth_level_pairs.push((positive_depth, levels));
                wet_node_indices.push(idx);
            }
        }

        let wet_count = depth_level_pairs.len();
        info!("Found {} wet nodes with depth-level data", wet_count);

        if wet_count < 10 {
            return Err(ReconstructionError::InsufficientData(wet_count));
        }

        // Debug: Show sample sigma structures
        // Convert depths array to slice for the function
        let depths_slice: Vec<f64> = depths.to_vec();
        self.debug_sigma_structures(&wet_node_indices, &depths_slice);

        // Analyze level usage statistics
        self.analyze_level_usage(&depth_level_pairs);

        // Sort by depth for analysis
        depth_level_pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        // Identify transitions and extract master grids
        let (master_depths, master_levels) = self.identify_master_grids(&depth_level_pairs)?;

        // Validate the extracted master grids
        let success_rate =
            self.validate_master_grids(&master_depths, &master_levels, &depth_level_pairs, hgrid)?;

        info!("Master grid extraction completed in {:?}", start.elapsed());
        info!("Success rate: {:.1}%", success_rate * 100.0);

        Ok((master_depths, master_levels))
    }

    fn debug_sigma_structures(&self, wet_nodes: &[usize], depths: &[f64]) {
        println!("\n=== Sigma Grid Structure Debug ===");

        // Sample nodes at different depths
        let samples = [0.5, 5.0, 10.0, 20.0, 50.0, 100.0];

        for target_depth in samples.iter() {
            // Find node closest to target depth
            let mut best_node = None;
            let mut best_diff = f64::MAX;

            for &node_idx in wet_nodes {
                let depth = -depths[node_idx];
                let diff = (depth - target_depth).abs();
                if diff < best_diff {
                    best_diff = diff;
                    best_node = Some(node_idx);
                }
            }

            if let Some(node_idx) = best_node {
                let actual_depth = -depths[node_idx];
                let levels = self.kbp[node_idx];

                println!(
                    "\nNode {} (depth={:.2}m, levels={}):",
                    node_idx, actual_depth, levels
                );

                // Show first and last few sigma values
                let nvrt = self.sigma_vqs.nrows();
                let start_idx = nvrt - levels;

                print!("  Sigma values: [");
                for i in 0..3.min(levels) {
                    let sigma = self.sigma_vqs[[start_idx + i, node_idx]];
                    if sigma != -9.0 {
                        print!("{:.4}, ", sigma);
                    }
                }
                if levels > 6 {
                    print!("..., ");
                }
                for i in (levels.saturating_sub(3))..levels {
                    let sigma = self.sigma_vqs[[start_idx + i, node_idx]];
                    if sigma != -9.0 {
                        print!("{:.4}, ", sigma);
                    }
                }
                println!("]");
            }
        }
    }

    fn analyze_level_usage(&self, depth_level_pairs: &[(f64, usize)]) {
        println!("\n=== Level Usage Statistics ===");

        // Count frequency of each level count
        let mut level_counts: HashMap<usize, usize> = HashMap::new();
        for (_, levels) in depth_level_pairs {
            *level_counts.entry(*levels).or_insert(0) += 1;
        }

        // Sort by level count
        let mut level_stats: Vec<_> = level_counts.into_iter().collect();
        level_stats.sort_by_key(|&(levels, _)| levels);

        println!("Levels | Count | Percentage");
        println!("-------|-------|------------");
        for (levels, count) in level_stats {
            let percentage = (count as f64 / depth_level_pairs.len() as f64) * 100.0;
            println!("{:6} | {:5} | {:6.1}%", levels, count, percentage);
        }

        // Linear relationship analysis
        self.analyze_linear_relationship(depth_level_pairs);
    }

    fn analyze_linear_relationship(&self, depth_level_pairs: &[(f64, usize)]) {
        // Perform log-linear regression
        let n = depth_level_pairs.len() as f64;
        let sum_x: f64 = depth_level_pairs.iter().map(|(d, _)| d.ln()).sum();
        let sum_y: f64 = depth_level_pairs.iter().map(|(_, l)| *l as f64).sum();
        let sum_xx: f64 = depth_level_pairs.iter().map(|(d, _)| d.ln() * d.ln()).sum();
        let sum_xy: f64 = depth_level_pairs
            .iter()
            .map(|(d, l)| d.ln() * (*l as f64))
            .sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x);
        let intercept = (sum_y - slope * sum_x) / n;

        // Calculate R-squared
        let y_mean = sum_y / n;
        let ss_tot: f64 = depth_level_pairs
            .iter()
            .map(|(_, l)| {
                let y = *l as f64;
                (y - y_mean) * (y - y_mean)
            })
            .sum();

        let ss_res: f64 = depth_level_pairs
            .iter()
            .map(|(d, l)| {
                let y_pred = slope * d.ln() + intercept;
                let y = *l as f64;
                (y - y_pred) * (y - y_pred)
            })
            .sum();

        let r_squared = 1.0 - (ss_res / ss_tot);

        println!("\n=== Linear Relationship Analysis ===");
        println!(
            "Log-linear fit: levels = {:.3} * ln(depth) + {:.3}",
            slope, intercept
        );
        println!(
            "R-squared: {:.3} ({:.1}% of variance explained)",
            r_squared,
            r_squared * 100.0
        );

        // Write detailed analysis to CSV
        if let Err(e) = self.write_linear_analysis_csv(depth_level_pairs, slope, intercept) {
            warn!("Failed to write linear analysis CSV: {}", e);
        }
    }

    fn write_linear_analysis_csv(
        &self,
        depth_level_pairs: &[(f64, usize)],
        slope: f64,
        intercept: f64,
    ) -> Result<(), ReconstructionError> {
        let mut wtr = csv::Writer::from_path("linear_analysis.csv")?;
        wtr.write_record(&["depth_m", "actual_levels", "predicted_levels", "residual"])?;

        for (depth, levels) in depth_level_pairs {
            let predicted = slope * depth.ln() + intercept;
            let residual = *levels as f64 - predicted;
            wtr.write_record(&[
                depth.to_string(),
                levels.to_string(),
                format!("{:.2}", predicted),
                format!("{:.2}", residual),
            ])?;
        }

        wtr.flush()?;
        info!("Linear analysis written to linear_analysis.csv");
        Ok(())
    }

    fn identify_master_grids(
        &self,
        depth_level_pairs: &[(f64, usize)],
    ) -> Result<(Vec<f64>, Vec<usize>), ReconstructionError> {
        info!(
            "Identifying master grids from {} depth-level pairs",
            depth_level_pairs.len()
        );

        // Use gradient-based transition detection
        let mut transitions = vec![];
        let window_size = 10;

        for i in window_size..depth_level_pairs.len() - window_size {
            let prev_window = &depth_level_pairs[i - window_size..i];
            let next_window = &depth_level_pairs[i..i + window_size];

            // Calculate average gradient in each window
            let prev_gradient = self.calculate_gradient(prev_window);
            let next_gradient = self.calculate_gradient(next_window);

            // Detect significant gradient changes
            if (prev_gradient - next_gradient).abs() > 0.5 {
                transitions.push(i);
            }
        }

        // Add boundaries
        transitions.insert(0, 0);
        transitions.push(depth_level_pairs.len() - 1);
        transitions.sort();
        transitions.dedup();

        // Extract representative points
        let mut master_depths = vec![];
        let mut master_levels = vec![];

        for i in 0..transitions.len() - 1 {
            let start = transitions[i];
            let end = transitions[i + 1];
            let mid = (start + end) / 2;

            if mid < depth_level_pairs.len() {
                master_depths.push(depth_level_pairs[mid].0);
                master_levels.push(depth_level_pairs[mid].1);
            }
        }

        // Simplify to 3-5 master grids
        let target_grids = 4;
        if master_depths.len() > target_grids {
            let (simplified_depths, simplified_levels) = self.simplify_master_grids(
                &master_depths,
                &master_levels,
                target_grids,
                depth_level_pairs,
            );
            master_depths = simplified_depths;
            master_levels = simplified_levels;
        }

        println!("\nExtracted {} master grids:", master_depths.len());
        for (i, (&depth, &levels)) in master_depths.iter().zip(master_levels.iter()).enumerate() {
            println!("  Grid {}: depth={:.1}m, levels={}", i + 1, depth, levels);
        }

        Ok((master_depths, master_levels))
    }

    fn calculate_gradient(&self, window: &[(f64, usize)]) -> f64 {
        if window.len() < 2 {
            return 0.0;
        }

        let mut total_gradient = 0.0;
        for i in 1..window.len() {
            let depth_diff = window[i].0 - window[i - 1].0;
            let level_diff = (window[i].1 as f64) - (window[i - 1].1 as f64);
            if depth_diff > 0.0 {
                total_gradient += level_diff / depth_diff;
            }
        }

        total_gradient / (window.len() - 1) as f64
    }

    fn simplify_master_grids(
        &self,
        depths: &[f64],
        levels: &[usize],
        target_count: usize,
        _all_pairs: &[(f64, usize)],
    ) -> (Vec<f64>, Vec<usize>) {
        // Use K-means-like approach to find optimal master grids
        let mut simplified_depths = vec![];
        let mut simplified_levels = vec![];

        // Always include shallow and deep boundaries
        simplified_depths.push(depths[0]);
        simplified_levels.push(levels[0]);

        // Add intermediate points based on data density
        let step = depths.len() / target_count;
        for i in 1..target_count - 1 {
            let idx = i * step;
            if idx < depths.len() {
                simplified_depths.push(depths[idx]);
                simplified_levels.push(levels[idx]);
            }
        }

        // Add deepest point
        simplified_depths.push(depths[depths.len() - 1]);
        simplified_levels.push(levels[levels.len() - 1]);

        simplified_depths.sort_by(|a, b| a.partial_cmp(b).unwrap());

        (simplified_depths, simplified_levels)
    }

    fn validate_master_grids(
        &self,
        master_depths: &[f64],
        master_levels: &[usize],
        depth_level_pairs: &[(f64, usize)],
        _hgrid: &Hgrid,
    ) -> Result<f64, ReconstructionError> {
        info!("Validating extracted master grids");

        let mut matching_count = 0;
        let mut non_matching_nodes = vec![];
        let tolerance = 2; // Allow 2 levels difference

        for (idx, (depth, actual_levels)) in depth_level_pairs.iter().enumerate() {
            // Interpolate expected levels from master grids
            let expected_levels = self.interpolate_levels(*depth, master_depths, master_levels);

            if (expected_levels as i32 - *actual_levels as i32).abs() <= tolerance {
                matching_count += 1;
            } else {
                non_matching_nodes.push((idx, *depth, *actual_levels, expected_levels));
            }
        }

        let success_rate = matching_count as f64 / depth_level_pairs.len() as f64;

        println!("\nValidation results:");
        println!(
            "  Matching nodes: {} / {}",
            matching_count,
            depth_level_pairs.len()
        );
        println!("  Success rate: {:.1}%", success_rate * 100.0);
        println!("  Non-matching nodes: {}", non_matching_nodes.len());

        // Write non-matching nodes to CSV
        if !non_matching_nodes.is_empty() {
            self.write_non_matching_csv(&non_matching_nodes)?;
        }

        Ok(success_rate)
    }

    fn interpolate_levels(
        &self,
        depth: f64,
        master_depths: &[f64],
        master_levels: &[usize],
    ) -> usize {
        // Find bracketing master grids
        for i in 1..master_depths.len() {
            if depth <= master_depths[i] {
                // Linear interpolation
                let t = (depth - master_depths[i - 1]) / (master_depths[i] - master_depths[i - 1]);
                let interpolated = master_levels[i - 1] as f64
                    + t * (master_levels[i] as f64 - master_levels[i - 1] as f64);
                return interpolated.round() as usize;
            }
        }

        // Beyond last master grid
        master_levels[master_levels.len() - 1]
    }

    fn write_non_matching_csv(
        &self,
        non_matching: &[(usize, f64, usize, usize)],
    ) -> Result<(), ReconstructionError> {
        let mut wtr = csv::Writer::from_path("non_matching_nodes.csv")?;
        wtr.write_record(&[
            "node_index",
            "depth_m",
            "actual_levels",
            "expected_levels",
            "difference",
        ])?;

        for (idx, depth, actual, expected) in non_matching {
            let diff = (*expected as i32 - *actual as i32).abs();
            wtr.write_record(&[
                idx.to_string(),
                format!("{:.2}", depth),
                actual.to_string(),
                expected.to_string(),
                diff.to_string(),
            ])?;
        }

        wtr.flush()?;
        info!("Non-matching nodes written to non_matching_nodes.csv");
        Ok(())
    }

    pub fn write_to_file(&self, filename: &PathBuf) -> std::io::Result<()> {
        info!("Writing VQS to file: {:?}", filename);
        let start = Instant::now();

        let mut file = File::create(filename)?;
        let result = write!(file, "{}", self);

        let elapsed = start.elapsed();
        info!("VQS file write completed in {:?}", elapsed);

        if elapsed.as_secs() > 5 {
            warn!("VQS file write took longer than expected: {:?}", elapsed);
        }

        result
    }

    pub fn ivcor(&self) -> usize {
        1
    }

    pub fn nvrt(&self) -> usize {
        self.sigma_vqs.nrows()
    }

    pub fn sigma(&self) -> &Array2<f64> {
        &self.sigma_vqs
    }

    pub fn transform(&self) -> Rc<dyn Transform> {
        self.transform.clone()
    }
    pub fn bottom_level_indices(&self) -> Vec<usize> {
        debug!("Computing bottom level indices from kbp");
        // Return nvrt + 1 - kbp for each node (SCHISM convention)
        self.kbp
            .iter()
            .map(|&kbp| {
                let bottom_idx = self.nvrt() + 1 - kbp;
                // Ensure the index is valid
                if bottom_idx < 1 {
                    warn!(
                        "Invalid bottom index {} from kbp={}, setting to 1",
                        bottom_idx, kbp
                    );
                    1
                } else if bottom_idx > self.nvrt() {
                    warn!(
                        "Invalid bottom index {} from kbp={}, setting to {}",
                        bottom_idx,
                        kbp,
                        self.nvrt()
                    );
                    self.nvrt()
                } else {
                    bottom_idx
                }
            })
            .collect()
    }

    fn iter_level_values(&self) -> IterLevelValues {
        trace!("Creating level values iterator");
        IterLevelValues {
            vqs: self,
            level: 0,
        }
    }

    fn values_at_level(&self, level: usize) -> Vec<f64> {
        trace!("Getting values at level {}", level);
        self.sigma_vqs.row(level - 1).to_vec()
    }

    pub fn make_z_mas_plot(&self) -> Result<Plot, TransformPlotterError> {
        info!("Generating z_mas plot");
        Ok(self.transform.make_zmas_plot()?)
    }

    /// Quick extraction of master grids for file loading
    /// This is a simplified version used during file loading
    fn quick_extract_master_grids(
        _sigma_vqs: &Array2<f64>,
        kbp: &[usize],
        hgrid: &Hgrid,
    ) -> Result<(Vec<f64>, Vec<usize>), String> {
        let depths = hgrid.depths();

        // Collect depth-level pairs for wet nodes
        let mut depth_level_pairs: Vec<(f64, usize)> = Vec::new();
        for (idx, &depth) in depths.iter().enumerate() {
            if depth < 0.0 {
                // Underwater
                depth_level_pairs.push((-depth, kbp[idx]));
            }
        }

        if depth_level_pairs.is_empty() {
            return Err("No wet nodes found".to_string());
        }

        // Sort by depth
        depth_level_pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        // Simple extraction: take quartiles
        let n = depth_level_pairs.len();
        let indices = vec![0, n / 4, n / 2, 3 * n / 4, n - 1];

        let mut master_depths = Vec::new();
        let mut master_levels = Vec::new();

        for &i in &indices {
            if i < n {
                master_depths.push(depth_level_pairs[i].0);
                master_levels.push(depth_level_pairs[i].1);
            }
        }

        // Remove duplicates while preserving order
        let mut unique_pairs: Vec<(f64, usize)> = Vec::new();
        for (depth, level) in master_depths.iter().zip(master_levels.iter()) {
            if unique_pairs.is_empty() || unique_pairs.last().unwrap().1 != *level {
                unique_pairs.push((*depth, *level));
            }
        }

        let (final_depths, final_levels): (Vec<f64>, Vec<usize>) = unique_pairs.into_iter().unzip();

        if final_depths.len() < 2 {
            return Err("Insufficient unique master grids".to_string());
        }

        Ok((final_depths, final_levels))
    }
}

pub struct IterLevelValues<'a> {
    vqs: &'a VQS,
    level: usize,
}

impl<'a> Iterator for IterLevelValues<'a> {
    type Item = (usize, Vec<f64>);

    fn next(&mut self) -> Option<Self::Item> {
        self.level += 1;
        if self.level > self.vqs.sigma_vqs.shape()[0] {
            trace!("Level iterator reached end at level {}", self.level - 1);
            return None;
        }

        if self.level % 10 == 0 {
            trace!("Level iterator at level {}", self.level);
        }

        let values = self.vqs.values_at_level(self.level);
        Some((self.level, values))
    }
}

impl fmt::Display for VQS {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        info!("Formatting VQS for display output");
        let start = Instant::now();
        let mut bytes_written = 0;

        // Write ivcor and nvrt values
        write!(f, "{:>12}\n", self.ivcor())?;
        write!(f, "{:>12}\n", self.nvrt())?;
        bytes_written += 26; // Approximate

        debug!(
            "Writing header: ivcor={}, nvrt={}",
            self.ivcor(),
            self.nvrt()
        );

        // Write number of levels at each node
        let bottom_indices = self.bottom_level_indices();

        debug!("Formatting {} bottom level indices", bottom_indices.len());

        // Use a more efficient string building approach
        let formatted_indices: String = bottom_indices
            .iter()
            .map(|&index| format!("{:>10}", index))
            .collect::<Vec<_>>()
            .join(" ");

        write!(f, "{}\n", formatted_indices)?;
        bytes_written += formatted_indices.len() + 1;

        trace!("Bottom indices string length: {}", formatted_indices.len());

        // Write sigma values level by level
        let mut level_count = 0;
        let values_start = Instant::now();

        for (level, values) in self.iter_level_values() {
            level_count += 1;

            // Write level number
            write!(f, "{:>10}", level)?;
            bytes_written += 10;

            // Format each value with proper spacing
            // Pre-allocate string capacity for better performance
            let mut line = String::with_capacity(values.len() * 14);

            for value in values {
                if (value - (-9.0)).abs() < 1e-10 {
                    // Use -9.0 for below-bottom points
                    line.push_str(&format!("{:14.6}", -9.0));
                } else {
                    line.push_str(&format!("{:14.6}", value));
                }
            }

            write!(f, "{}\n", line)?;
            bytes_written += line.len() + 1;

            if level % 10 == 0 {
                trace!("Formatted {} levels", level);
            }
        }

        let values_elapsed = values_start.elapsed();

        let total_elapsed = start.elapsed();
        info!(
            "VQS Display formatting completed: {} levels, ~{} bytes in {:?}",
            level_count, bytes_written, total_elapsed
        );

        if values_elapsed.as_secs() > 2 {
            warn!(
                "Sigma values formatting took {:?} for {} levels",
                values_elapsed, level_count
            );
        }

        if total_elapsed.as_secs() > 5 {
            warn!(
                "Total Display formatting took longer than expected: {:?}",
                total_elapsed
            );
        }

        Ok(())
    }
}
