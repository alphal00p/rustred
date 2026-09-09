use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::completion::stratum::ProspectiveColumnClassifier;
use crate::identity::{CompletedIbpSourceRows, TranslatedSourceRequest};

use super::super::{SpiredExecutionCase, ValidatedDirectShiftedSources};
use super::registry::StructuralRoleRegistry;
use super::{
    SpiredPreparedRequestChunk, SpiredPreparedRowPlan, SpiredPreparedRowSpan,
    SpiredPreparedTermRole, SpiredStructuralPreparationCensus, SpiredStructuralPreparationError,
    SpiredStructuralPreparationLimits, SpiredStructuralScopeIdentity,
};

const REQUEST_INSPECTIONS: &str = "prepared translated-source request inspections";
const SOURCE_TERM_INSPECTIONS: &str = "prepared exact source-term inspections";
const PREPARED_ROWS: &str = "prepared structural rows";
const PREPARED_TERM_ROLES: &str = "prepared structural term roles";
const REQUEST_OFFSET_CELLS: &str = "prepared request offset cells";
const STRUCTURAL_SHIFT_SCRATCH: &str = "structural shift preparation scratch";

/// Serial scheduler-chronology planner for one immutable exact case.
///
/// The planner itself is intentionally not synchronized. A coordinator calls
/// it in deterministic scheduler order, then shares the emitted immutable
/// chunks with every probe worker. Worker arrival can therefore never affect
/// compact forbidden IDs or row chronology.
#[derive(Debug)]
pub(crate) struct SpiredStructuralPreparation<'context, 'sources> {
    case: SpiredExecutionCase,
    validated_sources: ValidatedDirectShiftedSources<'context, 'sources>,
    registry: StructuralRoleRegistry,
    scope: Arc<SpiredStructuralScopeIdentity>,
    structural_shift_scratch: Vec<i64>,
    census: SpiredStructuralPreparationCensus,
    limits: SpiredStructuralPreparationLimits,
    poisoned: bool,
}

impl<'context, 'sources> SpiredStructuralPreparation<'context, 'sources> {
    pub(crate) fn try_new(
        context: &'context IndexedCoefficientContext,
        sources: &'sources CompletedIbpSourceRows,
        case: &SpiredExecutionCase,
        limits: SpiredStructuralPreparationLimits,
    ) -> Result<Self, SpiredStructuralPreparationError> {
        if sources.family_fingerprint() != case.stratum().family_fingerprint() {
            return Err(SpiredStructuralPreparationError::WrongCaseFamily);
        }
        if context.fingerprint() != case.stratum().context_fingerprint() {
            return Err(SpiredStructuralPreparationError::WrongCaseContext);
        }
        let validated_sources =
            ValidatedDirectShiftedSources::try_new(context, sources, limits.evaluation)?;
        let classifier = ProspectiveColumnClassifier::try_new_with_verified_snapshot(
            case.stratum().clone(),
            case.target_shift().clone(),
            case.owner_snapshot().verified_clone(),
            case.ordering(),
            limits.classification,
        )?;
        let registry = StructuralRoleRegistry::new(
            classifier,
            limits.max_cached_shifts,
            limits.max_cached_shift_coordinate_cells,
        );
        let census =
            SpiredStructuralPreparationCensus::with_exact_sources(validated_sources.census());
        Ok(Self {
            case: case.clone(),
            validated_sources,
            registry,
            scope: Arc::new(SpiredStructuralScopeIdentity::new()),
            structural_shift_scratch: Vec::new(),
            census,
            limits,
            poisoned: false,
        })
    }

    pub(crate) const fn case(&self) -> &SpiredExecutionCase {
        &self.case
    }

    pub(crate) const fn validated_sources(
        &self,
    ) -> &ValidatedDirectShiftedSources<'context, 'sources> {
        &self.validated_sources
    }

    pub(crate) const fn limits(&self) -> SpiredStructuralPreparationLimits {
        self.limits
    }

    pub(crate) const fn census(&self) -> SpiredStructuralPreparationCensus {
        self.census
    }

    pub(crate) const fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    pub(in crate::foundry::completion::spired) fn scope(
        &self,
    ) -> Arc<SpiredStructuralScopeIdentity> {
        Arc::clone(&self.scope)
    }

