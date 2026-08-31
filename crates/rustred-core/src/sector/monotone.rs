use std::sync::Arc;

use super::error::{Error, try_reserve_exact};
use super::{
    ComplexityComponent, InteriorBounds, Mask, OrderingPolicy, SectorInteriorDomain,
    ShiftComplexityKey, ShiftStrictDescentWitness,
};

mod target_partition;

pub use target_partition::{
    SectorMonotoneTargetCell, SectorMonotoneTargetCellKind, SectorMonotoneTargetCells,
    SectorMonotoneTargetPartition, SectorMonotoneTargetPartitionCensus,
};

/// The largest orthogonal parent-sector box on which one recurrence remains
/// i64-representable and its pivot stays in the parent sector.
///
/// RHS shifts need not preserve the parent mask. They are admitted separately
/// by [`SectorMonotoneShiftDescentWitness`], which either proves same-sector
/// descent or exposes a proper-subsector boundary cylinder. This domain alone
/// makes no statement about lower-sector rule availability or closure.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SectorMonotoneDomain {
    sector: Mask,
    bounds: Arc<Vec<InteriorBounds>>,
}

impl SectorMonotoneDomain {
    /// Construct an explicitly tightened parent-sector box for one rule.
    ///
    /// Unlike [`Self::try_maximal_for_rule`], this is the refinement seam used
    /// by exceptional and singleton foundry cells. Base points and the pivot
    /// remain in the declared parent sector; RHS shifts need only stay
    /// representable because their proper-subsector descent is proved
    /// separately.
    pub fn try_new_for_rule<S>(
        sector: Mask,
        bounds: impl IntoIterator<Item = InteriorBounds>,
        pivot_shift: &[i64],
        right_hand_side_shifts: &[S],
    ) -> Result<Self, Error>
    where
        S: AsRef<[i64]>,
    {
        check_arity(&sector, pivot_shift.len())?;
        for shift in right_hand_side_shifts {
            check_arity(&sector, shift.as_ref().len())?;
        }
        let interior = SectorInteriorDomain::try_new(sector, bounds)?;
        let mut retained = Vec::new();
        try_reserve_exact(
            &mut retained,
            interior.arity(),
            "sector-monotone domain bounds",
        )?;
        retained.extend_from_slice(interior.bounds());
        let domain = Self {
            sector: interior.sector().clone(),
            bounds: Arc::new(retained),
        };
        domain.require_parent_sector_shift(pivot_shift)?;
        for shift in right_hand_side_shifts {
            domain.require_representable_shift(shift.as_ref())?;
        }
        Ok(domain)
    }

    /// Construct the maximal representable parent-sector box for one pivot
    /// and all retained RHS shifts.
    ///
    /// Base points remain in `sector`; the pivot additionally remains in that
    /// same sector. RHS shifts constrain only i64 representability here.
    pub fn try_maximal_for_rule<S>(
        sector: Mask,
        pivot_shift: &[i64],
        right_hand_side_shifts: &[S],
    ) -> Result<Self, Error>
    where
        S: AsRef<[i64]>,
    {
        check_arity(&sector, pivot_shift.len())?;
        for shift in right_hand_side_shifts {
            check_arity(&sector, shift.as_ref().len())?;
        }

        let mut bounds = Vec::new();
        try_reserve_exact(&mut bounds, sector.arity(), "sector-monotone domain bounds")?;
        for (position, &active) in sector.active_bits().iter().enumerate() {
            let sector_lower = if active { 1_i64 } else { i64::MIN };
            let sector_upper = if active { i64::MAX } else { 0_i64 };
            let mut lower = i128::from(sector_lower);
            let mut upper = i128::from(sector_upper);

            tighten_representability(&mut lower, &mut upper, pivot_shift[position]);
            for shift in right_hand_side_shifts {
                tighten_representability(&mut lower, &mut upper, shift.as_ref()[position]);
            }

            // Unlike an RHS shift, the pivot must stay in the parent sector.
            let pivot = i128::from(pivot_shift[position]);
            lower = lower.max(i128::from(sector_lower) - pivot);
            upper = upper.min(i128::from(sector_upper) - pivot);
            lower = lower.max(i128::from(i64::MIN));
            upper = upper.min(i128::from(i64::MAX));
            if lower > upper {
                return Err(Error::EmptyShiftInterior { position });
            }
            bounds.push(InteriorBounds::new(
                i64::try_from(lower).map_err(|_| Error::EmptyShiftInterior { position })?,
                i64::try_from(upper).map_err(|_| Error::EmptyShiftInterior { position })?,
            ));
        }

        let domain = Self {
            sector,
            bounds: Arc::new(bounds),
        };
        domain.require_parent_sector_shift(pivot_shift)?;
        Ok(domain)
    }

