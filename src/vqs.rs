use crate::transforms::quadratic::QuadraticTransformBuilder;
use crate::transforms::quadratic::QuadraticTransformBuilderError;
use crate::transforms::traits::Transform;
use crate::transforms::StretchingFunction;
use crate::{kmeans_hsm, KMeansHSMCreateError};
use ndarray::Array1;
use ndarray::Array2;
use ndarray::Axis;
use schismrs_mesh::hgrid::Hgrid;
use std::cmp::max;
use std::f64::NAN;
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use thiserror::Error;
// use ndarray_stats::QuantileExt

pub struct VQS {
    sigma_vqs: Array2<f64>,
}

impl VQS {
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
        write!(f, "{:>12}\n", self.ivcor())?;
        write!(f, "{:>12}\n", self.nvrt())?;
        write!(
            f,
            " {}\n",
            self.bottom_level_indices()
                .iter()
                .map(|&index| format!("{:>10}", index))
                .collect::<Vec<_>>()
                .join(" ")
        )?;
        for (level, values) in self.iter_level_values() {
            let formatted_values: Vec<String> = values
                .iter()
                .map(|value| {
                    if value.is_nan() {
                        format!("{:15.6}", -9.0)
                    } else {
                        format!("{:15.6}", value)
                    }
                })
                .collect();

            write!(f, "{:>10}{}\n", level, formatted_values.join(""))
                .expect("Error writing to output");
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct VQSBuilder<'a> {
    hgrid: Option<&'a Hgrid>,
    depths: Option<&'a Vec<f64>>,
    nlevels: Option<&'a Vec<usize>>,
    stretching: Option<&'a StretchingFunction<'a>>,
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

        let transform = match stretching {
            StretchingFunction::Quadratic(opts) => {
                let mut builder = QuadraticTransformBuilder::default();
                builder.hgrid(hgrid);
                builder.depths(depths);
                builder.nlevels(nlevels);
                opts.as_ref().map(|opts| {
                    opts.etal.as_ref().map(|etal| {
                        builder.etal(etal);
                    });

                    opts.a_vqs0.as_ref().map(|a_vqs0| builder.a_vqs0(a_vqs0));
                });
                builder.build()?
            }
        };
        let z_mas = transform.zmas();
        let sigma_vqs = Self::build_sigma_vqs(
            z_mas,
            hgrid,
            depths,
            nlevels,
            transform.etal(),
            transform.a_vqs0(),
        )?;
        Ok(VQS { sigma_vqs })
    }

