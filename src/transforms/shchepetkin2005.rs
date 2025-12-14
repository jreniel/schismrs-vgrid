// schismrs-vgrid/src/transforms/shchepetkin2005.rs
//
// Implements ROMS/Rutgers VSTRETCHING=2: A. Shchepetkin (2005) UCLA-ROMS
//
// Reference:
//   Shchepetkin, A.F. and J.C. McWilliams, 2005: The regional oceanic
//   modeling system (ROMS): a split-explicit, free-surface,
//   topography-following-coordinate oceanic model, Ocean Modelling, 9, 347-404.
//
// See: https://www.myroms.org/wiki/Vertical_S-coordinate

use super::traits::Transform;
use libm::{cosh, sinh};
use ndarray::Array2;
use schismrs_hgrid::Hgrid;
use std::f64::NAN;
use thiserror::Error;

pub struct Shchepetkin2005Transform {
    zmas: Array2<f64>,
    etal: f64,
    a_vqs0: f64,
}

impl Transform for Shchepetkin2005Transform {
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
pub struct Shchepetkin2005TransformBuilder<'a> {
    hgrid: Option<&'a Hgrid>,
    etal: Option<&'a f64>,
    depths: Option<&'a Vec<f64>>,
    nlevels: Option<&'a Vec<usize>>,
    a_vqs0: Option<&'a f64>,
    theta_s: Option<&'a f64>,
    theta_b: Option<&'a f64>,
    hc: Option<&'a f64>,
}

impl<'a> Shchepetkin2005TransformBuilder<'a> {
    pub fn build(&self) -> Result<Shchepetkin2005Transform, Shchepetkin2005TransformBuilderError> {
        let hgrid = self.hgrid.ok_or_else(|| {
            Shchepetkin2005TransformBuilderError::UninitializedFieldError("hgrid".to_string())
        })?;
        let depths = self.depths.ok_or_else(|| {
            Shchepetkin2005TransformBuilderError::UninitializedFieldError("depths".to_string())
        })?;
        Self::validate_depths(hgrid, depths)?;
        let nlevels = self.nlevels.ok_or_else(|| {
            Shchepetkin2005TransformBuilderError::UninitializedFieldError("nlevels".to_string())
        })?;
        Self::validate_nlevels(nlevels)?;
        Self::validate_depths_and_nlevels(depths, nlevels)?;
        let etal = self.etal.ok_or_else(|| {
            Shchepetkin2005TransformBuilderError::UninitializedFieldError("etal".to_string())
        })?;
        Self::validate_etal(etal, &depths[0])?;
        let a_vqs0 = self.a_vqs0.ok_or_else(|| {
            Shchepetkin2005TransformBuilderError::UninitializedFieldError("a_vqs0".to_string())
        })?;
        Self::validate_a_vqs0(a_vqs0)?;
        let theta_s = self.theta_s.ok_or_else(|| {
            Shchepetkin2005TransformBuilderError::UninitializedFieldError("theta_s".to_string())
        })?;
        Self::validate_theta_s(theta_s)?;
        let theta_b = self.theta_b.ok_or_else(|| {
            Shchepetkin2005TransformBuilderError::UninitializedFieldError("theta_b".to_string())
        })?;
        Self::validate_theta_b(theta_b)?;
        let hc = self.hc.ok_or_else(|| {
            Shchepetkin2005TransformBuilderError::UninitializedFieldError("hc".to_string())
        })?;
        Self::validate_hc(hc)?;

        let zmas = Self::build_zmas(depths, nlevels, etal, theta_s, theta_b, hc);
        Ok(Shchepetkin2005Transform {
            zmas,
            etal: *etal,
            a_vqs0: *a_vqs0,
        })
    }

