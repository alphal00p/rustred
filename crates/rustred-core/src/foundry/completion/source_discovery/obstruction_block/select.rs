use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::domains::Ring;
use symbolica::domains::finite_field::Zp64;
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use crate::identity::TranslatedSourceRequest;

use super::super::nominate::{check_limit, checked_add, try_vec};
use super::super::{SourceDiscoveryError, SourceDiscoveryLimits};
use super::{ObstructionBlockProposalBatch, ObstructionBlockProposalCandidate};

const SELECTION_CANDIDATES: &str = "source-discovery obstruction-block selection candidates";
const SELECTION_COMPARISONS: &str = "source-discovery obstruction-block selection comparisons";
const SIGNATURE_RANK_OPERATIONS: &str =
    "source-discovery obstruction-block signature-rank operations";
const SIGNATURE_RANK_CELLS: &str = "source-discovery obstruction-block signature-rank cells";
const SELECTED_REQUESTS: &str = "source-discovery obstruction-block selected requests";

/// Greedily select a bounded proposal batch by marginal Symbolica rank, then
/// structural frontier cost, with one deterministic epoch-rotated breadth
/// slot. Width one deliberately reproduces the legacy q0-only ordering.
pub(crate) fn try_select_obstruction_block_proposals(
    batch: &ObstructionBlockProposalBatch,
    maximum: usize,
    breadth_rotation: usize,
    limits: SourceDiscoveryLimits,
) -> Result<Vec<TranslatedSourceRequest>, SourceDiscoveryError> {
    validate_batch(batch)?;
    let candidate_count = batch.candidates().len();
    check_limit(
        SELECTION_CANDIDATES,
        candidate_count,
        limits.max_block_selection_candidates,
    )?;
    let desired = maximum
        .min(limits.max_block_selected_requests)
        .min(candidate_count);
    if desired == 0 {
        return Ok(Vec::new());
    }
    let comparison_upper = checked_mul(SELECTION_COMPARISONS, candidate_count, desired)?;
    check_limit(
        SELECTION_COMPARISONS,
        comparison_upper,
        limits.max_block_selection_comparisons,
    )?;
    let reducer_slots = if batch.width() > 1 && desired > 1 {
        desired - 1
    } else {
        0
    };
    let rank_probes = checked_mul(SIGNATURE_RANK_OPERATIONS, candidate_count, reducer_slots)?;
    let rank_operations_upper = checked_add(SIGNATURE_RANK_OPERATIONS, rank_probes, reducer_slots)?;
    check_limit(
        SIGNATURE_RANK_OPERATIONS,
        rank_operations_upper,
        limits.max_block_signature_rank_operations,
    )?;
    let rank_cells_upper = checked_mul(SIGNATURE_RANK_CELLS, rank_operations_upper, batch.width())?;
    check_limit(
        SIGNATURE_RANK_CELLS,
        rank_cells_upper,
        limits.max_block_signature_rank_cells,
    )?;

    let mut canonical = try_vec(SELECTION_CANDIDATES, candidate_count)?;
    canonical.extend(0..candidate_count);
    for pair in batch.candidates().windows(2) {
        match pair[0].request().cmp(pair[1].request()) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => {
                return Err(SourceDiscoveryError::Invariant {
                    detail: "obstruction-block proposal batch repeats a canonical request",
                });
            }
            std::cmp::Ordering::Greater => {
                return Err(SourceDiscoveryError::Invariant {
                    detail: "obstruction-block proposal batch is not in canonical request order",
                });
            }
        }
    }

    if desired == 1 {
        return select_single_primary(batch, &canonical);
    }

    if batch.width() == 1 {
        return select_width_one_baseline(batch, &canonical, desired);
    }

    let field = Zp64::new(batch.modulus());
    let mut reducer = catch_unwind(AssertUnwindSafe(|| {
        SparseRowReducer::new(batch.width() as u32, field.clone(), LuLMode::Full)
    }))
    .map_err(|_| SourceDiscoveryError::Invariant {
        detail: "Symbolica panicked while creating the bounded signature reducer",
    })?;
    let mut selected = try_vec(SELECTION_CANDIDATES, candidate_count)?;
    selected.resize(candidate_count, false);
    let mut output = try_vec(SELECTED_REQUESTS, desired)?;
    let main_slots = desired - 1;
    let mut primary_cut = false;
    let mut comparisons = 0usize;
    let mut rank_operations = 0usize;

    for _ in 0..main_slots {
        let mut best: Option<(usize, bool)> = None;
        for &ordinal in &canonical {
            if selected[ordinal] {
                continue;
            }
            let candidate = &batch.candidates()[ordinal];
            let cuts_primary = !field.is_zero(&candidate.signature()[0]);
            if !primary_cut && !cuts_primary {
                continue;
            }
            comparisons = checked_add(SELECTION_COMPARISONS, comparisons, 1)?;
            check_limit(
                SELECTION_COMPARISONS,
                comparisons,
                limits.max_block_selection_comparisons,
            )?;
            rank_operations = checked_add(SIGNATURE_RANK_OPERATIONS, rank_operations, 1)?;
            check_limit(
                SIGNATURE_RANK_OPERATIONS,
                rank_operations,
                limits.max_block_signature_rank_operations,
            )?;
            let gain = signature_increases_rank(&reducer, candidate, &field)?;
            if best.is_none_or(|(best_ordinal, best_gain)| {
                proposal_preference(
                    candidate,
                    gain,
                    &batch.candidates()[best_ordinal],
                    best_gain,
                )
                .is_lt()
            }) {
                best = Some((ordinal, gain));
            }
        }
        let Some((ordinal, _)) = best else {
            if !primary_cut {
                return Err(SourceDiscoveryError::Invariant {
                    detail: "nonempty primary residual census has no q0-cutting block proposal",
                });
            }
            break;
        };
        rank_operations = checked_add(SIGNATURE_RANK_OPERATIONS, rank_operations, 1)?;
        check_limit(
            SIGNATURE_RANK_OPERATIONS,
            rank_operations,
            limits.max_block_signature_rank_operations,
        )?;
        add_signature(&mut reducer, &batch.candidates()[ordinal], &field)?;
        primary_cut |= !field.is_zero(&batch.candidates()[ordinal].signature()[0]);
        selected[ordinal] = true;
        output.push(batch.candidates()[ordinal].request().clone());
    }

    // Exactly one breadth slot remains when the requested batch has width.
    // Canonical cyclic order makes the slot independent of candidate arrival
    // order while intentionally admitting zero/dependent signatures.
    if output.len() < desired {
        let start = breadth_rotation % canonical.len();
        for offset in 0..canonical.len() {
            let ordinal = canonical[(start + offset) % canonical.len()];
            if !selected[ordinal] {
                selected[ordinal] = true;
                output.push(batch.candidates()[ordinal].request().clone());
                break;
            }
        }
    }
    if !primary_cut {
        return Err(SourceDiscoveryError::Invariant {
            detail: "block proposal selection did not retain a q0-cutting row",
        });
    }
    check_limit(
        SELECTED_REQUESTS,
        output.len(),
        limits.max_block_selected_requests,
    )?;
    Ok(output)
}

