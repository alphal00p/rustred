use symbolica::domains::finite_field::{FiniteFieldCore, ToFiniteField, Zp64};
use symbolica::prelude::Integer;

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::completion::guard::ExactGuardProbeWitness;
use crate::foundry::completion::source_discovery::CampaignModularProbe;
use crate::foundry::completion::stratum::ProspectiveColumnClassifier;
use crate::identity::{CompletedIbpSourceRows, TranslatedSourceRequest};

use super::super::{
    DirectShiftedSourceEvaluator, ShiftedModularSourceBuffer, SpiredExecutionCase,
    SpiredForbiddenTerm, SpiredModularHit, SpiredModularKernel, SpiredModularRow,
    SpiredValidatedPrime,
};
use super::registry::{CachedColumnRole, ProspectiveShiftRegistry};
use super::{SpiredStreamingError, SpiredStreamingLimits};

const FORBIDDEN_TERMS: &str = "classified forbidden terms in one row";
const PROBE_BASE_RESIDUES: &str = "canonical modular probe base-parameter residues";
const PROBE_INDEX_RESIDUES: &str = "canonical modular probe index residues";
const PREPARATION_REQUEST_INSPECTIONS: &str = "prepared translated-source request inspections";
const PREPARATION_SOURCE_TERM_INSPECTIONS: &str = "prepared exact source-term inspections";

/// One modular point's direct translated-source stream.
///
/// Exact shifts are classified once and assigned deterministic first-seen
/// compact IDs. Modular evidence produced here remains proposal-only and must
/// be rematerialized and exact-replayed before any rule can be published.
#[derive(Debug)]
pub(crate) struct SpiredStreamingDiscovery<'context, 'sources> {
    sources: &'sources CompletedIbpSourceRows,
    arity: usize,
    probe: CampaignModularProbe,
    guard_witness: Option<ExactGuardProbeWitness>,
    evaluator: DirectShiftedSourceEvaluator<'context, 'sources>,
    registry: ProspectiveShiftRegistry,
    kernel: SpiredModularKernel<u32>,
    source_buffer: ShiftedModularSourceBuffer,
    structural_shift_scratch: Vec<i64>,
    registered_forbidden_columns: usize,
    preparation_request_inspections: usize,
    preparation_source_term_inspections: usize,
    limits: SpiredStreamingLimits,
    poisoned: bool,
}

