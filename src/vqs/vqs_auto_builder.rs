// schishmrs-vgrid/src/vqs/vqs_auto_builder.rs

use super::errors::VQSAutoBuilderError;
use super::vqs::VQS;
use super::vqs_builder::VQSBuilder;
use crate::transforms::StretchingFunction;
use ndarray_stats::QuantileExt;
use schismrs_hgrid::hgrid::Hgrid;

#[derive(Default)]
pub struct VQSAutoBuilder<'a> {
    hgrid: Option<&'a Hgrid>,
    ngrids: Option<&'a usize>,
    stretching: Option<&'a StretchingFunction<'a>>,
    dz_bottom_min: Option<&'a f64>,
    initial_depth: Option<&'a f64>,
    shallow_levels: Option<&'a usize>,
    max_levels: Option<&'a usize>,
}

impl<'a> VQSAutoBuilder<'a> {
    pub fn build(&self) -> Result<VQS, VQSAutoBuilderError> {
        let hgrid = self
            .hgrid
            .ok_or_else(|| VQSAutoBuilderError::UninitializedFieldError("hgrid".to_string()))?;
        let stretching = self.stretching.ok_or_else(|| {
            VQSAutoBuilderError::UninitializedFieldError("stretching".to_string())
        })?;
        let ngrids = self
            .ngrids
            .ok_or_else(|| VQSAutoBuilderError::UninitializedFieldError("ngrids".to_string()))?;
        Self::validate_ngrids(ngrids)?;

        let dz_bottom_min = match self.dz_bottom_min {
            Some(value) => value.clone(),
            None => {
                // Get the largest negative value from hgrid.depths()
                let depths_array = hgrid.depths();
                match depths_array
                    .iter()
                    .filter(|&&d| d < 0.0)
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                {
                    Some(&max_negative) => -max_negative,
                    None => 0.0, // Default if no negative values exist
                }
            }
        };
        VQSBuilder::validate_dz_bottom_min(&dz_bottom_min)?;
        let initial_depth = self.initial_depth.ok_or_else(|| {
            VQSAutoBuilderError::UninitializedFieldError("initial_depth".to_string())
        })?;
        Self::validate_initial_depth(initial_depth, stretching.etal())?;
        let shallow_levels = self.shallow_levels.ok_or_else(|| {
            VQSAutoBuilderError::UninitializedFieldError("shallow_levels".to_string())
        })?;
        Self::validate_shallow_levels(shallow_levels)?;
        let max_levels = match self.max_levels {
            Some(max_levels) => *max_levels,
            None => Self::calculate_max_levels(shallow_levels, ngrids),
        };
        Self::validate_max_levels(shallow_levels, &max_levels)?;

        let (hsm, nlevels) =
            Self::build_hsm_and_nlevels(hgrid, ngrids, initial_depth, shallow_levels, &max_levels)?;
        Ok(VQSBuilder::default()
            .hgrid(&hgrid)
            .depths(&hsm)
            .nlevels(&nlevels)
            .stretching(&stretching)
            .dz_bottom_min(&dz_bottom_min)
            .build()?)
    }

    fn validate_ngrids(ngrids: &usize) -> Result<(), VQSAutoBuilderError> {
        if *ngrids < 2 {
            return Err(VQSAutoBuilderError::InvalidNgridsValue(*ngrids));
        }
        Ok(())
    }

    fn validate_shallow_levels(shallow_levels: &usize) -> Result<(), VQSAutoBuilderError> {
        if *shallow_levels < 2 {
            return Err(VQSAutoBuilderError::InvalidShallowLevels(*shallow_levels));
        }
        Ok(())
    }
    
    fn validate_max_levels(
        shallow_levels: &usize,
        max_levels: &usize,
    ) -> Result<(), VQSAutoBuilderError> {
        if *max_levels < *shallow_levels {
            return Err(VQSAutoBuilderError::InvalidMaxLevels(
                *shallow_levels,
                *max_levels,
            ));
        }
        Ok(())
    }

    fn calculate_max_levels(shallow_levels: &usize, clusters: &usize) -> usize {
        shallow_levels + clusters - 1
    }

    fn exponential_samples(start: f64, end: f64, steps: usize) -> Vec<f64> {
        let mut samples = Vec::with_capacity(steps);
        let scale = (end / start).powf(1.0 / (steps as f64 - 1.0));

        for i in 0..steps {
            samples.push(start * scale.powf(i as f64));
        }

        samples
    }

    fn build_hsm_and_nlevels(
        hgrid: &Hgrid,
        ngrids: &'a usize,
        initial_depth: &'a f64,
        shallow_levels: &usize,
        max_levels: &usize,
    ) -> Result<(Vec<f64>, Vec<usize>), VQSAutoBuilderError> {
        let max_depth = -hgrid.depths().min()?;
        let x1 = *shallow_levels as f64;
        let y1 = *initial_depth;
        let x2 = *max_levels as f64;
        let y2 = max_depth;
        let b = (y2 / y1).powf(1.0 / (x2 - x1));
        let a = y1 / b.powf(x1);
        let exp_function = |depth: f64| -> f64 { (depth / a).log(b) };
        let mut samples = Self::exponential_samples(*initial_depth, max_depth.clone(), *ngrids);
        samples[0] = *initial_depth;
        samples[*ngrids - 1] = max_depth;
        let mut hsm = Vec::new();
        let mut levels = Vec::new();
        for this_depth in samples.iter() {
            let mut level = exp_function(*this_depth).round() as usize;
            if level < *shallow_levels {
                level = *shallow_levels;
            }
            hsm.push(*this_depth);
            levels.push(level);
        }
        Ok((hsm, levels))
    }

    fn validate_initial_depth(
        initial_depth: &'a f64,
        etal: &'a f64,
    ) -> Result<(), VQSAutoBuilderError> {
        if *etal >= *initial_depth {
            return Err(VQSAutoBuilderError::InvalidInitialDepth(
                *initial_depth,
                *etal,
            ));
        }
        Ok(())
    }

    pub fn hgrid(&mut self, hgrid: &'a Hgrid) -> &mut Self {
        self.hgrid = Some(hgrid);
        self
    }
    
    pub fn ngrids(&mut self, ngrids: &'a usize) -> &mut Self {
        self.ngrids = Some(ngrids);
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
    
    pub fn initial_depth(&mut self, initial_depth: &'a f64) -> &mut Self {
        self.initial_depth = Some(initial_depth);
        self
    }
    
    pub fn shallow_levels(&mut self, shallow_levels: &'a usize) -> &mut Self {
        self.shallow_levels = Some(shallow_levels);
        self
    }
    
    pub fn max_levels(&mut self, max_levels: &'a usize) -> &mut Self {
        self.max_levels = Some(max_levels);
        self
    }
}