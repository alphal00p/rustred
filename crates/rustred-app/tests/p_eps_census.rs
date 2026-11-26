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

#[test]
fn p_eps_census_refuses_bad_arguments() {
    let run = |arguments: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_rustred"))
            .arg("p-eps-census")
            .args(arguments)
            .output()
            .unwrap()
    };
    for arguments in [
        &["--package", "p", "--out-dir", "o"][..],
        &["--package", "p", "--max-total", "7", "--out-dir", "o"],
        &["--package", "p", "--max-total", "0", "--out-dir", "o"],
        &["--unknown"],
    ] {
        let output = run(arguments);
        assert!(!output.status.success(), "{arguments:?} was accepted");
        assert!(!output.stderr.is_empty(), "{arguments:?}: no message");
    }
    // a directory without library.json is an input error, not a census
    let directory = std::env::temp_dir().join(format!("p-eps-census-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let missing = directory.join("no-package");
    let output = run(&[
        "--package",
        missing.to_str().unwrap(),
        "--max-total",
        "2",
        "--out-dir",
        directory.join("out").to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    std::fs::remove_dir_all(&directory).unwrap();
}
