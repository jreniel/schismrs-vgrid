//! Stretching function calculations for layer distribution preview
//!
//! Computes actual z-coordinates and layer thicknesses based on
//! S-transform or Quadratic stretching functions.

use libm::{sinh, tanh};

/// Parameters for stretching functions
#[derive(Clone, Debug)]
pub struct StretchingParams {
    /// Surface/bottom focusing parameter (0, 20]
    pub theta_f: f64,
    /// Bottom layer focusing [0, 1] - 0=surface, 1=bottom
    pub theta_b: f64,
    /// Stretching amplitude [-1, 1] - negative=bottom, positive=surface
    pub a_vqs0: f64,
    /// Water elevation (usually 0)
    pub etal: f64,
}

impl Default for StretchingParams {
    fn default() -> Self {
        Self {
            theta_f: 3.0,
            theta_b: 0.5,
            a_vqs0: -1.0,
            etal: 0.0,
        }
    }
}

/// Statistics for a zone between anchors
#[derive(Clone, Debug)]
pub struct ZoneStats {
    /// Zone name/description
    pub name: String,
    /// Depth range start
    pub depth_start: f64,
    /// Depth range end
    pub depth_end: f64,
    /// Number of levels in this zone
    pub num_levels: usize,
    /// Minimum layer thickness in zone
    pub min_dz: f64,
    /// Maximum layer thickness in zone
    pub max_dz: f64,
    /// Average layer thickness in zone
    pub avg_dz: f64,
    /// Layer thicknesses from top to bottom
    pub layer_thicknesses: Vec<f64>,
}

/// Compute z-coordinates for a master grid using S-transform
pub fn compute_s_transform_z(
    depth: f64,
    nlevels: usize,
    params: &StretchingParams,
    first_depth: f64,
) -> Vec<f64> {
    let mut z_coords = Vec::with_capacity(nlevels);

    for k in 0..nlevels {
        let sigma = (k as f64) / (1.0 - nlevels as f64);

        let cs = (1.0 - params.theta_b) * sinh(params.theta_f * sigma) / sinh(params.theta_f)
            + params.theta_b
                * (tanh(params.theta_f * (sigma + 0.5)) - tanh(params.theta_f * 0.5))
                / (2.0 * tanh(params.theta_f * 0.5));

        let z = params.etal * (1.0 + sigma) + first_depth * sigma + (depth - first_depth) * cs;
        z_coords.push(z);
    }

    z_coords
}

/// Compute z-coordinates using quadratic transform (simplified)
pub fn compute_quadratic_z(depth: f64, nlevels: usize, params: &StretchingParams) -> Vec<f64> {
    let mut z_coords = Vec::with_capacity(nlevels);

    for k in 0..nlevels {
        let sigma = (k as f64) / (1.0 - nlevels as f64);

        // Quadratic stretching: more uniform distribution
        // a_vqs0 controls skew: -1=bottom focus, 0=uniform, 1=surface focus
        let cs = if params.a_vqs0.abs() < 0.001 {
            sigma // Uniform
        } else {
            let a = params.a_vqs0;
            // Quadratic formula for stretching
            sigma + a * sigma * (1.0 + sigma)
        };

        let z = params.etal + cs * (depth + params.etal);
        z_coords.push(z);
    }

    z_coords
}

/// Compute layer thicknesses from z-coordinates
pub fn compute_layer_thicknesses(z_coords: &[f64]) -> Vec<f64> {
    if z_coords.len() < 2 {
        return vec![];
    }

    let mut thicknesses = Vec::with_capacity(z_coords.len() - 1);
    for i in 1..z_coords.len() {
        // z increases downward (more negative), so thickness is z[i-1] - z[i]
        let dz = (z_coords[i - 1] - z_coords[i]).abs();
        thicknesses.push(dz);
    }
    thicknesses
}

/// Compute statistics for all zones in a path
pub fn compute_path_zone_stats(
    depths: &[f64],
    nlevels: &[usize],
    params: &StretchingParams,
    use_s_transform: bool,
) -> Vec<ZoneStats> {
    if depths.is_empty() || nlevels.is_empty() {
        return vec![];
    }

    let first_depth = depths[0];
    let mut zones = Vec::new();

    for (i, (&depth, &nlev)) in depths.iter().zip(nlevels.iter()).enumerate() {
        // Compute z-coordinates for this anchor
        let z_coords = if use_s_transform {
            compute_s_transform_z(depth, nlev, params, first_depth)
        } else {
            compute_quadratic_z(depth, nlev, params)
        };

        let thicknesses = compute_layer_thicknesses(&z_coords);

        if thicknesses.is_empty() {
            continue;
        }

        let min_dz = thicknesses.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_dz = thicknesses.iter().cloned().fold(0.0, f64::max);
        let avg_dz = thicknesses.iter().sum::<f64>() / thicknesses.len() as f64;

        let depth_start = if i == 0 { 0.0 } else { depths[i - 1] };

        zones.push(ZoneStats {
            name: format!("Anchor {}", i + 1),
            depth_start,
            depth_end: depth,
            num_levels: nlev,
            min_dz,
            max_dz,
            avg_dz,
            layer_thicknesses: thicknesses,
        });
    }

    zones
}

/// Generate a simple ASCII visualization of layer distribution
pub fn layer_distribution_ascii(thicknesses: &[f64], width: usize) -> Vec<String> {
    if thicknesses.is_empty() {
        return vec![];
    }

    let max_dz = thicknesses.iter().cloned().fold(0.0, f64::max);
    let mut lines = Vec::new();

    for (i, &dz) in thicknesses.iter().enumerate() {
        let bar_len = ((dz / max_dz) * (width as f64 - 10.0)) as usize;
        let bar = "█".repeat(bar_len.max(1));
        lines.push(format!("{:2} {:5.2}m {}", i + 1, dz, bar));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s_transform_basic() {
        let params = StretchingParams::default();
        let z = compute_s_transform_z(10.0, 5, &params, 10.0);

        assert_eq!(z.len(), 5);
        // First z should be at surface (etal)
        assert!((z[0] - params.etal).abs() < 0.01);
        // Last z should be near -depth
        assert!((z[4] + 10.0).abs() < 1.0);
    }

    #[test]
    fn test_layer_thicknesses() {
        let z = vec![0.0, -2.0, -5.0, -10.0];
        let dz = compute_layer_thicknesses(&z);

        assert_eq!(dz.len(), 3);
        assert!((dz[0] - 2.0).abs() < 0.01);
        assert!((dz[1] - 3.0).abs() < 0.01);
        assert!((dz[2] - 5.0).abs() < 0.01);
    }
}
