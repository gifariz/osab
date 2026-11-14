//! JSON entry points shared by the desktop app (Tauri commands) and `osab api` (stdin/stdout).
//! Errors come back as data, so the app can show the message and highlight the named node.

use serde::Serialize;

use crate::model::{Id, Model};
use crate::results::Results;
use crate::{analyze, Error};

/// An error the app can show: `kind` is "json", "invalid", "unstable", "load_on_unstiff_dof", "io",
/// "too_big" (with `joints` and `limit`: the browser demo's size limit), "too_big_for_memory" (with
/// `needed_gb` and `free_gb`) or "crash" (the browser build stopped on a bug).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ErrorReport {
    pub kind: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dof: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case: Option<Id>,
    /// Kind "too_big": the analysis model's joints and the browser demo's limit (boxed, so the report
    /// stays small; written as the fields `joints` and `limit`).
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub size: Option<Box<SizeLimit>>,
    /// Kind "too_big_for_memory": the memory the analysis needs and the memory free (boxed, so the
    /// report stays small; written as the fields `needed_gb` and `free_gb`).
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub memory: Option<Box<MemoryGb>>,
}

/// The analysis model's joints and the size limit they passed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SizeLimit {
    pub joints: Option<u32>,
    pub limit: Option<u32>,
}

/// The memory an analysis needs and the memory free, in GB.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MemoryGb {
    pub needed_gb: f64,
    pub free_gb: f64,
}

impl ErrorReport {
    /// A report with a kind and a message only.
    pub fn new(kind: &'static str, message: String) -> Self {
        ErrorReport {
            kind,
            message,
            node: None,
            dof: None,
            case: None,
            size: None,
            memory: None,
        }
    }

    pub fn io(message: String) -> Self {
        Self::new("io", message)
    }
}

impl From<Error> for ErrorReport {
    fn from(e: Error) -> Self {
        let mut r = ErrorReport::new("", e.to_string());
        r.kind = match e {
            Error::Json(_) => "json",
            Error::Invalid(_) => "invalid",
            Error::Unstable { node, dof } => {
                (r.node, r.dof) = (Some(node), Some(dof));
                "unstable"
            }
            Error::LoadOnUnstiffDof { case, node, dof } => {
                (r.node, r.dof, r.case) = (Some(node), Some(dof), Some(case));
                "load_on_unstiff_dof"
            }
            Error::TooBig { joints, limit } => {
                r.size = Some(Box::new(SizeLimit {
                    joints: joints.try_into().ok(),
                    limit: limit.try_into().ok(),
                }));
                "too_big"
            }
            Error::TooBigForMemory { needed, free, .. } => {
                r.memory = Some(Box::new(MemoryGb {
                    needed_gb: needed,
                    free_gb: free,
                }));
                "too_big_for_memory"
            }
        };
        r
    }
}

/// A model with every member in the end order (end i at the lower Z, then X, then Y), and the ids of the members that were reversed.
#[derive(Debug, Clone, Serialize)]
pub struct Oriented {
    pub model: Model,
    pub reversed: Vec<Id>,
}

/// Reads a model without checking it, so a file with mistakes can still be opened and fixed.
pub fn parse_model(text: &str) -> Result<Model, ErrorReport> {
    serde_json::from_str(text).map_err(|e| Error::Json(e).into())
}

/// Checks and analyses a model given as JSON.
pub fn analyze_value(model: serde_json::Value) -> Result<Results, ErrorReport> {
    let model: Model = serde_json::from_value(model).map_err(Error::Json)?;
    Ok(analyze(&model)?)
}

/// Applies the end-order rule (see [`Model::oriented`]) to a model given as JSON.
pub fn orient_value(model: serde_json::Value) -> Result<Oriented, ErrorReport> {
    let model: Model = serde_json::from_value(model).map_err(Error::Json)?;
    let (model, reversed) = model.oriented();
    Ok(Oriented { model, reversed })
}

