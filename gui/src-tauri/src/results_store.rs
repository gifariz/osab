//! The last run's results, kept in this process: the window gets a summary (every case's name and
//! checks, the mesh, modal results without the mode shapes, centres and the cases' [`Geometry`]) and asks
//! for one case's values, or one mode's shape, when it shows them, so a model with many load cases or a
//! large mesh never sends them all at once. Each case is encoded as soon as the solver hands it on
//! (`case_codec`: its numbers only, 8 bytes each); past [`MEMORY_LIMIT`] the cases and shapes go to a folder
//! on disk, so a run needs the memory of a few cases. Saved next to the model file as a folder
//! `<name>.results` (an index with a fingerprint of the model, the summary, one file per case and per mode),
//! so reopening an unchanged model needs no run and reads only the summary.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use osab_solver::api::ErrorReport;
use osab_solver::model::{ComboType, JointMass, Units};
use osab_solver::piers::{PierForce, PierSection, SpandrelForce, SpandrelSection};
use osab_solver::results::{
    CaseResults, DiaphragmCentre, Equilibrium, MeshInfo, ModalResults, Mode, Step,
};
use osab_solver::story::{StoryDrift, StoryForce};
use osab_solver::{Id, Model, Results};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use crate::case_codec::{self, Geometry};

/// Case values kept in memory before the rest go to disk (bytes).
pub const MEMORY_LIMIT: usize = 256 << 20;

/// The format of a saved results folder (older folders are not read; 3: mode shapes in their own files).
const VERSION: u32 = 3;

/// A case's values as the window takes them (`case_codec`): in memory, or in a file.
pub enum CaseData {
    Memory(Vec<u8>),
    File(PathBuf),
}

/// Results as the window takes them: the summary (JSON), each case's values and each mode's shape.
pub struct Stored {
    pub summary: Box<RawValue>,
    cases: Vec<CaseData>,
    modes: Vec<CaseData>,
    /// The run's own folder of case files, removed with the results.
    own_dir: Option<PathBuf>,
}

impl Drop for Stored {
    fn drop(&mut self) {
        if let Some(d) = &self.own_dir {
            let _ = std::fs::remove_dir_all(d);
        }
    }
}

#[derive(Default)]
pub struct ResultsStore(pub Mutex<Option<Stored>>);

/// One case without its values: the name, the combination step, the equilibrium check and the case's
/// small per-story results (base reaction, story drifts and forces, pier and spandrel forces).
#[derive(Serialize)]
struct CaseHead {
    id: Id,
    #[serde(skip_serializing_if = "Option::is_none")]
    combination: Option<ComboType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    step: Option<Step>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    spectrum: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    equilibrium: Option<Equilibrium>,
    #[serde(skip_serializing_if = "Option::is_none")]
    base_reaction: Option<[f64; 6]>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    story_drifts: Vec<StoryDrift>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    story_forces: Vec<StoryForce>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pier_forces: Vec<PierForce>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    spandrel_forces: Vec<SpandrelForce>,
    /// The case has shell forces (the app offers shell views).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    has_shells: bool,
}

/// The results without the cases' values (the shape of `Results`, each case a `CaseHead`), and the
/// objects every case has (for reading the cases' numbers).
#[derive(Serialize)]
struct Summary<'a> {
    title: &'a str,
    units: &'a Units,
    notes: &'a [String],
    cases: &'a [CaseHead],
    #[serde(skip_serializing_if = "Option::is_none")]
    mesh: Option<&'a MeshInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    modal: Option<ModalHead<'a>>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    diaphragm_centers: &'a [DiaphragmCentre],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    pier_sections: &'a [PierSection],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    spandrel_sections: &'a [SpandrelSection],
    #[serde(skip_serializing_if = "Option::is_none")]
    geometry: Option<&'a Geometry>,
}

/// One mode without its shape.
#[derive(Serialize)]
struct ModeHead<'a> {
    mode: usize,
    period: f64,
    frequency: f64,
    circular_frequency: f64,
    eigenvalue: f64,
    participation: &'a [f64; 6],
    mass_ratio: &'a [f64; 6],
    sum_mass_ratio: &'a [f64; 6],
    #[serde(skip_serializing_if = "is_false")]
    static_mode: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// The modal results without the shapes, and the joints of the shapes (their order in each shape's bytes).
#[derive(Serialize)]
struct ModalHead<'a> {
    id: &'a Id,
    modes: Vec<ModeHead<'a>>,
    masses: &'a [JointMass],
    free_mass: &'a [f64; 6],
    centre_of_mass: &'a [f64; 3],
    load_participation: &'a [osab_solver::results::LoadParticipation],
    joints: Vec<Id>,
}

fn mode_head(m: &Mode) -> ModeHead<'_> {
    ModeHead {
        mode: m.mode,
        period: m.period,
        frequency: m.frequency,
        circular_frequency: m.circular_frequency,
        eigenvalue: m.eigenvalue,
        participation: &m.participation,
        mass_ratio: &m.mass_ratio,
        sum_mass_ratio: &m.sum_mass_ratio,
        static_mode: m.static_mode,
    }
}

