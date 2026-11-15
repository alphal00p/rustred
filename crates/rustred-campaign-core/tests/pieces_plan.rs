//! Lane pieces: the piece planner on constructed data (interfaces/pieces.md 3).
//!
//! Keys of arity 4: slots 0, 1 are lines (power >= 1), slots 2, 3 numerators (<= 0). The order is the engine's
//! legacy comparator (`NodeComparator::legacy`), i.e. a real full order; coarse or broken orders are `FnOrder`s.

use std::cmp::Ordering;

use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::pieces::plan::{HandDownTarget, PieceKind, RefusalReason};
use rustred_campaign_core::pieces::{
    Candidate, FnOrder, KeyOrder, NonZero, PlanError, PlanInput, Ranking, ServiceKind, Term,
    UnservedKey, plan,
};

fn cmp4() -> NodeComparator {
    NodeComparator::legacy(4, None).unwrap()
}

fn cand(kind: ServiceKind, terms: &[&[i32]], prov: &str) -> Candidate {
    Candidate {
        kind,
        terms: terms.iter().map(|t| Term::unknown(t.to_vec())).collect(),
        provenance: prov.into(),
        regular_at_4: None,
    }
}

fn ukey(key: &[i32], face: &str, cands: Vec<Candidate>) -> UnservedKey {
    UnservedKey {
        key: key.to_vec(),
        face: face.into(),
        candidates: cands,
    }
}

const LEAF: &[i32] = &[1, 1, 0, 0];

/// Face A = {0}x{}, face B = {1}x{}. A's hand-down key lands in B's core and B's in A's core: the face graph is a
/// 2-cycle, the key graph is acyclic.
fn coupled_faces() -> Vec<UnservedKey> {
    vec![
        // A: hand-down [3,1,0,0] -> B core [1,2,0,0]; core [2,1,0,0] by a recipe
        ukey(
            &[3, 1, 0, 0],
            "A",
            vec![cand(ServiceKind::HandDown, &[&[1, 2, 0, 0]], "g:A->B")],
        ),
        ukey(
            &[2, 1, 0, 0],
            "A",
            vec![cand(ServiceKind::CoreRecipe, &[LEAF, &[0, 1, 0, 0]], "recipe:A")],
        ),
        // B: hand-down [1,3,0,0] -> A core [2,1,0,0]; core [1,2,0,0] by a recipe
        ukey(
            &[1, 3, 0, 0],
            "B",
            vec![cand(ServiceKind::HandDown, &[&[2, 1, 0, 0]], "g:B->A")],
        ),
        ukey(
            &[1, 2, 0, 0],
            "B",
            vec![cand(ServiceKind::CoreRecipe, &[LEAF], "recipe:B")],
        ),
    ]
}

#[test]
fn faces_handing_down_into_each_others_cores_are_cyclic_at_face_level_and_planned_per_key() {
    let c = cmp4();
    let p = plan(&c, &PlanInput::new(4, coupled_faces())).unwrap();
    assert!(p.core_open.is_empty(), "{:?}", p.core_open);
    assert!(p.refused.is_empty(), "{:?}", p.refused);
    // face level: A -> B and B -> A
    assert!(p.face_report.face_level_cyclic);
    assert_eq!(p.face_report.face_cycles, vec![vec!["A".to_string(), "B".to_string()]]);
    assert!(p.face_report.piece_level_acyclic);
    assert!(p.face_report.key_level_acyclic);
    // hand-downs land in the OTHER face's core
    for ch in &p.chosen {
        if ch.kind == Some(ServiceKind::HandDown) {
            assert_eq!(ch.hand_down_target, Some(HandDownTarget::OtherFace));
            assert_eq!(ch.piece, PieceKind::HandDowns);
        }
    }
    // piece order: each core before the hand-down piece that lands in it
    let pos = |label: &str| {
        p.piece_order
            .iter()
            .position(|g| g.iter().any(|l| l == label))
            .unwrap_or_else(|| panic!("{label} not in {:?}", p.piece_order))
    };
    assert!(p.piece_order.iter().all(|g| g.len() == 1));
    assert!(pos("B#iii:core") < pos("A#ii:hand_downs"));
    assert!(pos("A#iii:core") < pos("B#ii:hand_downs"));
    // key order: every plan-internal dependency first
    let kpos = |k: &[i32]| p.key_order.iter().position(|x| x == k).unwrap();
    assert!(kpos(&[1, 2, 0, 0]) < kpos(&[3, 1, 0, 0]));
    assert!(kpos(&[2, 1, 0, 0]) < kpos(&[1, 3, 0, 0]));
    assert_eq!(p.edges, [5, 2, 3]);
    assert_eq!(p.hand_down_targets.get("other_face"), Some(&2));
}

