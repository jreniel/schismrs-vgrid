use humantime::format_duration;
use linfa::traits::{Fit, Predict};
use linfa::DatasetBase;
use linfa_clustering::{KMeans, KMeansError};
use log;
use ndarray::{s, Array1, Axis, ShapeError};
use schismrs_mesh::Hgrid;
use std::cmp::Ordering;
use std::time::Instant;
use thiserror::Error;

pub fn kmeans_hsm(hgrid: &Hgrid, nclusters: usize) -> Result<Vec<f64>, KMeansHSMCreateError> {
    log::info!(
        "Begin computing vertical distribution with nclusters={}",
        nclusters
    );
    let now = Instant::now();
    let mut depths: Vec<f64> = hgrid.depths().into_iter().collect();
    depths.sort_by(|a, b| a.partial_cmp(b).unwrap());
    depths.dedup();
    // keep only the underwater numbers.
    depths.retain(|&x| x <= 0.0);
    let depths = Array1::from(depths);
    let depth_len = depths.len();
    let observations = DatasetBase::from(depths.clone().into_shape((depth_len, 1))?);
    let model = KMeans::params(nclusters).fit(&observations)?;
    let predictions = model.predict(observations);
    let targets = predictions.targets();
    let centroids = model.centroids().to_owned();
    let mut min_depths = Vec::with_capacity(centroids.len());
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

        min_depths.push(min_depth);
    }

    let mut centroids: Vec<_> = centroids.into_iter().collect();
    centroids.sort_by(|a, b| b.partial_cmp(a).unwrap());

    let mut min_depths: Vec<_> = min_depths;
    min_depths.sort_by(|a, b| b.partial_cmp(a).unwrap());

    assert_eq!(centroids.len(), min_depths.len());
    centroids.iter().zip(min_depths.iter()).for_each(|(c, d)| {
        println!("Centroid: {:?}, Min Depth: {:?}", c, d);
    });

    // let centroid_indices = Array1::from_iter(0..nclusters);
    // let min_depths = centroid_indices
    //     .map(|centroid_idx| {
    //         let indexes = targets.eq(centroid_idx);
    //         let mut min = f64::MAX;
    //         indexes.into_iter().for_each(|i| {
    //             let depth = observations.records()[i][0];
    //             min = depth.min(min);
    //         });
    //         min
    //     })
    //     .collect();
    // dbg!(&min_depths);

    //     let closest_centroids = observations
    //         .targets()
    //         .into_iter()
    //         .map(|idx| centroids.index_axis(Axis(0), *idx))
    //         .collect::<Array1<_>>();
    //     let closest_centroids: Vec<f64> = closest_centroids
    //         .into_iter()
    //         .map(|centroid| centroid[0])
    //         .collect();

    //     dbg!(&closest_centroids);
    // let mut centroids: Vec<f64> = model.centroids().to_owned().into_iter().collect();
    // centroids.sort_by(|a, b| b.partial_cmp(a).unwrap());
    // for centroid in &centroids {
    //     if let Some(min_depth) = depths
    //         .iter()
    //         .zip(centroids.iter())
    //         .filter(|(_, &c)| c == *centroid)
    //         .map(|(&d, _)| d)
    //         .min_by(|&a, &b| a.partial_cmp(&b).unwrap())
    //     {
    //         println!("Centroid: {}, Minimum Depth: {}", centroid, min_depth);
    //     } else {
    //         println!("No depth found for centroid: {}", centroid);
    //     }
    // }
    log::debug!(
        "Took {} to compute vertical distribution.",
        format_duration(now.elapsed())
    );
    unimplemented!();
    let hsm = Vec::<f64>::new();
    Ok(hsm)
}

#[derive(Error, Debug)]
pub enum KMeansHSMCreateError {
    #[error(transparent)]
    NDArrayShapeError(#[from] ShapeError),
    #[error(transparent)]
    KMeansError(#[from] KMeansError),
}
