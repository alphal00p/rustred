//! Selective trusted immutable candidate inputs, independent of checkpoints.

use std::collections::BTreeSet;
use std::sync::Arc;

use rustred::family::IntegralFamily;
use rustred::persistence::SectionTag;
use rustred::reduction::ReductionLimits;
use rustred::sector::{Mask, OrderingPolicy, zero};
use rustred::solver::{
    CandidateOwnerContext, CandidateOwnerInput, CandidateOwnerPrograms, CandidateOwnerScope,
};

use crate::application::AppError;

use super::super::{CandidateBundleLimits, MAX_CANDIDATE_BUNDLE_BYTES, codec, policy};
use super::ingress::{IngressBudget, charge};

/// One explicitly selected owner from one already published immutable bundle.
/// Exactly one stored sector is required; no implicit monolithic selection.
#[derive(Clone, Copy, Debug)]
pub struct CandidateOwnerBundle<'a> {
    pub bytes: &'a [u8],
    pub owner_sector: &'a Mask,
}

/// Resource admission for an immutable selected-owner list, not coverage scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateOwnerLoadLimits {
    /// Per-bundle limits plus cumulative collections and coefficient tables.
    pub bundle: CandidateBundleLimits,
    pub max_total_input_bytes: usize,
    pub max_total_symbolica_state_bytes: usize,
    /// Conservative downset visits, including repeated masks across roots.
    /// Each distinct admitted mask is analyzed only once.
    pub max_zero_sector_visits: usize,
}

impl Default for CandidateOwnerLoadLimits {
    fn default() -> Self {
        let bundle = CandidateBundleLimits::default();
        Self {
            max_total_input_bytes: MAX_CANDIDATE_BUNDLE_BYTES,
            max_total_symbolica_state_bytes: bundle.binary_limits().max_state_bytes,
            max_zero_sector_visits: bundle.max_collection_entries,
            bundle,
        }
    }
}

struct OwnerDescriptor<const N: usize> {
    sector: [bool; N],
    root: [bool; N],
    ordering: OrderingPolicy,
}

