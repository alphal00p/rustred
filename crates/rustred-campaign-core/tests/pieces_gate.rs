//! Lane pieces: the ship gate on constructed data (interfaces/pieces.md 2).

use std::cmp::Ordering;
use std::path::PathBuf;

use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::pieces::gate::GateError;
use rustred_campaign_core::pieces::{
    EdgeOrder, FnOrder, GateConfig, GateVerdict, NonZero, ShipGate,
};

fn spill(name: &str) -> PathBuf {
    let base = std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join(format!("pieces-gate-test-{}-{name}", std::process::id()))
}

fn cmp4() -> NodeComparator {
    NodeComparator::legacy(4, None).unwrap()
}

const LEAF: &[i32] = &[1, 1, 0, 0];

#[test]
fn descending_relations_pass() {
    let c = cmp4();
    let mut g = ShipGate::new(&c, 4, GateConfig::disk(spill("pass"))).unwrap();
    g.push_relation(
        &[3, 1, 0, 0],
        "r",
        [(&[2, 1, 0, 0][..], NonZero::Yes), (LEAF, NonZero::Unknown)],
    )
    .unwrap();
    g.push_relation(&[2, 1, 0, 0], "r", [(LEAF, NonZero::Yes)])
        .unwrap();
    g.push_relation(&[2, 1, -1, 0], "r", [(&[2, 1, 0, 0][..], NonZero::Yes)])
        .unwrap();
    let r = g.finish().unwrap();
    assert_eq!(r.verdict, GateVerdict::Pass);
    assert_eq!((r.relations, r.edges, r.keys), (3, 4, 4));
    assert_eq!(r.graph.kahn_processed, 4);
    assert_eq!(r.graph.residual, 0);
    assert!(r.graph.witness_cycle.is_empty());
    assert!(
        !spill("pass").read_dir().unwrap().any(|_| true),
        "spill files removed"
    );
}

#[test]
fn a_non_descending_edge_closing_a_cycle_fails_with_a_witness() {
    let c = cmp4();
    let (a, b): (&[i32], &[i32]) = (&[3, 1, 0, 0], &[2, 1, 0, 0]);
    let mut g = ShipGate::new(&c, 4, GateConfig::disk(spill("cyc"))).unwrap();
    g.push_relation(a, "table", [(b, NonZero::Yes)]).unwrap();
    g.push_relation(
        b,
        "hand-alias",
        [(a, NonZero::Unknown), (LEAF, NonZero::Yes)],
    )
    .unwrap();
    let r = g.finish().unwrap();
    assert_eq!(
        r.verdict,
        GateVerdict::Fail {
            non_descending: 1,
            cyclic: true
        }
    );
    assert_eq!(r.non_descending_listed.len(), 1);
    let e = &r.non_descending_listed[0];
    assert_eq!(
        (e.key.as_slice(), e.term.as_slice(), e.order),
        (b, a, EdgeOrder::Higher)
    );
    assert_eq!(e.provenance, "hand-alias");
    assert_eq!(r.by_provenance["hand-alias"], [2, 1]);
    assert_eq!(r.by_provenance["table"], [1, 0]);
    // the witness cycle: a -> b -> a (rotated to the smallest id = first key interned)
    let w = &r.graph.witness_cycle;
    assert_eq!(w.len(), 2);
    assert_eq!((w[0].from.as_slice(), w[0].to.as_slice()), (a, b));
    assert_eq!(w[0].provenance, "table");
    assert_eq!(w[1].order, EdgeOrder::Higher);
    assert_eq!(w[1].provenance, "hand-alias");
    assert_eq!(r.graph.sccs.cyclic, 1);
    // residual = on the cycle (a, b) or reachable from it (the leaf)
    assert_eq!(r.graph.residual, 3);
    assert_eq!(r.graph.sccs.cyclic_nodes, 2);
}

#[test]
fn a_non_descending_edge_without_a_cycle_fails_acyclic() {
    let c = cmp4();
    let mut g = ShipGate::new(&c, 4, GateConfig::disk(spill("nd"))).unwrap();
    g.push_relation(&[2, 1, 0, 0], "x", [(&[3, 1, 0, 0][..], NonZero::Yes)])
        .unwrap();
    let r = g.finish().unwrap();
    assert_eq!(
        r.verdict,
        GateVerdict::Fail {
            non_descending: 1,
            cyclic: false
        }
    );
}

#[test]
fn an_order_that_misses_a_cycle_is_inconsistent() {
    let broken = FnOrder {
        f: |_: &[i32], _: &[i32]| Ordering::Less,
        label: "broken".into(),
    };
    let mut g = ShipGate::new(&broken, 4, GateConfig::disk(spill("inc"))).unwrap();
    g.push_relation(&[3, 1, 0, 0], "x", [(&[1, 3, 0, 0][..], NonZero::Yes)])
        .unwrap();
    g.push_relation(&[1, 3, 0, 0], "y", [(&[3, 1, 0, 0][..], NonZero::Yes)])
        .unwrap();
    let r = g.finish().unwrap();
    assert_eq!(r.verdict, GateVerdict::Inconsistent);
    assert!(!r.verdict.passed());
    assert_eq!(r.graph.witness_cycle.len(), 2);
}

#[test]
fn zero_terms_are_not_edges_unknown_terms_are() {
    let c = cmp4();
    let hi: &[i32] = &[4, 1, 0, 0];
    let mut g = ShipGate::new(&c, 4, GateConfig::disk(spill("zero"))).unwrap();
    g.push_relation(
        &[2, 1, 0, 0],
        "r",
        [(hi, NonZero::Zero), (LEAF, NonZero::Yes)],
    )
    .unwrap();
    let r = g.finish().unwrap();
    assert_eq!(r.verdict, GateVerdict::Pass);
    assert_eq!((r.edges, r.zero_terms_skipped, r.keys), (1, 1, 2));
    let mut g = ShipGate::new(&c, 4, GateConfig::disk(spill("unk"))).unwrap();
    g.push_relation(&[2, 1, 0, 0], "r", [(hi, NonZero::Unknown)])
        .unwrap();
    assert_eq!(g.finish().unwrap().non_descending, 1);
}

