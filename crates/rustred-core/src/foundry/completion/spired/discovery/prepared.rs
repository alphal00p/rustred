use std::sync::Arc;

use symbolica::domains::finite_field::{FiniteFieldCore, ToFiniteField, Zp64};
use symbolica::prelude::Integer;

use crate::foundry::completion::guard::ExactGuardProbeWitness;
use crate::foundry::completion::source_discovery::CampaignModularProbe;

use super::super::structure::{
    SpiredPreparedRequestChunk, SpiredPreparedRowPlan, SpiredPreparedRowView,
    SpiredPreparedTermRole, SpiredStructuralPreparation, SpiredStructuralScopeIdentity,
};
use super::super::{
    DirectShiftedSourceEvaluator, ShiftedModularResidueBuffer, SpiredForbiddenTerm,
    SpiredModularHit, SpiredModularKernel, SpiredModularLimits, SpiredModularRow,
    SpiredModularStreamOutcome, SpiredValidatedPrime,
};
use super::SpiredStreamingError;

const PROBE_BASE_RESIDUES: &str = "canonical modular probe base-parameter residues";
const PROBE_INDEX_RESIDUES: &str = "canonical modular probe index residues";
const FORBIDDEN_TERMS: &str = "prepared forbidden terms in one modular row";

/// One modular probe over immutable, case-local structural row plans.
///
/// Exact source validation and prospective role classification are absent from
/// this object. They ran once in [`SpiredStructuralPreparation`]. The probe
/// retains only its raw sample identity, coefficient-evaluation buffers,
/// compact ID scratch, and the two Symbolica sparse reducers.
#[derive(Debug)]
pub(crate) struct SpiredPreparedStreamingDiscovery<'context, 'sources> {
    probe: CampaignModularProbe,
    guard_witness: Option<ExactGuardProbeWitness>,
    evaluator: DirectShiftedSourceEvaluator<'context, 'sources>,
    kernel: SpiredModularKernel<u32>,
    residue_buffer: ShiftedModularResidueBuffer,
    forbidden_terms: Vec<SpiredForbiddenTerm<u32>>,
    scope: Arc<SpiredStructuralScopeIdentity>,
    next_row_ordinal: usize,
    poisoned: bool,
}

impl<'context, 'sources> SpiredPreparedStreamingDiscovery<'context, 'sources> {
    pub(crate) fn try_new(
        preparation: &SpiredStructuralPreparation<'context, 'sources>,
        probe: &CampaignModularProbe,
        modular_limits: SpiredModularLimits,
    ) -> Result<Self, SpiredStreamingError> {
        Self::try_new_inner(preparation, probe, None, modular_limits)
    }

    pub(crate) fn try_new_guarded(
        preparation: &SpiredStructuralPreparation<'context, 'sources>,
        probe: &CampaignModularProbe,
        witness: &ExactGuardProbeWitness,
        modular_limits: SpiredModularLimits,
    ) -> Result<Self, SpiredStreamingError> {
        Self::try_new_inner(preparation, probe, Some(witness), modular_limits)
    }

    fn try_new_inner(
        preparation: &SpiredStructuralPreparation<'context, 'sources>,
        probe: &CampaignModularProbe,
        witness: Option<&ExactGuardProbeWitness>,
        modular_limits: SpiredModularLimits,
    ) -> Result<Self, SpiredStreamingError> {
        let case = preparation.case();
        match (case.stratum().guards().is_empty(), witness) {
            (false, None) => {
                return Err(SpiredStreamingError::GuardedStratumRequiresSampleWitness {
                    guard_count: case.stratum().guards().len(),
                });
            }
            (true, Some(_)) => {
                return Err(
                    crate::foundry::completion::guard::ExactGuardProbeError::GuardBlindStratum
                        .into(),
                );
            }
            _ => {}
        }
        let context = preparation.validated_sources().context();
        let expected_base = context.base().parameter_names().len();
        if probe.base_parameters().len() != expected_base {
            return Err(
                super::super::DirectShiftedSourceError::WrongBaseParameterArity {
                    expected: expected_base,
                    actual: probe.base_parameters().len(),
                }
                .into(),
            );
        }
        let exact_indices = probe.try_index_anchor_for_stratum(case.stratum())?;
        if let Some(witness) = witness {
            witness.try_validate_scope(
                context,
                case.stratum(),
                probe.base_parameters(),
                &exact_indices,
            )?;
        }
        let prime = SpiredValidatedPrime::try_new(probe.modulus())?;
        let field = prime.field().clone();
        if let Some(witness) = witness {
            witness.try_validate_modulus(&field)?;
        }
        let kernel = SpiredModularKernel::try_new_with_validated_prime(prime, modular_limits)?;
        let base_parameter_residues =
            try_canonical_residues(probe.base_parameters(), &field, PROBE_BASE_RESIDUES)?;
        let index_residues = try_canonical_residues(&exact_indices, &field, PROBE_INDEX_RESIDUES)?;
        let evaluator = DirectShiftedSourceEvaluator::try_new_from_validated(
            preparation.validated_sources(),
            probe.modulus(),
            &base_parameter_residues,
            &index_residues,
        )?;
        Ok(Self {
            probe: probe.clone(),
            guard_witness: witness.cloned(),
            evaluator,
            kernel,
            residue_buffer: ShiftedModularResidueBuffer::default(),
            forbidden_terms: Vec::new(),
            scope: preparation.scope(),
            next_row_ordinal: 0,
            poisoned: false,
        })
    }

