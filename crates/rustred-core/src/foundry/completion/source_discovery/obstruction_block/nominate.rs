use symbolica::domains::Ring;
use symbolica::domains::finite_field::Zp64;

use crate::foundry::completion::frame::modular::ModularRightObstruction;
use crate::identity::{IntegralShift, TranslatedSourceRequest};

use super::super::nominate::{
    check_limit, checked_add, existing_requests, try_vec, validate_plan_scope,
};
use super::super::{
    IncidentTranslationNominations, OrdinarySourceIncidenceIndex, SourceDiscoveryError,
    SourceDiscoveryLimits,
};
use super::{
    ObstructionBlockNominationPlan, ObstructionBlockNominationUpperBound,
    ObstructionBlockNominations, UnionObstructionSupportEntry, UnionSupportNominations,
};

const UNION_BLOCK_ENTRIES: &str = "source-discovery obstruction-block raw entries";
const UNION_SUPPORT: &str = "source-discovery obstruction-block union support";
const UNION_SUPPORT_COORDINATES: &str =
    "source-discovery obstruction-block union-support coordinate cells";
const UNION_SUPPORT_COEFFICIENTS: &str =
    "source-discovery obstruction-block union-support coefficient cells";
const UNION_INCIDENCE_VISITS: &str = "source-discovery obstruction-block union incidence visits";
const UNION_RAW_REQUESTS: &str = "source-discovery obstruction-block union raw requests";
const UNION_UNIQUE_REQUESTS: &str = "source-discovery obstruction-block union unique requests";
const UNION_REQUEST_COORDINATES: &str =
    "source-discovery obstruction-block union request coordinate cells";
const UNION_SUBSET_COMPARISONS: &str =
    "source-discovery obstruction-block primary-subset comparisons";
const UNION_CANONICALIZATION_WORK: &str =
    "source-discovery obstruction-block canonicalization logical-work reservation";

struct RawBlockEntry {
    shift: IntegralShift,
    direction: usize,
    coefficient: symbolica::domains::finite_field::FiniteFieldElement<u64>,
}

impl OrdinarySourceIncidenceIndex<'_> {
    /// Build a scope-bound conservative envelope before any union support or
    /// request payload is allocated, enumerated, or sorted.
    pub(crate) fn try_plan_obstruction_block_nomination(
        &self,
        obstruction: &ModularRightObstruction<'_>,
        primary: &IncidentTranslationNominations,
        limits: SourceDiscoveryLimits,
    ) -> Result<ObstructionBlockNominationPlan, SourceDiscoveryError> {
        validate_plan_scope(self, obstruction.plan())?;
        validate_primary_scope(self, obstruction, primary)?;
        let upper_bound = nomination_upper_bound(self, obstruction, primary, limits)?;
        Ok(ObstructionBlockNominationPlan::from_parts(
            self.identity_owner(),
            obstruction.identity_owner(),
            primary.identity_owner(),
            upper_bound,
        ))
    }

    /// Build exact primary q0 nominations and a distinct proposal-only union
    /// census for every checked member of the bounded obstruction block.
    ///
    /// The primary value must come from the existing authoritative path and
    /// is borrowed without rerunning incidence or minting a second identity.
    /// The union has no provenance identities and cannot mint a residual seal.
    pub(crate) fn try_nominate_obstruction_block<'primary>(
        &self,
        obstruction: &ModularRightObstruction<'_>,
        primary: &'primary IncidentTranslationNominations,
        limits: SourceDiscoveryLimits,
    ) -> Result<ObstructionBlockNominations<'primary>, SourceDiscoveryError> {
        let plan = self.try_plan_obstruction_block_nomination(obstruction, primary, limits)?;
        self.try_nominate_obstruction_block_from_plan(&plan, obstruction, primary, limits)
    }

    pub(crate) fn try_nominate_obstruction_block_from_plan<'primary>(
        &self,
        plan: &ObstructionBlockNominationPlan,
        obstruction: &ModularRightObstruction<'_>,
        primary: &'primary IncidentTranslationNominations,
        limits: SourceDiscoveryLimits,
    ) -> Result<ObstructionBlockNominations<'primary>, SourceDiscoveryError> {
        validate_plan_scope(self, obstruction.plan())?;
        validate_primary(self, obstruction, primary)?;
        if !plan.belongs_to_incidence(&self.identity_owner())
            || !plan.obstruction_identity().belongs_to(obstruction)
            || !primary.owns_identity(plan.primary_identity())
        {
            return Err(SourceDiscoveryError::ScopeMismatch {
                detail: "obstruction-block nomination plan belongs to different checked inputs",
            });
        }
        let repeated_upper = nomination_upper_bound(self, obstruction, primary, limits)?;
        if repeated_upper != plan.upper_bound() {
            return Err(SourceDiscoveryError::Invariant {
                detail: "obstruction-block nomination changed its admitted work envelope",
            });
        }
        let support = union_support(self, obstruction, limits)?;
        let existing = existing_requests(self, obstruction.plan(), limits)?;
        let (requests, raw, unique_before, excluded) =
            nominate_union_requests(self, &support, &existing, limits)?;

        let mut primary_requests = try_vec(UNION_UNIQUE_REQUESTS, primary.requests().len())?;
        primary_requests.extend_from_slice(primary.requests());
        verify_primary_subset(&primary_requests, &requests, limits)?;
        let union = UnionSupportNominations::from_parts(
            requests,
            primary_requests,
            support,
            raw,
            unique_before,
            excluded,
            plan.upper_bound(),
        );
        Ok(ObstructionBlockNominations::from_parts(primary, union))
    }
}