impl<'context, 'sources> SpiredStreamingDiscovery<'context, 'sources> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_new(
        context: &'context IndexedCoefficientContext,
        sources: &'sources CompletedIbpSourceRows,
        case: &SpiredExecutionCase,
        probe: &CampaignModularProbe,
        limits: SpiredStreamingLimits,
    ) -> Result<Self, SpiredStreamingError> {
        Self::try_new_inner(context, sources, case, probe, None, limits)
    }

    /// Admit a guarded case only through a witness bound to this exact raw
    /// integer/base probe. The witness is retained with the stream and cannot
    /// be replaced by modular branch evidence.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_new_guarded(
        context: &'context IndexedCoefficientContext,
        sources: &'sources CompletedIbpSourceRows,
        case: &SpiredExecutionCase,
        probe: &CampaignModularProbe,
        witness: &ExactGuardProbeWitness,
        limits: SpiredStreamingLimits,
    ) -> Result<Self, SpiredStreamingError> {
        Self::try_new_inner(context, sources, case, probe, Some(witness), limits)
    }

    #[allow(clippy::too_many_arguments)]
    fn try_new_inner(
        context: &'context IndexedCoefficientContext,
        sources: &'sources CompletedIbpSourceRows,
        case: &SpiredExecutionCase,
        probe: &CampaignModularProbe,
        witness: Option<&ExactGuardProbeWitness>,
        limits: SpiredStreamingLimits,
    ) -> Result<Self, SpiredStreamingError> {
        if sources.family_fingerprint() != case.stratum().family_fingerprint() {
            return Err(SpiredStreamingError::WrongCaseFamily);
        }
        if context.fingerprint() != case.stratum().context_fingerprint() {
            return Err(SpiredStreamingError::WrongCaseContext);
        }
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
        // Construct the validated Symbolica field once, then reject exact
        // nonzero guards erased by this prime before allocating either sparse
        // reducer. The token is consumed by the kernel immediately afterward.
        let prime = SpiredValidatedPrime::try_new(probe.modulus())?;
        let field = prime.field().clone();
        if let Some(witness) = witness {
            witness.try_validate_modulus(&field)?;
        }
        let kernel = SpiredModularKernel::try_new_with_validated_prime(prime, limits.modular)?;
        let base_parameter_residues =
            try_canonical_residues(probe.base_parameters(), &field, PROBE_BASE_RESIDUES)?;
        let index_residues = try_canonical_residues(&exact_indices, &field, PROBE_INDEX_RESIDUES)?;
        let evaluator = DirectShiftedSourceEvaluator::try_new(
            context,
            sources,
            probe.modulus(),
            &base_parameter_residues,
            &index_residues,
            limits.evaluation,
        )?;
        let classifier = ProspectiveColumnClassifier::try_new_with_verified_snapshot(
            case.stratum().clone(),
            case.target_shift().clone(),
            case.owner_snapshot().verified_clone(),
            case.ordering(),
            limits.classification,
        )?;
        let registry = ProspectiveShiftRegistry::new(
            classifier,
            limits.max_cached_shifts,
            limits.max_cached_shift_coordinate_cells,
        );
        Ok(Self {
            sources,
            arity: case.arity(),
            probe: probe.clone(),
            guard_witness: witness.cloned(),
            evaluator,
            registry,
            kernel,
            source_buffer: ShiftedModularSourceBuffer::default(),
            structural_shift_scratch: Vec::new(),
            registered_forbidden_columns: 0,
            preparation_request_inspections: 0,
            preparation_source_term_inspections: 0,
            limits,
            poisoned: false,
        })
    }

    pub(crate) const fn is_poisoned(&self) -> bool {
        self.poisoned || self.kernel.is_poisoned()
    }

    /// The retained raw integer/chart probe that identifies all modular
    /// evidence emitted by this stream and can be reused for fresh exact replay.
    pub(crate) const fn probe(&self) -> &CampaignModularProbe {
        &self.probe
    }

    pub(crate) const fn guard_witness(&self) -> Option<&ExactGuardProbeWitness> {
        self.guard_witness.as_ref()
    }

    pub(crate) fn rows_consumed(&self) -> usize {
        self.kernel.rows_consumed()
    }

    pub(crate) fn cached_shift_count(&self) -> usize {
        self.registry.len()
    }

    pub(crate) fn forbidden_column_count(&self) -> usize {
        self.registry.forbidden_column_count()
    }

    pub(crate) const fn registered_forbidden_column_count(&self) -> usize {
        self.registered_forbidden_columns
    }

    /// Classify the structural union of a deterministic request chunk and add
    /// all newly discovered forbidden IDs to both Symbolica reducers at once.
    ///
    /// A shift absent from the registry was absent from every successfully
    /// consumed earlier row because those rows were exhaustively classified,
    /// so its pre-registration is an exact historical-structural-zero claim.
    /// Callers may omit this optimization; row admission retains a sound
    /// dynamic-registration fallback.
    pub(crate) fn try_prepare_requests(
        &mut self,
        requests: &[TranslatedSourceRequest],
    ) -> Result<usize, SpiredStreamingError> {
        if self.is_poisoned() {
            return Err(SpiredStreamingError::Poisoned);
        }
        let result = self.try_prepare_requests_inner(requests);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_prepare_requests_inner(
        &mut self,
        requests: &[TranslatedSourceRequest],
    ) -> Result<usize, SpiredStreamingError> {
        let (next_request_inspections, next_source_term_inspections) =
            self.try_preflight_preparation_work(requests)?;
        // Charge the complete admitted structural scan before mutation. Any
        // later failure poisons the coordinator, so partially completed work
        // can never be retried outside this cumulative envelope.
        self.preparation_request_inspections = next_request_inspections;
        self.preparation_source_term_inspections = next_source_term_inspections;

        let arity = self.arity;
        self.structural_shift_scratch.clear();
        self.structural_shift_scratch
            .try_reserve_exact(arity.saturating_sub(self.structural_shift_scratch.capacity()))
            .map_err(|_| SpiredStreamingError::AllocationFailure {
                resource: "structural shift preparation scratch",
                requested: arity,
            })?;
        for request in requests {
            let source_ordinal = request.source_ordinal();
            let source = self.sources.source_relation(source_ordinal).ok_or(
                super::super::DirectShiftedSourceError::SourceOrdinalOutOfRange {
                    source_ordinal,
                    source_count: self.sources.source_row_count(),
                },
            )?;
            if request.offset().len() != arity {
                return Err(super::super::DirectShiftedSourceError::WrongOffsetArity {
                    expected: arity,
                    actual: request.offset().len(),
                }
                .into());
            }
            for (term_ordinal, source_shift) in source.terms().keys().enumerate() {
                self.structural_shift_scratch.clear();
                for (position, (&offset, &term)) in request
                    .offset()
                    .values()
                    .iter()
                    .zip(source_shift.values())
                    .enumerate()
                {
                    self.structural_shift_scratch
                        .push(offset.checked_add(term).ok_or(
                            super::super::DirectShiftedSourceError::StructuralShiftOverflow {
                                term_ordinal,
                                position,
                                offset,
                                source_shift: term,
                            },
                        )?);
                }
                self.registry.try_classify(&self.structural_shift_scratch)?;
            }
        }

        let forbidden_columns = self.registry.forbidden_column_count();
        let added = forbidden_columns
            .checked_sub(self.registered_forbidden_columns)
            .ok_or(SpiredStreamingError::Invariant {
                detail: "registered forbidden count exceeds the prospective registry",
            })?;
        if added == 0 {
            return Ok(0);
        }
        let mut new_ids = Vec::new();
        new_ids
            .try_reserve_exact(added)
            .map_err(|_| SpiredStreamingError::AllocationFailure {
                resource: "pre-registered forbidden column IDs",
                requested: added,
            })?;
        for ordinal in self.registered_forbidden_columns..forbidden_columns {
            new_ids.push(u32::try_from(ordinal).map_err(|_| {
                SpiredStreamingError::ForbiddenColumnIdNotRepresentable {
                    forbidden_columns: ordinal,
                }
            })?);
        }
        let inserted = self
            .kernel
            .try_preregister_historical_zero_columns(&new_ids)?;
        if inserted != added {
            return Err(SpiredStreamingError::Invariant {
                detail: "fresh compact forbidden IDs were already present in the modular kernel",
            });
        }
        self.registered_forbidden_columns = forbidden_columns;
        Ok(added)
    }

    fn try_preflight_preparation_work(
        &self,
        requests: &[TranslatedSourceRequest],
    ) -> Result<(usize, usize), SpiredStreamingError> {
        let request_inspections = self
            .preparation_request_inspections
            .checked_add(requests.len())
            .ok_or(SpiredStreamingError::ResourceCountOverflow {
                resource: PREPARATION_REQUEST_INSPECTIONS,
            })?;
        check_preparation_limit(
            PREPARATION_REQUEST_INSPECTIONS,
            request_inspections,
            self.limits.max_preparation_request_inspections,
        )?;

        let mut source_term_inspections = self.preparation_source_term_inspections;
        for request in requests {
            let source_ordinal = request.source_ordinal();
            let source = self.sources.source_relation(source_ordinal).ok_or(
                super::super::DirectShiftedSourceError::SourceOrdinalOutOfRange {
                    source_ordinal,
                    source_count: self.sources.source_row_count(),
                },
            )?;
            if request.offset().len() != self.arity {
                return Err(super::super::DirectShiftedSourceError::WrongOffsetArity {
                    expected: self.arity,
                    actual: request.offset().len(),
                }
                .into());
            }
            source_term_inspections = source_term_inspections
                .checked_add(source.terms().len())
                .ok_or(SpiredStreamingError::ResourceCountOverflow {
                    resource: PREPARATION_SOURCE_TERM_INSPECTIONS,
                })?;
            check_preparation_limit(
                PREPARATION_SOURCE_TERM_INSPECTIONS,
                source_term_inspections,
                self.limits.max_preparation_source_term_inspections,
            )?;
        }
        Ok((request_inspections, source_term_inspections))
    }

    pub(crate) fn try_consume_chunk(
        &mut self,
        requests: &[TranslatedSourceRequest],
    ) -> Result<Option<SpiredModularHit>, SpiredStreamingError> {
        self.try_prepare_requests(requests)?;
        for request in requests {
            if let Some(hit) = self.try_consume_request(request)? {
                return Ok(Some(hit));
            }
        }
        Ok(None)
    }

    pub(crate) fn try_consume_request(
        &mut self,
        request: &TranslatedSourceRequest,
    ) -> Result<Option<SpiredModularHit>, SpiredStreamingError> {
        if self.is_poisoned() {
            return Err(SpiredStreamingError::Poisoned);
        }
        // Evaluation is transactional and precedes all registry mutation. A
        // singular sample may therefore be reported and another request or
        // probe tried without poisoning this stream.
        self.evaluator
            .try_evaluate_request(request, &mut self.source_buffer)?;

        let result = self.try_classify_and_reduce(request);
        if result.is_err() {
            self.poisoned = true;
        } else {
            self.registered_forbidden_columns = self.registry.forbidden_column_count();
        }
        result
    }

    fn try_classify_and_reduce(
        &mut self,
        request: &TranslatedSourceRequest,
    ) -> Result<Option<SpiredModularHit>, SpiredStreamingError> {
        let mut forbidden_terms = Vec::new();
        forbidden_terms
            .try_reserve_exact(self.source_buffer.len())
            .map_err(|_| SpiredStreamingError::AllocationFailure {
                resource: FORBIDDEN_TERMS,
                requested: self.source_buffer.len(),
            })?;
        let mut target_residue = None;
        for term in self.source_buffer.terms() {
            match self.registry.try_classify(term.structural_shift())? {
                CachedColumnRole::Target => {
                    if target_residue.replace(term.residue()).is_some() {
                        return Err(SpiredStreamingError::DuplicateTargetTerm);
                    }
                }
                CachedColumnRole::Allowed => {}
                CachedColumnRole::Forbidden(column) => {
                    forbidden_terms.push(SpiredForbiddenTerm::new(column, term.residue()));
                }
            }
        }
        self.kernel
            .try_push_row(SpiredModularRow::new(
                request.clone(),
                forbidden_terms,
                target_residue.unwrap_or(0),
            ))
            .map_err(SpiredStreamingError::from)
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

fn check_preparation_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SpiredStreamingError> {
    if requested > limit {
        Err(SpiredStreamingError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