#[test]
fn the_alias_edge_at_equal_rank_and_the_self_edge_are_refused() {
    let coarse = FnOrder {
        f: |a: &[i32], b: &[i32]| {
            let m = |k: &[i32]| {
                k.iter()
                    .map(|&x| if x > 0 { x - 1 } else { -x })
                    .sum::<i32>()
            };
            m(a).cmp(&m(b))
        },
        label: "coarse".into(),
    };
    let mut g = ShipGate::new(&coarse, 4, GateConfig::disk(spill("alias"))).unwrap();
    g.push_edge(&[3, 1, 0, 0], "alias", &[1, 3, 0, 0], NonZero::Yes)
        .unwrap();
    let r = g.finish().unwrap();
    assert_eq!((r.non_descending, r.non_descending_equal), (1, 1));
    assert_eq!(r.non_descending_listed[0].order, EdgeOrder::Equal);
    let c = cmp4();
    let mut g = ShipGate::new(&c, 4, GateConfig::disk(spill("self"))).unwrap();
    g.push_edge(&[3, 1, 0, 0], "self", &[3, 1, 0, 0], NonZero::Yes)
        .unwrap();
    let r = g.finish().unwrap();
    assert_eq!(r.non_descending_equal, 1);
    assert_eq!(r.graph.sccs.self_loops, 1);
    assert_eq!(r.graph.witness_cycle.len(), 1);
}

#[test]
fn keys_beyond_i8_and_arity_errors() {
    let c = NodeComparator::legacy(3, None).unwrap();
    let mut g = ShipGate::new(&c, 3, GateConfig::disk(spill("wide"))).unwrap();
    g.push_edge(&[3, 1, 0], "a", &[2, 1, 0], NonZero::Yes)
        .unwrap();
    g.push_edge(&[300, 1, 0], "a", &[3, 1, 0], NonZero::Yes)
        .unwrap();
    g.push_edge(&[300, 1, -200], "a", &[300, 1, 0], NonZero::Yes)
        .unwrap();
    assert!(matches!(
        g.push_edge(&[1, 1], "a", &[1, 1, 0], NonZero::Yes),
        Err(GateError::Arity { .. })
    ));
    let r = g.finish().unwrap();
    assert_eq!(r.verdict, GateVerdict::Pass);
    assert_eq!(r.keys, 4);
}

#[test]
fn the_listed_offenders_are_the_smallest_whatever_the_push_order() {
    let c = cmp4();
    let mut edges: Vec<(Vec<i32>, Vec<i32>)> = (1..30)
        .map(|i| (vec![1 + i % 3, 1, -(i / 3), 0], vec![5 + i, 1, 0, 0]))
        .collect();
    let run = |edges: &[(Vec<i32>, Vec<i32>)], tag: &str| {
        let mut cfg = GateConfig::disk(spill(tag));
        cfg.max_listed = 5;
        let mut g = ShipGate::new(&c, 4, cfg).unwrap();
        for (k, t) in edges {
            g.push_edge(k, "x", t, NonZero::Yes).unwrap();
        }
        g.finish().unwrap()
    };
    let r1 = run(&edges, "lo1");
    edges.reverse();
    let r2 = run(&edges, "lo2");
    assert_eq!(r1.non_descending, 29);
    assert_eq!(r1.non_descending_listed.len(), 5);
    assert_eq!(r1.non_descending_listed, r2.non_descending_listed);
}

#[test]
fn memory_and_disk_storage_give_the_same_report() {
    let c = cmp4();
    type Rel<'a> = (Vec<i32>, &'a str, Vec<(Vec<i32>, NonZero)>);
    let rels: Vec<Rel> = vec![
        (
            vec![3, 1, 0, 0],
            "a",
            vec![
                (vec![2, 1, 0, 0], NonZero::Yes),
                (vec![4, 1, 0, 0], NonZero::Zero),
            ],
        ),
        (
            vec![2, 1, 0, 0],
            "b",
            vec![
                (vec![3, 1, 0, 0], NonZero::Unknown),
                (LEAF.to_vec(), NonZero::Yes),
            ],
        ),
        (
            vec![1, 3, 0, 0],
            "c",
            vec![(vec![1, 2, 0, 0], NonZero::Yes)],
        ),
        (
            vec![1, 2, 0, 0],
            "c",
            vec![(vec![1, 2, 0, 0], NonZero::Yes)],
        ),
    ];
    let run = |cfg: GateConfig| {
        let mut g = ShipGate::new(&c, 4, cfg).unwrap();
        for (k, p, t) in &rels {
            g.push_relation(k, p, t.iter().map(|(x, n)| (x.as_slice(), *n)))
                .unwrap();
        }
        let mut r = g.finish().unwrap();
        r.seconds_intake = 0.0;
        r.seconds_graph = 0.0;
        r.edges_per_second = 0.0;
        r.memory_bytes = 0;
        r.graph.method.clear();
        r.graph.csr_passes = 0;
        r
    };
    let m = run(GateConfig::memory());
    let d = run(GateConfig::disk(spill("md")));
    assert_eq!(m, d);
    assert_eq!(m.non_descending, 2); // b -> a (higher) and the self edge
    assert_eq!(m.graph.sccs.cyclic, 2);
    assert_eq!(m.graph.sccs.self_loops, 1);
}
