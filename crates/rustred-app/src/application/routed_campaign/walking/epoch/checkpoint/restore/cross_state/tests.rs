use super::super::super::super::ledger6::{Counters, Transition};
use super::super::super::super::state::EpochState;
use super::super::super::tests::state;
use super::*;
use crate::application::routed_campaign::walking::descendant_closure::Tracker;
use crate::application::routed_campaign::walking::queue::{Domain, Phase};
use rustred::solver::DomainPowerBounds;

fn fixture() -> EpochState<2> {
    let mut state = state(4);
    state.k = 1;
    for (id, transition, flags, targets) in [
        (
            0,
            Transition::T4Native {
                epoch: 1,
                residual: false,
                dband: false,
            },
            3,
            vec![1],
        ),
        (1, Transition::T5Frontier { epoch: 1 }, 2, vec![2]),
        (2, Transition::T6Error { epoch: 1, err: 1 }, 0, vec![3]),
    ] {
        state.ledger.apply(id, Transition::T2Reserve).unwrap();
        state.ledger.apply(id, transition).unwrap();
        state.nodes[id as usize] = flags;
        state.edges.append_run(id, &targets, false).unwrap();
        state.edges.fold_record(
            id,
            state.ledger.tag(id).unwrap() as u8,
            targets.len() as u32,
        );
        for target in targets {
            state.tracker.edge(id as usize, target as usize);
        }
        state
            .tracker
            .finish(id as usize, flags & 2 != 0, flags & 1 != 0);
    }
    state.frontier_counts.insert(1, 2);
    state.counters = WalkCounters {
        natives: 3,
        completed: 2,
        native_errors: 1,
        initial_inspected: 2,
        frontiers: 5,
        merges: 1,
        ..Default::default()
    };
    state
}

fn check(state: &mut EpochState<2>, digest: &str, closure: Option<&[u8]>) -> io::Result<()> {
    check_inputs(state, digest, closure, 0)
}

fn check_inputs(
    state: &mut EpochState<2>,
    digest: &str,
    closure: Option<&[u8]>,
    input_frontiers: usize,
) -> io::Result<()> {
    validate(View {
        store: &state.store,
        ledger: &state.ledger,
        nodes: &mut state.nodes,
        live: &state.live,
        edges: &mut state.edges,
        anchors: &state.anchors,
        frontier_counts: &state.frontier_counts,
        closure_flags: closure,
        walk: &state.counters,
        k: state.k,
        p0: state.p0,
        input_frontiers,
        records_digest: digest,
    })
}

#[test]
fn initial_input_obligations_contribute_without_a_native_frontier() {
    let mut state = state(1);
    state.counters.frontiers = 1;
    let digest = state.edges.records_digest();
    check_inputs(&mut state, &digest, None, 1).unwrap();
    assert!(check_inputs(&mut state, &digest, None, 0).is_err());
    assert!(check_inputs(&mut state, &digest, None, 2).is_err());
    let mut state = fixture();
    state.counters.frontiers += 1;
    let digest = state.edges.records_digest();
    check_inputs(&mut state, &digest, None, 1).unwrap();
}

fn replace(state: &mut EpochState<2>, id: usize, replacement: Entry6) {
    let mut ledger = Ledger6::default();
    for (index, &word) in state.ledger.words().iter().enumerate() {
        ledger
            .restore_word(if index == id {
                replacement.encode()
            } else {
                word
            })
            .unwrap();
    }
    state.ledger = ledger;
}

#[test]
fn c2_prefix_frontiers_are_not_lost_and_record_hash_remains_appendable() {
    let mut state = fixture();
    let digest = state.edges.records_digest();
    let mut original = std::mem::replace(&mut state.edges, EdgeStore::new());
    state.edges = EdgeStore::from_owned_log(original.log().to_vec(), state.watermark()).unwrap();
    let closure: Vec<_> = state.tracker.node_flags().collect();
    check(&mut state, &digest, Some(&closure)).unwrap();
    assert_eq!(state.edges.records_digest(), digest);
    assert_eq!(state.counters.frontiers, 5);
    assert_eq!(state.frontier_counts.values().copied().sum::<u32>(), 2);
    assert_eq!(state.nodes, [3, 2, 0, 0], "census never escapes in flags");
    original.fold_record(3, Tag::Native as u8, 0);
    state.edges.fold_record(3, Tag::Native as u8, 0);
    assert_eq!(state.edges.records_digest(), original.records_digest());
}

