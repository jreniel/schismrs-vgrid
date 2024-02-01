use super::traits::Transform;
use ndarray::Array2;
use schismrs_hgrid::Hgrid;
use std::f64::NAN;
use thiserror::Error;

pub struct QuadraticTransform {
    zmas: Array2<f64>,
    etal: f64,
    a_vqs0: f64,
}

impl Transform for QuadraticTransform {
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

#[derive(Default)]
pub struct QuadraticTransformBuilder<'a> {
    hgrid: Option<&'a Hgrid>,
    etal: Option<&'a f64>,
    depths: Option<&'a Vec<f64>>,
    nlevels: Option<&'a Vec<usize>>,
    a_vqs0: Option<&'a f64>,
    skew_decay_rate: Option<&'a f64>,
}

impl<'a> QuadraticTransformBuilder<'a> {
    pub fn build(&self) -> Result<QuadraticTransform, QuadraticTransformBuilderError> {
        let hgrid = self.hgrid.ok_or_else(|| {
            QuadraticTransformBuilderError::UninitializedFieldError("hgrid".to_string())
        })?;
        let depths = self.depths.ok_or_else(|| {
            QuadraticTransformBuilderError::UninitializedFieldError("depths".to_string())
        })?;
        Self::validate_depths(hgrid, depths)?;
        let nlevels = self.nlevels.ok_or_else(|| {
            QuadraticTransformBuilderError::UninitializedFieldError("nlevels".to_string())
        })?;
        Self::validate_nlevels(nlevels)?;
        Self::validate_depths_and_nlevels(depths, nlevels)?;
        let etal = self.etal.ok_or_else(|| {
            QuadraticTransformBuilderError::UninitializedFieldError("etal".to_string())
        })?;
        Self::validate_etal(etal, &depths[0])?;
        let a_vqs0 = self.a_vqs0.ok_or_else(|| {
            QuadraticTransformBuilderError::UninitializedFieldError("a_vqs0".to_string())
        })?;
        Self::validate_a_vqs0(a_vqs0)?;
        let skew_decay_rate = self.skew_decay_rate.ok_or_else(|| {
            QuadraticTransformBuilderError::UninitializedFieldError("skew_decay_rate".to_string())
        })?;
        // Self::validate_skew_decay_rate
        let zmas = Self::build_zmas(depths, nlevels, etal, a_vqs0, skew_decay_rate);
        Ok(QuadraticTransform {
            zmas,
            etal: *etal,
            a_vqs0: *a_vqs0,
        })
    }

    pub fn build_zmas(
        depths: &Vec<f64>,
        nlevels: &Vec<usize>,
        etal: &f64,
        a_vqs0: &f64,
        skew_decay_rate: &f64,
    ) -> Array2<f64> {
        let num_grids = depths.len();
        let max_levels = nlevels.iter().max().unwrap();
        let mut z_mas = Array2::from_elem((*max_levels, num_grids), NAN);
        let a_vqs = Self::build_vertical_stretching_factors(num_grids, a_vqs0, skew_decay_rate);
        for (m, &depth) in depths.iter().enumerate() {
            let nlev = nlevels[m];
            for k in 0..nlev {
                let sigma = (k as f64) / (1. - nlev as f64);
                let tmp = a_vqs[m] * sigma * sigma + (1.0 + a_vqs[m]) * sigma;
                z_mas[[k, m]] = tmp * (*etal + depth) + *etal;
            }
        }
        // verify:
        if log::log_enabled!(log::Level::Debug) {
            log::debug!("z_mas levels:");

            for row in 0..z_mas.shape()[0] {
                let mut row_string = String::from(format!("{:4} ", row + 1));
                for col in 0..z_mas.shape()[1] {
                    row_string += &format!("{:12.4} ", z_mas[[row, col]]);
                }
                println!("{}", row_string);
            }
        }

        // use std::fs::File;
        // use std::io::{Error, Write};
        // let mut file = File::create("rust.12").expect("Unable to create file");
        // for row in 0..z_mas.shape()[0] {
        //     let mut row_string = String::from(format!("{:4} ", row + 1));
        //     for col in 0..z_mas.shape()[1] {
        //         row_string += &format!("{:12.4} ", z_mas[[row, col]]);
        //     }
        //     write!(file, " {}\n", row_string).unwrap();
        // }
        // file.flush().unwrap();
        //
        //
        //
        //
        //
        // use std::fs::File;
        // use std::io::{Error, Write};
        // let mut file = File::create("vgrid_master.out").expect("Unable to create file");
        // for (m, &depth) in depths.iter().enumerate() {
        //     write!(file, " {:5} {:5} {:12.4} ", m + 1, nlevels[m], depth)
        //         .expect("Unable to write to file");
        //     for k in 0..nlevels[m] {
        //         write!(file, "{:12.4} ", z_mas[[k, m]]).expect("Unable to write to file");
        //     }
        //     writeln!(file).expect("Unable to write newline to file");
        // }

        // // Ensure data is flushed to the file
        // file.flush().expect("Unable to flush file");

        // let mut file = File::create("a_vqs0.out").expect("Unable to create file");
        // for (m, &value) in a_vqs.iter().enumerate() {
        //     write!(file, "{} {}\n", m + 1, value).unwrap();
        // }

        // file.flush().unwrap(); // Ensure data is flushed to the file
        z_mas
    }

