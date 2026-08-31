use crate::sector::{
    Error as SectorError, OrderingPolicy, SectorMonotoneShiftDescentWitness,
    SectorMonotoneTargetCellKind,
};

use super::super::frame::PhysicalFramePlan;
use super::{
    DecoratedStratum, DecoratedStratumId, ImmutableOwnerSnapshot, ImmutableOwnerSnapshotId,
    ImmutableOwnerWitness, StratumRegistryError, StratumRegistryLimits, check_limit, checked_add,
    checked_mul, try_reserve,
};

/// Why one non-target physical column cannot enter the allowed RHS block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ForbiddenColumnReason {
    NotStrictDescent,
    InactiveLineActivation {
        position: usize,
        shift: i64,
    },
    UnownedProperSubsector {
        first_cell_ordinal: usize,
        proper_subsector_cell_count: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ForbiddenColumnDescriptor {
    column: usize,
    reason: ForbiddenColumnReason,
}

/// Prospective role of an exact shift after the current semantic domain is
/// monotonically refined to represent it. This is a checked scheduling
/// classification only; it does not add the shift to the physical plan or
/// mint a retained descent/owner witness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProspectiveColumnKind {
    Target,
    Allowed,
    Forbidden,
}

impl ForbiddenColumnDescriptor {
    pub(crate) const fn column(&self) -> usize {
        self.column
    }

    pub(crate) const fn reason(&self) -> &ForbiddenColumnReason {
        &self.reason
    }
}

/// Immutable lower-sector owner retained for one exact target-sector cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ProperSubsectorOwner {
    cell_ordinal: usize,
    owner: ImmutableOwnerWitness,
}

impl ProperSubsectorOwner {
    pub(crate) const fn cell_ordinal(self) -> usize {
        self.cell_ordinal
    }

    pub(crate) const fn owner(self) -> ImmutableOwnerWitness {
        self.owner
    }
}

/// Whole-stratum proof that one physical column is an admissible RHS column.
///
/// The sector witness proves strict descent on the same-sector cell and at
/// every proper-subsector transition. Every materialized proper-subsector
/// child additionally points into the immutable owner snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AllowedColumnDescriptor {
    column: usize,
    descent: SectorMonotoneShiftDescentWitness,
    proper_subsector_owners: Box<[ProperSubsectorOwner]>,
}

impl AllowedColumnDescriptor {
    pub(crate) const fn column(&self) -> usize {
        self.column
    }

    pub(crate) const fn descent(&self) -> &SectorMonotoneShiftDescentWitness {
        &self.descent
    }

    pub(crate) fn proper_subsector_owners(&self) -> &[ProperSubsectorOwner] {
        &self.proper_subsector_owners
    }
}

/// Exhaustive target/allowed/forbidden physical-column registry.
///
/// The target is in neither returned column slice. Every other raw physical
/// column appears exactly once. Provenance identity columns cannot enter
/// because the registry borrows the raw [`PhysicalFramePlan`] column table.
#[derive(Debug)]
pub(crate) struct TargetColumnPartition<'frame> {
    frame: &'frame PhysicalFramePlan,
    stratum: DecoratedStratum,
    owners: ImmutableOwnerSnapshot,
    target_column: usize,
    ordering: OrderingPolicy,
    allowed: Box<[AllowedColumnDescriptor]>,
    forbidden: Box<[usize]>,
    forbidden_descriptors: Box<[ForbiddenColumnDescriptor]>,
    limits: StratumRegistryLimits,
}