#[test]
fn a_true_key_level_cycle_gets_no_plan_through_it_and_a_witness() {
    let c = cmp4();
    // K1 and K2 at the same dots+rank: an alias pair, each offered as a composite through the other.
    let (k1, k2): (&[i32], &[i32]) = (&[3, 1, 0, 0], &[1, 3, 0, 0]);
    let lower_is_k2 = c.cmp_keys(k2, k1) == Ordering::Less;
    let keys = vec![
        ukey(k1, "A", vec![cand(ServiceKind::Composite, &[k2], "alias:1->2")]),
        ukey(k2, "B", vec![cand(ServiceKind::Composite, &[k1], "alias:2->1")]),
    ];
    let p = plan(&c, &PlanInput::new(4, keys)).unwrap();
    // exactly one of the two edges descends: the higher key is served through the lower, the lower is left to core
    let (hi, lo) = if lower_is_k2 { (k1, k2) } else { (k2, k1) };
    assert_eq!(p.core_open, vec![lo.to_vec()]);
    assert_eq!(p.refused.len(), 1);
    assert_eq!(p.refused[0].key, lo.to_vec());
    assert_eq!(p.refused[0].reason, RefusalReason::NonDescending);
    let ch = p.chosen.iter().find(|x| x.key == hi).unwrap();
    assert_eq!(ch.kind, Some(ServiceKind::Composite));
    // the candidate graph had the cycle: reported, with the edge the plan does NOT use
    assert_eq!(p.candidate_cyclic_sccs, 1);
    assert_eq!(p.candidate_cycles.len(), 1);
    let cyc = &p.candidate_cycles[0];
    assert_eq!(cyc.len(), 2);
    assert_eq!(cyc.iter().filter(|e| e.in_plan).count(), 1);
    assert!(cyc.iter().any(|e| !e.in_plan && e.from == lo));
}

#[test]
fn a_broken_order_that_calls_both_alias_edges_descending_is_caught_by_the_graph_check() {
    // the guard: an order under which every edge "descends" while the edges form a cycle (the hand alias cycle came
    // from edges believed fine) must fail LOUDLY, not produce a plan
    let broken = FnOrder {
        f: |_: &[i32], _: &[i32]| Ordering::Less,
        label: "broken: everything lower".into(),
    };
    let keys = vec![
        ukey(&[3, 1, 0, 0], "A", vec![cand(ServiceKind::Composite, &[&[1, 3, 0, 0]], "x")]),
        ukey(&[1, 3, 0, 0], "B", vec![cand(ServiceKind::Composite, &[&[3, 1, 0, 0]], "y")]),
    ];
    match plan(&broken, &PlanInput::new(4, keys)) {
        Err(PlanError::Inconsistent { cycle }) => {
            assert_eq!(cycle.len(), 2);
            assert!(cycle.contains(&vec![3, 1, 0, 0]) && cycle.contains(&vec![1, 3, 0, 0]));
        }
        other => panic!("expected Inconsistent, got {other:?}"),
    }
}