    pub(crate) const fn probe(&self) -> &CampaignModularProbe {
        &self.probe
    }

    pub(crate) const fn guard_witness(&self) -> Option<&ExactGuardProbeWitness> {
        self.guard_witness.as_ref()
    }

    pub(crate) const fn is_poisoned(&self) -> bool {
        self.poisoned || self.kernel.is_poisoned()
    }

    pub(crate) fn rows_consumed(&self) -> usize {
        self.kernel.rows_consumed()
    }

    pub(crate) fn registered_forbidden_column_count(&self) -> usize {
        self.kernel.forbidden_columns().len()
    }

    /// Structural coordinate buffers are deliberately absent from prepared
    /// probes; exposed for a direct memory/work regression assertion.
    pub(crate) const fn retained_structural_coordinate_cells(&self) -> usize {
        0
    }

    pub(crate) fn try_consume_chunk(
        &mut self,
        chunk: &SpiredPreparedRequestChunk,
    ) -> Result<Option<SpiredModularHit>, SpiredStreamingError> {
        if self.is_poisoned() {
            return Err(SpiredStreamingError::Poisoned);
        }
        if !Arc::ptr_eq(&self.scope, chunk.scope()) {
            self.poisoned = true;
            return Err(SpiredStreamingError::PreparedPlanScopeMismatch);
        }
        if chunk.first_row_ordinal() != self.next_row_ordinal {
            self.poisoned = true;
            return Err(SpiredStreamingError::PreparedPlanChronology {
                expected_row: self.next_row_ordinal,
                actual_row: chunk.first_row_ordinal(),
            });
        }
        for row in chunk.rows() {
            if let Some(hit) = self.try_consume_row(row.view())? {
                return Ok(Some(hit));
            }
        }
        Ok(None)
    }

    /// Consume one row from a flat case-local structural tape.
    ///
    /// This is the allocation-free target-run path. The explicit scope token
    /// preserves the same non-forgeable join as a shared request chunk.
    pub(crate) fn try_consume_prepared_row(
        &mut self,
        scope: &Arc<SpiredStructuralScopeIdentity>,
        row: &SpiredPreparedRowPlan,
    ) -> Result<Option<SpiredModularHit>, SpiredStreamingError> {
        if self.is_poisoned() {
            return Err(SpiredStreamingError::Poisoned);
        }
        if !Arc::ptr_eq(&self.scope, scope) {
            self.poisoned = true;
            return Err(SpiredStreamingError::PreparedPlanScopeMismatch);
        }
        self.try_consume_row(row.view())
    }

    /// Consume one allocation-free row view from a flat role arena.
    pub(crate) fn try_consume_prepared_row_view(
        &mut self,
        scope: &Arc<SpiredStructuralScopeIdentity>,
        row: SpiredPreparedRowView<'_>,
    ) -> Result<Option<SpiredModularHit>, SpiredStreamingError> {
        if self.is_poisoned() {
            return Err(SpiredStreamingError::Poisoned);
        }
        if !Arc::ptr_eq(&self.scope, scope) {
            self.poisoned = true;
            return Err(SpiredStreamingError::PreparedPlanScopeMismatch);
        }
        self.try_consume_row(row)
    }

    /// Consume one row from the original prepared chronology while retaining
    /// the first modular hit and searching the configured post-hit window.
    ///
    /// The returned later candidate remains modular scheduling evidence only;
    /// this probe does not perform exact lifting or publish authority.
    pub(crate) fn try_consume_prepared_row_continuing(
        &mut self,
        scope: &Arc<SpiredStructuralScopeIdentity>,
        row: &SpiredPreparedRowPlan,
    ) -> Result<SpiredModularStreamOutcome, SpiredStreamingError> {
        self.try_consume_prepared_row_view_continuing(scope, row.view())
    }