fn raw(text: String) -> Box<RawValue> {
    RawValue::from_string(text).expect("valid JSON")
}

fn io(what: &str, path: &Path, e: std::io::Error) -> ErrorReport {
    ErrorReport::io(format!("cannot {what} {}: {e}", path.display()))
}

/// Collects a run's cases as the solver hands them on (`push`), then the summary (`finish`).
pub struct Collector {
    dir: PathBuf,
    limit: usize,
    heads: Vec<CaseHead>,
    geometry: Option<Geometry>,
    cases: Vec<CaseData>,
    bytes: usize,
    /// Cases or shapes went to files (the folder is made).
    spilled: bool,
    failed: Option<ErrorReport>,
}

impl Collector {
    /// Cases past `limit` bytes go to files in `dir` (made when needed, emptied first).
    pub fn new(dir: PathBuf, limit: usize) -> Collector {
        Collector {
            dir,
            limit,
            heads: Vec::new(),
            geometry: None,
            cases: Vec::new(),
            bytes: 0,
            spilled: false,
            failed: None,
        }
    }

    pub fn push(&mut self, c: CaseResults) {
        if self.failed.is_some() {
            return;
        }
        self.heads.push(CaseHead {
            id: c.id.clone(),
            combination: c.combination,
            step: c.step,
            spectrum: c.spectrum,
            equilibrium: c.equilibrium.clone(),
            base_reaction: c.base_reaction,
            story_drifts: c.story_drifts.clone(),
            story_forces: c.story_forces.clone(),
            pier_forces: c.pier_forces.clone(),
            spandrel_forces: c.spandrel_forces.clone(),
            has_shells: !c.shells.is_empty(),
        });
        // The first case's objects are the run's geometry.
        let g = self.geometry.get_or_insert_with(|| Geometry::of(&c));
        let data = case_codec::encode(&c, g);
        drop(c);
        let name = format!("case-{}.bin", self.cases.len());
        if let Some(d) = self.hold(name, data) {
            self.cases.push(d);
        }
    }

    /// Keeps a case's or a shape's bytes in memory, or past the limit in a file `name` (None when that failed).
    fn hold(&mut self, name: String, data: Vec<u8>) -> Option<CaseData> {
        self.bytes += data.len();
        if self.bytes <= self.limit {
            return Some(CaseData::Memory(data));
        }
        if !self.spilled {
            let _ = std::fs::remove_dir_all(&self.dir);
            if let Err(e) = std::fs::create_dir_all(&self.dir) {
                self.failed = Some(io("make", &self.dir, e));
                return None;
            }
            self.spilled = true;
        }
        let path = self.dir.join(name);
        match std::fs::write(&path, data) {
            Ok(()) => Some(CaseData::File(path)),
            Err(e) => {
                self.failed = Some(io("write", &path, e));
                None
            }
        }
    }

    /// The results kept for the window: `rest` is the run's results without the cases.
    pub fn finish(mut self, rest: &Results) -> Result<Stored, ErrorReport> {
        // The shapes' joints: the first mode's (every mode has the same).
        let joints: Vec<Id> = rest
            .modal
            .as_ref()
            .and_then(|m| m.modes.first())
            .map(|m| m.shape.iter().map(|d| d.node.clone()).collect())
            .unwrap_or_default();
        let mut modes = Vec::new();
        for (k, m) in rest.modal.iter().flat_map(|m| &m.modes).enumerate() {
            let data = case_codec::encode_shape(&m.shape, &joints);
            if let Some(d) = self.hold(format!("mode-{k}.bin"), data) {
                modes.push(d);
            }
        }
        if let Some(e) = self.failed {
            return Err(e);
        }
        let modal = rest.modal.as_ref().map(|m: &ModalResults| ModalHead {
            id: &m.id,
            modes: m.modes.iter().map(mode_head).collect(),
            masses: &m.masses,
            free_mass: &m.free_mass,
            centre_of_mass: &m.centre_of_mass,
            load_participation: &m.load_participation,
            joints,
        });
        let summary = Summary {
            title: &rest.title,
            units: &rest.units,
            notes: &rest.notes,
            cases: &self.heads,
            mesh: rest.mesh.as_ref(),
            modal,
            diaphragm_centers: &rest.diaphragm_centers,
            pier_sections: &rest.pier_sections,
            spandrel_sections: &rest.spandrel_sections,
            geometry: self.geometry.as_ref(),
        };
        Ok(Stored {
            summary: raw(serde_json::to_string(&summary).expect("results serialize")),
            cases: self.cases,
            modes,
            own_dir: self.spilled.then_some(self.dir),
        })
    }
}

impl Stored {
    /// Deletes the results of the cases at `indices` (the summary's order) and, with `modal`, the modal
    /// results; the other cases keep their values, their indices close up.
    pub fn delete(&mut self, indices: &[usize], modal: bool) {
        let mut summary: serde_json::Value =
            serde_json::from_str(self.summary.get()).expect("summary is JSON");
        let mut k = 0;
        self.cases.retain(|_| {
            k += 1;
            !indices.contains(&(k - 1))
        });
        if let Some(cases) = summary["cases"].as_array_mut() {
            let mut k = 0;
            cases.retain(|_| {
                k += 1;
                !indices.contains(&(k - 1))
            });
        }
        if modal {
            summary["modal"] = serde_json::Value::Null;
            self.modes.clear();
        }
        self.summary = raw(summary.to_string());
    }