    /// Tighten an existing rule domain just enough to keep one additional RHS
    /// shift representable.
    ///
    /// Growing exact frames add translated rows over time. Their semantic
    /// domain must therefore shrink monotonically as new extreme shifts
    /// approach the `i64` carrier boundary. This operation preserves every
    /// existing bound, rechecks that the pivot stays in the parent sector, and
    /// never widens a caller-supplied exceptional refinement.
    pub fn try_refine_for_additional_rhs_shift(
        &self,
        pivot_shift: &[i64],
        additional_shift: &[i64],
    ) -> Result<Self, Error> {
        self.check_arity(pivot_shift.len())?;
        self.check_arity(additional_shift.len())?;
        let mut bounds = Vec::new();
        try_reserve_exact(
            &mut bounds,
            self.arity(),
            "sector-monotone refined domain bounds",
        )?;
        for (position, (&current, &shift)) in self.bounds.iter().zip(additional_shift).enumerate() {
            let mut lower = i128::from(current.lower());
            let mut upper = i128::from(current.upper());
            tighten_representability(&mut lower, &mut upper, shift);
            lower = lower.max(i128::from(current.lower()));
            upper = upper.min(i128::from(current.upper()));
            if lower > upper {
                return Err(Error::EmptyShiftInterior { position });
            }
            bounds.push(InteriorBounds::new(
                i64::try_from(lower).map_err(|_| Error::EmptyShiftInterior { position })?,
                i64::try_from(upper).map_err(|_| Error::EmptyShiftInterior { position })?,
            ));
        }
        Self::try_new_for_rule(
            self.sector.clone(),
            bounds,
            pivot_shift,
            &[additional_shift],
        )
    }

    pub fn sector(&self) -> &Mask {
        &self.sector
    }

    pub fn arity(&self) -> usize {
        self.sector.arity()
    }

    pub fn bounds(&self) -> &[InteriorBounds] {
        self.bounds.as_slice()
    }

    pub fn contains(&self, indices: &[i64]) -> Result<bool, Error> {
        self.check_arity(indices.len())?;
        Ok(self
            .bounds
            .iter()
            .zip(indices)
            .all(|(&bounds, &index)| bounds.contains(index)))
    }

    /// Whether a shift stays i64-representable everywhere on this box. No RHS
    /// sector-preservation claim is implied.
    pub fn covers_representable_shift(&self, shift: &[i64]) -> Result<bool, Error> {
        self.check_arity(shift.len())?;
        Ok(self.bounds.iter().zip(shift).all(|(&bounds, &shift)| {
            let lower = i128::from(bounds.lower()) + i128::from(shift);
            let upper = i128::from(bounds.upper()) + i128::from(shift);
            i128::from(i64::MIN) <= lower && upper <= i128::from(i64::MAX)
        }))
    }

