use std::cmp::Ordering;
use std::fmt;
use std::sync::Arc;

use super::error::{Error, try_reserve_exact};
use super::{ComplexityComponent, Mask, OrderingPolicy, SectorInteriorDomain};

/// The n-independent part of the exact integral key for a shift in a fixed
/// sector.
///
/// On a sector-preserving interior, substituting `n + shift` into
/// [`super::ComplexityKey`] contributes the same symbolic n terms to every
/// candidate. This key retains precisely the remaining signed offsets, in the
/// v1 comparison order: corner distance, dots, numerators, then coordinate
/// excess.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ShiftComplexityKey {
    policy: OrderingPolicy,
    arity: usize,
    sector: Mask,
    corner_distance_offset: i128,
    dot_offset: i128,
    numerator_offset: i128,
    index_excess_offsets: Arc<Vec<i128>>,
}

impl PartialOrd for ShiftComplexityKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ShiftComplexityKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.policy
            .cmp(&other.policy)
            .then_with(|| self.arity.cmp(&other.arity))
            .then_with(|| self.sector.cmp(&other.sector))
            .then_with(|| {
                self.corner_distance_offset
                    .cmp(&other.corner_distance_offset)
            })
            .then_with(|| self.dot_offset.cmp(&other.dot_offset))
            .then_with(|| self.numerator_offset.cmp(&other.numerator_offset))
            .then_with(|| {
                self.policy.compare_coordinate_slices(
                    self.index_excess_offsets.as_slice(),
                    other.index_excess_offsets.as_slice(),
                )
            })
    }
}

impl ShiftComplexityKey {
    pub fn policy(&self) -> OrderingPolicy {
        self.policy
    }

    pub fn arity(&self) -> usize {
        self.arity
    }

    pub fn sector(&self) -> &Mask {
        &self.sector
    }

    pub fn corner_distance_offset(&self) -> i128 {
        self.corner_distance_offset
    }

    pub fn dot_offset(&self) -> i128 {
        self.dot_offset
    }

    pub fn numerator_offset(&self) -> i128 {
        self.numerator_offset
    }

    pub fn index_excess_offsets(&self) -> &[i128] {
        self.index_excess_offsets.as_slice()
    }

    /// Recover one original i64 shift coordinate from its exact excess offset.
    pub fn shift_at(&self, position: usize) -> Result<i64, Error> {
        let offset =
            self.index_excess_offsets
                .get(position)
                .copied()
                .ok_or(Error::IndexOutOfRange {
                    position,
                    arity: self.arity,
                })?;
        let shift = if self.sector.is_active(position)? {
            offset
        } else {
            offset.checked_neg().ok_or(Error::ComplexityOverflow {
                measure: "shift coordinate",
            })?
        };
        i64::try_from(shift).map_err(|_| Error::ComplexityOverflow {
            measure: "shift coordinate",
        })
    }

    pub(super) fn verifies_for_sector(&self, policy: OrderingPolicy, sector: &Mask) -> bool {
        if self.policy != policy
            || self.sector != *sector
            || self.arity != sector.arity()
            || self.index_excess_offsets.len() != self.arity
        {
            return false;
        }
        let mut dots = 0_i128;
        let mut numerators = 0_i128;
        for (&active, &offset) in self
            .sector
            .active_bits()
            .iter()
            .zip(self.index_excess_offsets.iter())
        {
            let target = if active { &mut dots } else { &mut numerators };
            let Some(sum) = target.checked_add(offset) else {
                return false;
            };
            *target = sum;
        }
        let Some(corner_distance) = dots.checked_add(numerators) else {
            return false;
        };
        dots == self.dot_offset
            && numerators == self.numerator_offset
            && corner_distance == self.corner_distance_offset
    }

    fn verifies_for(&self, policy: OrderingPolicy, domain: &SectorInteriorDomain) -> bool {
        if !self.verifies_for_sector(policy, domain.sector()) {
            return false;
        }
        for position in 0..self.arity {
            let Ok(shift) = self.shift_at(position) else {
                return false;
            };
            let bounds = domain.bounds()[position];
            let translated_lower = i128::from(bounds.lower()) + i128::from(shift);
            let translated_upper = i128::from(bounds.upper()) + i128::from(shift);
            let active = self.sector.active_bits()[position];
            let sector_lower = if active { 1_i64 } else { i64::MIN };
            let sector_upper = if active { i64::MAX } else { 0_i64 };
            if translated_lower < i128::from(sector_lower)
                || translated_upper > i128::from(sector_upper)
            {
                return false;
            }
        }
        true
    }
}

impl fmt::Display for ShiftComplexityKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}|arity={}|sector={}|corner-offset={}|dot-offset={}|numerator-offset={}|excess-offsets=[",
            self.policy.stable_id(),
            self.arity,
            self.sector,
            self.corner_distance_offset,
            self.dot_offset,
            self.numerator_offset,
        )?;
        for (position, offset) in self.index_excess_offsets.iter().enumerate() {
            if position != 0 {
                formatter.write_str(",")?;
            }
            write!(formatter, "{offset}")?;
        }
        formatter.write_str("]")
    }
}

/// Exact evidence that one shift is lower than another at every integer point
/// of a representable sector interior.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ShiftStrictDescentWitness {
    policy: OrderingPolicy,
    domain: SectorInteriorDomain,
    source: ShiftComplexityKey,
    target: ShiftComplexityKey,
    decisive_component: ComplexityComponent,
}

impl ShiftStrictDescentWitness {
    pub fn policy(&self) -> OrderingPolicy {
        self.policy
    }

