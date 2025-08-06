// schishmrs-vgrid/src/vqs/errors.rs

use crate::transforms::quadratic::QuadraticTransformBuilderError;
use crate::transforms::s::STransformBuilderError;
use crate::transforms::transforms::StretchingFunctionError;
use crate::KMeansHSMCreateError;
use ndarray::Array1;
use ndarray_stats::errors::MinMaxError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VQSBuilderError {
    #[error("Unitialized field on VQSBuilder: {0}")]
    UninitializedFieldError(String),
    #[error(transparent)]
    QuadraticTransformBuilderError(#[from] QuadraticTransformBuilderError),
    #[error(transparent)]
    STransformBuilderError(#[from] STransformBuilderError),
    #[error("dz_bottom_min must be > 0")]
    InvalidDzBottomMin,
    #[error("Failed to find a master vgrid for node id: {0} and depth {1}")]
    FailedToFindAMasterVgrid(usize, f64),
    #[error("Failed to find a bottom for node id: {0}, depth {1}, z3={2}, z_mas={3}")]
    FailedToFindABottom(usize, f64, f64, Array1<f64>),
    #[error("Inverted Z for node id: {0}, depth {1}, m0[i]={2}, k={3}, znd[[k-1, i]]={4}, znd[[k, i]]={5}")]
    InvertedZ(usize, f64, usize, usize, f64, f64),
    #[error(transparent)]
    StretchingFunctionError(#[from] StretchingFunctionError),
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
    #[error("max_levels must be > shallow_levels but got max_levels={1}, shallow_levels={0}")]
    InvalidMaxLevels(usize, usize),
}

#[derive(Error, Debug)]
pub enum VQSAutoBuilderError {
    #[error("Unitialized field on VQSAutoBuilder: {0}")]
    UninitializedFieldError(String),
    #[error(transparent)]
    VQSBuilderError(#[from] VQSBuilderError),
    #[error("shallow_levels must be >= 2 but got {0}")]
    InvalidShallowLevels(usize),
    #[error("max_levels must be > shallow_levels but got shallow_levels={0} and max_levels={1}")]
    InvalidMaxLevels(usize, usize),
    #[error("initial_depth must be > than etal, but got initial_depth={0} and etal={1}")]
    InvalidInitialDepth(f64, f64),
    #[error("ngrids must be >= 2 but got {0}")]
    InvalidNgridsValue(usize),
    #[error(transparent)]
    MinMaxError(#[from] MinMaxError),
    #[error(transparent)]
    STransformBuilderError(#[from] STransformBuilderError),
    #[error(transparent)]
    QuadraticTransformBuilderError(#[from] QuadraticTransformBuilderError),
}