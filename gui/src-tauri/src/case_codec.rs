//! A case's values as binary for the window: the parts that are the same in every case (joint ids,
//! frame and shell ids, nodes, lengths, axes) are sent once with the summary as the run's [`Geometry`],
//! and each case is just its numbers, 8-byte floats with no text to parse. The window puts them back
//! into the same objects (`gui/src/ipc/resultCases.ts`). A case whose objects differ from the geometry
//! goes as JSON instead.
//!
//! Bytes: `OWB1` and 4 zero bytes, then the floats (little-endian): every displacement (UX..RZ of each
//! joint in the geometry's order), every reaction (FX..MZ), each frame's station count and its stations
//! (x, P, V2, V3, T, M2, M3, ux, uy, uz), the same for each link, each shell corner's forces (F11..V23),
//! each joint drift (X, Y).
//! Or `OWJ1`, 4 zero bytes and the case as JSON.
//!
//! A mode's shape goes the same way: `OWB1` and UX..RZ of each joint in the order of the summary's modal
//! `joints`, or `OWJ1` and the shape as JSON when its joints are others.

use osab_solver::results::{CaseResults, Disp, ElementResults, NodeValues};
use osab_solver::Id;
use serde::Serialize;

pub const BINARY: &[u8; 8] = b"OWB1\0\0\0\0";
pub const JSON: &[u8; 8] = b"OWJ1\0\0\0\0";

/// A frame or link as every case has it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ElementGeometry {
    element: Id,
    nodes: [Id; 2],
    length: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    offsets: Option<[f64; 2]>,
    axes: [[f64; 3]; 3],
}

/// A shell element as every case has it, with the joint of each corner.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ShellGeometry {
    element: Id,
    area: Id,
    nodes: Vec<Id>,
    axes: [[f64; 3]; 3],
    corners: Vec<Id>,
}

/// A joint drift's joints as every case has them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DriftGeometry {
    story: String,
    node: Id,
    below: Id,
}

/// The objects of a run's cases, in their order.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Geometry {
    displacements: Vec<Id>,
    reactions: Vec<Id>,
    frames: Vec<ElementGeometry>,
    links: Vec<ElementGeometry>,
    shells: Vec<ShellGeometry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    joint_drifts: Vec<DriftGeometry>,
}

fn element(e: &ElementResults) -> ElementGeometry {
    ElementGeometry {
        element: e.element.clone(),
        nodes: e.nodes.clone(),
        length: e.length,
        offsets: (e.offsets != [0.0, 0.0]).then_some(e.offsets),
        axes: e.axes,
    }
}

impl Geometry {
    pub fn of(c: &CaseResults) -> Geometry {
        Geometry {
            displacements: c.displacements.iter().map(|d| d.node.clone()).collect(),
            reactions: c.reactions.iter().map(|r| r.node.clone()).collect(),
            frames: c.frames.iter().map(element).collect(),
            links: c.links.iter().map(element).collect(),
            shells: c
                .shells
                .iter()
                .map(|s| ShellGeometry {
                    element: s.element.clone(),
                    area: s.area.clone(),
                    nodes: s.nodes.clone(),
                    axes: s.axes,
                    corners: s.corners.iter().map(|c| c.node.clone()).collect(),
                })
                .collect(),
            joint_drifts: c
                .joint_drifts
                .iter()
                .map(|d| DriftGeometry {
                    story: d.story.clone(),
                    node: d.node.clone(),
                    below: d.below.clone(),
                })
                .collect(),
        }
    }
}

/// The case as bytes for the window: binary when its objects are the geometry's, else JSON.
pub fn encode(c: &CaseResults, g: &Geometry) -> Vec<u8> {
    if Geometry::of(c) != *g {
        let mut out = JSON.to_vec();
        out.extend(serde_json::to_vec(c).expect("results serialize"));
        return out;
    }
    let mut v: Vec<f64> = Vec::new();
    for d in &c.displacements {
        v.extend(d.values.as_array());
    }
    for r in &c.reactions {
        v.extend(r.values.as_array());
    }
    for e in c.frames.iter().chain(&c.links) {
        v.push(e.stations.len() as f64);
        for s in &e.stations {
            v.extend([s.x, s.P, s.V2, s.V3, s.T, s.M2, s.M3, s.ux, s.uy, s.uz]);
        }
    }
    for s in &c.shells {
        for k in &s.corners {
            v.extend(k.as_array());
        }
    }
    for d in &c.joint_drifts {
        v.extend(d.drift);
    }
    floats(&v)
}

/// A mode's shape as bytes for the window: binary when its joints are `joints` in that order, else JSON.
pub fn encode_shape(shape: &[NodeValues<Disp>], joints: &[Id]) -> Vec<u8> {
    if shape.len() != joints.len() || shape.iter().zip(joints).any(|(d, j)| d.node != *j) {
        let mut out = JSON.to_vec();
        out.extend(serde_json::to_vec(shape).expect("results serialize"));
        return out;
    }
    let v: Vec<f64> = shape.iter().flat_map(|d| d.values.as_array()).collect();
    floats(&v)
}

