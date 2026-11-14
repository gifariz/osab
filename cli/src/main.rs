//! OSAB developer CLI.
//!
//!   osab solve <model.osab> [-o results.json] [--only parts] [--timing] [--threads N] [--memory GB] [--force]
//!                                                mesh and solve a model, print a summary, write results
//!                                                (compact JSON) case by case (--only: the parts of each case to write,
//!                                                of displacements, reactions, frames, links, shells, stories,
//!                                                piers (pier and spandrel forces),
//!                                                comma-separated; --timing: the time of each phase;
//!                                                --threads: threads for the solver, 0 = one per core
//!                                                (the default))
//!   osab mesh <model.osab> [-o analysis.json] [--timing]
//!                                                the mesh step only: print a summary, write the
//!                                                analysis model (joints, elements, frame pieces)
//!   osab bench [benchmarks-dir] [--threads N]   compare all benchmarks with their reference results
//!   osab api <analyze|orient|mesh>              JSON model on stdin -> {"ok": ...} or {"error": ...}
//!                                                on stdout (the same replies as the app's solver)

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use osab_solver::{api, bench, Model};

const USAGE: &str = "usage:\n  osab solve <model.osab> [-o results.json] [--only parts] [--timing] [--threads N] [--memory GB] [--force]\n  osab mesh <model.osab> [-o analysis.json] [--timing]\n  osab bench [benchmarks-dir] [--threads N]";

/// Reads the value of `--threads` and sets the solver's threads.
fn threads(value: Option<&String>) -> Result<(), String> {
    let n = value.and_then(|v| v.parse::<usize>().ok()).ok_or(USAGE)?;
    osab_solver::parallel::set_threads(n);
    Ok(())
}

#[cfg(feature = "mem-profile")]
mod mem;

