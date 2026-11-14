//! Runs every benchmark in tests/benchmarks (mesh step, then the solver) against its stored references.

use std::path::Path;

#[test]
fn all_benchmarks_match_references() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/benchmarks");
    let run = |m: &osab_solver::Model| osab_mesh::run(m).map_err(|e| e.to_string());
    let checks = osab_solver::bench::run_all(&root, &run).expect("benchmarks run");
    let report = osab_solver::bench::report(&checks);
    println!("{report}");
    assert!(!checks.is_empty(), "no benchmark values found");
    assert!(
        checks.iter().all(|c| c.pass),
        "benchmark mismatch:\n{report}"
    );
}