    /// Exact number of reachable first-pinched cylinders retained for one RHS
    /// shift. Once a coordinate is pinched throughout its bound, later
    /// cylinders are unreachable and are not counted.
    pub(crate) fn retained_pinch_threshold_count(&self, shift: &[i64]) -> Result<usize, Error> {
        self.check_arity(shift.len())?;
        let mut count = 0usize;
        for ((&bounds, &active), &shift) in
            self.bounds.iter().zip(self.sector.active_bits()).zip(shift)
        {
            if active && shift < 0 && -i128::from(shift) >= i128::from(bounds.lower()) {
                count = count.checked_add(1).ok_or(Error::ComplexityOverflow {
                    measure: "sector-monotone pinch threshold count",
                })?;
                if -i128::from(shift) >= i128::from(bounds.upper()) {
                    break;
                }
            }
        }
        Ok(count)
    }

    /// Translate one contained point without imposing a target-sector mask.
    pub fn checked_translate(
        &self,
        indices: &[i64],
        shift: &[i64],
    ) -> Result<Option<Vec<i64>>, Error> {
        self.check_arity(indices.len())?;
        self.check_arity(shift.len())?;
        if !self.contains(indices)? {
            return Ok(None);
        }
        let mut translated = Vec::new();
        try_reserve_exact(
            &mut translated,
            self.arity(),
            "translated sector-monotone point",
        )?;
        for (&index, &shift) in indices.iter().zip(shift) {
            let Some(index) = index.checked_add(shift) else {
                return Ok(None);
            };
            translated.push(index);
        }
        Ok(Some(translated))
    }

    fn require_representable_shift(&self, shift: &[i64]) -> Result<(), Error> {
        self.check_arity(shift.len())?;
        for (position, (&bounds, &shift)) in self.bounds.iter().zip(shift).enumerate() {
            let lower = i128::from(bounds.lower()) + i128::from(shift);
            let upper = i128::from(bounds.upper()) + i128::from(shift);
            if lower < i128::from(i64::MIN) || upper > i128::from(i64::MAX) {
                return Err(Error::ShiftNotCovered { position, shift });
            }
        }
        Ok(())
    }

    fn require_parent_sector_shift(&self, shift: &[i64]) -> Result<(), Error> {
        self.require_representable_shift(shift)?;
        for (position, ((&bounds, &shift), &active)) in self
            .bounds
            .iter()
            .zip(shift)
            .zip(self.sector.active_bits())
            .enumerate()
        {
            let lower = i128::from(bounds.lower()) + i128::from(shift);
            let upper = i128::from(bounds.upper()) + i128::from(shift);
            let sector_lower = if active { 1_i64 } else { i64::MIN };
            let sector_upper = if active { i64::MAX } else { 0_i64 };
            if lower < i128::from(sector_lower) || upper > i128::from(sector_upper) {
                return Err(Error::PivotLeavesParentSector { position, shift });
            }
        }
        Ok(())
    }

    fn check_arity(&self, actual: usize) -> Result<(), Error> {
        check_arity(&self.sector, actual)
    }
}

/// One active coordinate threshold in a compact disjoint pinch partition.
///
/// The corresponding cylinder consists of points for which every preceding
/// threshold is on its same-sector side and this coordinate is at most
/// `pinched_upper`. `same_sector_lower == None` means the coordinate is
/// pinched everywhere in the domain, so no later cylinder is reachable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActivePinchThreshold {
    position: usize,
    pinched_upper: i64,
    same_sector_lower: Option<i64>,
}

impl ActivePinchThreshold {
    pub fn position(self) -> usize {
        self.position
    }

    pub fn pinched_upper(self) -> i64 {
        self.pinched_upper
    }

    pub fn same_sector_lower(self) -> Option<i64> {
        self.same_sector_lower
    }
}

/// The unique cell of one term-local sector-monotone partition containing a
/// concrete parent-sector point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SectorMonotonePointClass {
    SameSector,
    ProperSubsector {
        cylinder_ordinal: usize,
        pinched_position: usize,
    },
}