#[cfg(feature = "mem-profile")]
#[global_allocator]
static COUNTING: mem::Counting = mem::Counting;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    #[cfg(feature = "mem-profile")]
    let _ = osab_solver::timing::MEMORY.set(mem::probe);
    // One thread per core unless --threads says otherwise (the results do not depend on it).
    osab_solver::parallel::set_threads(0);
    let result = match args.first().map(String::as_str) {
        Some("solve") => solve(&args[1..]),
        Some("mesh") => mesh(&args[1..]),
        Some("bench") => run_bench(&args[1..]),
        Some("api") => run_api(&args[1..]),
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

/// The parts of a case's results `--only` can keep.
const PARTS: [&str; 7] = [
    "displacements",
    "reactions",
    "frames",
    "links",
    "shells",
    "stories",
    "piers",
];

fn solve(args: &[String]) -> Result<(), String> {
    let mut input = None;
    let mut output = None;
    let mut timing = false;
    let mut only: Option<Vec<String>> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" => output = Some(PathBuf::from(it.next().ok_or(USAGE)?)),
            "--timing" => timing = true,
            "--threads" => threads(it.next())?,
            // The free memory to plan with, in GB, instead of what the system reports.
            "--memory" => {
                let gb = it
                    .next()
                    .and_then(|v| v.trim_end_matches(['G', 'g']).parse::<f64>().ok())
                    .filter(|v| *v > 0.0)
                    .ok_or(USAGE)?;
                osab_solver::memory::set_available(Some((gb * 1024.0 * 1024.0 * 1024.0) as u64));
            }
            // Go on even when the estimate says the analysis will not fit in memory.
            "--force" => osab_solver::memory::set_force(true),
            "--only" => {
                let list: Vec<String> = it
                    .next()
                    .ok_or(USAGE)?
                    .split(',')
                    .map(str::to_string)
                    .collect();
                if let Some(bad) = list.iter().find(|p| !PARTS.contains(&p.as_str())) {
                    return Err(format!(
                        "--only: unknown part {bad} (parts: {})",
                        PARTS.join(", ")
                    ));
                }
                only = Some(list);
            }
            _ if input.is_none() => input = Some(PathBuf::from(a)),
            _ => return Err(USAGE.to_string()),
        }
    }
    let input = input.ok_or(USAGE)?;
    if timing {
        osab_solver::timing::record();
    }
    let mut clock = osab_solver::timing::Clock::new();
    let text = std::fs::read_to_string(&input).map_err(|e| format!("{}: {e}", input.display()))?;
    let model: Model = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    clock.lap("read model");
    // Each case is summed up and written as soon as it is known (into a file of the cases, put together
    // with the rest at the end), so many load cases need the memory of a few.
    let mut lines: Vec<String> = Vec::new();
    let mut cases_file = match &output {
        Some(out) => {
            let path = with_suffix(out, ".cases.part");
            let f = std::fs::File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            Some((path, std::io::BufWriter::new(f), 0usize))
        }
        None => None,
    };
    let run = osab_mesh::run_each(&model, None, &mut |mut case| {
        let filter_time = osab_solver::timing::Span::new("results: filtering and summary");
        lines.push(summary_line(&case));
        let Some((_, w, n)) = &mut cases_file else {
            return Ok(());
        };
        if let Some(keep) = &only {
            let keep = |p: &str| keep.iter().any(|k| k == p);
            if !keep("displacements") {
                case.displacements.clear();
            }
            if !keep("reactions") {
                case.reactions.clear();
            }
            if !keep("frames") {
                case.frames.clear();
            }
            if !keep("links") {
                case.links.clear();
            }
            if !keep("shells") {
                case.shells.clear();
            }
            if !keep("stories") {
                case.joint_drifts.clear();
                case.story_drifts.clear();
                case.story_forces.clear();
            }
            if !keep("piers") {
                case.pier_forces.clear();
                case.spandrel_forces.clear();
            }
        }
        drop(filter_time);
        let _write_time = osab_solver::timing::Span::new("writing: case serialization");
        // Compact, as an element of the results' `cases`.
        (|| -> std::io::Result<()> {
            if *n > 0 {
                w.write_all(b",")?;
            }
            serde_json::to_writer(&mut *w, &case)?;
            Ok(())
        })()
        .map_err(|e| osab_solver::Error::Invalid(format!("results file: {e}")))?;
        *n += 1;
        Ok(())
    });
    let results = run.map_err(|e| {
        if let Some((path, _, _)) = &cases_file {
            let _ = std::fs::remove_file(path);
        }
        if timing {
            eprint!(
                "time per phase until it stopped:\n{}",
                osab_solver::timing::report(&osab_solver::timing::take())
            );
        }
        let hint = if matches!(e, osab_solver::Error::TooBigForMemory { .. }) {
            " (--force runs it anyway; --memory GB gives the free memory to plan with)"
        } else {
            ""
        };
        format!("{e}{hint}")
    })?;
    clock.lap("run (total)");

    println!(
        "{}: {} nodes, {} frames, {} load cases (units {}, {})",
        if model.title.is_empty() {
            input.display().to_string()
        } else {
            model.title.clone()
        },
        model.nodes.len(),
        model.frames.len(),
        model.load_cases.len(),
        model.units.force,
        model.units.length
    );
    for note in &results.notes {
        println!("note: {note}");
    }
    for line in &lines {
        println!("{line}");
    }
    clock.lap("summary");
    if let (Some(out), Some((path, w, n))) = (output, cases_file) {
        // The rest of the results with an empty `cases`, then the cases written so far in its place.
        let head = serde_json::to_string(&results).expect("results serialize");
        let key = ",\"cases\":";
        let at = head.find(&format!("{key}[]")).expect("results have cases") + key.len();
        let size = (|| -> std::io::Result<u64> {
            w.into_inner().map_err(|e| e.into_error())?;
            let mut f = std::io::BufWriter::new(std::fs::File::create(&out)?);
            f.write_all(&head.as_bytes()[..at])?;
            f.write_all(b"[")?;
            if n > 0 {
                std::io::copy(&mut std::fs::File::open(&path)?, &mut f)?;
            }
            f.write_all(b"]")?;
            f.write_all(&head.as_bytes()[at + 2..])?;
            f.flush()?;
            Ok(std::fs::metadata(&out)?.len())
        })()
        .map_err(|e| format!("{}: {e}", out.display()))?;
        let _ = std::fs::remove_file(&path);
        clock.lap("write results file");
        println!(
            "results written to {} ({:.1} MB)",
            out.display(),
            size as f64 * 1e-6
        );
    }
    if timing {
        print!(
            "time per phase:\n{}",
            osab_solver::timing::report(&osab_solver::timing::take())
        );
        #[cfg(feature = "mem-profile")]
        println!(
            "most memory in use at any time: {:.2} GB",
            mem::highest() as f64 / (1024.0 * 1024.0 * 1024.0)
        );
    }
    Ok(())
}

