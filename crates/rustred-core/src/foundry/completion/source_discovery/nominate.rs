use crate::foundry::completion::frame::PhysicalFramePlan;
use crate::foundry::completion::frame::modular::ModularRightObstruction;
use crate::identity::{IntegralShift, TranslatedSourceRequest};

use super::model::IncidentNominationOrigin;
use super::{
    IncidentTranslationNominations, OrdinarySourceIncidenceIndex, SourceDiscoveryError,
    SourceDiscoveryLimits,
};

const SUPPORT_ENTRIES: &str = "source-discovery obstruction support entries";
const INCIDENCE_VISITS: &str = "source-discovery inverse-incidence visits";
const CANDIDATE_COORDINATES: &str = "source-discovery candidate coordinate cells";
const RAW_REQUESTS: &str = "source-discovery raw translated-source requests";
const UNIQUE_REQUESTS: &str = "source-discovery unique translated-source requests";
const EXISTING_REQUESTS: &str = "source-discovery existing translated-source requests";

impl OrdinarySourceIncidenceIndex<'_> {
    /// Nominate every declared ordinary-source translation touching one raw
    /// target.  This is the structural `q = e_target` bootstrap only; it is
    /// not represented as a checked modular obstruction.
    pub(crate) fn try_nominate_target_unit(
        &self,
        target: &IntegralShift,
        limits: SourceDiscoveryLimits,
    ) -> Result<IncidentTranslationNominations, SourceDiscoveryError> {
        if target.len() != self.arity() {
            return Err(SourceDiscoveryError::WrongArity {
                object: "target-unit shift",
                expected: self.arity(),
                actual: target.len(),
            });
        }
        nominate(
            self,
            IncidentNominationOrigin::TargetUnit,
            &[target],
            &[],
            limits,
        )
    }

    /// Nominate every declared translation incident to the nonzero support of
    /// one checked target-normalized modular right obstruction.
    ///
    /// Requests already materialized by the obstruction's immutable physical
    /// plan are removed after canonical deduplication.  Numeric coefficients
    /// are deliberately not inspected in this structural slice.
    pub(crate) fn try_nominate_obstruction(
        &self,
        obstruction: &ModularRightObstruction<'_>,
        limits: SourceDiscoveryLimits,
    ) -> Result<IncidentTranslationNominations, SourceDiscoveryError> {
        let plan = obstruction.plan();
        validate_plan_scope(self, plan)?;
        check_limit(
            SUPPORT_ENTRIES,
            obstruction.entries().len(),
            limits.max_obstruction_support,
        )?;
        let mut support = try_vec(SUPPORT_ENTRIES, obstruction.entries().len())?;
        for entry in obstruction.entries() {
            let physical = *obstruction
                .logical_physical_columns()
                .get(entry.logical_column())
                .ok_or(SourceDiscoveryError::Invariant {
                    detail: "modular obstruction entry is outside its logical column map",
                })?;
            support.push(
                plan.columns()
                    .get(physical)
                    .ok_or(SourceDiscoveryError::Invariant {
                        detail: "modular obstruction support is outside its physical frame",
                    })?,
            );
        }
        if support.is_empty() {
            return Err(SourceDiscoveryError::Invariant {
                detail: "checked modular obstruction has empty nonzero support",
            });
        }
        support.sort_unstable_by(|left, right| left.values().cmp(right.values()));
        if support
            .windows(2)
            .any(|pair| pair[0].values() >= pair[1].values())
        {
            return Err(SourceDiscoveryError::Invariant {
                detail: "checked modular obstruction repeats one raw support shift",
            });
        }

        let existing = existing_requests(self, plan, limits)?;
        nominate(
            self,
            IncidentNominationOrigin::CheckedObstruction(obstruction.identity_owner()),
            &support,
            &existing,
            limits,
        )
    }
}

