use super::traits::BuildVQS;
use crate::{kmeans_hsm, KMeansHSMCreateError};
use schismrs_mesh::Hgrid;
use thiserror::Error;

pub struct QuadraticTransform {
    etal: f64,
    hsm: Vec<f64>,
    a_vqs: Vec<f64>,
}

impl BuildVQS for QuadraticTransform {
    fn nvrt(&self) -> u8 {
        // a_vqs(m)=max(-1.d0,a_vqs0-(m-1)*0.03)
        // tmp=a_vqs(m)*sigma*sigma+(1+a_vqs(m))*sigma !transformed sigma
        // z_mas(k,m)=tmp*(etal+hsm(m))+etal
        unimplemented!()
    }

    fn values_at_level(&self, level: u8) -> Vec<f64> {
        unimplemented!()
    }

    fn iter_level_values(&self) -> super::traits::IterLevelValues {
        unimplemented!()
    }

    fn bottom_level_indices(&self) -> Vec<u8> {
        unimplemented!()
    }
}

#[derive(Clone)]
pub struct QuadraticTransformOpts {
    pub etal: Option<f64>,
}

pub struct QuadraticTransformKMeansBuilder<'a> {
    hgrid: Option<&'a Hgrid>,
    etal: Option<f64>,
    nclusters: Option<u8>,
}

impl<'a> QuadraticTransformKMeansBuilder<'a> {
    pub fn build(&self) -> Result<QuadraticTransform, QuadraticTransformKMeansBuilderError> {
        let etal = self.etal.unwrap();
        let hgrid = self.hgrid.ok_or_else(|| {
            QuadraticTransformKMeansBuilderError::UninitializedFieldError("hgrid".to_string())
        })?;
        let nclusters = self.nclusters.ok_or_else(|| {
            QuadraticTransformKMeansBuilderError::UninitializedFieldError("nclusters".to_string())
        })?;
        let hsm = kmeans_hsm(hgrid, nclusters)?;
        Ok(QuadraticTransform { etal, hsm, a_vqs })
    }
    pub fn hgrid(&mut self, hgrid: &'a Hgrid) -> &mut Self {
        self.hgrid = Some(hgrid);
        self
    }
    pub fn nclusters(&mut self, nclusters: u8) -> &mut Self {
        self.nclusters = Some(nclusters);
        self
    }
    pub fn etal(&mut self, etal: f64) -> &mut Self {
        self.etal = Some(etal);
        self
    }
}

impl<'a> Default for QuadraticTransformKMeansBuilder<'a> {
    fn default() -> Self {
        Self {
            etal: Some(0.),
            hgrid: None,
            nclusters: None,
        }
    }
}
#[derive(Error, Debug)]
pub enum QuadraticTransformKMeansBuilderError {
    #[error("Unitialized field on QuadraticTransformKMeansBuilder: {0}")]
    UninitializedFieldError(String),
    // #[error("Unitialized field on VQSKMeansBuilder: {0}")]
    // UninitializedFieldError(String),
    #[error(transparent)]
    KMeansHSMCreateError(#[from] KMeansHSMCreateError),
}
