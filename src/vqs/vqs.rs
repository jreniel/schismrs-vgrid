// schismrs-vgrid/src/vqs/vqs.rs

use crate::transforms::traits::{Transform, TransformPlotterError};
use log::{debug, info, trace, warn};
use ndarray::Array2;
use plotly::Plot;
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
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
        
        // FIXED: Now that all nodes (including dry) have kbp >= 2,
        // we can use the simple conversion formula
        // SCHISM convention: nvrt + 1 - kbp gives the bottom level index
        self.kbp.iter().map(|&kbp| {
            if kbp == 0 {
                // This should not happen anymore, but just in case
                warn!("Found node with kbp=0, converting to 2-level node");
                self.nvrt() - 1  // 2 levels
            } else {
                self.nvrt() + 1 - kbp
            }
        }).collect()
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
        
        debug!("Writing header: ivcor={}, nvrt={}", self.ivcor(), self.nvrt());

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