    /// Allocation-free view variant of
    /// [`Self::try_consume_prepared_row_continuing`].
    pub(crate) fn try_consume_prepared_row_view_continuing(
        &mut self,
        scope: &Arc<SpiredStructuralScopeIdentity>,
        row: SpiredPreparedRowView<'_>,
    ) -> Result<SpiredModularStreamOutcome, SpiredStreamingError> {
        if self.is_poisoned() {
            return Err(SpiredStreamingError::Poisoned);
        }
        if !Arc::ptr_eq(&self.scope, scope) {
            self.poisoned = true;
            return Err(SpiredStreamingError::PreparedPlanScopeMismatch);
        }
        self.try_consume_row_continuing(row)
    }

    /// Advance over one immutable prepared chunk without admitting any row.
    ///
    /// This is the source-exclusion path. It authenticates scope and complete
    /// scheduler chronology, but deliberately does not evaluate a coefficient,
    /// register a forbidden column, or touch either Symbolica reducer. A
    /// forbidden identity first used by a later admitted row is then inserted
    /// dynamically by the kernel, which is sound because skipped rows are not
    /// members of that probe-local linear system.
    pub(crate) fn try_skip_chunk(
        &mut self,
        chunk: &SpiredPreparedRequestChunk,
    ) -> Result<(), SpiredStreamingError> {
        if self.is_poisoned() {
            return Err(SpiredStreamingError::Poisoned);
        }
        if self.kernel.has_hit() {
            return Err(super::super::SpiredModularError::AlreadyHit.into());
        }
        if !Arc::ptr_eq(&self.scope, chunk.scope()) {
            self.poisoned = true;
            return Err(SpiredStreamingError::PreparedPlanScopeMismatch);
        }
        if chunk.first_row_ordinal() != self.next_row_ordinal {
            self.poisoned = true;
            return Err(SpiredStreamingError::PreparedPlanChronology {
                expected_row: self.next_row_ordinal,
                actual_row: chunk.first_row_ordinal(),
            });
        }
        let next = self
            .next_row_ordinal
            .checked_add(chunk.rows().len())
            .ok_or(SpiredStreamingError::ResourceCountOverflow {
                resource: "prepared probe row chronology",
            })?;
        for (offset, row) in chunk.rows().iter().enumerate() {
            let expected = self.next_row_ordinal.checked_add(offset).ok_or(
                SpiredStreamingError::ResourceCountOverflow {
                    resource: "prepared probe row chronology",
                },
            )?;
            if row.ordinal() != expected {
                self.poisoned = true;
                return Err(SpiredStreamingError::PreparedPlanChronology {
                    expected_row: expected,
                    actual_row: row.ordinal(),
                });
            }
        }
        self.next_row_ordinal = next;
        Ok(())
    }

    /// Skip one row from a flat case-local structural tape without touching
    /// modular coefficient or reducer state.
    pub(crate) fn try_skip_prepared_row(
        &mut self,
        scope: &Arc<SpiredStructuralScopeIdentity>,
        row: &SpiredPreparedRowPlan,
    ) -> Result<(), SpiredStreamingError> {
        self.try_skip_prepared_row_view(scope, row.view())
    }

    /// Skip one allocation-free row view from a flat role arena.
    pub(crate) fn try_skip_prepared_row_view(
        &mut self,
        scope: &Arc<SpiredStructuralScopeIdentity>,
        row: SpiredPreparedRowView<'_>,
    ) -> Result<(), SpiredStreamingError> {
        if self.is_poisoned() {
            return Err(SpiredStreamingError::Poisoned);
        }
        if self.kernel.has_hit() {
            return Err(super::super::SpiredModularError::AlreadyHit.into());
        }
        if !Arc::ptr_eq(&self.scope, scope) {
            self.poisoned = true;
            return Err(SpiredStreamingError::PreparedPlanScopeMismatch);
        }
        if row.ordinal() != self.next_row_ordinal {
            self.poisoned = true;
            return Err(SpiredStreamingError::PreparedPlanChronology {
                expected_row: self.next_row_ordinal,
                actual_row: row.ordinal(),
            });
        }
        self.next_row_ordinal = self.next_row_ordinal.checked_add(1).ok_or(
            SpiredStreamingError::ResourceCountOverflow {
                resource: "prepared probe row chronology",
            },
        )?;
        Ok(())
    }

    /// Advance one excluded prepared row while an opt-in post-hit comparison
    /// keeps the canonical scheduler chronology alive.
    ///
    /// The row is deliberately absent from both reducers, exactly as in the
    /// pre-hit source-exclusion path. Only the authenticated tape cursor moves.
    pub(crate) fn try_skip_prepared_row_view_continuing(
        &mut self,
        scope: &Arc<SpiredStructuralScopeIdentity>,
        row: SpiredPreparedRowView<'_>,
    ) -> Result<(), SpiredStreamingError> {
        if self.is_poisoned() {
            return Err(SpiredStreamingError::Poisoned);
        }
        if !self.kernel.has_hit() {
            self.poisoned = true;
            return Err(SpiredStreamingError::Invariant {
                detail: "post-hit prepared-row skip preceded the first target pivot",
            });
        }
        if !Arc::ptr_eq(&self.scope, scope) {
            self.poisoned = true;
            return Err(SpiredStreamingError::PreparedPlanScopeMismatch);
        }
        if row.ordinal() != self.next_row_ordinal {
            self.poisoned = true;
            return Err(SpiredStreamingError::PreparedPlanChronology {
                expected_row: self.next_row_ordinal,
                actual_row: row.ordinal(),
            });
        }
        self.next_row_ordinal = self.next_row_ordinal.checked_add(1).ok_or(
            SpiredStreamingError::ResourceCountOverflow {
                resource: "prepared probe row chronology",
            },
        )?;
        Ok(())
    }

