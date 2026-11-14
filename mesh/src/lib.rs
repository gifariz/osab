//! OSAB mesh step: turns the drawn objects (any flat floor or wall polygon, openings, frames) into the
//! analysis model (3- and 4-node shells, frame pieces, mesh joints, edge constraints)
//! by auto-meshing, runs the solver on it, and tells the results in objects again.
//!
//! ```no_run
//! let text = std::fs::read_to_string("model.json").unwrap();
//! let model = osab_solver::Model::from_json(&text).unwrap();
//! let results = osab_mesh::run(&model).unwrap();
//! println!("{}", results.to_json());
//! ```
//!
//! The solver crate knows nothing about meshing; this crate is the "run" step of the app and the CLI.

#![allow(
    clippy::needless_range_loop,
    clippy::neg_cmp_op_on_partial_ord,
    clippy::type_complexity
)]

pub mod aisc;
pub mod api;
mod arrange;
mod boxes;
mod build;
mod geom;
mod grade;
mod hanging;
mod mapped;
mod nulls;
mod panels;
mod points;
mod rect;
pub mod section;
mod stitch;
mod stored;
mod tri;

pub use build::{default_size, mesh, Meshed};

use osab_solver::results::CaseResults;
use osab_solver::{analyze_each, Error, Model, Results};

/// Meshes the model, analyses the mesh and returns the results per object (frames whole, shells with
/// their area, mesh joints in the joint lists, the mesh itself in `mesh`).
pub fn run(model: &Model) -> Result<Results, Error> {
    run_limited(model, None)
}

/// Like [`run`], but stops before the solve when the analysis model has more than `max_joints` joints
/// (the browser demo's size limit; the desktop app and the CLI have none).
pub fn run_limited(model: &Model, max_joints: Option<usize>) -> Result<Results, Error> {
    let mut cases = Vec::new();
    let mut r = run_each(model, max_joints, &mut |c| {
        cases.push(c);
        Ok(())
    })?;
    r.cases = cases;
    Ok(r)
}

/// Like [`run_limited`], but hands each case's results (told in objects) to `out` as soon as they are
/// known, in the same order, and returns the rest (`cases` empty): a model with many load cases needs
/// the memory of a few.
pub fn run_each(
    model: &Model,
    max_joints: Option<usize>,
    out: &mut dyn FnMut(CaseResults) -> Result<(), Error>,
) -> Result<Results, Error> {
    let mut clock = osab_solver::timing::Clock::new();
    let m = mesh(model)?;
    clock.lap("mesh step");
    let joints = m.model.nodes.len();
    if let Some(limit) = max_joints.filter(|&limit| joints > limit) {
        return Err(Error::TooBig { joints, limit });
    }
    let objects = stitch::Objects::new(&m, &model.frames);
    let r = analyze_each(&m.model, &mut |mut c| {
        {
            let _object_time = osab_solver::timing::Span::new("results: frame objects");
            objects.case(&mut c);
        }
        out(c)
    })?;
    clock.lap("analysis (total)");
    let r = stitch::finish(&m, r);
    clock.lap("results per object");
    Ok(r)
}