/// Universal proof that one RHS shift is below a pivot throughout a
/// [`SectorMonotoneDomain`].
///
/// The same-sector cell, when nonempty, carries the ordinary structural shift
/// witness. Its complement is encoded by a deterministic first-pinched
/// threshold partition. Every such cylinder loses at least one active line,
/// activates no inactive line, and is therefore strictly lower at the
/// `PropagatorCount` component. This proof records dependencies; it does not
/// assert that rules for those subsectors exist.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SectorMonotoneShiftDescentWitness {
    policy: OrderingPolicy,
    domain: SectorMonotoneDomain,
    pivot: ShiftComplexityKey,
    target: ShiftComplexityKey,
    same_sector_descent: Option<ShiftStrictDescentWitness>,
    thresholds: Arc<Vec<ActivePinchThreshold>>,
}

impl SectorMonotoneShiftDescentWitness {
    pub fn policy(&self) -> OrderingPolicy {
        self.policy
    }

    pub fn domain(&self) -> &SectorMonotoneDomain {
        &self.domain
    }

    pub fn pivot(&self) -> &ShiftComplexityKey {
        &self.pivot
    }

    pub fn target(&self) -> &ShiftComplexityKey {
        &self.target
    }

    pub fn same_sector_descent(&self) -> Option<&ShiftStrictDescentWitness> {
        self.same_sector_descent.as_ref()
    }

    pub fn thresholds(&self) -> &[ActivePinchThreshold] {
        self.thresholds.as_slice()
    }

    /// Number of nonempty, pairwise-disjoint proper-subsector cylinders.
    pub fn proper_subsector_cylinder_count(&self) -> usize {
        self.thresholds
            .iter()
            .position(|threshold| threshold.same_sector_lower.is_none())
            .map_or(self.thresholds.len(), |ordinal| ordinal + 1)
    }

    pub const fn proper_subsector_decisive_component(&self) -> ComplexityComponent {
        ComplexityComponent::PropagatorCount
    }

    /// Classify one point. `Ok(None)` means it lies outside the universal box.
    pub fn classify(&self, indices: &[i64]) -> Result<Option<SectorMonotonePointClass>, Error> {
        if !self.domain.contains(indices)? {
            return Ok(None);
        }
        for (cylinder_ordinal, threshold) in self.thresholds.iter().enumerate() {
            if indices[threshold.position] <= threshold.pinched_upper {
                return Ok(Some(SectorMonotonePointClass::ProperSubsector {
                    cylinder_ordinal,
                    pinched_position: threshold.position,
                }));
            }
        }
        Ok(Some(SectorMonotonePointClass::SameSector))
    }

    /// Recheck representability, pivot preservation, inactive-line safety,
    /// the exact threshold partition, and the whole same-sector witness.
    pub fn verify(&self) -> bool {
        if !self
            .pivot
            .verifies_for_sector(self.policy, self.domain.sector())
            || !self
                .target
                .verifies_for_sector(self.policy, self.domain.sector())
            || !key_is_representable(&self.domain, &self.pivot)
            || !key_is_representable(&self.domain, &self.target)
            || !key_preserves_parent_sector(&self.domain, &self.pivot)
        {
            return false;
        }

        let mut expected_threshold = 0usize;
        let mut same_sector_nonempty = true;
        let mut partition_closed = false;
        for position in 0..self.domain.arity() {
            let Ok(target_shift) = self.target.shift_at(position) else {
                return false;
            };
            let active = self.domain.sector.active_bits()[position];
            if !active
                && target_shift > 0
                && i128::from(self.domain.bounds[position].upper()) + i128::from(target_shift) > 0
            {
                return false;
            }
            if !active || target_shift >= 0 || partition_closed {
                continue;
            }
            let bounds = self.domain.bounds[position];
            let raw_pinched_upper = -i128::from(target_shift);
            if raw_pinched_upper < i128::from(bounds.lower()) {
                continue;
            }
            let pinched_upper_i128 = raw_pinched_upper.min(i128::from(bounds.upper()));
            let Ok(pinched_upper) = i64::try_from(pinched_upper_i128) else {
                return false;
            };
            let raw_same_lower = raw_pinched_upper + 1;
            let same_lower_i128 = raw_same_lower.max(i128::from(bounds.lower()));
            let same_sector_lower = if same_lower_i128 <= i128::from(bounds.upper()) {
                let Ok(lower) = i64::try_from(same_lower_i128) else {
                    return false;
                };
                Some(lower)
            } else {
                same_sector_nonempty = false;
                partition_closed = true;
                None
            };
            if self.thresholds.get(expected_threshold)
                != Some(&ActivePinchThreshold {
                    position,
                    pinched_upper,
                    same_sector_lower,
                })
            {
                return false;
            }
            expected_threshold += 1;
        }
        if expected_threshold != self.thresholds.len() {
            return false;
        }

        match (&self.same_sector_descent, same_sector_nonempty) {
            (Some(witness), true) => {
                witness.verify()
                    && witness.policy() == self.policy
                    && witness.source() == &self.pivot
                    && witness.target() == &self.target
                    && same_sector_bounds_match(&self.domain, &self.target, witness.domain())
            }
            (None, false) => !self.thresholds.is_empty(),
            _ => false,
        }
    }
}