    fn build_sigma_vqs(
        z_mas: &Array2<f64>,
        hgrid: &Hgrid,
        hsm: &Vec<f64>,
        nv_vqs: &Vec<usize>,
        etal: &f64,
        a_vqs0: &f64,
    ) -> Result<Array2<f64>, VQSBuilderError> {
        let nvrt = z_mas.nrows();
        let dp = -hgrid.depths();
        let np = dp.len();
        let mut sigma_vqs = Array2::from_elem((nvrt, np), NAN);
        let mut kbp = Array1::zeros(np);
        let eta2 = Array1::from_elem(np, etal);
        // let mut znd = Array2::from_elem((nvrt, np), NAN);
        let uninitialized_m0_value = hsm.len() + 1;
        let mut m0 = Array1::from_elem(np, uninitialized_m0_value);
        for i in 0..np {
            for m in 1..hsm.len() {
                if dp[i] > hsm[m - 1] && dp[i] <= hsm[m] {
                    m0[i] = m;
                    break;
                }
            }
        }
        for i in 0..np {
            let this_dp = dp[i];
            if this_dp <= hsm[0] {
                kbp[i] = nv_vqs[0];
                for k in 0..nv_vqs[0] {
                    let sigma = k as f64 / (1. - nv_vqs[0] as f64);
                    sigma_vqs[[k, i]] = a_vqs0 * sigma * sigma + (1. + a_vqs0) * sigma;
                    // znd[[k, i]] = sigma_vqs[[k, i]] * (eta2[i] + dp[i]) + eta2[i];
                }
            // compute sigma_vqs based on depth & stretching
            } else {
                for k in 0..nvrt {
                    let z1 = z_mas[[max(0, k as i64 - 1) as usize, m0[i]]];
                    let z2 = z_mas[[k, m0[i]]];
                    let zrat = (this_dp - hsm[m0[i] - 1]) / (hsm[m0[i]] - hsm[m0[i] - 1]);
                    let z3 = z1 + (z2 - z1) * zrat;
                    sigma_vqs[[k, i]] = (z3 + this_dp) / (eta2[i] + this_dp);
                }

                // set surface & bottom
                sigma_vqs[[0, i]] = 0.0;
                sigma_vqs[[kbp[i], i]] = -1.0;
                // for k in 1..kbp[i] {
                //     if sigma_vqs[[k, i]] >= sigma_vqs[[k - 1, i]] {
                //         // The reference Fortran code had this monotonicity check,
                //         // but I'm not entirely sure that this condition is reachable.
                //         unimplemented!("non monotonic")
                //     }
                // }
            }
        }
        sigma_vqs.invert_axis(Axis(0));
        Ok(sigma_vqs)
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
}

#[derive(Error, Debug)]
pub enum VQSBuilderError {
    #[error("Unitialized field on VQSBuilder: {0}")]
    UninitializedFieldError(String),
    #[error(transparent)]
    QuadraticTransformBuilderError(#[from] QuadraticTransformBuilderError),
}

pub struct VQSKMeansBuilder<'a> {
    hgrid: Option<&'a Hgrid>,
    nclusters: Option<&'a usize>,
    stretching: Option<&'a StretchingFunction<'a>>,
    shallow_threshold: Option<&'a f64>,
    shallow_levels: Option<&'a usize>,
}

impl<'a> Default for VQSKMeansBuilder<'a> {
    fn default() -> Self {
        Self {
            hgrid: None,
            stretching: None,
            nclusters: None,
            shallow_levels: Some(&1),
            shallow_threshold: Some(&0.),
        }
    }
}

impl<'a> VQSKMeansBuilder<'a> {
    pub fn build(&self) -> Result<VQS, VQSKMeansBuilderError> {
        let hgrid = self
            .hgrid
            .ok_or_else(|| VQSKMeansBuilderError::UninitializedFieldError("hgrid".to_string()))?;
        let stretching = self.stretching.ok_or_else(|| {
            VQSKMeansBuilderError::UninitializedFieldError("stretching".to_string())
        })?;
        let nclusters = self.nclusters.ok_or_else(|| {
            VQSKMeansBuilderError::UninitializedFieldError("nclusters".to_string())
        })?;
        let shallow_threshold = self.shallow_threshold.ok_or_else(|| {
            VQSKMeansBuilderError::UninitializedFieldError("shallow_threshold".to_string())
        })?;
        let shallow_levels = self.shallow_levels.ok_or_else(|| {
            VQSKMeansBuilderError::UninitializedFieldError("shallow_levels".to_string())
        })?;
        let mut hsm = kmeans_hsm(hgrid, nclusters, shallow_threshold)?;
        hsm.iter_mut().for_each(|depth| *depth = depth.abs());
        let mut nlevels = Vec::<usize>::with_capacity(*nclusters);
        for i in *shallow_levels..(*shallow_levels + *nclusters) {
            nlevels.push(i);
        }
        Ok(VQSBuilder::default()
            .hgrid(&hgrid)
            .depths(&hsm)
            .nlevels(&nlevels)
            .stretching(&stretching)
            .build()?)
    }

    pub fn hgrid(&mut self, hgrid: &'a Hgrid) -> &mut Self {
        self.hgrid = Some(hgrid);
        self
    }
    pub fn nclusters(&mut self, nclusters: &'a usize) -> &mut Self {
        self.nclusters = Some(nclusters);
        self
    }
    pub fn stretching(&mut self, stretching: &'a StretchingFunction) -> &mut Self {
        self.stretching = Some(stretching);
        self
    }
    pub fn shallow_threshold(&mut self, shallow_threshold: &'a f64) -> &mut Self {
        self.shallow_threshold = Some(shallow_threshold);
        self
    }
    pub fn shallow_levels(&mut self, shallow_levels: &'a usize) -> &mut Self {
        self.shallow_levels = Some(shallow_levels);
        self
    }
}

#[derive(Error, Debug)]
pub enum VQSKMeansBuilderError {
    #[error("Unitialized field on VQSKMeansBuilder: {0}")]
    UninitializedFieldError(String),
    #[error(transparent)]
    VQSBuilderError(#[from] VQSBuilderError),
    #[error(transparent)]
    KMeansHSMCreateError(#[from] KMeansHSMCreateError),
}
