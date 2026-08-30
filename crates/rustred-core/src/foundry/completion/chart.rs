use crate::family::IntegralKey;
use crate::sector::Mask;

use super::{CompletionGeometryError, LatticeBox, LatticePoint};

const INACTIVE_I64_MIN_COORDINATE: u64 = 1_u64 << 63;

/// Exact bijection between one declared sector and its representable part of
/// the nonnegative local-coordinate lattice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SectorChart {
    sector: Mask,
}

impl SectorChart {
    pub(crate) fn new(sector: Mask) -> Self {
        Self { sector }
    }

    pub(crate) fn sector(&self) -> &Mask {
        &self.sector
    }

    pub(crate) fn carrier_box(&self) -> Result<LatticeBox, CompletionGeometryError> {
        let mut lower = Vec::new();
        let mut upper = Vec::new();
        lower.try_reserve_exact(self.sector.arity()).map_err(|_| {
            CompletionGeometryError::AllocationFailure {
                resource: "sector-chart carrier lower endpoints",
                requested: self.sector.arity(),
            }
        })?;
        upper.try_reserve_exact(self.sector.arity()).map_err(|_| {
            CompletionGeometryError::AllocationFailure {
                resource: "sector-chart carrier upper endpoints",
                requested: self.sector.arity(),
            }
        })?;
        for &active in self.sector.active_bits() {
            lower.push(0);
            upper.push(Some(if active {
                i64::MAX as u64 - 1
            } else {
                INACTIVE_I64_MIN_COORDINATE
            }));
        }
        LatticeBox::try_from_preallocated(lower, upper)
    }

    pub(crate) fn to_lattice(
        &self,
        integral: &IntegralKey,
    ) -> Result<LatticePoint, CompletionGeometryError> {
        self.check_arity("integral key", integral.powers().len())?;
        let mut coordinates = Vec::new();
        coordinates
            .try_reserve_exact(self.sector.arity())
            .map_err(|_| CompletionGeometryError::AllocationFailure {
                resource: "sector-chart lattice coordinates",
                requested: self.sector.arity(),
            })?;
        for (position, (&power, &active)) in integral
            .powers()
            .iter()
            .zip(self.sector.active_bits())
            .enumerate()
        {
            let coordinate = if active {
                if power < 1 {
                    return Err(CompletionGeometryError::IntegralOutsideSector {
                        position,
                        power,
                        active,
                    });
                }
                (power - 1) as u64
            } else {
                if power > 0 {
                    return Err(CompletionGeometryError::IntegralOutsideSector {
                        position,
                        power,
                        active,
                    });
                }
                power.unsigned_abs()
            };
            coordinates.push(coordinate);
        }
        LatticePoint::try_from_preallocated(coordinates)
    }

    pub(crate) fn to_integral(
        &self,
        point: &LatticePoint,
    ) -> Result<IntegralKey, CompletionGeometryError> {
        self.check_arity("lattice point", point.arity())?;
        let mut powers = Vec::new();
        powers.try_reserve_exact(self.sector.arity()).map_err(|_| {
            CompletionGeometryError::AllocationFailure {
                resource: "sector-chart integral powers",
                requested: self.sector.arity(),
            }
        })?;
        for (position, (&coordinate, &active)) in point
            .coordinates()
            .iter()
            .zip(self.sector.active_bits())
            .enumerate()
        {
            let power = if active {
                let coordinate = i64::try_from(coordinate).map_err(|_| {
                    CompletionGeometryError::CoordinateNotRepresentable {
                        position,
                        coordinate,
                        active,
                    }
                })?;
                coordinate.checked_add(1).ok_or(
                    CompletionGeometryError::CoordinateNotRepresentable {
                        position,
                        coordinate: coordinate as u64,
                        active,
                    },
                )?
            } else if coordinate == INACTIVE_I64_MIN_COORDINATE {
                i64::MIN
            } else {
                let coordinate = i64::try_from(coordinate).map_err(|_| {
                    CompletionGeometryError::CoordinateNotRepresentable {
                        position,
                        coordinate,
                        active,
                    }
                })?;
                -coordinate
            };
            powers.push(power);
        }
        IntegralKey::try_from_preallocated(powers).map_err(|_| CompletionGeometryError::Invariant {
            detail: "a nonempty sector chart produced an empty integral key",
        })
    }

    fn check_arity(
        &self,
        object: &'static str,
        actual: usize,
    ) -> Result<(), CompletionGeometryError> {
        let expected = self.sector.arity();
        if actual == expected {
            Ok(())
        } else {
            Err(CompletionGeometryError::WrongArity {
                object,
                expected,
                actual,
            })
        }
    }
}