impl OrderingPolicy {
    /// Prove one shift lower than a pivot over a maximal parent-sector box,
    /// splitting same-sector and pinched regions without enumerating masks.
    pub fn prove_sector_monotone_shift_descent(
        self,
        domain: &SectorMonotoneDomain,
        pivot_shift: &[i64],
        target_shift: &[i64],
    ) -> Result<SectorMonotoneShiftDescentWitness, Error> {
        domain.require_parent_sector_shift(pivot_shift)?;
        domain.require_representable_shift(target_shift)?;

        let threshold_count = domain.retained_pinch_threshold_count(target_shift)?;
        let mut thresholds = Vec::new();
        try_reserve_exact(
            &mut thresholds,
            threshold_count,
            "sector-monotone pinch thresholds",
        )?;
        let mut same_bounds = Vec::new();
        try_reserve_exact(
            &mut same_bounds,
            domain.arity(),
            "sector-monotone same-sector bounds",
        )?;
        let mut same_sector_nonempty = true;
        let mut partition_closed = false;
        for (position, ((&bounds, &target_shift), &active)) in domain
            .bounds
            .iter()
            .zip(target_shift)
            .zip(domain.sector.active_bits())
            .enumerate()
        {
            // A positive shift on an inactive line is an activation only if
            // the tightened cell can actually reach a positive power. This
            // distinction is essential for numerator recurrences on faces:
            // e.g. n<=-1 with shift +1 remains wholly in the same inactive
            // sector. The maximal untightened domain still rejects it.
            if !active
                && target_shift > 0
                && i128::from(bounds.upper()) + i128::from(target_shift) > 0
            {
                return Err(Error::InactiveLineActivation {
                    position,
                    shift: target_shift,
                });
            }
            let mut same_lower = bounds.lower();
            if active && target_shift < 0 && !partition_closed {
                let raw_pinched_upper = -i128::from(target_shift);
                if raw_pinched_upper >= i128::from(bounds.lower()) {
                    let pinched_upper = i64::try_from(
                        raw_pinched_upper.min(i128::from(bounds.upper())),
                    )
                    .map_err(|_| Error::ComplexityOverflow {
                        measure: "sector-monotone pinch threshold",
                    })?;
                    let candidate = (raw_pinched_upper + 1).max(i128::from(bounds.lower()));
                    let threshold_same_lower = if candidate <= i128::from(bounds.upper()) {
                        Some(
                            i64::try_from(candidate).map_err(|_| Error::ComplexityOverflow {
                                measure: "sector-monotone same-sector threshold",
                            })?,
                        )
                    } else {
                        same_sector_nonempty = false;
                        partition_closed = true;
                        None
                    };
                    thresholds.push(ActivePinchThreshold {
                        position,
                        pinched_upper,
                        same_sector_lower: threshold_same_lower,
                    });
                    if let Some(candidate) = threshold_same_lower {
                        same_lower = candidate;
                    }
                }
            }
            same_bounds.push(InteriorBounds::new(same_lower, bounds.upper()));
        }

        let same_sector_descent = if same_sector_nonempty {
            let same_domain = SectorInteriorDomain::try_new(domain.sector.clone(), same_bounds)?;
            Some(self.prove_shift_strict_descent(&same_domain, pivot_shift, target_shift)?)
        } else {
            None
        };
        let (pivot, target) = if let Some(witness) = &same_sector_descent {
            (witness.source().clone(), witness.target().clone())
        } else {
            (
                self.shift_complexity_key(domain.sector(), pivot_shift)?,
                self.shift_complexity_key(domain.sector(), target_shift)?,
            )
        };
        let witness = SectorMonotoneShiftDescentWitness {
            policy: self,
            domain: domain.clone(),
            pivot,
            target,
            same_sector_descent,
            thresholds: Arc::new(thresholds),
        };
        if !witness.verify() {
            return Err(Error::NotStrictDescent);
        }
        Ok(witness)
    }
}

