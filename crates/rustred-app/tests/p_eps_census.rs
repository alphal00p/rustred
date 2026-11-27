//! `rustred p-eps-census` (application::propagator::eps_census): the generated universe at one to three loops
//! against hand counts and the sectors of the generated tops, key decoding, and the census of small bounds
//! against a ForcerTree DAG of the generated tops; plus the command's argument errors through the binary.
use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

use rustred::propagator::topology::{ChildMode, GraphKey, PTopo, TopologyService, canonical};
use rustred_app::propagator::eps_census::{
    Cause, IndexedPresentation, LoopUniverse, PackageIndex, census_rows, decode_key,
    generated_top_dags, loop_universe, summary_tsv,
};

fn universes(max: u8) -> BTreeMap<u8, LoopUniverse> {
    (1..=max).map(|l| (l, loop_universe(l))).collect()
}

/// The index of the ForcerTree DAG of `tops`, every node presented once (ids in DAG order, kinds by eps).
fn dag_index(tops: &[PTopo]) -> PackageIndex {
    let dag = TopologyService::new().reduction_graph(tops, ChildMode::ForcerTree);
    let mut index = PackageIndex::default();
    for top in tops {
        *index.tops.entry(top.loops()).or_default() += 1;
    }
    for (id, node) in dag
        .nodes
        .iter()
        .filter(|n| !n.topo.lines.is_empty())
        .enumerate()
    {
        index.by_key.insert(
            node.key.as_bytes().to_vec(),
            vec![IndexedPresentation {
                id: id as u32,
                kind: if node.eps_lines > 0 { "eps" } else { "integer" }.to_owned(),
                loops: node.loops,
                line_eps: node.topo.lines.iter().map(|l| l.2).collect(),
            }],
        );
    }
    index
}

#[test]
fn small_universes_match_the_hand_count_and_the_top_sectors() {
    let u = universes(3);
    // one loop: the bubble; two loops: kite, sunset, bubble + line, two bubbles in a chain
    assert_eq!((u[&1].normal.len(), u[&1].tops.len()), (1, 1));
    assert_eq!((u[&2].normal.len(), u[&2].tops.len()), (4, 1));
    // three loops: Mincer's LA, BE, NO
    assert_eq!(u[&3].tops.len(), 3);
    // the ForcerTree DAG of the generated one- and two-loop tops is the 7-node DAG of the built-in P1L, P2L
    // (`rustred p-census` test)
    let dags = generated_top_dags(&u, &PackageIndex::default());
    assert_eq!(dags[1].1, 7);
    for universe in u.values() {
        assert_eq!(universe.not_normal_outside, 0);
        let normal: BTreeSet<GraphKey> = universe.normal.keys().cloned().collect();
        assert_eq!(normal, universe.top_sectors, "L = {}", universe.loops);
    }
}

#[test]
fn key_decoding_inverts_the_canonical_key() {
    let u = universes(3);
    for universe in u.values() {
        for (key, topo) in &universe.normal {
            let decoded = decode_key(key.as_bytes()).unwrap();
            assert_eq!(&decoded, topo);
            assert_eq!(&canonical(&decoded).0, key);
        }
    }
}

#[test]
fn census_of_small_bounds_against_a_dag_of_tops() {
    let u = universes(3);
    let tops = |max: u8| -> Vec<PTopo> {
        (u.values().filter(|x| x.loops <= max))
            .flat_map(|x| x.tops.iter().map(|k| x.normal[k].clone()))
            .collect()
    };
    // S = 2 with the tops of one and two loops: the bubble (0 or 1 unit) and the 4 two-loop graphs, all found
    let index = dag_index(&tops(2));
    let rows = census_rows(&u, &index, 2);
    assert_eq!(rows.len(), 2 + 4);
    assert!(
        rows.iter()
            .all(|r| r.presentation.is_some() && r.slots_match == Some(true))
    );
    // S = 3: the bubble with units {0,0},{1,0},{2,0},{1,1}; the two-loop graphs with at most one unit
    // (kite 1+2 orbits, chain 1+1, bubble + line 1+3, sunset 1+1); the three-loop normal forms
    let rows = census_rows(&u, &index, 3);
    let at = |l: u8| rows.iter().filter(|r| r.loops == l).count();
    assert_eq!((at(1), at(2), at(3)), (4, 11, u[&3].normal.len()));
    // without three-loop tops, every row with L + sum(u) = 3 is missing and above the tops
    for r in &rows {
        let expect_missing = r.loops + r.total == 3;
        assert_eq!(r.presentation.is_none(), expect_missing, "{r:?}");
        if expect_missing {
            assert_eq!(r.cause, Some(Cause::AboveTops));
        }
    }
    // with the three-loop tops: at most the rows that need two insertions of one node integrated in another
    // order are missing, and each missing row is classified
    let index = dag_index(&tops(3));
    let rows = census_rows(&u, &index, 3);
    for r in rows.iter().filter(|r| r.presentation.is_none()) {
        assert!(r.loops + r.total == 3 && r.total > 0, "{r:?}");
        assert!(matches!(
            r.cause,
            Some(Cause::ParentPresent | Cause::AncestorPresent { .. } | Cause::Unreached)
        ));
    }
    assert!(
        rows.iter()
            .filter(|r| r.presentation.is_some())
            .all(|r| r.slots_match == Some(true))
    );
    let summary = summary_tsv(&rows);
    assert!(
        summary
            .lines()
            .last()
            .unwrap()
            .starts_with(&format!("all\t-\t-\t-\t{}\t", rows.len()))
    );
}

