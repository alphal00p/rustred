use std::sync::Arc;

use symbolica::domains::finite_field::FiniteFieldElement;

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::completion::frame::modular::{ModularPhysicalFrame, ModularSampleFingerprint};
use crate::identity::{CompletedIbpSourceRows, RowId, TranslatedSource, TranslatedSourceRequest};
use crate::sector::Mask;

use super::super::nominate::{check_limit, checked_add, try_vec};
use super::super::{OrdinarySourceIncidenceIndex, SourceDiscoveryError, SourceDiscoveryLimits};

const CACHE_ROWS: &str = "source-discovery probe row-cache rows";
const CACHE_VALUES: &str = "source-discovery probe row-cache finite-field value cells";
const CACHE_REQUEST_COORDINATES: &str = "source-discovery probe row-cache request coordinate cells";
const CACHE_LOOKUP_COMPARISONS: &str = "source-discovery probe row-cache lookup comparisons";
const CACHE_PHYSICAL_EVALUATIONS: &str = "source-discovery probe row-cache physical evaluations";
const CACHE_INSERTION_MOVES: &str = "source-discovery probe row-cache insertion moves";
const CACHE_SOURCE_ROWS: &str = "source-discovery probe row-cache source chronology";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProbeRowEvaluationCacheTelemetry {
    rows: usize,
    value_cells: usize,
    lookup_comparisons: usize,
    physical_evaluations: usize,
    cache_hits: usize,
    insertion_moves: usize,
}

impl ProbeRowEvaluationCacheTelemetry {
    pub(crate) const fn rows(self) -> usize {
        self.rows
    }

    pub(crate) const fn value_cells(self) -> usize {
        self.value_cells
    }

    pub(crate) const fn lookup_comparisons(self) -> usize {
        self.lookup_comparisons
    }

    pub(crate) const fn physical_evaluations(self) -> usize {
        self.physical_evaluations
    }

    pub(crate) const fn cache_hits(self) -> usize {
        self.cache_hits
    }

    pub(crate) const fn insertion_moves(self) -> usize {
        self.insertion_moves
    }
}

#[derive(Clone, Debug)]
struct CachedRowEvaluation {
    request: TranslatedSourceRequest,
    values: Arc<[FiniteFieldElement<u64>]>,
}

/// Bounded complete-row cache private to one declared modular probe.
///
/// The cache is bound to the exact incidence owner, family/context strings,
/// completed source chronology, first sampled sector, and full modulus/point
/// fingerprint. It never retains plan/source ordinals as keys and cannot
/// supply exact-replay or sampled-dual authority.
#[derive(Debug)]
pub(crate) struct ProbeRowEvaluationCache {
    incidence_identity: Arc<()>,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    arity: usize,
    source_rows: Box<[RowId]>,
    sector: Option<Mask>,
    sample: Option<ModularSampleFingerprint>,
    entries: Vec<CachedRowEvaluation>,
    telemetry: ProbeRowEvaluationCacheTelemetry,
}

impl ProbeRowEvaluationCache {
    pub(crate) fn try_new(
        incidence: &OrdinarySourceIncidenceIndex<'_>,
        completed: &CompletedIbpSourceRows,
        limits: SourceDiscoveryLimits,
    ) -> Result<Self, SourceDiscoveryError> {
        if !completed.is_complete_ordinary() {
            return Err(SourceDiscoveryError::WrongSourceLayout {
                actual: completed.layout_name(),
            });
        }
        if completed.source_row_count() != incidence.source_count() {
            return Err(SourceDiscoveryError::CompletedSourceChronologyMismatch);
        }
        let mut source_rows = try_vec(CACHE_SOURCE_ROWS, incidence.source_count())?;
        for ordinal in 0..incidence.source_count() {
            let completed_row = completed
                .source_row_id(ordinal)
                .ok_or(SourceDiscoveryError::CompletedSourceChronologyMismatch)?;
            let incidence_row = incidence.sources()[ordinal].row_id();
            if completed_row != incidence_row {
                return Err(SourceDiscoveryError::CompletedSourceChronologyMismatch);
            }
            source_rows.push(completed_row.clone());
        }
        check_limit(CACHE_ROWS, 0, limits.max_row_cache_rows)?;
        Ok(Self {
            incidence_identity: incidence.identity_owner(),
            family_fingerprint: Arc::from(incidence.family_fingerprint()),
            context_fingerprint: Arc::from(incidence.context_fingerprint()),
            arity: incidence.arity(),
            source_rows: source_rows.into_boxed_slice(),
            sector: None,
            sample: None,
            entries: Vec::new(),
            telemetry: ProbeRowEvaluationCacheTelemetry::default(),
        })
    }