    /// One case's values, as `case_codec` encodes them.
    pub fn case(&self, index: usize) -> Result<Vec<u8>, ErrorReport> {
        read(self.cases.get(index), || {
            format!("no results for case {index}: run the analysis again")
        })
    }

    /// One mode's shape (its index in the summary's modes), as `case_codec` encodes it.
    pub fn mode(&self, index: usize) -> Result<Vec<u8>, ErrorReport> {
        read(self.modes.get(index), || {
            format!("no shape for mode {}: run the analysis again", index + 1)
        })
    }
}

fn read(data: Option<&CaseData>, missing: impl Fn() -> String) -> Result<Vec<u8>, ErrorReport> {
    match data {
        Some(CaseData::Memory(v)) => Ok(v.clone()),
        Some(CaseData::File(p)) => std::fs::read(p).map_err(|e| io("read", p, e)),
        None => Err(ErrorReport::io(missing())),
    }
}

/// Fingerprint of a model's analysis data (the app's `gui` part and `not_run` left out): FNV-1a of its JSON.
pub fn model_hash(model: &Model) -> String {
    let mut m = model.clone();
    m.gui = None;
    // Which cases are set to run changes no result.
    m.not_run.clear();
    let text = osab_solver::api::model_to_json(&m);
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// The results folder of a model file: `frame.json` -> `frame.results`.
pub fn results_dir(model_path: &Path) -> PathBuf {
    let stem = model_path.file_stem().unwrap_or_default().to_string_lossy();
    model_path.with_file_name(format!("{stem}.results"))
}

/// The folder's index: its format, the model's fingerprint and the number of cases and of mode shapes.
#[derive(Serialize, Deserialize)]
struct Index {
    #[serde(default)]
    version: u32,
    model_hash: String,
    cases: usize,
    #[serde(default)]
    modes: usize,
}

/// Writes the results next to the model file, for `model` (the model as saved). They go to a new
/// folder first, which then replaces the old one, so results read from the old one stay readable.
pub fn save(model_path: &Path, model: &Model, results: &Stored) -> Result<(), ErrorReport> {
    let dir = results_dir(model_path);
    let tmp = dir.with_extension("results-new");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| io("make", &tmp, e))?;
    let write = |name: String, data: &[u8]| -> Result<(), ErrorReport> {
        let path = tmp.join(name);
        let mut f = std::io::BufWriter::new(
            std::fs::File::create(&path).map_err(|e| io("write", &path, e))?,
        );
        f.write_all(data).map_err(|e| io("write", &path, e))
    };
    write("summary.json".into(), results.summary.get().as_bytes())?;
    let cases = results
        .cases
        .iter()
        .enumerate()
        .map(|(k, c)| (format!("case-{k}.bin"), c));
    let modes = results
        .modes
        .iter()
        .enumerate()
        .map(|(k, c)| (format!("mode-{k}.bin"), c));
    for (name, c) in cases.chain(modes) {
        match c {
            CaseData::Memory(v) => write(name, v)?,
            CaseData::File(p) => {
                let to = tmp.join(name);
                std::fs::copy(p, &to).map_err(|e| io("write", &to, e))?;
            }
        }
    }
    let index = Index {
        version: VERSION,
        model_hash: model_hash(model),
        cases: results.cases.len(),
        modes: results.modes.len(),
    };
    write(
        "index.json".into(),
        serde_json::to_string(&index)
            .expect("index serializes")
            .as_bytes(),
    )?;
    // Cases read from the folder being replaced keep their paths: the new folder has the same names.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::rename(&tmp, &dir).map_err(|e| io("save", &dir, e))
}

/// The saved results of `model` (read from `model_path`), when the folder is there, in this format and
/// belongs to it: the summary is read, the cases stay in their files.
pub fn load(model_path: &Path, model: &Model) -> Option<Stored> {
    let dir = results_dir(model_path);
    let index: Index =
        serde_json::from_str(&std::fs::read_to_string(dir.join("index.json")).ok()?).ok()?;
    if index.version != VERSION || index.model_hash != model_hash(model) {
        return None;
    }
    let summary = raw(std::fs::read_to_string(dir.join("summary.json")).ok()?);
    let files = |prefix: &str, n: usize| -> Vec<CaseData> {
        (0..n)
            .map(|k| CaseData::File(dir.join(format!("{prefix}-{k}.bin"))))
            .collect()
    };
    let cases = files("case", index.cases);
    let modes = files("mode", index.modes);
    cases
        .iter()
        .chain(&modes)
        .all(|c| matches!(c, CaseData::File(p) if p.is_file()))
        .then_some(Stored {
            summary,
            cases,
            modes,
            own_dir: None,
        })
}