    fn try_consume_row(
        &mut self,
        row: SpiredPreparedRowView<'_>,
    ) -> Result<Option<SpiredModularHit>, SpiredStreamingError> {
        let modular_row = self.try_evaluate_row(row)?;
        let result = self.kernel.try_push_row(modular_row);
        self.finish_row(result)
    }

    fn try_consume_row_continuing(
        &mut self,
        row: SpiredPreparedRowView<'_>,
    ) -> Result<SpiredModularStreamOutcome, SpiredStreamingError> {
        let modular_row = self.try_evaluate_row(row)?;
        let result = self.kernel.try_push_row_continuing(modular_row);
        self.finish_row(result)
    }

    fn try_evaluate_row(
        &mut self,
        row: SpiredPreparedRowView<'_>,
    ) -> Result<SpiredModularRow<u32>, SpiredStreamingError> {
        if row.ordinal() != self.next_row_ordinal {
            self.poisoned = true;
            return Err(SpiredStreamingError::PreparedPlanChronology {
                expected_row: self.next_row_ordinal,
                actual_row: row.ordinal(),
            });
        }

        // Singular samples are retry outcomes. Evaluate before mutating either
        // reducer or the historical-zero frontier.
        self.evaluator
            .try_evaluate_residues(row.request(), &mut self.residue_buffer)?;
        if self.residue_buffer.len() != row.term_roles().len() {
            self.poisoned = true;
            return Err(SpiredStreamingError::PreparedTermCountMismatch {
                row: row.ordinal(),
                planned: row.term_roles().len(),
                evaluated: self.residue_buffer.len(),
            });
        }
        self.forbidden_terms.clear();
        self.forbidden_terms
            .try_reserve_exact(row.term_roles().len())
            .map_err(|_| SpiredStreamingError::AllocationFailure {
                resource: FORBIDDEN_TERMS,
                requested: row.term_roles().len(),
            })?;
        let mut target_residue = None;
        for (&role, &residue) in row.term_roles().iter().zip(self.residue_buffer.residues()) {
            match role {
                SpiredPreparedTermRole::Target => {
                    if target_residue.replace(residue).is_some() {
                        self.poisoned = true;
                        return Err(SpiredStreamingError::DuplicateTargetTerm);
                    }
                }
                SpiredPreparedTermRole::Allowed => {}
                SpiredPreparedTermRole::Forbidden(column) => {
                    if usize::try_from(column).unwrap_or(usize::MAX) >= row.forbidden_frontier() {
                        self.poisoned = true;
                        return Err(SpiredStreamingError::Invariant {
                            detail: "prepared term references a forbidden ID beyond its row frontier",
                        });
                    }
                    self.forbidden_terms
                        .push(SpiredForbiddenTerm::new(column, residue));
                }
            }
        }
        let forbidden_terms = std::mem::take(&mut self.forbidden_terms);
        Ok(SpiredModularRow::new(
            row.request().clone(),
            forbidden_terms,
            target_residue.unwrap_or(0),
        ))
    }

    fn finish_row<Outcome>(
        &mut self,
        result: Result<Outcome, super::super::SpiredModularError>,
    ) -> Result<Outcome, SpiredStreamingError> {
        match result {
            Ok(outcome) => {
                self.next_row_ordinal = self.next_row_ordinal.checked_add(1).ok_or(
                    SpiredStreamingError::ResourceCountOverflow {
                        resource: "prepared probe row chronology",
                    },
                )?;
                Ok(outcome)
            }
            Err(error) => {
                self.poisoned = true;
                Err(error.into())
            }
        }
    }
}

fn try_canonical_residues(
    values: &[i64],
    field: &Zp64,
    resource: &'static str,
) -> Result<Vec<u64>, SpiredStreamingError> {
    let mut residues = Vec::new();
    residues.try_reserve_exact(values.len()).map_err(|_| {
        SpiredStreamingError::AllocationFailure {
            resource,
            requested: values.len(),
        }
    })?;
    residues.extend(values.iter().map(|&value| {
        let element = Integer::from(value).to_finite_field(field);
        field.from_element(&element)
    }));
    Ok(residues)
}