fn run_census(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rustred"))
        .arg("p-eps-census")
        .args(arguments)
        .output()
        .unwrap()
}

/// A fresh scratch directory for one test.
fn scratch(name: &str) -> std::path::PathBuf {
    let directory =
        std::env::temp_dir().join(format!("p-eps-census-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

#[test]
fn p_eps_census_prints_its_help_and_refuses_bad_arguments() {
    for help in ["--help", "-h"] {
        let output = run_census(&[help]);
        assert_eq!(output.status.code(), Some(0));
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(
            text.contains("USAGE:") && text.contains("EXIT STATUS:"),
            "{text}"
        );
    }
    for arguments in [
        &["--package", "p", "--out-dir", "o"][..],
        &["--package", "p", "--max-total", "7", "--out-dir", "o"],
        &["--package", "p", "--max-total", "0", "--out-dir", "o"],
        &["--unknown"],
        &["--help", "--force"],
        &[
            "--package",
            "p",
            "--max-total",
            "2",
            "--out-dir",
            "o",
            "--axes",
            "--axes-extended",
        ],
        &[
            "--package",
            "p",
            "--max-total",
            "2",
            "--out-dir",
            "o",
            "--axes-max-loops",
            "3",
        ],
        &[
            "--package",
            "p",
            "--max-total",
            "2",
            "--out-dir",
            "o",
            "--axes",
            "--axes-max-loops",
            "0",
        ],
    ] {
        let output = run_census(arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(!output.stderr.is_empty(), "{arguments:?}: no message");
    }
    let directory = scratch("arguments");
    let out = directory.join("out");
    // a directory without library.json: exit 3 (input io), no census
    let missing = directory.join("no-package");
    let output = run_census(&[
        "--package",
        missing.to_str().unwrap(),
        "--max-total",
        "2",
        "--out-dir",
        out.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(3));
    // a malformed library.json: exit 4 (input)
    let malformed = directory.join("malformed");
    std::fs::create_dir_all(&malformed).unwrap();
    std::fs::write(malformed.join("library.json"), "{\"scope\": [1, 2]}").unwrap();
    let output = run_census(&[
        "--package",
        malformed.to_str().unwrap(),
        "--max-total",
        "2",
        "--out-dir",
        out.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(4));
    // an existing table without --force: exit 4 before anything is read
    std::fs::write(out.join("eps-census-S2.tsv"), "").unwrap();
    let output = run_census(&[
        "--package",
        missing.to_str().unwrap(),
        "--max-total",
        "2",
        "--out-dir",
        out.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(4));
    std::fs::remove_dir_all(&directory).unwrap();
}

/// A `library.json` with the one-loop top and the node of the one-loop bubble (the index the census reads).
fn one_loop_manifest() -> String {
    let bubble = PTopo::new(2, 0, 1, [(0, 1, 0), (0, 1, 0)]);
    let key = canonical(&bubble).0.to_hex();
    let line = r#"{"line": {"eps_units": 0, "momentum": [1, 0]}}"#;
    format!(
        r#"{{"identity": "test", "scope": [1, 1], "presentations": [
            {{"index": 0, "loops": 1, "kind": {{"top": {{"name": "P1L"}}}}, "slots": [{line}, {line}], "node": null}},
            {{"index": 1, "loops": 1, "kind": "insertion_adapted", "slots": [{line}, {line}], "node": {{"index": 0, "key": "{key}"}}}}
        ]}}"#
    )
}

#[test]
fn p_eps_census_exit_status_is_its_verdict() {
    let directory = scratch("verdict");
    let package = directory.join("package");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(package.join("library.json"), one_loop_manifest()).unwrap();
    let census = |s: &str, out: &str| {
        run_census(&[
            "--package",
            package.to_str().unwrap(),
            "--max-total",
            s,
            "--out-dir",
            directory.join(out).to_str().unwrap(),
        ])
    };
    // S = 1: the bubble without units is the package's node: complete, exit 0
    let output = census("1", "s1");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("s1/eps-census-S1.json")).unwrap())
            .unwrap();
    assert_eq!(
        (report["rows"].as_u64(), report["missing"].as_u64()),
        (Some(1), Some(0))
    );
    // S = 2: the bubble with one unit and the four two-loop graphs are missing: exit 1, tables written
    let output = census("2", "s2");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("s2/eps-census-S2.json")).unwrap())
            .unwrap();
    assert_eq!(
        (report["rows"].as_u64(), report["missing"].as_u64()),
        (Some(6), Some(5))
    );
    std::fs::remove_dir_all(&directory).unwrap();
}