#[test]
fn flags_epochs_frontier_inventory_and_error_classes_fail_closed() {
    for mutation in 0..13 {
        let mut state = fixture();
        let digest = state.edges.records_digest();
        match mutation {
            0 => state.nodes[0] &= !NODE_SEALED,
            1 => state.nodes[2] |= NODE_INSPECTED,
            2 => state.nodes[0] |= NODE_ANCHORED,
            3 => state.nodes[0] |= RUN_SEEN,
            4 => replace(
                &mut state,
                0,
                Entry6::Native {
                    epoch: 2,
                    residual: false,
                    dband: false,
                },
            ),
            5 => replace(&mut state, 2, Entry6::NativeError { epoch: 1, err: 0 }),
            6 => {
                state.frontier_counts.remove(&1);
            }
            7 => {
                state.frontier_counts.insert(2, 3);
            }
            8 => state.counters.frontiers = 1,
            9 => state.counters.completed += 1,
            10 => replace(
                &mut state,
                3,
                Entry6::Reserved(Counters {
                    attempts: 8,
                    ..Default::default()
                }),
            ),
            11 => replace(&mut state, 3, Entry6::Exhausted(Counters::default())),
            12 => replace(
                &mut state,
                0,
                Entry6::Native {
                    epoch: 0,
                    residual: false,
                    dband: false,
                },
            ),
            _ => unreachable!(),
        }
        assert!(
            check(&mut state, &digest, None).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn duplicate_missing_unmerged_and_reordered_runs_and_hash_mutations_refuse() {
    for log in [
        vec![0, 1, 1, 0, 1, 1, 2, 1, 3],
        vec![0, 1, 1, 1, 1, 2],
        vec![0, 1, 1, 1, 1, 2, 3, 0],
    ] {
        let mut state = fixture();
        let digest = state.edges.records_digest();
        state.edges = EdgeStore::from_owned_log(log, 4).unwrap();
        assert!(check(&mut state, &digest, None).is_err());
        assert_eq!(state.nodes, [3, 2, 0, 0], "error clears scratch too");
    }
    let mut state = fixture();
    let digest = state.edges.records_digest();
    replace(
        &mut state,
        0,
        Entry6::Native {
            epoch: 2,
            residual: false,
            dband: false,
        },
    );
    state.k = 2;
    state.counters.merges = 2;
    assert!(
        check(&mut state, &digest, None)
            .unwrap_err()
            .to_string()
            .contains("merge order")
    );
    let mut state = fixture();
    assert!(check(&mut state, &"0".repeat(64), None).is_err());
    assert_eq!(state.nodes, [3, 2, 0, 0]);
}

#[test]
fn stale_closed_bits_never_certify_a_pending_descendant() {
    let mut state = fixture();
    let digest = state.edges.records_digest();
    let mut closure: Vec<_> = state.tracker.node_flags().collect();
    closure[0] |= 4; // source0 depends on unclosed1 -> error2 -> Pending3
    assert!(
        check(&mut state, &digest, Some(&closure))
            .unwrap_err()
            .to_string()
            .contains("unclosed descendant")
    );
    closure[0] &= !4;
    closure[3] |= 4; // an unsealed Pending cannot itself be closed
    assert!(check(&mut state, &digest, Some(&closure)).is_err());
    assert_eq!(state.nodes, [3, 2, 0, 0]);
}

fn alias_fixture() -> EpochState<2> {
    let mut state = EpochState::new(100, 100, 100);
    for high in [0, 2] {
        super::super::super::super::admit_initial(
            &mut state,
            &Domain {
                phase: Phase::Apply,
                owner: [true, false],
                lower: vec![0, 0],
                upper: vec![Some(high), Some(0)],
                rank: Some(100),
                powers: DomainPowerBounds::default(),
            },
        )
        .unwrap();
    }
    state.p0 = 0;
    state.k = 1;
    state
        .ledger
        .apply(0, Transition::T3Alias { to: 1 })
        .unwrap();
    state.nodes[0] = NODE_SEALED;
    state.set_live(0, false);
    state.edges.append_run(0, &[1], false).unwrap();
    state.tracker = Tracker::new(0);
    state.tracker.discovered(2);
    state.tracker.edge(0, 1);
    state.tracker.finish(0, false, true);
    state.counters.aliases = 1;
    state.counters.transfers = 1;
    state.counters.merges = 1;
    state
}

#[test]
fn alias_is_forward_unprotected_retired_contained_and_has_exact_run() {
    let mut state = alias_fixture();
    let digest = state.edges.records_digest();
    check(&mut state, &digest, None).unwrap();
    state.p0 = 1;
    assert!(check(&mut state, &digest, None).is_err());
    state.p0 = 0;
    state.set_live(0, true);
    assert!(check(&mut state, &digest, None).is_err());
    state.set_live(0, false);
    state.edges = EdgeStore::from_owned_log(vec![0, 0], 2).unwrap();
    assert!(check(&mut state, &digest, None).is_err());
    let mut state = alias_fixture();
    state.store.unique_mut().unwrap().domains.swap(0, 1); // canonical outer no longer contains source
    assert!(
        check(&mut state, &digest, None)
            .unwrap_err()
            .to_string()
            .contains("does not contain")
    );
}

fn anchor_fixture(cut: i64) -> EpochState<2> {
    use super::super::super::super::anchors::{AnchorRef, AnchorScope, Lent};
    let mut state = state(2);
    state.p0 = 1;
    state.k = 1;
    state.ledger.apply(1, Transition::T2Reserve).unwrap();
    state
        .ledger
        .apply(
            1,
            Transition::T4Native {
                epoch: 1,
                residual: false,
                dband: true,
            },
        )
        .unwrap();
    state.nodes[1] = NODE_SEALED | NODE_INSPECTED | NODE_ANCHORED;
    state
        .anchors
        .push(AnchorRecord {
            node: 1,
            kind: AnchorKind::InitialDBand,
            dispatch_version: 0,
            scope: AnchorScope::DBandCut(cut),
            anchors: vec![AnchorRef {
                anchor: 0,
                stamp: None,
                lent: Lent::Full,
            }],
        })
        .unwrap();
    state.edges.append_run(1, &[0], false).unwrap();
    state.edges.fold_record(1, Tag::Native as u8, 1);
    state.counters = WalkCounters {
        natives: 1,
        completed: 1,
        partials: 1,
        merges: 1,
        ..Default::default()
    };
    state
}

#[test]
fn initial_anchor_reuses_exact_cover_and_requires_its_borrowed_run_edge() {
    let mut state = anchor_fixture(i64::MAX);
    let digest = state.edges.records_digest();
    check(&mut state, &digest, None).unwrap();
    state.edges = EdgeStore::from_owned_log(vec![1, 0], 2).unwrap();
    assert!(
        check(&mut state, &digest, None)
            .unwrap_err()
            .to_string()
            .contains("dependency missing")
    );
    assert_eq!(
        state.nodes,
        [0, NODE_SEALED | NODE_INSPECTED | NODE_ANCHORED]
    );
    let mut state = anchor_fixture(-100);
    assert!(
        check(&mut state, &digest, None)
            .unwrap_err()
            .to_string()
            .contains("geometry or provenance")
    );
    let mut state = anchor_fixture(i64::MAX);
    replace(
        &mut state,
        1,
        Entry6::Native {
            epoch: 1,
            residual: false,
            dband: false,
        },
    );
    assert!(check(&mut state, &digest, None).is_err());
}
