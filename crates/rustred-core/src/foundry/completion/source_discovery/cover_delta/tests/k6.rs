//! Oracle-disabled K=6 proof that one replayed interior owner removes an
//! exact positive-dimensional part of the initial blind orthant.

use std::sync::Arc;

use crate::foundry::completion::frame::admission::{
    ExactOwnerCoverObstructionKind, ExactOwnerCoverStatus,
};
use crate::foundry::completion::source_discovery::ExactSemanticExecutableOwner;
use crate::foundry::completion::source_discovery::test_fixtures::OracleDisabledK6Fixture;

use super::super::{
    CanonicalExactOwnerLedger, ExactOwnerCoverDelta, ExactOwnerCoverDeltaKind,
    ExactOwnerLedgerCoverStatus,
};

fn apply_first_owner(
    fixture: &OracleDisabledK6Fixture,
    owner: Arc<ExactSemanticExecutableOwner>,
) -> (CanonicalExactOwnerLedger, ExactOwnerCoverDelta) {
    let mut ledger = fixture.new_ledger();
    let initial = ledger.try_clone_uncovered_partition().unwrap();
    assert_eq!(initial.boxes().len(), 1);
    assert_eq!(
        initial.boxes()[0].lower(),
        vec![0; fixture.sector().arity()]
    );
    assert_eq!(
        initial.boxes()[0].upper(),
        vec![None; fixture.sector().arity()]
    );
    let delta = ledger.try_apply_owner(owner).unwrap();
    (ledger, delta)
}

#[test]
fn oracle_disabled_k6_interior_replay_strictly_shrinks_the_exact_blind_orthant() {
    let fixture = OracleDisabledK6Fixture::shared();
    assert_eq!(fixture.completed().source_row_count(), 9);
    assert_eq!(fixture.predecessor().closed_layer_count(), 0);

    let seed_ledger = fixture.new_ledger();
    let plan = fixture.plan(&seed_ledger, 2, 0);
    assert_eq!(plan.tasks().len(), 1);
    let task = &plan.tasks()[0];
    assert_eq!(task.lattice_target(), &[2; 6]);
    assert_eq!(task.target_shift().values(), &[-2, -2, 2, -2, 2, 2]);

    let first_owner = fixture.replay_owner(task);
    let second_owner = fixture.replay_owner(task);
    assert_eq!(
        first_owner.content_order_key(),
        second_owner.content_order_key()
    );

    let (first_ledger, first_delta) = apply_first_owner(fixture, first_owner);
    let (second_ledger, second_delta) = apply_first_owner(fixture, second_owner);
    assert_eq!(first_delta, second_delta);
    assert_eq!(
        first_delta.kind(),
        ExactOwnerCoverDeltaKind::StrictGeometricShrink
    );
    assert_eq!(
        first_delta.baseline().status(),
        ExactOwnerLedgerCoverStatus::OwnerFree
    );
    assert_eq!(first_delta.baseline().uncovered_box_count(), 1);
    assert_eq!(
        first_delta.updated().status(),
        ExactOwnerLedgerCoverStatus::Compiled(ExactOwnerCoverStatus::Incomplete(
            ExactOwnerCoverObstructionKind::NonFinite,
        ))
    );
    assert_eq!(first_delta.updated().uncovered_box_count(), 6);
    assert!(!first_delta.updated().uncovered_is_finite());
    assert_eq!(first_delta.updated().missing_terminal_count(), 0);
    assert_eq!(first_delta.updated().guard_incomplete_owner_count(), 0);

    let first_summary = first_ledger
        .proof_owner_summary(0)
        .expect("the exact K=6 cover must retain its one canonical proof owner");
    assert_eq!(first_summary.leading_lattice_point(), &[2; 6]);
    assert!(first_summary.compiled_guard_total());
    let first_dag = first_summary.semantic_dag_census();
    assert_eq!(first_dag.candidates(), 1);
    assert!(first_dag.atoms() > 0);
    assert!(first_dag.candidate_atom_references() >= first_dag.atoms());
    assert!(first_dag.memo_states() > 0);
    assert!(first_dag.nodes() > 0);
    assert_eq!(first_dag.edges(), 2 * first_dag.nodes());
    assert!(first_dag.has_reachable_incomplete());
    assert_eq!(first_ledger.proof_owner_summary(1), None);

    let second_summary = second_ledger
        .proof_owner_summary(0)
        .expect("the repeated exact K=6 cover must retain its proof owner");
    assert_eq!(first_summary, second_summary);

    let first_partition = first_ledger.try_clone_uncovered_partition().unwrap();
    let second_partition = second_ledger.try_clone_uncovered_partition().unwrap();
    assert_eq!(first_partition.boxes(), second_partition.boxes());
    assert_eq!(
        first_partition.split_operations(),
        second_partition.split_operations()
    );
}