impl<'frame> TargetColumnPartition<'frame> {
    pub(crate) fn try_new(
        frame: &'frame PhysicalFramePlan,
        target_column: usize,
        stratum: DecoratedStratum,
        owners: ImmutableOwnerSnapshot,
        ordering: OrderingPolicy,
        limits: StratumRegistryLimits,
    ) -> Result<Self, StratumRegistryError> {
        if !stratum.try_verify(limits)? {
            return Err(StratumRegistryError::Invariant {
                detail: "incoming decorated stratum failed cold verification",
            });
        }
        if !owners.try_verify(limits)? {
            return Err(StratumRegistryError::Invariant {
                detail: "incoming immutable owner snapshot failed cold verification",
            });
        }
        validate_scope(frame, &stratum, &owners, target_column, limits)?;
        let pivot = frame.columns()[target_column].values();
        validate_pivot(ordering, stratum.domain(), pivot)?;

        let non_target_columns =
            frame
                .columns()
                .len()
                .checked_sub(1)
                .ok_or(StratumRegistryError::Invariant {
                    detail: "a physical frame has no target column",
                })?;
        let mut allowed = Vec::new();
        let mut forbidden = Vec::new();
        let mut forbidden_descriptors = Vec::new();
        try_reserve(
            &mut allowed,
            non_target_columns,
            "decorated-stratum allowed-column descriptors",
        )?;
        try_reserve(
            &mut forbidden,
            non_target_columns,
            "decorated-stratum forbidden columns",
        )?;
        try_reserve(
            &mut forbidden_descriptors,
            non_target_columns,
            "decorated-stratum forbidden-column descriptors",
        )?;

        let mut admitted_target_cells = 0usize;
        let mut admitted_owner_probes = 0usize;
        let mut retained_owner_witnesses = 0usize;
        for (column, shift) in frame.columns().iter().enumerate() {
            if column == target_column {
                continue;
            }
            let descent = match ordering.prove_sector_monotone_shift_descent(
                stratum.domain(),
                pivot,
                shift.values(),
            ) {
                Ok(descent) => descent,
                Err(SectorError::NotStrictDescent) => {
                    push_forbidden(
                        &mut forbidden,
                        &mut forbidden_descriptors,
                        column,
                        ForbiddenColumnReason::NotStrictDescent,
                    );
                    continue;
                }
                Err(SectorError::InactiveLineActivation { position, shift }) => {
                    push_forbidden(
                        &mut forbidden,
                        &mut forbidden_descriptors,
                        column,
                        ForbiddenColumnReason::InactiveLineActivation { position, shift },
                    );
                    continue;
                }
                Err(error) => return Err(StratumRegistryError::Sector(error)),
            };
            let census = descent.target_sector_partition_census()?;
            admitted_target_cells = checked_add(
                "decorated-stratum target-sector cells",
                admitted_target_cells,
                census.cell_count(),
            )?;
            check_limit(
                "decorated-stratum target-sector cells",
                admitted_target_cells,
                limits.max_target_sector_cells,
            )?;
            let partition = descent.try_target_sector_partition()?;
            let proper_count = partition.proper_subsector_cell_count();
            if proper_count != 0 && owners.route_count() == 0 {
                push_forbidden(
                    &mut forbidden,
                    &mut forbidden_descriptors,
                    column,
                    ForbiddenColumnReason::UnownedProperSubsector {
                        first_cell_ordinal: 0,
                        proper_subsector_cell_count: proper_count,
                    },
                );
                continue;
            }
            let potential_retained = checked_add(
                "decorated-stratum retained owner witnesses",
                retained_owner_witnesses,
                proper_count,
            )?;
            check_limit(
                "decorated-stratum retained owner witnesses",
                potential_retained,
                limits.max_retained_owner_witnesses,
            )?;
            let mut proper_owners = Vec::new();
            try_reserve(
                &mut proper_owners,
                proper_count,
                "decorated-stratum proper-subsector owner witnesses",
            )?;
            let mut first_unowned = None;
            for cell_ordinal in 0..proper_count {
                let cell = partition.cell(cell_ordinal)?;
                if cell.kind() != SectorMonotoneTargetCellKind::ProperSubsector {
                    return Err(StratumRegistryError::Invariant {
                        detail: "proper-subsector partition prefix contains a same-sector cell",
                    });
                }
                admitted_owner_probes = checked_add(
                    "decorated-stratum immutable-owner probes",
                    admitted_owner_probes,
                    owners.route_candidates_for_sector(cell.target_domain().sector()),
                )?;
                check_limit(
                    "decorated-stratum immutable-owner probes",
                    admitted_owner_probes,
                    limits.max_owner_probes,
                )?;
                let Some(owner) = owners.owner_for(frame.sector(), ordering, cell.target_domain())
                else {
                    first_unowned = Some(cell_ordinal);
                    break;
                };
                proper_owners.push(ProperSubsectorOwner {
                    cell_ordinal,
                    owner,
                });
            }
            if let Some(first_cell_ordinal) = first_unowned {
                push_forbidden(
                    &mut forbidden,
                    &mut forbidden_descriptors,
                    column,
                    ForbiddenColumnReason::UnownedProperSubsector {
                        first_cell_ordinal,
                        proper_subsector_cell_count: proper_count,
                    },
                );
                continue;
            }
            retained_owner_witnesses = potential_retained;
            allowed.push(AllowedColumnDescriptor {
                column,
                descent,
                proper_subsector_owners: proper_owners.into_boxed_slice(),
            });
        }

        let result = Self {
            frame,
            stratum,
            owners,
            target_column,
            ordering,
            allowed: allowed.into_boxed_slice(),
            forbidden: forbidden.into_boxed_slice(),
            forbidden_descriptors: forbidden_descriptors.into_boxed_slice(),
            limits,
        };
        if !result.partition_is_exhaustive() {
            return Err(StratumRegistryError::Invariant {
                detail: "target/allowed/forbidden columns do not form an exact partition",
            });
        }
        Ok(result)
    }

