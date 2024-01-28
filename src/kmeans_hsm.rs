use humantime::format_duration;
use linfa::traits::{Fit, Predict};
use linfa::DatasetBase;
use linfa_clustering::{KMeans, KMeansError};
use log;
use ndarray::{Array1, ShapeError};
use schismrs_hgrid::Hgrid;
use std::cmp::Ordering;
use std::time::Instant;
use thiserror::Error;

pub fn kmeans_hsm(
    hgrid: &Hgrid,
    nclusters: &usize,
    etal: &f64,
) -> Result<Vec<f64>, KMeansHSMCreateError> {
    log::info!(
        "Begin computing vertical distribution with nclusters={}",
        nclusters
    );
    let now = Instant::now();
    let mut depths: Vec<f64> = hgrid.depths().into_iter().collect();
    depths.sort_by(|a, b| a.partial_cmp(b).unwrap());
    depths.dedup();
    // keep only the underwater numbers.
    depths.retain(|&x| x <= *etal);
    let depths = Array1::from(depths);
    let depth_len = depths.len();
    let observations = DatasetBase::from(depths.clone().into_shape((depth_len, 1))?);
    let model = KMeans::params(*nclusters).fit(&observations)?;
    let predictions = model.predict(observations);
    let targets = predictions.targets();
    let centroids = model.centroids().to_owned();
    let mut hsm = Vec::with_capacity(centroids.len());
    // find the minimum depth associated to each computed centroid
    for (index, _) in centroids.iter().enumerate() {
        let mut min_depth = f64::INFINITY;

        for (&depth, &cluster) in depths.iter().zip(targets.iter()) {
            if cluster as usize == index {
                match depth.partial_cmp(&min_depth) {
                    Some(Ordering::Less) => min_depth = depth,
                    _ => (),
                }
            }
        }
        hsm.push(min_depth);
    }
    hsm.sort_by(|a, b| b.partial_cmp(a).unwrap());
    log::debug!(
        "Took {} to compute vertical distribution.",
        format_duration(now.elapsed())
    );
    Ok(hsm)
}

#[derive(Error, Debug)]
pub enum KMeansHSMCreateError {
    #[error(transparent)]
    NDArrayShapeError(#[from] ShapeError),
    #[error(transparent)]
    KMeansError(#[from] KMeansError),
}
