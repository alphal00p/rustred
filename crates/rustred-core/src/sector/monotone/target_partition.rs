use std::sync::Arc;

use super::SectorMonotoneShiftDescentWitness;
use crate::sector::{Error, InteriorBounds, Mask, SectorInteriorDomain};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum CoordinatePartition {
    FixedActive,
    FixedInactive,
    AlwaysPinched,
    Optional {
        choice_ordinal: usize,
        pinched_upper: i64,
        active_lower: i64,
    },
}

/// Whether one exact target-sector cell stays in the parent sector or enters
/// a fixed proper subsector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SectorMonotoneTargetCellKind {
    SameSector,
    ProperSubsector,
}

/// Allocation-free structural census of one exact target-sector partition.
///
/// Callers must admit aggregate cell work before iterating. A six-loop index
/// space can describe millions of cells while retaining only O(K) partition
/// metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SectorMonotoneTargetPartitionCensus {
    optional_coordinate_count: usize,
    cell_count: usize,
    proper_subsector_cell_count: usize,
}

impl SectorMonotoneTargetPartitionCensus {
    pub fn optional_coordinate_count(self) -> usize {
        self.optional_coordinate_count
    }

    pub fn cell_count(self) -> usize {
        self.cell_count
    }

    pub fn proper_subsector_cell_count(self) -> usize {
        self.proper_subsector_cell_count
    }
}

/// One exact orthogonal cell of a sector-monotone RHS shift.
///
/// `base_domain` is expressed in the recurrence variables `n`.
/// `pivot_domain` is its image under the rule pivot shift, and
/// `target_domain` is its image under this RHS shift. Both image masks are
/// fixed throughout the cell. A proper-subsector cell is therefore a usable
/// geometric component of a symbolic dependency key; unlike a first-pinched
/// cylinder, it cannot hide additional target-sector changes in later
/// coordinates. Family/context identity and coefficient/guard applicability
/// belong to the foundry obligation, not this sector value.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SectorMonotoneTargetCell {
    ordinal: usize,
    base_domain: SectorInteriorDomain,
    pivot_domain: SectorInteriorDomain,
    target_domain: SectorInteriorDomain,
}

impl SectorMonotoneTargetCell {
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub fn base_domain(&self) -> &SectorInteriorDomain {
        &self.base_domain
    }

    pub fn pivot_domain(&self) -> &SectorInteriorDomain {
        &self.pivot_domain
    }

    pub fn target_domain(&self) -> &SectorInteriorDomain {
        &self.target_domain
    }

    pub fn kind(&self) -> SectorMonotoneTargetCellKind {
        if self.pivot_domain.sector() == self.target_domain.sector() {
            SectorMonotoneTargetCellKind::SameSector
        } else {
            SectorMonotoneTargetCellKind::ProperSubsector
        }
    }

    pub fn pinched_positions(&self) -> impl Iterator<Item = usize> + '_ {
        self.pivot_domain
            .sector()
            .active_bits()
            .iter()
            .zip(self.target_domain.sector().active_bits())
            .enumerate()
            .filter_map(|(position, (&pivot, &target))| (pivot && !target).then_some(position))
    }
}

/// Exact finite product partition of one term-local sector-monotone shift.
///
/// Only coordinates whose translated interval straddles zero contribute a
/// binary choice. The partition stores O(K) metadata and exposes cells lazily;
/// constructing it never allocates or enumerates its potentially exponential
/// cell set. Optional coordinates are ordered by denominator position with
/// the inactive branch before the active branch, so the all-same-sector cell
/// (when present) is last.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SectorMonotoneTargetPartition {
    witness: SectorMonotoneShiftDescentWitness,
    pivot_shift: Arc<Vec<i64>>,
    target_shift: Arc<Vec<i64>>,
    coordinates: Arc<Vec<CoordinatePartition>>,
    optional_coordinate_count: usize,
    cell_count: usize,
    proper_subsector_cell_count: usize,
}

