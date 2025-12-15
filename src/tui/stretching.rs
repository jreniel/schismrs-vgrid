//! Stretching function calculations for layer distribution preview
//!
//! Computes actual z-coordinates and layer thicknesses based on
//! various stretching functions: S-transform, Quadratic, and ROMS variants.

use libm::{cosh, exp, log, sinh, tanh};
use rayon::prelude::*;

/// Parameters for stretching functions
#[derive(Clone, Debug)]
pub struct StretchingParams {
    /// S-transform: Surface/bottom focusing parameter (0, 20]
    pub theta_f: f64,
    /// S-transform/ROMS: Bottom layer focusing [0, 1] for S, [0, 4] for ROMS
    pub theta_b: f64,
    /// Stretching amplitude [-1, 1] - negative=bottom, positive=surface
    pub a_vqs0: f64,
    /// Water elevation (usually 0)
    pub etal: f64,
    /// ROMS: Surface stretching parameter [0, 10]
    pub theta_s: f64,
    /// ROMS: Critical depth in meters (>0) - controls stretching transition width
    pub hc: f64,
}

impl Default for StretchingParams {
    fn default() -> Self {
        Self {
            theta_f: 3.0,
            theta_b: 0.5,
            a_vqs0: -1.0,
            etal: 0.0,
            theta_s: 5.0,
            hc: 5.0,
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

/// Compute z-coordinates using Shchepetkin (2005) UCLA-ROMS stretching
///
/// Reference: Shchepetkin, A.F. and J.C. McWilliams, 2005
pub fn compute_shchepetkin2005_z(depth: f64, nlevels: usize, params: &StretchingParams) -> Vec<f64> {
    let mut z_coords = Vec::with_capacity(nlevels);
    let kb = nlevels;
    let kbm1 = kb - 1;
    let ds = 1.0 / (kbm1 as f64);
    let aweight = 1.0_f64;
    let bweight = 1.0_f64;

    // Compute sigma and Cs at each level
    let mut sc_w = vec![0.0_f64; kb];
    let mut cs_w = vec![0.0_f64; kb];

    sc_w[kbm1] = 0.0;
    cs_w[kbm1] = 0.0;

    for k in (1..kbm1).rev() {
        let cff_w = ds * ((k as f64) - (kbm1 as f64));
        sc_w[k] = cff_w;

        if params.theta_s > 0.0 {
            let csur = (1.0 - cosh(params.theta_s * cff_w)) / (cosh(params.theta_s) - 1.0);

            if params.theta_b > 0.0 {
                let cbot = sinh(params.theta_b * (cff_w + 1.0)) / sinh(params.theta_b) - 1.0;
                let sigma_plus_1 = cff_w + 1.0;
                let cweight = sigma_plus_1.powf(aweight)
                    * (1.0 + (aweight / bweight) * (1.0 - sigma_plus_1.powf(bweight)));
                cs_w[k] = cweight * csur + (1.0 - cweight) * cbot;
            } else {
                cs_w[k] = csur;
            }
        } else {
            cs_w[k] = cff_w;
        }
    }

    sc_w[0] = -1.0;
    cs_w[0] = -1.0;

    // Convert to z-coordinates (flip indexing for SCHISM convention)
    let h = depth;
    let hinv = 1.0 / (params.hc + h);

    for k in 0..kb {
        let roms_k = kbm1 - k;
        let cff2_w = (params.hc * sc_w[roms_k] + cs_w[roms_k] * h) * hinv;
        z_coords.push(cff2_w * h);
    }

    z_coords
}

/// Compute z-coordinates using Shchepetkin (2010) UCLA-ROMS double stretching
pub fn compute_shchepetkin2010_z(depth: f64, nlevels: usize, params: &StretchingParams) -> Vec<f64> {
    let mut z_coords = Vec::with_capacity(nlevels);
    let kb = nlevels;
    let kbm1 = kb - 1;
    let ds = 1.0 / (kbm1 as f64);

    let mut sc_w = vec![0.0_f64; kb];
    let mut cs_w = vec![0.0_f64; kb];

    sc_w[kbm1] = 0.0;
    cs_w[kbm1] = 0.0;

    for k in (1..kbm1).rev() {
        let cff_w = ds * ((k as f64) - (kbm1 as f64));
        sc_w[k] = cff_w;

        // Surface stretching
        let csur = if params.theta_s > 0.0 {
            (1.0 - cosh(params.theta_s * cff_w)) / (cosh(params.theta_s) - 1.0)
        } else {
            -cff_w * cff_w
        };

        // Bottom stretching (double stretching)
        if params.theta_b > 0.0 {
            let cbot = (exp(params.theta_b * csur) - 1.0) / (1.0 - exp(-params.theta_b));
            cs_w[k] = cbot;
        } else {
            cs_w[k] = csur;
        }
    }

    sc_w[0] = -1.0;
    cs_w[0] = -1.0;

    let h = depth;
    let hinv = 1.0 / (params.hc + h);

    for k in 0..kb {
        let roms_k = kbm1 - k;
        let cff2_w = (params.hc * sc_w[roms_k] + cs_w[roms_k] * h) * hinv;
        z_coords.push(cff2_w * h);
    }

    z_coords
}

/// Compute z-coordinates using R. Geyer stretching for high bottom boundary layer resolution
///
/// Designed for relatively shallow applications with high bottom resolution needs.
pub fn compute_geyer_z(depth: f64, nlevels: usize, params: &StretchingParams) -> Vec<f64> {
    const HSCALE: f64 = 3.0;

    let mut z_coords = Vec::with_capacity(nlevels);
    let kb = nlevels;
    let kbm1 = kb - 1;
    let ds = 1.0 / (kbm1 as f64);

    // In Geyer's formulation, theta_s is surface exponent, theta_b is bottom exponent
    let exp_sur = params.theta_s;
    let exp_bot = params.theta_b;
    let log_cosh_hscale = log(cosh(HSCALE));

    let mut sc_w = vec![0.0_f64; kb];
    let mut cs_w = vec![0.0_f64; kb];

    sc_w[kbm1] = 0.0;
    cs_w[kbm1] = 0.0;

    for k in (1..kbm1).rev() {
        let cff_w = ds * ((k as f64) - (kbm1 as f64));
        sc_w[k] = cff_w;

        let cbot = log(cosh(HSCALE * (cff_w + 1.0).powf(exp_bot))) / log_cosh_hscale - 1.0;
        let csur = -log(cosh(HSCALE * cff_w.abs().powf(exp_sur))) / log_cosh_hscale;
        let cweight = 0.5 * (1.0 - tanh(HSCALE * (cff_w + 0.5)));
        cs_w[k] = cweight * cbot + (1.0 - cweight) * csur;
    }

    sc_w[0] = -1.0;
    cs_w[0] = -1.0;

    let h = depth;
    let hinv = 1.0 / (params.hc + h);

    for k in 0..kb {
        let roms_k = kbm1 - k;
        let cff2_w = (params.hc * sc_w[roms_k] + cs_w[roms_k] * h) * hinv;
        z_coords.push(cff2_w * h);
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

/// Result of applying bottom truncation to z-coordinates
#[derive(Clone, Debug)]
pub struct TruncationResult {
    /// Z-coordinates after truncation (may have fewer levels)
    pub z_coords: Vec<f64>,
    /// Actual number of levels after truncation
    pub actual_levels: usize,
    /// Whether truncation occurred
    pub was_truncated: bool,
}

/// Apply bottom layer truncation to z-coordinates
///
/// This mimics the logic in vqs_builder.rs where layers are stopped
/// when the bottom layer would be thinner than dz_bottom_min.
///
/// # Arguments
/// * `z_coords` - Z-coordinates from stretching function (surface to bottom)
/// * `depth` - Total water depth at this point
/// * `dz_bottom_min` - Minimum allowed bottom layer thickness
///
/// # Returns
/// TruncationResult with truncated z-coordinates and metadata
pub fn apply_bottom_truncation(
    z_coords: &[f64],
    depth: f64,
    dz_bottom_min: f64,
) -> TruncationResult {
    let requested_levels = z_coords.len();

    if z_coords.is_empty() {
        return TruncationResult {
            z_coords: vec![],
            actual_levels: 0,
            was_truncated: false,
        };
    }

    // Threshold: stop when z would go below -depth + dz_bottom_min
    let threshold = -depth + dz_bottom_min;

    let mut truncated: Vec<f64> = Vec::with_capacity(z_coords.len());

    for &z in z_coords {
        if z >= threshold {
            truncated.push(z);
        } else {
            // We've hit the bottom threshold
            break;
        }
    }

    // Ensure at least 2 levels (surface and bottom)
    if truncated.len() < 2 && z_coords.len() >= 2 {
        truncated = vec![z_coords[0], -depth];
    }

    let actual_levels = truncated.len();
    let was_truncated = actual_levels < requested_levels;

    TruncationResult {
        z_coords: truncated,
        actual_levels,
        was_truncated,
    }
}

/// Stretching type for compute functions (mirrors app::StretchingType)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StretchingKind {
    Quadratic,
    S,
    Shchepetkin2005,
    Shchepetkin2010,
    Geyer,
}

/// Compute z-coordinates for a given stretching type
pub fn compute_z_for_stretching(
    depth: f64,
    nlevels: usize,
    params: &StretchingParams,
    first_depth: f64,
    stretching: StretchingKind,
) -> Vec<f64> {
    match stretching {
        StretchingKind::S => compute_s_transform_z(depth, nlevels, params, first_depth),
        StretchingKind::Quadratic => compute_quadratic_z(depth, nlevels, params),
        StretchingKind::Shchepetkin2005 => compute_shchepetkin2005_z(depth, nlevels, params),
        StretchingKind::Shchepetkin2010 => compute_shchepetkin2010_z(depth, nlevels, params),
        StretchingKind::Geyer => compute_geyer_z(depth, nlevels, params),
    }
}

/// Compute z-coordinates with bottom truncation applied
///
/// Combines stretching calculation with truncation in one call.
pub fn compute_z_with_truncation(
    depth: f64,
    nlevels: usize,
    params: &StretchingParams,
    first_depth: f64,
    dz_bottom_min: f64,
    stretching: StretchingKind,
) -> TruncationResult {
    let z_coords = compute_z_for_stretching(depth, nlevels, params, first_depth, stretching);
    apply_bottom_truncation(&z_coords, depth, dz_bottom_min)
}

/// Compute statistics for all zones in a path (at anchor depths only - for display without mesh)
pub fn compute_path_zone_stats(
    depths: &[f64],
    nlevels: &[usize],
    params: &StretchingParams,
    stretching: StretchingKind,
) -> Vec<ZoneStats> {
    if depths.is_empty() || nlevels.is_empty() {
        return vec![];
    }

    let first_depth = depths[0];
    let mut zones = Vec::new();

    for (i, (&depth, &nlev)) in depths.iter().zip(nlevels.iter()).enumerate() {
        // Compute z-coordinates for this anchor
        let z_coords = compute_z_for_stretching(depth, nlev, params, first_depth, stretching);

        let thicknesses = compute_layer_thicknesses(&z_coords);

        if thicknesses.is_empty() {
            continue;
        }

        // Only consider positive thicknesses above epsilon for min_dz
        const DZ_EPSILON: f64 = 1e-6;
        let min_dz = thicknesses.iter().cloned()
            .filter(|&dz| dz > DZ_EPSILON)
            .fold(f64::INFINITY, f64::min);
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

/// Interpolate nlevels for a given depth based on anchor depths and nlevels
fn interpolate_nlevels(node_depth: f64, anchor_depths: &[f64], anchor_nlevels: &[usize]) -> usize {
    if anchor_depths.is_empty() || anchor_nlevels.is_empty() {
        return 2;
    }

    // If shallower than first anchor, use first anchor's nlevels
    if node_depth <= anchor_depths[0] {
        return anchor_nlevels[0];
    }

    // If deeper than last anchor, use last anchor's nlevels
    if node_depth >= *anchor_depths.last().unwrap() {
        return *anchor_nlevels.last().unwrap();
    }

    // Find the zone this depth falls into and interpolate
    for i in 1..anchor_depths.len() {
        if node_depth <= anchor_depths[i] {
            let d0 = anchor_depths[i - 1];
            let d1 = anchor_depths[i];
            let n0 = anchor_nlevels[i - 1] as f64;
            let n1 = anchor_nlevels[i] as f64;

            // Linear interpolation
            let t = (node_depth - d0) / (d1 - d0);
            let interpolated = n0 + t * (n1 - n0);
            return interpolated.round() as usize;
        }
    }

    *anchor_nlevels.last().unwrap()
}

/// Per-node statistics for parallel reduction
#[derive(Clone, Default)]
struct NodeStats {
    min_dz: f64,
    max_dz: f64,
    dz_sum: f64,
    dz_count: usize,
    node_count: usize,
}

impl NodeStats {
    fn new() -> Self {
        Self {
            min_dz: f64::INFINITY,
            max_dz: 0.0,
            dz_sum: 0.0,
            dz_count: 0,
            node_count: 0,
        }
    }

    fn merge(mut self, other: Self) -> Self {
        self.min_dz = self.min_dz.min(other.min_dz);
        self.max_dz = self.max_dz.max(other.max_dz);
        self.dz_sum += other.dz_sum;
        self.dz_count += other.dz_count;
        self.node_count += other.node_count;
        self
    }
}

/// Compute statistics using actual mesh node depths (parallelized with rayon)
///
/// This provides accurate min/avg/max layer thicknesses across real mesh nodes,
/// rather than just at anchor depths.
pub fn compute_mesh_zone_stats(
    anchor_depths: &[f64],
    anchor_nlevels: &[usize],
    mesh_depths: &[f64],
    params: &StretchingParams,
    stretching: StretchingKind,
) -> Vec<ZoneStats> {
    if anchor_depths.is_empty() || anchor_nlevels.is_empty() || mesh_depths.is_empty() {
        return vec![];
    }

    let first_anchor_depth = anchor_depths[0];

    // Process each anchor/zone
    anchor_depths
        .iter()
        .zip(anchor_nlevels.iter())
        .enumerate()
        .map(|(i, (&anchor_depth, &anchor_nlev))| {
            let depth_start = if i == 0 { 0.0 } else { anchor_depths[i - 1] };
            let depth_end = anchor_depth;

            // Parallel computation across all mesh nodes for this zone
            let stats = mesh_depths
                .par_iter()
                .filter(|&&node_depth| {
                    if i == 0 {
                        node_depth > 0.0 && node_depth <= depth_end
                    } else {
                        node_depth > depth_start && node_depth <= depth_end
                    }
                })
                .fold(NodeStats::new, |mut acc, &node_depth| {
                    acc.node_count += 1;

                    // Interpolate nlevels for this node's depth
                    let node_nlevels = interpolate_nlevels(node_depth, anchor_depths, anchor_nlevels);

                    // Compute z-coordinates at this node's actual depth
                    let z_coords = compute_z_for_stretching(
                        node_depth,
                        node_nlevels,
                        params,
                        first_anchor_depth,
                        stretching,
                    );

                    let thicknesses = compute_layer_thicknesses(&z_coords);

                    // Only consider positive thicknesses above epsilon for min_dz
                    const DZ_EPSILON: f64 = 1e-6;
                    for &dz in &thicknesses {
                        if dz > DZ_EPSILON {
                            acc.min_dz = acc.min_dz.min(dz);
                        }
                        acc.max_dz = acc.max_dz.max(dz);
                        acc.dz_sum += dz;
                        acc.dz_count += 1;
                    }

                    acc
                })
                .reduce(NodeStats::new, NodeStats::merge);

            // If no nodes in zone, fall back to anchor-based calculation
            let (min_dz, avg_dz, max_dz, thicknesses) = if stats.dz_count > 0 {
                (
                    stats.min_dz,
                    stats.dz_sum / stats.dz_count as f64,
                    stats.max_dz,
                    vec![], // Don't store all thicknesses for mesh stats
                )
            } else {
                // Fallback: compute at anchor depth
                let z_coords = compute_z_for_stretching(
                    anchor_depth,
                    anchor_nlev,
                    params,
                    first_anchor_depth,
                    stretching,
                );
                let thicknesses = compute_layer_thicknesses(&z_coords);
                // Only consider positive thicknesses above epsilon for min
                const DZ_EPSILON: f64 = 1e-6;
                let min = thicknesses.iter().cloned()
                    .filter(|&dz| dz > DZ_EPSILON)
                    .fold(f64::INFINITY, f64::min);
                let max = thicknesses.iter().cloned().fold(0.0, f64::max);
                let avg = thicknesses.iter().sum::<f64>() / thicknesses.len().max(1) as f64;
                (min, avg, max, thicknesses)
            };

            ZoneStats {
                name: format!("Zone {} ({} nodes)", i + 1, stats.node_count),
                depth_start,
                depth_end,
                num_levels: anchor_nlev,
                min_dz,
                max_dz,
                avg_dz,
                layer_thicknesses: thicknesses,
            }
        })
        .collect()
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