#[test]
fn an_edge_equal_under_the_order_is_refused() {
    // a coarse order (dots + rank only): the alias K -> gK sits at EQUAL rank and must be refused
    let coarse = FnOrder {
        f: |a: &[i32], b: &[i32]| {
            let m = |k: &[i32]| k.iter().map(|&x| if x > 0 { x - 1 } else { -x }).sum::<i32>();
            m(a).cmp(&m(b))
        },
        label: "coarse dots+rank".into(),
    };
    let keys = vec![ukey(
        &[3, 1, 0, 0],
        "A",
        vec![
            cand(ServiceKind::Composite, &[&[1, 3, 0, 0]], "alias"),
            cand(ServiceKind::CoreRecipe, &[LEAF], "recipe"),
        ],
    )];
    let p = plan(&coarse, &PlanInput::new(4, keys.clone())).unwrap();
    assert_eq!(p.refused.len(), 1);
    assert_eq!(p.refused[0].reason, RefusalReason::Equal);
    assert_eq!(p.refused[0].term, Some(vec![1, 3, 0, 0]));
    assert_eq!(p.chosen[0].kind, Some(ServiceKind::CoreRecipe));
    // under the FULL order: the key itself as a term (K -> K) is equal and refused
    let c = cmp4();
    let selfk = vec![ukey(
        &[2, 1, 0, 0],
        "A",
        vec![cand(ServiceKind::Composite, &[&[2, 1, 0, 0], LEAF], "self")],
    )];
    let p = plan(&c, &PlanInput::new(4, selfk)).unwrap();
    assert_eq!(p.refused[0].reason, RefusalReason::Equal);
    assert_eq!(p.core_open, vec![vec![2, 1, 0, 0]]);
}

#[test]
fn a_hand_down_into_the_same_faces_own_core() {
    // one face {0,1}x{}: an element exchanging lines 0 <-> 1 sends a key to its mirror at equal dots+rank; the
    // full order decides which is lower: the higher hands down INTO THE SAME FACE'S core
    let c = cmp4();
    let (a, b): (&[i32], &[i32]) = (&[2, 3, 0, 0], &[3, 2, 0, 0]);
    let (hi, lo) = if c.cmp_keys(b, a) == Ordering::Less { (a, b) } else { (b, a) };
    let keys = vec![
        ukey(hi, "F", vec![cand(ServiceKind::HandDown, &[lo], "g:swap01")]),
        ukey(lo, "F", vec![cand(ServiceKind::CoreRecipe, &[&[2, 2, 0, 0], LEAF], "recipe:F")]),
    ];
    let p = plan(&c, &PlanInput::new(4, keys)).unwrap();
    let ch = p.chosen.iter().find(|x| x.key == hi).unwrap();
    assert_eq!(ch.hand_down_target, Some(HandDownTarget::OwnCore));
    assert_eq!(p.hand_down_targets.get("own_core"), Some(&1));
    // the face hands into itself: face level "cyclic" by a self loop, piece and key level acyclic
    assert_eq!(p.face_report.face_graph["F"]["F"], 1);
    assert!(p.face_report.face_level_cyclic);
    assert!(p.face_report.piece_level_acyclic);
    let pos = |l: &str| p.piece_order.iter().position(|g| g.iter().any(|x| x == l)).unwrap();
    assert!(pos("F#iii:core") < pos("F#ii:hand_downs"));
    let core = p.pieces.iter().find(|x| x.id.label() == "F#iii:core").unwrap();
    assert_eq!((core.keys, core.core_solved, core.core_open), (1, 1, 0));
}

