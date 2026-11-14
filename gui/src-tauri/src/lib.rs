//! OSAB desktop app: the Tauri window plus the commands the web UI calls.
//! The commands are thin wrappers around `osab_mesh::api` (mesh step, then the solver) and
//! `osab_solver::api`; errors go back to the UI as data. The results stay here (`results_store`): the UI
//! gets a summary and asks for each case it shows.

mod case_codec;
mod results_store;

use std::path::Path;

use osab_solver::api::{self, ErrorReport, Oriented};
use osab_solver::Model;
use results_store::{Collector, ResultsStore};
use serde_json::value::RawValue;

type Store<'a> = tauri::State<'a, ResultsStore>;

/// Meshes, checks and analyses the model (runs off the UI thread). Returns the results' summary.
#[tauri::command(async)]
fn analyze(
    model: serde_json::Value,
    force: Option<bool>,
    store: Store,
) -> Result<Box<RawValue>, ErrorReport> {
    // `force`: go on even when the estimate says the analysis will not fit in memory (*Run anyway* was
    // chosen in the app). Only for this run.
    osab_solver::memory::set_force(force.unwrap_or(false));
    let r = run_analysis(model, &store);
    osab_solver::memory::set_force(false);
    r
}

fn run_analysis(
    model: serde_json::Value,
    store: &ResultsStore,
) -> Result<Box<RawValue>, ErrorReport> {
    run_analysis_with(model, store, results_store::MEMORY_LIMIT)
}

/// The run, each case turned into JSON as the solver hands it on (past `limit` bytes, into files).
fn run_analysis_with(
    model: serde_json::Value,
    store: &ResultsStore,
    limit: usize,
) -> Result<Box<RawValue>, ErrorReport> {
    // The previous results go first: their files are in the same folder.
    *store.0.lock().unwrap() = None;
    static RUN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let run = RUN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("osab-results-{}-{run}", std::process::id()));
    let mut collect = Collector::new(dir, limit);
    let rest = osab_mesh::api::analyze_value_each(model, None, &mut |c| {
        collect.push(c);
        Ok(())
    })?;
    let stored = collect.finish(&rest)?;
    let summary = stored.summary.clone();
    *store.0.lock().unwrap() = Some(stored);
    Ok(summary)
}

/// One case's values (its index in the summary's cases) of the results kept here.
#[tauri::command(async)]
fn result_case(index: usize, store: Store) -> Result<tauri::ipc::Response, ErrorReport> {
    case_of(index, &store).map(tauri::ipc::Response::new)
}

fn case_of(index: usize, store: &ResultsStore) -> Result<Vec<u8>, ErrorReport> {
    match store.0.lock().unwrap().as_ref() {
        Some(s) => s.case(index),
        None => Err(ErrorReport::io(format!(
            "no results for case {index}: run the analysis again"
        ))),
    }
}

/// One mode's shape (its index in the summary's modes) of the results kept here.
#[tauri::command(async)]
fn result_mode(index: usize, store: Store) -> Result<tauri::ipc::Response, ErrorReport> {
    mode_of(index, &store).map(tauri::ipc::Response::new)
}

fn mode_of(index: usize, store: &ResultsStore) -> Result<Vec<u8>, ErrorReport> {
    match store.0.lock().unwrap().as_ref() {
        Some(s) => s.mode(index),
        None => Err(ErrorReport::io(format!(
            "no shape for mode {}: run the analysis again",
            index + 1
        ))),
    }
}

/// Deletes the results of the cases at `indices` (the summary's order) and, with `modal`, the modal
/// results (Analyze > Load Cases to Run).
#[tauri::command(async)]
fn delete_results(indices: Vec<usize>, modal: bool, store: Store) {
    delete_results_in(&indices, modal, &store);
}

fn delete_results_in(indices: &[usize], modal: bool, store: &ResultsStore) {
    if let Some(s) = store.0.lock().unwrap().as_mut() {
        s.delete(indices, modal);
    }
}

/// The saved results of the model file at `path` when they belong to it (then kept here as the current
/// results); none otherwise.
#[tauri::command(async)]
fn open_results(path: String, store: Store) -> Result<Option<Box<RawValue>>, ErrorReport> {
    saved_results(Path::new(&path), &store)
}