fn check_arity(sector: &Mask, actual: usize) -> Result<(), Error> {
    if actual == sector.arity() {
        Ok(())
    } else {
        Err(Error::WrongArity {
            expected: sector.arity(),
            actual,
        })
    }
}

fn tighten_representability(lower: &mut i128, upper: &mut i128, shift: i64) {
    let shift = i128::from(shift);
    *lower = (*lower).max(i128::from(i64::MIN) - shift);
    *upper = (*upper).min(i128::from(i64::MAX) - shift);
}

fn key_is_representable(domain: &SectorMonotoneDomain, key: &ShiftComplexityKey) -> bool {
    (0..domain.arity()).all(|position| {
        let Ok(shift) = key.shift_at(position) else {
            return false;
        };
        let bounds = domain.bounds[position];
        let lower = i128::from(bounds.lower()) + i128::from(shift);
        let upper = i128::from(bounds.upper()) + i128::from(shift);
        i128::from(i64::MIN) <= lower && upper <= i128::from(i64::MAX)
    })
}

fn key_preserves_parent_sector(domain: &SectorMonotoneDomain, key: &ShiftComplexityKey) -> bool {
    (0..domain.arity()).all(|position| {
        let Ok(shift) = key.shift_at(position) else {
            return false;
        };
        let bounds = domain.bounds[position];
        let lower = i128::from(bounds.lower()) + i128::from(shift);
        let upper = i128::from(bounds.upper()) + i128::from(shift);
        let active = domain.sector.active_bits()[position];
        let sector_lower = if active { 1_i64 } else { i64::MIN };
        let sector_upper = if active { i64::MAX } else { 0_i64 };
        i128::from(sector_lower) <= lower && upper <= i128::from(sector_upper)
    })
}

fn same_sector_bounds_match(
    domain: &SectorMonotoneDomain,
    target: &ShiftComplexityKey,
    same_domain: &SectorInteriorDomain,
) -> bool {
    if same_domain.sector() != domain.sector() || same_domain.arity() != domain.arity() {
        return false;
    }
    for position in 0..domain.arity() {
        let Ok(shift) = target.shift_at(position) else {
            return false;
        };
        let bounds = domain.bounds[position];
        let expected_lower = if domain.sector.active_bits()[position] && shift < 0 {
            let candidate = -i128::from(shift) + 1;
            if candidate > i128::from(bounds.upper()) {
                return false;
            }
            i64::try_from(candidate.max(i128::from(bounds.lower()))).ok()
        } else {
            Some(bounds.lower())
        };
        if Some(same_domain.bounds()[position].lower()) != expected_lower
            || same_domain.bounds()[position].upper() != bounds.upper()
        {
            return false;
        }
    }
    true
}
