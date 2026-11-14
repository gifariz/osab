//! Load combinations: each type against values worked out from the load-case results.

use osab_solver::results::{CaseResults, Step};
use osab_solver::{analyze, Model};

/// A 4 m cantilever along X (fixed at node 1) with a 2 m long side arm, and three load cases. Case B
/// has a point load inside the cantilever, so its stations differ from A's.
fn model(combos: &str) -> Model {
    let text = format!(
        r#"{{
      "materials": [{{"id": "S", "E": 2e8, "nu": 0.3}}],
      "sections": [{{"id": "R", "A": 0.01, "I22": 1e-5, "I33": 2e-5, "J": 1e-5}}],
      "nodes": [{{"id": "1"}}, {{"id": "2", "x": 4}}, {{"id": "3", "x": 4, "y": 2}}],
      "frames": [
        {{"id": "B1", "type": "frame", "nodes": ["1", "2"], "material": "S", "section": "R"}},
        {{"id": "B2", "type": "frame", "nodes": ["2", "3"], "material": "S", "section": "R"}}],
      "supports": [{{"node": "1", "fix": "fixed"}}],
      "load_cases": [
        {{"id": "A", "node_loads": [{{"node": "3", "fz": -10}}]}},
        {{"id": "B", "node_loads": [{{"node": "2", "fx": 5, "fy": 3}}],
          "frame_loads": [{{"type": "point", "frame": "B1", "dir": "Z", "p": -8, "at": 1.5}}]}},
        {{"id": "C", "frame_loads": [{{"type": "uniform", "frame": "B2", "dir": "Z", "w": -4}}]}}],
      "load_combinations": {combos}
    }}"#
    );
    Model::from_json(&text).unwrap()
}

const COMBOS: &str = r#"[
  {"id": "L1", "type": "linear-add", "items": [{"case": "A", "scale": 1.2}, {"case": "B", "scale": 1.6}]},
  {"id": "L2", "type": "linear-add", "items": [{"combo": "L1", "scale": 0.5}, {"case": "C", "scale": -1}]},
  {"id": "ENV", "type": "envelope", "items": [{"combo": "L1", "scale": 1}, {"case": "C", "scale": 1}, {"case": "B", "scale": -1}]},
  {"id": "LENV", "type": "linear-add", "items": [{"combo": "ENV", "scale": -1}, {"case": "A", "scale": 1}]}
]"#;

fn get<'a>(r: &'a [CaseResults], id: &str, step: Option<Step>) -> &'a CaseResults {
    r.iter()
        .find(|c| c.id.to_string() == id && c.step == step)
        .unwrap()
}

/// Every output value of a result at a given set of query points: displacements, reactions, frame
/// forces at x along each frame (both sides of B's point load), shell-free.
fn sample(c: &CaseResults) -> Vec<f64> {
    let mut v: Vec<f64> = c
        .displacements
        .iter()
        .flat_map(|d| d.values.as_array())
        .collect();
    v.extend(c.reactions.iter().flat_map(|r| r.values.as_array()));
    for e in &c.frames {
        // Station positions of both frames (envelopes exist only there).
        let xs: &[f64] = if e.length > 3.0 {
            &[0.0, 0.8, 1.5, 2.0, 4.0]
        } else {
            &[0.0, 0.8, 1.2, 2.0]
        };
        for &x in xs {
            v.extend(e.forces_at(x));
        }
    }
    v
}

fn close(a: &[f64], b: &[f64], what: &str) {
    assert_eq!(a.len(), b.len());
    let scale = b.iter().fold(1e-12f64, |m, x| m.max(x.abs()));
    for (k, (x, y)) in a.iter().zip(b).enumerate() {
        assert!(
            (x - y).abs() <= 1e-9 * scale,
            "{what} value {k}: {x} vs {y}"
        );
    }
}

