// schishmrs-vgrid/src/vqs/vqs.rs

use crate::transforms::traits::{Transform, TransformPlotterError};
use ndarray::Array2;
use plotly::Plot;
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;

pub struct VQS {
    sigma_vqs: Array2<f64>,
    // _depths: Array1<f64>,
    // _etal: f64,
    _znd: Array2<f64>,
    // z_mas: Array2<f64>,
    transform: Rc<dyn Transform>,
}

impl VQS {
    pub fn new(
        sigma_vqs: Array2<f64>,
        _znd: Array2<f64>,
        transform: Rc<dyn Transform>,
    ) -> Self {
        Self {
            sigma_vqs,
            _znd,
            transform,
        }
    }

    pub fn write_to_file(&self, filename: &PathBuf) -> std::io::Result<()> {
        let mut file = File::create(filename)?;
        write!(file, "{}", self)?;
        Ok(())
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
        let num_columns = self.sigma_vqs.shape()[1];
        let num_rows = self.sigma_vqs.shape()[0];

        let mut indices = Vec::with_capacity(num_columns);

        for col in 0..num_columns {
            let mut row_index = 0;
            while row_index < num_rows && self.sigma_vqs[[row_index, col]].is_nan() {
                row_index += 1;
            }
            indices.push(row_index + 1);
        }

        indices
    }

    fn iter_level_values(&self) -> IterLevelValues {
        IterLevelValues {
            vqs: self,
            level: 0,
        }
    }

    fn values_at_level(&self, level: usize) -> Vec<f64> {
        self.sigma_vqs.row(level - 1).to_vec()
    }

    pub fn make_z_mas_plot(&self) -> Result<Plot, TransformPlotterError> {
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
            return None;
        }
        let values = self.vqs.values_at_level(self.level);
        Some((self.level, values))
    }
}

impl fmt::Display for VQS {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Write ivcor and nvrt values
        write!(f, "{:>12}\n", self.ivcor())?;
        write!(f, "{:>12}\n", self.nvrt())?;

        // Write number of levels at each node
        write!(
            f,
            "{}",
            self.bottom_level_indices()
                .iter()
                .map(|&index| format!("{:>10}", index))
                .collect::<Vec<_>>()
                .join(" ")
        )?;
        write!(f, "\n")?; // Make sure to end the line

        // Write sigma values level by level
        for (level, values) in self.iter_level_values() {
            // Write level number followed by values
            write!(f, "{:>10}", level)?;

            // Format each value with proper spacing
            for value in values {
                if value.is_nan() {
                    // Use -9.0 for below-bottom points
                    write!(f, "{:14.6}", -9.0)?;
                } else {
                    write!(f, "{:14.6}", value)?;
                }
            }
            write!(f, "\n")?;
        }

        Ok(())
    }
}