fn saved_results(path: &Path, store: &ResultsStore) -> Result<Option<Box<RawValue>>, ErrorReport> {
    let model = read_model(path)?;
    let Some(stored) = results_store::load(path, &model) else {
        return Ok(None);
    };
    let summary = stored.summary.clone();
    *store.0.lock().unwrap() = Some(stored);
    Ok(Some(summary))
}

/// The analysis mesh only (View > Show Analysis Mesh before a run).
#[tauri::command(async)]
fn mesh_model(model: serde_json::Value) -> Result<osab_mesh::api::MeshView, ErrorReport> {
    osab_mesh::api::mesh_value(model)
}

/// Applies the frame end-order rule, mirroring member loads of reversed members.
#[tauri::command]
fn orient_model(model: serde_json::Value) -> Result<Oriented, ErrorReport> {
    api::orient_value(model)
}

/// A section shape's properties and outline (Define > Frame Sections shows them).
#[tauri::command]
fn section_shape(
    input: serde_json::Value,
) -> Result<osab_mesh::section::SectionProps, ErrorReport> {
    osab_mesh::section::section_value(input)
}

/// Writes a file the window made (Display > Tables > Export): the bytes are the request's body, the path
/// its `path` header (percent-encoded UTF-8).
#[tauri::command]
fn write_file(request: tauri::ipc::Request) -> Result<(), ErrorReport> {
    let tauri::ipc::InvokeBody::Raw(data) = request.body() else {
        return Err(ErrorReport::io("write_file: no file data".into()));
    };
    let path = request
        .headers()
        .get("path")
        .and_then(|v| v.to_str().ok())
        .map(percent_decode)
        .ok_or_else(|| ErrorReport::io("write_file: no path".into()))?;
    std::fs::write(&path, data).map_err(|e| ErrorReport::io(format!("cannot write {path}: {e}")))
}

/// `%XX` escapes decoded (UTF-8).
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The steel section list's names and types (Define > Frame Sections > From the steel list).
#[tauri::command]
fn section_list() -> Vec<(&'static str, &'static str)> {
    osab_mesh::section::section_list()
}

/// A spectrum function's points (Define > Functions > Response Spectrum draws them).
#[tauri::command]
fn spectrum_function(function: serde_json::Value) -> Result<api::SpectrumPoints, ErrorReport> {
    api::spectrum_value(function)
}

/// The model file the app was started with: the first argument when it is an existing `.osab` or `.json`
/// file (Windows passes a double-clicked model file this way).
#[tauri::command]
fn startup_file() -> Option<String> {
    let arg = std::env::args().nth(1)?;
    let path = Path::new(&arg);
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    (path.is_file() && (ext == "osab" || ext == "json")).then_some(arg)
}

/// The model file's JSON as it is (checked by the solver's reader): the app's own fields, such as labels
/// and special joints, stay.
#[tauri::command(async)]
fn open_model(path: String) -> Result<serde_json::Value, ErrorReport> {
    let text = read_text(Path::new(&path))?;
    api::parse_model(&text)?;
    serde_json::from_str(&text).map_err(|e| osab_solver::Error::Json(e).into())
}

/// Saves the model; with `with_results` (the results kept here belong to it) the results go next to it.
#[tauri::command(async)]
fn save_model(
    path: String,
    model: serde_json::Value,
    with_results: Option<bool>,
    store: Store,
) -> Result<(), ErrorReport> {
    save_with_results(
        Path::new(&path),
        model,
        with_results.unwrap_or(false),
        &store,
    )
}

fn save_with_results(
    path: &Path,
    model: serde_json::Value,
    with_results: bool,
    store: &ResultsStore,
) -> Result<(), ErrorReport> {
    // Checked by the solver's reader, then written as the app sent it (its own fields stay).
    let _: Model = serde_json::from_value(model.clone())
        .map_err(|e| ErrorReport::from(osab_solver::Error::Json(e)))?;
    let text = serde_json::to_string_pretty(&model).expect("JSON serializes");
    std::fs::write(path, text + "\n")
        .map_err(|e| ErrorReport::io(format!("cannot save {}: {e}", path.display())))?;
    if with_results {
        if let Some(stored) = store.0.lock().unwrap().as_ref() {
            // Fingerprinted as the file reads back, so reopening it finds them.
            results_store::save(path, &read_model(path)?, stored)?;
        }
    }
    Ok(())
}