    /// Prepare one scheduler-contiguous request chunk and assign all new
    /// forbidden IDs in exact request/term chronology.
    pub(crate) fn try_prepare_requests(
        &mut self,
        requests: &[TranslatedSourceRequest],
    ) -> Result<SpiredPreparedRequestChunk, SpiredStructuralPreparationError> {
        if self.poisoned {
            return Err(SpiredStructuralPreparationError::Poisoned);
        }
        let result = self.try_prepare_requests_inner(requests);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    /// Prepare exactly one scheduler row without wrapping it in a separately
    /// allocated shared chunk. Target workspaces retain many such rows in one
    /// flat tape and share that tape by immutable borrow across probes.
    pub(crate) fn try_prepare_one_request(
        &mut self,
        request: &TranslatedSourceRequest,
    ) -> Result<SpiredPreparedRowPlan, SpiredStructuralPreparationError> {
        if self.poisoned {
            return Err(SpiredStructuralPreparationError::Poisoned);
        }
        let result = self.try_prepare_rows_inner(std::slice::from_ref(request));
        let result = result.and_then(|(_, mut rows)| {
            if rows.len() != 1 {
                return Err(SpiredStructuralPreparationError::Invariant {
                    detail: "single-request preparation emitted a non-singleton row batch",
                });
            }
            rows.pop()
                .ok_or(SpiredStructuralPreparationError::Invariant {
                    detail: "single-request preparation emitted no row",
                })
        });
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    /// Append one row's roles directly to a caller-owned flat arena.
    ///
    /// The complete row/role census and all configured caps are checked before
    /// the arena reserves or structural classification begins. Target-run
    /// workspaces reserve their aggregate arena first, making the reserve here
    /// a no-op and avoiding both a transient row `Vec` and a boxed role slice.
    pub(crate) fn try_prepare_one_request_into(
        &mut self,
        request: &TranslatedSourceRequest,
        term_roles: &mut Vec<SpiredPreparedTermRole>,
    ) -> Result<SpiredPreparedRowSpan, SpiredStructuralPreparationError> {
        if self.poisoned {
            return Err(SpiredStructuralPreparationError::Poisoned);
        }
        let role_checkpoint = term_roles.len();
        let result = self.try_prepare_one_request_into_inner(request, term_roles);
        if result.is_err() {
            term_roles.truncate(role_checkpoint);
            self.poisoned = true;
        }
        result
    }

    fn try_prepare_one_request_into_inner(
        &mut self,
        request: &TranslatedSourceRequest,
        term_roles: &mut Vec<SpiredPreparedTermRole>,
    ) -> Result<SpiredPreparedRowSpan, SpiredStructuralPreparationError> {
        let preflight = self.try_preflight_requests(std::slice::from_ref(request))?;
        let source_term_count = preflight.added_source_terms;
        let requested_term_roles =
            checked_add(PREPARED_TERM_ROLES, term_roles.len(), source_term_count)?;
        term_roles
            .try_reserve_exact(source_term_count)
            .map_err(|_| SpiredStructuralPreparationError::AllocationFailure {
                resource: PREPARED_TERM_ROLES,
                requested: requested_term_roles,
            })?;
        self.try_prepare_structural_shift_scratch()?;

        let first_term_role = term_roles.len();
        let mut target_terms = self.census.target_terms();
        let mut allowed_terms = self.census.allowed_terms();
        let mut forbidden_terms = self.census.forbidden_terms();
        let forbidden_frontier = self.try_append_request_roles(
            request,
            preflight.first_row_ordinal,
            term_roles,
            &mut target_terms,
            &mut allowed_terms,
            &mut forbidden_terms,
        )?;
        let retained = term_roles.len().checked_sub(first_term_role).ok_or(
            SpiredStructuralPreparationError::Invariant {
                detail: "flat role arena moved backwards during one-row preparation",
            },
        )?;
        if retained != source_term_count {
            return Err(SpiredStructuralPreparationError::Invariant {
                detail: "flat role arena retained a non-source term count",
            });
        }
        let row = SpiredPreparedRowSpan::new(
            preflight.first_row_ordinal,
            request.clone(),
            first_term_role,
            retained,
            forbidden_frontier,
        );
        self.install_preflight_census(preflight, target_terms, allowed_terms, forbidden_terms);
        Ok(row)
    }

    fn try_prepare_requests_inner(
        &mut self,
        requests: &[TranslatedSourceRequest],
    ) -> Result<SpiredPreparedRequestChunk, SpiredStructuralPreparationError> {
        let (first_row_ordinal, rows) = self.try_prepare_rows_inner(requests)?;
        Ok(SpiredPreparedRequestChunk::new(
            Arc::clone(&self.scope),
            first_row_ordinal,
            rows.into_boxed_slice(),
        ))
    }

    fn try_prepare_rows_inner(
        &mut self,
        requests: &[TranslatedSourceRequest],
    ) -> Result<(usize, Vec<SpiredPreparedRowPlan>), SpiredStructuralPreparationError> {
        let preflight = self.try_preflight_requests(requests)?;
        self.try_prepare_structural_shift_scratch()?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(requests.len()).map_err(|_| {
            SpiredStructuralPreparationError::AllocationFailure {
                resource: PREPARED_ROWS,
                requested: requests.len(),
            }
        })?;

        let mut target_terms = self.census.target_terms();
        let mut allowed_terms = self.census.allowed_terms();
        let mut forbidden_terms = self.census.forbidden_terms();
        for (chunk_ordinal, request) in requests.iter().enumerate() {
            let row_ordinal = preflight
                .first_row_ordinal
                .checked_add(chunk_ordinal)
                .ok_or(SpiredStructuralPreparationError::ResourceCountOverflow {
                    resource: PREPARED_ROWS,
                })?;
            let source = self
                .validated_sources
                .sources()
                .source_relation(request.source_ordinal())
                .ok_or(SpiredStructuralPreparationError::Invariant {
                    detail: "preflighted source request disappeared from sealed rows",
                })?;
            let mut roles = Vec::new();
            roles.try_reserve_exact(source.terms().len()).map_err(|_| {
                SpiredStructuralPreparationError::AllocationFailure {
                    resource: PREPARED_TERM_ROLES,
                    requested: source.terms().len(),
                }
            })?;
            let forbidden_frontier = self.try_append_request_roles(
                request,
                row_ordinal,
                &mut roles,
                &mut target_terms,
                &mut allowed_terms,
                &mut forbidden_terms,
            )?;
            rows.push(SpiredPreparedRowPlan::new(
                row_ordinal,
                request.clone(),
                roles.into_boxed_slice(),
                forbidden_frontier,
            ));
        }

        self.install_preflight_census(preflight, target_terms, allowed_terms, forbidden_terms);
        Ok((preflight.first_row_ordinal, rows))
    }

    fn try_preflight_requests(
        &self,
        requests: &[TranslatedSourceRequest],
    ) -> Result<PreparedRowsPreflight, SpiredStructuralPreparationError> {
        let first_row_ordinal = self.census.prepared_rows();
        let next_rows = checked_add(PREPARED_ROWS, first_row_ordinal, requests.len())?;
        check_limit(PREPARED_ROWS, next_rows, self.limits.max_prepared_rows)?;
        let next_request_inspections = checked_add(
            REQUEST_INSPECTIONS,
            self.census.request_inspections(),
            requests.len(),
        )?;
        check_limit(
            REQUEST_INSPECTIONS,
            next_request_inspections,
            self.limits.max_request_inspections,
        )?;

        let mut added_source_terms = 0usize;
        let mut added_offset_cells = 0usize;
        for request in requests {
            let source = self
                .validated_sources
                .sources()
                .source_relation(request.source_ordinal())
                .ok_or(
                    crate::foundry::completion::spired::DirectShiftedSourceError::SourceOrdinalOutOfRange {
                        source_ordinal: request.source_ordinal(),
                        source_count: self.validated_sources.sources().source_row_count(),
                    },
                )?;
            if request.offset().len() != self.case.arity() {
                return Err(
                    crate::foundry::completion::spired::DirectShiftedSourceError::WrongOffsetArity {
                        expected: self.case.arity(),
                        actual: request.offset().len(),
                    }
                    .into(),
                );
            }
            added_source_terms = checked_add(
                SOURCE_TERM_INSPECTIONS,
                added_source_terms,
                source.terms().len(),
            )?;
            added_offset_cells = checked_add(
                REQUEST_OFFSET_CELLS,
                added_offset_cells,
                request.offset().len(),
            )?;
        }
        let next_source_terms = checked_add(
            SOURCE_TERM_INSPECTIONS,
            self.census.source_term_inspections(),
            added_source_terms,
        )?;
        check_limit(
            SOURCE_TERM_INSPECTIONS,
            next_source_terms,
            self.limits.max_source_term_inspections,
        )?;
        let next_term_roles = checked_add(
            PREPARED_TERM_ROLES,
            self.census.emitted_term_roles(),
            added_source_terms,
        )?;
        check_limit(
            PREPARED_TERM_ROLES,
            next_term_roles,
            self.limits.max_prepared_term_roles,
        )?;
        let next_offset_cells = checked_add(
            REQUEST_OFFSET_CELLS,
            self.census.emitted_request_offset_cells(),
            added_offset_cells,
        )?;
        check_limit(
            REQUEST_OFFSET_CELLS,
            next_offset_cells,
            self.limits.max_prepared_request_offset_cells,
        )?;
        Ok(PreparedRowsPreflight {
            first_row_ordinal,
            next_rows,
            next_request_inspections,
            next_source_terms,
            next_term_roles,
            next_offset_cells,
            added_source_terms,
        })
    }

    fn try_prepare_structural_shift_scratch(
        &mut self,
    ) -> Result<(), SpiredStructuralPreparationError> {
        self.structural_shift_scratch.clear();
        self.structural_shift_scratch
            .try_reserve_exact(self.case.arity())
            .map_err(|_| SpiredStructuralPreparationError::AllocationFailure {
                resource: STRUCTURAL_SHIFT_SCRATCH,
                requested: self.case.arity(),
            })
    }

    fn try_append_request_roles(
        &mut self,
        request: &TranslatedSourceRequest,
        row_ordinal: usize,
        roles: &mut Vec<SpiredPreparedTermRole>,
        target_terms: &mut usize,
        allowed_terms: &mut usize,
        forbidden_terms: &mut usize,
    ) -> Result<usize, SpiredStructuralPreparationError> {
        let source = self
            .validated_sources
            .sources()
            .source_relation(request.source_ordinal())
            .ok_or(SpiredStructuralPreparationError::Invariant {
                detail: "preflighted source request disappeared from sealed rows",
            })?;
        let mut row_target_terms = 0usize;
        for (term_ordinal, source_shift) in source.terms().keys().enumerate() {
            self.structural_shift_scratch.clear();
            for (position, (&offset, &term)) in request
                .offset()
                .values()
                .iter()
                .zip(source_shift.values())
                .enumerate()
            {
                self.structural_shift_scratch.push(offset.checked_add(term).ok_or(
                    crate::foundry::completion::spired::DirectShiftedSourceError::StructuralShiftOverflow {
                        term_ordinal,
                        position,
                        offset,
                        source_shift: term,
                    },
                )?);
            }
            let role = self.registry.try_classify(&self.structural_shift_scratch)?;
            match role {
                SpiredPreparedTermRole::Target => {
                    row_target_terms = checked_add("target terms in one row", row_target_terms, 1)?;
                    *target_terms = checked_add("prepared target terms", *target_terms, 1)?;
                }
                SpiredPreparedTermRole::Allowed => {
                    *allowed_terms = checked_add("prepared allowed terms", *allowed_terms, 1)?;
                }
                SpiredPreparedTermRole::Forbidden(_) => {
                    *forbidden_terms =
                        checked_add("prepared forbidden terms", *forbidden_terms, 1)?;
                }
            }
            roles.push(role);
        }
        if row_target_terms > 1 {
            return Err(SpiredStructuralPreparationError::DuplicateTargetTerm { row_ordinal });
        }
        Ok(self.registry.forbidden_columns())
    }

    fn install_preflight_census(
        &mut self,
        preflight: PreparedRowsPreflight,
        target_terms: usize,
        allowed_terms: usize,
        forbidden_terms: usize,
    ) {
        self.census.update_emitted(
            preflight.next_rows,
            preflight.next_request_inspections,
            preflight.next_source_terms,
            target_terms,
            allowed_terms,
            forbidden_terms,
            preflight.next_offset_cells,
            preflight.next_term_roles,
        );
        self.census.update_registry(
            self.registry.len(),
            self.registry.coordinate_cells(),
            self.registry.forbidden_columns(),
            self.registry.target_sector_cells(),
            self.registry.owner_probes(),
            self.registry.retained_owner_witnesses(),
        );
    }
}

#[derive(Clone, Copy, Debug)]
struct PreparedRowsPreflight {
    first_row_ordinal: usize,
    next_rows: usize,
    next_request_inspections: usize,
    next_source_terms: usize,
    next_term_roles: usize,
    next_offset_cells: usize,
    added_source_terms: usize,
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SpiredStructuralPreparationError> {
    left.checked_add(right)
        .ok_or(SpiredStructuralPreparationError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SpiredStructuralPreparationError> {
    if requested > limit {
        Err(SpiredStructuralPreparationError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