impl SectorMonotoneTargetPartition {
    fn try_new(witness: &SectorMonotoneShiftDescentWitness) -> Result<Self, Error> {
        let census = target_partition_census(witness)?;
        let pivot_shift = Arc::new(copy_shift(
            witness.pivot(),
            "sector-monotone partition pivot shift",
        )?);
        let target_shift = Arc::new(copy_shift(
            witness.target(),
            "sector-monotone partition target shift",
        )?);
        let mut coordinates = Vec::new();
        super::super::error::try_reserve_exact(
            &mut coordinates,
            witness.domain().arity(),
            "sector-monotone target coordinate partitions",
        )?;
        let mut optional_coordinate_count = 0usize;
        for (position, ((&bounds, &active), &shift)) in witness
            .domain()
            .bounds()
            .iter()
            .zip(witness.domain().sector().active_bits())
            .zip(target_shift.iter())
            .enumerate()
        {
            let coordinate = if !active {
                if shift > 0 {
                    return Err(Error::InactiveLineActivation { position, shift });
                }
                CoordinatePartition::FixedInactive
            } else if shift >= 0 {
                CoordinatePartition::FixedActive
            } else {
                let pinched_upper = -i128::from(shift);
                if pinched_upper < i128::from(bounds.lower()) {
                    CoordinatePartition::FixedActive
                } else if pinched_upper >= i128::from(bounds.upper()) {
                    CoordinatePartition::AlwaysPinched
                } else {
                    let pinched_upper =
                        i64::try_from(pinched_upper).map_err(|_| Error::ComplexityOverflow {
                            measure: "target-sector partition pinch threshold",
                        })?;
                    let active_lower =
                        pinched_upper
                            .checked_add(1)
                            .ok_or(Error::ComplexityOverflow {
                                measure: "target-sector partition active threshold",
                            })?;
                    let choice_ordinal = optional_coordinate_count;
                    optional_coordinate_count = optional_coordinate_count.checked_add(1).ok_or(
                        Error::ComplexityOverflow {
                            measure: "target-sector optional coordinate count",
                        },
                    )?;
                    CoordinatePartition::Optional {
                        choice_ordinal,
                        pinched_upper,
                        active_lower,
                    }
                }
            };
            coordinates.push(coordinate);
        }
        debug_assert_eq!(optional_coordinate_count, census.optional_coordinate_count);
        Ok(Self {
            witness: witness.clone(),
            pivot_shift,
            target_shift,
            coordinates: Arc::new(coordinates),
            optional_coordinate_count: census.optional_coordinate_count,
            cell_count: census.cell_count,
            proper_subsector_cell_count: census.proper_subsector_cell_count,
        })
    }

    pub fn witness(&self) -> &SectorMonotoneShiftDescentWitness {
        &self.witness
    }

    pub fn optional_coordinate_count(&self) -> usize {
        self.optional_coordinate_count
    }

    pub fn cell_count(&self) -> usize {
        self.cell_count
    }

    pub fn proper_subsector_cell_count(&self) -> usize {
        self.proper_subsector_cell_count
    }

    /// Classify one stable cell ordinal without materializing its domains.
    pub fn cell_kind(&self, ordinal: usize) -> Result<SectorMonotoneTargetCellKind, Error> {
        if ordinal >= self.cell_count {
            return Err(Error::TargetSectorCellOutOfRange {
                ordinal,
                cell_count: self.cell_count,
            });
        }
        Ok(if ordinal < self.proper_subsector_cell_count {
            SectorMonotoneTargetCellKind::ProperSubsector
        } else {
            SectorMonotoneTargetCellKind::SameSector
        })
    }

    /// Number of proper-subsector cells strictly before a resume ordinal.
    /// `cell_count` itself is the valid exhausted cursor.
    pub fn proper_subsector_cell_count_before(&self, next_ordinal: usize) -> Result<usize, Error> {
        if next_ordinal > self.cell_count {
            return Err(Error::TargetSectorCellOutOfRange {
                ordinal: next_ordinal,
                cell_count: self.cell_count,
            });
        }
        Ok(next_ordinal.min(self.proper_subsector_cell_count))
    }

