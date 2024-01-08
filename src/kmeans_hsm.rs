use schismrs_mesh::Hgrid;
use thiserror::Error;

pub fn kmeans_hsm(hgrid: &Hgrid, nclusters: u8) -> Result<Vec<f64>, KMeansHSMCreateError> {
    let mut hsm = Vec::<f64>::new();
    Ok(hsm)
}

#[derive(Error, Debug)]
pub enum KMeansHSMCreateError {}