#[test]
fn every_combination_type_matches_the_cases() {
    let r = analyze(&model(COMBOS)).unwrap();
    let ids: Vec<String> = r
        .cases
        .iter()
        .map(|c| format!("{}{:?}", c.id, c.step))
        .collect();
    assert_eq!(
        ids,
        [
            "ANone",
            "BNone",
            "CNone",
            "L1None",
            "L2None",
            "ENVSome(Max)",
            "ENVSome(Min)",
            "LENVSome(Max)",
            "LENVSome(Min)"
        ]
    );
    let c = |id| sample(get(&r.cases, id, None));
    let (a, b, cc) = (c("A"), c("B"), c("C"));
    let n = a.len();
    let lin = |f: &dyn Fn(usize) -> f64| (0..n).map(f).collect::<Vec<f64>>();

    // Linear Add: one value, solved as a case (with its own equilibrium check), nested factors multiply.
    let l1 = lin(&|k| 1.2 * a[k] + 1.6 * b[k]);
    close(&c("L1"), &l1, "L1");
    close(&c("L2"), &lin(&|k| 0.5 * l1[k] - cc[k]), "L2");
    assert!(get(&r.cases, "L1", None).equilibrium_ok());

    // Envelope (a negative factor swaps the part's max and min; here the parts have one value).
    let parts = [l1.clone(), cc.clone(), lin(&|k| -b[k])];
    let env_max = lin(&|k| parts.iter().map(|p| p[k]).fold(f64::NEG_INFINITY, f64::max));
    let env_min = lin(&|k| parts.iter().map(|p| p[k]).fold(f64::INFINITY, f64::min));
    let s = |id, step| sample(get(&r.cases, id, Some(step)));
    close(&s("ENV", Step::Max), &env_max, "ENV max");
    close(&s("ENV", Step::Min), &env_min, "ENV min");
    assert!(get(&r.cases, "ENV", Some(Step::Max)).equilibrium.is_none());

    // Linear Add of an envelope: -1 x ENV turns ENV's min into the max.
    close(
        &s("LENV", Step::Max),
        &lin(&|k| -env_min[k] + a[k]),
        "LENV max",
    );
    close(
        &s("LENV", Step::Min),
        &lin(&|k| -env_max[k] + a[k]),
        "LENV min",
    );
}

#[test]
fn srss_is_plus_and_minus_the_root_of_the_squares() {
    let r = analyze(&model(&COMBOS.replace(
        "
]",
        r#",
  {"id": "S1", "type": "srss", "items": [{"case": "A", "scale": 1.2}, {"case": "B", "scale": -0.5}, {"case": "C", "scale": 1}]},
  {"id": "S2", "type": "srss", "items": [{"combo": "ENV", "scale": 2}, {"case": "A", "scale": 1}]},
  {"id": "LS", "type": "linear-add", "items": [{"combo": "S1", "scale": -0.5}, {"case": "C", "scale": 1}]},
  {"id": "ES", "type": "envelope", "items": [{"combo": "S1", "scale": 1}, {"combo": "L1", "scale": 1}]},
  {"id": "SS", "type": "srss", "items": [{"combo": "S1", "scale": 1}, {"case": "B", "scale": 3}]}
]"#,
    )))
    .unwrap();
    let c = |id| sample(get(&r.cases, id, None));
    let s = |id, step| sample(get(&r.cases, id, Some(step)));
    let (a, b, cc, l1) = (c("A"), c("B"), c("C"), c("L1"));
    let n = a.len();
    let lin = |f: &dyn Fn(usize) -> f64| (0..n).map(f).collect::<Vec<f64>>();
    let both = |id, hi: &[f64]| {
        close(&s(id, Step::Max), hi, id);
        close(&s(id, Step::Min), &lin(&|k| -hi[k]), id);
    };
    // Of load cases: the sign of a factor makes no difference.
    let s1 = lin(&|k| ((1.2 * a[k]).powi(2) + (0.5 * b[k]).powi(2) + cc[k].powi(2)).sqrt());
    both("S1", &s1);
    // A part with a maximum and a minimum counts with the larger size of the two.
    let (emax, emin) = (s("ENV", Step::Max), s("ENV", Step::Min));
    both(
        "S2",
        &lin(&|k| ((2.0 * emax[k].abs().max(emin[k].abs())).powi(2) + a[k].powi(2)).sqrt()),
    );
    // Inside a Linear Add or an envelope it is +R and -R; a negative factor swaps them.
    close(
        &s("LS", Step::Max),
        &lin(&|k| 0.5 * s1[k] + cc[k]),
        "LS max",
    );
    close(
        &s("LS", Step::Min),
        &lin(&|k| -0.5 * s1[k] + cc[k]),
        "LS min",
    );
    close(&s("ES", Step::Max), &lin(&|k| s1[k].max(l1[k])), "ES max");
    close(
        &s("ES", Step::Min),
        &lin(&|k| (-s1[k]).min(l1[k])),
        "ES min",
    );
    both(
        "SS",
        &lin(&|k| (s1[k].powi(2) + (3.0 * b[k]).powi(2)).sqrt()),
    );
    assert!(get(&r.cases, "S1", Some(Step::Max)).equilibrium.is_none());
}