fn nominate(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    origin: IncidentNominationOrigin,
    support: &[&IntegralShift],
    existing: &[TranslatedSourceRequest],
    limits: SourceDiscoveryLimits,
) -> Result<IncidentTranslationNominations, SourceDiscoveryError> {
    if support.is_empty() {
        return Err(SourceDiscoveryError::Invariant {
            detail: "inverse-incidence nomination received empty support",
        });
    }
    check_limit(
        SUPPORT_ENTRIES,
        support.len(),
        limits.max_obstruction_support,
    )?;
    for shift in support {
        if shift.len() != incidence.arity() {
            return Err(SourceDiscoveryError::WrongArity {
                object: "obstruction support shift",
                expected: incidence.arity(),
                actual: shift.len(),
            });
        }
    }

    let raw_count = checked_mul(
        INCIDENCE_VISITS,
        support.len(),
        incidence.term_occurrences(),
    )?;
    check_limit(INCIDENCE_VISITS, raw_count, limits.max_incidence_visits)?;
    check_limit(RAW_REQUESTS, raw_count, limits.max_raw_requests)?;
    let coordinate_cells = checked_mul(CANDIDATE_COORDINATES, raw_count, incidence.arity())?;
    check_limit(
        CANDIDATE_COORDINATES,
        coordinate_cells,
        limits.max_candidate_coordinate_cells,
    )?;

    let mut requests = try_vec(RAW_REQUESTS, raw_count)?;
    for (support_ordinal, supported) in support.iter().enumerate() {
        for (source_ordinal, source) in incidence.sources().iter().enumerate() {
            for (term_ordinal, source_shift) in source.terms().keys().enumerate() {
                for (position, (&left, &right)) in supported
                    .values()
                    .iter()
                    .zip(source_shift.values())
                    .enumerate()
                {
                    if left.checked_sub(right).is_none() {
                        return Err(SourceDiscoveryError::ShiftOverflow {
                            support_ordinal,
                            source_ordinal,
                            term_ordinal,
                            position,
                            support: left,
                            source_shift: right,
                        });
                    }
                }
                let offset = IntegralShift::try_new_with_component_limit(
                    supported
                        .values()
                        .iter()
                        .zip(source_shift.values())
                        .map(|(&left, &right)| {
                            left.checked_sub(right)
                                .expect("inverse-incidence subtraction was checked component-wise")
                        }),
                    incidence.arity(),
                )
                .map_err(SourceDiscoveryError::ShiftConstruction)?;
                requests.push(TranslatedSourceRequest::new(source_ordinal, offset));
            }
        }
    }
    if requests.len() != raw_count {
        return Err(SourceDiscoveryError::Invariant {
            detail: "inverse-incidence enumeration changed its preflighted visit count",
        });
    }
    requests.sort_unstable();
    requests.dedup();
    let unique_before_existing_exclusion = requests.len();
    check_limit(
        UNIQUE_REQUESTS,
        unique_before_existing_exclusion,
        limits.max_unique_requests,
    )?;

    if existing.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(SourceDiscoveryError::Invariant {
            detail: "existing translated-source requests are not canonical and unique",
        });
    }
    requests.retain(|request| existing.binary_search(request).is_err());
    let excluded_existing_requests = unique_before_existing_exclusion - requests.len();
    Ok(IncidentTranslationNominations::from_parts(
        incidence.identity_owner(),
        origin,
        requests,
        raw_count,
        unique_before_existing_exclusion,
        excluded_existing_requests,
    ))
}

#[cfg(test)]
pub(super) fn nominate_support_for_test(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    support: &[&IntegralShift],
    existing: &[TranslatedSourceRequest],
    limits: SourceDiscoveryLimits,
) -> Result<IncidentTranslationNominations, SourceDiscoveryError> {
    nominate(
        incidence,
        IncidentNominationOrigin::TargetUnit,
        support,
        existing,
        limits,
    )
}

#[cfg(test)]
pub(super) fn empty_obstruction_nominations_for_test(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    obstruction: &ModularRightObstruction<'_>,
) -> Result<IncidentTranslationNominations, SourceDiscoveryError> {
    validate_plan_scope(incidence, obstruction.plan())?;
    Ok(IncidentTranslationNominations::from_parts(
        incidence.identity_owner(),
        IncidentNominationOrigin::CheckedObstruction(obstruction.identity_owner()),
        Vec::new(),
        0,
        0,
        0,
    ))
}

pub(super) fn existing_requests(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    plan: &PhysicalFramePlan,
    limits: SourceDiscoveryLimits,
) -> Result<Vec<TranslatedSourceRequest>, SourceDiscoveryError> {
    check_limit(
        EXISTING_REQUESTS,
        plan.source_instances().len(),
        limits.max_existing_requests,
    )?;
    let mut existing = try_vec(EXISTING_REQUESTS, plan.source_instances().len())?;
    for source in plan.source_instances() {
        let provenance = source.provenance();
        let expected = incidence.sources().get(provenance.source_ordinal()).ok_or(
            SourceDiscoveryError::ScopeMismatch {
                detail: "physical frame source ordinal is outside the declared ordinary module",
            },
        )?;
        if provenance.source_row() != expected.row_id() {
            return Err(SourceDiscoveryError::ScopeMismatch {
                detail: "physical frame source identity differs from the declared ordinary module",
            });
        }
        if provenance.offset().len() != incidence.arity() {
            return Err(SourceDiscoveryError::WrongArity {
                object: "existing source offset",
                expected: incidence.arity(),
                actual: provenance.offset().len(),
            });
        }
        existing.push(TranslatedSourceRequest::new(
            provenance.source_ordinal(),
            provenance.offset().clone(),
        ));
    }
    existing.sort_unstable();
    let materialized_count = existing.len();
    existing.dedup();
    if existing.len() != materialized_count {
        return Err(SourceDiscoveryError::Invariant {
            detail: "physical frame repeats one translated-source identity",
        });
    }
    Ok(existing)
}

pub(super) fn validate_plan_scope(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    plan: &PhysicalFramePlan,
) -> Result<(), SourceDiscoveryError> {
    if plan.family_fingerprint() != incidence.family_fingerprint() {
        return Err(SourceDiscoveryError::ScopeMismatch {
            detail: "physical frame belongs to a different integral family",
        });
    }
    if plan.context_fingerprint() != incidence.context_fingerprint() {
        return Err(SourceDiscoveryError::ScopeMismatch {
            detail: "physical frame belongs to a different coefficient context",
        });
    }
    if plan.sector().arity() != incidence.arity() {
        return Err(SourceDiscoveryError::WrongArity {
            object: "physical frame sector",
            expected: incidence.arity(),
            actual: plan.sector().arity(),
        });
    }
    Ok(())
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SourceDiscoveryError> {
    if requested > limit {
        Err(SourceDiscoveryError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SourceDiscoveryError> {
    left.checked_add(right)
        .ok_or(SourceDiscoveryError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SourceDiscoveryError> {
    left.checked_mul(right)
        .ok_or(SourceDiscoveryError::ResourceCountOverflow { resource })
}

pub(super) fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, SourceDiscoveryError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| SourceDiscoveryError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}