    /// Build the master grid z-coordinates using Shchepetkin (2005) stretching.
    ///
    /// The ROMS formulation computes:
    /// 1. sigma coordinate: sigma = (k - KB) / (KB - 1) for k = KB-1 down to 0
    /// 2. Stretching function Cs at each level
    /// 3. z = (hc * sigma + Cs * h) / (hc + h) * h
    pub fn build_zmas(
        depths: &Vec<f64>,
        nlevels: &Vec<usize>,
        _etal: &f64,
        theta_s: &f64,
        theta_b: &f64,
        hc: &f64,
    ) -> Array2<f64> {
        let num_grids = depths.len();
        let max_levels = *nlevels.iter().max().unwrap();
        let mut z_mas = Array2::from_elem((max_levels, num_grids), NAN);

        // Constants for stretching weight calculation
        let aweight = 1.0_f64;
        let bweight = 1.0_f64;

        for (m, &depth) in depths.iter().enumerate() {
            let kb = nlevels[m];
            let kbm1 = kb - 1;
            let ds = 1.0 / (kbm1 as f64);

            // Compute sc_w (sigma at W-points) and Cs_w (stretching function)
            // Note: Fortran uses 0:KB indexing, we use 0..kb
            let mut sc_w = vec![0.0_f64; kb];
            let mut cs_w = vec![0.0_f64; kb];

            // Surface boundary
            sc_w[kbm1] = 0.0;
            cs_w[kbm1] = 0.0;

            // Interior and bottom
            for k in (1..kbm1).rev() {
                let cff_w = ds * ((k as f64) - (kbm1 as f64));
                sc_w[k] = cff_w;

                if *theta_s > 0.0 {
                    let csur = (1.0 - cosh(*theta_s * cff_w)) / (cosh(*theta_s) - 1.0);

                    if *theta_b > 0.0 {
                        let cbot = sinh(*theta_b * (cff_w + 1.0)) / sinh(*theta_b) - 1.0;
                        let sigma_plus_1 = cff_w + 1.0;
                        let cweight = sigma_plus_1.powf(aweight)
                            * (1.0
                                + (aweight / bweight) * (1.0 - sigma_plus_1.powf(bweight)));
                        cs_w[k] = cweight * csur + (1.0 - cweight) * cbot;
                    } else {
                        cs_w[k] = csur;
                    }
                } else {
                    cs_w[k] = cff_w;
                }
            }

            // Bottom boundary
            sc_w[0] = -1.0;
            cs_w[0] = -1.0;

            // Convert from ROMS sigma/Cs to z-coordinates
            // z_w = (hc * sc_w + Cs_w * h) / (hc + h) * h
            // But we need to flip the indexing: ROMS has 0=bottom, KB=surface
            // SCHISM has 0=surface, nvrt=bottom (in our convention: k=0 is surface)
            let h = depth;
            let hinv = 1.0 / (*hc + h);

            for k in 0..kb {
                // Flip: SCHISM k=0 is surface, so we map from ROMS k=kbm1-k
                let roms_k = kbm1 - k;
                let cff2_w = (*hc * sc_w[roms_k] + cs_w[roms_k] * h) * hinv;
                z_mas[[k, m]] = cff2_w * h;
            }
        }

        z_mas
    }

    fn validate_depths_and_nlevels(
        depths: &Vec<f64>,
        nlevels: &Vec<usize>,
    ) -> Result<(), Shchepetkin2005TransformBuilderError> {
        if depths.len() != nlevels.len() {
            return Err(
                Shchepetkin2005TransformBuilderError::DepthsAndLevelsSizeMismatch(
                    depths.len(),
                    nlevels.len(),
                ),
            );
        }
        Ok(())
    }

    fn validate_a_vqs0(a_vqs0: &f64) -> Result<(), Shchepetkin2005TransformBuilderError> {
        if *a_vqs0 < -1.0 || *a_vqs0 > 1.0 {
            return Err(Shchepetkin2005TransformBuilderError::InvalidAVqs0(*a_vqs0));
        }
        Ok(())
    }

    pub fn validate_etal(
        etal: &f64,
        depths0: &f64,
    ) -> Result<(), Shchepetkin2005TransformBuilderError> {
        if *etal >= *depths0 {
            return Err(Shchepetkin2005TransformBuilderError::InvalidEtalValue(
                *depths0, *etal,
            ));
        }
        Ok(())
    }

    fn validate_depths(
        hgrid: &Hgrid,
        depths: &Vec<f64>,
    ) -> Result<(), Shchepetkin2005TransformBuilderError> {
        let mut prev_depth = depths[0];
        for &depth in &depths[1..] {
            if depth <= prev_depth {
                return Err(Shchepetkin2005TransformBuilderError::InvalidDepths);
            }
            prev_depth = depth;
        }

        let hgrid_depths = hgrid.depths_positive_up();
        let mut min_hgrid_depth = f64::MAX;
        for &depth in &hgrid_depths {
            min_hgrid_depth = min_hgrid_depth.min(depth);
        }
        let last_depth = depths[depths.len() - 1];
        if last_depth < -min_hgrid_depth {
            return Err(Shchepetkin2005TransformBuilderError::InvalidLastDepth(
                last_depth,
                -min_hgrid_depth,
            ));
        }

        Ok(())
    }