/// `path` with `suffix` added to its file name.
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

/// One line of the run's summary: a case's largest translation and its equilibrium check.
fn summary_line(case: &osab_solver::results::CaseResults) -> String {
    let largest = case
        .displacements
        .iter()
        .flat_map(|n| (0..3).map(move |d| (&n.node, d, n.values.as_array()[d])))
        .max_by(|a, b| a.2.abs().total_cmp(&b.2.abs()));
    let movement = match largest {
        Some((node, d, v)) => format!(
            "max translation {v:.6e} ({}) at node {node}",
            ["ux", "uy", "uz"][d]
        ),
        None => "no nodes".to_string(),
    };
    let name = match case.step {
        Some(step) => format!("{} ({step:?})", case.id),
        None => case.id.to_string(),
    };
    match &case.equilibrium {
        Some(eq) => {
            let ok = if eq.is_ok() { "ok" } else { "CHECK" };
            format!(
                "  case {name}: {movement}; equilibrium {ok} (error {:.1e}, residual {:.1e})",
                eq.relative_error, eq.residual
            )
        }
        None if case.spectrum => {
            let b = case.base_reaction.unwrap_or_default();
            format!(
                "  spectrum case {name}: {movement}; base reactions FX {:.6e}, FY {:.6e}",
                b[0], b[1]
            )
        }
        None => format!("  combination {name}: {movement}"),
    }
}

fn mesh(args: &[String]) -> Result<(), String> {
    let mut input = None;
    let mut output = None;
    let mut timing = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" => output = Some(PathBuf::from(it.next().ok_or(USAGE)?)),
            "--timing" => timing = true,
            _ if input.is_none() => input = Some(PathBuf::from(a)),
            _ => return Err(USAGE.to_string()),
        }
    }
    let input = input.ok_or(USAGE)?;
    let text = std::fs::read_to_string(&input).map_err(|e| format!("{}: {e}", input.display()))?;
    let model: Model = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if timing {
        osab_solver::timing::record();
    }
    let mesh_and_report = || {
        let m = osab_mesh::mesh(&model);
        if timing {
            print!(
                "time per phase:
{}",
                osab_solver::timing::report(&osab_solver::timing::take())
            );
        }
        m
    };
    let m = mesh_and_report().map_err(|e| e.to_string())?;
    println!(
        "{}: {} joints ({} made by the mesh step), {} shell elements, {} frame elements, {} edge constraints",
        input.display(),
        m.model.nodes.len(),
        m.info.joints.len(),
        m.model.areas.len(),
        m.model.frames.len(),
        m.model.edge_constraints.len()
    );
    for note in &m.notes {
        println!("note: {note}");
    }
    if let Some(out) = output {
        let json = api::model_to_json(&m.model);
        std::fs::write(
            &out,
            json + "
",
        )
        .map_err(|e| format!("{}: {e}", out.display()))?;
        println!("analysis model written to {}", out.display());
    }
    Ok(())
}

fn run_bench(args: &[String]) -> Result<(), String> {
    let mut root = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--threads" => threads(it.next())?,
            _ if root.is_none() => root = Some(PathBuf::from(a)),
            _ => return Err(USAGE.to_string()),
        }
    }
    let root = root.unwrap_or_else(|| Path::new("tests/benchmarks").to_path_buf());
    let run = |m: &Model| osab_mesh::run(m).map_err(|e| e.to_string());
    let checks = bench::run_all(&root, &run)?;
    print!("{}", bench::report(&checks));
    if checks.iter().all(|c| c.pass) {
        println!("\nall {} values within tolerance", checks.len());
        Ok(())
    } else {
        Err("some benchmark values are outside tolerance".into())
    }
}

/// Prints {"ok": value} or {"error": report}; both count as a successful run.
fn run_api(args: &[String]) -> Result<(), String> {
    let command = args.first().map(String::as_str).unwrap_or_default();
    if !osab_mesh::api::COMMANDS.contains(&command) {
        return Err(USAGE.to_string());
    }
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    println!("{}", osab_mesh::api::call(command, &text, None));
    Ok(())
}
