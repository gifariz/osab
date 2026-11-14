//! P-Delta: a geometric stiffness added to the stiffness of every case (load cases, the modal case and
//! so spectrum cases, combinations). Rules and their probes: docs/phases/P_DELTA_PLAN.md.
//!
//! - **From the mass:** every joint with mass gets a negative spring -m g / z to the ground in UX and
//!   UY, z its height above the base level; no axial force anywhere.
//! - **From loads:** the P-Delta load (load patterns times factors) is solved with P-Delta by
//!   constant-stiffness iterations on the stiffness without P-Delta (then Newton steps from the start,
//!   when those run out or diverge); each frame's mean axial force at the last iterate gives its
//!   consistent geometric stiffness
//!   (`elements::frame::local_stiffness`), which then stays for every case.

use crate::dof::NDF;
use crate::elements::{distributed_part, LocalLoad};
use crate::model::Model;

/// The P-Delta load's iterations stop when the relative unbalance is below this.
pub const TOLERANCE: f64 = 1e-4;
/// Constant-stiffness iterations first, then Newton iterations, at most.
pub const CONSTANT_STIFFNESS: usize = 10;
pub const NEWTON: usize = 40;

/// Whether the constant-stiffness iterations stop, given the relative unbalance of each so far: when
/// they run out, or when the unbalance has grown twice in a row (the first iteration's not compared).
pub fn leave_constant_stiffness(unbalance: &[f64]) -> bool {
    let n = unbalance.len();
    n >= CONSTANT_STIFFNESS
        || (n >= 4 && unbalance[n - 1] > unbalance[n - 2] && unbalance[n - 2] > unbalance[n - 3])
}

/// Newton iterations without a new lowest unbalance after which they are given up: the P-Delta load
/// does not converge (each such iteration is a factorization).
pub const STALLED: usize = 4;

/// Whether the Newton iterations are going nowhere, given the relative unbalance after each so far:
/// none of the last [`STALLED`] is below the lowest before them.
pub fn newton_stalled(unbalance: &[f64]) -> bool {
    let n = unbalance.len();
    if n <= STALLED {
        return false;
    }
    let best = unbalance[..n - STALLED]
        .iter()
        .cloned()
        .fold(f64::INFINITY, f64::min);
    unbalance[n - STALLED..].iter().all(|u| !(*u < best))
}

/// The mass method's springs at every joint: -m g / z in UX and UY (the joint's own mass in each), z the
/// joint's height above the base level (the lowest story level; without stories, the lowest joint).
/// None at or below the base level.
pub fn mass_springs(model: &Model, masses: &[[f64; 6]]) -> Vec<[f64; NDF]> {
    let lowest = model
        .nodes
        .iter()
        .map(|n| n.z)
        .fold(f64::INFINITY, f64::min);
    let base = model.stories.first().map_or(lowest, |s| s.z);
    let top = model
        .nodes
        .iter()
        .map(|n| n.z)
        .fold(f64::NEG_INFINITY, f64::max);
    let tol = 1e-9 * (top - base).abs().max(1.0);
    let g = model.units.gravity();
    model
        .nodes
        .iter()
        .zip(masses)
        .map(|(n, m)| {
            let mut k = [0.0; NDF];
            let z = n.z - base;
            if z > tol {
                k[0] = -m[0] * g / z;
                k[1] = -m[1] * g / z;
            }
            k
        })
        .collect()
}

/// A member's mean axial force over its length (tension positive), from its local end forces (nodes
/// on the member) and its member loads: P(x) = -(N_i + the axial loads between end i and x).
pub fn mean_axial(end: &[f64; 12], loads: &[LocalLoad], l: f64) -> f64 {
    // ∫ (axial loads between 0 and x) dx over the length.
    let mut s = 0.0;
    for ld in loads {
        match *ld {
            LocalLoad::Point { dir: 0, p, at } => s += p * (l - at).max(0.0),
            LocalLoad::Distributed {
                dir: 0,
                w1,
                w2,
                from,
                to,
            } => {
                let (f, m1) = distributed_part(w1, w2, from, to, 0.0, l);
                s += l * f - m1;
            }
            _ => {}
        }
    }
    -end[0] - s / l
}

/// The relative unbalance of the P-Delta load: the size (Euclidean norm) of the unbalance `r` on the
/// equations over that of the external joint forces, the joint loads on the equations `loads` and the
/// support reactions' forces `reactions` (member loads count through the reactions only); matches the
/// measured unbalance to 4e-6 (docs/SOURCES.md).
pub fn relative_unbalance(r: &[f64], loads: &[f64], reactions: &[f64]) -> f64 {
    let sq = |v: &[f64]| v.iter().map(|x| x * x).sum::<f64>();
    let f = (sq(loads) + sq(reactions)).sqrt();
    if f > 0.0 {
        sq(r).sqrt() / f
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newton_gives_up_after_four_steps_without_a_new_lowest_unbalance() {
        // Falling: goes on, however slowly.
        assert!(!newton_stalled(&[5.0, 3.0, 2.9, 2.8, 2.7, 2.6, 2.5]));
        // Too few steps to tell.
        assert!(!newton_stalled(&[5.0, 6.0, 7.0, 8.0]));
        // Four steps at or above the lowest so far (growing, or going round in circles).
        assert!(newton_stalled(&[5.0, 6.0, 7.0, 8.0, 9.0]));
        assert!(newton_stalled(&[5.0, 3.0, 4.0, 3.0, 4.0, 3.0]));
        // A new lowest value within the last four keeps it going.
        assert!(!newton_stalled(&[5.0, 3.0, 4.0, 3.5, 4.0, 2.9]));
        // An unbalance that is not a number counts as not falling.
        assert!(newton_stalled(&[
            5.0,
            f64::NAN,
            f64::NAN,
            f64::NAN,
            f64::NAN
        ]));
    }

    #[test]
    fn constant_stiffness_stops_when_it_runs_out_or_grows_twice_in_a_row() {
        // The first comparison does not count: three rises are needed from the start (probe 5, NW101).
        assert!(!leave_constant_stiffness(&[64.6, 65.0, 65.6]));
        assert!(leave_constant_stiffness(&[64.6, 65.0, 65.6, 66.2]));
        // Falling first, then growing twice (NWM1).
        let falling = [66.2, 33.1, 16.9, 9.25, 6.22, 5.53, 5.74];
        assert!(!leave_constant_stiffness(&falling));
        assert!(leave_constant_stiffness(&[&falling[..], &[6.23]].concat()));
        // Growing once only.
        assert!(!leave_constant_stiffness(&[9.0, 8.0, 7.0, 7.5, 7.2]));
        // Out of iterations (NW99).
        assert!(leave_constant_stiffness(&[
            10.0, 9.0, 8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0
        ]));
    }

    #[test]
    fn mean_axial_force_of_a_point_load_and_a_uniform_one() {
        // 3 m member, end force 3000 at end i (compression), an axial point load -3000 at 1 m: P is
        // -3000 over the first metre, 0 after; mean -1000 (probe 2, KPT).
        let mut end = [0.0; 12];
        end[0] = 3000.0;
        let pt = [LocalLoad::Point {
            dir: 0,
            p: -3000.0,
            at: 1.0,
        }];
        assert!((mean_axial(&end, &pt, 3.0) + 1000.0).abs() < 1e-9);
        // A uniform axial load -1000 / m from 3000 at end i to 0: mean -1500 (probe 1, KW).
        let un = [LocalLoad::uniform(0, -1000.0, 0.0, 3.0)];
        assert!((mean_axial(&end, &un, 3.0) + 1500.0).abs() < 1e-9);
    }
}