    pub fn cells(&self) -> SectorMonotoneTargetCells<'_> {
        SectorMonotoneTargetCells {
            partition: self,
            next_ordinal: 0,
        }
    }

    /// Resume lazy materialization at one stable cell ordinal. `cell_count`
    /// itself is the valid exhausted cursor.
    pub fn cells_from(&self, next_ordinal: usize) -> Result<SectorMonotoneTargetCells<'_>, Error> {
        if next_ordinal > self.cell_count {
            return Err(Error::TargetSectorCellOutOfRange {
                ordinal: next_ordinal,
                cell_count: self.cell_count,
            });
        }
        Ok(SectorMonotoneTargetCells {
            partition: self,
            next_ordinal,
        })
    }

    pub fn cell(&self, ordinal: usize) -> Result<SectorMonotoneTargetCell, Error> {
        self.cell_kind(ordinal)?;
        let domain = self.witness.domain();
        let mut base_bounds = Vec::new();
        let mut target_sector_bits = Vec::new();
        super::super::error::try_reserve_exact(
            &mut base_bounds,
            domain.arity(),
            "sector-monotone target-cell base bounds",
        )?;
        super::super::error::try_reserve_exact(
            &mut target_sector_bits,
            domain.arity(),
            "sector-monotone target-cell sector bits",
        )?;
        for ((&bounds, &active), coordinate) in domain
            .bounds()
            .iter()
            .zip(domain.sector().active_bits())
            .zip(self.coordinates.iter())
        {
            let (bounds, target_active) = match *coordinate {
                CoordinatePartition::FixedActive => (bounds, true),
                CoordinatePartition::FixedInactive => (bounds, false),
                CoordinatePartition::AlwaysPinched => (bounds, false),
                CoordinatePartition::Optional {
                    choice_ordinal,
                    pinched_upper,
                    active_lower,
                } => {
                    let pinched = ((ordinal >> choice_ordinal) & 1) == 0;
                    if pinched {
                        (InteriorBounds::new(bounds.lower(), pinched_upper), false)
                    } else {
                        (InteriorBounds::new(active_lower, bounds.upper()), true)
                    }
                }
            };
            base_bounds.push(bounds);
            target_sector_bits.push(target_active && active);
        }
        let base_domain =
            SectorInteriorDomain::try_from_preallocated(domain.sector().clone(), base_bounds)?;
        let pivot_domain = translate_domain(
            &base_domain,
            domain.sector().clone(),
            self.pivot_shift.as_slice(),
            "sector-monotone target-cell pivot bounds",
        )?;
        let target_sector = Mask::try_from_preallocated(target_sector_bits)?;
        let target_domain = translate_domain(
            &base_domain,
            target_sector,
            self.target_shift.as_slice(),
            "sector-monotone target-cell target bounds",
        )?;
        Ok(SectorMonotoneTargetCell {
            ordinal,
            base_domain,
            pivot_domain,
            target_domain,
        })
    }

    /// Cold-path replay of the complete O(K) partition construction.
    /// Allocation or arithmetic failures remain typed instead of being
    /// misreported as a false mathematical witness.
    pub fn try_verify(&self) -> Result<bool, Error> {
        Ok(Self::try_new(&self.witness)? == *self)
    }

    /// Rebuild one ordinal and compare its exact base/pivot/target domains.
    pub fn try_verifies_cell(&self, cell: &SectorMonotoneTargetCell) -> Result<bool, Error> {
        Ok(self.cell(cell.ordinal)? == *cell)
    }
}

impl SectorMonotoneShiftDescentWitness {
    /// Count fixed-target-sector cells without retaining partition metadata or
    /// enumerating any cell.
    pub fn target_sector_partition_census(
        &self,
    ) -> Result<SectorMonotoneTargetPartitionCensus, Error> {
        target_partition_census(self)
    }

    /// Refine this compact first-pinched proof into a lazy exact partition
    /// whose every cell has one fixed target-sector mask.
    pub fn try_target_sector_partition(&self) -> Result<SectorMonotoneTargetPartition, Error> {
        SectorMonotoneTargetPartition::try_new(self)
    }
}

/// Lazy stable-ordinal iterator over exact target-sector cells.
pub struct SectorMonotoneTargetCells<'a> {
    partition: &'a SectorMonotoneTargetPartition,
    next_ordinal: usize,
}