fn validate_primary_scope(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    obstruction: &ModularRightObstruction<'_>,
    primary: &IncidentTranslationNominations,
) -> Result<(), SourceDiscoveryError> {
    if !incidence.owns_identity(primary.incidence_identity()) {
        return Err(SourceDiscoveryError::NominationIncidenceMismatch);
    }
    match primary.origin() {
        super::super::model::IncidentNominationOrigin::CheckedObstruction(identity)
            if identity.belongs_to(obstruction) => {}
        _ => return Err(SourceDiscoveryError::NominationObstructionMismatch),
    }
    Ok(())
}

fn validate_primary(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    obstruction: &ModularRightObstruction<'_>,
    primary: &IncidentTranslationNominations,
) -> Result<(), SourceDiscoveryError> {
    validate_primary_scope(incidence, obstruction, primary)?;
    let expected = primary
        .unique_before_existing_exclusion()
        .checked_sub(primary.excluded_existing_requests())
        .ok_or(SourceDiscoveryError::Invariant {
            detail: "primary obstruction nomination telemetry underflowed",
        })?;
    if expected != primary.requests().len()
        || primary.requests().windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(SourceDiscoveryError::Invariant {
            detail: "primary obstruction nominations are not canonical and census-complete",
        });
    }
    Ok(())
}