/// A spectrum function's points as an analysis uses them, and for ASCE 7-16 its coefficients.
#[derive(Debug, Serialize)]
pub struct SpectrumPoints {
    pub points: Vec<[f64; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asce7_16: Option<AsceValues>,
}

/// Fa, Fv, SDS and SD1 of an ASCE 7-16 function.
#[derive(Debug, Serialize)]
pub struct AsceValues {
    pub fa: f64,
    pub fv: f64,
    pub sds: f64,
    pub sd1: f64,
}

/// Checks one spectrum function given as JSON (as a model's would be) and gives its points.
pub fn spectrum_value(function: serde_json::Value) -> Result<SpectrumPoints, ErrorReport> {
    let f: crate::spectrum::SpectrumFunction =
        serde_json::from_value(function).map_err(Error::Json)?;
    crate::spectrum::validate(std::slice::from_ref(&f), &[], false, &|_| false)?;
    let asce7_16 = f.asce7_16.as_ref().map(|a| {
        let (fa, fv) = a.coefficients();
        let (sds, sd1) = a.sds_sd1();
        AsceValues {
            fa: fa.unwrap_or(0.0),
            fv: fv.unwrap_or(0.0),
            sds,
            sd1,
        }
    });
    Ok(SpectrumPoints {
        points: f.table(),
        asce7_16,
    })
}

/// The model as the app saves it (pretty JSON).
pub fn model_to_json(model: &Model) -> String {
    serde_json::to_string_pretty(model).expect("model serializes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn spectrum_function_points_and_errors() {
        let p = spectrum_value(json!({"id": "T", "points": [[0, 0.4], [0.5, 1]]})).unwrap();
        assert_eq!(p.points, [[0.0, 0.4], [0.5, 1.0]]);
        assert!(p.asce7_16.is_none());
        let f = json!({"id": "A", "asce7_16": {"ss": 0.8, "s1": 0.35, "tl": 8, "site_class": "D"}});
        let p = spectrum_value(f.clone()).unwrap();
        let ours: crate::spectrum::SpectrumFunction = serde_json::from_value(f).unwrap();
        assert_eq!(p.points, ours.table());
        let a = p.asce7_16.unwrap();
        // Table 11.4-1 at Ss = 0.8 (between 0.75 and 1.0) and Table 11.4-2 at S1 = 0.35 (between 0.3 and 0.4).
        assert!((a.fa - 1.18).abs() < 1e-12 && (a.fv - 1.95).abs() < 1e-12);
        assert!((a.sds - 2.0 / 3.0 * 1.18 * 0.8).abs() < 1e-12);
        let err = |f| spectrum_value(f).unwrap_err().message;
        assert!(err(
            json!({"id": "F", "asce7_16": {"ss": 0.8, "s1": 0.35, "tl": 8, "site_class": "F"}})
        )
        .contains("fa and fv"));
        assert!(err(json!({"id": "E", "points": []})).contains("no points"));
    }

    fn beam(fix: serde_json::Value) -> serde_json::Value {
        json!({
            "materials": [{"id": "m", "E": 200e6, "nu": 0.3}],
            "sections": [{"id": "s", "A": 0.01, "I22": 5e-5, "I33": 2e-4, "J": 1e-5}],
            "nodes": [{"id": "1", "x": 4}, {"id": 2, "x": 0}],
            "frames": [{"id": "B1", "type": "frame", "nodes": ["1", "2"], "material": "m", "section": "s"}],
            "supports": [{"node": "2", "fix": fix}],
            "load_cases": [{"id": "D", "node_loads": [{"node": "1", "fz": -1}]}],
            "gui": {"grid": {"x": [0, 4]}}
        })
    }

    #[test]
    fn unstable_model_names_node_and_dof() {
        let mut m = beam(json!(["uz"]));
        // By default the run goes on and lists the directions without stiffness.
        assert!(!analyze_value(m.clone()).unwrap().instabilities.is_empty());
        m["on_instability"] = json!("stop");
        let err = analyze_value(m).unwrap_err();
        assert_eq!(err.kind, "unstable");
        assert!(err.node.is_some() && err.dof.is_some());
        let text = serde_json::to_string(&err).unwrap();
        assert!(text.contains("\"kind\":\"unstable\""), "{text}");
    }

    #[test]
    fn invalid_and_json_errors() {
        let mut m = beam(json!("fixed"));
        m["frames"][0]["section"] = json!("nope");
        assert_eq!(analyze_value(m).unwrap_err().kind, "invalid");
        assert_eq!(analyze_value(json!({"nodes": 3})).unwrap_err().kind, "json");
        assert_eq!(parse_model("{").unwrap_err().kind, "json");
    }

    #[test]
    fn orient_reverses_and_keeps_gui_block() {
        let o = orient_value(beam(json!("fixed"))).unwrap();
        assert_eq!(o.reversed, vec![Id::from("B1")]);
        assert_eq!(o.model.frames[0].nodes, [Id::from("2"), Id::from("1")]);
        let saved = model_to_json(&o.model);
        assert!(saved.contains("\"gui\""), "{saved}");
        // Saved files open again unchanged.
        assert_eq!(model_to_json(&parse_model(&saved).unwrap()), saved);
        assert!(analyze_value(beam(json!("fixed"))).is_ok());
    }

    #[test]
    fn springs_and_links_survive_save_and_open() {
        let mut m = beam(json!("fixed"));
        m["nodes"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": "3", "x": 4, "z": 1}));
        m["springs"] = json!([{"node": "1", "uz": 5000, "ry": 2e4}]);
        m["links"] = json!([{"id": "L1", "type": "rigid", "nodes": ["3", "1"]}]);
        let o = orient_value(m.clone()).unwrap();
        // Links follow the end order too (lower Z first).
        assert_eq!(o.model.links[0].nodes, [Id::from("1"), Id::from("3")]);
        let saved = model_to_json(&o.model);
        assert!(
            saved.contains("\"springs\"") && saved.contains("\"links\""),
            "{saved}"
        );
        assert_eq!(model_to_json(&parse_model(&saved).unwrap()), saved);
        let r = analyze_value(m).unwrap();
        assert_eq!(r.cases[0].links.len(), 1);
        // Files without springs or links are saved without those keys.
        assert!(
            !model_to_json(&orient_value(beam(json!("fixed"))).unwrap().model).contains("springs")
        );
    }
}

#[cfg(test)]
mod memory_report_tests {
    use super::*;

    #[test]
    fn size_errors_report_their_numbers() {
        let r = ErrorReport::from(Error::TooBigForMemory {
            needed: 41.25,
            free: 27.5,
            factor: 9.0,
            combinations: 1.0,
            equations: 1_900_000,
        });
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["kind"], "too_big_for_memory");
        assert_eq!(v["needed_gb"], 41.25);
        assert_eq!(v["free_gb"], 27.5);
        // Other kinds carry no memory fields.
        let v = serde_json::to_value(ErrorReport::io("x".into())).unwrap();
        assert!(v.get("needed_gb").is_none() && v.get("joints").is_none());
        // The browser's size limit keeps its two fields.
        let v = serde_json::to_value(ErrorReport::from(Error::TooBig {
            joints: 90_000,
            limit: 60_000,
        }))
        .unwrap();
        assert_eq!(
            (v["joints"].as_u64(), v["limit"].as_u64()),
            (Some(90_000), Some(60_000))
        );
    }
}
