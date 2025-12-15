// schismrs-vgrid/src/vqs/errors.rs

use crate::transforms::quadratic::QuadraticTransformBuilderError;
use crate::transforms::s::STransformBuilderError;
use crate::transforms::transforms::StretchingFunctionError;
use ndarray::Array1;
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
    #[error("{0}")]
    InvalidBottomIndices(String),
}

// Error types for reconstruction and loading
#[derive(Error, Debug)]
pub enum ReconstructionError {
    #[error("Insufficient data: only {0} wet nodes found (minimum: 10)")]
    InsufficientData(usize),
    #[error("Clustering failed: {0}")]
    ClusteringFailed(String),
    #[error("No valid master grids could be extracted")]
    NoValidMasterGrids,
    #[error("Depth-level relationship is not monotonic")]
    NonMonotonicRelationship,
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("CSV error: {0}")]
    CsvError(#[from] csv::Error),
}

#[derive(Error, Debug)]
pub enum VQSLoadError {
    #[error("File IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Invalid vgrid.in format: {0}")]
    InvalidFormat(String),
    #[error("Unsupported ivcor value: {0} (only ivcor=1 is supported)")]
    UnsupportedIvcor(i32),
    #[error("Inconsistent dimensions: nvrt={0}, but found {1} levels")]
    InconsistentDimensions(usize, usize),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Node count mismatch: hgrid has {0} nodes but vgrid has {1}")]
    NodeCountMismatch(usize, usize),
}
