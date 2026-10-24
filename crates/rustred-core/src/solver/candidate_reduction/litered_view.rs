//! Crate-private, read-only view of one prepared candidate program for the
//! modular LiteRed DAG lane (`crate::solver::litered`).
//!
//! The view exposes the exact prepared formulas and the exact one-step
//! evaluator. It grants no authority beyond the candidate applier's own:
//! formulas still carry no source-replay provenance or cover proof.

use std::collections::{BTreeMap, BTreeSet};

use super::model::{CandidateReductionError, PreparedRule};
use super::reducer::{CandidateProgram, CandidateReducer};
use crate::algebra::{Coefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::family::IntegralKey;
use crate::reduction::{ReductionLimits, ReductionRequest, ReductionStatistics};
use crate::sector::OrderingPolicy;

#[derive(Clone, Copy, Debug)]
pub(crate) struct CandidateProgramView<'a, const N: usize> {
    program: &'a CandidateProgram<N>,
    limits: ReductionLimits,
}

impl<const N: usize> CandidateReducer<N> {
    pub(crate) fn program_view(&self) -> CandidateProgramView<'_, N> {
        CandidateProgramView {
            program: &self.program,
            limits: self.limits,
        }
    }
}

impl<'a, const N: usize> CandidateProgramView<'a, N> {
    pub(crate) fn family_fingerprint(&self) -> &'a str {
        self.program.family_fingerprint.as_str()
    }
    pub(crate) fn context(&self) -> &'a IndexedCoefficientContext {
        &self.program.context
    }
    pub(crate) fn root_sector(&self) -> [bool; N] {
        self.program.root_sector
    }
    pub(crate) fn ordering(&self) -> &'a OrderingPolicy {
        &self.program.ordering
    }
    pub(crate) fn limits(&self) -> ReductionLimits {
        self.limits
    }
    pub(crate) fn rules(&self) -> &'a BTreeMap<[bool; N], Vec<PreparedRule<N>>> {
        &self.program.rules
    }
    pub(crate) fn whole_piece_alternatives(&self, sector: &[bool; N]) -> &'a [usize] {
        self.program
            .whole_piece_alternatives
            .get(sector)
            .map_or(&[], Vec::as_slice)
    }
    pub(crate) fn terminals(&self) -> &'a BTreeSet<IntegralKey> {
        &self.program.terminals
    }
    pub(crate) fn zero_sectors(&self) -> &'a BTreeSet<[bool; N]> {
        &self.program.zero_sectors
    }
    pub(crate) fn source_conditions(&self) -> &'a [IndexedPolynomial] {
        &self.program.source_conditions
    }

    /// Exact baseline and selected rule ordinals of the ordinary evaluator.
    pub(crate) fn exact_selection(
        &self,
        target: &IntegralKey,
    ) -> Result<(usize, usize), CandidateReductionError> {
        let evaluator = self.program.evaluator(target, self.limits)?;
        evaluator.validate_target(target)?;
        let (baseline, selected) = evaluator.selected_rule(target)?;
        Ok((baseline.ordinal, selected.ordinal))
    }

    /// Exact one-step application of the ordinary evaluator (zero-sector
    /// children dropped, strict descent checked, RHS coalesced exactly).
    pub(crate) fn exact_step(
        &self,
        target: &IntegralKey,
    ) -> Result<BTreeMap<IntegralKey, Coefficient>, CandidateReductionError> {
        let mut request = ReductionRequest::default();
        let mut statistics = ReductionStatistics::default();
        let evaluator = self.program.evaluator(target, self.limits)?;
        evaluator.validate_target(target)?;
        evaluator.apply(target, &mut request, &mut statistics)
    }
}