/// Load selected single-sector candidate programs into one shared native context.
///
/// All structural records and cumulative resource limits are checked before
/// importing the first Symbolica frame. Family, exact saved solver policy,
/// rank and finite-case policy must agree; saved roots and orderings may differ.
/// The complete checkpoint loader remains a separate all-shards, locked API.
/// This function has no filesystem access, search, checkpoint mutation, graph
/// matching, source replay, terminal filtering or closure authority.
///
/// Native zero proofs are prepared once over the union of saved-root downsets;
/// missing evidence is never treated as zero. One shared core context generates
/// the ordinary source conditions once, not per owner. All decoded rules and
/// terminals remain resident: byte admission is not a peak-RSS guarantee.
/// Only load immutable payloads from a trusted matching RustRed/Symbolica stack;
/// native state import is not a hostile-input parser or rollback transaction.
pub fn load_generated_candidate_owners<const N: usize>(
    inputs: &[CandidateOwnerBundle<'_>],
    limits: CandidateOwnerLoadLimits,
    reduction_limits: ReductionLimits,
) -> Result<(Arc<IntegralFamily>, CandidateOwnerPrograms<N>), AppError> {
    if !(1..=16).contains(&N) {
        return Err(AppError::input("candidate owner/reducer arity mismatch"));
    }
    if inputs.is_empty() {
        return Err(AppError::input("selected candidate owner list is empty"));
    }
    // Owner count and all borrowed byte lengths are admitted before allocating
    // per-owner metadata or decoding a structural/native payload.
    let mut ingress = IngressBudget::new(limits.bundle, inputs.len())?;
    let mut input_bytes = 0;
    for input in inputs {
        charge(
            &mut input_bytes,
            input.bytes.len(),
            limits.max_total_input_bytes,
            "selected owner aggregate input-byte budget exceeded",
        )?;
    }
    let mut descriptors = Vec::new();
    descriptors
        .try_reserve_exact(inputs.len())
        .map_err(|_| AppError::limit("selected owner descriptor allocation failed"))?;
    let mut owners = BTreeSet::new();
    let mut roots = BTreeSet::new();
    let mut state_bytes = 0;
    let mut family_fingerprint = None;
    let mut solver_policy = None;
    for input in inputs {
        let (envelope, record, _) = codec::read_structure(input.bytes, limits.bundle)?;
        ingress.admit_structure(&envelope, &record)?;
        charge(
            &mut state_bytes,
            envelope
                .section(SectionTag::SYMBOLICA_STATE)
                .expect("checked section")
                .len(),
            limits.max_total_symbolica_state_bytes,
            "selected owner aggregate Symbolica-state byte budget exceeded",
        )?;
        if record.root_sector.len() != N || input.owner_sector.arity() != N {
            return Err(AppError::input("candidate owner/reducer arity mismatch"));
        }
        if record.sectors.len() != 1 {
            return Err(AppError::input(
                "selected owner bundle must contain exactly one sector",
            ));
        }
        if record.sectors[0].sector != input.owner_sector.active_bits() {
            return Err(AppError::input(
                "selected owner mask differs from saved sector",
            ));
        }
        let sector: [bool; N] = record.sectors[0]
            .sector
            .as_slice()
            .try_into()
            .expect("checked arity");
        if !owners.insert(sector) {
            return Err(AppError::input("duplicate selected candidate owner mask"));
        }
        let root: [bool; N] = record
            .root_sector
            .as_slice()
            .try_into()
            .expect("checked arity");
        // The common codec has already checked that the saved sector is a
        // subset of this root; never substitute a narrower fabricated root.
        roots.insert(root);
        if let Some(expected) = &family_fingerprint {
            if expected != &record.family_fingerprint {
                return Err(AppError::input("selected owner family fingerprints differ"));
            }
        } else {
            family_fingerprint = Some(record.family_fingerprint.clone());
        }
        if let Some(expected) = &solver_policy {
            if expected != &record.solver_policy {
                return Err(AppError::input(
                    "selected owner saved solver policies differ",
                ));
            }
        } else {
            solver_policy = Some(record.solver_policy.clone());
        }
        descriptors.push(OwnerDescriptor {
            sector,
            root,
            ordering: super::candidate_ordering(N, record.permutation.as_deref())?,
        });
    }
    let saved_policy = policy::parse(solver_policy.as_deref().expect("nonempty input"))?;
    let zero_masks = needed_zero_masks(&roots, limits.max_zero_sector_visits)?;

    // Only now may any native state be imported. Every borrowed input is
    // immutable for this call, so the preflight and import inspect identical
    // bytes without filesystem races, directory scans or repeated prehashes.
    let first = codec::read(inputs[0].bytes, limits.bundle)?;
    let family = Arc::new(super::reconstruct_family::<N>(&first, limits.bundle)?);
    let proofs = zero_proofs::<N>(&family, &zero_masks)?;
    let context = Arc::new(
        CandidateOwnerContext::try_new(
            Arc::clone(&family),
            CandidateOwnerScope {
                max_numerator_rank: saved_policy.max_numerator_rank,
                finite_case_policy: saved_policy.finite_case_policy,
            },
            proofs,
            reduction_limits,
        )
        .map_err(|error| AppError::execution(error.to_string()))?,
    );
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(inputs.len())
        .map_err(|_| AppError::limit("selected owner program allocation failed"))?;
    let mut first = Some(first);
    for (input, descriptor) in inputs.iter().zip(descriptors) {
        let bundle = match first.take() {
            Some(first) => first,
            None => {
                let bundle = codec::read(input.bytes, limits.bundle)?;
                let imported_family = super::reconstruct_family::<N>(&bundle, limits.bundle)?;
                if imported_family.fingerprint() != family.fingerprint() {
                    return Err(AppError::input("selected owner native family differs"));
                }
                bundle
            }
        };
        let mut solutions = codec::solutions::<N>(
            &bundle,
            context.coefficient_context(),
            context.index_variables(),
            limits.bundle,
        )?;
        if solutions.len() != 1 || solutions[0].0 != descriptor.sector {
            return Err(AppError::input(
                "selected owner reconstructed sector differs",
            ));
        }
        decoded.push(CandidateOwnerInput {
            sector: descriptor.sector,
            saved_root: descriptor.root,
            ordering: descriptor.ordering,
            solution: solutions.pop().expect("checked single sector").1,
        });
    }
    let programs = CandidateOwnerPrograms::try_new(context, decoded)
        .map_err(|error| AppError::execution(error.to_string()))?;
    Ok((family, programs))
}

fn needed_zero_masks<const N: usize>(
    roots: &BTreeSet<[bool; N]>,
    maximum_visits: usize,
) -> Result<BTreeSet<usize>, AppError> {
    let mut visits = 0;
    let mut masks = BTreeSet::new();
    for root in roots {
        let root_visits = 1usize
            .checked_shl(root.iter().filter(|&&active| active).count() as u32)
            .ok_or_else(|| {
                AppError::limit("selected owner zero-sector enumeration budget exceeded")
            })?;
        charge(
            &mut visits,
            root_visits,
            maximum_visits,
            "selected owner zero-sector enumeration budget exceeded",
        )?;
        let bits = root.iter().enumerate().fold(0usize, |bits, (i, active)| {
            bits | (usize::from(*active) << i)
        });
        let mut subset = bits;
        loop {
            masks.insert(subset);
            if subset == 0 {
                break;
            }
            subset = (subset - 1) & bits;
        }
    }
    Ok(masks)
}

fn zero_proofs<const N: usize>(
    family: &IntegralFamily,
    masks: &BTreeSet<usize>,
) -> Result<Vec<zero::Certificate>, AppError> {
    let analyzer = zero::Analyzer::try_unrestricted(family)
        .map_err(|error| AppError::execution(error.to_string()))?;
    let mut proofs = Vec::new();
    for &bits in masks {
        let sector: [bool; N] = std::array::from_fn(|axis| bits & (1 << axis) != 0);
        let mask = Mask::try_new(sector).map_err(|error| AppError::input(error.to_string()))?;
        match analyzer
            .analyze(&mask)
            .map_err(|error| AppError::execution(error.to_string()))?
        {
            zero::Decision::ProvedZero(proof) => proofs.push(proof),
            zero::Decision::Inconclusive(_) => {}
            zero::Decision::Excluded(_) => {
                return Err(AppError::internal_invariant(
                    "unrestricted zero analysis excluded a selected-owner sector",
                ));
            }
        }
    }
    Ok(proofs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_owner_zero_preparation_is_bounded_root_union_not_full_family() {
        // Distinct roots cost four visits each, although their downsets share
        // the zero mask and one line. Repeated roots cost no extra work.
        let roots = BTreeSet::from([
            [true, true, false, false],
            [false, true, true, false],
            [true, true, false, false],
        ]);
        assert!(needed_zero_masks(&roots, 7).is_err());
        assert_eq!(
            needed_zero_masks(&roots, 8).unwrap(),
            BTreeSet::from([0, 1, 2, 3, 4, 6])
        );
        // Even an arity-16 family with a one-line root costs only two visits.
        let mut root = [false; 16];
        root[15] = true;
        assert_eq!(
            needed_zero_masks(&BTreeSet::from([root]), 2).unwrap(),
            BTreeSet::from([0, 1 << 15])
        );
    }
}