    fn build_vertical_stretching_factors(
        num_grids: usize,
        a_vqs0: &f64,
        skew_decay_rate: &f64,
    ) -> Vec<f64> {
        let mut a_vqs = Vec::with_capacity(num_grids);
        for m in 0..num_grids {
            let mut a = *a_vqs0;
            if m != 0 {
                a = a - m as f64 * *skew_decay_rate;
            }
            a = a.max(-1.0);
            a_vqs.push(a);
        }
        a_vqs
    }

    fn validate_depths_and_nlevels(
        depths: &Vec<f64>,
        nlevels: &Vec<usize>,
    ) -> Result<(), QuadraticTransformBuilderError> {
        let depth_len = depths.len();
        let nlevels_len = nlevels.len();
        if depth_len != nlevels_len {
            return Err(QuadraticTransformBuilderError::DepthsAndLevelsSizeMismatch(
                depth_len,
                nlevels_len,
            ));
        }
        Ok(())
    }

    pub fn validate_a_vqs0(a_vqs0: &f64) -> Result<(), QuadraticTransformBuilderError> {
        if *a_vqs0 < -1.0 || *a_vqs0 > 1.0 {
            return Err(QuadraticTransformBuilderError::InvalidAVqs0(*a_vqs0));
        }
        Ok(())
    }

    pub fn validate_etal(etal: &f64, depths0: &f64) -> Result<(), QuadraticTransformBuilderError> {
        if *etal >= *depths0 {
            return Err(QuadraticTransformBuilderError::InvalidEtalValue(
                *depths0, *etal,
            ));
        }
        Ok(())
    }
    fn validate_depths(
        hgrid: &Hgrid,
        depths: &Vec<f64>,
    ) -> Result<(), QuadraticTransformBuilderError> {
        let mut prev_depth = depths[0];
        for &depth in &depths[1..] {
            if depth <= prev_depth {
                return Err(QuadraticTransformBuilderError::InvalidDepths);
            }
            prev_depth = depth;
        }

        let hgrid_depths = hgrid.depths();
        let mut min_hgrid_depth = f64::MAX;
        for &depth in &hgrid_depths {
            min_hgrid_depth = min_hgrid_depth.min(depth);
        }
        let last_depth = depths[depths.len() - 1];
        if last_depth < -min_hgrid_depth {
            return Err(QuadraticTransformBuilderError::InvalidLastDepth(
                last_depth,
                -min_hgrid_depth,
            ));
        }

        Ok(())
    }

    fn validate_nlevels(nlevels: &Vec<usize>) -> Result<(), QuadraticTransformBuilderError> {
        let mut prev_nlevel = nlevels[0];
        if prev_nlevel < 2 {
            return Err(QuadraticTransformBuilderError::InvalidFirstLevel);
        }
        for &nlevel in &nlevels[1..] {
            if nlevel < prev_nlevel {
                return Err(QuadraticTransformBuilderError::InvalidNLevels);
            }
            prev_nlevel = nlevel;
        }
        Ok(())
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
    pub fn etal(&mut self, etal: &'a f64) -> &mut Self {
        self.etal = Some(etal);
        self
    }
    pub fn skew_decay_rate(&mut self, skew_decay_rate: &'a f64) -> &mut Self {
        self.skew_decay_rate = Some(skew_decay_rate);
        self
    }
    pub fn a_vqs0(&mut self, a_vqs0: &'a f64) -> &mut Self {
        self.a_vqs0 = Some(a_vqs0);
        self
    }
}

#[derive(Error, Debug)]
pub enum QuadraticTransformBuilderError {
    #[error("Unitialized field on QuadraticTransformBuilder: {0}")]
    UninitializedFieldError(String),
    #[error(
        "depths and nlevels array must be of the same length. Got lengths {0} and {1} respectively"
    )]
    DepthsAndLevelsSizeMismatch(usize, usize),
    #[error("depths vector must be strictly increasing")]
    InvalidDepths,
    #[error("First level in nlevels must be >= 2")]
    InvalidFirstLevel,
    #[error("nlevels vector must be strictly increasing")]
    InvalidNLevels,
    #[error("Last depth provided was {0} but it must be greater or equal than {1} which is the deepest point in hgrid.")]
    InvalidLastDepth(f64, f64),
    #[error("a_vqs0 must be < 0 and >= -1, but got {0}")]
    InvalidAVqs0(f64),
    #[error("etal must be smaller than the first depth, (which is {0}) but got {1}")]
    InvalidEtalValue(f64, f64),
}

#[derive(Clone, Debug)]
pub struct QuadraticTransformOpts<'a> {
    pub etal: &'a f64,
    pub a_vqs0: &'a f64,
    pub skew_decay_rate: &'a f64,
}

impl<'a> QuadraticTransformOpts<'a> {
    pub fn new() -> Self {
        Self {
            etal: &0.,
            a_vqs0: &0.,
            skew_decay_rate: &0.03,
        }
    }
    pub fn etal(&mut self, etal: &'a f64) -> &mut Self {
        self.etal = etal;
        self
    }
    pub fn a_vqs0(&mut self, a_vqs0: &'a f64) -> &mut Self {
        self.a_vqs0 = a_vqs0;
        self
    }
    pub fn skew_decay_rate(&mut self, skew_decay_rate: &'a f64) -> &mut Self {
        self.skew_decay_rate = skew_decay_rate;
        self
    }
}