#[test]
fn ranking_terms_and_zero_terms() {
    let c = cmp4();
    let k: &[i32] = &[3, 1, 0, 0];
    let cands = vec![
        cand(ServiceKind::CoreRecipe, &[LEAF], "recipe"),
        cand(ServiceKind::Composite, &[&[2, 1, 0, 0], LEAF], "comp2"),
        cand(ServiceKind::Composite, &[LEAF], "comp1"),
    ];
    // default: composite first, fewer terms first
    let p = plan(&c, &PlanInput::new(4, vec![ukey(k, "A", cands.clone())])).unwrap();
    assert_eq!(p.chosen[0].provenance.as_deref(), Some("comp1"));
    // caller's ranking: recipes first
    let mut inp = PlanInput::new(4, vec![ukey(k, "A", cands.clone())]);
    inp.ranking = Ranking {
        kinds: vec![ServiceKind::CoreRecipe, ServiceKind::Composite],
        singular_last: true,
    };
    assert_eq!(plan(&c, &inp).unwrap().chosen[0].kind, Some(ServiceKind::CoreRecipe));
    // a kind outside the ranking is never chosen
    inp.ranking = Ranking {
        kinds: vec![ServiceKind::TableRule],
        singular_last: true,
    };
    let p = plan(&c, &inp).unwrap();
    assert_eq!(p.core_open, vec![k.to_vec()]);
    assert!(p.refused.iter().all(|r| r.reason == RefusalReason::KindNotRanked));
    // a HIGHER term with an exactly zero coefficient is not an edge; the same term with unknown non-zero-ness is
    let higher: &[i32] = &[4, 1, 0, 0];
    let zero = Candidate {
        kind: ServiceKind::Composite,
        terms: vec![Term::new(higher.to_vec(), NonZero::Zero), Term::new(LEAF.to_vec(), NonZero::Yes)],
        provenance: "zero-higher".into(),
        regular_at_4: None,
    };
    let p = plan(&c, &PlanInput::new(4, vec![ukey(k, "A", vec![zero.clone()])])).unwrap();
    assert_eq!(p.chosen[0].provenance.as_deref(), Some("zero-higher"));
    assert_eq!(p.chosen[0].edge_terms, 1);
    let mut unk = zero;
    unk.terms[0].nonzero = NonZero::Unknown;
    let p = plan(&c, &PlanInput::new(4, vec![ukey(k, "A", vec![unk])])).unwrap();
    assert_eq!(p.refused[0].reason, RefusalReason::NonDescending);
    // a hand-down must have exactly one edge term
    let hd2 = cand(ServiceKind::HandDown, &[LEAF, &[2, 1, 0, 0]], "hd2");
    let p = plan(&c, &PlanInput::new(4, vec![ukey(k, "A", vec![hd2])])).unwrap();
    assert_eq!(p.refused[0].reason, RefusalReason::HandDownNotOneTerm);
}

#[test]
fn errors_arity_and_face_conflict() {
    let c = cmp4();
    let bad = vec![ukey(&[1, 1, 0], "A", vec![])];
    assert!(matches!(plan(&c, &PlanInput::new(4, bad)), Err(PlanError::Arity { .. })));
    let conflict = vec![ukey(LEAF, "A", vec![]), ukey(LEAF, "B", vec![])];
    assert!(matches!(
        plan(&c, &PlanInput::new(4, conflict)),
        Err(PlanError::FaceConflict { .. })
    ));
}