fn nomination_upper_bound(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    obstruction: &ModularRightObstruction<'_>,
    primary: &IncidentTranslationNominations,
    limits: SourceDiscoveryLimits,
) -> Result<ObstructionBlockNominationUpperBound, SourceDiscoveryError> {
    let block = obstruction.proposal_block();
    let width = block.directions().len();
    if width == 0 || width > 4 {
        return Err(SourceDiscoveryError::Invariant {
            detail: "proposal obstruction block width is outside 1..=4",
        });
    }
    let raw_block_entries = block
        .directions()
        .iter()
        .try_fold(0usize, |total, direction| {
            checked_add(UNION_BLOCK_ENTRIES, total, direction.entries().len())
        })?;
    check_limit(
        UNION_BLOCK_ENTRIES,
        raw_block_entries,
        limits.max_union_block_entries,
    )?;
    check_limit(
        UNION_SUPPORT,
        raw_block_entries,
        limits.max_union_support_entries,
    )?;
    let support_coordinates = checked_mul(
        UNION_SUPPORT_COORDINATES,
        raw_block_entries,
        incidence.arity(),
    )?;
    check_limit(
        UNION_SUPPORT_COORDINATES,
        support_coordinates,
        limits.max_union_support_coordinate_cells,
    )?;
    let dense_coefficients = checked_mul(UNION_SUPPORT_COEFFICIENTS, raw_block_entries, width)?;
    check_limit(
        UNION_SUPPORT_COEFFICIENTS,
        dense_coefficients,
        limits.max_union_support_coefficient_cells,
    )?;
    let raw_request_visits = checked_mul(
        UNION_INCIDENCE_VISITS,
        raw_block_entries,
        incidence.term_occurrences(),
    )?;
    check_limit(
        UNION_INCIDENCE_VISITS,
        raw_request_visits,
        limits.max_union_incidence_visits,
    )?;
    check_limit(
        UNION_RAW_REQUESTS,
        raw_request_visits,
        limits.max_union_raw_requests,
    )?;
    let request_coordinates = checked_mul(
        UNION_REQUEST_COORDINATES,
        raw_request_visits,
        incidence.arity(),
    )?;
    check_limit(
        UNION_REQUEST_COORDINATES,
        request_coordinates,
        limits.max_union_request_coordinate_cells,
    )?;
    let coordinate_cells = checked_add(
        UNION_REQUEST_COORDINATES,
        support_coordinates,
        request_coordinates,
    )?;
    let subset_comparisons = checked_add(
        UNION_SUBSET_COMPARISONS,
        primary.requests().len(),
        raw_request_visits,
    )?;
    check_limit(
        UNION_SUBSET_COMPARISONS,
        subset_comparisons,
        limits.max_union_subset_comparisons,
    )?;

    let existing = obstruction.plan().source_instances().len();
    check_limit(
        "source-discovery existing translated-source requests",
        existing,
        limits.max_existing_requests,
    )?;
    let key_coordinates = checked_add(UNION_CANONICALIZATION_WORK, incidence.arity(), 1)?;
    // `sort_unstable` does not expose its comparator census. Reserve a stable
    // logical `n * ceil(log2(max(n, 2)))` unit per sorted input plus the exact
    // linear/binary-search envelopes below. This is explicitly scheduling
    // input work, not a claim about the implementation's comparator constant.
    let logical_sort_reservation = [raw_block_entries, raw_request_visits, existing]
        .into_iter()
        .try_fold(0usize, |total, count| {
            checked_add(
                UNION_CANONICALIZATION_WORK,
                total,
                logical_sort_work(count)?,
            )
        })?;
    let dedup_and_subset = [
        raw_block_entries.saturating_sub(1),
        raw_request_visits.saturating_sub(1),
        existing.saturating_sub(1),
        primary.requests().len(),
        raw_request_visits,
    ]
    .into_iter()
    .try_fold(0usize, |total, count| {
        checked_add(UNION_CANONICALIZATION_WORK, total, count)
    })?;
    let exclusion_searches = checked_mul(
        UNION_CANONICALIZATION_WORK,
        raw_request_visits,
        bit_length(existing),
    )?;
    let logical_operation_reservation = [
        logical_sort_reservation,
        dedup_and_subset,
        exclusion_searches,
    ]
    .into_iter()
    .try_fold(0usize, |total, count| {
        checked_add(UNION_CANONICALIZATION_WORK, total, count)
    })?;
    let canonicalization_work_reservation = checked_mul(
        UNION_CANONICALIZATION_WORK,
        logical_operation_reservation,
        key_coordinates,
    )?;
    check_limit(
        UNION_CANONICALIZATION_WORK,
        canonicalization_work_reservation,
        limits.max_union_canonicalization_logical_work_reservation,
    )?;

    Ok(ObstructionBlockNominationUpperBound::from_parts(
        raw_block_entries,
        raw_request_visits,
        coordinate_cells,
        dense_coefficients,
        canonicalization_work_reservation,
        subset_comparisons,
    ))
}

fn logical_sort_work(count: usize) -> Result<usize, SourceDiscoveryError> {
    let normalized = count.max(2);
    let levels = usize::BITS as usize - normalized.saturating_sub(1).leading_zeros() as usize;
    checked_mul(UNION_CANONICALIZATION_WORK, count, levels)
}

const fn bit_length(count: usize) -> usize {
    if count == 0 {
        0
    } else {
        usize::BITS as usize - count.leading_zeros() as usize
    }
}