/// The trace keys of `--axes` and `--axes-extended` on a two-loop sunset with one eps line (slots: lines k1, k2,
/// k1+k2-q with one unit, ISPs k1.q, k2.q): plain = the eps line at 0 and -1; extended = at -2, and at 0, -1, -2
/// with the numerator k1.q (first ISP at -1), alone and with a dot on the first integer line (k1 at power 2).
#[test]
fn the_axis_trace_keys_of_both_modes() {
    use rustred::algebra::CoefficientContext;
    use rustred::propagator::PresentationId;
    use rustred::propagator::presentation::{
        Presentation, PresentationKind, Slot, SlotIsp, SlotLine,
    };
    use rustred_app::propagator::eps_census::{AxesTrace, axis_trace_inputs};
    let line = |momentum: [i64; 3], eps_units: u8| {
        Slot::Line(SlotLine {
            momentum: momentum.to_vec(),
            eps_units,
        })
    };
    let isp = |left: [i64; 3]| {
        Slot::Isp(SlotIsp {
            left: left.to_vec(),
            right: vec![0, 0, 1],
        })
    };
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let sunset = Presentation::from_slots(
        PresentationId(7),
        PresentationKind::EpsNode,
        "sunset_eps",
        2,
        vec![
            line([1, 0, 0], 0),
            line([0, 1, 0], 0),
            line([1, 1, -1], 1),
            isp([1, 0, 0]),
            isp([0, 1, 0]),
        ],
        &context,
        None,
    )
    .unwrap();
    let keys = |mode| -> Vec<Vec<i64>> {
        axis_trace_inputs(&sunset, mode)
            .into_iter()
            .map(|(id, powers)| {
                assert_eq!(id, PresentationId(7));
                powers
            })
            .collect()
    };
    assert_eq!(
        keys(AxesTrace::Plain),
        [vec![1, 1, -1, 0, 0], vec![1, 1, 0, 0, 0]]
    );
    assert_eq!(
        keys(AxesTrace::Extended),
        [
            vec![1, 1, -2, -1, 0],
            vec![1, 1, -2, 0, 0],
            vec![1, 1, -1, -1, 0],
            vec![1, 1, 0, -1, 0],
            vec![2, 1, -2, -1, 0],
            vec![2, 1, -1, -1, 0],
            vec![2, 1, 0, -1, 0],
        ]
    );
}

/// `--trace-keys` lines: `<presentation> TAB <powers>`, comments and blanks skipped, duplicates once; malformed lines refused.
#[test]
fn trace_key_files_parse() {
    use rustred_app::propagator::eps_census::parse_trace_keys;
    let keys = parse_trace_keys("# comment\n3\t1,1,-2,0\n\n3\t1,1,-2,0\n0\t1, 2\n").unwrap();
    assert_eq!(keys, [(0, vec![1, 2]), (3, vec![1, 1, -2, 0])]);
    for bad in ["3 1,1", "x\t1", "3\t1,a"] {
        assert!(parse_trace_keys(bad).is_err(), "{bad:?}");
    }
}