    pub fn validate_theta_s(theta_s: &f64) -> Result<(), Shchepetkin2005TransformBuilderError> {
        if *theta_s < 0.0 || *theta_s > 10.0 {
            return Err(Shchepetkin2005TransformBuilderError::InvalidThetaS(
                *theta_s,
            ));
        }
        Ok(())
    }

    pub fn validate_theta_b(theta_b: &f64) -> Result<(), Shchepetkin2005TransformBuilderError> {
        if *theta_b < 0.0 || *theta_b > 4.0 {
            return Err(Shchepetkin2005TransformBuilderError::InvalidThetaB(
                *theta_b,
            ));
        }
        Ok(())
    }

    pub fn validate_hc(hc: &f64) -> Result<(), Shchepetkin2005TransformBuilderError> {
        if *hc <= 0.0 {
            return Err(Shchepetkin2005TransformBuilderError::InvalidHc(*hc));
        }
        Ok(())
    }

    fn validate_nlevels(nlevels: &Vec<usize>) -> Result<(), Shchepetkin2005TransformBuilderError> {
        let mut prev_nlevel = nlevels[0];
        if prev_nlevel < 2 {
            return Err(Shchepetkin2005TransformBuilderError::InvalidFirstLevel);
        }
        for &nlevel in &nlevels[1..] {
            if nlevel < prev_nlevel {
                return Err(Shchepetkin2005TransformBuilderError::InvalidNLevels);
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

    pub fn theta_s(&mut self, theta_s: &'a f64) -> &mut Self {
        self.theta_s = Some(theta_s);
        self
    }

    pub fn theta_b(&mut self, theta_b: &'a f64) -> &mut Self {
        self.theta_b = Some(theta_b);
        self
    }

    pub fn hc(&mut self, hc: &'a f64) -> &mut Self {
        self.hc = Some(hc);
        self
    }

    pub fn a_vqs0(&mut self, a_vqs0: &'a f64) -> &mut Self {
        self.a_vqs0 = Some(a_vqs0);
        self
    }
}

#[derive(Clone, Debug)]
pub struct Shchepetkin2005Opts<'a> {
    pub etal: &'a f64,
    pub a_vqs0: &'a f64,
    pub theta_s: &'a f64,
    pub theta_b: &'a f64,
    pub hc: &'a f64,
}

impl<'a> Shchepetkin2005Opts<'a> {
    pub fn new(
        etal: &'a f64,
        a_vqs0: &'a f64,
        theta_s: &'a f64,
        theta_b: &'a f64,
        hc: &'a f64,
    ) -> Self {
        Self {
            etal,
            a_vqs0,
            theta_s,
            theta_b,
            hc,
        }
    }
}

#[derive(Error, Debug)]
pub enum Shchepetkin2005TransformBuilderError {
    #[error("Uninitialized field on Shchepetkin2005TransformBuilder: {0}")]
    UninitializedFieldError(String),
    #[error(
        "depths and nlevels array must be of the same length. Got lengths {0} and {1} respectively"
    )]
    DepthsAndLevelsSizeMismatch(usize, usize),
    #[error("depths vector must be strictly increasing")]
    InvalidDepths,
    #[error("First level in nlevels must be >= 2")]
    InvalidFirstLevel,
    #[error("nlevels vector must be non-decreasing")]
    InvalidNLevels,
    #[error("Last depth provided was {0} but it must be greater or equal than {1} which is the deepest point in hgrid.")]
    InvalidLastDepth(f64, f64),
    #[error("a_vqs0 must be in [-1, 1], but got {0}")]
    InvalidAVqs0(f64),
    #[error("theta_s must be in [0, 10], but got {0}")]
    InvalidThetaS(f64),
    #[error("theta_b must be in [0, 4], but got {0}")]
    InvalidThetaB(f64),
    #[error("hc (critical depth) must be > 0, but got {0}")]
    InvalidHc(f64),
    #[error("etal must be smaller than the first depth (which is {0}), but got {1}")]
    InvalidEtalValue(f64, f64),
}