fn union_support(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    obstruction: &ModularRightObstruction<'_>,
    limits: SourceDiscoveryLimits,
) -> Result<Vec<UnionObstructionSupportEntry>, SourceDiscoveryError> {
    let block = obstruction.proposal_block();
    let width = block.directions().len();
    if width == 0 || width > 4 {
        return Err(SourceDiscoveryError::Invariant {
            detail: "proposal obstruction block width is outside 1..=4",
        });
    }
    let raw_count = block
        .directions()
        .iter()
        .try_fold(0usize, |total, direction| {
            checked_add(UNION_BLOCK_ENTRIES, total, direction.entries().len())
        })?;
    check_limit(
        UNION_BLOCK_ENTRIES,
        raw_count,
        limits.max_union_block_entries,
    )?;
    check_limit(UNION_SUPPORT, raw_count, limits.max_union_support_entries)?;
    let coordinate_upper = checked_mul(UNION_SUPPORT_COORDINATES, raw_count, incidence.arity())?;
    check_limit(
        UNION_SUPPORT_COORDINATES,
        coordinate_upper,
        limits.max_union_support_coordinate_cells,
    )?;
    let coefficient_upper = checked_mul(UNION_SUPPORT_COEFFICIENTS, raw_count, width)?;
    check_limit(
        UNION_SUPPORT_COEFFICIENTS,
        coefficient_upper,
        limits.max_union_support_coefficient_cells,
    )?;
    let mut raw = try_vec(UNION_BLOCK_ENTRIES, raw_count)?;
    for (direction_ordinal, direction) in block.directions().iter().enumerate() {
        for entry in direction.entries() {
            let physical = *obstruction
                .logical_physical_columns()
                .get(entry.logical_column())
                .ok_or(SourceDiscoveryError::Invariant {
                    detail: "obstruction-block entry is outside its logical column map",
                })?;
            let shift = obstruction.plan().columns().get(physical).ok_or(
                SourceDiscoveryError::Invariant {
                    detail: "obstruction-block support is outside its physical frame",
                },
            )?;
            if shift.len() != incidence.arity() {
                return Err(SourceDiscoveryError::WrongArity {
                    object: "obstruction-block support",
                    expected: incidence.arity(),
                    actual: shift.len(),
                });
            }
            raw.push(RawBlockEntry {
                shift: shift.clone(),
                direction: direction_ordinal,
                coefficient: entry.coefficient().clone(),
            });
        }
    }
    raw.sort_unstable_by(|left, right| {
        left.shift
            .values()
            .cmp(right.shift.values())
            .then_with(|| left.direction.cmp(&right.direction))
    });

    let field = Zp64::new(obstruction.sample_fingerprint().modulus());
    let mut support = try_vec(UNION_SUPPORT, raw_count)?;
    let mut cursor = 0usize;
    while cursor < raw.len() {
        let start = cursor;
        cursor += 1;
        while cursor < raw.len() && raw[cursor].shift == raw[start].shift {
            cursor += 1;
        }
        let requested = checked_add(UNION_SUPPORT, support.len(), 1)?;
        check_limit(UNION_SUPPORT, requested, limits.max_union_support_entries)?;
        let coordinate_cells =
            checked_mul(UNION_SUPPORT_COORDINATES, requested, incidence.arity())?;
        check_limit(
            UNION_SUPPORT_COORDINATES,
            coordinate_cells,
            limits.max_union_support_coordinate_cells,
        )?;
        let coefficient_cells = checked_mul(UNION_SUPPORT_COEFFICIENTS, requested, width)?;
        check_limit(
            UNION_SUPPORT_COEFFICIENTS,
            coefficient_cells,
            limits.max_union_support_coefficient_cells,
        )?;
        let mut coefficients = try_vec(UNION_SUPPORT_COEFFICIENTS, width)?;
        coefficients.resize_with(width, || field.zero());
        for raw_entry in &raw[start..cursor] {
            if !field.is_zero(&coefficients[raw_entry.direction]) {
                return Err(SourceDiscoveryError::Invariant {
                    detail: "one obstruction-block direction repeats a raw support shift",
                });
            }
            coefficients[raw_entry.direction] = raw_entry.coefficient.clone();
        }
        support.push(UnionObstructionSupportEntry::from_parts(
            raw[start].shift.clone(),
            coefficients,
        ));
    }
    if support.is_empty()
        || support
            .windows(2)
            .any(|pair| pair[0].shift().values() >= pair[1].shift().values())
        || support.iter().any(|entry| {
            entry.coefficients().len() != width
                || entry
                    .coefficients()
                    .iter()
                    .all(|coefficient| field.is_zero(coefficient))
        })
    {
        return Err(SourceDiscoveryError::Invariant {
            detail: "obstruction-block union support is empty or noncanonical",
        });
    }
    Ok(support)
}