fn select_single_primary(
    batch: &ObstructionBlockProposalBatch,
    canonical: &[usize],
) -> Result<Vec<TranslatedSourceRequest>, SourceDiscoveryError> {
    let field = Zp64::new(batch.modulus());
    let mut best: Option<usize> = None;
    for &ordinal in canonical {
        if field.is_zero(&batch.candidates()[ordinal].signature()[0]) {
            continue;
        }
        if best.is_none_or(|best| {
            structural_preference(&batch.candidates()[ordinal], &batch.candidates()[best]).is_lt()
        }) {
            best = Some(ordinal);
        }
    }
    let Some(best) = best else {
        return Err(SourceDiscoveryError::Invariant {
            detail: "cap-one obstruction-block selection has no q0-cutting row",
        });
    };
    let mut output = Vec::new();
    output
        .try_reserve_exact(1)
        .map_err(|_| SourceDiscoveryError::AllocationFailure {
            resource: SELECTED_REQUESTS,
            requested: 1,
        })?;
    output.push(batch.candidates()[best].request().clone());
    Ok(output)
}

fn select_width_one_baseline(
    batch: &ObstructionBlockProposalBatch,
    canonical: &[usize],
    desired: usize,
) -> Result<Vec<TranslatedSourceRequest>, SourceDiscoveryError> {
    let field = Zp64::new(batch.modulus());
    let mut selected = try_vec(SELECTION_CANDIDATES, batch.candidates().len())?;
    selected.resize(batch.candidates().len(), false);
    let mut output = Vec::new();
    output
        .try_reserve_exact(desired)
        .map_err(|_| SourceDiscoveryError::AllocationFailure {
            resource: SELECTED_REQUESTS,
            requested: desired,
        })?;
    for _ in 0..desired {
        let mut best: Option<usize> = None;
        for &ordinal in canonical {
            if selected[ordinal] || field.is_zero(&batch.candidates()[ordinal].signature()[0]) {
                continue;
            }
            if best.is_none_or(|best| {
                structural_preference(&batch.candidates()[ordinal], &batch.candidates()[best])
                    .is_lt()
            }) {
                best = Some(ordinal);
            }
        }
        let Some(best) = best else { break };
        selected[best] = true;
        output.push(batch.candidates()[best].request().clone());
    }
    if output.is_empty() {
        return Err(SourceDiscoveryError::Invariant {
            detail: "width-one block proposal selection has no q0-cutting row",
        });
    }
    Ok(output)
}

