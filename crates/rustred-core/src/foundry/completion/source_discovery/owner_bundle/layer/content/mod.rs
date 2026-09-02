//! Canonical one-time content identity for a published solved sector.
//!
//! Discovery samples and live pointer tokens are intentionally absent. Exact
//! source, rewrite, guard, descent, lower-owner, and terminal semantics are
//! streamed through a byte-bounded BLAKE3 encoder at publication only.

mod algebra;
mod cell;
mod encoder;
mod exact;
mod semantic;

use std::cmp::Ordering;

use crate::foundry::completion::frame::admission::ExactCircuitSemanticDag;
use crate::foundry::completion::frame::admission::ExactOwnerCoverStatus;
use crate::foundry::completion::source_discovery::{AdmittedExactRuleCandidate, FreshTaskEpoch};
use crate::foundry::completion::stratum::{StratumRegistryError, StratumRegistryLimits};

use super::super::{
    ClosedExactExecutableOwnerCover, ExactOwnerContentOrderKey, ExactSemanticExecutableOwner,
};
use super::model::ClosedSectorLayerContentId;
use algebra::{append_integral_key, append_mask, append_u64_slice};
use cell::{append_rule_cell, append_rule_cell_with_first_guard_override};
use encoder::BoundedContentHasher;
use exact::{append_exact_circuit, append_physical_plan};
use semantic::{append_guard_refinement, append_semantic_dag};

const DOMAIN: &[u8] = b"rustred.closed-sector-layer-content.v2\0";
const OWNER_ORDER_DOMAIN: &[u8] = b"rustred.exact-executable-owner-order.v2\0";
const OWNER_ENCODED_RESOURCE: &str = "exact executable owner canonical encoded bytes";
const OWNER_COLLISION_RESOURCE: &str = "exact executable owner collision-fallback canonical bytes";

/// Stream the complete self-delimiting owner encoding into one compact
/// digest/length order key. The byte limit still applies to the exact encoded
/// payload even though only the fixed-size key remains resident afterward.
pub(in crate::foundry::completion::source_discovery::owner_bundle) fn try_build_owner_content_key(
    epoch: &FreshTaskEpoch,
    semantic: &ExactCircuitSemanticDag,
    executable: &[AdmittedExactRuleCandidate],
    limit: usize,
) -> Result<ExactOwnerContentOrderKey, StratumRegistryError> {
    let mut output = BoundedContentHasher::digest(limit, OWNER_ENCODED_RESOURCE);
    output.raw(OWNER_ORDER_DOMAIN)?;
    append_owner_content(&mut output, epoch, semantic, executable, None, None)?;
    let (digest, encoded_len) = output.finish_digest();
    Ok(ExactOwnerContentOrderKey::from_digest_and_len(
        digest,
        encoded_len,
    ))
}

/// Resolve an equal compact digest/length key without assuming collision
/// resistance. This path is intentionally fallible and cold: it reconstructs
/// each bounded exact byte stream and applies the original lexicographic
/// structural comparison.
pub(in crate::foundry::completion::source_discovery::owner_bundle) fn try_compare_owner_content_exact(
    left: &ExactSemanticExecutableOwner,
    right: &ExactSemanticExecutableOwner,
) -> Result<Ordering, StratumRegistryError> {
    let left = try_build_exact_owner_content(left)?;
    let right = try_build_exact_owner_content(right)?;
    Ok(left.cmp(&right))
}

fn try_build_exact_owner_content(
    owner: &ExactSemanticExecutableOwner,
) -> Result<Box<[u8]>, StratumRegistryError> {
    let expected_len = owner.content_order_key().encoded_len();
    let mut output = BoundedContentHasher::exact(expected_len, OWNER_COLLISION_RESOURCE);
    output.raw(OWNER_ORDER_DOMAIN)?;
    append_owner_content(
        &mut output,
        owner.epoch(),
        owner.semantic(),
        owner.executable_candidates(),
        None,
        None,
    )?;
    let bytes = output.finish_exact();
    if bytes.len() != expected_len {
        return Err(StratumRegistryError::Invariant {
            detail: "owner compact key encoded length differs from exact collision replay",
        });
    }
    Ok(bytes)
}

