use super::traits::Transform;
use libm::sinh;
use libm::tanh;
use ndarray::Array2;
use schismrs_hgrid::Hgrid;
use std::f64::NAN;
use thiserror::Error;

pub struct STransform {
    zmas: Array2<f64>,
    etal: f64,
    a_vqs0: f64,
    _theta_f: f64,
    _theta_b: f64,
}

impl Transform for STransform {
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
pub struct STransformBuilder<'a> {
    hgrid: Option<&'a Hgrid>,
    etal: Option<&'a f64>,
    depths: Option<&'a Vec<f64>>,
    nlevels: Option<&'a Vec<usize>>,
    a_vqs0: Option<&'a f64>,
    theta_b: Option<&'a f64>,
    theta_f: Option<&'a f64>,
}

// impl<'a> Default for STransformBuilder<'a> {
//     fn default() -> Self {
//         Self {
//             etal: Some(&0.),
//             hgrid: None,
//             depths: None,
//             nlevels: None,
//             a_vqs0: Some(&-1.),
//             // theta_b: Some(&0.001),
//             theta_b: None,
//             // theta_f: Some(&1.),
//             theta_f: None,
//         }
//     }
// }

impl<'a> STransformBuilder<'a> {
    pub fn build(&self) -> Result<STransform, STransformBuilderError> {
        let hgrid = self
            .hgrid
            .ok_or_else(|| STransformBuilderError::UninitializedFieldError("hgrid".to_string()))?;
        let depths = self
            .depths
            .ok_or_else(|| STransformBuilderError::UninitializedFieldError("depths".to_string()))?;
        Self::validate_depths(hgrid, depths)?;
        let nlevels = self.nlevels.ok_or_else(|| {
            STransformBuilderError::UninitializedFieldError("nlevels".to_string())
        })?;
        Self::validate_nlevels(nlevels)?;
        Self::validate_depths_and_nlevels(depths, nlevels)?;
        let etal = self
            .etal
            .ok_or_else(|| STransformBuilderError::UninitializedFieldError("etal".to_string()))?;
        Self::validate_etal(etal, &depths[0])?;
        let a_vqs0 = self
            .a_vqs0
            .ok_or_else(|| STransformBuilderError::UninitializedFieldError("a_vqs0".to_string()))?;
        Self::validate_a_vqs0(a_vqs0)?;
        let theta_b = self.theta_b.ok_or_else(|| {
            STransformBuilderError::UninitializedFieldError("theta_b".to_string())
        })?;
        Self::validate_theta_b(theta_b)?;
        let theta_f = self.theta_f.ok_or_else(|| {
            STransformBuilderError::UninitializedFieldError("theta_f".to_string())
        })?;
        Self::validate_theta_f(theta_f)?;
        let zmas = Self::build_zmas(depths, nlevels, etal, theta_b, theta_f);
        Ok(STransform {
            zmas,
            etal: *etal,
            a_vqs0: *a_vqs0,
            _theta_f: *theta_f,
            _theta_b: *theta_b,
        })
    }

    pub fn build_zmas(
        depths: &Vec<f64>,
        nlevels: &Vec<usize>,
        etal: &f64,
        theta_b: &f64,
        theta_f: &f64,
    ) -> Array2<f64> {
        let num_grids = depths.len();
        let max_levels = nlevels.iter().max().unwrap();
        let mut z_mas = Array2::from_elem((*max_levels, num_grids), NAN);
        for (m, &depth) in depths.iter().enumerate() {
            let nlev = nlevels[m];
            for k in 0..nlev {
                let sigma = (k as f64) / (1. - nlev as f64);
                let cs = (1. - *theta_b) * sinh(*theta_f * sigma) / sinh(*theta_f)
                    + *theta_b * (tanh(*theta_f * (sigma + 0.5)) - tanh(*theta_f * 0.5))
                        / (2. * tanh(*theta_f * 0.5));
                z_mas[[k, m]] = *etal * (1. + sigma) + depths[0] * sigma + (depth - depths[0]) * cs;
            }
        }
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

        // let mut file = File::create("rust.12").expect("Unable to create file");
        // for row in 0..z_mas.shape()[0] {
        //     let mut row_string = String::from(format!("{:4} ", row + 1));
        //     for col in 0..z_mas.shape()[1] {
        //         row_string += &format!("{:12.4} ", z_mas[[row, col]]);
        //     }
        //     write!(file, " {}\n", row_string).unwrap();
        // }
        // file.flush().unwrap();
        // unimplemented!("Debugging z_mas");
        // // verify:
        // if log::log_enabled!(log::Level::Debug) {
        //     log::debug!("z_mas levels:");
        //     for row in 0..z_mas.shape()[0] {
        //         let mut row_string = String::from(format!("{:4} ", row + 1));
        //         for col in 0..z_mas.shape()[1] {
        //             row_string += &format!("{:12.4} ", z_mas[[row, col]]);
        //         }
        //         println!("{}", row_string);
        //     }
        // }
        z_mas
    }

