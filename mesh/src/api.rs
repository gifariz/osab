//! JSON entry points of the run step, shared by the desktop app (Tauri commands), `osab api` and the
//! browser build (`web/`). Errors come back as data (the solver's `ErrorReport`).

use serde::Serialize;

use osab_solver::api::{orient_value, ErrorReport};
use osab_solver::results::{CaseResults, MeshInfo};
use osab_solver::{Error, Model, Results};

/// The commands of [`call`].
pub const COMMANDS: [&str; 6] = [
    "analyze",
    "mesh",
    "orient",
    "spectrum",
    "section",
    "section_list",
];

/// Runs one app command on a model given as JSON text and returns the reply as JSON text (see
/// [`reply`]): `analyze` (mesh, check, solve), `mesh` (the analysis mesh only), `orient` (end
/// order), `spectrum` (a spectrum function's points; the input is the function) or `section` (a section
/// shape's properties and outline; the input is `{shape, materials, length}`) or `section_list` (the steel
/// list's names and types; any input). `max_joints` stops `analyze` on bigger analysis models (the browser demo's size limit).
/// Used by `osab api` and the browser build, so both answer alike.
pub fn call(command: &str, input: &str, max_joints: Option<usize>) -> String {
    let model = serde_json::from_str(input).map_err(|e| ErrorReport::from(Error::Json(e)));
    match command {
        "analyze" => reply(model.and_then(|m| analyze_value_limited(m, max_joints))),
        "mesh" => reply(model.and_then(mesh_value)),
        "orient" => reply(model.and_then(orient_value)),
        "spectrum" => reply(model.and_then(osab_solver::api::spectrum_value)),
        "section" => reply(model.and_then(crate::section::section_value)),
        "section_list" => reply(Ok(crate::section::section_list())),
        _ => reply::<()>(Err(ErrorReport::new(
            "invalid",
            format!("unknown command {command:?}"),
        ))),
    }
}

/// `{"ok": value}` or `{"error": report}` as JSON text.
pub fn reply<T: Serialize>(result: Result<T, ErrorReport>) -> String {
    #[derive(Serialize)]
    #[serde(rename_all = "lowercase")]
    enum Reply<T> {
        Ok(T),
        Error(ErrorReport),
    }
    let r = match result {
        Ok(v) => Reply::Ok(v),
        Err(e) => Reply::Error(e),
    };
    serde_json::to_string(&r).expect("reply serializes")
}

/// Meshes, checks and analyses a model given as JSON.
pub fn analyze_value(model: serde_json::Value) -> Result<Results, ErrorReport> {
    analyze_value_limited(model, None)
}

/// [`analyze_value`] with the size limit of [`crate::run_limited`].
pub fn analyze_value_limited(
    model: serde_json::Value,
    max_joints: Option<usize>,
) -> Result<Results, ErrorReport> {
    let model: Model = serde_json::from_value(model).map_err(Error::Json)?;
    Ok(crate::run_limited(&model, max_joints)?)
}

/// [`analyze_value_limited`] handing each case's results to `out` as soon as they are known (see
/// [`crate::run_each`]); returns the rest, `cases` empty.
pub fn analyze_value_each(
    model: serde_json::Value,
    max_joints: Option<usize>,
    out: &mut dyn FnMut(CaseResults) -> Result<(), Error>,
) -> Result<Results, ErrorReport> {
    let model: Model = serde_json::from_value(model).map_err(Error::Json)?;
    Ok(crate::run_each(&model, max_joints, out)?)
}

/// The analysis mesh only (for showing it before a run), with the mesh step's notes.
#[derive(Debug, Clone, Serialize)]
pub struct MeshView {
    pub notes: Vec<String>,
    pub mesh: MeshInfo,
}