fn read_text(path: &Path) -> Result<String, ErrorReport> {
    std::fs::read_to_string(path)
        .map_err(|e| ErrorReport::io(format!("cannot open {}: {e}", path.display())))
}

fn read_model(path: &Path) -> Result<Model, ErrorReport> {
    api::parse_model(&read_text(path)?)
}

pub fn run() {
    // Every core for the analysis: the results are the same on any number of threads.
    osab_solver::parallel::set_threads(0);
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ResultsStore::default())
        .invoke_handler(tauri::generate_handler![
            analyze,
            result_case,
            result_mode,
            delete_results,
            open_results,
            mesh_model,
            orient_model,
            spectrum_function,
            section_shape,
            section_list,
            write_file,
            open_model,
            save_model,
            startup_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running OSAB");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example() -> serde_json::Value {
        let text = include_str!("../../../examples/portal_frame.osab");
        serde_json::from_str(text).unwrap()
    }

    fn json(raw: &RawValue) -> serde_json::Value {
        serde_json::from_str(raw.get()).unwrap()
    }

    /// Case `k` of the results kept in `store`, read back as JSON the way the window reads it.
    fn case_json(k: usize, store: &ResultsStore) -> serde_json::Value {
        let bytes = case_of(k, store).unwrap();
        let guard = store.0.lock().unwrap();
        let summary = json(&guard.as_ref().unwrap().summary);
        case_codec::decode(&bytes, &summary["cases"][k], &summary["geometry"])
    }

    #[test]
    fn cases_read_back_from_their_bytes_equal_the_solver_results() {
        // Frames and shells (B24: a slab on columns).
        let slab: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/benchmarks/b24_slab_on_columns/model.json"
        ))
        .unwrap();
        // Spectrum cases, base reactions and story results (B64: a two-story building).
        let building: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/benchmarks/b64_spectrum_building/model.json"
        ))
        .unwrap();
        // Pier and spandrel forces (B67: a wall with a door and a header).
        let wall: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/benchmarks/b67_pier_spandrel/model.json"
        ))
        .unwrap();
        for model in [example(), slab, building, wall] {
            // One run: each case kept as it is and handed to the store.
            let mut cases = Vec::new();
            let mut collect = Collector::new(std::env::temp_dir(), usize::MAX);
            let rest = osab_mesh::api::analyze_value_each(model, None, &mut |c| {
                cases.push(c.clone());
                collect.push(c);
                Ok(())
            })
            .unwrap();
            let store = ResultsStore::default();
            *store.0.lock().unwrap() = Some(collect.finish(&rest).unwrap());
            for (k, c) in cases.iter().enumerate() {
                // Through JSON text (read exactly: `float_roundtrip`), as the summary's numbers are.
                let want: serde_json::Value =
                    serde_json::from_str(&serde_json::to_string(c).unwrap()).unwrap();
                assert_eq!(case_json(k, &store), want, "case {}", c.id);
                assert_eq!(&case_of(k, &store).unwrap()[..4], b"OWB1");
            }
        }
    }

    /// Mode `k`'s shape of the results kept in `store`, read back as JSON the way the window reads it.
    fn mode_json(k: usize, store: &ResultsStore) -> serde_json::Value {
        let bytes = mode_of(k, store).unwrap();
        let guard = store.0.lock().unwrap();
        let summary = json(&guard.as_ref().unwrap().summary);
        case_codec::decode_shape(&bytes, &summary["modal"]["joints"])
    }

    #[test]
    fn mode_shapes_come_one_by_one_and_read_back_equal_the_solver_results() {
        // B53: a building with a modal case of several modes.
        let model: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/benchmarks/b53_modal_building/model.json"
        ))
        .unwrap();
        let mut collect = Collector::new(std::env::temp_dir(), usize::MAX);
        let rest = osab_mesh::api::analyze_value_each(model.clone(), None, &mut |c| {
            collect.push(c);
            Ok(())
        })
        .unwrap();
        let store = ResultsStore::default();
        *store.0.lock().unwrap() = Some(collect.finish(&rest).unwrap());
        let summary = json(&store.0.lock().unwrap().as_ref().unwrap().summary);
        // Through JSON text, as the summary's numbers are: the modal results without the shapes.
        let want: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&rest.modal).unwrap()).unwrap();
        let modes = want["modes"].as_array().unwrap();
        assert!(modes.len() > 1);
        for (k, m) in modes.iter().enumerate() {
            let mut head = m.clone();
            head.as_object_mut().unwrap().remove("shape");
            assert_eq!(summary["modal"]["modes"][k], head, "mode {}", k + 1);
            assert_eq!(mode_json(k, &store), m["shape"], "mode {}", k + 1);
            assert_eq!(&mode_of(k, &store).unwrap()[..4], b"OWB1");
        }
        for key in [
            "id",
            "masses",
            "free_mass",
            "centre_of_mass",
            "load_participation",
        ] {
            assert_eq!(summary["modal"][key], want[key], "{key}");
        }
        assert_eq!(mode_of(modes.len(), &store).unwrap_err().kind, "io");
        // Saved with the model and reopened: the same shapes, from their files.
        let dir = std::env::temp_dir().join(format!("osab-gui-modes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("b53.json");
        save_with_results(&path, model, true, &store).unwrap();
        let opened = ResultsStore::default();
        assert_eq!(
            json(&saved_results(&path, &opened).unwrap().unwrap()),
            summary
        );
        for (k, m) in modes.iter().enumerate() {
            assert_eq!(mode_json(k, &opened), m["shape"]);
        }
        // Deleting the modal results deletes the shapes.
        delete_results_in(&[], true, &opened);
        assert_eq!(mode_of(0, &opened).unwrap_err().kind, "io");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn analyze_returns_a_summary_and_each_case_on_request() {
        let store = ResultsStore::default();
        let r = json(&run_analysis(example(), &store).unwrap());
        let cases = r["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 2);
        // The summary has each case's name and checks, not its values.
        assert!(cases
            .iter()
            .all(|c| c["equilibrium"].is_object() && c.get("displacements").is_none()));
        let first = case_json(0, &store);
        assert_eq!(first["id"], cases[0]["id"]);
        assert!(!first["displacements"].as_array().unwrap().is_empty());
        assert_eq!(case_of(2, &store).unwrap_err().kind, "io");
        // An unstable model runs with a warning by default; asked to stop, it is an error of its own kind.
        let mut bad = example();
        bad["supports"] = serde_json::json!([]);
        let warned = json(&run_analysis(bad.clone(), &store).unwrap());
        assert!(warned["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n.as_str().unwrap().contains("unstable")));
        bad["on_instability"] = serde_json::json!("stop");
        let err = run_analysis(bad, &store).unwrap_err();
        assert_eq!(err.kind, "unstable");
    }

    #[test]
    fn export_paths_are_percent_decoded() {
        assert_eq!(
            percent_decode("D%3A%5Cprojects%5Cb26%20tables.xlsx"),
            r"D:\projects\b26 tables.xlsx"
        );
        assert_eq!(percent_decode("caf%C3%A9%2"), "café%2");
    }

    #[test]
    fn deleted_results_close_up_and_are_not_saved() {
        let store = ResultsStore::default();
        run_analysis(example(), &store).unwrap();
        let second = case_json(1, &store);
        delete_results_in(&[0], true, &store);
        let lock = store.0.lock().unwrap();
        let summary = json(&lock.as_ref().unwrap().summary);
        drop(lock);
        assert_eq!(summary["cases"].as_array().unwrap().len(), 1);
        assert!(summary["modal"].is_null());
        assert_eq!(case_json(0, &store), second);
        assert_eq!(case_of(1, &store).unwrap_err().kind, "io");
        // Which cases run changes no result: saved results stay valid.
        let mut m: Model = serde_json::from_value(example()).unwrap();
        let h = results_store::model_hash(&m);
        m.not_run = vec![m.load_cases[0].id.clone()];
        assert_eq!(results_store::model_hash(&m), h);
    }

    #[test]
    fn results_saved_with_the_model_come_back_until_the_model_changes() {
        let dir = std::env::temp_dir().join(format!("osab-gui-results-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("portal.json");
        let store = ResultsStore::default();
        let summary = json(&run_analysis(example(), &store).unwrap());
        let case = case_json(1, &store);
        // Without results only the model is written.
        save_with_results(&path, example(), false, &store).unwrap();
        assert!(!dir.join("portal.results").exists());
        let mut with_gui = example();
        with_gui["gui"] = serde_json::json!({"units": "US"});
        save_with_results(&path, with_gui, true, &store).unwrap();
        assert!(dir.join("portal.results").join("index.json").exists());
        // Reopened: the same summary and cases (the app's gui part does not count).
        let fresh = ResultsStore::default();
        assert_eq!(
            json(&saved_results(&path, &fresh).unwrap().unwrap()),
            summary
        );
        assert_eq!(case_json(1, &fresh), case);
        // The model changed in the file: its old results are not used.
        let mut changed = example();
        changed["nodes"][1]["x"] = serde_json::json!(0.5);
        save_with_results(&path, changed, false, &store).unwrap();
        assert!(saved_results(&path, &ResultsStore::default())
            .unwrap()
            .is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_large_run_keeps_its_cases_on_disk_and_saves_and_reopens_them() {
        let dir = std::env::temp_dir().join(format!("osab-gui-spill-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("portal.json");
        let memory = ResultsStore::default();
        let summary = json(&run_analysis(example(), &memory).unwrap());
        // A limit of 0 bytes: every case goes to a file at once; the same summary and values.
        let disk = ResultsStore::default();
        assert_eq!(
            json(&run_analysis_with(example(), &disk, 0).unwrap()),
            summary
        );
        for k in 0..2 {
            assert_eq!(case_json(k, &disk), case_json(k, &memory));
        }
        // Saved from the files, reopened, saved again over the folder it was read from.
        save_with_results(&path, example(), true, &disk).unwrap();
        let opened = ResultsStore::default();
        saved_results(&path, &opened).unwrap().unwrap();
        save_with_results(&path, example(), true, &opened).unwrap();
        for k in 0..2 {
            assert_eq!(case_json(k, &opened), case_json(k, &memory));
        }
        // A new run's results replace the old ones, and the old run's case files go with them.
        let runs = || {
            let prefix = format!("osab-results-{}-", std::process::id());
            std::fs::read_dir(std::env::temp_dir())
                .unwrap()
                .filter(|e| {
                    e.as_ref()
                        .unwrap()
                        .file_name()
                        .to_string_lossy()
                        .starts_with(&prefix)
                })
                .count()
        };
        assert_eq!(runs(), 1);
        run_analysis_with(example(), &disk, 0).unwrap();
        assert_eq!(runs(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_then_open_gives_the_same_model() {
        let dir = std::env::temp_dir().join(format!("osab-gui-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("portal.json");
        let mut m = example();
        m["gui"] = serde_json::json!({"grid": {"x": [{"label": "A", "coord": 0}]}});
        // The app's own fields (labels, special joints) are kept as they are.
        m["nodes"][0]["label"] = serde_json::json!("J1");
        m["nodes"][0]["special"] = serde_json::json!(true);
        m["frames"][0]["label"] = serde_json::json!("C1");
        let store = ResultsStore::default();
        save_with_results(&path, m.clone(), false, &store).unwrap();
        let opened = open_model(path.display().to_string()).unwrap();
        assert_eq!(opened, m);
        let first = std::fs::read_to_string(&path).unwrap();
        save_with_results(&path, opened, false, &store).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), first);
        assert_eq!(
            open_model(dir.join("missing.json").display().to_string())
                .unwrap_err()
                .kind,
            "io"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn orient_model_applies_the_end_order() {
        let mut m = example();
        m["frames"][1]["nodes"] = serde_json::json!(["3", "2"]);
        let o = orient_model(m).unwrap();
        // The beam was given right-to-left; end i becomes the left (lower X) node.
        assert_eq!(o.reversed, vec![osab_solver::Id::from("2")]);
        assert_eq!(o.model.frames[1].nodes[0], osab_solver::Id::from("2"));
    }
}