impl SectorMonotoneTargetCells<'_> {
    /// Stable ordinal of the next cell, or `cell_count` after exhaustion.
    pub fn next_ordinal(&self) -> usize {
        self.next_ordinal
    }

    /// Exact remaining work count for explicit caller-side admission. This is
    /// deliberately not an iterator upper size hint, so generic collection
    /// cannot eagerly reserve an exponential cell buffer.
    pub fn remaining_cell_count(&self) -> usize {
        self.partition.cell_count - self.next_ordinal
    }
}

impl Iterator for SectorMonotoneTargetCells<'_> {
    type Item = Result<SectorMonotoneTargetCell, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_ordinal == self.partition.cell_count {
            return None;
        }
        let ordinal = self.next_ordinal;
        self.next_ordinal += 1;
        Some(self.partition.cell(ordinal))
    }
}

fn target_partition_census(
    witness: &SectorMonotoneShiftDescentWitness,
) -> Result<SectorMonotoneTargetPartitionCensus, Error> {
    if !witness.verify() {
        return Err(Error::NotStrictDescent);
    }
    let mut optional_coordinate_count = 0usize;
    let mut has_always_pinched_coordinate = false;
    for position in 0..witness.domain().arity() {
        let bounds = witness.domain().bounds()[position];
        let active = witness.domain().sector().active_bits()[position];
        let shift = witness.target().shift_at(position)?;
        if !active {
            if shift > 0 {
                return Err(Error::InactiveLineActivation { position, shift });
            }
            continue;
        }
        if shift >= 0 {
            continue;
        }
        let pinched_upper = -i128::from(shift);
        if pinched_upper < i128::from(bounds.lower()) {
            continue;
        }
        if pinched_upper >= i128::from(bounds.upper()) {
            has_always_pinched_coordinate = true;
        } else {
            optional_coordinate_count =
                optional_coordinate_count
                    .checked_add(1)
                    .ok_or(Error::ComplexityOverflow {
                        measure: "target-sector optional coordinate count",
                    })?;
        }
    }
    let shift_width =
        u32::try_from(optional_coordinate_count).map_err(|_| Error::ComplexityOverflow {
            measure: "target-sector cell count",
        })?;
    let cell_count = 1usize
        .checked_shl(shift_width)
        .ok_or(Error::ComplexityOverflow {
            measure: "target-sector cell count",
        })?;
    let proper_subsector_cell_count = if has_always_pinched_coordinate {
        cell_count
    } else {
        cell_count
            .checked_sub(1)
            .expect("one binary product cell exists")
    };
    Ok(SectorMonotoneTargetPartitionCensus {
        optional_coordinate_count,
        cell_count,
        proper_subsector_cell_count,
    })
}

fn copy_shift(
    key: &crate::sector::ShiftComplexityKey,
    resource: &'static str,
) -> Result<Vec<i64>, Error> {
    let mut shift = Vec::new();
    super::super::error::try_reserve_exact(&mut shift, key.arity(), resource)?;
    for position in 0..key.arity() {
        shift.push(key.shift_at(position)?);
    }
    Ok(shift)
}

fn translate_domain(
    base: &SectorInteriorDomain,
    target_sector: Mask,
    shift: &[i64],
    resource: &'static str,
) -> Result<SectorInteriorDomain, Error> {
    let mut bounds = Vec::new();
    super::super::error::try_reserve_exact(&mut bounds, base.arity(), resource)?;
    for (&bounds_at_position, &shift) in base.bounds().iter().zip(shift) {
        let lower =
            bounds_at_position
                .lower()
                .checked_add(shift)
                .ok_or(Error::ComplexityOverflow {
                    measure: "target-sector translated lower bound",
                })?;
        let upper =
            bounds_at_position
                .upper()
                .checked_add(shift)
                .ok_or(Error::ComplexityOverflow {
                    measure: "target-sector translated upper bound",
                })?;
        bounds.push(InteriorBounds::new(lower, upper));
    }
    SectorInteriorDomain::try_from_preallocated(target_sector, bounds)
}
