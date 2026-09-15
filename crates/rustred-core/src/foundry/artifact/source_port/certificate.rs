//! Original-source joins retained for checked program lowering.
//!
//! These private records do not establish closure or construct an artifact.
//! They must be retained with the authenticated family/context, coordinate
//! case, full target identity and exact application cover by their owner.
//! No preconditioned basis ordinal, compact Power or GPLU state is persisted.

use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::identity::RowId;
use crate::solver::{SectorRule, Seed};

use super::{SourcePortAuditError, error};

/// Identify exactly which ordinary-row normalization the weights multiply.
/// Conversion from the adapter to original `ParametricRelation` rows verifies
/// every normalization scale, translates it, and multiplies the source weight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OriginalRowNormalization {
    NativeDenominatorClearedOrdinaryV1,
    OriginalGeneratorOrdinaryV1,
}

#[derive(Debug)]
pub(super) struct OriginalSourceContribution<const N: usize> {
    pub source_row: RowId,
    /// Original source translation relative to canonical target indices.
    /// Fixed coordinates are specialized only AFTER translating the source.
    pub offset: [i64; N],
    pub weight: Coefficient,
}

/// The associated proof is the complete combined residual
/// `sum(weight * FULL translated original row) - FULL target identity`.
/// There are deliberately no per-source deletion instructions: a physical
/// product may vanish only after the weighted terms have been combined.
/// Cold loading must recompute this residual and its exact sign-cell proof;
/// no sparse elimination, preconditioning or source search is needed then.
#[derive(Debug)]
pub(super) struct OriginalSourceReplay<const N: usize> {
    pub normalization: OriginalRowNormalization,
    pub contributions: Vec<OriginalSourceContribution<N>>,
    /// Translated pre-cancellation source conditions, including parameter-only
    /// poles which the integer-case geometry deliberately does not represent.
    pub source_conditions: Vec<CoefficientPolynomial>,
}

impl<const N: usize> OriginalSourceReplay<N> {
    /// Call only after full weighted-original replay. This preserves every
    /// weight/request association before deleting algebraically zero weights.
    pub(super) fn retain_checked(
        requests: Vec<(RowId, [i64; N])>,
        weights: Vec<Coefficient>,
    ) -> Result<Self, SourcePortAuditError> {
        if requests.len() != weights.len() {
            return Err(error("ordinary replay request/weight count mismatch"));
        }
        let contributions = requests
            .into_iter()
            .zip(weights)
            .filter_map(|((source_row, offset), weight)| {
                (!weight.is_zero()).then_some(OriginalSourceContribution {
                    source_row,
                    offset,
                    weight,
                })
            })
            .collect();
        Ok(Self {
            normalization: OriginalRowNormalization::NativeDenominatorClearedOrdinaryV1,
            contributions,
            source_conditions: Vec::new(),
        })
    }
}

pub(super) fn source_offset<const N: usize>(
    rule: &SectorRule<N>,
    seed: &Seed<N>,
    canonical_translation: &[i16; N],
) -> [i64; N] {
    std::array::from_fn(|axis| {
        if seed.integral[axis].is_symbolic() {
            i64::from(seed.shifts[axis]) + i64::from(canonical_translation[axis])
        } else {
            i64::from(seed.integral[axis].value()) - i64::from(rule.candidate.target[axis].value())
        }
    })
}
