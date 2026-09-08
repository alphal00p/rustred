use crate::identity::IntegralShift;
use crate::sector::{Error as SectorError, OrderingPolicy, SectorMonotoneTargetCellKind};

use super::{
    DecoratedStratum, ImmutableOwnerSnapshot, StratumRegistryError, StratumRegistryLimits,
    VerifiedImmutableOwnerSnapshot, check_limit, checked_add,
};

/// Prospective role of an exact shift after the current semantic domain is
/// monotonically refined to represent it. This is checked scheduling
/// information only: it does not mint a retained descent or owner witness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProspectiveColumnKind {
    Target,
    Allowed,
    Forbidden,
}

/// Exact work retained or performed while assigning one prospective role.
/// Streaming callers aggregate this cost once per unique cached shift so a
/// succession of individually small classifications cannot evade the same
/// resource policy as a materialized physical partition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProspectiveClassificationCost {
    target_sector_cells: usize,
    owner_probes: usize,
    retained_owner_witnesses: usize,
}

impl ProspectiveClassificationCost {
    pub(crate) const fn target_sector_cells(self) -> usize {
        self.target_sector_cells
    }

    pub(crate) const fn owner_probes(self) -> usize {
        self.owner_probes
    }

    pub(crate) const fn retained_owner_witnesses(self) -> usize {
        self.retained_owner_witnesses
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProspectiveColumnClassification {
    kind: ProspectiveColumnKind,
    cost: ProspectiveClassificationCost,
}

impl ProspectiveColumnClassification {
    pub(crate) const fn kind(self) -> ProspectiveColumnKind {
        self.kind
    }

    pub(crate) const fn cost(self) -> ProspectiveClassificationCost {
        self.cost
    }
}

/// Frame-independent classifier for structural shifts encountered by a
/// streaming completion search.
///
/// Construction authenticates the fixed target, decorated case, ordering,
/// and immutable lower-owner snapshot once. Individual classifications then
/// reuse exactly the same descent and owner policy as
/// [`super::TargetColumnPartition`] without materializing a physical frame.
#[derive(Clone, Debug)]
pub(crate) struct ProspectiveColumnClassifier {
    stratum: DecoratedStratum,
    owners: VerifiedImmutableOwnerSnapshot,
    target_shift: IntegralShift,
    ordering: OrderingPolicy,
    limits: StratumRegistryLimits,
}

impl ProspectiveColumnClassifier {
    pub(crate) fn try_new(
        stratum: DecoratedStratum,
        target_shift: IntegralShift,
        owners: ImmutableOwnerSnapshot,
        ordering: OrderingPolicy,
        limits: StratumRegistryLimits,
    ) -> Result<Self, StratumRegistryError> {
        if !owners.try_verify(limits)? {
            return Err(StratumRegistryError::Invariant {
                detail: "incoming immutable owner snapshot failed cold verification",
            });
        }
        Self::try_new_with_verified_snapshot(
            stratum,
            target_shift,
            owners.verified_clone(),
            ordering,
            limits,
        )
    }

    /// Construct from an already installed owner snapshot while retaining all
    /// cheap fixed-scope and resource checks.
    pub(crate) fn try_new_with_verified_snapshot(
        stratum: DecoratedStratum,
        target_shift: IntegralShift,
        owners: VerifiedImmutableOwnerSnapshot,
        ordering: OrderingPolicy,
        limits: StratumRegistryLimits,
    ) -> Result<Self, StratumRegistryError> {
        if !stratum.try_verify(limits)? {
            return Err(StratumRegistryError::Invariant {
                detail: "incoming decorated stratum failed cold verification",
            });
        }
        owners.try_preflight_limits(limits)?;
        validate_fixed_scope(&stratum, &target_shift, &owners, ordering)?;
        Ok(Self {
            stratum,
            owners,
            target_shift,
            ordering,
            limits,
        })
    }

    pub(crate) const fn stratum(&self) -> &DecoratedStratum {
        &self.stratum
    }

    pub(crate) const fn target_shift(&self) -> &IntegralShift {
        &self.target_shift
    }

    pub(crate) const fn ordering(&self) -> OrderingPolicy {
        self.ordering
    }

    pub(crate) const fn limits(&self) -> StratumRegistryLimits {
        self.limits
    }

    pub(crate) fn try_classify_shift(
        &self,
        shift: &[i64],
    ) -> Result<ProspectiveColumnKind, StratumRegistryError> {
        Ok(self.try_classify_shift_with_cost(shift)?.kind())
    }

    pub(crate) fn try_classify_shift_with_cost(
        &self,
        shift: &[i64],
    ) -> Result<ProspectiveColumnClassification, StratumRegistryError> {
        try_classify_prospective_shift(
            &self.stratum,
            &self.owners,
            self.target_shift.values(),
            self.ordering,
            self.limits,
            shift,
        )
    }
}

fn validate_fixed_scope(
    stratum: &DecoratedStratum,
    target_shift: &IntegralShift,
    owners: &VerifiedImmutableOwnerSnapshot,
    ordering: OrderingPolicy,
) -> Result<(), StratumRegistryError> {
    let arity = stratum.domain().arity();
    if target_shift.len() != arity {
        return Err(StratumRegistryError::Sector(SectorError::WrongArity {
            expected: arity,
            actual: target_shift.len(),
        }));
    }
    if owners.family_fingerprint() != stratum.family_fingerprint() {
        return Err(StratumRegistryError::WrongOwnerFamily);
    }
    if owners.context_fingerprint() != stratum.context_fingerprint() {
        return Err(StratumRegistryError::WrongOwnerContext);
    }
    if owners.arity() != arity {
        return Err(StratumRegistryError::WrongOwnerArity {
            owner: 0,
            expected: arity,
            actual: owners.arity(),
        });
    }
    if let Some(snapshot_ordering) = owners.canonicalizer_ordering()
        && snapshot_ordering != ordering
    {
        return Err(StratumRegistryError::WrongOwnerRouteCanonicalizer);
    }
    if !stratum
        .domain()
        .covers_representable_shift(target_shift.values())?
    {
        return Err(StratumRegistryError::UncoveredProspectiveTarget);
    }
    try_validate_pivot(ordering, stratum.domain(), target_shift.values())
}

pub(super) fn try_validate_pivot(
    ordering: OrderingPolicy,
    domain: &crate::sector::SectorMonotoneDomain,
    pivot: &[i64],
) -> Result<(), StratumRegistryError> {
    match ordering.prove_sector_monotone_shift_descent(domain, pivot, pivot) {
        Err(SectorError::NotStrictDescent) => Ok(()),
        Err(error) => Err(StratumRegistryError::Sector(error)),
        Ok(_) => Err(StratumRegistryError::Invariant {
            detail: "an ordering proved one shift strictly below itself",
        }),
    }
}

pub(super) fn try_classify_prospective_shift(
    stratum: &DecoratedStratum,
    owners: &VerifiedImmutableOwnerSnapshot,
    pivot: &[i64],
    ordering: OrderingPolicy,
    limits: StratumRegistryLimits,
    shift: &[i64],
) -> Result<ProspectiveColumnClassification, StratumRegistryError> {
    if shift.len() != stratum.domain().arity() {
        return Err(StratumRegistryError::Sector(SectorError::WrongArity {
            expected: stratum.domain().arity(),
            actual: shift.len(),
        }));
    }
    if shift == pivot {
        return Ok(ProspectiveColumnClassification {
            kind: ProspectiveColumnKind::Target,
            cost: ProspectiveClassificationCost::default(),
        });
    }
    let prospective_domain = stratum
        .domain()
        .try_refine_for_additional_rhs_shift(pivot, shift)?;
    let descent =
        match ordering.prove_sector_monotone_shift_descent(&prospective_domain, pivot, shift) {
            Ok(descent) => descent,
            Err(SectorError::NotStrictDescent | SectorError::InactiveLineActivation { .. }) => {
                return Ok(ProspectiveColumnClassification {
                    kind: ProspectiveColumnKind::Forbidden,
                    cost: ProspectiveClassificationCost::default(),
                });
            }
            Err(error) => return Err(StratumRegistryError::Sector(error)),
        };
    let census = descent.target_sector_partition_census()?;
    check_limit(
        "prospective target-sector cells",
        census.cell_count(),
        limits.max_target_sector_cells,
    )?;
    let partition = descent.try_target_sector_partition()?;
    let proper_count = partition.proper_subsector_cell_count();
    let target_sector_cells = census.cell_count();
    if proper_count != 0 && owners.route_count() == 0 {
        return Ok(ProspectiveColumnClassification {
            kind: ProspectiveColumnKind::Forbidden,
            cost: ProspectiveClassificationCost {
                target_sector_cells,
                owner_probes: 0,
                retained_owner_witnesses: 0,
            },
        });
    }
    check_limit(
        "prospective retained owner witnesses",
        proper_count,
        limits.max_retained_owner_witnesses,
    )?;
    let mut owner_probes = 0usize;
    for cell_ordinal in 0..proper_count {
        let cell = partition.cell(cell_ordinal)?;
        if cell.kind() != SectorMonotoneTargetCellKind::ProperSubsector {
            return Err(StratumRegistryError::Invariant {
                detail: "prospective proper-subsector prefix contains a same-sector cell",
            });
        }
        owner_probes = checked_add(
            "prospective immutable-owner probes",
            owner_probes,
            owners.route_candidates_for_sector(cell.target_domain().sector()),
        )?;
        check_limit(
            "prospective immutable-owner probes",
            owner_probes,
            limits.max_owner_probes,
        )?;
        if owners
            .owner_for(stratum.domain().sector(), ordering, cell.target_domain())
            .is_none()
        {
            return Ok(ProspectiveColumnClassification {
                kind: ProspectiveColumnKind::Forbidden,
                cost: ProspectiveClassificationCost {
                    target_sector_cells,
                    owner_probes,
                    retained_owner_witnesses: 0,
                },
            });
        }
    }
    Ok(ProspectiveColumnClassification {
        kind: ProspectiveColumnKind::Allowed,
        cost: ProspectiveClassificationCost {
            target_sector_cells,
            owner_probes,
            retained_owner_witnesses: proper_count,
        },
    })
}