    pub(crate) const fn frame(&self) -> &'frame PhysicalFramePlan {
        self.frame
    }

    pub(crate) const fn target_column(&self) -> usize {
        self.target_column
    }

    pub(crate) const fn stratum(&self) -> &DecoratedStratum {
        &self.stratum
    }

    pub(crate) const fn stratum_id(&self) -> &DecoratedStratumId {
        self.stratum.id()
    }

    pub(crate) const fn snapshot_id(&self) -> &ImmutableOwnerSnapshotId {
        self.owners.id()
    }

    pub(crate) const fn ordering(&self) -> OrderingPolicy {
        self.ordering
    }

    pub(crate) fn allowed_columns(&self) -> &[AllowedColumnDescriptor] {
        &self.allowed
    }

    pub(crate) fn forbidden_columns(&self) -> &[usize] {
        &self.forbidden
    }

    pub(crate) fn forbidden_descriptors(&self) -> &[ForbiddenColumnDescriptor] {
        &self.forbidden_descriptors
    }

    pub(crate) fn is_allowed(&self, column: usize) -> bool {
        self.allowed
            .binary_search_by_key(&column, AllowedColumnDescriptor::column)
            .is_ok()
    }

    pub(crate) fn allowed_descriptor(&self, column: usize) -> Option<&AllowedColumnDescriptor> {
        self.allowed
            .binary_search_by_key(&column, AllowedColumnDescriptor::column)
            .ok()
            .map(|ordinal| &self.allowed[ordinal])
    }

    pub(crate) fn forbidden_reason(&self, column: usize) -> Option<&ForbiddenColumnReason> {
        self.forbidden_descriptors
            .binary_search_by_key(&column, ForbiddenColumnDescriptor::column)
            .ok()
            .map(|ordinal| self.forbidden_descriptors[ordinal].reason())
    }

    /// Classify a shift not yet materialized in the physical plan under the
    /// exact same descent and immutable-lower-owner policy as existing
    /// columns, after applying the monotone representability refinement which
    /// the next growing epoch would require. The result is proposal telemetry,
    /// never admission authority; a selected row is fully repartitioned when
    /// the next fresh epoch is built.
    pub(crate) fn try_classify_prospective_shift(
        &self,
        shift: &[i64],
    ) -> Result<ProspectiveColumnKind, StratumRegistryError> {
        if shift.len() != self.frame.sector().arity() {
            return Err(StratumRegistryError::Sector(SectorError::WrongArity {
                expected: self.frame.sector().arity(),
                actual: shift.len(),
            }));
        }
        let pivot = self.frame.columns()[self.target_column].values();
        if shift == pivot {
            return Ok(ProspectiveColumnKind::Target);
        }
        let prospective_domain = self
            .stratum
            .domain()
            .try_refine_for_additional_rhs_shift(pivot, shift)?;
        let descent = match self.ordering.prove_sector_monotone_shift_descent(
            &prospective_domain,
            pivot,
            shift,
        ) {
            Ok(descent) => descent,
            Err(SectorError::NotStrictDescent | SectorError::InactiveLineActivation { .. }) => {
                return Ok(ProspectiveColumnKind::Forbidden);
            }
            Err(error) => return Err(StratumRegistryError::Sector(error)),
        };
        let census = descent.target_sector_partition_census()?;
        check_limit(
            "prospective target-sector cells",
            census.cell_count(),
            self.limits.max_target_sector_cells,
        )?;
        check_limit(
            "prospective retained owner witnesses",
            census.proper_subsector_cell_count(),
            self.limits.max_retained_owner_witnesses,
        )?;

        let partition = descent.try_target_sector_partition()?;
        let proper_count = partition.proper_subsector_cell_count();
        if proper_count != 0 && self.owners.route_count() == 0 {
            return Ok(ProspectiveColumnKind::Forbidden);
        }
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
                self.owners
                    .route_candidates_for_sector(cell.target_domain().sector()),
            )?;
            check_limit(
                "prospective immutable-owner probes",
                owner_probes,
                self.limits.max_owner_probes,
            )?;
            if self
                .owners
                .owner_for(self.frame.sector(), self.ordering, cell.target_domain())
                .is_none()
            {
                return Ok(ProspectiveColumnKind::Forbidden);
            }
        }
        Ok(ProspectiveColumnKind::Allowed)
    }

    /// Cold-path reconstruction of the complete registry and all retained
    /// lower-sector owner witnesses.
    pub(crate) fn try_verify(&self) -> Result<bool, StratumRegistryError> {
        if !self.stratum.try_verify(self.limits)? || !self.owners.try_verify(self.limits)? {
            return Ok(false);
        }
        if !self.retained_owner_witnesses_verify()? {
            return Ok(false);
        }
        let rebuilt = Self::try_new(
            self.frame,
            self.target_column,
            self.stratum.clone(),
            self.owners.clone(),
            self.ordering,
            self.limits,
        )?;
        Ok(self.equivalent_to(&rebuilt))
    }

    fn retained_owner_witnesses_verify(&self) -> Result<bool, StratumRegistryError> {
        for descriptor in &self.allowed {
            let partition = descriptor.descent().try_target_sector_partition()?;
            if partition.proper_subsector_cell_count() != descriptor.proper_subsector_owners().len()
            {
                return Ok(false);
            }
            for (expected_ordinal, retained) in descriptor
                .proper_subsector_owners()
                .iter()
                .copied()
                .enumerate()
            {
                if retained.cell_ordinal() != expected_ordinal {
                    return Ok(false);
                }
                let cell = partition.cell(retained.cell_ordinal())?;
                if cell.kind() != SectorMonotoneTargetCellKind::ProperSubsector
                    || !self.owners.verifies_witness(
                        self.frame.sector(),
                        self.ordering,
                        cell.target_domain(),
                        retained.owner(),
                    )
                {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn equivalent_to(&self, other: &Self) -> bool {
        std::ptr::eq(self.frame, other.frame)
            && self.stratum == other.stratum
            && self.owners == other.owners
            && self.target_column == other.target_column
            && self.ordering == other.ordering
            && self.allowed == other.allowed
            && self.forbidden == other.forbidden
            && self.forbidden_descriptors == other.forbidden_descriptors
            && self.limits == other.limits
    }

    fn partition_is_exhaustive(&self) -> bool {
        if self
            .allowed
            .windows(2)
            .any(|pair| pair[0].column >= pair[1].column)
            || self.forbidden.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .forbidden_descriptors
                .windows(2)
                .any(|pair| pair[0].column >= pair[1].column)
            || self.forbidden.len() != self.forbidden_descriptors.len()
            || self
                .forbidden
                .iter()
                .zip(self.forbidden_descriptors.iter())
                .any(|(&column, descriptor)| column != descriptor.column)
        {
            return false;
        }
        let mut allowed = 0usize;
        let mut forbidden = 0usize;
        for column in 0..self.frame.columns().len() {
            if column == self.target_column {
                continue;
            }
            match (
                self.allowed
                    .get(allowed)
                    .map(AllowedColumnDescriptor::column),
                self.forbidden.get(forbidden).copied(),
            ) {
                (Some(candidate), _) if candidate == column => allowed += 1,
                (_, Some(candidate)) if candidate == column => forbidden += 1,
                _ => return false,
            }
        }
        allowed == self.allowed.len() && forbidden == self.forbidden.len()
    }
}

fn validate_scope(
    frame: &PhysicalFramePlan,
    stratum: &DecoratedStratum,
    owners: &ImmutableOwnerSnapshot,
    target_column: usize,
    limits: StratumRegistryLimits,
) -> Result<(), StratumRegistryError> {
    if target_column >= frame.columns().len() {
        return Err(StratumRegistryError::TargetColumnOutOfRange {
            target: target_column,
            columns: frame.columns().len(),
        });
    }
    if frame.family_fingerprint() != stratum.family_fingerprint() {
        return Err(StratumRegistryError::WrongFrameFamily);
    }
    if frame.context_fingerprint() != stratum.context_fingerprint() {
        return Err(StratumRegistryError::WrongFrameContext);
    }
    if frame.sector() != stratum.domain().sector() {
        return Err(StratumRegistryError::WrongFrameSector);
    }
    if owners.family_fingerprint() != stratum.family_fingerprint() {
        return Err(StratumRegistryError::WrongOwnerFamily);
    }
    if owners.context_fingerprint() != stratum.context_fingerprint() {
        return Err(StratumRegistryError::WrongOwnerContext);
    }
    if owners.arity() != stratum.domain().arity() {
        return Err(StratumRegistryError::WrongOwnerArity {
            owner: 0,
            expected: stratum.domain().arity(),
            actual: owners.arity(),
        });
    }
    check_limit(
        "decorated-stratum physical columns",
        frame.columns().len(),
        limits.max_physical_columns,
    )?;
    let coordinate_cells = checked_mul(
        "decorated-stratum physical-column coordinate cells",
        frame.columns().len(),
        stratum.domain().arity(),
    )?;
    check_limit(
        "decorated-stratum physical-column coordinate cells",
        coordinate_cells,
        limits.max_column_coordinate_cells,
    )?;
    for (column, shift) in frame.columns().iter().enumerate() {
        if !stratum
            .domain()
            .covers_representable_shift(shift.values())?
        {
            return Err(StratumRegistryError::UncoveredPhysicalShift { column });
        }
    }
    Ok(())
}

fn validate_pivot(
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

fn push_forbidden(
    columns: &mut Vec<usize>,
    descriptors: &mut Vec<ForbiddenColumnDescriptor>,
    column: usize,
    reason: ForbiddenColumnReason,
) {
    columns.push(column);
    descriptors.push(ForbiddenColumnDescriptor { column, reason });
}