fn nominate_union_requests(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    support: &[UnionObstructionSupportEntry],
    existing: &[TranslatedSourceRequest],
    limits: SourceDiscoveryLimits,
) -> Result<(Vec<TranslatedSourceRequest>, usize, usize, usize), SourceDiscoveryError> {
    let raw_count = checked_mul(
        UNION_INCIDENCE_VISITS,
        support.len(),
        incidence.term_occurrences(),
    )?;
    check_limit(
        UNION_INCIDENCE_VISITS,
        raw_count,
        limits.max_union_incidence_visits,
    )?;
    check_limit(UNION_RAW_REQUESTS, raw_count, limits.max_union_raw_requests)?;
    let coordinate_cells = checked_mul(UNION_REQUEST_COORDINATES, raw_count, incidence.arity())?;
    check_limit(
        UNION_REQUEST_COORDINATES,
        coordinate_cells,
        limits.max_union_request_coordinate_cells,
    )?;
    let mut requests = try_vec(UNION_RAW_REQUESTS, raw_count)?;
    for (support_ordinal, supported) in support.iter().enumerate() {
        for (source_ordinal, source) in incidence.sources().iter().enumerate() {
            for (term_ordinal, source_shift) in source.terms().keys().enumerate() {
                for (position, (&left, &right)) in supported
                    .shift()
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
                        .shift()
                        .values()
                        .iter()
                        .zip(source_shift.values())
                        .map(|(&left, &right)| {
                            left.checked_sub(right)
                                .expect("union incidence subtraction was checked component-wise")
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
            detail: "obstruction-block union incidence changed its preflighted visit count",
        });
    }
    requests.sort_unstable();
    requests.dedup();
    let unique_before = requests.len();
    check_limit(
        UNION_UNIQUE_REQUESTS,
        unique_before,
        limits.max_union_unique_requests,
    )?;
    requests.retain(|request| existing.binary_search(request).is_err());
    let excluded = unique_before - requests.len();
    Ok((requests, raw_count, unique_before, excluded))
}

fn verify_primary_subset(
    primary: &[TranslatedSourceRequest],
    union: &[TranslatedSourceRequest],
    limits: SourceDiscoveryLimits,
) -> Result<(), SourceDiscoveryError> {
    let mut primary_ordinal = 0usize;
    let mut union_ordinal = 0usize;
    let mut comparisons = 0usize;
    while primary_ordinal < primary.len() && union_ordinal < union.len() {
        comparisons = checked_add(UNION_SUBSET_COMPARISONS, comparisons, 1)?;
        check_limit(
            UNION_SUBSET_COMPARISONS,
            comparisons,
            limits.max_union_subset_comparisons,
        )?;
        match primary[primary_ordinal].cmp(&union[union_ordinal]) {
            std::cmp::Ordering::Less => {
                return Err(SourceDiscoveryError::Invariant {
                    detail: "primary obstruction nominations are not an exact union subset",
                });
            }
            std::cmp::Ordering::Equal => {
                primary_ordinal += 1;
                union_ordinal += 1;
            }
            std::cmp::Ordering::Greater => union_ordinal += 1,
        }
    }
    if primary_ordinal != primary.len() {
        return Err(SourceDiscoveryError::Invariant {
            detail: "primary obstruction nominations are not an exact union subset",
        });
    }
    Ok(())
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SourceDiscoveryError> {
    left.checked_mul(right)
        .ok_or(SourceDiscoveryError::ResourceCountOverflow { resource })
}
