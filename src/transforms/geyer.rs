// schismrs-vgrid/src/transforms/geyer.rs
//
// Implements ROMS/Rutgers VSTRETCHING=3: R. Geyer stretching function
// for high bottom boundary layer resolution in relatively shallow applications.
//
// See: https://www.myroms.org/wiki/Vertical_S-coordinate

use super::traits::Transform;
use libm::{cosh, log, tanh};
use ndarray::Array2;
use schismrs_hgrid::Hgrid;
use std::f64::NAN;
use thiserror::Error;

/// Hscale constant used in the Geyer stretching function
const HSCALE: f64 = 3.0;

pub struct GeyerTransform {
    zmas: Array2<f64>,
    etal: f64,
    a_vqs0: f64,
    theta_s: f64,
    theta_b: f64,
    hc: f64,
}

impl Transform for GeyerTransform {
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
pub struct GeyerTransformBuilder<'a> {
    hgrid: Option<&'a Hgrid>,
    etal: Option<&'a f64>,
    depths: Option<&'a Vec<f64>>,
    nlevels: Option<&'a Vec<usize>>,
    a_vqs0: Option<&'a f64>,
    theta_s: Option<&'a f64>,
    theta_b: Option<&'a f64>,
    hc: Option<&'a f64>,
}

impl<'a> GeyerTransformBuilder<'a> {
    pub fn build(&self) -> Result<GeyerTransform, GeyerTransformBuilderError> {
        let hgrid = self.hgrid.ok_or_else(|| {
            GeyerTransformBuilderError::UninitializedFieldError("hgrid".to_string())
        })?;
        let depths = self.depths.ok_or_else(|| {
            GeyerTransformBuilderError::UninitializedFieldError("depths".to_string())
        })?;
        Self::validate_depths(hgrid, depths)?;
        let nlevels = self.nlevels.ok_or_else(|| {
            GeyerTransformBuilderError::UninitializedFieldError("nlevels".to_string())
        })?;
        Self::validate_nlevels(nlevels)?;
        Self::validate_depths_and_nlevels(depths, nlevels)?;
        let etal = self.etal.ok_or_else(|| {
            GeyerTransformBuilderError::UninitializedFieldError("etal".to_string())
        })?;
        Self::validate_etal(etal, &depths[0])?;
        let a_vqs0 = self.a_vqs0.ok_or_else(|| {
            GeyerTransformBuilderError::UninitializedFieldError("a_vqs0".to_string())
        })?;
        Self::validate_a_vqs0(a_vqs0)?;
        let theta_s = self.theta_s.ok_or_else(|| {
            GeyerTransformBuilderError::UninitializedFieldError("theta_s".to_string())
        })?;
        Self::validate_theta_s(theta_s)?;
        let theta_b = self.theta_b.ok_or_else(|| {
            GeyerTransformBuilderError::UninitializedFieldError("theta_b".to_string())
        })?;
        Self::validate_theta_b(theta_b)?;
        let hc = self.hc.ok_or_else(|| {
            GeyerTransformBuilderError::UninitializedFieldError("hc".to_string())
        })?;
        Self::validate_hc(hc)?;

        let zmas = Self::build_zmas(depths, nlevels, etal, theta_s, theta_b, hc);
        Ok(GeyerTransform {
            zmas,
            etal: *etal,
            a_vqs0: *a_vqs0,
            theta_s: *theta_s,
            theta_b: *theta_b,
            hc: *hc,
        })
    }

    /// Build the master grid z-coordinates using R. Geyer stretching function.
    ///
    /// This formulation is designed for high bottom boundary layer resolution
    /// in relatively shallow applications.
    ///
    /// In this function:
    /// - theta_s is used as the surface exponent (exp_sur)
    /// - theta_b is used as the bottom exponent (exp_bot)
    ///
    /// Formula:
    /// Cbot = log(cosh(Hscale * (sigma + 1)^exp_bot)) / log(cosh(Hscale)) - 1
    /// Csur = -log(cosh(Hscale * |sigma|^exp_sur)) / log(cosh(Hscale))
    /// Cweight = 0.5 * (1 - tanh(Hscale * (sigma + 0.5)))
    /// Cs = Cweight * Cbot + (1 - Cweight) * Csur
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

        // In Geyer's formulation:
        // exp_sur = theta_s (surface exponent)
        // exp_bot = theta_b (bottom exponent)
        let exp_sur = *theta_s;
        let exp_bot = *theta_b;

        let log_cosh_hscale = log(cosh(HSCALE));

