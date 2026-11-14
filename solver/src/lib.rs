//! OSAB solver core: linear static analysis of 3D frames, trusses and flat shells
//! (Shell-Thin, Shell-Thick and Membrane), with joint springs, rigid links and load combinations.
//!
//! ```no_run
//! let text = std::fs::read_to_string("model.json").unwrap();
//! let model = osab_solver::Model::from_json(&text).unwrap();
//! let results = osab_solver::analyze(&model).unwrap();
//! println!("{}", results.to_json());
//! ```

// Matrix code reads best with explicit indices; `!(x > 0.0)` is used on purpose so NaN also fails.
#![allow(clippy::needless_range_loop, clippy::neg_cmp_op_on_partial_ord)]

pub mod analysis;
pub mod api;
pub mod area;
pub mod arms;
pub mod axes;
pub mod bench;
pub mod combine;
pub mod diaphragm;
pub mod dof;
pub mod eigen;
pub mod elements;
pub mod mass;
pub mod memory;
pub mod model;
pub mod order;
pub mod parallel;
pub mod pdelta;
pub mod piers;
pub mod results;
pub mod rigid;
pub mod ritz;
pub mod sparse;
pub mod spectrum;
pub mod story;
pub mod timing;

pub use analysis::{analyze, analyze_each};
pub use model::{Id, Model};
pub use results::Results;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot read model JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid model: {0}")]
    Invalid(String),
    #[error(
        "structure is unstable (a mechanism) at node {node}, DOF {dof}: add supports or members"
    )]
    Unstable { node: Id, dof: &'static str },
    #[error(
        "load case {case}: load on node {node} DOF {dof}, but no element gives that DOF stiffness"
    )]
    LoadOnUnstiffDof {
        case: Id,
        node: Id,
        dof: &'static str,
    },
    #[error("the analysis model has {joints} joints; this version solves up to {limit}")]
    TooBig { joints: usize, limit: usize },
    #[error(
        "the analysis needs about {needed:.1} GB of memory and {free:.1} GB is free \
         ({equations} equations; the factorized stiffness alone is {factor:.1} GB, the combinations \
         with a maximum and a minimum {combinations:.1} GB): use a coarser mesh or fewer such \
         combinations, or close other programs"
    )]
    TooBigForMemory {
        needed: f64,
        free: f64,
        factor: f64,
        combinations: f64,
        equations: usize,
    },
}