fn validate_batch(batch: &ObstructionBlockProposalBatch) -> Result<(), SourceDiscoveryError> {
    if batch.width() == 0
        || batch.width() > 4
        || batch
            .candidates()
            .iter()
            .any(|candidate| candidate.signature().len() != batch.width())
    {
        return Err(SourceDiscoveryError::Invariant {
            detail: "obstruction-block proposal batch has an invalid signature width",
        });
    }
    Ok(())
}

fn proposal_preference(
    left: &ObstructionBlockProposalCandidate,
    left_gain: bool,
    right: &ObstructionBlockProposalCandidate,
    right_gain: bool,
) -> std::cmp::Ordering {
    right_gain
        .cmp(&left_gain)
        .then_with(|| structural_preference(left, right))
}

fn structural_preference(
    left: &ObstructionBlockProposalCandidate,
    right: &ObstructionBlockProposalCandidate,
) -> std::cmp::Ordering {
    left.score()
        .compare_preference(right.score())
        .then_with(|| left.request().cmp(right.request()))
}

fn signature_increases_rank(
    reducer: &SparseRowReducer<Zp64>,
    candidate: &ObstructionBlockProposalCandidate,
    field: &Zp64,
) -> Result<bool, SourceDiscoveryError> {
    let mut probe = reducer.clone();
    add_signature_inner(&mut probe, candidate, field)
}

fn add_signature(
    reducer: &mut SparseRowReducer<Zp64>,
    candidate: &ObstructionBlockProposalCandidate,
    field: &Zp64,
) -> Result<(), SourceDiscoveryError> {
    let _ = add_signature_inner(reducer, candidate, field)?;
    Ok(())
}

fn add_signature_inner(
    reducer: &mut SparseRowReducer<Zp64>,
    candidate: &ObstructionBlockProposalCandidate,
    field: &Zp64,
) -> Result<bool, SourceDiscoveryError> {
    let nonzero = candidate
        .signature()
        .iter()
        .filter(|coefficient| !field.is_zero(coefficient))
        .count();
    let mut values = try_vec(SIGNATURE_RANK_CELLS, nonzero)?;
    let mut columns = try_vec(SIGNATURE_RANK_CELLS, nonzero)?;
    for (column, coefficient) in candidate.signature().iter().enumerate() {
        if !field.is_zero(coefficient) {
            values.push(coefficient.clone());
            columns.push(column as u32);
        }
    }
    catch_unwind(AssertUnwindSafe(|| reducer.add_row(&values, &columns)))
        .map(|pivot| pivot.is_some())
        .map_err(|_| SourceDiscoveryError::Invariant {
            detail: "Symbolica panicked while updating bounded obstruction-signature rank",
        })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SourceDiscoveryError> {
    left.checked_mul(right)
        .ok_or(SourceDiscoveryError::ResourceCountOverflow { resource })
}