pub fn mesh_value(model: serde_json::Value) -> Result<MeshView, ErrorReport> {
    let model: Model = serde_json::from_value(model).map_err(Error::Json)?;
    let m = crate::mesh(&model)?;
    Ok(MeshView {
        notes: m.notes,
        mesh: m.info,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn slab(extra: serde_json::Value) -> serde_json::Value {
        let mut m = json!({
            "materials": [{"id": "C", "E": 25e6, "nu": 0.2}],
            "sections": [],
            "area_sections": [{"id": "S", "type": "shell-thin", "material": "C", "thickness": 0.2}],
            "nodes": [{"id": "1"}, {"id": "2", "x": 5}, {"id": "3", "x": 5, "y": 4}, {"id": "4", "y": 4}],
            "areas": [{"id": "F1", "nodes": ["1", "2", "3", "4"], "section": "S"}],
            "supports": [{"node": "1", "fix": "fixed"}, {"node": "2", "fix": "fixed"},
                         {"node": "3", "fix": "fixed"}, {"node": "4", "fix": "fixed"}],
            "load_cases": [{"id": "Q", "area_loads": [{"area": "F1", "dir": "Z", "q": -10}]}]
        });
        for (k, v) in extra.as_object().unwrap() {
            m[k] = v.clone();
        }
        m
    }

    #[test]
    fn default_mesh_is_an_equal_grid_and_results_name_the_area() {
        let r = analyze_value(slab(json!({}))).unwrap();
        // 5 x 4 m at 1.25 m: 4 x 4 elements (probe case f_rect).
        let mesh = r.mesh.as_ref().unwrap();
        assert_eq!(mesh.shells.len(), 16);
        assert_eq!(mesh.joints.len(), 21);
        let c = &r.cases[0];
        assert_eq!(c.shells.len(), 16);
        assert!(c
            .shells
            .iter()
            .all(|s| s.area.0 == "F1" && s.element.0.starts_with("F1-")));
        // The four supports carry the whole load.
        let fz: f64 = c.reactions.iter().map(|r| r.values.fz).sum();
        assert!((fz - 200.0).abs() < 1e-9, "{fz}");
        let v = mesh_value(slab(json!({}))).unwrap();
        assert_eq!(v.mesh.shells.len(), 16);
    }

    #[test]
    fn mesh_off_keeps_one_element() {
        let r = analyze_value(slab(json!({"mesh": "off"}))).unwrap();
        assert!(r.mesh.is_none());
        assert_eq!(r.cases[0].shells.len(), 1);
        assert_eq!(r.cases[0].shells[0].element.0, "F1");
    }

    #[test]
    fn call_replies_ok_or_error_and_applies_the_size_limit() {
        let text = slab(json!({})).to_string();
        let ok: serde_json::Value = serde_json::from_str(&call("analyze", &text, None)).unwrap();
        assert_eq!(ok["ok"]["mesh"]["joints"].as_array().unwrap().len(), 21);
        // 4 corner joints + 21 mesh joints = 25 analysis joints: a limit of 25 solves, 24 stops
        // before the solve.
        assert!(call("analyze", &text, Some(25)).starts_with("{\"ok\":"));
        let big: serde_json::Value =
            serde_json::from_str(&call("analyze", &text, Some(24))).unwrap();
        assert_eq!(big["error"]["kind"], "too_big");
        assert_eq!(
            (
                big["error"]["joints"].as_u64(),
                big["error"]["limit"].as_u64()
            ),
            (Some(25), Some(24))
        );
        // The limit is for runs only; the mesh view and orient still work.
        assert!(call("mesh", &text, Some(24)).starts_with("{\"ok\":"));
        assert!(call("orient", &text, Some(24)).starts_with("{\"ok\":"));
        let err = |command: &str, input: &str| -> serde_json::Value {
            serde_json::from_str::<serde_json::Value>(&call(command, input, None)).unwrap()["error"]
                ["kind"]
                .clone()
        };
        assert_eq!(err("analyze", "{"), "json");
        assert_eq!(err("solve", &text), "invalid");
    }
}