        for (m, &depth) in depths.iter().enumerate() {
            let kb = nlevels[m];
            let kbm1 = kb - 1;
            let ds = 1.0 / (kbm1 as f64);

            // Compute sc_w (sigma at W-points) and Cs_w (stretching function)
            let mut sc_w = vec![0.0_f64; kb];
            let mut cs_w = vec![0.0_f64; kb];

            // Surface boundary
            sc_w[kbm1] = 0.0;
            cs_w[kbm1] = 0.0;

            // Interior and bottom
            for k in (1..kbm1).rev() {
                let cff_w = ds * ((k as f64) - (kbm1 as f64));
                sc_w[k] = cff_w;

                // Bottom stretching
                let cbot = log(cosh(HSCALE * (cff_w + 1.0).powf(exp_bot))) / log_cosh_hscale - 1.0;

                // Surface stretching
                let csur = -log(cosh(HSCALE * cff_w.abs().powf(exp_sur))) / log_cosh_hscale;

                // Weight function for blending
                let cweight = 0.5 * (1.0 - tanh(HSCALE * (cff_w + 0.5)));

                // Weighted combination
                cs_w[k] = cweight * cbot + (1.0 - cweight) * csur;
            }

            // Bottom boundary
            sc_w[0] = -1.0;
            cs_w[0] = -1.0;

            // Convert from ROMS sigma/Cs to z-coordinates
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
    ) -> Result<(), GeyerTransformBuilderError> {
        if depths.len() != nlevels.len() {
            return Err(GeyerTransformBuilderError::DepthsAndLevelsSizeMismatch(
                depths.len(),
                nlevels.len(),
            ));
        }
        Ok(())
    }

    fn validate_a_vqs0(a_vqs0: &f64) -> Result<(), GeyerTransformBuilderError> {
        if *a_vqs0 < -1.0 || *a_vqs0 > 1.0 {
            return Err(GeyerTransformBuilderError::InvalidAVqs0(*a_vqs0));
        }
        Ok(())
    }

    pub fn validate_etal(etal: &f64, depths0: &f64) -> Result<(), GeyerTransformBuilderError> {
        if *etal >= *depths0 {
            return Err(GeyerTransformBuilderError::InvalidEtalValue(*depths0, *etal));
        }
        Ok(())
    }

    fn validate_depths(hgrid: &Hgrid, depths: &Vec<f64>) -> Result<(), GeyerTransformBuilderError> {
        let mut prev_depth = depths[0];
        for &depth in &depths[1..] {
            if depth <= prev_depth {
                return Err(GeyerTransformBuilderError::InvalidDepths);
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
            return Err(GeyerTransformBuilderError::InvalidLastDepth(
                last_depth,
                -min_hgrid_depth,
            ));
        }

        Ok(())
    }

    pub fn validate_theta_s(theta_s: &f64) -> Result<(), GeyerTransformBuilderError> {
        // For Geyer, theta_s is the surface exponent, typically > 0
        if *theta_s < 0.0 || *theta_s > 10.0 {
            return Err(GeyerTransformBuilderError::InvalidThetaS(*theta_s));
        }
        Ok(())
    }

    pub fn validate_theta_b(theta_b: &f64) -> Result<(), GeyerTransformBuilderError> {
        // For Geyer, theta_b is the bottom exponent, typically > 0
        if *theta_b < 0.0 || *theta_b > 10.0 {
            return Err(GeyerTransformBuilderError::InvalidThetaB(*theta_b));
        }
        Ok(())
    }

    pub fn validate_hc(hc: &f64) -> Result<(), GeyerTransformBuilderError> {
        if *hc <= 0.0 {
            return Err(GeyerTransformBuilderError::InvalidHc(*hc));
        }
        Ok(())
    }

    fn validate_nlevels(nlevels: &Vec<usize>) -> Result<(), GeyerTransformBuilderError> {
        let mut prev_nlevel = nlevels[0];
        if prev_nlevel < 2 {
            return Err(GeyerTransformBuilderError::InvalidFirstLevel);
        }
        for &nlevel in &nlevels[1..] {
            if nlevel < prev_nlevel {
                return Err(GeyerTransformBuilderError::InvalidNLevels);
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
pub struct GeyerOpts<'a> {
    pub etal: &'a f64,
    pub a_vqs0: &'a f64,
    pub theta_s: &'a f64,
    pub theta_b: &'a f64,
    pub hc: &'a f64,
}

impl<'a> GeyerOpts<'a> {
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
pub enum GeyerTransformBuilderError {
    #[error("Uninitialized field on GeyerTransformBuilder: {0}")]
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
    #[error("theta_s (surface exponent) must be in [0, 10], but got {0}")]
    InvalidThetaS(f64),
    #[error("theta_b (bottom exponent) must be in [0, 10], but got {0}")]
    InvalidThetaB(f64),
    #[error("hc (critical depth) must be > 0, but got {0}")]
    InvalidHc(f64),
    #[error("etal must be smaller than the first depth (which is {0}), but got {1}")]
    InvalidEtalValue(f64, f64),
}
