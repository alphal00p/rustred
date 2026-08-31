use symbolica::domains::Ring;
use symbolica::domains::finite_field::Zp64;

use crate::identity::{IntegralShift, TranslatedSourceRequest};

use super::super::SourceDiscoveryLimits;
use super::{
    ObstructionBlockProposalBatch, ObstructionBlockProposalCandidate,
    ObstructionBlockProposalScore, try_select_obstruction_block_proposals,
};

const PRIME: u64 = 1_000_000_007;

fn request(offset: i64) -> TranslatedSourceRequest {
    TranslatedSourceRequest::new(0, IntegralShift::try_new([offset]).unwrap())
}

fn candidate(
    offset: i64,
    signature: &[bool],
    new_forbidden: usize,
    new_physical: usize,
    overlap: usize,
    width: usize,
) -> ObstructionBlockProposalCandidate {
    let field = Zp64::new(PRIME);
    ObstructionBlockProposalCandidate::from_parts(
        request(offset),
        signature
            .iter()
            .map(|&nonzero| if nonzero { field.one() } else { field.zero() })
            .collect(),
        ObstructionBlockProposalScore::new(new_forbidden, new_physical, overlap, width),
    )
}

fn selected(
    candidates: Vec<ObstructionBlockProposalCandidate>,
    width: usize,
    maximum: usize,
    rotation: usize,
    limits: SourceDiscoveryLimits,
) -> Vec<TranslatedSourceRequest> {
    let batch = ObstructionBlockProposalBatch::from_candidates_for_test(PRIME, width, candidates);
    try_select_obstruction_block_proposals(&batch, maximum, rotation, limits).unwrap()
}

#[test]
fn marginal_rank_precedes_cost_and_one_rotated_breadth_slot_admits_zero() {
    let candidates = vec![
        candidate(0, &[true, false, false, false], 0, 0, 9, 1),
        candidate(1, &[true, true, false, false], 3, 3, 1, 9),
        candidate(2, &[true, false, true, false], 4, 4, 1, 9),
        candidate(3, &[true, true, true, false], 0, 0, 99, 1),
        candidate(4, &[false, false, false, false], 0, 0, 100, 1),
    ];
    assert_eq!(
        selected(
            candidates.clone(),
            4,
            1,
            4,
            SourceDiscoveryLimits::default(),
        ),
        vec![request(3)],
        "a cap-one batch must retain a q0-cutting row"
    );
    assert_eq!(
        selected(
            candidates.clone(),
            4,
            4,
            4,
            SourceDiscoveryLimits::default(),
        ),
        vec![request(3), request(0), request(1), request(4)]
    );

    let permuted = vec![
        candidates[4].clone(),
        candidates[2].clone(),
        candidates[0].clone(),
        candidates[3].clone(),
        candidates[1].clone(),
    ];
    let permuted = ObstructionBlockProposalBatch::from_candidates_for_test(PRIME, 4, permuted);
    assert_eq!(
        try_select_obstruction_block_proposals(&permuted, 4, 4, SourceDiscoveryLimits::default(),)
            .unwrap_err(),
        super::super::SourceDiscoveryError::Invariant {
            detail: "obstruction-block proposal batch is not in canonical request order",
        },
        "the evaluator, rather than an unmetered selector sort, owns canonical arrival order"
    );
}

#[test]
fn width_one_is_exact_q0_only_structural_order_without_breadth_drift() {
    let candidates = vec![
        candidate(0, &[true], 1, 1, 99, 1),
        candidate(1, &[true], 0, 2, 99, 1),
        candidate(2, &[true], 0, 0, 1, 1),
        candidate(3, &[true], 0, 0, 2, 9),
        candidate(4, &[false], 0, 0, 999, 1),
    ];
    assert_eq!(
        selected(
            candidates,
            1,
            4,
            usize::MAX,
            SourceDiscoveryLimits::default(),
        ),
        vec![request(3), request(2), request(1), request(0)]
    );
}