/// A larger random instance: faces = support of the dotted lines, candidates of every kind, some non-descending.
fn random_instance(seed: u64, n: usize) -> Vec<UnservedKey> {
    let mut s = seed;
    let mut rnd = move |m: u64| {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (s >> 33) % m
    };
    let mut keys: Vec<Vec<i32>> = Vec::new();
    while keys.len() < n {
        let k = vec![
            1 + rnd(4) as i32,
            1 + rnd(4) as i32,
            -(rnd(3) as i32),
            -(rnd(3) as i32),
        ];
        if !keys.contains(&k) {
            keys.push(k);
        }
    }
    let kinds = [
        ServiceKind::Composite,
        ServiceKind::TwoRow,
        ServiceKind::HandDown,
        ServiceKind::CoreRecipe,
        ServiceKind::TableRule,
    ];
    keys.iter()
        .map(|k| {
            let face = format!(
                "{{{}}}",
                (0..2).filter(|&i| k[i] > 1).map(|i| i.to_string()).collect::<Vec<_>>().join(",")
            );
            let nc = rnd(4) as usize;
            let cands = (0..nc)
                .map(|ci| {
                    let kind = kinds[rnd(5) as usize];
                    let nt = if kind == ServiceKind::HandDown { 1 } else { 1 + rnd(3) as usize };
                    let terms = (0..nt)
                        .map(|_| {
                            let t = if rnd(3) == 0 {
                                LEAF.to_vec()
                            } else {
                                keys[rnd(keys.len() as u64) as usize].clone()
                            };
                            let nz = match rnd(5) {
                                0 => NonZero::Zero,
                                1 => NonZero::Yes,
                                _ => NonZero::Unknown,
                            };
                            Term::new(t, nz)
                        })
                        .collect();
                    Candidate {
                        kind,
                        terms,
                        provenance: format!("p{ci}"),
                        regular_at_4: None,
                    }
                })
                .collect();
            UnservedKey {
                key: k.clone(),
                face,
                candidates: cands,
            }
        })
        .collect()
}

#[test]
fn determinism_under_any_input_order() {
    let c = cmp4();
    let base = random_instance(7, 40);
    let p0 = plan(&c, &PlanInput::new(4, base.clone())).unwrap();
    let j0 = serde_json::to_string(&p0).unwrap();
    assert!(p0.chosen.iter().any(|c| c.kind.is_some()) && !p0.refused.is_empty());
    let mut s = 12345u64;
    for _ in 0..10 {
        let mut v = base.clone();
        // Fisher-Yates on keys and on each key's candidates
        for i in (1..v.len()).rev() {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
            v.swap(i, (s >> 33) as usize % (i + 1));
        }
        for u in &mut v {
            for i in (1..u.candidates.len()).rev() {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
                u.candidates.swap(i, (s >> 33) as usize % (i + 1));
            }
        }
        let p = plan(&c, &PlanInput::new(4, v)).unwrap();
        assert_eq!(serde_json::to_string(&p).unwrap(), j0);
    }
}

#[test]
fn random_plans_descend_on_every_chosen_edge_and_respect_the_key_order() {
    let c = cmp4();
    for seed in 1..30u64 {
        let inst = random_instance(seed, 30);
        let p = plan(&c, &PlanInput::new(4, inst.clone())).unwrap();
        let pos: std::collections::BTreeMap<Vec<i32>, usize> =
            p.key_order.iter().cloned().enumerate().map(|(i, k)| (k, i)).collect();
        assert_eq!(pos.len(), inst.len());
        // rebuild the chosen candidate from provenance + kind and check every edge
        for ch in &p.chosen {
            let Some(kind) = ch.kind else {
                assert_eq!(ch.piece, PieceKind::Core);
                continue;
            };
            let u = inst.iter().find(|u| u.key == ch.key).unwrap();
            let viable: Vec<&Candidate> = u
                .candidates
                .iter()
                .filter(|cd| cd.kind == kind && Some(&cd.provenance) == ch.provenance.as_ref())
                .collect();
            assert!(!viable.is_empty());
            let cd = viable
                .iter()
                .find(|cd| {
                    cd.terms
                        .iter()
                        .filter(|t| t.nonzero != NonZero::Zero)
                        .all(|t| c.cmp_keys(&t.key, &u.key) == Ordering::Less)
                })
                .expect("a descending candidate with the chosen provenance");
            for t in cd.terms.iter().filter(|t| t.nonzero != NonZero::Zero) {
                if let Some(&j) = pos.get(&t.key) {
                    assert!(j < pos[&ch.key], "dependency after its user");
                }
            }
        }
    }
}