#[test]
fn envelope_stations_include_every_part_s_point_loads_and_deflections() {
    let r = analyze(&model(COMBOS)).unwrap();
    let env = get(&r.cases, "ENV", Some(Step::Max));
    let b1 = env
        .frames
        .iter()
        .find(|e| e.element.to_string() == "B1")
        .unwrap();
    // 11 equally spaced stations plus both sides of B's point load at 1.5 m.
    assert_eq!(b1.stations.len(), 13);
    assert_eq!(
        b1.stations
            .iter()
            .filter(|s| (s.x - 1.5).abs() < 1e-12)
            .count(),
        2
    );
    // Station deflections are enveloped too: uz at the tip is the largest of the parts'.
    let tip = |c: &CaseResults| {
        c.frames
            .iter()
            .find(|e| e.element.to_string() == "B1")
            .unwrap()
            .stations
            .last()
            .unwrap()
            .uz
    };
    let l1 = tip(get(&r.cases, "L1", None));
    let cc = tip(get(&r.cases, "C", None));
    let b = tip(get(&r.cases, "B", None));
    assert!((b1.stations.last().unwrap().uz - l1.max(cc).max(-b)).abs() < 1e-12);
}

#[test]
fn bad_combinations_are_rejected_with_a_message() {
    let err = |combos: &str| {
        let full = format!(
            r#"{{"materials": [], "sections": [], "nodes": [], "load_cases": [{{"id": "A"}}], "load_combinations": {combos}}}"#
        );
        Model::from_json(&full).unwrap_err().to_string()
    };
    assert!(
        err(r#"[{"id": "X", "type": "linear-add", "items": [{"case": "Q", "scale": 1}]}]"#)
            .contains("unknown load case Q")
    );
    assert!(
        err(r#"[{"id": "X", "type": "linear-add", "items": [{"combo": "Q", "scale": 1}]}]"#)
            .contains("unknown load combination Q")
    );
    assert!(
        err(r#"[{"id": "A", "type": "envelope", "items": [{"case": "A", "scale": 1}]}]"#)
            .contains("same name")
    );
    assert!(err(
        r#"[{"id": "X", "type": "envelope", "items": [{"case": "A", "combo": "X", "scale": 1}]}]"#
    )
    .contains("one case or one combo"));
    let looped = r#"[{"id": "X", "type": "linear-add", "items": [{"combo": "Y", "scale": 1}]},
                     {"id": "Y", "type": "envelope", "items": [{"combo": "X", "scale": 1}]}]"#;
    assert!(err(looped).contains("contains itself"));
    assert!(
        err(r#"[{"id": "X", "type": "range-add", "items": [{"case": "A", "scale": 1}]}]"#)
            .contains("unknown variant")
    );
}

#[test]
fn results_handed_on_one_by_one_come_in_order_with_the_same_values() {
    // The envelope comes before the one-valued combination it is made of: it waits for L1's solve, and
    // L1 then follows it.
    let m = model(
        r#"[
      {"id": "E", "type": "envelope", "items": [{"combo": "L1", "scale": 1}, {"case": "C", "scale": 1}]},
      {"id": "L1", "type": "linear-add", "items": [{"case": "A", "scale": 1.2}, {"case": "B", "scale": 1.6}]},
      {"id": "L2", "type": "linear-add", "items": [{"combo": "L1", "scale": 0.5}, {"case": "C", "scale": -1}]}
    ]"#,
    );
    let mut each = Vec::new();
    let rest = osab_solver::analyze_each(&m, &mut |c| {
        each.push(c);
        Ok(())
    })
    .unwrap();
    assert!(rest.cases.is_empty());
    let order: Vec<String> = each
        .iter()
        .map(|c| {
            format!(
                "{}{}",
                c.id,
                c.step
                    .map_or("", |s| if s == Step::Max { "+" } else { "-" })
            )
        })
        .collect();
    assert_eq!(order, ["A", "B", "C", "E+", "E-", "L1", "L2"]);
    let all = analyze(&m).unwrap();
    let json = |c: &CaseResults| serde_json::to_string(c).unwrap();
    assert_eq!(
        each.iter().map(json).collect::<Vec<_>>(),
        all.cases.iter().map(json).collect::<Vec<_>>()
    );
}

/// Combinations listed before the cases and combinations they are made of: every result is the same,
/// to the last digit, as with the combinations listed after their parts, and the results come in the
/// listed order. The envelopes have stations of their own (B's point load is in some, not in others),
/// one is part of others, and a part is used by several.
#[test]
fn combinations_listed_before_their_parts_give_the_same_results() {
    let combos = [
        r#"{"id": "L1", "type": "linear-add", "items": [{"case": "A", "scale": 1.2}, {"case": "B", "scale": 1.6}]}"#,
        r#"{"id": "L2", "type": "linear-add", "items": [{"combo": "L1", "scale": 0.5}, {"case": "C", "scale": -1}]}"#,
        r#"{"id": "L3", "type": "linear-add", "items": [{"case": "A", "scale": 0.9}, {"case": "C", "scale": 1.5}]}"#,
        r#"{"id": "ENV", "type": "envelope", "items": [{"combo": "L1", "scale": 1}, {"case": "C", "scale": 1}, {"case": "B", "scale": -1}]}"#,
        r#"{"id": "ENV2", "type": "envelope", "items": [{"combo": "L3", "scale": 1}, {"case": "A", "scale": -2}, {"case": "C", "scale": 1}]}"#,
        r#"{"id": "LENV", "type": "linear-add", "items": [{"combo": "ENV", "scale": -1}, {"case": "A", "scale": 1}, {"combo": "L2", "scale": 0.3}]}"#,
        r#"{"id": "SR", "type": "srss", "items": [{"combo": "L2", "scale": 1}, {"combo": "ENV2", "scale": 1}, {"case": "A", "scale": 1}, {"combo": "L2", "scale": -0.5}]}"#,
        r#"{"id": "ALL", "type": "envelope", "items": [{"combo": "SR", "scale": 1}, {"combo": "LENV", "scale": 1}, {"combo": "L3", "scale": 1}, {"combo": "ENV", "scale": 0.5}]}"#,
    ];
    let run = |order: &[usize]| -> Vec<(String, String)> {
        let list: Vec<&str> = order.iter().map(|&k| combos[k]).collect();
        let m = model(&format!("[{}]", list.join(",")));
        let mut each = Vec::new();
        osab_solver::analyze_each(&m, &mut |c| {
            let name = format!("{}{:?}", c.id, c.step);
            each.push((name, serde_json::to_string(&c).unwrap()));
            Ok(())
        })
        .unwrap();
        each
    };
    let names = |r: &[(String, String)]| -> Vec<String> { r.iter().map(|x| x.0.clone()).collect() };
    let listed = run(&[0, 1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(
        names(&listed)[3..],
        [
            "L1None",
            "L2None",
            "L3None",
            "ENVSome(Max)",
            "ENVSome(Min)",
            "ENV2Some(Max)",
            "ENV2Some(Min)",
            "LENVSome(Max)",
            "LENVSome(Min)",
            "SRSome(Max)",
            "SRSome(Min)",
            "ALLSome(Max)",
            "ALLSome(Min)"
        ]
    );
    for order in [
        [7, 6, 5, 4, 3, 2, 1, 0],
        [5, 7, 0, 6, 3, 1, 4, 2],
        [3, 4, 7, 2, 6, 0, 5, 1],
    ] {
        let other = run(&order);
        // The listed order: the three load cases, then each combination (max then min) as listed.
        let mut want: Vec<String> = names(&listed)[..3].to_vec();
        for k in order {
            let id = combos[k].split('"').nth(3).unwrap();
            want.extend(names(&listed).into_iter().filter(|n| {
                n.strip_prefix(id)
                    .is_some_and(|rest| !rest.starts_with(char::is_numeric))
            }));
        }
        assert_eq!(names(&other), want, "order {order:?}");
        for (name, json) in &other {
            let same = &listed.iter().find(|x| &x.0 == name).unwrap().1;
            assert!(
                json == same,
                "{name} differs with the combinations listed as {order:?}"
            );
        }
    }
}

#[test]
fn a_combination_with_no_load_cases_is_left_out_with_a_warning() {
    // E has no load cases and U uses it: both are left out, named once in a warning; the rest run.
    let r = analyze(&model(
        r#"[
      {"id": "E", "type": "linear-add", "items": []},
      {"id": "U", "type": "linear-add", "items": [{"combo": "E", "scale": 1}, {"case": "A", "scale": 1}]},
      {"id": "L1", "type": "linear-add", "items": [{"case": "A", "scale": 1.2}, {"case": "B", "scale": 1.6}]}
    ]"#,
    ))
    .unwrap();
    let ids: Vec<String> = r.cases.iter().map(|c| c.id.to_string()).collect();
    assert_eq!(ids, ["A", "B", "C", "L1"]);
    let warned: Vec<&String> = r
        .notes
        .iter()
        .filter(|n| n.contains("no load cases in them"))
        .collect();
    assert_eq!(warned.len(), 1, "{:?}", r.notes);
    assert!(
        warned[0].contains(": E;") && warned[0].contains("combinations that use them: U"),
        "{}",
        warned[0]
    );
}