    pub(crate) const fn telemetry(&self) -> ProbeRowEvaluationCacheTelemetry {
        self.telemetry
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_evaluate(
        &mut self,
        incidence: &OrdinarySourceIncidenceIndex<'_>,
        context: &IndexedCoefficientContext,
        request: &TranslatedSourceRequest,
        source: &TranslatedSource,
        frame: &ModularPhysicalFrame<'_>,
        candidate_ordinal: usize,
        limits: SourceDiscoveryLimits,
    ) -> Result<Arc<[FiniteFieldElement<u64>]>, SourceDiscoveryError> {
        self.validate_scope(incidence, context, request, source, frame)?;
        let position = self.try_find(request, limits)?;
        if let Ok(position) = position {
            self.telemetry.cache_hits =
                checked_add(CACHE_LOOKUP_COMPARISONS, self.telemetry.cache_hits, 1)?;
            return Ok(self.entries[position].values.clone());
        }
        let insertion = position.expect_err("cache lookup result was checked as a miss");
        let moved_entries =
            self.entries
                .len()
                .checked_sub(insertion)
                .ok_or(SourceDiscoveryError::Invariant {
                    detail: "row-cache insertion lies outside its sorted entry storage",
                })?;
        let insertion_moves = checked_add(
            CACHE_INSERTION_MOVES,
            self.telemetry.insertion_moves,
            moved_entries,
        )?;
        check_limit(
            CACHE_INSERTION_MOVES,
            insertion_moves,
            limits.max_row_cache_insertion_moves,
        )?;
        let requested_rows = checked_add(CACHE_ROWS, self.entries.len(), 1)?;
        check_limit(CACHE_ROWS, requested_rows, limits.max_row_cache_rows)?;
        let request_cells = checked_mul(CACHE_REQUEST_COORDINATES, requested_rows, self.arity)?;
        check_limit(
            CACHE_REQUEST_COORDINATES,
            request_cells,
            limits.max_row_cache_request_coordinate_cells,
        )?;
        let requested_values = checked_add(
            CACHE_VALUES,
            self.telemetry.value_cells,
            source.terms().len(),
        )?;
        check_limit(
            CACHE_VALUES,
            requested_values,
            limits.max_row_cache_value_cells,
        )?;
        let physical_evaluations = checked_add(
            CACHE_PHYSICAL_EVALUATIONS,
            self.telemetry.physical_evaluations,
            1,
        )?;
        check_limit(
            CACHE_PHYSICAL_EVALUATIONS,
            physical_evaluations,
            limits.max_row_cache_physical_evaluations,
        )?;
        self.entries
            .try_reserve(1)
            .map_err(|_| SourceDiscoveryError::AllocationFailure {
                resource: CACHE_ROWS,
                requested: requested_rows,
            })?;
        let mut evaluated = try_vec(CACHE_VALUES, source.terms().len())?;
        // This counter describes work attempted, not only cacheable successes.
        // Record it immediately before crossing the fallible evaluation
        // boundary so a singular row remains visible to outer accounting
        // while still leaving the cache entries/value-cell census unchanged.
        self.telemetry.physical_evaluations = physical_evaluations;
        frame
            .try_evaluate_translated_source(context, source, &mut evaluated)
            .map_err(|error| SourceDiscoveryError::CandidateEvaluation {
                candidate_ordinal,
                source_ordinal: request.source_ordinal(),
                error,
            })?;
        if evaluated.len() != source.terms().len() {
            return Err(SourceDiscoveryError::Invariant {
                detail: "cached complete modular row changed exact term cardinality",
            });
        }
        let values: Arc<[FiniteFieldElement<u64>]> = Arc::from(evaluated);
        self.entries.insert(
            insertion,
            CachedRowEvaluation {
                request: request.clone(),
                values: values.clone(),
            },
        );
        self.telemetry.rows = requested_rows;
        self.telemetry.value_cells = requested_values;
        self.telemetry.insertion_moves = insertion_moves;
        Ok(values)
    }

    fn validate_scope(
        &mut self,
        incidence: &OrdinarySourceIncidenceIndex<'_>,
        context: &IndexedCoefficientContext,
        request: &TranslatedSourceRequest,
        source: &TranslatedSource,
        frame: &ModularPhysicalFrame<'_>,
    ) -> Result<(), SourceDiscoveryError> {
        if !incidence.owns_identity(&self.incidence_identity) {
            return Err(SourceDiscoveryError::ScopeMismatch {
                detail: "row cache belongs to a different incidence owner",
            });
        }
        if incidence.family_fingerprint() != self.family_fingerprint.as_ref()
            || frame.plan().family_fingerprint() != self.family_fingerprint.as_ref()
            || incidence.context_fingerprint() != self.context_fingerprint.as_ref()
            || frame.plan().context_fingerprint() != self.context_fingerprint.as_ref()
            || context.fingerprint() != self.context_fingerprint.as_ref()
        {
            return Err(SourceDiscoveryError::ScopeMismatch {
                detail: "row cache family or coefficient context changed",
            });
        }
        if request.offset().len() != self.arity
            || frame.plan().sector().arity() != self.arity
            || source.provenance().source_ordinal() != request.source_ordinal()
            || source.provenance().offset() != request.offset()
        {
            return Err(SourceDiscoveryError::ScopeMismatch {
                detail: "row cache request/source arity or provenance changed",
            });
        }
        let row = self.source_rows.get(request.source_ordinal()).ok_or(
            SourceDiscoveryError::ScopeMismatch {
                detail: "row cache request is outside completed source chronology",
            },
        )?;
        if source.row_id() != row || incidence.sources()[request.source_ordinal()].row_id() != row {
            return Err(SourceDiscoveryError::CompletedSourceChronologyMismatch);
        }
        match &self.sector {
            Some(sector) if sector != frame.plan().sector() => {
                return Err(SourceDiscoveryError::ScopeMismatch {
                    detail: "row cache sampled sector changed within one probe",
                });
            }
            None => self.sector = Some(frame.plan().sector().clone()),
            Some(_) => {}
        }
        match &self.sample {
            Some(sample) if sample != frame.sample_fingerprint().as_ref() => {
                return Err(SourceDiscoveryError::ScopeMismatch {
                    detail: "row cache modulus or complete evaluation point changed within one probe",
                });
            }
            None => self.sample = Some(frame.sample_fingerprint().as_ref().clone()),
            Some(_) => {}
        }
        Ok(())
    }

    fn try_find(
        &mut self,
        request: &TranslatedSourceRequest,
        limits: SourceDiscoveryLimits,
    ) -> Result<Result<usize, usize>, SourceDiscoveryError> {
        let mut left = 0usize;
        let mut right = self.entries.len();
        while left < right {
            let comparisons = checked_add(
                CACHE_LOOKUP_COMPARISONS,
                self.telemetry.lookup_comparisons,
                1,
            )?;
            check_limit(
                CACHE_LOOKUP_COMPARISONS,
                comparisons,
                limits.max_row_cache_lookup_comparisons,
            )?;
            self.telemetry.lookup_comparisons = comparisons;
            let middle = left + (right - left) / 2;
            match self.entries[middle].request.cmp(request) {
                std::cmp::Ordering::Less => left = middle + 1,
                std::cmp::Ordering::Equal => return Ok(Ok(middle)),
                std::cmp::Ordering::Greater => right = middle,
            }
        }
        Ok(Err(left))
    }
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SourceDiscoveryError> {
    left.checked_mul(right)
        .ok_or(SourceDiscoveryError::ResourceCountOverflow { resource })
}
