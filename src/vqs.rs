use crate::transforms::quadratic::QuadraticTransformBuilder;
use crate::transforms::quadratic::QuadraticTransformBuilderError;
use crate::transforms::s::STransformBuilder;
use crate::transforms::s::STransformBuilderError;
use crate::transforms::traits::{Transform, TransformPlotterError};
use crate::transforms::StretchingFunction;
use crate::{kmeans_hsm, KMeansHSMCreateError};
use ndarray::Array1;
use ndarray::Array2;
use ndarray::Axis;
use plotly::Plot;
use schismrs_hgrid::hgrid::Hgrid;
use std::cmp::min;
use std::f64::NAN;
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use thiserror::Error;

pub struct VQS {
    sigma_vqs: Array2<f64>,
    // _depths: Array1<f64>,
    // _etal: f64,
    _znd: Array2<f64>,
    // z_mas: Array2<f64>,
    transform: Rc<dyn Transform>,
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

        let dz_bottom_min = self
            .dz_bottom_min
            .clone()
            .ok_or_else(|| VQSBuilderError::UninitializedFieldError("dz_bottom_min".to_string()))?;
        Self::validate_dz_bottom_min(dz_bottom_min)?;
        let transform: Rc<dyn Transform> = match stretching {
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
                    opts.skew_decay_rate
                        .as_ref()
                        .map(|skew_decay_rate| builder.skew_decay_rate(skew_decay_rate));
                });
                Rc::new(builder.build()?)
            }
            StretchingFunction::S(opts) => {
                let mut builder = STransformBuilder::default();
                builder.hgrid(hgrid);
                builder.depths(depths);
                builder.nlevels(nlevels);
                opts.as_ref().map(|opts| {
                    opts.etal.as_ref().map(|etal| {
                        builder.etal(etal);
                    });

                    opts.a_vqs0.as_ref().map(|a_vqs0| builder.a_vqs0(a_vqs0));
                    opts.theta_b
                        .as_ref()
                        .map(|theta_b| builder.theta_b(theta_b));
                    opts.theta_f
                        .as_ref()
                        .map(|theta_f| builder.theta_f(theta_f));
                });
                Rc::new(builder.build()?)
            }
        };
        let z_mas = transform.zmas();
        let etal = transform.etal();
        let (sigma_vqs, znd) = Self::build_sigma_vqs(
            z_mas,
            hgrid,
            depths,
            nlevels,
            etal,
            transform.a_vqs0(),
            dz_bottom_min,
        )?;
        // let depths = hgrid.depths();
        Ok(VQS {
            sigma_vqs,
            // _depths: depths,
            // _etal: *etal,
            _znd: znd,
            // z_mas: z_mas.clone(),
            transform,
        })
    }

    fn build_sigma_vqs(
        z_mas: &Array2<f64>,
        hgrid: &Hgrid,
        hsm: &Vec<f64>,
        nv_vqs: &Vec<usize>,
        etal: &f64,
        a_vqs0: &f64,
        dz_bottom_min: &f64,
    ) -> Result<(Array2<f64>, Array2<f64>), VQSBuilderError> {
        let nvrt = z_mas.nrows();
        let dp = -hgrid.depths();
        let np = dp.len();
        let mut sigma_vqs = Array2::from_elem((nvrt, np), NAN);
        let mut kbp = Array1::zeros(np);
        let eta2 = Array1::from_elem(np, etal);
        let mut znd = Array2::from_elem((nvrt, np), NAN);
        let uninitialized_m0_value = hsm.len() + 1;
        let mut m0 = Array1::from_elem(np, uninitialized_m0_value);
        for i in 0..np {
            if dp[i] <= hsm[0] {
                kbp[i] = nv_vqs[0];
                for k in 0..nv_vqs[0] {
                    let sigma = (k as f64) / (1.0 - nv_vqs[0] as f64);
                    sigma_vqs[[k, i]] = a_vqs0 * sigma * sigma + (1.0 + a_vqs0) * sigma;
                    znd[[k, i]] = sigma_vqs[[k, i]] * (eta2[i] + dp[i]) + eta2[i];
                }
            } else {
                m0[i] = 0;
                let mut zrat = 0.;
                for m in 1..hsm.len() {
                    if dp[i] > hsm[m - 1] && dp[i] <= hsm[m] {
                        m0[i] = m;
                        zrat = (dp[i] - hsm[m - 1]) / (hsm[m] - hsm[m - 1]);
                        break;
                    }
                }
                if m0[i] == 0 {
                    return Err(VQSBuilderError::FailedToFindAMasterVgrid(i + 1, dp[i]));
                }

                // interpolate vertical levels
                kbp[i] = 0;
                let mut z3 = NAN;
                for k in 0..nv_vqs[m0[i]] {
                    let z1 = z_mas[[min(k, nv_vqs[m0[i] - 1]), m0[i] - 1]];
                    let z2 = z_mas[[k, m0[i]]];
                    z3 = z1 + (z2 - z1) * zrat;

                    if z3 >= -dp[i] + dz_bottom_min {
                        znd[[k, i]] = z3;
                    } else {
                        kbp[i] = k;
                        break;
                    }
                }
                if kbp[i] == 0 {
                    return Err(VQSBuilderError::FailedToFindABottom(
                        i + 1,
                        dp[i],
                        z3,
                        z_mas.index_axis(Axis(1), m0[i]).to_owned(),
                    ));
                }
                znd[[kbp[i], i]] = -dp[i];
                for k in 1..kbp[i] {
                    if znd[[k - 1, i]] <= znd[[k, i]] {
                        return Err(VQSBuilderError::InvertedZ(
                            i + 1,
                            dp[i],
                            m0[i],
                            k,
                            znd[[k - 1, i]],
                            znd[[k, i]],
                        ));
                    }
                }
            }
        }
        // let mut file = File::create("znd.out").expect("Unable to create file");
        // for j in 0..znd.ncols() {
        //     let line = (0..znd.nrows())
        //         .map(|i| format!("{:16.6}", znd[[i, j]])) // Format each number with 16 decimal places and align
        //         .collect::<Vec<_>>()
        //         .join(" ");
        //     writeln!(file, "{:10} {:16.6} {}", j + 1, dp[j], line)
        //         .expect("Unable to write to file");
        // }
        // file.flush().expect("Unable to flush file");
        // unimplemented!("wrote znd.out");
        sigma_vqs.invert_axis(Axis(0));
        Ok((sigma_vqs, znd))
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
    fn validate_dz_bottom_min(dz_bottom_min: &f64) -> Result<(), VQSBuilderError> {
        if *dz_bottom_min < 0. {
            return Err(VQSBuilderError::InvalidDzBottomMin);
        }
        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum VQSBuilderError {
    #[error("Unitialized field on VQSBuilder: {0}")]
    UninitializedFieldError(String),
    #[error(transparent)]
    QuadraticTransformBuilderError(#[from] QuadraticTransformBuilderError),
    #[error(transparent)]
    STransformBuilderError(#[from] STransformBuilderError),
    #[error("dz_bottom_min must be >= 0")]
    InvalidDzBottomMin,
    #[error("Failed to find a master vgrid for node id: {0} and depth {1}")]
    FailedToFindAMasterVgrid(usize, f64),
    #[error("Failed to find a bottom for node id: {0}, depth {1}, z3={2}, z_mas={3}")]
    FailedToFindABottom(usize, f64, f64, Array1<f64>),
    #[error("Inverted Z for node id: {0}, depth {1}, m0[i]={2}, k={3}, znd[[k-1, i]]={4}, znd[[k, i]]={5}")]
    InvertedZ(usize, f64, usize, usize, f64, f64),
}

#[derive(Default)]
pub struct VQSKMeansBuilder<'a> {
    hgrid: Option<&'a Hgrid>,
    nclusters: Option<&'a usize>,
    stretching: Option<&'a StretchingFunction<'a>>,
    etal: Option<&'a f64>,
    shallow_levels: Option<&'a usize>,
    dz_bottom_min: Option<&'a f64>,
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
        let etal = self
            .etal
            .ok_or_else(|| VQSKMeansBuilderError::UninitializedFieldError("etal".to_string()))?;
        let shallow_levels = self.shallow_levels.ok_or_else(|| {
            VQSKMeansBuilderError::UninitializedFieldError("shallow_levels".to_string())
        })?;
        Self::validate_shallow_levels(shallow_levels)?;
        let dz_bottom_min = self.dz_bottom_min.ok_or_else(|| {
            VQSKMeansBuilderError::UninitializedFieldError("dz_bottom_min".to_string())
        })?;
        let mut hsm = kmeans_hsm(hgrid, nclusters, etal)?;
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
            .dz_bottom_min(&dz_bottom_min)
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
    pub fn etal(&mut self, etal: &'a f64) -> &mut Self {
        self.etal = Some(etal);
        self
    }
    pub fn shallow_levels(&mut self, shallow_levels: &'a usize) -> &mut Self {
        self.shallow_levels = Some(shallow_levels);
        self
    }
    pub fn dz_bottom_min(&mut self, dz_bottom_min: &'a f64) -> &mut Self {
        self.dz_bottom_min = Some(dz_bottom_min);
        self
    }
    fn validate_shallow_levels(shallow_levels: &'a usize) -> Result<(), VQSKMeansBuilderError> {
        if *shallow_levels < 2 {
            return Err(VQSKMeansBuilderError::InvalidShallowLevels);
        }
        Ok(())
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
    #[error("shallow_levels must be >= 2")]
    InvalidShallowLevels,
}