    pub fn domain(&self) -> &SectorInteriorDomain {
        &self.domain
    }

    pub fn source(&self) -> &ShiftComplexityKey {
        &self.source
    }

    pub fn target(&self) -> &ShiftComplexityKey {
        &self.target
    }

    pub fn decisive_component(&self) -> ComplexityComponent {
        self.decisive_component
    }

    /// Recheck both universal domain inclusions and the exact structural key
    /// comparison without evaluating a particular anchor.
    pub fn verify(&self) -> bool {
        self.source.verifies_for(self.policy, &self.domain)
            && self.target.verifies_for(self.policy, &self.domain)
            && self.target < self.source
            && first_differing_component(&self.source, &self.target)
                == Some(self.decisive_component)
    }
}

impl OrderingPolicy {
    /// Construct the exact algebraic n-independent offset key for a shift and
    /// a fixed sector mask.
    ///
    /// This operation alone does not prove that the shift preserves the mask
    /// or remains i64-representable. Uniform comparison requires
    /// [`Self::compare_shifts_on_domain`], while strict descent requires
    /// [`Self::prove_shift_strict_descent`].
    pub fn shift_complexity_key(
        self,
        sector: &Mask,
        shift: &[i64],
    ) -> Result<ShiftComplexityKey, Error> {
        self.require_arity(sector.arity())?;
        if shift.len() != sector.arity() {
            return Err(Error::WrongArity {
                expected: sector.arity(),
                actual: shift.len(),
            });
        }
        let mut dots = 0_i128;
        let mut numerators = 0_i128;
        let mut index_excess_offsets = Vec::new();
        try_reserve_exact(
            &mut index_excess_offsets,
            shift.len(),
            "shift complexity index excess offsets",
        )?;
        for (&active, &shift) in sector.active_bits().iter().zip(shift) {
            let offset = if active {
                i128::from(shift)
            } else {
                -i128::from(shift)
            };
            index_excess_offsets.push(offset);
            let (target, measure) = if active {
                (&mut dots, "shift dot offset")
            } else {
                (&mut numerators, "shift numerator offset")
            };
            *target = target
                .checked_add(offset)
                .ok_or(Error::ComplexityOverflow { measure })?;
        }
        let corner_distance_offset =
            dots.checked_add(numerators)
                .ok_or(Error::ComplexityOverflow {
                    measure: "shift corner-distance offset",
                })?;
        Ok(ShiftComplexityKey {
            policy: self,
            arity: shift.len(),
            sector: sector.clone(),
            corner_distance_offset,
            dot_offset: dots,
            numerator_offset: numerators,
            index_excess_offsets: Arc::new(index_excess_offsets),
        })
    }

    /// Compare two shifts by the exact structural remainder of the v1 key on
    /// an interior that universally covers both shifts.
    pub fn compare_shifts_on_domain(
        self,
        domain: &SectorInteriorDomain,
        left: &[i64],
        right: &[i64],
    ) -> Result<Ordering, Error> {
        if left.len() != right.len() {
            return Err(Error::WrongArity {
                expected: left.len(),
                actual: right.len(),
            });
        }
        domain.require_shift(left)?;
        domain.require_shift(right)?;
        Ok(self
            .shift_complexity_key(domain.sector(), left)?
            .cmp(&self.shift_complexity_key(domain.sector(), right)?))
    }

    /// Prove strict structural descent uniformly over a checked interior.
    pub fn prove_shift_strict_descent(
        self,
        domain: &SectorInteriorDomain,
        source_shift: &[i64],
        target_shift: &[i64],
    ) -> Result<ShiftStrictDescentWitness, Error> {
        if source_shift.len() != target_shift.len() {
            return Err(Error::WrongArity {
                expected: source_shift.len(),
                actual: target_shift.len(),
            });
        }
        domain.require_shift(source_shift)?;
        domain.require_shift(target_shift)?;
        let source = self.shift_complexity_key(domain.sector(), source_shift)?;
        let target = self.shift_complexity_key(domain.sector(), target_shift)?;
        if target >= source {
            return Err(Error::NotStrictDescent);
        }
        let decisive_component =
            first_differing_component(&source, &target).ok_or(Error::NotStrictDescent)?;
        Ok(ShiftStrictDescentWitness {
            policy: self,
            domain: domain.clone(),
            source,
            target,
            decisive_component,
        })
    }
}

fn first_differing_component(
    source: &ShiftComplexityKey,
    target: &ShiftComplexityKey,
) -> Option<ComplexityComponent> {
    if source.arity != target.arity {
        return Some(ComplexityComponent::Arity);
    }
    if source.sector.active_count() != target.sector.active_count() {
        return Some(ComplexityComponent::PropagatorCount);
    }
    if source.sector != target.sector {
        return source
            .sector
            .active_bits()
            .iter()
            .zip(target.sector.active_bits())
            .position(|(left, right)| left != right)
            .map(|position| ComplexityComponent::SectorBit { position });
    }
    if source.corner_distance_offset != target.corner_distance_offset {
        return Some(ComplexityComponent::CornerDistance);
    }
    if source.dot_offset != target.dot_offset {
        return Some(ComplexityComponent::DotPower);
    }
    if source.numerator_offset != target.numerator_offset {
        return Some(ComplexityComponent::NumeratorPower);
    }
    source
        .policy
        .first_differing_coordinate(
            source.index_excess_offsets.as_slice(),
            target.index_excess_offsets.as_slice(),
        )
        .map(|position| ComplexityComponent::IndexExcess { position })
}