#[test]
fn selection_work_is_preflighted_at_exact_boundaries() {
    let candidates = vec![
        candidate(0, &[true, false], 0, 0, 2, 2),
        candidate(1, &[true, true], 0, 0, 2, 2),
        candidate(2, &[false, false], 0, 0, 2, 2),
    ];
    let batch = ObstructionBlockProposalBatch::from_candidates_for_test(PRIME, 2, candidates);
    let mut exact = SourceDiscoveryLimits::default();
    exact.max_block_selection_candidates = 3;
    exact.max_block_selection_comparisons = 9;
    exact.max_block_signature_rank_operations = 8;
    exact.max_block_signature_rank_cells = 16;
    exact.max_block_selected_requests = 3;
    assert!(try_select_obstruction_block_proposals(&batch, 3, 2, exact).is_ok());

    let mut below = exact;
    below.max_block_selection_comparisons = 8;
    assert_eq!(
        try_select_obstruction_block_proposals(&batch, 3, 2, below).unwrap_err(),
        super::super::SourceDiscoveryError::ResourceLimit {
            resource: "source-discovery obstruction-block selection comparisons",
            requested: 9,
            limit: 8,
        }
    );
    let mut below_rank = exact;
    below_rank.max_block_signature_rank_cells = 15;
    assert_eq!(
        try_select_obstruction_block_proposals(&batch, 3, 2, below_rank).unwrap_err(),
        super::super::SourceDiscoveryError::ResourceLimit {
            resource: "source-discovery obstruction-block signature-rank cells",
            requested: 16,
            limit: 15,
        }
    );
}

#[test]
fn cap_one_selection_is_structural_and_uses_no_signature_reducer_budget() {
    let candidates = vec![
        candidate(0, &[true, false], 0, 0, 1, 1),
        candidate(1, &[false, true], 9, 9, 9, 9),
        candidate(2, &[true, true], 0, 0, 2, 1),
    ];
    let batch = ObstructionBlockProposalBatch::from_candidates_for_test(PRIME, 2, candidates);
    let mut exact = SourceDiscoveryLimits::default();
    exact.max_block_selection_candidates = 3;
    exact.max_block_selection_comparisons = 3;
    exact.max_block_signature_rank_operations = 0;
    exact.max_block_signature_rank_cells = 0;
    exact.max_block_selected_requests = 1;
    assert_eq!(
        try_select_obstruction_block_proposals(&batch, 1, usize::MAX, exact).unwrap(),
        vec![request(2)],
    );

    let mut below = exact;
    below.max_block_selection_comparisons = 2;
    assert_eq!(
        try_select_obstruction_block_proposals(&batch, 1, usize::MAX, below).unwrap_err(),
        super::super::SourceDiscoveryError::ResourceLimit {
            resource: "source-discovery obstruction-block selection comparisons",
            requested: 3,
            limit: 2,
        }
    );
}

#[test]
fn q0_crosscheck_preflights_candidate_windows_and_exact_equality_work() {
    let candidates = vec![
        candidate(0, &[true, false], 0, 0, 1, 1),
        candidate(1, &[false, true], 0, 0, 1, 1),
        candidate(2, &[true, true], 0, 0, 1, 1),
    ];
    let batch = ObstructionBlockProposalBatch::from_candidates_for_test(PRIME, 2, candidates);
    let primary = [request(0), request(2)];
    let mut exact = SourceDiscoveryLimits::default();
    // Two canonical candidate windows plus two q0 equality comparisons.
    exact.max_block_primary_crosscheck_comparisons = 4;
    batch
        .try_verify_primary_requests_for_test(&primary, exact)
        .unwrap();

    let mut below = exact;
    below.max_block_primary_crosscheck_comparisons = 3;
    assert_eq!(
        batch
            .try_verify_primary_requests_for_test(&primary, below)
            .unwrap_err(),
        super::super::SourceDiscoveryError::ResourceLimit {
            resource: "source-discovery obstruction-block q0 residual cross-check comparisons",
            requested: 4,
            limit: 3,
        }
    );
}