    fn validate_depths_and_nlevels(
        depths: &Vec<f64>,
        nlevels: &Vec<usize>,
    ) -> Result<(), STransformBuilderError> {
        let depth_len = depths.len();
        let nlevels_len = nlevels.len();
        if depth_len != nlevels_len {
            return Err(STransformBuilderError::DepthsAndLevelsSizeMismatch(
                depth_len,
                nlevels_len,
            ));
        }
        Ok(())
    }

    fn validate_a_vqs0(a_vqs0: &f64) -> Result<(), STransformBuilderError> {
        if *a_vqs0 < -1.0 || *a_vqs0 > 1.0 {
            return Err(STransformBuilderError::InvalidAVqs0(*a_vqs0));
        }
        Ok(())
    }

    pub fn validate_etal(etal: &f64, depths0: &f64) -> Result<(), STransformBuilderError> {
        if *etal >= *depths0 {
            return Err(STransformBuilderError::InvalidEtalValue(*depths0, *etal));
        }
        Ok(())
    }
    fn validate_depths(hgrid: &Hgrid, depths: &Vec<f64>) -> Result<(), STransformBuilderError> {
        let mut prev_depth = depths[0];
        for &depth in &depths[1..] {
            if depth <= prev_depth {
                return Err(STransformBuilderError::InvalidDepths);
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
            return Err(STransformBuilderError::InvalidLastDepth(
                last_depth,
                -min_hgrid_depth,
            ));
        }

        Ok(())
    }

    pub fn validate_theta_b(theta_b: &f64) -> Result<(), STransformBuilderError> {
        if !(0.0 <= *theta_b && *theta_b <= 1.0) {
            return Err(STransformBuilderError::InvalidThetaB(*theta_b));
        };
        Ok(())
    }

    pub fn validate_theta_f(theta_f: &f64) -> Result<(), STransformBuilderError> {
        if *theta_f <= 0. || *theta_f > 20. {
            return Err(STransformBuilderError::InvalidThetaF(*theta_f));
        };
        Ok(())
    }
    fn validate_nlevels(nlevels: &Vec<usize>) -> Result<(), STransformBuilderError> {
        let mut prev_nlevel = nlevels[0];
        if prev_nlevel < 2 {
            return Err(STransformBuilderError::InvalidFirstLevel);
        }
        for &nlevel in &nlevels[1..] {
            if nlevel < prev_nlevel {
                return Err(STransformBuilderError::InvalidNLevels);
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
    pub fn theta_b(&mut self, theta_b: &'a f64) -> &mut Self {
        self.theta_b = Some(theta_b);
        self
    }
    pub fn theta_f(&mut self, theta_f: &'a f64) -> &mut Self {
        self.theta_f = Some(theta_f);
        self
    }
    pub fn a_vqs0(&mut self, a_vqs0: &'a f64) -> &mut Self {
        self.a_vqs0 = Some(a_vqs0);
        self
    }
}

#[derive(Clone, Debug)]
pub struct STransformOpts<'a> {
    pub etal: &'a f64,
    pub a_vqs0: &'a f64,
    pub theta_b: &'a f64,
    pub theta_f: &'a f64,
}

impl<'a> STransformOpts<'a> {
    pub fn new() -> Self {
        Self {
            etal: &0.,
            a_vqs0: &0.,
            theta_b: &0.,
            theta_f: &0.001,
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
    pub fn theta_b(&mut self, theta_b: &'a f64) -> &mut Self {
        self.theta_b = theta_b;
        self
    }
    pub fn theta_f(&mut self, theta_f: &'a f64) -> &mut Self {
        self.theta_f = theta_f;
        self
    }
}
#[derive(Error, Debug)]
pub enum STransformBuilderError {
    #[error("Unitialized field on STransformBuilder: {0}")]
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
    #[error("theta_b must be in [0., 1.], but got {0}")]
    InvalidThetaB(f64),
    #[error("theta_f must be larger than 0, and smaller or equal to 20., but got {0}")]
    InvalidThetaF(f64),
    #[error("etal must be smaller than the first depth, (which is {0}) but got {1}")]
    InvalidEtalValue(f64, f64),
}