fn floats(v: &[f64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + 8 * v.len());
    out.extend_from_slice(BINARY);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

/// A mode's shape back as JSON, from its bytes and the summary's modal `joints` (as the window reads it).
#[cfg(test)]
pub fn decode_shape(bytes: &[u8], joints: &serde_json::Value) -> serde_json::Value {
    use serde_json::json;
    if &bytes[..8] == JSON {
        return serde_json::from_slice(&bytes[8..]).unwrap();
    }
    assert_eq!(&bytes[..8], BINARY);
    let v: Vec<f64> = bytes[8..]
        .as_chunks::<8>()
        .0
        .iter()
        .map(|b| f64::from_le_bytes(*b))
        .collect();
    let joints = joints.as_array().unwrap();
    assert_eq!(v.len(), 6 * joints.len(), "every number read");
    json!(joints
        .iter()
        .zip(v.chunks(6))
        .map(|(n, d)| json!({"node": n, "ux": d[0], "uy": d[1], "uz": d[2], "rx": d[3], "ry": d[4], "rz": d[5]}))
        .collect::<Vec<_>>())
}

/// The case back as JSON, from its bytes and the summary's `geometry` (what the window does, for tests).
#[cfg(test)]
pub fn decode(bytes: &[u8], head: &serde_json::Value, g: &serde_json::Value) -> serde_json::Value {
    use serde_json::{json, Map, Value};
    if &bytes[..8] == JSON {
        return serde_json::from_slice(&bytes[8..]).unwrap();
    }
    assert_eq!(&bytes[..8], BINARY);
    let v: Vec<f64> = bytes[8..]
        .as_chunks::<8>()
        .0
        .iter()
        .map(|b| f64::from_le_bytes(*b))
        .collect();
    let mut k = 0;
    let mut take = |names: &[&str], start: Map<String, Value>| -> Value {
        let mut o = start;
        for n in names {
            o.insert((*n).into(), json!(v[k]));
            k += 1;
        }
        Value::Object(o)
    };
    let with = |key: &str, id: &Value| -> Map<String, Value> {
        let mut m = Map::new();
        m.insert(key.into(), id.clone());
        m
    };
    let list = |key: &str| g[key].as_array().unwrap().clone();
    let mut out = Map::new();
    for key in [
        "id",
        "combination",
        "step",
        "spectrum",
        "base_reaction",
        "story_drifts",
        "story_forces",
        "pier_forces",
        "spandrel_forces",
    ] {
        if let Some(x) = head.get(key) {
            out.insert(key.into(), x.clone());
        }
    }
    let d: Vec<Value> = list("displacements")
        .iter()
        .map(|n| take(&["ux", "uy", "uz", "rx", "ry", "rz"], with("node", n)))
        .collect();
    let r: Vec<Value> = list("reactions")
        .iter()
        .map(|n| take(&["fx", "fy", "fz", "mx", "my", "mz"], with("node", n)))
        .collect();
    out.insert("displacements".into(), json!(d));
    out.insert("reactions".into(), json!(r));
    for key in ["frames", "links"] {
        let mut items = Vec::new();
        for e in list(key) {
            let n = take(&["n"], Map::new())["n"].as_f64().unwrap() as usize;
            let stations: Vec<Value> = (0..n)
                .map(|_| {
                    take(
                        &["x", "P", "V2", "V3", "T", "M2", "M3", "ux", "uy", "uz"],
                        Map::new(),
                    )
                })
                .collect();
            let mut o = e.as_object().unwrap().clone();
            o.insert("stations".into(), json!(stations));
            items.push(Value::Object(o));
        }
        out.insert(key.into(), json!(items));
    }
    let shells: Vec<Value> = list("shells")
        .iter()
        .map(|s| {
            let mut o = s.as_object().unwrap().clone();
            let corners: Vec<Value> = o["corners"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| {
                    take(
                        &["F11", "F22", "F12", "M11", "M22", "M12", "V13", "V23"],
                        with("node", n),
                    )
                })
                .collect();
            o.insert("corners".into(), json!(corners));
            Value::Object(o)
        })
        .collect();
    if !shells.is_empty() {
        out.insert("shells".into(), json!(shells));
    }
    if let Some(e) = head.get("equilibrium") {
        out.insert("equilibrium".into(), e.clone());
    }
    if let Some(list) = g.get("joint_drifts").and_then(|l| l.as_array()) {
        let drifts: Vec<Value> = list
            .iter()
            .map(|d| {
                let mut o = d.as_object().unwrap().clone();
                let xy = take(&["x", "y"], Map::new());
                o.insert("drift".into(), json!([xy["x"], xy["y"]]));
                Value::Object(o)
            })
            .collect();
        out.insert("joint_drifts".into(), json!(drifts));
    }
    assert_eq!(k, v.len(), "every number read");
    Value::Object(out)
}
