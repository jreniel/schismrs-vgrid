// schishmrs-vgrid/src/vqs/vqs_kmeans_builder.rs

use super::errors::VQSKMeansBuilderError;
use super::vqs::VQS;
use super::vqs_builder::VQSBuilder;
use crate::transforms::StretchingFunction;
use crate::{kmeans_hsm, KMeansHSMCreateError};
use ndarray::Array;
use schismrs_hgrid::hgrid::Hgrid;

#[derive(Default)]
pub struct VQSKMeansBuilder<'a> {
    hgrid: Option<&'a Hgrid>,
    nclusters: Option<&'a usize>,
    stretching: Option<&'a StretchingFunction<'a>>,
    etal: Option<&'a f64>,
    shallow_levels: Option<&'a usize>,
    dz_bottom_min: Option<&'a f64>,
    max_levels: Option<&'a usize>,
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
        
        let max_levels = match self.max_levels {
            Some(max_levels) => *max_levels,
            None => Self::calculate_max_levels(shallow_levels, nclusters),
        };
        Self::validate_max_levels(shallow_levels, &max_levels)?;

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
        let mut hsm = kmeans_hsm(hgrid, nclusters, etal)?;
        hsm.iter_mut().for_each(|depth| *depth = depth.abs());
        let mut nlevels = Vec::<usize>::with_capacity(*nclusters);
        let levels = Array::linspace(*shallow_levels as f64, max_levels as f64, *nclusters);
        for level in levels.iter() {
            let mut level = level.round() as usize;
            if level < *shallow_levels {
                level = *shallow_levels;
            }
            nlevels.push(level);
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
    
    pub fn max_levels(&mut self, max_levels: &'a usize) -> &mut Self {
        self.max_levels = Some(max_levels);
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

    fn calculate_max_levels(shallow_levels: &usize, clusters: &usize) -> usize {
        shallow_levels + clusters - 1
    }
    
    fn validate_max_levels(
        shallow_levels: &usize,
        max_levels: &usize,
    ) -> Result<(), VQSKMeansBuilderError> {
        if *shallow_levels > *max_levels {
            return Err(VQSKMeansBuilderError::InvalidMaxLevels(
                *shallow_levels,
                *max_levels,
            ));
        }
        Ok(())
    }
}