pub(super) fn try_build_content_id(
    sealed: &ClosedExactExecutableOwnerCover,
    limits: StratumRegistryLimits,
) -> Result<ClosedSectorLayerContentId, StratumRegistryError> {
    try_build_content_id_with_overrides(sealed, limits, None, None)
}

#[cfg(test)]
pub(super) fn try_build_content_id_with_first_circuit_for_test(
    sealed: &ClosedExactExecutableOwnerCover,
    limits: StratumRegistryLimits,
    circuit: &crate::foundry::completion::frame::exact::ExactTargetCircuit,
) -> Result<ClosedSectorLayerContentId, StratumRegistryError> {
    try_build_content_id_with_overrides(sealed, limits, Some(circuit), None)
}

#[cfg(test)]
pub(super) fn try_build_content_id_with_first_cell_guard_for_test(
    sealed: &ClosedExactExecutableOwnerCover,
    limits: StratumRegistryLimits,
    guard: &crate::algebra::IndexedPolynomial,
) -> Result<ClosedSectorLayerContentId, StratumRegistryError> {
    try_build_content_id_with_overrides(sealed, limits, None, Some(guard))
}

fn try_build_content_id_with_overrides(
    sealed: &ClosedExactExecutableOwnerCover,
    limits: StratumRegistryLimits,
    first_circuit_override: Option<&crate::foundry::completion::frame::exact::ExactTargetCircuit>,
    first_cell_guard_override: Option<&crate::algebra::IndexedPolynomial>,
) -> Result<ClosedSectorLayerContentId, StratumRegistryError> {
    let cover = sealed.executable_cover();
    let proof = cover.proof_cover();
    let mut output = BoundedContentHasher::new(limits.max_owner_identity_bytes);
    output.raw(DOMAIN)?;

    // Publication scope and the immutable lower-layer dependency.
    output.text(proof.family_fingerprint())?;
    output.text(proof.context_fingerprint())?;
    append_mask(&mut output, proof.sector())?;
    output.text(&proof.ordering().stable_id())?;
    output.text(proof.owner_snapshot_id().as_str())?;
    output.text(sealed.predecessor_snapshot().id().as_str())?;
    // Closure is authority only on this exact finite carrier.  Owner regions
    // may use contextual `None` endpoints at its boundary, so encoding only
    // those normalized regions would permit distinct proof universes to share
    // a layer identity.
    append_u64_slice(&mut output, proof.closure_carrier().lower())?;
    output.count(proof.closure_carrier().upper().len())?;
    for &upper in proof.closure_carrier().upper() {
        match upper {
            Some(value) => {
                output.tag(1)?;
                output.u64(value)?;
            }
            None => output.tag(0)?,
        }
    }

    // Canonical geometric owner skeleton. Semantic circuits are encoded from
    // the paired executable owners below, while the complete exact regions
    // and their totality claims live only in the proof cover. Both finite and
    // unbounded upper endpoints are committed: equal lower corners do not
    // imply equal cylinders.
    output.count(proof.owners().len())?;
    for owner in proof.owners() {
        output.usize(owner.id().ordinal())?;
        append_u64_slice(&mut output, owner.region().lower())?;
        output.count(owner.region().upper().len())?;
        for &upper in owner.region().upper() {
            match upper {
                Some(value) => {
                    output.tag(1)?;
                    output.u64(value)?;
                }
                None => output.tag(0)?,
            }
        }
        output.boolean(owner.is_guard_total())?;
    }

    output.count(cover.owners().len())?;
    for (owner_ordinal, owner) in cover.owners().iter().enumerate() {
        append_owner_content(
            &mut output,
            owner.epoch(),
            owner.semantic(),
            owner.executable_candidates(),
            (owner_ordinal == 0)
                .then_some(first_circuit_override)
                .flatten(),
            (owner_ordinal == 0)
                .then_some(first_cell_guard_override)
                .flatten(),
        )?;
    }

    // Both proof and executable terminal tables are committed. They are
    // deliberately retained separately because selection reads each table at
    // a different boundary, even though the compiler has joined them.
    output.count(proof.terminals().len())?;
    for terminal in proof.terminals() {
        append_u64_slice(&mut output, terminal.point().coordinates())?;
        append_integral_key(&mut output, terminal.integral())?;
    }
    output.count(cover.terminals().len())?;
    for terminal in cover.terminals() {
        append_integral_key(&mut output, terminal)?;
    }

    output.count(proof.finite_point_owners().len())?;
    for owner in proof.finite_point_owners() {
        append_u64_slice(&mut output, owner.point().coordinates())?;
        output.usize(owner.owner().ordinal())?;
        output.usize(owner.candidate_ordinal())?;
        append_exact_circuit(&mut output, owner.circuit())?;
    }

    let uncovered = proof.uncovered_partition();
    output.count(uncovered.boxes().len())?;
    for cell in uncovered.boxes() {
        append_u64_slice(&mut output, cell.lower())?;
        output.count(cell.upper().len())?;
        for &upper in cell.upper() {
            match upper {
                Some(value) => {
                    output.tag(1)?;
                    output.u64(value)?;
                }
                None => output.tag(0)?,
            }
        }
    }
    output.usize(uncovered.split_operations())?;
    output.count(proof.missing_terminals().len())?;
    for point in proof.missing_terminals() {
        append_u64_slice(&mut output, point.coordinates())?;
    }
    output.count(proof.guard_incomplete_owners().len())?;
    for owner in proof.guard_incomplete_owners() {
        output.usize(owner.ordinal())?;
    }
    match proof.status() {
        ExactOwnerCoverStatus::Closed => output.tag(0)?,
        ExactOwnerCoverStatus::Incomplete(obstruction) => {
            output.tag(1)?;
            output.tag(match obstruction {
                crate::foundry::completion::frame::admission::ExactOwnerCoverObstructionKind::NonFinite => 0,
                crate::foundry::completion::frame::admission::ExactOwnerCoverObstructionKind::GuardIncomplete => 1,
                crate::foundry::completion::frame::admission::ExactOwnerCoverObstructionKind::FiniteTerminalOwnership => 2,
            })?;
        }
    }

    Ok(ClosedSectorLayerContentId::from_bounded_digest(
        output.finish(),
    ))
}

fn append_owner_content(
    output: &mut BoundedContentHasher,
    epoch: &FreshTaskEpoch,
    semantic: &ExactCircuitSemanticDag,
    executable: &[AdmittedExactRuleCandidate],
    first_circuit_override: Option<&crate::foundry::completion::frame::exact::ExactTargetCircuit>,
    first_cell_guard_override: Option<&crate::algebra::IndexedPolynomial>,
) -> Result<(), StratumRegistryError> {
    append_physical_plan(output, epoch.plan())?;
    append_semantic_dag(output, semantic)?;
    output.count(executable.len())?;
    for (candidate_ordinal, candidate) in executable.iter().enumerate() {
        let is_first = candidate_ordinal == 0;
        append_exact_circuit(
            output,
            if is_first {
                first_circuit_override.unwrap_or_else(|| candidate.circuit())
            } else {
                candidate.circuit()
            },
        )?;
        if is_first && first_cell_guard_override.is_some() {
            append_rule_cell_with_first_guard_override(
                output,
                candidate.cell(),
                first_cell_guard_override,
            )?;
        } else {
            append_rule_cell(output, candidate.cell())?;
        }
        append_guard_refinement(output, candidate.guard_refinement())?;
    }
    Ok(())
}
